// Copyright (c) 2025 Rajan Panneer Selvam
//
// Licensed under the Business Source License 1.1 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     https://www.mariadb.com/bsl11
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use kube::{
    api::{Api, DynamicObject, ListParams},
    discovery::{verbs, ApiCapabilities, ApiGroup, ApiResource, Discovery, Scope},
    Client,
};
use serde_json::{json, Value};

use crate::errors::AppError;
use crate::models::cluster::KubernetesClusterConfig;
use crate::services::kubernetes::client::ClientFactory;
use crate::services::kubernetes::vpa_inventory::{
    VpaConditionInventoryItem, VpaContainerPolicyInventoryItem, VpaInventoryItem,
};

const VPA_API_GROUP: &str = "autoscaling.k8s.io";
const VPA_KIND: &str = "VerticalPodAutoscaler";
const VPA_PLURAL: &str = "verticalpodautoscalers";

pub struct VerticalPodAutoscalerService;

impl VerticalPodAutoscalerService {
    pub fn new() -> Self {
        Self
    }

    pub async fn list_inventory(
        &self,
        cluster: &KubernetesClusterConfig,
        cluster_id: &str,
        namespace: Option<&str>,
    ) -> Result<Vec<VpaInventoryItem>, AppError> {
        let client = ClientFactory::get_client(cluster).await?;
        let discovery = Discovery::new(client.clone())
            .filter(&[VPA_API_GROUP])
            .run()
            .await
            .map_err(|e| AppError::ExternalService(format!("VPA discovery failed: {}", e)))?;

        let Some(group) = discovery.get(VPA_API_GROUP) else {
            return Ok(Vec::new());
        };
        let Some((resource, capabilities)) = resolve_vpa_resource(group) else {
            return Ok(Vec::new());
        };
        if !capabilities.supports_operation(verbs::LIST) {
            return Ok(Vec::new());
        }

        let namespace = namespace
            .map(str::trim)
            .filter(|namespace| !namespace.is_empty());
        let fallback_namespace = namespace
            .filter(|namespace| *namespace != "all")
            .unwrap_or("");
        let collected_at = Utc::now();
        let mut inventory = list_dynamic_vpas(
            &client,
            &resource,
            &capabilities,
            cluster_id,
            namespace,
            fallback_namespace,
            collected_at,
        )
        .await?;

        inventory.sort_by(|left, right| {
            (left.namespace.as_str(), left.name.as_str())
                .cmp(&(right.namespace.as_str(), right.name.as_str()))
        });
        Ok(inventory)
    }
}

fn resolve_vpa_resource(group: &ApiGroup) -> Option<(ApiResource, ApiCapabilities)> {
    group
        .resources_by_stability()
        .into_iter()
        .find(|(resource, _)| {
            resource.group == VPA_API_GROUP
                && resource.kind == VPA_KIND
                && resource.plural == VPA_PLURAL
        })
}

async fn list_dynamic_vpas(
    client: &Client,
    resource: &ApiResource,
    capabilities: &ApiCapabilities,
    cluster_id: &str,
    namespace: Option<&str>,
    fallback_namespace: &str,
    collected_at: DateTime<Utc>,
) -> Result<Vec<VpaInventoryItem>, AppError> {
    let api: Api<DynamicObject> = match namespace {
        Some(namespace) if namespace != "all" && capabilities.scope == Scope::Namespaced => {
            Api::namespaced_with(client.clone(), namespace, resource)
        }
        _ => Api::all_with(client.clone(), resource),
    };

    let list = api.list(&ListParams::default()).await.map_err(|e| {
        AppError::ExternalService(format!("Failed to list VerticalPodAutoscalers: {}", e))
    })?;

    Ok(list
        .items
        .into_iter()
        .map(|item| {
            convert_dynamic_vpa_to_inventory(
                item,
                cluster_id,
                resource,
                fallback_namespace,
                collected_at,
            )
        })
        .collect())
}

fn convert_dynamic_vpa_to_inventory(
    item: DynamicObject,
    cluster_id: &str,
    resource: &ApiResource,
    fallback_namespace: &str,
    collected_at: DateTime<Utc>,
) -> VpaInventoryItem {
    let spec = object_field(&item.data, "spec");
    let status = object_field(&item.data, "status");
    let target_ref = object_field(&spec, "targetRef");
    let update_policy = object_field(&spec, "updatePolicy");
    let resource_policy = object_field(&spec, "resourcePolicy");
    let recommendation = object_field(&status, "recommendation");
    let api_version = item
        .types
        .as_ref()
        .map(|type_meta| type_meta.api_version.clone())
        .unwrap_or_else(|| resource.api_version.clone());

    VpaInventoryItem {
        cluster_id: cluster_id.to_string(),
        namespace: item
            .metadata
            .namespace
            .clone()
            .unwrap_or_else(|| fallback_namespace.to_string()),
        name: item.metadata.name.clone().unwrap_or_default(),
        api_version,
        labels: item.metadata.labels.clone().unwrap_or_default(),
        annotations: item.metadata.annotations.clone().unwrap_or_default(),
        target_api_version: string_field(&target_ref, "apiVersion"),
        target_kind: string_field(&target_ref, "kind"),
        target_name: string_field(&target_ref, "name"),
        update_mode: string_field(&update_policy, "updateMode"),
        recommendation_container_count: recommendation
            .get("containerRecommendations")
            .and_then(Value::as_array)
            .map(Vec::len)
            .unwrap_or(0),
        container_policies: container_policies_from_resource_policy(&resource_policy),
        conditions: conditions_from_status(&status),
        spec,
        status,
        created_at: item
            .metadata
            .creation_timestamp
            .as_ref()
            .map(|timestamp| timestamp.0),
        collected_at,
    }
}

fn object_field(value: &Value, key: &str) -> Value {
    value.get(key).cloned().unwrap_or_else(|| json!({}))
}

fn string_field(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn string_array_field(value: &Value, key: &str) -> Vec<String> {
    value
        .get(key)
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::trim)
                .filter(|item| !item.is_empty())
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

fn string_map_field(value: &Value, key: &str) -> BTreeMap<String, String> {
    value
        .get(key)
        .and_then(Value::as_object)
        .map(|map| {
            map.iter()
                .filter_map(|(key, value)| {
                    value
                        .as_str()
                        .map(str::trim)
                        .filter(|value| !value.is_empty())
                        .map(|value| (key.clone(), value.to_string()))
                })
                .collect()
        })
        .unwrap_or_default()
}

fn container_policies_from_resource_policy(
    resource_policy: &Value,
) -> Vec<VpaContainerPolicyInventoryItem> {
    resource_policy
        .get("containerPolicies")
        .and_then(Value::as_array)
        .map(|policies| {
            policies
                .iter()
                .filter_map(container_policy_from_value)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

fn container_policy_from_value(policy: &Value) -> Option<VpaContainerPolicyInventoryItem> {
    if !policy.is_object() {
        return None;
    }

    Some(VpaContainerPolicyInventoryItem {
        container_name: string_field(policy, "containerName"),
        mode: string_field(policy, "mode"),
        controlled_resources: string_array_field(policy, "controlledResources"),
        min_allowed: string_map_field(policy, "minAllowed"),
        max_allowed: string_map_field(policy, "maxAllowed"),
    })
}

fn conditions_from_status(status: &Value) -> Vec<VpaConditionInventoryItem> {
    status
        .get("conditions")
        .and_then(Value::as_array)
        .map(|conditions| {
            conditions
                .iter()
                .filter_map(condition_from_value)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

fn condition_from_value(condition: &Value) -> Option<VpaConditionInventoryItem> {
    Some(VpaConditionInventoryItem {
        type_: string_field(condition, "type")?,
        status: string_field(condition, "status")?,
        reason: string_field(condition, "reason"),
        message: string_field(condition, "message"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use k8s_openapi::apimachinery::pkg::apis::meta::v1::Time;
    use kube::core::GroupVersionKind;

    fn map(values: &[(&str, &str)]) -> BTreeMap<String, String> {
        values
            .iter()
            .map(|(key, value)| ((*key).to_string(), (*value).to_string()))
            .collect()
    }

    fn vpa_resource() -> ApiResource {
        ApiResource::from_gvk_with_plural(
            &GroupVersionKind::gvk(VPA_API_GROUP, "v1", VPA_KIND),
            VPA_PLURAL,
        )
    }

    #[test]
    fn vpa_inventory_conversion_preserves_metadata_policy_and_status() {
        let resource = vpa_resource();
        let created_at = Utc.with_ymd_and_hms(2026, 6, 1, 12, 0, 0).unwrap();
        let collected_at = Utc.with_ymd_and_hms(2026, 6, 10, 0, 0, 0).unwrap();
        let mut vpa = DynamicObject::new("checkout-vpa", &resource)
            .within("apps")
            .data(json!({
                "spec": {
                    "targetRef": {
                        "apiVersion": "apps/v1",
                        "kind": "Deployment",
                        "name": "checkout"
                    },
                    "updatePolicy": {
                        "updateMode": "Auto"
                    },
                    "resourcePolicy": {
                        "containerPolicies": [{
                            "containerName": "*",
                            "mode": "Auto",
                            "controlledResources": ["cpu", "memory"],
                            "minAllowed": {
                                "cpu": "100m",
                                "memory": "128Mi"
                            },
                            "maxAllowed": {
                                "cpu": "2",
                                "memory": "2Gi"
                            }
                        }]
                    }
                },
                "status": {
                    "recommendation": {
                        "containerRecommendations": [{
                            "containerName": "api"
                        }]
                    },
                    "conditions": [{
                        "type": "RecommendationProvided",
                        "status": "True"
                    }]
                }
            }));
        vpa.metadata.labels = Some(map(&[("team", "payments")]));
        vpa.metadata.annotations = Some(map(&[("cost-center", "cc-12")]));
        vpa.metadata.creation_timestamp = Some(Time(created_at));

        let item =
            convert_dynamic_vpa_to_inventory(vpa, "cluster-a", &resource, "fallback", collected_at);

        assert_eq!(item.cluster_id, "cluster-a");
        assert_eq!(item.namespace, "apps");
        assert_eq!(item.name, "checkout-vpa");
        assert_eq!(item.api_version, "autoscaling.k8s.io/v1");
        assert_eq!(item.labels["team"], "payments");
        assert_eq!(item.annotations["cost-center"], "cc-12");
        assert_eq!(item.created_at, Some(created_at));
        assert_eq!(item.collected_at, collected_at);
        assert_eq!(item.target_api_version, Some("apps/v1".to_string()));
        assert_eq!(item.target_kind, Some("Deployment".to_string()));
        assert_eq!(item.target_name, Some("checkout".to_string()));
        assert_eq!(item.update_mode, Some("Auto".to_string()));
        assert_eq!(item.recommendation_container_count, 1);
        assert_eq!(item.container_policies.len(), 1);
        assert_eq!(
            item.container_policies[0].container_name,
            Some("*".to_string())
        );
        assert_eq!(
            item.container_policies[0].controlled_resources,
            vec!["cpu".to_string(), "memory".to_string()]
        );
        assert_eq!(item.container_policies[0].min_allowed["cpu"], "100m");
        assert_eq!(item.container_policies[0].max_allowed["memory"], "2Gi");
        assert_eq!(item.conditions.len(), 1);
        assert_eq!(item.conditions[0].type_, "RecommendationProvided");
    }

    #[test]
    fn vpa_inventory_conversion_uses_fallback_namespace_when_missing() {
        let resource = vpa_resource();
        let collected_at = Utc.with_ymd_and_hms(2026, 6, 10, 0, 0, 0).unwrap();
        let vpa = DynamicObject::new("fallback-vpa", &resource).data(json!({
            "spec": {
                "targetRef": {
                    "kind": "Deployment",
                    "name": "checkout"
                }
            }
        }));

        let item = convert_dynamic_vpa_to_inventory(
            vpa,
            "cluster-a",
            &resource,
            "requested-namespace",
            collected_at,
        );

        assert_eq!(item.namespace, "requested-namespace");
        assert_eq!(item.target_kind, Some("Deployment".to_string()));
        assert_eq!(item.target_name, Some("checkout".to_string()));
        assert_eq!(item.update_mode, None);
        assert!(item.container_policies.is_empty());
    }
}

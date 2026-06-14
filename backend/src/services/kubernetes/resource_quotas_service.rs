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

use crate::errors::AppError;
use crate::models::cluster::KubernetesClusterConfig;
use crate::services::kubernetes::client::ClientFactory;
use crate::services::kubernetes::resource_quota_inventory::{
    ResourceQuotaInventoryItem, ResourceQuotaScopeSelectorInventoryItem,
};
use chrono::{DateTime, Utc};
use k8s_openapi::api::core::v1::ResourceQuota;
use k8s_openapi::apimachinery::pkg::api::resource::Quantity;
use kube::api::{DeleteParams, ListParams, Patch, PatchParams};
use kube::{Api, ResourceExt};
use std::collections::BTreeMap;

pub struct ResourceQuotasService;

fn quantity_map_to_strings(map: &BTreeMap<String, Quantity>) -> BTreeMap<String, String> {
    map.iter()
        .map(|(key, value)| (key.clone(), value.0.clone()))
        .collect()
}

fn convert_kube_resource_quota_to_inventory(
    quota: &ResourceQuota,
    cluster_id: &str,
    current_namespace: &str,
    collected_at: DateTime<Utc>,
) -> ResourceQuotaInventoryItem {
    let namespace = quota
        .namespace()
        .unwrap_or_else(|| current_namespace.to_string());
    let spec = quota.spec.as_ref();
    let status = quota.status.as_ref();
    let hard = status
        .and_then(|status| status.hard.as_ref())
        .map(quantity_map_to_strings)
        .or_else(|| {
            spec.and_then(|spec| spec.hard.as_ref())
                .map(quantity_map_to_strings)
        })
        .unwrap_or_default();
    let used = status
        .and_then(|status| status.used.as_ref())
        .map(quantity_map_to_strings)
        .unwrap_or_default();
    let scopes = spec
        .and_then(|spec| spec.scopes.clone())
        .unwrap_or_default();
    let scope_selector = spec
        .and_then(|spec| spec.scope_selector.as_ref())
        .and_then(|selector| selector.match_expressions.as_ref())
        .map(|expressions| {
            expressions
                .iter()
                .map(|expression| ResourceQuotaScopeSelectorInventoryItem {
                    scope_name: expression.scope_name.clone(),
                    operator: expression.operator.clone(),
                    values: expression.values.clone().unwrap_or_default(),
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    ResourceQuotaInventoryItem {
        cluster_id: cluster_id.to_string(),
        namespace,
        name: quota.name_any(),
        labels: quota.metadata.labels.clone().unwrap_or_default(),
        annotations: quota.metadata.annotations.clone().unwrap_or_default(),
        hard,
        used,
        scopes,
        scope_selector,
        created_at: quota
            .metadata
            .creation_timestamp
            .as_ref()
            .map(|timestamp| timestamp.0),
        collected_at,
    }
}

impl ResourceQuotasService {
    pub fn new() -> Self {
        Self
    }

    async fn api(
        cluster: &KubernetesClusterConfig,
        namespace: &str,
    ) -> Result<Api<ResourceQuota>, AppError> {
        let client = ClientFactory::get_client(cluster).await?;
        Ok(if namespace.is_empty() || namespace == "all" {
            Api::all(client)
        } else {
            Api::namespaced(client, namespace)
        })
    }

    pub async fn list(
        &self,
        cluster: &KubernetesClusterConfig,
        namespace: &str,
    ) -> Result<Vec<ResourceQuota>, AppError> {
        let api = Self::api(cluster, namespace).await?;
        let list = api
            .list(&ListParams::default())
            .await
            .map_err(|e| AppError::Kubernetes(e.to_string()))?;
        Ok(list.items)
    }

    pub async fn list_inventory(
        &self,
        cluster: &KubernetesClusterConfig,
        cluster_id: &str,
        namespace: Option<&str>,
    ) -> Result<Vec<ResourceQuotaInventoryItem>, AppError> {
        let namespace = namespace
            .map(str::trim)
            .filter(|namespace| !namespace.is_empty());
        let namespace_arg = namespace.unwrap_or("");
        let fallback_namespace = namespace
            .filter(|namespace| *namespace != "all")
            .unwrap_or("");
        let collected_at = Utc::now();

        let api = Self::api(cluster, namespace_arg).await?;
        let list = api
            .list(&ListParams::default())
            .await
            .map_err(|e| AppError::Kubernetes(e.to_string()))?;
        let mut inventory = list
            .items
            .iter()
            .map(|quota| {
                convert_kube_resource_quota_to_inventory(
                    quota,
                    cluster_id,
                    fallback_namespace,
                    collected_at,
                )
            })
            .collect::<Vec<_>>();
        inventory.sort_by(|left, right| {
            (left.namespace.as_str(), left.name.as_str())
                .cmp(&(right.namespace.as_str(), right.name.as_str()))
        });
        Ok(inventory)
    }

    pub async fn get(
        &self,
        cluster: &KubernetesClusterConfig,
        namespace: &str,
        name: &str,
    ) -> Result<ResourceQuota, AppError> {
        let api: Api<ResourceQuota> =
            Api::namespaced(ClientFactory::get_client(cluster).await?, namespace);
        api.get(name)
            .await
            .map_err(|e| AppError::Kubernetes(e.to_string()))
    }

    pub async fn upsert(
        &self,
        cluster: &KubernetesClusterConfig,
        namespace: &str,
        item: &ResourceQuota,
    ) -> Result<ResourceQuota, AppError> {
        let api: Api<ResourceQuota> =
            Api::namespaced(ClientFactory::get_client(cluster).await?, namespace);
        let params = PatchParams::apply("mayyam").force();
        api.patch(
            item.metadata.name.as_ref().ok_or_else(|| {
                AppError::BadRequest("ResourceQuota.metadata.name required".into())
            })?,
            &params,
            &Patch::Apply(item),
        )
        .await
        .map_err(|e| AppError::Kubernetes(e.to_string()))
    }

    pub async fn delete(
        &self,
        cluster: &KubernetesClusterConfig,
        namespace: &str,
        name: &str,
    ) -> Result<(), AppError> {
        let api: Api<ResourceQuota> =
            Api::namespaced(ClientFactory::get_client(cluster).await?, namespace);
        api.delete(name, &DeleteParams::default())
            .await
            .map_err(|e| AppError::Kubernetes(e.to_string()))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use k8s_openapi::api::core::v1::{
        ResourceQuotaSpec, ResourceQuotaStatus, ScopeSelector, ScopedResourceSelectorRequirement,
    };
    use k8s_openapi::apimachinery::pkg::apis::meta::v1::{ObjectMeta, Time};

    fn map(values: &[(&str, &str)]) -> BTreeMap<String, String> {
        values
            .iter()
            .map(|(key, value)| ((*key).to_string(), (*value).to_string()))
            .collect()
    }

    fn quantities(values: &[(&str, &str)]) -> BTreeMap<String, Quantity> {
        values
            .iter()
            .map(|(key, value)| ((*key).to_string(), Quantity((*value).to_string())))
            .collect()
    }

    #[test]
    fn resource_quota_inventory_conversion_preserves_metadata_spec_and_status() {
        let created_at = Utc.with_ymd_and_hms(2026, 6, 1, 12, 0, 0).unwrap();
        let collected_at = Utc.with_ymd_and_hms(2026, 6, 10, 0, 0, 0).unwrap();
        let quota = ResourceQuota {
            metadata: ObjectMeta {
                name: Some("apps-quota".to_string()),
                namespace: Some("apps".to_string()),
                labels: Some(map(&[("team", "platform")])),
                annotations: Some(map(&[("cost-center", "cc-42")])),
                creation_timestamp: Some(Time(created_at)),
                ..Default::default()
            },
            spec: Some(ResourceQuotaSpec {
                hard: Some(quantities(&[("pods", "10"), ("requests.cpu", "4")])),
                scopes: Some(vec!["NotTerminating".to_string()]),
                scope_selector: Some(ScopeSelector {
                    match_expressions: Some(vec![ScopedResourceSelectorRequirement {
                        operator: "In".to_string(),
                        scope_name: "PriorityClass".to_string(),
                        values: Some(vec!["critical".to_string()]),
                    }]),
                }),
            }),
            status: Some(ResourceQuotaStatus {
                hard: Some(quantities(&[("pods", "10"), ("requests.cpu", "4")])),
                used: Some(quantities(&[("pods", "4"), ("requests.cpu", "1500m")])),
            }),
        };

        let item =
            convert_kube_resource_quota_to_inventory(&quota, "cluster-a", "fallback", collected_at);

        assert_eq!(item.cluster_id, "cluster-a");
        assert_eq!(item.namespace, "apps");
        assert_eq!(item.name, "apps-quota");
        assert_eq!(item.labels["team"], "platform");
        assert_eq!(item.annotations["cost-center"], "cc-42");
        assert_eq!(item.created_at, Some(created_at));
        assert_eq!(item.collected_at, collected_at);
        assert_eq!(item.hard["pods"], "10");
        assert_eq!(item.used["requests.cpu"], "1500m");
        assert_eq!(item.scopes, vec!["NotTerminating".to_string()]);
        assert_eq!(item.scope_selector.len(), 1);
        assert_eq!(item.scope_selector[0].scope_name, "PriorityClass");
    }

    #[test]
    fn resource_quota_inventory_conversion_uses_fallback_namespace_and_spec_hard() {
        let collected_at = Utc.with_ymd_and_hms(2026, 6, 10, 0, 0, 0).unwrap();
        let quota = ResourceQuota {
            metadata: ObjectMeta {
                name: Some("fallback-quota".to_string()),
                ..Default::default()
            },
            spec: Some(ResourceQuotaSpec {
                hard: Some(quantities(&[("pods", "5")])),
                scopes: None,
                scope_selector: None,
            }),
            status: None,
        };

        let item = convert_kube_resource_quota_to_inventory(
            &quota,
            "cluster-a",
            "requested-namespace",
            collected_at,
        );

        assert_eq!(item.namespace, "requested-namespace");
        assert_eq!(item.hard["pods"], "5");
        assert!(item.used.is_empty());
        assert!(item.scopes.is_empty());
        assert!(item.scope_selector.is_empty());
    }
}

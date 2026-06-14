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
use crate::services::kubernetes::custom_resource_definition_inventory::{
    CustomResourceDefinitionConditionInventoryItem, CustomResourceDefinitionInventoryItem,
    CustomResourceDefinitionVersionInventoryItem,
};
use crate::services::kubernetes::custom_resource_inventory::CustomResourceInventoryItem;
use chrono::{DateTime, Utc};
use k8s_openapi::apiextensions_apiserver::pkg::apis::apiextensions::v1::CustomResourceDefinition;
use kube::{
    api::{Api, DynamicObject, GroupVersionKind, ListParams},
    discovery::{ApiResource, Discovery, Scope},
    Client, ResourceExt,
};
use serde_json::{json, Value};

pub struct CrdsService;

fn convert_kube_crd_to_inventory(
    crd: &CustomResourceDefinition,
    cluster_id: &str,
    collected_at: DateTime<Utc>,
) -> CustomResourceDefinitionInventoryItem {
    let versions = crd
        .spec
        .versions
        .iter()
        .map(|version| {
            let subresources = version.subresources.as_ref();
            CustomResourceDefinitionVersionInventoryItem {
                name: version.name.clone(),
                served: version.served,
                storage: version.storage,
                deprecated: version.deprecated.unwrap_or(false),
                has_schema: version
                    .schema
                    .as_ref()
                    .and_then(|schema| schema.open_api_v3_schema.as_ref())
                    .is_some(),
                has_status_subresource: subresources
                    .and_then(|subresource| subresource.status.as_ref())
                    .is_some(),
                has_scale_subresource: subresources
                    .and_then(|subresource| subresource.scale.as_ref())
                    .is_some(),
                additional_printer_columns_count: version
                    .additional_printer_columns
                    .as_ref()
                    .map(Vec::len)
                    .unwrap_or_default(),
            }
        })
        .collect();
    let stored_versions = crd
        .status
        .as_ref()
        .and_then(|status| status.stored_versions.clone())
        .unwrap_or_default();
    let conditions = crd
        .status
        .as_ref()
        .and_then(|status| status.conditions.clone())
        .unwrap_or_default()
        .into_iter()
        .map(|condition| CustomResourceDefinitionConditionInventoryItem {
            condition_type: condition.type_,
            status: condition.status,
            reason: condition.reason,
            message: condition.message,
        })
        .collect();

    CustomResourceDefinitionInventoryItem {
        cluster_id: cluster_id.to_string(),
        name: crd.name_any(),
        labels: crd.metadata.labels.clone().unwrap_or_default(),
        annotations: crd.metadata.annotations.clone().unwrap_or_default(),
        group: crd.spec.group.clone(),
        scope: crd.spec.scope.clone(),
        kind: crd.spec.names.kind.clone(),
        plural: crd.spec.names.plural.clone(),
        singular: crd.spec.names.singular.clone(),
        short_names: crd.spec.names.short_names.clone().unwrap_or_default(),
        categories: crd.spec.names.categories.clone().unwrap_or_default(),
        preserve_unknown_fields: crd.spec.preserve_unknown_fields,
        versions,
        stored_versions,
        conditions,
        created_at: crd
            .metadata
            .creation_timestamp
            .as_ref()
            .map(|timestamp| timestamp.0),
        collected_at,
    }
}

async fn list_custom_resource_version_inventory(
    client: &Client,
    resource: &ApiResource,
    scope: &str,
    cluster_id: &str,
    namespace: Option<&str>,
    fallback_namespace: &str,
    collected_at: DateTime<Utc>,
) -> Result<Vec<CustomResourceInventoryItem>, AppError> {
    let is_namespaced = scope.eq_ignore_ascii_case("Namespaced");
    let api: Api<DynamicObject> = match namespace {
        Some(namespace) if namespace != "all" && is_namespaced => {
            Api::namespaced_with(client.clone(), namespace, resource)
        }
        _ => Api::all_with(client.clone(), resource),
    };

    let list = api.list(&ListParams::default()).await.map_err(|e| {
        AppError::ExternalService(format!(
            "Failed to list CustomResources for {}/{}: {}",
            resource.api_version, resource.plural, e
        ))
    })?;

    Ok(list
        .items
        .into_iter()
        .map(|item| {
            convert_dynamic_custom_resource_to_inventory(
                item,
                cluster_id,
                resource,
                scope,
                fallback_namespace,
                collected_at,
            )
        })
        .collect())
}

fn convert_dynamic_custom_resource_to_inventory(
    item: DynamicObject,
    cluster_id: &str,
    resource: &ApiResource,
    scope: &str,
    fallback_namespace: &str,
    collected_at: DateTime<Utc>,
) -> CustomResourceInventoryItem {
    let spec = object_field(&item.data, "spec");
    let status = object_field(&item.data, "status");
    let api_version = item
        .types
        .as_ref()
        .map(|type_meta| type_meta.api_version.clone())
        .unwrap_or_else(|| resource.api_version.clone());
    let kind = item
        .types
        .as_ref()
        .map(|type_meta| type_meta.kind.clone())
        .unwrap_or_else(|| resource.kind.clone());
    let namespace = item.metadata.namespace.clone().or_else(|| {
        if scope.eq_ignore_ascii_case("Namespaced") && !fallback_namespace.is_empty() {
            Some(fallback_namespace.to_string())
        } else {
            None
        }
    });

    CustomResourceInventoryItem {
        cluster_id: cluster_id.to_string(),
        namespace,
        name: item.metadata.name.clone().unwrap_or_default(),
        api_version,
        kind,
        group: resource.group.clone(),
        version: resource.version.clone(),
        plural: resource.plural.clone(),
        scope: scope.to_string(),
        labels: item.metadata.labels.clone().unwrap_or_default(),
        annotations: item.metadata.annotations.clone().unwrap_or_default(),
        owner_references_count: item
            .metadata
            .owner_references
            .as_ref()
            .map(Vec::len)
            .unwrap_or_default(),
        finalizers: item.metadata.finalizers.clone().unwrap_or_default(),
        has_status: status
            .as_object()
            .map(|object| !object.is_empty())
            .unwrap_or(false),
        ready_condition_status: readiness_condition_status(&status),
        deletion_timestamp: item
            .metadata
            .deletion_timestamp
            .as_ref()
            .map(|timestamp| timestamp.0),
        spec_keys: object_keys(&spec),
        status_keys: object_keys(&status),
        sensitive_field_paths: sensitive_field_paths(&spec, &status),
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

fn object_keys(value: &Value) -> Vec<String> {
    let mut keys = value
        .as_object()
        .map(|object| object.keys().cloned().collect::<Vec<_>>())
        .unwrap_or_default();
    keys.sort();
    keys
}

fn readiness_condition_status(status: &Value) -> Option<String> {
    let conditions = status.get("conditions")?.as_array()?;
    for wanted_type in ["Ready", "Available", "Healthy"] {
        if let Some(status) = conditions.iter().find_map(|condition| {
            let condition_type = condition.get("type").and_then(Value::as_str)?;
            if condition_type.eq_ignore_ascii_case(wanted_type) {
                condition
                    .get("status")
                    .and_then(Value::as_str)
                    .map(str::to_string)
            } else {
                None
            }
        }) {
            return Some(status);
        }
    }
    None
}

fn sensitive_field_paths(spec: &Value, status: &Value) -> Vec<String> {
    let mut paths = Vec::new();
    collect_sensitive_field_paths(spec, "spec", &mut paths);
    collect_sensitive_field_paths(status, "status", &mut paths);
    paths.sort();
    paths.dedup();
    paths
}

fn collect_sensitive_field_paths(value: &Value, path: &str, paths: &mut Vec<String>) {
    match value {
        Value::Object(object) => {
            for (key, child) in object {
                let child_path = format!("{}.{}", path, key);
                if is_sensitive_key(key) && has_non_empty_evidence(child) {
                    paths.push(child_path.clone());
                }
                collect_sensitive_field_paths(child, &child_path, paths);
            }
        }
        Value::Array(values) => {
            for (index, child) in values.iter().enumerate() {
                collect_sensitive_field_paths(child, &format!("{}[{}]", path, index), paths);
            }
        }
        _ => {}
    }
}

fn is_sensitive_key(key: &str) -> bool {
    let normalized = key
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .collect::<String>()
        .to_ascii_lowercase();
    matches!(
        normalized.as_str(),
        "password"
            | "token"
            | "secret"
            | "credential"
            | "credentials"
            | "privatekey"
            | "clientsecret"
            | "accesskey"
            | "secretkey"
            | "apikey"
    )
}

fn has_non_empty_evidence(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::String(value) => !value.trim().is_empty(),
        Value::Array(values) => !values.is_empty(),
        Value::Object(object) => !object.is_empty(),
        Value::Bool(_) | Value::Number(_) => true,
    }
}

impl CrdsService {
    pub fn new() -> Self {
        Self
    }

    pub async fn list_crds(
        &self,
        cluster_config: &KubernetesClusterConfig,
    ) -> Result<Vec<Value>, AppError> {
        let client = ClientFactory::get_client(cluster_config).await?;
        let crds: Api<CustomResourceDefinition> = Api::all(client);

        let crd_list = crds
            .list(&ListParams::default())
            .await
            .map_err(|e| AppError::ExternalService(format!("Failed to list CRDs: {}", e)))?;

        let mut formatted_crds = Vec::new();
        for crd in crd_list {
            if let Ok(value) = serde_json::to_value(&crd) {
                formatted_crds.push(value);
            }
        }

        Ok(formatted_crds)
    }

    pub async fn list_inventory(
        &self,
        cluster_config: &KubernetesClusterConfig,
        cluster_id: &str,
    ) -> Result<Vec<CustomResourceDefinitionInventoryItem>, AppError> {
        let client = ClientFactory::get_client(cluster_config).await?;
        let crds: Api<CustomResourceDefinition> = Api::all(client);
        let collected_at = Utc::now();
        let crd_list = crds.list(&ListParams::default()).await.map_err(|e| {
            AppError::ExternalService(format!(
                "Failed to list CustomResourceDefinition inventory: {}",
                e
            ))
        })?;

        let mut inventory = crd_list
            .items
            .iter()
            .map(|crd| convert_kube_crd_to_inventory(crd, cluster_id, collected_at))
            .collect::<Vec<_>>();
        inventory.sort_by(|left, right| left.name.cmp(&right.name));
        Ok(inventory)
    }

    pub async fn list_custom_resource_inventory(
        &self,
        cluster_config: &KubernetesClusterConfig,
        cluster_id: &str,
        namespace: Option<&str>,
    ) -> Result<Vec<CustomResourceInventoryItem>, AppError> {
        let client = ClientFactory::get_client(cluster_config).await?;
        let crds: Api<CustomResourceDefinition> = Api::all(client.clone());
        let collected_at = Utc::now();
        let namespace = namespace
            .map(str::trim)
            .filter(|namespace| !namespace.is_empty());
        let fallback_namespace = namespace
            .filter(|namespace| *namespace != "all")
            .unwrap_or("");
        let crd_list = crds.list(&ListParams::default()).await.map_err(|e| {
            AppError::ExternalService(format!(
                "Failed to list CustomResourceDefinitions for CustomResource inventory: {}",
                e
            ))
        })?;

        let mut inventory = Vec::new();
        for crd in crd_list.items {
            for version in crd.spec.versions.iter().filter(|version| version.served) {
                let gvk =
                    GroupVersionKind::gvk(&crd.spec.group, &version.name, &crd.spec.names.kind);
                let resource = ApiResource::from_gvk_with_plural(&gvk, &crd.spec.names.plural);
                inventory.extend(
                    list_custom_resource_version_inventory(
                        &client,
                        &resource,
                        &crd.spec.scope,
                        cluster_id,
                        namespace,
                        fallback_namespace,
                        collected_at,
                    )
                    .await?,
                );
            }
        }

        inventory.sort_by(|left, right| {
            (
                left.group.as_str(),
                left.version.as_str(),
                left.plural.as_str(),
                left.namespace.as_deref().unwrap_or(""),
                left.name.as_str(),
            )
                .cmp(&(
                    right.group.as_str(),
                    right.version.as_str(),
                    right.plural.as_str(),
                    right.namespace.as_deref().unwrap_or(""),
                    right.name.as_str(),
                ))
        });
        Ok(inventory)
    }

    pub async fn get_crd_details(
        &self,
        cluster_config: &KubernetesClusterConfig,
        crd_name: &str,
    ) -> Result<Value, AppError> {
        let client = ClientFactory::get_client(cluster_config).await?;
        let crds: Api<CustomResourceDefinition> = Api::all(client);

        let crd = crds
            .get(crd_name)
            .await
            .map_err(|e| AppError::ExternalService(format!("Failed to get CRD details: {}", e)))?;

        serde_json::to_value(&crd)
            .map_err(|e| AppError::Internal(format!("Failed to serialize CRD details: {}", e)))
    }

    /// Generic fallback for dynamically dealing with custom resources based on their GroupVersionKind
    pub async fn list_custom_resources(
        &self,
        cluster_config: &KubernetesClusterConfig,
        group: &str,
        version: &str,
        plural: &str,
        namespace: Option<&str>,
    ) -> Result<Vec<Value>, AppError> {
        let client = ClientFactory::get_client(cluster_config).await?;
        let discovery = Discovery::new(client.clone())
            .run()
            .await
            .map_err(|e| AppError::ExternalService(format!("Discovery failed: {}", e)))?;

        let gvk = GroupVersionKind::gvk(group, version, "");

        // Use discovery to find the exact APIResource matching the requested group/version/plural
        let _api_group = discovery.resolve_gvk(&gvk).ok_or_else(|| {
            AppError::NotFound(format!("ApiGroup {}/{} not found", group, version))
        })?;

        // Fallback or explicit check for resource by plural if gvk resolution doesn't match perfectly.
        // We really want the resource by plural name since that maps to the REST endpoint.
        let mut target_ar: Option<(ApiResource, kube::discovery::ApiCapabilities)> = None;
        if let Some(group_info) = discovery.get(group) {
            for (ar, caps) in group_info.recommended_resources() {
                if ar.plural == plural && ar.version == version && ar.group == group {
                    target_ar = Some((ar, caps));
                    break;
                }
            }
        }

        let (ar, caps) = target_ar.ok_or_else(|| {
            AppError::NotFound(format!(
                "Resource {} not found in {}/{}",
                plural, group, version
            ))
        })?;

        let api: Api<DynamicObject> = match namespace {
            Some(ns) if caps.scope == Scope::Namespaced => {
                Api::namespaced_with(client.clone(), ns, &ar)
            }
            _ => Api::all_with(client.clone(), &ar),
        };

        let list = api.list(&ListParams::default()).await.map_err(|e| {
            AppError::ExternalService(format!("Failed to list CustomResources: {}", e))
        })?;

        let mut items = Vec::new();
        for item in list {
            if let Ok(value) = serde_json::to_value(&item) {
                items.push(value);
            }
        }

        Ok(items)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use k8s_openapi::apiextensions_apiserver::pkg::apis::apiextensions::v1::{
        CustomResourceColumnDefinition, CustomResourceDefinitionCondition,
        CustomResourceDefinitionNames, CustomResourceDefinitionSpec,
        CustomResourceDefinitionStatus, CustomResourceDefinitionVersion,
        CustomResourceSubresourceStatus, CustomResourceSubresources, CustomResourceValidation,
        JSONSchemaProps,
    };
    use k8s_openapi::apimachinery::pkg::apis::meta::v1::{ObjectMeta, Time};
    use serde_json::json;
    use std::collections::BTreeMap;

    fn map(values: &[(&str, &str)]) -> BTreeMap<String, String> {
        values
            .iter()
            .map(|(key, value)| ((*key).to_string(), (*value).to_string()))
            .collect()
    }

    #[test]
    fn custom_resource_definition_inventory_conversion_preserves_metadata_spec_versions_status() {
        let created_at = Utc.with_ymd_and_hms(2026, 6, 1, 12, 0, 0).unwrap();
        let collected_at = Utc.with_ymd_and_hms(2026, 6, 10, 0, 0, 0).unwrap();
        let crd = CustomResourceDefinition {
            metadata: ObjectMeta {
                name: Some("widgets.example.com".to_string()),
                labels: Some(map(&[("team", "platform")])),
                annotations: Some(map(&[("cost-center", "cc-42")])),
                creation_timestamp: Some(Time(created_at)),
                ..Default::default()
            },
            spec: CustomResourceDefinitionSpec {
                group: "example.com".to_string(),
                names: CustomResourceDefinitionNames {
                    kind: "Widget".to_string(),
                    plural: "widgets".to_string(),
                    singular: Some("widget".to_string()),
                    short_names: Some(vec!["wdg".to_string()]),
                    categories: Some(vec!["all".to_string()]),
                    ..Default::default()
                },
                preserve_unknown_fields: Some(false),
                scope: "Namespaced".to_string(),
                versions: vec![CustomResourceDefinitionVersion {
                    additional_printer_columns: Some(vec![CustomResourceColumnDefinition {
                        json_path: ".spec.size".to_string(),
                        name: "Size".to_string(),
                        type_: "string".to_string(),
                        ..Default::default()
                    }]),
                    name: "v1".to_string(),
                    schema: Some(CustomResourceValidation {
                        open_api_v3_schema: Some(JSONSchemaProps {
                            type_: Some("object".to_string()),
                            ..Default::default()
                        }),
                    }),
                    served: true,
                    storage: true,
                    subresources: Some(CustomResourceSubresources {
                        status: Some(CustomResourceSubresourceStatus(json!({}))),
                        ..Default::default()
                    }),
                    ..Default::default()
                }],
                ..Default::default()
            },
            status: Some(CustomResourceDefinitionStatus {
                conditions: Some(vec![
                    CustomResourceDefinitionCondition {
                        type_: "Established".to_string(),
                        status: "True".to_string(),
                        reason: Some("InitialNamesAccepted".to_string()),
                        ..Default::default()
                    },
                    CustomResourceDefinitionCondition {
                        type_: "NamesAccepted".to_string(),
                        status: "True".to_string(),
                        reason: Some("NoConflicts".to_string()),
                        message: Some("names are accepted".to_string()),
                        ..Default::default()
                    },
                ]),
                stored_versions: Some(vec!["v1".to_string()]),
                ..Default::default()
            }),
        };

        let item = convert_kube_crd_to_inventory(&crd, "cluster-a", collected_at);

        assert_eq!(item.cluster_id, "cluster-a");
        assert_eq!(item.name, "widgets.example.com");
        assert_eq!(item.labels["team"], "platform");
        assert_eq!(item.annotations["cost-center"], "cc-42");
        assert_eq!(item.group, "example.com");
        assert_eq!(item.scope, "Namespaced");
        assert_eq!(item.kind, "Widget");
        assert_eq!(item.plural, "widgets");
        assert_eq!(item.singular.as_deref(), Some("widget"));
        assert_eq!(item.short_names, vec!["wdg".to_string()]);
        assert_eq!(item.categories, vec!["all".to_string()]);
        assert_eq!(item.preserve_unknown_fields, Some(false));
        assert_eq!(item.created_at, Some(created_at));
        assert_eq!(item.collected_at, collected_at);
        assert_eq!(item.stored_versions, vec!["v1".to_string()]);
        assert_eq!(item.conditions.len(), 2);
        assert_eq!(item.conditions[0].condition_type, "Established");
        assert_eq!(
            item.conditions[1].message.as_deref(),
            Some("names are accepted")
        );
        assert_eq!(item.versions.len(), 1);
        assert_eq!(item.versions[0].name, "v1");
        assert!(item.versions[0].served);
        assert!(item.versions[0].storage);
        assert!(!item.versions[0].deprecated);
        assert!(item.versions[0].has_schema);
        assert!(item.versions[0].has_status_subresource);
        assert!(!item.versions[0].has_scale_subresource);
        assert_eq!(item.versions[0].additional_printer_columns_count, 1);
    }

    #[test]
    fn custom_resource_definition_inventory_conversion_handles_missing_optional_state() {
        let collected_at = Utc.with_ymd_and_hms(2026, 6, 10, 0, 0, 0).unwrap();
        let crd = CustomResourceDefinition {
            metadata: ObjectMeta {
                name: Some("gadgets.example.com".to_string()),
                ..Default::default()
            },
            spec: CustomResourceDefinitionSpec {
                group: "example.com".to_string(),
                names: CustomResourceDefinitionNames {
                    kind: "Gadget".to_string(),
                    plural: "gadgets".to_string(),
                    ..Default::default()
                },
                preserve_unknown_fields: Some(true),
                scope: "Cluster".to_string(),
                versions: vec![CustomResourceDefinitionVersion {
                    name: "v1alpha1".to_string(),
                    served: true,
                    storage: true,
                    ..Default::default()
                }],
                ..Default::default()
            },
            status: None,
        };

        let item = convert_kube_crd_to_inventory(&crd, "cluster-a", collected_at);

        assert_eq!(item.cluster_id, "cluster-a");
        assert_eq!(item.name, "gadgets.example.com");
        assert!(item.labels.is_empty());
        assert!(item.annotations.is_empty());
        assert_eq!(item.group, "example.com");
        assert_eq!(item.scope, "Cluster");
        assert_eq!(item.kind, "Gadget");
        assert_eq!(item.plural, "gadgets");
        assert!(item.singular.is_none());
        assert!(item.short_names.is_empty());
        assert!(item.categories.is_empty());
        assert_eq!(item.preserve_unknown_fields, Some(true));
        assert!(item.stored_versions.is_empty());
        assert!(item.conditions.is_empty());
        assert!(item.created_at.is_none());
        assert_eq!(item.versions.len(), 1);
        assert_eq!(item.versions[0].name, "v1alpha1");
        assert!(item.versions[0].served);
        assert!(item.versions[0].storage);
        assert!(!item.versions[0].has_schema);
        assert!(!item.versions[0].has_status_subresource);
        assert_eq!(item.versions[0].additional_printer_columns_count, 0);
    }

    fn custom_resource_api_resource() -> ApiResource {
        ApiResource::from_gvk_with_plural(
            &GroupVersionKind::gvk("example.com", "v1", "Widget"),
            "widgets",
        )
    }

    #[test]
    fn custom_resource_inventory_conversion_preserves_dynamic_metadata_spec_status() {
        let resource = custom_resource_api_resource();
        let created_at = Utc.with_ymd_and_hms(2026, 6, 1, 12, 0, 0).unwrap();
        let deleted_at = Utc.with_ymd_and_hms(2026, 6, 2, 12, 0, 0).unwrap();
        let collected_at = Utc.with_ymd_and_hms(2026, 6, 10, 0, 0, 0).unwrap();
        let mut custom_resource = DynamicObject::new("widget-a", &resource)
            .within("apps")
            .data(json!({
                "spec": {
                    "replicas": 2,
                    "credentials": {
                        "token": "plain-text-token"
                    }
                },
                "status": {
                    "conditions": [
                        {
                            "type": "Ready",
                            "status": "False",
                            "reason": "ControllerError"
                        }
                    ],
                    "observedGeneration": 1
                }
            }));
        custom_resource.metadata.labels = Some(map(&[("team", "platform")]));
        custom_resource.metadata.annotations = Some(map(&[("cost-center", "cc-42")]));
        custom_resource.metadata.finalizers =
            Some(vec!["cleanup.example.com/finalizer".to_string()]);
        custom_resource.metadata.creation_timestamp = Some(Time(created_at));
        custom_resource.metadata.deletion_timestamp = Some(Time(deleted_at));

        let item = convert_dynamic_custom_resource_to_inventory(
            custom_resource,
            "cluster-a",
            &resource,
            "Namespaced",
            "fallback",
            collected_at,
        );

        assert_eq!(item.cluster_id, "cluster-a");
        assert_eq!(item.namespace.as_deref(), Some("apps"));
        assert_eq!(item.name, "widget-a");
        assert_eq!(item.api_version, "example.com/v1");
        assert_eq!(item.kind, "Widget");
        assert_eq!(item.group, "example.com");
        assert_eq!(item.version, "v1");
        assert_eq!(item.plural, "widgets");
        assert_eq!(item.scope, "Namespaced");
        assert_eq!(item.labels["team"], "platform");
        assert_eq!(item.annotations["cost-center"], "cc-42");
        assert_eq!(
            item.finalizers,
            vec!["cleanup.example.com/finalizer".to_string()]
        );
        assert!(item.has_status);
        assert_eq!(item.ready_condition_status.as_deref(), Some("False"));
        assert_eq!(item.deletion_timestamp, Some(deleted_at));
        assert_eq!(item.created_at, Some(created_at));
        assert_eq!(item.collected_at, collected_at);
        assert_eq!(
            item.spec_keys,
            vec!["credentials".to_string(), "replicas".to_string()]
        );
        assert_eq!(
            item.status_keys,
            vec!["conditions".to_string(), "observedGeneration".to_string()]
        );
        assert!(item
            .sensitive_field_paths
            .iter()
            .any(|path| path == "spec.credentials.token"));
    }

    #[test]
    fn custom_resource_inventory_conversion_handles_cluster_scoped_optional_state() {
        let resource = custom_resource_api_resource();
        let collected_at = Utc.with_ymd_and_hms(2026, 6, 10, 0, 0, 0).unwrap();
        let custom_resource = DynamicObject::new("cluster-widget", &resource).data(json!({
            "spec": {
                "size": "small"
            }
        }));

        let item = convert_dynamic_custom_resource_to_inventory(
            custom_resource,
            "cluster-a",
            &resource,
            "Cluster",
            "",
            collected_at,
        );

        assert_eq!(item.cluster_id, "cluster-a");
        assert!(item.namespace.is_none());
        assert_eq!(item.name, "cluster-widget");
        assert_eq!(item.scope, "Cluster");
        assert!(item.labels.is_empty());
        assert!(item.annotations.is_empty());
        assert!(item.finalizers.is_empty());
        assert!(!item.has_status);
        assert!(item.ready_condition_status.is_none());
        assert!(item.deletion_timestamp.is_none());
        assert_eq!(item.spec_keys, vec!["size".to_string()]);
        assert!(item.status_keys.is_empty());
        assert!(item.sensitive_field_paths.is_empty());
        assert!(item.created_at.is_none());
    }
}

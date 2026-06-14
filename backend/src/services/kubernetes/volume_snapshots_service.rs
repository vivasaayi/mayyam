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
use crate::services::kubernetes::volume_snapshot_inventory::VolumeSnapshotInventoryItem;

const VOLUME_SNAPSHOT_API_GROUP: &str = "snapshot.storage.k8s.io";
const VOLUME_SNAPSHOT_API_VERSION: &str = "v1";
const VOLUME_SNAPSHOT_KIND: &str = "VolumeSnapshot";
const VOLUME_SNAPSHOT_PLURAL: &str = "volumesnapshots";

pub struct VolumeSnapshotsService;

impl VolumeSnapshotsService {
    pub fn new() -> Self {
        Self
    }

    pub async fn list_inventory(
        &self,
        cluster: &KubernetesClusterConfig,
        cluster_id: &str,
        namespace: Option<&str>,
    ) -> Result<Vec<VolumeSnapshotInventoryItem>, AppError> {
        let client = ClientFactory::get_client(cluster).await?;
        let discovery = Discovery::new(client.clone())
            .filter(&[VOLUME_SNAPSHOT_API_GROUP])
            .run()
            .await
            .map_err(|e| {
                AppError::ExternalService(format!("VolumeSnapshot discovery failed: {}", e))
            })?;

        let Some(group) = discovery.get(VOLUME_SNAPSHOT_API_GROUP) else {
            return Ok(Vec::new());
        };
        let Some((resource, capabilities)) = resolve_volume_snapshot_resource(group) else {
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
        let mut inventory = list_dynamic_volume_snapshots(
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

fn resolve_volume_snapshot_resource(group: &ApiGroup) -> Option<(ApiResource, ApiCapabilities)> {
    group
        .resources_by_stability()
        .into_iter()
        .find(|(resource, _)| {
            resource.group == VOLUME_SNAPSHOT_API_GROUP
                && resource.version == VOLUME_SNAPSHOT_API_VERSION
                && resource.kind == VOLUME_SNAPSHOT_KIND
                && resource.plural == VOLUME_SNAPSHOT_PLURAL
        })
}

async fn list_dynamic_volume_snapshots(
    client: &Client,
    resource: &ApiResource,
    capabilities: &ApiCapabilities,
    cluster_id: &str,
    namespace: Option<&str>,
    fallback_namespace: &str,
    collected_at: DateTime<Utc>,
) -> Result<Vec<VolumeSnapshotInventoryItem>, AppError> {
    let api: Api<DynamicObject> = match namespace {
        Some(namespace) if namespace != "all" && capabilities.scope == Scope::Namespaced => {
            Api::namespaced_with(client.clone(), namespace, resource)
        }
        _ => Api::all_with(client.clone(), resource),
    };

    let list = api
        .list(&ListParams::default())
        .await
        .map_err(|e| AppError::ExternalService(format!("Failed to list VolumeSnapshots: {}", e)))?;

    Ok(list
        .items
        .into_iter()
        .map(|item| {
            convert_dynamic_volume_snapshot_to_inventory(
                item,
                cluster_id,
                resource,
                fallback_namespace,
                collected_at,
            )
        })
        .collect())
}

fn convert_dynamic_volume_snapshot_to_inventory(
    item: DynamicObject,
    cluster_id: &str,
    resource: &ApiResource,
    fallback_namespace: &str,
    collected_at: DateTime<Utc>,
) -> VolumeSnapshotInventoryItem {
    let spec = object_field(&item.data, "spec");
    let status = object_field(&item.data, "status");
    let source = object_field(&spec, "source");
    let error = object_field(&status, "error");
    let api_version = item
        .types
        .as_ref()
        .map(|type_meta| type_meta.api_version.clone())
        .unwrap_or_else(|| resource.api_version.clone());

    VolumeSnapshotInventoryItem {
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
        snapshot_class_name: string_field(&spec, "volumeSnapshotClassName"),
        source_persistent_volume_claim_name: string_field(&source, "persistentVolumeClaimName"),
        source_volume_snapshot_content_name: string_field(&source, "volumeSnapshotContentName"),
        bound_volume_snapshot_content_name: string_field(&status, "boundVolumeSnapshotContentName"),
        ready_to_use: status.get("readyToUse").and_then(Value::as_bool),
        restore_size: string_field(&status, "restoreSize"),
        error_message: string_field(&error, "message"),
        error_time: time_field(&error, "time"),
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

fn time_field(value: &Value, key: &str) -> Option<DateTime<Utc>> {
    let raw = string_field(value, key)?;
    DateTime::parse_from_rfc3339(&raw)
        .ok()
        .map(|timestamp| timestamp.with_timezone(&Utc))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use k8s_openapi::apimachinery::pkg::apis::meta::v1::Time;
    use kube::core::GroupVersionKind;
    use std::collections::BTreeMap;

    fn map(values: &[(&str, &str)]) -> BTreeMap<String, String> {
        values
            .iter()
            .map(|(key, value)| ((*key).to_string(), (*value).to_string()))
            .collect()
    }

    fn volume_snapshot_resource() -> ApiResource {
        ApiResource::from_gvk_with_plural(
            &GroupVersionKind::gvk(
                VOLUME_SNAPSHOT_API_GROUP,
                VOLUME_SNAPSHOT_API_VERSION,
                VOLUME_SNAPSHOT_KIND,
            ),
            VOLUME_SNAPSHOT_PLURAL,
        )
    }

    #[test]
    fn volume_snapshot_inventory_conversion_preserves_metadata_spec_and_status() {
        let resource = volume_snapshot_resource();
        let created_at = Utc.with_ymd_and_hms(2026, 6, 1, 12, 0, 0).unwrap();
        let error_time = Utc.with_ymd_and_hms(2026, 6, 2, 12, 0, 0).unwrap();
        let collected_at = Utc.with_ymd_and_hms(2026, 6, 10, 0, 0, 0).unwrap();
        let mut snapshot = DynamicObject::new("data-snapshot", &resource)
            .within("apps")
            .data(json!({
                "spec": {
                    "volumeSnapshotClassName": "csi-gp3-snapshots",
                    "source": {
                        "persistentVolumeClaimName": "data"
                    }
                },
                "status": {
                    "boundVolumeSnapshotContentName": "snapcontent-abc",
                    "readyToUse": false,
                    "restoreSize": "100Gi",
                    "error": {
                        "message": "snapshot controller timeout",
                        "time": "2026-06-02T12:00:00Z"
                    }
                }
            }));
        snapshot.metadata.labels = Some(map(&[("team", "storage")]));
        snapshot.metadata.annotations = Some(map(&[("cost-center", "cc-12")]));
        snapshot.metadata.creation_timestamp = Some(Time(created_at));

        let item = convert_dynamic_volume_snapshot_to_inventory(
            snapshot,
            "cluster-a",
            &resource,
            "fallback",
            collected_at,
        );

        assert_eq!(item.cluster_id, "cluster-a");
        assert_eq!(item.namespace, "apps");
        assert_eq!(item.name, "data-snapshot");
        assert_eq!(item.api_version, "snapshot.storage.k8s.io/v1");
        assert_eq!(item.labels["team"], "storage");
        assert_eq!(item.annotations["cost-center"], "cc-12");
        assert_eq!(
            item.snapshot_class_name.as_deref(),
            Some("csi-gp3-snapshots")
        );
        assert_eq!(
            item.source_persistent_volume_claim_name.as_deref(),
            Some("data")
        );
        assert!(item.source_volume_snapshot_content_name.is_none());
        assert_eq!(
            item.bound_volume_snapshot_content_name.as_deref(),
            Some("snapcontent-abc")
        );
        assert_eq!(item.ready_to_use, Some(false));
        assert_eq!(item.restore_size.as_deref(), Some("100Gi"));
        assert_eq!(
            item.error_message.as_deref(),
            Some("snapshot controller timeout")
        );
        assert_eq!(item.error_time, Some(error_time));
        assert_eq!(item.created_at, Some(created_at));
        assert_eq!(item.collected_at, collected_at);
    }

    #[test]
    fn volume_snapshot_inventory_conversion_uses_fallback_namespace_and_content_source() {
        let resource = volume_snapshot_resource();
        let collected_at = Utc.with_ymd_and_hms(2026, 6, 10, 0, 0, 0).unwrap();
        let snapshot = DynamicObject::new("content-snapshot", &resource).data(json!({
            "spec": {
                "source": {
                    "volumeSnapshotContentName": "pre-provisioned-content"
                }
            },
            "status": {
                "readyToUse": true
            }
        }));

        let item = convert_dynamic_volume_snapshot_to_inventory(
            snapshot,
            "cluster-a",
            &resource,
            "requested-namespace",
            collected_at,
        );

        assert_eq!(item.namespace, "requested-namespace");
        assert_eq!(item.name, "content-snapshot");
        assert!(item.snapshot_class_name.is_none());
        assert!(item.source_persistent_volume_claim_name.is_none());
        assert_eq!(
            item.source_volume_snapshot_content_name.as_deref(),
            Some("pre-provisioned-content")
        );
        assert_eq!(item.ready_to_use, Some(true));
        assert!(item.restore_size.is_none());
        assert!(item.error_message.is_none());
        assert!(item.error_time.is_none());
        assert!(item.created_at.is_none());
    }
}

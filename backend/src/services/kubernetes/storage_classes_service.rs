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
use crate::services::kubernetes::storage_class_inventory::StorageClassInventoryItem;
use chrono::{DateTime, Utc};
use k8s_openapi::api::storage::v1::StorageClass;
use kube::{api::ListParams, Api, ResourceExt};
use serde_json::Value;

pub struct StorageClassesService;

fn convert_kube_storage_class_to_inventory(
    storage_class: &StorageClass,
    cluster_id: &str,
    collected_at: DateTime<Utc>,
) -> StorageClassInventoryItem {
    StorageClassInventoryItem {
        cluster_id: cluster_id.to_string(),
        name: storage_class.name_any(),
        labels: storage_class.metadata.labels.clone().unwrap_or_default(),
        annotations: storage_class
            .metadata
            .annotations
            .clone()
            .unwrap_or_default(),
        provisioner: storage_class.provisioner.clone(),
        parameters: storage_class.parameters.clone().unwrap_or_default(),
        reclaim_policy: storage_class.reclaim_policy.clone(),
        volume_binding_mode: storage_class.volume_binding_mode.clone(),
        allow_volume_expansion: storage_class.allow_volume_expansion,
        mount_options: storage_class.mount_options.clone().unwrap_or_default(),
        allowed_topologies_count: storage_class
            .allowed_topologies
            .as_ref()
            .map(Vec::len)
            .unwrap_or_default(),
        created_at: storage_class
            .metadata
            .creation_timestamp
            .as_ref()
            .map(|timestamp| timestamp.0),
        collected_at,
    }
}

impl StorageClassesService {
    pub fn new() -> Self {
        StorageClassesService
    }

    pub async fn list_storage_classes(
        &self,
        cluster_config: &KubernetesClusterConfig,
    ) -> Result<Vec<Value>, AppError> {
        let client = ClientFactory::get_client(cluster_config).await?;
        let storage_classes: Api<StorageClass> = Api::all(client);

        let sc_list = storage_classes
            .list(&ListParams::default())
            .await
            .map_err(|e| {
                AppError::ExternalService(format!("Failed to list StorageClasses: {}", e))
            })?;

        let mut formatted_sc = Vec::new();
        for sc in sc_list {
            if let Ok(value) = serde_json::to_value(&sc) {
                formatted_sc.push(value);
            }
        }

        Ok(formatted_sc)
    }

    pub async fn list_inventory(
        &self,
        cluster_config: &KubernetesClusterConfig,
        cluster_id: &str,
    ) -> Result<Vec<StorageClassInventoryItem>, AppError> {
        let client = ClientFactory::get_client(cluster_config).await?;
        let storage_classes: Api<StorageClass> = Api::all(client);
        let collected_at = Utc::now();
        let sc_list = storage_classes
            .list(&ListParams::default())
            .await
            .map_err(|e| {
                AppError::ExternalService(format!("Failed to list StorageClass inventory: {}", e))
            })?;

        let mut inventory = sc_list
            .items
            .iter()
            .map(|storage_class| {
                convert_kube_storage_class_to_inventory(storage_class, cluster_id, collected_at)
            })
            .collect::<Vec<_>>();
        inventory.sort_by(|left, right| left.name.cmp(&right.name));
        Ok(inventory)
    }

    pub async fn get_storage_class_details(
        &self,
        cluster_config: &KubernetesClusterConfig,
        storage_class_name: &str,
    ) -> Result<Value, AppError> {
        let client = ClientFactory::get_client(cluster_config).await?;
        let storage_classes: Api<StorageClass> = Api::all(client);

        let sc = storage_classes.get(storage_class_name).await.map_err(|e| {
            AppError::ExternalService(format!("Failed to get StorageClass details: {}", e))
        })?;

        serde_json::to_value(&sc).map_err(|e| {
            AppError::Internal(format!("Failed to serialize StorageClass details: {}", e))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use k8s_openapi::api::core::v1::{TopologySelectorLabelRequirement, TopologySelectorTerm};
    use k8s_openapi::apimachinery::pkg::apis::meta::v1::{ObjectMeta, Time};
    use std::collections::BTreeMap;

    fn map(values: &[(&str, &str)]) -> BTreeMap<String, String> {
        values
            .iter()
            .map(|(key, value)| ((*key).to_string(), (*value).to_string()))
            .collect()
    }

    #[test]
    fn storage_class_inventory_conversion_preserves_metadata_parameters_and_topologies() {
        let created_at = Utc.with_ymd_and_hms(2026, 6, 1, 12, 0, 0).unwrap();
        let collected_at = Utc.with_ymd_and_hms(2026, 6, 10, 0, 0, 0).unwrap();
        let storage_class = StorageClass {
            metadata: ObjectMeta {
                name: Some("fast-encrypted".to_string()),
                labels: Some(map(&[("team", "storage")])),
                annotations: Some(map(&[("cost-center", "cc-42")])),
                creation_timestamp: Some(Time(created_at)),
                ..Default::default()
            },
            allow_volume_expansion: Some(true),
            allowed_topologies: Some(vec![TopologySelectorTerm {
                match_label_expressions: Some(vec![TopologySelectorLabelRequirement {
                    key: "topology.kubernetes.io/zone".to_string(),
                    values: vec!["us-east-1a".to_string()],
                }]),
            }]),
            mount_options: Some(vec!["discard".to_string()]),
            parameters: Some(map(&[("type", "gp3"), ("encrypted", "true")])),
            provisioner: "ebs.csi.aws.com".to_string(),
            reclaim_policy: Some("Delete".to_string()),
            volume_binding_mode: Some("WaitForFirstConsumer".to_string()),
        };

        let item =
            convert_kube_storage_class_to_inventory(&storage_class, "cluster-a", collected_at);

        assert_eq!(item.cluster_id, "cluster-a");
        assert_eq!(item.name, "fast-encrypted");
        assert_eq!(item.labels["team"], "storage");
        assert_eq!(item.annotations["cost-center"], "cc-42");
        assert_eq!(item.created_at, Some(created_at));
        assert_eq!(item.collected_at, collected_at);
        assert_eq!(item.provisioner, "ebs.csi.aws.com");
        assert_eq!(item.parameters["encrypted"], "true");
        assert_eq!(item.parameters["type"], "gp3");
        assert_eq!(item.reclaim_policy.as_deref(), Some("Delete"));
        assert_eq!(
            item.volume_binding_mode.as_deref(),
            Some("WaitForFirstConsumer")
        );
        assert_eq!(item.allow_volume_expansion, Some(true));
        assert_eq!(item.mount_options, vec!["discard".to_string()]);
        assert_eq!(item.allowed_topologies_count, 1);
    }

    #[test]
    fn storage_class_inventory_conversion_handles_missing_optional_state() {
        let collected_at = Utc.with_ymd_and_hms(2026, 6, 10, 0, 0, 0).unwrap();
        let storage_class = StorageClass {
            metadata: ObjectMeta {
                name: Some("basic".to_string()),
                ..Default::default()
            },
            provisioner: "kubernetes.io/no-provisioner".to_string(),
            ..Default::default()
        };

        let item =
            convert_kube_storage_class_to_inventory(&storage_class, "cluster-a", collected_at);

        assert_eq!(item.cluster_id, "cluster-a");
        assert_eq!(item.name, "basic");
        assert_eq!(item.provisioner, "kubernetes.io/no-provisioner");
        assert!(item.labels.is_empty());
        assert!(item.annotations.is_empty());
        assert!(item.parameters.is_empty());
        assert!(item.reclaim_policy.is_none());
        assert!(item.volume_binding_mode.is_none());
        assert_eq!(item.allow_volume_expansion, None);
        assert!(item.mount_options.is_empty());
        assert_eq!(item.allowed_topologies_count, 0);
        assert!(item.created_at.is_none());
    }
}

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
use k8s_openapi::api::core::v1::PersistentVolumeClaim;
use k8s_openapi::apimachinery::pkg::api::resource::Quantity;
use kube::api::ListParams;
use kube::config::{Config as KubeConfig, KubeConfigOptions, Kubeconfig};
use kube::{Api, Client, ResourceExt};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use crate::errors::AppError;
use crate::models::cluster::KubernetesClusterConfig;
use crate::services::kubernetes::persistent_volume_claim_inventory::{
    PersistentVolumeClaimConditionInventoryItem, PersistentVolumeClaimInventoryItem,
};

#[derive(Debug, Serialize, Deserialize)]
pub struct PersistentVolumeClaimInfo {
    pub name: String,
    pub namespace: String,
    pub status: String,
    pub volume: String, // Name of the bound PersistentVolume
    pub capacity: Option<String>,
    pub access_modes: Vec<String>,
    pub storage_class: Option<String>,
    pub age: String,
}

pub struct PersistentVolumeClaimsService;

fn quantity_map_to_strings(map: &BTreeMap<String, Quantity>) -> BTreeMap<String, String> {
    map.iter()
        .map(|(key, value)| (key.clone(), value.0.clone()))
        .collect()
}

fn convert_kube_persistent_volume_claim_to_inventory(
    pvc: &PersistentVolumeClaim,
    cluster_id: &str,
    current_namespace: &str,
    collected_at: DateTime<Utc>,
) -> PersistentVolumeClaimInventoryItem {
    let namespace = pvc
        .namespace()
        .unwrap_or_else(|| current_namespace.to_string());
    let spec = pvc.spec.as_ref();
    let status = pvc.status.as_ref();
    let requested_storage = spec
        .and_then(|spec| spec.resources.as_ref())
        .and_then(|resources| resources.requests.as_ref())
        .map(quantity_map_to_strings)
        .unwrap_or_default();
    let capacity = status
        .and_then(|status| status.capacity.as_ref())
        .map(quantity_map_to_strings)
        .unwrap_or_default();
    let access_modes = status
        .and_then(|status| status.access_modes.clone())
        .or_else(|| spec.and_then(|spec| spec.access_modes.clone()))
        .unwrap_or_default();
    let conditions = status
        .and_then(|status| status.conditions.as_ref())
        .map(|conditions| {
            conditions
                .iter()
                .map(|condition| PersistentVolumeClaimConditionInventoryItem {
                    condition_type: condition.type_.clone(),
                    status: condition.status.clone(),
                    reason: condition.reason.clone(),
                    message: condition.message.clone(),
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    PersistentVolumeClaimInventoryItem {
        cluster_id: cluster_id.to_string(),
        namespace,
        name: pvc.name_any(),
        labels: pvc.metadata.labels.clone().unwrap_or_default(),
        annotations: pvc.metadata.annotations.clone().unwrap_or_default(),
        requested_storage,
        capacity,
        access_modes,
        storage_class_name: spec.and_then(|spec| spec.storage_class_name.clone()),
        volume_mode: spec.and_then(|spec| spec.volume_mode.clone()),
        volume_name: spec.and_then(|spec| spec.volume_name.clone()),
        phase: status.and_then(|status| status.phase.clone()),
        conditions,
        created_at: pvc
            .metadata
            .creation_timestamp
            .as_ref()
            .map(|timestamp| timestamp.0),
        collected_at,
    }
}

impl PersistentVolumeClaimsService {
    pub fn new() -> Self {
        PersistentVolumeClaimsService {}
    }

    async fn get_kube_client(cluster_config: &KubernetesClusterConfig) -> Result<Client, AppError> {
        let kubeconfig = if let Some(path) = &cluster_config.kube_config_path {
            Kubeconfig::read_from(path).map_err(|e| {
                AppError::ExternalService(format!("Failed to read kubeconfig from path: {}", e))
            })?
        } else {
            let infer_config = kube::Config::infer().await.map_err(|e| {
                AppError::ExternalService(format!("Failed to infer Kubernetes config: {}", e))
            })?;
            return Client::try_from(infer_config).map_err(|e| {
                AppError::ExternalService(format!(
                    "Failed to create Kubernetes client from inferred config: {}",
                    e
                ))
            });
        };

        let client_config = KubeConfig::from_custom_kubeconfig(
            kubeconfig,
            &KubeConfigOptions {
                context: cluster_config.kube_context.clone(),
                cluster: None,
                user: None,
            },
        )
        .await
        .map_err(|e| {
            AppError::ExternalService(format!("Failed to create Kubernetes client config: {}", e))
        })?;

        Client::try_from(client_config).map_err(|e| {
            AppError::ExternalService(format!("Failed to create Kubernetes client: {}", e))
        })
    }

    pub async fn list_persistent_volume_claims(
        &self,
        cluster_config: &KubernetesClusterConfig,
        namespace: &str,
    ) -> Result<Vec<PersistentVolumeClaimInfo>, AppError> {
        let client = Self::get_kube_client(cluster_config).await?;
        let api: Api<PersistentVolumeClaim> = Api::namespaced(client, namespace);
        let lp = ListParams::default();
        let pvc_list = api.list(&lp).await.map_err(|e| {
            AppError::ExternalService(format!(
                "Failed to list PVCs in namespace '{}': {}",
                namespace, e
            ))
        })?;

        let mut infos = Vec::new();
        for pvc in pvc_list {
            let name = pvc.name_any();
            let status = pvc
                .status
                .as_ref()
                .and_then(|s| s.phase.clone())
                .unwrap_or_else(|| "Unknown".to_string());
            let volume = pvc
                .spec
                .as_ref()
                .and_then(|s| s.volume_name.clone())
                .unwrap_or_else(|| "-".to_string());

            let capacity = pvc
                .status
                .as_ref()
                .and_then(|s| s.capacity.as_ref())
                .and_then(|cap_map| cap_map.get("storage"))
                .map(|q: &Quantity| q.0.clone());

            let access_modes = pvc
                .spec
                .as_ref()
                .and_then(|s| s.access_modes.as_ref())
                .map_or_else(Vec::new, |modes| modes.clone());

            let storage_class = pvc.spec.as_ref().and_then(|s| s.storage_class_name.clone());

            let age = pvc.metadata.creation_timestamp.as_ref().map_or_else(
                || "Unknown".to_string(),
                |ts| {
                    let creation_time = ts.0;
                    let duration = Utc::now().signed_duration_since(creation_time);
                    if duration.num_days() > 0 {
                        format!("{}d", duration.num_days())
                    } else if duration.num_hours() > 0 {
                        format!("{}h", duration.num_hours())
                    } else if duration.num_minutes() > 0 {
                        format!("{}m", duration.num_minutes())
                    } else {
                        format!("{}s", duration.num_seconds())
                    }
                },
            );

            infos.push(PersistentVolumeClaimInfo {
                name,
                namespace: namespace.to_string(),
                status,
                volume,
                capacity,
                access_modes,
                storage_class,
                age,
            });
        }
        Ok(infos)
    }

    pub async fn list_inventory(
        &self,
        cluster_config: &KubernetesClusterConfig,
        cluster_id: &str,
        namespace: Option<&str>,
    ) -> Result<Vec<PersistentVolumeClaimInventoryItem>, AppError> {
        let namespace = namespace
            .map(str::trim)
            .filter(|namespace| !namespace.is_empty());
        let namespace_arg = namespace.unwrap_or("");
        let fallback_namespace = namespace
            .filter(|namespace| *namespace != "all")
            .unwrap_or("");
        let client = Self::get_kube_client(cluster_config).await?;
        let api: Api<PersistentVolumeClaim> = if namespace_arg.is_empty() || namespace_arg == "all"
        {
            Api::all(client)
        } else {
            Api::namespaced(client, namespace_arg)
        };
        let collected_at = Utc::now();
        let pvc_list = api.list(&ListParams::default()).await.map_err(|e| {
            AppError::ExternalService(format!("Failed to list PVC inventory: {}", e))
        })?;

        let mut inventory = pvc_list
            .items
            .iter()
            .map(|pvc| {
                convert_kube_persistent_volume_claim_to_inventory(
                    pvc,
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

    pub async fn get_persistent_volume_claim_details(
        &self,
        cluster_config: &KubernetesClusterConfig,
        namespace: &str,
        name: &str,
    ) -> Result<PersistentVolumeClaim, AppError> {
        let client = Self::get_kube_client(cluster_config).await?;
        let api: Api<PersistentVolumeClaim> = Api::namespaced(client, namespace);
        api.get(name).await.map_err(|e| {
            AppError::ExternalService(format!(
                "Failed to get PVC '{}' in namespace '{}': {}",
                name, namespace, e
            ))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use k8s_openapi::api::core::v1::{
        PersistentVolumeClaimCondition, PersistentVolumeClaimSpec, PersistentVolumeClaimStatus,
        ResourceRequirements,
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
    fn persistent_volume_claim_inventory_conversion_preserves_metadata_spec_status() {
        let created_at = Utc.with_ymd_and_hms(2026, 6, 1, 12, 0, 0).unwrap();
        let collected_at = Utc.with_ymd_and_hms(2026, 6, 10, 0, 0, 0).unwrap();
        let pvc = PersistentVolumeClaim {
            metadata: ObjectMeta {
                name: Some("data".to_string()),
                namespace: Some("apps".to_string()),
                labels: Some(map(&[("team", "storage")])),
                annotations: Some(map(&[("cost-center", "cc-42")])),
                creation_timestamp: Some(Time(created_at)),
                ..Default::default()
            },
            spec: Some(PersistentVolumeClaimSpec {
                access_modes: Some(vec!["ReadWriteOnce".to_string()]),
                resources: Some(ResourceRequirements {
                    requests: Some(quantities(&[("storage", "100Gi")])),
                    ..Default::default()
                }),
                storage_class_name: Some("fast".to_string()),
                volume_mode: Some("Filesystem".to_string()),
                volume_name: Some("pv-fast-1".to_string()),
                ..Default::default()
            }),
            status: Some(PersistentVolumeClaimStatus {
                access_modes: Some(vec!["ReadWriteOnce".to_string()]),
                capacity: Some(quantities(&[("storage", "100Gi")])),
                conditions: Some(vec![PersistentVolumeClaimCondition {
                    type_: "FileSystemResizePending".to_string(),
                    status: "False".to_string(),
                    reason: Some("Complete".to_string()),
                    message: Some("Resize complete".to_string()),
                    ..Default::default()
                }]),
                phase: Some("Bound".to_string()),
                ..Default::default()
            }),
        };

        let item = convert_kube_persistent_volume_claim_to_inventory(
            &pvc,
            "cluster-a",
            "fallback",
            collected_at,
        );

        assert_eq!(item.cluster_id, "cluster-a");
        assert_eq!(item.namespace, "apps");
        assert_eq!(item.name, "data");
        assert_eq!(item.labels["team"], "storage");
        assert_eq!(item.annotations["cost-center"], "cc-42");
        assert_eq!(item.created_at, Some(created_at));
        assert_eq!(item.collected_at, collected_at);
        assert_eq!(item.requested_storage["storage"], "100Gi");
        assert_eq!(item.capacity["storage"], "100Gi");
        assert_eq!(item.access_modes, vec!["ReadWriteOnce".to_string()]);
        assert_eq!(item.storage_class_name.as_deref(), Some("fast"));
        assert_eq!(item.volume_mode.as_deref(), Some("Filesystem"));
        assert_eq!(item.volume_name.as_deref(), Some("pv-fast-1"));
        assert_eq!(item.phase.as_deref(), Some("Bound"));
        assert_eq!(item.conditions.len(), 1);
        assert_eq!(item.conditions[0].condition_type, "FileSystemResizePending");
    }

    #[test]
    fn persistent_volume_claim_inventory_conversion_uses_fallback_namespace_and_spec_access_modes()
    {
        let collected_at = Utc.with_ymd_and_hms(2026, 6, 10, 0, 0, 0).unwrap();
        let pvc = PersistentVolumeClaim {
            metadata: ObjectMeta {
                name: Some("scratch".to_string()),
                ..Default::default()
            },
            spec: Some(PersistentVolumeClaimSpec {
                access_modes: Some(vec!["ReadWriteMany".to_string()]),
                ..Default::default()
            }),
            status: None,
        };

        let item = convert_kube_persistent_volume_claim_to_inventory(
            &pvc,
            "cluster-a",
            "requested-namespace",
            collected_at,
        );

        assert_eq!(item.namespace, "requested-namespace");
        assert_eq!(item.name, "scratch");
        assert_eq!(item.access_modes, vec!["ReadWriteMany".to_string()]);
        assert!(item.requested_storage.is_empty());
        assert!(item.capacity.is_empty());
        assert!(item.conditions.is_empty());
        assert!(item.phase.is_none());
    }
}

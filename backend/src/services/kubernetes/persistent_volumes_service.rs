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
use k8s_openapi::api::core::v1::PersistentVolume;
use k8s_openapi::apimachinery::pkg::api::resource::Quantity;
use kube::api::ListParams;
use kube::config::{Config as KubeConfig, KubeConfigOptions, Kubeconfig};
use kube::{Api, Client, ResourceExt};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use crate::errors::AppError;
use crate::models::cluster::KubernetesClusterConfig;
use crate::services::kubernetes::persistent_volume_inventory::{
    PersistentVolumeClaimRefInventoryItem, PersistentVolumeInventoryItem,
};

#[derive(Debug, Serialize, Deserialize)]
pub struct PersistentVolumeInfo {
    pub name: String,
    pub capacity: Option<String>,
    pub access_modes: Vec<String>,
    pub reclaim_policy: String,
    pub status: String,
    pub claim: String, // Namespace/Name of the bound PVC
    pub storage_class: String,
    pub reason: String, // Reason for status, if any
    pub age: String,
}

pub struct PersistentVolumesService;

fn quantity_map_to_strings(map: &BTreeMap<String, Quantity>) -> BTreeMap<String, String> {
    map.iter()
        .map(|(key, value)| (key.clone(), value.0.clone()))
        .collect()
}

fn persistent_volume_source_types(pv: &PersistentVolume) -> Vec<String> {
    let mut source_types = Vec::new();
    let Some(spec) = pv.spec.as_ref() else {
        return source_types;
    };

    if spec.aws_elastic_block_store.is_some() {
        source_types.push("awsElasticBlockStore".to_string());
    }
    if spec.azure_disk.is_some() {
        source_types.push("azureDisk".to_string());
    }
    if spec.azure_file.is_some() {
        source_types.push("azureFile".to_string());
    }
    if spec.cephfs.is_some() {
        source_types.push("cephfs".to_string());
    }
    if spec.cinder.is_some() {
        source_types.push("cinder".to_string());
    }
    if spec.csi.is_some() {
        source_types.push("csi".to_string());
    }
    if spec.fc.is_some() {
        source_types.push("fc".to_string());
    }
    if spec.flex_volume.is_some() {
        source_types.push("flexVolume".to_string());
    }
    if spec.flocker.is_some() {
        source_types.push("flocker".to_string());
    }
    if spec.gce_persistent_disk.is_some() {
        source_types.push("gcePersistentDisk".to_string());
    }
    if spec.glusterfs.is_some() {
        source_types.push("glusterfs".to_string());
    }
    if spec.host_path.is_some() {
        source_types.push("hostPath".to_string());
    }
    if spec.iscsi.is_some() {
        source_types.push("iscsi".to_string());
    }
    if spec.local.is_some() {
        source_types.push("local".to_string());
    }
    if spec.nfs.is_some() {
        source_types.push("nfs".to_string());
    }
    if spec.photon_persistent_disk.is_some() {
        source_types.push("photonPersistentDisk".to_string());
    }
    if spec.portworx_volume.is_some() {
        source_types.push("portworxVolume".to_string());
    }
    if spec.quobyte.is_some() {
        source_types.push("quobyte".to_string());
    }
    if spec.rbd.is_some() {
        source_types.push("rbd".to_string());
    }
    if spec.scale_io.is_some() {
        source_types.push("scaleIO".to_string());
    }
    if spec.storageos.is_some() {
        source_types.push("storageOS".to_string());
    }
    if spec.vsphere_volume.is_some() {
        source_types.push("vsphereVolume".to_string());
    }

    source_types
}

fn convert_kube_persistent_volume_to_inventory(
    pv: &PersistentVolume,
    cluster_id: &str,
    collected_at: DateTime<Utc>,
) -> PersistentVolumeInventoryItem {
    let spec = pv.spec.as_ref();
    let status = pv.status.as_ref();
    let csi = spec.and_then(|spec| spec.csi.as_ref());
    let claim_ref = spec
        .and_then(|spec| spec.claim_ref.as_ref())
        .map(|claim_ref| PersistentVolumeClaimRefInventoryItem {
            namespace: claim_ref.namespace.clone(),
            name: claim_ref.name.clone(),
        });

    PersistentVolumeInventoryItem {
        cluster_id: cluster_id.to_string(),
        name: pv.name_any(),
        labels: pv.metadata.labels.clone().unwrap_or_default(),
        annotations: pv.metadata.annotations.clone().unwrap_or_default(),
        capacity: spec
            .and_then(|spec| spec.capacity.as_ref())
            .map(quantity_map_to_strings)
            .unwrap_or_default(),
        access_modes: spec
            .and_then(|spec| spec.access_modes.clone())
            .unwrap_or_default(),
        reclaim_policy: spec.and_then(|spec| spec.persistent_volume_reclaim_policy.clone()),
        phase: status.and_then(|status| status.phase.clone()),
        reason: status.and_then(|status| status.reason.clone()),
        claim_ref,
        storage_class_name: spec.and_then(|spec| spec.storage_class_name.clone()),
        volume_mode: spec.and_then(|spec| spec.volume_mode.clone()),
        source_types: persistent_volume_source_types(pv),
        csi_driver: csi.map(|csi| csi.driver.clone()),
        csi_volume_handle_present: csi
            .map(|csi| !csi.volume_handle.trim().is_empty())
            .unwrap_or(false),
        created_at: pv
            .metadata
            .creation_timestamp
            .as_ref()
            .map(|timestamp| timestamp.0),
        collected_at,
    }
}

impl PersistentVolumesService {
    pub fn new() -> Self {
        PersistentVolumesService {}
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

    pub async fn list_persistent_volumes(
        &self,
        cluster_config: &KubernetesClusterConfig,
    ) -> Result<Vec<PersistentVolumeInfo>, AppError> {
        let client = Self::get_kube_client(cluster_config).await?;
        let api: Api<PersistentVolume> = Api::all(client);
        let lp = ListParams::default();
        let pv_list = api
            .list(&lp)
            .await
            .map_err(|e| AppError::ExternalService(format!("Failed to list PVs: {}", e)))?;

        let mut infos = Vec::new();
        for pv in pv_list {
            let name = pv.name_any();
            let spec = pv.spec.as_ref();
            let status = pv.status.as_ref();

            let capacity = spec
                .and_then(|s| s.capacity.as_ref())
                .and_then(|cap_map| cap_map.get("storage"))
                .map(|q: &Quantity| q.0.clone());

            let access_modes = spec
                .and_then(|s| s.access_modes.as_ref())
                .map_or_else(Vec::new, |modes| modes.clone());

            let reclaim_policy = spec
                .and_then(|s| s.persistent_volume_reclaim_policy.as_ref())
                .cloned()
                .unwrap_or_else(|| "Unknown".to_string());

            let pv_status = status
                .and_then(|s| s.phase.clone())
                .unwrap_or_else(|| "Unknown".to_string());
            let reason = status
                .and_then(|s| s.reason.clone())
                .unwrap_or_else(|| "".to_string());

            let claim_ref = spec.and_then(|s| s.claim_ref.as_ref());
            let claim = claim_ref.map_or_else(
                || "-".to_string(),
                |cr| {
                    format!(
                        "{}/{}",
                        cr.namespace.as_deref().unwrap_or(""),
                        cr.name.as_deref().unwrap_or("")
                    )
                },
            );

            let storage_class = spec
                .and_then(|s| s.storage_class_name.clone())
                .unwrap_or_else(|| "-".to_string());

            let age = pv.metadata.creation_timestamp.as_ref().map_or_else(
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

            infos.push(PersistentVolumeInfo {
                name,
                capacity,
                access_modes,
                reclaim_policy,
                status: pv_status,
                claim,
                storage_class,
                reason,
                age,
            });
        }
        Ok(infos)
    }

    pub async fn list_inventory(
        &self,
        cluster_config: &KubernetesClusterConfig,
        cluster_id: &str,
    ) -> Result<Vec<PersistentVolumeInventoryItem>, AppError> {
        let client = Self::get_kube_client(cluster_config).await?;
        let api: Api<PersistentVolume> = Api::all(client);
        let collected_at = Utc::now();
        let pv_list = api
            .list(&ListParams::default())
            .await
            .map_err(|e| AppError::ExternalService(format!("Failed to list PVs: {}", e)))?;

        let mut inventory = pv_list
            .items
            .iter()
            .map(|pv| convert_kube_persistent_volume_to_inventory(pv, cluster_id, collected_at))
            .collect::<Vec<_>>();
        inventory.sort_by(|left, right| left.name.cmp(&right.name));
        Ok(inventory)
    }

    pub async fn get_persistent_volume_details(
        &self,
        cluster_config: &KubernetesClusterConfig,
        name: &str,
    ) -> Result<PersistentVolume, AppError> {
        let client = Self::get_kube_client(cluster_config).await?;
        let api: Api<PersistentVolume> = Api::all(client);
        api.get(name)
            .await
            .map_err(|e| AppError::ExternalService(format!("Failed to get PV '{}': {}", name, e)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use k8s_openapi::api::core::v1::{
        CSIPersistentVolumeSource, HostPathVolumeSource, ObjectReference, PersistentVolumeSpec,
        PersistentVolumeStatus,
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
    fn persistent_volume_inventory_conversion_preserves_metadata_spec_status_and_csi() {
        let created_at = Utc.with_ymd_and_hms(2026, 6, 1, 12, 0, 0).unwrap();
        let collected_at = Utc.with_ymd_and_hms(2026, 6, 10, 0, 0, 0).unwrap();
        let pv = PersistentVolume {
            metadata: ObjectMeta {
                name: Some("pv-fast-1".to_string()),
                labels: Some(map(&[("team", "storage")])),
                annotations: Some(map(&[("cost-center", "cc-42")])),
                creation_timestamp: Some(Time(created_at)),
                ..Default::default()
            },
            spec: Some(PersistentVolumeSpec {
                access_modes: Some(vec!["ReadWriteOnce".to_string()]),
                capacity: Some(quantities(&[("storage", "100Gi")])),
                claim_ref: Some(ObjectReference {
                    namespace: Some("apps".to_string()),
                    name: Some("data".to_string()),
                    ..Default::default()
                }),
                csi: Some(CSIPersistentVolumeSource {
                    driver: "ebs.csi.aws.com".to_string(),
                    volume_handle: "vol-123".to_string(),
                    ..Default::default()
                }),
                persistent_volume_reclaim_policy: Some("Retain".to_string()),
                storage_class_name: Some("fast".to_string()),
                volume_mode: Some("Filesystem".to_string()),
                ..Default::default()
            }),
            status: Some(PersistentVolumeStatus {
                phase: Some("Bound".to_string()),
                reason: Some("Ready".to_string()),
                ..Default::default()
            }),
        };

        let item = convert_kube_persistent_volume_to_inventory(&pv, "cluster-a", collected_at);

        assert_eq!(item.cluster_id, "cluster-a");
        assert_eq!(item.name, "pv-fast-1");
        assert_eq!(item.labels["team"], "storage");
        assert_eq!(item.annotations["cost-center"], "cc-42");
        assert_eq!(item.created_at, Some(created_at));
        assert_eq!(item.collected_at, collected_at);
        assert_eq!(item.capacity["storage"], "100Gi");
        assert_eq!(item.access_modes, vec!["ReadWriteOnce".to_string()]);
        assert_eq!(item.reclaim_policy.as_deref(), Some("Retain"));
        assert_eq!(item.phase.as_deref(), Some("Bound"));
        assert_eq!(item.reason.as_deref(), Some("Ready"));
        let claim_ref = item.claim_ref.expect("claim ref");
        assert_eq!(claim_ref.namespace.as_deref(), Some("apps"));
        assert_eq!(claim_ref.name.as_deref(), Some("data"));
        assert_eq!(item.storage_class_name.as_deref(), Some("fast"));
        assert_eq!(item.volume_mode.as_deref(), Some("Filesystem"));
        assert_eq!(item.source_types, vec!["csi".to_string()]);
        assert_eq!(item.csi_driver.as_deref(), Some("ebs.csi.aws.com"));
        assert!(item.csi_volume_handle_present);
    }

    #[test]
    fn persistent_volume_inventory_conversion_detects_host_path_and_missing_optional_state() {
        let collected_at = Utc.with_ymd_and_hms(2026, 6, 10, 0, 0, 0).unwrap();
        let pv = PersistentVolume {
            metadata: ObjectMeta {
                name: Some("pv-local-dev".to_string()),
                ..Default::default()
            },
            spec: Some(PersistentVolumeSpec {
                host_path: Some(HostPathVolumeSource {
                    path: "/var/lib/dev".to_string(),
                    type_: Some("DirectoryOrCreate".to_string()),
                }),
                ..Default::default()
            }),
            status: None,
        };

        let item = convert_kube_persistent_volume_to_inventory(&pv, "cluster-a", collected_at);

        assert_eq!(item.name, "pv-local-dev");
        assert_eq!(item.source_types, vec!["hostPath".to_string()]);
        assert!(item.capacity.is_empty());
        assert!(item.access_modes.is_empty());
        assert!(item.claim_ref.is_none());
        assert!(item.phase.is_none());
        assert!(item.reason.is_none());
        assert!(item.csi_driver.is_none());
        assert!(!item.csi_volume_handle_present);
    }
}

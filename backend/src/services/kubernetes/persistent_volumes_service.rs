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


use chrono::Utc;
use k8s_openapi::api::core::v1::PersistentVolume;
use k8s_openapi::apimachinery::pkg::api::resource::Quantity;
use kube::api::ListParams;
use kube::config::{Config as KubeConfig, KubeConfigOptions, Kubeconfig};
use kube::{Api, Client, ResourceExt};
use serde::{Deserialize, Serialize};

use crate::errors::AppError;
use crate::models::cluster::KubernetesClusterConfig;

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

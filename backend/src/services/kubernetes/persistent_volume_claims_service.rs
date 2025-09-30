use chrono::Utc;
use k8s_openapi::api::core::v1::PersistentVolumeClaim;
use k8s_openapi::apimachinery::pkg::api::resource::Quantity;
use kube::api::ListParams;
use kube::config::{Config as KubeConfig, KubeConfigOptions, Kubeconfig};
use kube::{Api, Client, ResourceExt};
use serde::{Deserialize, Serialize};

use crate::errors::AppError;
use crate::models::cluster::KubernetesClusterConfig;

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

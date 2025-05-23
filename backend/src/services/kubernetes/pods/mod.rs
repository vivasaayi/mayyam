// filepath: /Users/rajanpanneerselvam/work/mayyam/backend/src/services/kubernetes/pods.rs
use kube::{Client, Api};
use kube::config::{Kubeconfig, KubeConfigOptions, Config as KubeConfig};
use k8s_openapi::api::core::v1::Pod;
use serde::{Serialize, Deserialize};
use std::sync::Arc;
use crate::errors::AppError;
use crate::models::cluster::KubernetesClusterConfig; // Assuming this path is correct

#[derive(Debug, Serialize, Deserialize)]
pub struct PodInfo {
    pub name: String,
    pub namespace: String,
    pub status: String,
    pub age: String, // Or a datetime type
    pub ip: Option<String>,
    pub node: Option<String>,
}

pub struct PodService {
    // If you have a shared Kubernetes client or factory, it could be a field here.
    // For now, we'll create clients on-demand or per-instance.
}

impl PodService {
    pub fn new() -> Self {
        PodService {}
    }

    async fn get_kube_client(cluster_config: &KubernetesClusterConfig) -> Result<Client, AppError> {
        let kubeconfig = if let Some(path) = &cluster_config.kube_config_path {
            Kubeconfig::read_from(path).map_err(|e| AppError::ExternalService(format!("Failed to read kubeconfig from path: {}", e)))?
        } else {
            let infer_config = kube::Config::infer().await.map_err(|e| AppError::ExternalService(format!("Failed to infer Kubernetes config: {}", e)))?;
            return Client::try_from(infer_config).map_err(|e| AppError::ExternalService(format!("Failed to create Kubernetes client from inferred config: {}", e)));
        };
        
        let client_config = KubeConfig::from_custom_kubeconfig(kubeconfig, &KubeConfigOptions {
            context: cluster_config.kube_context.clone(),
            cluster: None,
            user: None,
        }).await.map_err(|e| AppError::ExternalService(format!("Failed to create Kubernetes client config: {}", e)))?;

        Client::try_from(client_config).map_err(|e| AppError::ExternalService(format!("Failed to create Kubernetes client: {}", e)))
    }

    pub async fn list_pods(
        &self,
        cluster_config: &KubernetesClusterConfig, // Pass the specific cluster config
        namespace: &str,
    ) -> Result<Vec<PodInfo>, AppError> {
        let client = Self::get_kube_client(cluster_config).await?;
        let pods_api: Api<Pod> = Api::namespaced(client, namespace);

        let lp = kube::api::ListParams::default();
        let pod_list = pods_api.list(&lp).await.map_err(|e| {
            AppError::ExternalService(format!("Failed to list pods in namespace '{}': {}", namespace, e))
        })?;

        let mut pod_infos = Vec::new();
        for p in pod_list {
            let name = p.metadata.name.unwrap_or_else(|| "Unknown".to_string());
            let status_phase = p.status.as_ref().and_then(|s| s.phase.clone()).unwrap_or_else(|| "Unknown".to_string());
            let ip = p.status.as_ref().and_then(|s| s.pod_ip.clone());
            let node_name = p.spec.as_ref().and_then(|s| s.node_name.clone());
            
            // Age calculation would be more complex, using creationTimestamp
            // For simplicity, using a placeholder.
            let age = p.metadata.creation_timestamp.as_ref().map_or_else(
                || "Unknown".to_string(),
                |ts| {
                    let now = chrono::Utc::now();
                    let creation_time = ts.0;
                    let duration = now.signed_duration_since(creation_time);
                    // Format duration as string, e.g., "2d3h", "5m"
                    // This is a simplified representation
                    if duration.num_days() > 0 {
                        format!("{}d", duration.num_days())
                    } else if duration.num_hours() > 0 {
                        format!("{}h", duration.num_hours())
                    } else if duration.num_minutes() > 0 {
                        format!("{}m", duration.num_minutes())
                    } else {
                        format!("{}s", duration.num_seconds())
                    }
                }
            );

            pod_infos.push(PodInfo {
                name,
                namespace: namespace.to_string(),
                status: status_phase,
                age,
                ip,
                node: node_name,
            });
        }
        Ok(pod_infos)
    }

    pub async fn get_pod_logs(
        &self,
        cluster_config: &KubernetesClusterConfig,
        namespace: &str,
        pod_name: &str,
    ) -> Result<String, AppError> {
        let client = Self::get_kube_client(cluster_config).await?;
        let pods_api: Api<Pod> = Api::namespaced(client, namespace);
        let lp = kube::api::LogParams::default();
        pods_api.logs(pod_name, &lp).await.map_err(|e| {
            AppError::ExternalService(format!(
                "Failed to get logs for pod \'{}\' in namespace \'{}\': {}",
                pod_name, namespace, e
            ))
        })
    }

    pub async fn delete_pod(
        &self,
        cluster_config: &KubernetesClusterConfig,
        namespace: &str,
        pod_name: &str,
    ) -> Result<(), AppError> {
        let client = Self::get_kube_client(cluster_config).await?;
        let pods_api: Api<Pod> = Api::namespaced(client, namespace);
        let dp = kube::api::DeleteParams::default();
        pods_api.delete(pod_name, &dp).await.map_err(|e| {
            AppError::ExternalService(format!(
                "Failed to delete pod \'{}\' in namespace \'{}\': {}",
                pod_name, namespace, e
            ))
        })?;
        Ok(())
    }

    pub async fn get_pod_details(
        &self,
        cluster_config: &KubernetesClusterConfig,
        namespace: &str,
        pod_name: &str,
    ) -> Result<Pod, AppError> {
        let client = Self::get_kube_client(cluster_config).await?;
        let pods_api: Api<Pod> = Api::namespaced(client, namespace);
        pods_api.get(pod_name).await.map_err(|e| {
            AppError::ExternalService(format!(
                "Failed to get details for pod \'{}\' in namespace \'{}\': {}",
                pod_name, namespace, e
            ))
        })
    }
}

// You might need to adjust the KubernetesClusterConfig model path
// if it's different from `crate::models::cluster::KubernetesClusterConfig`
// Also, ensure chrono is a dependency if you want to implement age calculation properly.
// Add `chrono = { version = "0.4", features = ["serde"] }` to Cargo.toml
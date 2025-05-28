use kube::{Client, Api, ResourceExt};
use kube::api::ListParams;
use kube::config::{Kubeconfig, KubeConfigOptions, Config as KubeConfig};
use k8s_openapi::api::core::v1::Namespace;
use serde::{Serialize, Deserialize};
use chrono::Utc;

use crate::errors::AppError;
use crate::models::cluster::KubernetesClusterConfig;

#[derive(Debug, Serialize, Deserialize)]
pub struct NamespaceInfo {
    pub name: String,
    pub status: String,
    pub age: String,
}

pub struct NamespacesService;

impl NamespacesService {
    pub fn new() -> Self {
        NamespacesService {}
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

    pub async fn list_namespaces(
        &self,
        cluster_config: &KubernetesClusterConfig,
    ) -> Result<Vec<NamespaceInfo>, AppError> {
        let client = Self::get_kube_client(cluster_config).await?;
        let api: Api<Namespace> = Api::all(client);
        let lp = ListParams::default();
        let ns_list = api.list(&lp).await.map_err(|e| {
            AppError::ExternalService(format!("Failed to list namespaces: {}", e))
        })?;

        let mut infos = Vec::new();
        for ns in ns_list {
            let name = ns.name_any();
            let status = ns.status.as_ref().and_then(|s| s.phase.clone()).unwrap_or_else(|| "Unknown".to_string());
            
            let age = ns.metadata.creation_timestamp.as_ref().map_or_else(
                || "Unknown".to_string(),
                |ts| {
                    let creation_time = ts.0;
                    let duration = Utc::now().signed_duration_since(creation_time);
                    if duration.num_days() > 0 { format!("{}d", duration.num_days()) }
                    else if duration.num_hours() > 0 { format!("{}h", duration.num_hours()) }
                    else if duration.num_minutes() > 0 { format!("{}m", duration.num_minutes()) }
                    else { format!("{}s", duration.num_seconds()) }
                }
            );

            infos.push(NamespaceInfo {
                name,
                status,
                age,
            });
        }
        Ok(infos)
    }

    pub async fn get_namespace_details(
        &self,
        cluster_config: &KubernetesClusterConfig,
        name: &str,
    ) -> Result<Namespace, AppError> {
        let client = Self::get_kube_client(cluster_config).await?;
        let api: Api<Namespace> = Api::all(client);
        api.get(name).await.map_err(|e| {
            AppError::ExternalService(format!("Failed to get namespace '{}': {}", name, e))
        })
    }
}

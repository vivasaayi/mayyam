use crate::services::kubernetes::client::ClientFactory;
use chrono::Utc;
use k8s_openapi::api::core::v1::Namespace;
use kube::api::{DeleteParams, ListParams, PostParams};
use kube::{Api, Client, ResourceExt};
use serde::{Deserialize, Serialize};

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
        ClientFactory::get_client(cluster_config).await
    }

    pub async fn list_namespaces(
        &self,
        cluster_config: &KubernetesClusterConfig,
    ) -> Result<Vec<NamespaceInfo>, AppError> {
        let client = Self::get_kube_client(cluster_config).await?;
        let api: Api<Namespace> = Api::all(client);
        let lp = ListParams::default();
        let ns_list = api
            .list(&lp)
            .await
            .map_err(|e| AppError::ExternalService(format!("Failed to list namespaces: {}", e)))?;

        let mut infos = Vec::new();
        for ns in ns_list {
            let name = ns.name_any();
            let status = ns
                .status
                .as_ref()
                .and_then(|s| s.phase.clone())
                .unwrap_or_else(|| "Unknown".to_string());

            let age = ns.metadata.creation_timestamp.as_ref().map_or_else(
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

            infos.push(NamespaceInfo { name, status, age });
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

    pub async fn create_namespace(
        &self,
        cluster_config: &KubernetesClusterConfig,
        name: &str,
        labels: Option<std::collections::BTreeMap<String, String>>,
    ) -> Result<Namespace, AppError> {
        let client = Self::get_kube_client(cluster_config).await?;
        let api: Api<Namespace> = Api::all(client);
        let ns = Namespace {
            metadata: kube::api::ObjectMeta {
                name: Some(name.to_string()),
                labels,
                ..Default::default()
            },
            ..Default::default()
        };
        api.create(&PostParams::default(), &ns).await.map_err(|e| {
            AppError::ExternalService(format!("Failed to create namespace '{}': {}", name, e))
        })
    }

    pub async fn delete_namespace(
        &self,
        cluster_config: &KubernetesClusterConfig,
        name: &str,
    ) -> Result<(), AppError> {
        let client = Self::get_kube_client(cluster_config).await?;
        let api: Api<Namespace> = Api::all(client);
        api.delete(name, &DeleteParams::default())
            .await
            .map_err(|e| {
                AppError::ExternalService(format!("Failed to delete namespace '{}': {}", name, e))
            })?;
        Ok(())
    }
}

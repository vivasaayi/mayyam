use chrono::Utc;
use k8s_openapi::api::core::v1::Node;
use kube::api::ListParams;
use kube::config::{Config as KubeConfig, KubeConfigOptions, Kubeconfig};
use kube::{Api, Client, ResourceExt};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use crate::errors::AppError;
use crate::models::cluster::KubernetesClusterConfig;

#[derive(Debug, Serialize, Deserialize)]
pub struct NodeCondition {
    pub type_: String,
    pub status: String,
    pub last_heartbeat_time: Option<String>,
    pub last_transition_time: Option<String>,
    pub reason: Option<String>,
    pub message: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct NodeInfo {
    pub name: String,
    pub status: String, // Simplified status (e.g., Ready, NotReady)
    pub roles: Vec<String>,
    pub age: String,
    pub version: String, // Kubelet version
    pub internal_ip: String,
    pub external_ip: Option<String>,
    pub os_image: String,
    pub kernel_version: String,
    pub container_runtime_version: String,
    // pub conditions: Vec<NodeCondition>, // Can be too verbose for list view
}

pub struct NodesService;

impl NodesService {
    pub fn new() -> Self {
        NodesService {}
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

    fn get_node_status(node: &Node) -> String {
        if let Some(conditions) = &node.status.as_ref().and_then(|s| s.conditions.as_ref()) {
            for condition in &**conditions {
                if condition.type_ == "Ready" {
                    return if condition.status == "True" {
                        "Ready".to_string()
                    } else {
                        format!(
                            "NotReady ({})",
                            condition.reason.as_deref().unwrap_or("Unknown")
                        )
                    };
                }
            }
        }
        "Unknown".to_string()
    }

    fn get_node_roles(node: &Node) -> Vec<String> {
        let mut roles = Vec::new();
        if let Some(labels) = &node.metadata.labels {
            for (key, value) in labels {
                if key.starts_with("node-role.kubernetes.io/") && value == "true" {
                    roles.push(
                        key.trim_start_matches("node-role.kubernetes.io/")
                            .to_string(),
                    );
                }
                // Check for common master/control-plane labels
                if key == "kubernetes.io/role" && (value == "master" || value == "control-plane") {
                    if !roles.contains(&value) {
                        // Avoid duplicates if specific role label also exists
                        roles.push(value.clone());
                    }
                }
            }
        }
        if roles.is_empty() {
            roles.push("<none>".to_string());
        }
        roles.sort();
        roles
    }

    pub async fn list_nodes(
        &self,
        cluster_config: &KubernetesClusterConfig,
    ) -> Result<Vec<NodeInfo>, AppError> {
        let client = Self::get_kube_client(cluster_config).await?;
        let api: Api<Node> = Api::all(client);
        let lp = ListParams::default();
        let node_list = api
            .list(&lp)
            .await
            .map_err(|e| AppError::ExternalService(format!("Failed to list nodes: {}", e)))?;

        let mut infos = Vec::new();
        for n in node_list {
            let name = n.name_any();
            let status = Self::get_node_status(&n);
            let roles = Self::get_node_roles(&n);

            let age = n.metadata.creation_timestamp.as_ref().map_or_else(
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

            let node_info = n.status.as_ref().and_then(|s| s.node_info.as_ref());
            let version =
                node_info.map_or_else(|| "Unknown".to_string(), |ni| ni.kubelet_version.clone());
            let os_image =
                node_info.map_or_else(|| "Unknown".to_string(), |ni| ni.os_image.clone());
            let kernel_version =
                node_info.map_or_else(|| "Unknown".to_string(), |ni| ni.kernel_version.clone());
            let container_runtime_version = node_info.map_or_else(
                || "Unknown".to_string(),
                |ni| ni.container_runtime_version.clone(),
            );

            let mut internal_ip = "Unknown".to_string();
            let mut external_ip = None;
            if let Some(addresses) = n.status.as_ref().and_then(|s| s.addresses.as_ref()) {
                for addr in addresses {
                    if addr.type_ == "InternalIP" {
                        internal_ip = addr.address.clone();
                    }
                    if addr.type_ == "ExternalIP" {
                        external_ip = Some(addr.address.clone());
                    }
                }
            }

            infos.push(NodeInfo {
                name,
                status,
                roles,
                age,
                version,
                internal_ip,
                external_ip,
                os_image,
                kernel_version,
                container_runtime_version,
            });
        }
        Ok(infos)
    }

    pub async fn get_node_details(
        &self,
        cluster_config: &KubernetesClusterConfig,
        name: &str,
    ) -> Result<Node, AppError> {
        let client = Self::get_kube_client(cluster_config).await?;
        let api: Api<Node> = Api::all(client);
        api.get(name)
            .await
            .map_err(|e| AppError::ExternalService(format!("Failed to get node '{}': {}", name, e)))
    }
}

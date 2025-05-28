// filepath: /Users/rajanpanneerselvam/work/mayyam/backend/src/services/kubernetes/pods.rs
use kube::{Client, Api, ResourceExt}; // Added ResourceExt
use kube::config::{Kubeconfig, KubeConfigOptions, Config as KubeConfig};
use k8s_openapi::api::core::v1::Pod;
use serde::{Serialize, Deserialize};
use std::sync::Arc;
use chrono::Utc; // Added Utc
use std::collections::BTreeMap; // Added BTreeMap

use crate::errors::AppError;
use crate::models::cluster::KubernetesClusterConfig;

#[derive(Debug, Serialize, Deserialize, Clone)] // Added Clone
pub struct ContainerInfo {
    pub name: String,
    pub image: String,
    pub ready: bool,
    pub restarts: i32,
    // Potentially add ports, env, mounts later if needed for detailed views
}

#[derive(Debug, Serialize, Deserialize, Clone)] // Added Clone
pub struct PodInfo {
    pub name: String,
    pub namespace: String,
    pub status: String, // e.g., Running, Pending, Succeeded, Failed, Unknown
    pub age: String,
    pub ip: Option<String>,
    pub node_name: Option<String>, // Renamed from 'node' for clarity
    pub containers: Vec<ContainerInfo>,
    pub restart_count: i32, // Total restarts for all containers in the pod
    pub controlled_by: Option<String>, // Name of the controller (e.g., ReplicaSet name)
    pub controller_kind: Option<String>, // Kind of the controller (e.g., ReplicaSet)
    pub labels: Option<BTreeMap<String, String>>,
    pub annotations: Option<BTreeMap<String, String>>,
    pub qos_class: Option<String>,
    // Add other fields as necessary, e.g., volumes, conditions
}

pub struct PodService; // Removed pub struct PodService { ... }

// Helper function to convert Kubernetes Pod to our PodInfo struct
// This can be used by other services like DeploymentsService, StatefulSetsService, etc.
pub fn convert_kube_pod_to_pod_info(pod: &Pod, current_namespace: &str) -> PodInfo {
    let pod_name = pod.name_any();
    let pod_namespace = pod.namespace().unwrap_or_else(|| current_namespace.to_string());

    let status_phase = pod.status.as_ref().and_then(|s| s.phase.clone()).unwrap_or_else(|| "Unknown".to_string());
    let pod_ip = pod.status.as_ref().and_then(|s| s.pod_ip.clone());
    let node_name = pod.spec.as_ref().and_then(|s| s.node_name.clone());
    
    let age = pod.metadata.creation_timestamp.as_ref().map_or_else(
        || "Unknown".to_string(),
        |ts| {
            let creation_time = ts.0;
            let duration = Utc::now().signed_duration_since(creation_time);
            if duration.num_days() > 0 { format!("{}d", duration.num_days()) }
            else if duration.num_hours() > 0 { format!("{}h", duration.num_hours()) }
            else if duration.num_minutes() > 0 { format!("{}m", duration.num_minutes()) }
            else { format!("{}s", duration.num_seconds().max(0)) } // Ensure non-negative seconds
        }
    );

    let mut container_infos = Vec::new();
    let mut total_restarts: i32 = 0;
    if let Some(spec_containers) = pod.spec.as_ref().map(|s| &s.containers) {
        let k8s_container_statuses = pod.status.as_ref().and_then(|s| s.container_statuses.as_ref());
        for container_spec in spec_containers {
            let status_opt = k8s_container_statuses.and_then(|statuses| {
                statuses.iter().find(|cs| cs.name == container_spec.name)
            });

            let ready = status_opt.map_or(false, |cs| cs.ready);
            let restarts = status_opt.map_or(0, |cs| cs.restart_count);
            total_restarts += restarts;

            container_infos.push(ContainerInfo {
                name: container_spec.name.clone(),
                image: container_spec.image.clone().unwrap_or_default(),
                ready,
                restarts,
            });
        }
    }
    
    let (controlled_by, controller_kind) = pod.metadata.owner_references.as_ref()
        .and_then(|owners| owners.first())
        .map_or((None, None), |owner_ref| (Some(owner_ref.name.clone()), Some(owner_ref.kind.clone())));

    PodInfo {
        name: pod_name,
        namespace: pod_namespace,
        status: status_phase,
        age,
        ip: pod_ip,
        node_name,
        containers: container_infos,
        restart_count: total_restarts,
        controlled_by,
        controller_kind,
        labels: pod.metadata.labels.clone(),
        annotations: pod.metadata.annotations.clone(),
        qos_class: pod.status.as_ref().and_then(|s| s.qos_class.clone()),
    }
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
        cluster_config: &KubernetesClusterConfig,
        namespace: &str,
    ) -> Result<Vec<PodInfo>, AppError> {
        let client = Self::get_kube_client(cluster_config).await?;
        let pods_api: Api<Pod> = Api::namespaced(client, namespace);

        let lp = kube::api::ListParams::default();
        let pod_list = pods_api.list(&lp).await.map_err(|e| {
            AppError::ExternalService(format!("Failed to list pods in namespace \'{}\': {}", namespace, e))
        })?;

        let pod_infos = pod_list.iter()
            .map(|p| convert_kube_pod_to_pod_info(p, namespace)) // Use the helper
            .collect();
        
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
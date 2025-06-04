use kube::{api::{DeleteParams, ListParams, LogParams, ObjectMeta}, config::{KubeConfigOptions, Kubeconfig, Config as KubeConfig}, Api, Client, ResourceExt};
use k8s_openapi::api::core::v1::{Pod, Event, PodSpec, PodStatus};
use serde::{Deserialize, Serialize};
use tracing::{debug, info, error};
use std::sync::Arc;
use chrono::Utc;
use std::collections::BTreeMap;

use crate::{models::cluster::KubernetesClusterConfig, errors::AppError};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PodDetail { 
    pub metadata: Option<ObjectMeta>,
    pub spec: Option<PodSpec>,
    pub status: Option<PodStatus>,
}

impl From<Pod> for PodDetail {
    fn from(pod: Pod) -> Self {
        PodDetail {
            metadata: Some(pod.metadata), // Corrected: Wrap with Some()
            spec: pod.spec,
            status: pod.status,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ContainerInfo {
    pub name: String,
    pub image: String,
    pub ready: bool,
    pub restarts: i32,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PodInfo {
    pub name: String,
    pub namespace: String,
    pub status: String, 
    pub age: String,
    pub ip: Option<String>,
    pub node_name: Option<String>,
    pub containers: Vec<ContainerInfo>,
    pub restart_count: i32, 
    pub controlled_by: Option<String>,
    pub controller_kind: Option<String>,
    pub labels: Option<BTreeMap<String, String>>,
    pub annotations: Option<BTreeMap<String, String>>,
    pub qos_class: Option<String>,
}

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
            else { format!("{}s", duration.num_seconds().max(0)) }
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

#[derive(Clone)]
pub struct PodService;

impl PodService {
    pub fn new() -> Self {
        PodService
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
        debug!(target: "mayyam::services::kubernetes::pod", cluster_name = cluster_config.api_server_url.as_deref().unwrap_or("unknown"), %namespace, "Listing pods");
        let client = Self::get_kube_client(cluster_config).await?;
        
        let api: Api<Pod> = if namespace.is_empty() || namespace == "all" { 
            Api::all(client)
        } else {
            Api::namespaced(client, namespace)
        };
        let lp = ListParams::default();
        match api.list(&lp).await {
            Ok(pod_list) => {
                info!(target: "mayyam::services::kubernetes::pod", cluster_name = cluster_config.api_server_url.as_deref().unwrap_or("unknown"), %namespace, count = pod_list.items.len(), "Successfully listed pods");
                let actual_namespace = if namespace.is_empty() || namespace == "all" { "" } else { namespace };
                let pod_infos = pod_list.iter()
                    .map(|p| convert_kube_pod_to_pod_info(p, actual_namespace))
                    .collect();
                Ok(pod_infos)
            }
            Err(e) => {
                error!(target: "mayyam::services::kubernetes::pod", cluster_name = cluster_config.api_server_url.as_deref().unwrap_or("unknown"), %namespace, error = %e, "Failed to list pods");
                Err(AppError::Kubernetes(e.to_string()))
            }
        }
    }

    pub async fn get_pod_details(
        &self,
        cluster_config: &KubernetesClusterConfig,
        namespace: &str,
        pod_name: &str,
    ) -> Result<PodDetail, AppError> {
        debug!(target: "mayyam::services::kubernetes::pod", cluster_name = cluster_config.api_server_url.as_deref().unwrap_or("unknown"), %namespace, %pod_name, "Getting pod details");
        let client = Self::get_kube_client(cluster_config).await?;
        let api: Api<Pod> = Api::namespaced(client, namespace);
        match api.get(pod_name).await {
            Ok(pod) => {
                info!(target: "mayyam::services::kubernetes::pod", cluster_name = cluster_config.api_server_url.as_deref().unwrap_or("unknown"), %namespace, %pod_name, "Successfully retrieved pod details");
                Ok(PodDetail::from(pod))
            }
            Err(e) => {
                error!(target: "mayyam::services::kubernetes::pod", cluster_name = cluster_config.api_server_url.as_deref().unwrap_or("unknown"), %namespace, %pod_name, error = %e, "Failed to get pod details");
                Err(AppError::Kubernetes(e.to_string()))
            }
        }
    }

    pub async fn get_pod_events(
        &self,
        cluster_config: &KubernetesClusterConfig,
        namespace: &str,
        pod_name: &str,
    ) -> Result<Vec<Event>, AppError> {
        debug!(target: "mayyam::services::kubernetes::pod", cluster_name = cluster_config.api_server_url.as_deref().unwrap_or("unknown"), %namespace, %pod_name, "Getting pod events");
        let client = Self::get_kube_client(cluster_config).await?;

        let pod_api: Api<Pod> = Api::namespaced(client.clone(), namespace);
        let pod_object = pod_api.get(pod_name).await.map_err(|e| {
            error!(target: "mayyam::services::kubernetes::pod", cluster_name = cluster_config.api_server_url.as_deref().unwrap_or("unknown"), %namespace, %pod_name, error = %e, "Failed to retrieve pod to get its UID for events");
            AppError::NotFound(format!("Could not retrieve pod '{}' to get its UID: {}", pod_name, e))
        })?;
        
        let pod_uid = pod_object.metadata.uid.ok_or_else(|| {
            error!(target: "mayyam::services::kubernetes::pod", cluster_name = cluster_config.api_server_url.as_deref().unwrap_or("unknown"), %namespace, %pod_name, "Pod is missing UID, cannot fetch events.");
            AppError::Internal(format!("Pod '{}' in namespace '{}' does not have a UID, cannot fetch events.", pod_name, namespace))
        })?;

        let event_api: Api<Event> = Api::namespaced(client, namespace);
        let lp = ListParams::default()
            .fields(&format!("involvedObject.uid={}", pod_uid))
            .timeout(10);

        match event_api.list(&lp).await {
            Ok(event_list) => {
                info!(target: "mayyam::services::kubernetes::pod", cluster_name = cluster_config.api_server_url.as_deref().unwrap_or("unknown"), %namespace, %pod_name, count = event_list.items.len(), "Successfully fetched pod events");
                Ok(event_list.items)
            }
            Err(e) => {
                error!(target: "mayyam::services::kubernetes::pod", cluster_name = cluster_config.api_server_url.as_deref().unwrap_or("unknown"), %namespace, %pod_name, error = %e, "Error fetching pod events");
                Err(AppError::Kubernetes(e.to_string()))
            }
        }
    }

    pub async fn get_pod_logs(
        &self,
        cluster_config: &KubernetesClusterConfig,
        namespace: &str,
        pod_name: &str,
        container_name: Option<&str>,
        previous: bool, 
        tail_lines: Option<i64>,
    ) -> Result<String, AppError> {
        debug!(target: "mayyam::services::kubernetes::pod", cluster_name = cluster_config.api_server_url.as_deref().unwrap_or("unknown"), %namespace, %pod_name, "Getting pod logs");
        let client = Self::get_kube_client(cluster_config).await?;
        let api: Api<Pod> = Api::namespaced(client, namespace);
        let mut lp = LogParams::default();
        if let Some(c_name) = container_name {
            lp.container = Some(c_name.to_string());
        }
        lp.previous = previous;
        lp.tail_lines = tail_lines;

        match api.logs(pod_name, &lp).await {
            Ok(logs) => {
                info!(target: "mayyam::services::kubernetes::pod", cluster_name = cluster_config.api_server_url.as_deref().unwrap_or("unknown"), %namespace, %pod_name, "Successfully fetched pod logs");
                Ok(logs)
            }
            Err(e) => {
                error!(target: "mayyam::services::kubernetes::pod", cluster_name = cluster_config.api_server_url.as_deref().unwrap_or("unknown"), %namespace, %pod_name, error = %e, "Error fetching pod logs");
                Err(AppError::Kubernetes(e.to_string()))
            }
        }
    }

    pub async fn delete_pod(
        &self,
        cluster_config: &KubernetesClusterConfig,
        namespace: &str,
        pod_name: &str,
    ) -> Result<(), AppError> {
        debug!(target: "mayyam::services::kubernetes::pod", cluster_name = cluster_config.api_server_url.as_deref().unwrap_or("unknown"), %namespace, %pod_name, "Deleting pod");
        let client = Self::get_kube_client(cluster_config).await?;
        let api: Api<Pod> = Api::namespaced(client, namespace);
        let dp = DeleteParams::default();
        match api.delete(pod_name, &dp).await {
            Ok(_) => {
                info!(target: "mayyam::services::kubernetes::pod", cluster_name = cluster_config.api_server_url.as_deref().unwrap_or("unknown"), %namespace, %pod_name, "Successfully deleted pod");
                Ok(())
            }
            Err(e) => {
                error!(target: "mayyam::services::kubernetes::pod", cluster_name = cluster_config.api_server_url.as_deref().unwrap_or("unknown"), %namespace, %pod_name, error = %e, "Error deleting pod");
                Err(AppError::Kubernetes(e.to_string()))
            }
        }
    }
}

impl Default for PodService {
    fn default() -> Self {
        Self::new()
    }
}

// filepath: /Users/rajanpanneerselvam/work/mayyam/backend/src/services/kubernetes/daemon_sets/mod.rs
use kube::{Client, Api, ResourceExt};
use kube::api::{ListParams, Patch, PatchParams, DeleteParams};
use kube::config::{Kubeconfig, KubeConfigOptions, Config as KubeConfig};
use k8s_openapi::api::apps::v1::DaemonSet;
use k8s_openapi::api::core::v1::Pod;
use k8s_openapi::apimachinery::pkg::apis::meta::v1::LabelSelector;
use serde::{Serialize, Deserialize};
use serde_json::json;
use std::collections::BTreeMap;
use chrono::Utc;

use crate::errors::AppError;
use crate::models::cluster::KubernetesClusterConfig;
// Use the PodInfo and convert_kube_pod_to_pod_info from the pods module
use crate::services::kubernetes::pods::{PodInfo, convert_kube_pod_to_pod_info};

#[derive(Debug, Serialize, Deserialize)]
pub struct DaemonSetInfo {
    pub name: String,
    pub namespace: String,
    pub desired_number_scheduled: i32,
    pub current_number_scheduled: i32,
    pub number_ready: i32,
    pub number_available: i32,
    pub number_misscheduled: i32,
    pub age: String,
    pub images: Vec<String>,
}

pub struct DaemonSetsService;

fn label_selector_to_string(selector: &LabelSelector) -> Option<String> {
    let mut parts = Vec::new();
    if let Some(match_labels) = &selector.match_labels {
        for (key, value) in match_labels {
            parts.push(format!("{}={}", key, value));
        }
    }
    if let Some(match_expressions) = &selector.match_expressions {
        for expr in match_expressions {
            let key = &expr.key;
            let op = expr.operator.as_str();
            match op {
                "In" => {
                    if let Some(values) = &expr.values {
                        parts.push(format!("{} in ({})", key, values.join(",")));
                    }
                }
                "NotIn" => {
                    if let Some(values) = &expr.values {
                        parts.push(format!("{} notin ({})", key, values.join(",")));
                    }
                }
                "Exists" => parts.push(key.to_string()),
                "DoesNotExist" => parts.push(format!("!{}", key)),
                _ => {} // Unknown operator, or let Kubernetes handle malformed selectors
            }
        }
    }
    if parts.is_empty() { None } else { Some(parts.join(",")) }
}

impl DaemonSetsService {
    pub fn new() -> Self {
        DaemonSetsService {}
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

    pub async fn list_daemon_sets(
        &self,
        cluster_config: &KubernetesClusterConfig,
        namespace: &str,
    ) -> Result<Vec<DaemonSetInfo>, AppError> {
        let client = Self::get_kube_client(cluster_config).await?;
        let api: Api<DaemonSet> = Api::namespaced(client, namespace);
        let lp = ListParams::default();
        let ds_list = api.list(&lp).await.map_err(|e| {
            AppError::ExternalService(format!("Failed to list daemon sets in namespace \'{}\': {}", namespace, e))
        })?;

        let mut infos = Vec::new();
        for ds in ds_list {
            let name = ds.name_any();
            let status = ds.status.as_ref();
            let spec = ds.spec.as_ref();

            let age = ds.metadata.creation_timestamp.as_ref().map_or_else(
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

            let images = spec
                .and_then(|s| s.template.spec.as_ref())
                .map(|pod_spec| {
                    pod_spec.containers.iter()
                        .filter_map(|c| c.image.clone())
                        .collect::<Vec<String>>()
                })
                .unwrap_or_default();

            infos.push(DaemonSetInfo {
                name,
                namespace: namespace.to_string(),
                desired_number_scheduled: status.map_or(0, |s| s.desired_number_scheduled),
                current_number_scheduled: status.map_or(0, |s| s.current_number_scheduled),
                number_ready: status.map_or(0, |s| s.number_ready),
                number_available: status.map_or(0, |s| s.number_available.unwrap_or(0)),
                number_misscheduled: status.map_or(0, |s| s.number_misscheduled),
                age,
                images,
            });
        }
        Ok(infos)
    }

    pub async fn get_daemon_set_details(
        &self,
        cluster_config: &KubernetesClusterConfig,
        namespace: &str,
        name: &str,
    ) -> Result<DaemonSet, AppError> {
        let client = Self::get_kube_client(cluster_config).await?;
        let api: Api<DaemonSet> = Api::namespaced(client, namespace);
        api.get(name).await.map_err(|e| {
            AppError::ExternalService(format!("Failed to get daemon set \'{}\' in namespace \'{}\': {}", name, namespace, e))
        })
    }

    pub async fn get_pods_for_daemon_set(
        &self,
        cluster_config: &KubernetesClusterConfig,
        namespace: &str,
        daemon_set_name: &str,
    ) -> Result<Vec<PodInfo>, AppError> {
        let client = Self::get_kube_client(cluster_config).await?;
        let daemon_set_api: Api<DaemonSet> = Api::namespaced(client.clone(), namespace);
        let pod_api: Api<Pod> = Api::namespaced(client, namespace);

        let daemon_set = daemon_set_api.get(daemon_set_name).await.map_err(|e| {
            AppError::ExternalService(format!(
                "Failed to get daemon set '{}' in namespace '{}': {}",
                daemon_set_name, namespace, e
            ))
        })?;

        let selector = daemon_set.spec.as_ref().map(|spec| &spec.selector);
        let label_selector_str = selector
            .and_then(label_selector_to_string) // Use the existing helper
            .unwrap_or_default();

        if label_selector_str.is_empty() {
            return Ok(vec![]); // No selector means no pods
        }

        let lp = ListParams::default().labels(&label_selector_str);
        let pod_list = pod_api.list(&lp).await.map_err(|e| {
            AppError::ExternalService(format!(
                "Failed to list pods for daemon set '{}' in namespace '{}' using selector '{}': {}",
                daemon_set_name, namespace, label_selector_str, e
            ))
        })?;

        let pod_infos = pod_list.iter()
            .map(|pod| convert_kube_pod_to_pod_info(pod, namespace)) // Use shared helper
            .collect();

        Ok(pod_infos)
    }

    pub async fn delete_daemon_set(
        &self,
        cluster_config: &KubernetesClusterConfig,
        namespace: &str,
        name: &str,
    ) -> Result<(), AppError> {
        let client = Self::get_kube_client(cluster_config).await?;
        let api: Api<DaemonSet> = Api::namespaced(client, namespace);
        api.delete(name, &DeleteParams::default()).await.map_err(|e| {
            AppError::ExternalService(format!("Failed to delete daemon set \'{}\' in namespace \'{}\': {}", name, namespace, e))
        })?;
        Ok(())
    }

    pub async fn restart_daemon_set(
        &self,
        cluster_config: &KubernetesClusterConfig,
        namespace: &str,
        name: &str,
    ) -> Result<(), AppError> {
        let client = Self::get_kube_client(cluster_config).await?;
        let api: Api<DaemonSet> = Api::namespaced(client, namespace);
        
        let mut annotations = BTreeMap::new();
        annotations.insert(
            "kubectl.kubernetes.io/restartedAt".to_string(),
            Utc::now().to_rfc3339(),
        );

        let patch = json!({
            "spec": {
                "template": {
                    "metadata": {
                        "annotations": annotations
                    }
                }
            }
        });

        api.patch(name, &PatchParams::default(), &Patch::Merge(&patch)).await.map_err(|e| {
            AppError::ExternalService(format!("Failed to restart daemon set \'{}\' in namespace \'{}\': {}", name, namespace, e))
        })?;
        Ok(())
    }

    pub async fn delete_all_pods_for_daemon_set(
        &self,
        cluster_config: &KubernetesClusterConfig,
        namespace: &str,
        daemon_set_name: &str,
    ) -> Result<(), AppError> {
        let client = Self::get_kube_client(cluster_config).await?;
        let ds_api: Api<DaemonSet> = Api::namespaced(client.clone(), namespace);
        
        let ds = ds_api.get(daemon_set_name).await.map_err(|e| AppError::ExternalService(format!("Failed to get daemon set \'{}\': {}", daemon_set_name, e)))?;
        
        let selector_string = ds.spec.as_ref()
            .map(|spec| spec.selector.clone())
            .and_then(|selector| label_selector_to_string(&selector));

        if let Some(sel) = selector_string {
            let pods_api: Api<Pod> = Api::namespaced(client, namespace);
            let lp = ListParams::default().labels(&sel);
            let pod_list = pods_api.list(&lp).await.map_err(|e| AppError::ExternalService(format!("Failed to list pods for daemon set \'{}\': {}", daemon_set_name, e)))?;
            
            for pod in pod_list {
                let pod_name = pod.name_any();
                pods_api.delete(&pod_name, &DeleteParams::default()).await.map_err(|e| {
                    AppError::ExternalService(format!("Failed to delete pod \'{}\' for daemon set \'{}\': {}", pod_name, daemon_set_name, e))
                })?;
            }
        }
        // If no selector, or selector doesn't match any pods, this is a no-op for pod deletion.
        Ok(())
    }
}

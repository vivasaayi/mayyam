// filepath: /Users/rajanpanneerselvam/work/mayyam/backend/src/services/kubernetes/stateful_sets_service.rs
use kube::{Client, Api, ResourceExt};
use kube::api::{ListParams, Patch, PatchParams, DeleteParams};
use kube::config::{Kubeconfig, KubeConfigOptions, Config as KubeConfig};
use k8s_openapi::api::apps::v1::StatefulSet;
use k8s_openapi::api::core::v1::Pod;
use k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta;
use serde::{Serialize, Deserialize};
use serde_json::json;
use std::collections::BTreeMap;
use chrono::Utc;

use crate::errors::AppError;
use crate::models::cluster::KubernetesClusterConfig;

#[derive(Debug, Serialize, Deserialize)]
pub struct StatefulSetInfo {
    pub name: String,
    pub namespace: String,
    pub replicas: i32,
    pub ready_replicas: i32,
    pub updated_replicas: i32,
    pub age: String,
    pub images: Vec<String>,
}

pub struct StatefulSetsService;

impl StatefulSetsService {
    pub fn new() -> Self {
        StatefulSetsService {}
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

    pub async fn list_stateful_sets(
        &self,
        cluster_config: &KubernetesClusterConfig,
        namespace: &str,
    ) -> Result<Vec<StatefulSetInfo>, AppError> {
        let client = Self::get_kube_client(cluster_config).await?;
        let api: Api<StatefulSet> = Api::namespaced(client, namespace);
        let lp = ListParams::default();
        let sts_list = api.list(&lp).await.map_err(|e| {
            AppError::ExternalService(format!("Failed to list stateful sets in namespace '{}': {}", namespace, e))
        })?;

        let mut infos = Vec::new();
        for s in sts_list {
            let name = s.name_any();
            let replicas = s.spec.as_ref().and_then(|s| s.replicas).unwrap_or(0);
            let ready_replicas = s.status.as_ref().and_then(|s| s.ready_replicas).unwrap_or(0);
            let updated_replicas = s.status.as_ref().and_then(|s| s.updated_replicas).unwrap_or(0);
            
            let age = s.metadata.creation_timestamp.as_ref().map_or_else(
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
                }
            );

            let images = s.spec.as_ref()
                .and_then(|s| s.template.spec.as_ref())
                .map(|pod_spec| {
                    pod_spec.containers.iter()
                        .filter_map(|c| c.image.clone())
                        .collect::<Vec<String>>()
                })
                .unwrap_or_default();

            infos.push(StatefulSetInfo {
                name,
                namespace: namespace.to_string(),
                replicas,
                ready_replicas,
                updated_replicas,
                age,
                images,
            });
        }
        Ok(infos)
    }

    pub async fn get_stateful_set_details(
        &self,
        cluster_config: &KubernetesClusterConfig,
        namespace: &str,
        name: &str,
    ) -> Result<StatefulSet, AppError> {
        let client = Self::get_kube_client(cluster_config).await?;
        let api: Api<StatefulSet> = Api::namespaced(client, namespace);
        api.get(name).await.map_err(|e| {
            AppError::ExternalService(format!("Failed to get stateful set '{}' in namespace '{}': {}", name, namespace, e))
        })
    }

    pub async fn delete_stateful_set(
        &self,
        cluster_config: &KubernetesClusterConfig,
        namespace: &str,
        name: &str,
    ) -> Result<(), AppError> {
        let client = Self::get_kube_client(cluster_config).await?;
        let api: Api<StatefulSet> = Api::namespaced(client, namespace);
        api.delete(name, &DeleteParams::default()).await.map_err(|e| {
            AppError::ExternalService(format!("Failed to delete stateful set '{}' in namespace '{}': {}", name, namespace, e))
        })?;
        Ok(())
    }

    pub async fn scale_stateful_set(
        &self,
        cluster_config: &KubernetesClusterConfig,
        namespace: &str,
        name: &str,
        replicas: i32,
    ) -> Result<(), AppError> {
        let client = Self::get_kube_client(cluster_config).await?;
        let api: Api<StatefulSet> = Api::namespaced(client, namespace);
        let patch = json!({
            "spec": { "replicas": replicas }
        });
        api.patch_scale(name, &PatchParams::default(), &Patch::Merge(&patch)).await.map_err(|e| {
            AppError::ExternalService(format!("Failed to scale stateful set '{}' in namespace '{}': {}", name, namespace, e))
        })?;
        Ok(())
    }

    pub async fn restart_stateful_set(
        &self,
        cluster_config: &KubernetesClusterConfig,
        namespace: &str,
        name: &str,
    ) -> Result<(), AppError> {
        let client = Self::get_kube_client(cluster_config).await?;
        let api: Api<StatefulSet> = Api::namespaced(client, namespace);
        
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
            AppError::ExternalService(format!("Failed to restart stateful set '{}' in namespace '{}': {}", name, namespace, e))
        })?;
        Ok(())
    }

    pub async fn delete_all_pods_for_stateful_set(
        &self,
        cluster_config: &KubernetesClusterConfig,
        namespace: &str,
        stateful_set_name: &str,
    ) -> Result<(), AppError> {
        let client = Self::get_kube_client(cluster_config).await?;
        let sts_api: Api<StatefulSet> = Api::namespaced(client.clone(), namespace);
        let pod_api: Api<Pod> = Api::namespaced(client, namespace);

        // 1. Get the stateful set to find its label selector
        let sts = sts_api.get(stateful_set_name).await.map_err(|e| {
            AppError::ExternalService(format!(
                "Failed to get stateful set \'{}\' in namespace \'{}\': {}",
                stateful_set_name, namespace, e
            ))
        })?;

        let selector = sts.spec.and_then(|s| Some(s.selector));
        if selector.is_none() {
            return Err(AppError::ExternalService(format!(
                "StatefulSet \'{}\' in namespace \'{}\' does not have a selector",
                stateful_set_name, namespace
            )));
        }

        let label_selector_str = selector.unwrap().match_labels.map(|labels| {
            labels
                .into_iter()
                .map(|(k, v)| format!("{}={}", k, v))
                .collect::<Vec<String>>()
                .join(",")
        }).unwrap_or_default();

        if label_selector_str.is_empty() {
            return Err(AppError::ExternalService(format!(
                "Label selector for stateful set \'{}\' in namespace \'{}\' is empty or could not be constructed.",
                stateful_set_name, namespace
            )));
        }

        // 2. List pods matching the selector
        let lp = ListParams::default().labels(&label_selector_str);
        let pods_to_delete = pod_api.list(&lp).await.map_err(|e| {
            AppError::ExternalService(format!(
                "Failed to list pods for stateful set \'{}\' in namespace \'{}\' using selector \'{}\': {}",
                stateful_set_name, namespace, label_selector_str, e
            ))
        })?;

        // 3. Delete each pod
        for pod in pods_to_delete {
            let pod_name = pod.name_any();
            pod_api
                .delete(&pod_name, &DeleteParams::default())
                .await
                .map_err(|e| {
                    AppError::ExternalService(format!(
                        "Failed to delete pod \'{}\' for stateful set \'{}\' in namespace \'{}\': {}",
                        pod_name, stateful_set_name, namespace, e
                    ))
                })?;
        }

        Ok(())
    }
}

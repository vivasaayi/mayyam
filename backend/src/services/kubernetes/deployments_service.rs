// filepath: /Users/rajanpanneerselvam/work/mayyam/backend/src/services/kubernetes/deployments_service.rs
use chrono::Utc;
use k8s_openapi::api::apps::v1::Deployment;
use k8s_openapi::api::core::v1::Pod;
use kube::api::{DeleteParams, ListParams, Patch, PatchParams};
use kube::{Api, Client, ResourceExt}; // Added ResourceExt
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeMap;

use crate::errors::AppError;
use crate::models::cluster::KubernetesClusterConfig;
// Use the PodInfo and convert_kube_pod_to_pod_info from the pod module
use crate::services::kubernetes::client::ClientFactory;
use crate::services::kubernetes::pod::PodInfo;

#[derive(Debug, Serialize, Deserialize)]
pub struct DeploymentInfo {
    pub name: String,
    pub namespace: String,
    pub replicas: i32,
    pub available_replicas: i32,
    pub updated_replicas: i32,
    pub age: String,
    pub images: Vec<String>,
}

pub struct DeploymentsService;

impl DeploymentsService {
    pub fn new() -> Self {
        DeploymentsService {}
    }

    async fn get_kube_client(cluster_config: &KubernetesClusterConfig) -> Result<Client, AppError> {
        ClientFactory::get_client(cluster_config).await
    }

    pub async fn list_deployments(
        &self,
        cluster_config: &KubernetesClusterConfig,
        namespace: &str,
    ) -> Result<Vec<DeploymentInfo>, AppError> {
        let client = Self::get_kube_client(cluster_config).await?;
        let api: Api<Deployment> = Api::namespaced(client, namespace);
        let lp = ListParams::default();
        let deployment_list = api.list(&lp).await.map_err(|e| {
            AppError::ExternalService(format!(
                "Failed to list deployments in namespace '{}': {}",
                namespace, e
            ))
        })?;

        let mut infos = Vec::new();
        for d in deployment_list {
            let name = d.name_any();
            let replicas = d.spec.as_ref().and_then(|s| s.replicas).unwrap_or(0);
            let available_replicas = d
                .status
                .as_ref()
                .and_then(|s| s.available_replicas)
                .unwrap_or(0);
            let updated_replicas = d
                .status
                .as_ref()
                .and_then(|s| s.updated_replicas)
                .unwrap_or(0);

            let age = d.metadata.creation_timestamp.as_ref().map_or_else(
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

            let images = d
                .spec
                .as_ref()
                .and_then(|s| s.template.spec.as_ref())
                .map(|pod_spec| {
                    pod_spec
                        .containers
                        .iter()
                        .filter_map(|c| c.image.clone())
                        .collect::<Vec<String>>()
                })
                .unwrap_or_default();

            infos.push(DeploymentInfo {
                name,
                namespace: namespace.to_string(),
                replicas,
                available_replicas,
                updated_replicas,
                age,
                images,
            });
        }
        Ok(infos)
    }

    pub async fn get_deployment_details(
        &self,
        cluster_config: &KubernetesClusterConfig,
        namespace: &str,
        name: &str,
    ) -> Result<Deployment, AppError> {
        let client = Self::get_kube_client(cluster_config).await?;
        let api: Api<Deployment> = Api::namespaced(client, namespace);
        api.get(name).await.map_err(|e| {
            AppError::ExternalService(format!(
                "Failed to get deployment '{}' in namespace '{}': {}",
                name, namespace, e
            ))
        })
    }

    pub async fn delete_deployment(
        &self,
        cluster_config: &KubernetesClusterConfig,
        namespace: &str,
        name: &str,
    ) -> Result<(), AppError> {
        let client = Self::get_kube_client(cluster_config).await?;
        let api: Api<Deployment> = Api::namespaced(client, namespace);
        api.delete(name, &DeleteParams::default())
            .await
            .map_err(|e| {
                AppError::ExternalService(format!(
                    "Failed to delete deployment '{}' in namespace '{}': {}",
                    name, namespace, e
                ))
            })?;
        Ok(())
    }

    pub async fn scale_deployment(
        &self,
        cluster_config: &KubernetesClusterConfig,
        namespace: &str,
        name: &str,
        replicas: i32,
    ) -> Result<(), AppError> {
        let client = Self::get_kube_client(cluster_config).await?;
        let api: Api<Deployment> = Api::namespaced(client, namespace);
        let patch = json!({
            "spec": { "replicas": replicas }
        });
        api.patch_scale(name, &PatchParams::default(), &Patch::Merge(&patch))
            .await
            .map_err(|e| {
                AppError::ExternalService(format!(
                    "Failed to scale deployment '{}' in namespace '{}': {}",
                    name, namespace, e
                ))
            })?;
        Ok(())
    }

    pub async fn restart_deployment(
        &self,
        cluster_config: &KubernetesClusterConfig,
        namespace: &str,
        name: &str,
    ) -> Result<(), AppError> {
        let client = Self::get_kube_client(cluster_config).await?;
        let api: Api<Deployment> = Api::namespaced(client, namespace);

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

        api.patch(name, &PatchParams::default(), &Patch::Merge(&patch))
            .await
            .map_err(|e| {
                AppError::ExternalService(format!(
                    "Failed to restart deployment '{}' in namespace '{}': {}",
                    name, namespace, e
                ))
            })?;
        Ok(())
    }

    pub async fn get_pods_for_deployment(
        &self,
        cluster_config: &KubernetesClusterConfig,
        namespace: &str,
        deployment_name: &str,
    ) -> Result<Vec<PodInfo>, AppError> {
        let client = Self::get_kube_client(cluster_config).await?;
        let deployment_api: Api<Deployment> = Api::namespaced(client.clone(), namespace);
        let pod_api: Api<Pod> = Api::namespaced(client, namespace);

        let deployment = deployment_api.get(deployment_name).await.map_err(|e| {
            AppError::ExternalService(format!(
                "Failed to get deployment '{}' in namespace '{}': {}",
                deployment_name, namespace, e
            ))
        })?;

        let label_selector_str = deployment
            .spec
            .as_ref()
            .and_then(|spec| spec.selector.match_labels.as_ref()) // Ensure selector and match_labels exist
            .map(|labels_map| {
                labels_map
                    .iter()
                    .map(|(k, v)| format!("{}={}", k, v))
                    .collect::<Vec<String>>()
                    .join(",")
            })
            .unwrap_or_default(); // If no labels, selector is empty string

        if label_selector_str.is_empty() {
            // If the selector string is empty (e.g. deployment has no selector labels),
            // then no pods will match. Return an empty list.
            return Ok(vec![]);
        }

        let lp = ListParams::default().labels(&label_selector_str);
        let pod_list = pod_api.list(&lp).await.map_err(|e| {
            AppError::ExternalService(format!(
                "Failed to list pods for deployment '{}' in namespace '{}' using selector '{}': {}",
                deployment_name, namespace, label_selector_str, e
            ))
        })?;

        // Convert each Pod to PodInfo
        let mut pod_infos = Vec::new();
        for pod in pod_list {
            let info =
                crate::services::kubernetes::pod::convert_kube_pod_to_pod_info(&pod, namespace);
            pod_infos.push(info);
        }

        Ok(pod_infos)
    }

    pub async fn delete_all_pods_for_deployment(
        &self,
        cluster_config: &KubernetesClusterConfig,
        namespace: &str,
        deployment_name: &str,
    ) -> Result<(), AppError> {
        let client = Self::get_kube_client(cluster_config).await?;
        let deployment_api: Api<Deployment> = Api::namespaced(client.clone(), namespace);
        let pod_api: Api<Pod> = Api::namespaced(client, namespace);

        // 1. Get the deployment to find its label selector
        let deployment = deployment_api.get(deployment_name).await.map_err(|e| {
            AppError::ExternalService(format!(
                "Failed to get deployment \'{}\' in namespace \'{}\': {}",
                deployment_name, namespace, e
            ))
        })?;

        let selector = deployment.spec.and_then(|s| Some(s.selector));
        if selector.is_none() {
            return Err(AppError::ExternalService(format!(
                "Deployment \'{}\' in namespace \'{}\' does not have a selector",
                deployment_name, namespace
            )));
        }
        let label_selector_str = selector
            .unwrap()
            .match_labels
            .map(|labels| {
                labels
                    .into_iter()
                    .map(|(k, v)| format!("{}={}", k, v))
                    .collect::<Vec<String>>()
                    .join(",")
            })
            .unwrap_or_default();

        if label_selector_str.is_empty() {
            return Err(AppError::ExternalService(format!(
                "Label selector for deployment \'{}\' in namespace \'{}\' is empty or could not be constructed.",
                deployment_name, namespace
            )));
        }

        // 2. List pods matching the selector
        let lp = ListParams::default().labels(&label_selector_str);
        let pods_to_delete = pod_api.list(&lp).await.map_err(|e| {
            AppError::ExternalService(format!(
                "Failed to list pods for deployment \'{}\' in namespace \'{}\' using selector \'{}\': {}",
                deployment_name, namespace, label_selector_str, e
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
                        "Failed to delete pod \'{}\' for deployment \'{}\' in namespace \'{}\': {}",
                        pod_name, deployment_name, namespace, e
                    ))
                })?;
            // Consider adding a small delay or logging here if needed
        }

        Ok(())
    }
}

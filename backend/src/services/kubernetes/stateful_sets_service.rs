// filepath: /Users/rajanpanneerselvam/work/mayyam/backend/src/services/kubernetes/stateful_sets_service.rs
use chrono::Utc;
use k8s_openapi::api::apps::v1::StatefulSet;
use k8s_openapi::api::core::v1::Pod;
use k8s_openapi::apimachinery::pkg::apis::meta::v1::LabelSelector;
use kube::api::{DeleteParams, ListParams, Patch, PatchParams};
use kube::config::{Config as KubeConfig, KubeConfigOptions, Kubeconfig};
use kube::{Api, Client, ResourceExt};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeMap;

use crate::errors::AppError;
use crate::models::cluster::KubernetesClusterConfig;

#[derive(Debug, Serialize, Deserialize)]
pub struct StatefulSetInfo {
    pub name: String,
    pub namespace: String,
    pub desired_replicas: i32,
    pub ready_replicas: i32,
    pub current_replicas: i32,
    pub updated_replicas: i32,
    pub age: String,
    pub images: Vec<String>,
}

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
                _ => {} // Unknown operator
            }
        }
    }
    if parts.is_empty() {
        None
    } else {
        Some(parts.join(","))
    }
}

pub struct StatefulSetsService;

impl StatefulSetsService {
    pub fn new() -> Self {
        StatefulSetsService {}
    }

    async fn get_kube_client(cluster_config: &KubernetesClusterConfig) -> Result<Client, AppError> {
        let kubeconfig = if let Some(path) = &cluster_config.kube_config_path {
            Kubeconfig::read_from(path).map_err(|e| {
                AppError::ExternalService(format!("Failed to read kubeconfig from path: {}", e))
            })?
        } else {
            // Fallback to in-cluster or default context if path is not provided
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
                cluster: None, // Use context's cluster
                user: None,    // Use context's user
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

    pub async fn list_stateful_sets(
        &self,
        cluster_config: &KubernetesClusterConfig,
        namespace: &str,
    ) -> Result<Vec<StatefulSetInfo>, AppError> {
        let client = Self::get_kube_client(cluster_config).await?;
        let api: Api<StatefulSet> = Api::namespaced(client, namespace);
        let lp = ListParams::default();
        let sts_list = api.list(&lp).await.map_err(|e| {
            AppError::ExternalService(format!(
                "Failed to list stateful sets in namespace '{}': {}",
                namespace, e
            ))
        })?;

        let mut infos = Vec::new();
        for sts in sts_list {
            let name = sts.name_any();
            let spec = sts.spec.as_ref();
            let status = sts.status.as_ref();

            let age = sts.metadata.creation_timestamp.as_ref().map_or_else(
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

            let images = spec
                .and_then(|s| s.template.spec.as_ref())
                .map(|pod_spec| {
                    pod_spec
                        .containers
                        .iter()
                        .filter_map(|c| c.image.clone())
                        .collect::<Vec<String>>()
                })
                .unwrap_or_default();

            infos.push(StatefulSetInfo {
                name,
                namespace: namespace.to_string(),
                desired_replicas: spec.and_then(|s| s.replicas).unwrap_or(0),
                ready_replicas: status.and_then(|s| s.ready_replicas).unwrap_or(0),
                current_replicas: status.and_then(|s| s.current_replicas).unwrap_or(0),
                updated_replicas: status.and_then(|s| s.updated_replicas).unwrap_or(0),
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
            AppError::ExternalService(format!(
                "Failed to get stateful set '{}' in namespace '{}': {}",
                name, namespace, e
            ))
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
        api.delete(name, &DeleteParams::default())
            .await
            .map_err(|e| {
                AppError::ExternalService(format!(
                    "Failed to delete stateful set '{}' in namespace '{}': {}",
                    name, namespace, e
                ))
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
        api.patch_scale(name, &PatchParams::default(), &Patch::Merge(&patch))
            .await
            .map_err(|e| {
                AppError::ExternalService(format!(
                    "Failed to scale stateful set '{}' in namespace '{}': {}",
                    name, namespace, e
                ))
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

        api.patch(name, &PatchParams::default(), &Patch::Merge(&patch))
            .await
            .map_err(|e| {
                AppError::ExternalService(format!(
                    "Failed to restart stateful set '{}' in namespace '{}': {}",
                    name, namespace, e
                ))
            })?;
        Ok(())
    }

    pub async fn get_pods_for_stateful_set(
        &self,
        cluster_config: &KubernetesClusterConfig,
        namespace: &str,
        stateful_set_name: &str,
    ) -> Result<Vec<Pod>, AppError> {
        let client = Self::get_kube_client(cluster_config).await?;
        let sts_api: Api<StatefulSet> = Api::namespaced(client.clone(), namespace);
        let pod_api: Api<Pod> = Api::namespaced(client, namespace);

        let sts = sts_api.get(stateful_set_name).await.map_err(|e| {
            AppError::ExternalService(format!(
                "Failed to get stateful set '{}' in namespace '{}': {}",
                stateful_set_name, namespace, e
            ))
        })?;

        let selector_opt = sts.spec.map(|s| s.selector); // Corrected line
        if selector_opt.is_none() {
            return Err(AppError::ExternalService(format!(
                "StatefulSet '{}' in namespace '{}' does not have spec or selector",
                stateful_set_name, namespace
            )));
        }

        let label_selector_str =
            label_selector_to_string(&selector_opt.unwrap()).unwrap_or_default();

        if label_selector_str.is_empty() {
            return Err(AppError::ExternalService(format!(
                "Label selector for stateful set '{}' in namespace '{}' is empty or could not be constructed.",
                stateful_set_name, namespace
            )));
        }

        let lp = ListParams::default().labels(&label_selector_str);
        let pods = pod_api.list(&lp).await.map_err(|e| {
            AppError::ExternalService(format!(
                "Failed to list pods for stateful set '{}' in namespace '{}' using selector '{}': {}",
                stateful_set_name, namespace, label_selector_str, e
            ))
        })?;
        Ok(pods.items)
    }
}

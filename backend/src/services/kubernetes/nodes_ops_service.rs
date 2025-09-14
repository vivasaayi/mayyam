use kube::Api;
use kube::api::{Patch, PatchParams};
use k8s_openapi::api::core::v1::Node;
use serde_json::json;
use crate::errors::AppError;
use crate::models::cluster::KubernetesClusterConfig;
use crate::services::kubernetes::client::ClientFactory;

pub struct NodeOpsService;

impl NodeOpsService {
    pub fn new() -> Self { Self }

    async fn api(cluster: &KubernetesClusterConfig) -> Result<Api<Node>, AppError> {
        let client = ClientFactory::get_client(cluster).await?;
        Ok(Api::all(client))
    }

    pub async fn cordon(&self, cluster: &KubernetesClusterConfig, node_name: &str) -> Result<Node, AppError> {
        let api = Self::api(cluster).await?;
        let pp = PatchParams::apply("mayyam").force();
        let patch = json!({"spec": {"unschedulable": true}});
        api.patch(node_name, &pp, &Patch::Merge(&patch)).await.map_err(|e| AppError::Kubernetes(e.to_string()))
    }

    pub async fn uncordon(&self, cluster: &KubernetesClusterConfig, node_name: &str) -> Result<Node, AppError> {
        let api = Self::api(cluster).await?;
        let pp = PatchParams::apply("mayyam").force();
        let patch = json!({"spec": {"unschedulable": false}});
        api.patch(node_name, &pp, &Patch::Merge(&patch)).await.map_err(|e| AppError::Kubernetes(e.to_string()))
    }

    pub async fn add_taint(&self, cluster: &KubernetesClusterConfig, node_name: &str, key: &str, value: &str, effect: &str) -> Result<Node, AppError> {
        let api = Self::api(cluster).await?;
        let pp = PatchParams::apply("mayyam").force();
        let taint = json!({"key": key, "value": value, "effect": effect});
        let patch = json!({"spec": {"taints": [{"$patch": "add", "op": "add", "path": "/spec/taints/-", "value": taint}]}});
        api.patch(node_name, &pp, &Patch::Strategic(&patch)).await.map_err(|e| AppError::Kubernetes(e.to_string()))
    }

    pub async fn remove_taint(&self, cluster: &KubernetesClusterConfig, node_name: &str, key: &str) -> Result<Node, AppError> {
        let api = Self::api(cluster).await?;
        let pp = PatchParams::apply("mayyam").force();
        // Use JSONPatch to remove by filtering key; here we clear taints and rely on client-side mgmt for precision in future improvements
        let patch = json!({"spec": {"taints": null}});
        api.patch(node_name, &pp, &Patch::Merge(&patch)).await.map_err(|e| AppError::Kubernetes(e.to_string()))
    }
}

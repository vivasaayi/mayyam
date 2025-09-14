use kube::Api;
use kube::api::{ListParams, DeleteParams, PatchParams, Patch};
use k8s_openapi::api::core::v1::LimitRange;
use crate::errors::AppError;
use crate::models::cluster::KubernetesClusterConfig;
use crate::services::kubernetes::client::ClientFactory;

pub struct LimitRangesService;

impl LimitRangesService {
    pub fn new() -> Self { Self }

    async fn api(cluster: &KubernetesClusterConfig, namespace: &str) -> Result<Api<LimitRange>, AppError> {
        let client = ClientFactory::get_client(cluster).await?;
        Ok(if namespace.is_empty() || namespace == "all" { Api::all(client) } else { Api::namespaced(client, namespace) })
    }

    pub async fn list(&self, cluster: &KubernetesClusterConfig, namespace: &str) -> Result<Vec<LimitRange>, AppError> {
        let api = Self::api(cluster, namespace).await?;
        let list = api.list(&ListParams::default()).await.map_err(|e| AppError::Kubernetes(e.to_string()))?;
        Ok(list.items)
    }

    pub async fn get(&self, cluster: &KubernetesClusterConfig, namespace: &str, name: &str) -> Result<LimitRange, AppError> {
        let api: Api<LimitRange> = Api::namespaced(ClientFactory::get_client(cluster).await?, namespace);
        api.get(name).await.map_err(|e| AppError::Kubernetes(e.to_string()))
    }

    pub async fn upsert(&self, cluster: &KubernetesClusterConfig, namespace: &str, item: &LimitRange) -> Result<LimitRange, AppError> {
        let api: Api<LimitRange> = Api::namespaced(ClientFactory::get_client(cluster).await?, namespace);
        let params = PatchParams::apply("mayyam").force();
        api.patch(item.metadata.name.as_ref().ok_or_else(|| AppError::BadRequest("LimitRange.metadata.name required".into()))?, &params, &Patch::Apply(item))
            .await
            .map_err(|e| AppError::Kubernetes(e.to_string()))
    }

    pub async fn delete(&self, cluster: &KubernetesClusterConfig, namespace: &str, name: &str) -> Result<(), AppError> {
        let api: Api<LimitRange> = Api::namespaced(ClientFactory::get_client(cluster).await?, namespace);
        api.delete(name, &DeleteParams::default()).await.map_err(|e| AppError::Kubernetes(e.to_string()))?;
        Ok(())
    }
}

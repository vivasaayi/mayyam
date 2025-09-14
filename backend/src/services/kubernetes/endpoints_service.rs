use kube::Api;
use kube::api::{ListParams, DeleteParams, PatchParams, Patch};
use k8s_openapi::api::core::v1::Endpoints;
use k8s_openapi::api::discovery::v1::EndpointSlice;
use crate::errors::AppError;
use crate::models::cluster::KubernetesClusterConfig;
use crate::services::kubernetes::client::ClientFactory;

pub struct EndpointsService;

impl EndpointsService {
    pub fn new() -> Self { Self }

    async fn endpoints_api(cluster: &KubernetesClusterConfig, namespace: &str) -> Result<Api<Endpoints>, AppError> {
        let client = ClientFactory::get_client(cluster).await?;
        Ok(if namespace.is_empty() || namespace == "all" { Api::all(client) } else { Api::namespaced(client, namespace) })
    }

    async fn endpoint_slice_api(cluster: &KubernetesClusterConfig, namespace: &str) -> Result<Api<EndpointSlice>, AppError> {
        let client = ClientFactory::get_client(cluster).await?;
        Ok(if namespace.is_empty() || namespace == "all" { Api::all(client) } else { Api::namespaced(client, namespace) })
    }

    pub async fn list_endpoints(&self, cluster: &KubernetesClusterConfig, namespace: &str) -> Result<Vec<Endpoints>, AppError> {
        let api = Self::endpoints_api(cluster, namespace).await?;
        let list = api.list(&ListParams::default()).await.map_err(|e| AppError::Kubernetes(e.to_string()))?;
        Ok(list.items)
    }

    pub async fn list_endpoint_slices(&self, cluster: &KubernetesClusterConfig, namespace: &str) -> Result<Vec<EndpointSlice>, AppError> {
        let api = Self::endpoint_slice_api(cluster, namespace).await?;
        let list = api.list(&ListParams::default()).await.map_err(|e| AppError::Kubernetes(e.to_string()))?;
        Ok(list.items)
    }

    pub async fn get_endpoints(&self, cluster: &KubernetesClusterConfig, namespace: &str, name: &str) -> Result<Endpoints, AppError> {
        let api: Api<Endpoints> = Api::namespaced(ClientFactory::get_client(cluster).await?, namespace);
        api.get(name).await.map_err(|e| AppError::Kubernetes(e.to_string()))
    }

    pub async fn upsert_endpoints(&self, cluster: &KubernetesClusterConfig, namespace: &str, item: &Endpoints) -> Result<Endpoints, AppError> {
        let api: Api<Endpoints> = Api::namespaced(ClientFactory::get_client(cluster).await?, namespace);
        let params = PatchParams::apply("mayyam").force();
        api.patch(item.metadata.name.as_ref().ok_or_else(|| AppError::BadRequest("Endpoints.metadata.name required".into()))?, &params, &Patch::Apply(item))
            .await
            .map_err(|e| AppError::Kubernetes(e.to_string()))
    }

    pub async fn delete_endpoints(&self, cluster: &KubernetesClusterConfig, namespace: &str, name: &str) -> Result<(), AppError> {
        let api: Api<Endpoints> = Api::namespaced(ClientFactory::get_client(cluster).await?, namespace);
        api.delete(name, &DeleteParams::default()).await.map_err(|e| AppError::Kubernetes(e.to_string()))?;
        Ok(())
    }
}

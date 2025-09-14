use kube::{Api, Client};
use kube::api::{ListParams, DeleteParams, PatchParams, Patch, PostParams};
use k8s_openapi::api::batch::v1::Job;
use crate::errors::AppError;
use crate::models::cluster::KubernetesClusterConfig;
use crate::services::kubernetes::client::ClientFactory;

pub struct JobsService;

impl JobsService {
    pub fn new() -> Self { Self }

    async fn api(cluster: &KubernetesClusterConfig, namespace: &str) -> Result<Api<Job>, AppError> {
        let client = ClientFactory::get_client(cluster).await?;
        Ok(if namespace.is_empty() || namespace == "all" { Api::all(client) } else { Api::namespaced(client, namespace) })
    }

    pub async fn list(&self, cluster: &KubernetesClusterConfig, namespace: &str) -> Result<Vec<Job>, AppError> {
        let api = Self::api(cluster, namespace).await?;
        let lp = ListParams::default();
        let list = api.list(&lp).await.map_err(|e| AppError::Kubernetes(e.to_string()))?;
        Ok(list.items)
    }

    pub async fn get(&self, cluster: &KubernetesClusterConfig, namespace: &str, name: &str) -> Result<Job, AppError> {
        let api: Api<Job> = Api::namespaced(ClientFactory::get_client(cluster).await?, namespace);
        api.get(name).await.map_err(|e| AppError::Kubernetes(e.to_string()))
    }

    pub async fn upsert(&self, cluster: &KubernetesClusterConfig, namespace: &str, job: &Job) -> Result<Job, AppError> {
        let api: Api<Job> = Api::namespaced(ClientFactory::get_client(cluster).await?, namespace);
        let params = PatchParams::apply("mayyam").force();
        api.patch(job.metadata.name.as_ref().ok_or_else(|| AppError::BadRequest("Job.metadata.name required".into()))?, &params, &Patch::Apply(job))
            .await
            .map_err(|e| AppError::Kubernetes(e.to_string()))
    }

    pub async fn delete(&self, cluster: &KubernetesClusterConfig, namespace: &str, name: &str) -> Result<(), AppError> {
        let api: Api<Job> = Api::namespaced(ClientFactory::get_client(cluster).await?, namespace);
        api.delete(name, &DeleteParams::default()).await.map_err(|e| AppError::Kubernetes(e.to_string()))?;
        Ok(())
    }
}

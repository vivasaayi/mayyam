use kube::Api;
use kube::api::{PostParams};
use k8s_openapi::api::authorization::v1::{SelfSubjectAccessReview, SelfSubjectAccessReviewSpec, ResourceAttributes, SubjectAccessReviewStatus};
use crate::errors::AppError;
use crate::models::cluster::KubernetesClusterConfig;
use crate::services::kubernetes::client::ClientFactory;

pub struct AuthorizationService;

impl AuthorizationService {
    pub fn new() -> Self { Self }

    fn build_ssar(namespace: Option<String>, verb: String, group: Option<String>, resource: String, subresource: Option<String>, name: Option<String>) -> SelfSubjectAccessReview {
        let attrs = ResourceAttributes {
            namespace,
            verb: Some(verb),
            group,
            resource: Some(resource),
            subresource,
            name,
            ..Default::default()
        };
        SelfSubjectAccessReview {
            spec: SelfSubjectAccessReviewSpec { resource_attributes: Some(attrs), ..Default::default() },
            ..Default::default()
        }
    }

    pub async fn can(&self, cluster: &KubernetesClusterConfig, namespace: Option<String>, verb: &str, group: Option<&str>, resource: &str, subresource: Option<&str>, name: Option<&str>) -> Result<SubjectAccessReviewStatus, AppError> {
        let client = ClientFactory::get_client(cluster).await?;
        let api: Api<SelfSubjectAccessReview> = Api::all(client);
        let body = Self::build_ssar(namespace, verb.to_string(), group.map(|s| s.to_string()), resource.to_string(), subresource.map(|s| s.to_string()), name.map(|s| s.to_string()));
    let res = api.create(&PostParams::default(), &body).await.map_err(|e| AppError::Kubernetes(e.to_string()))?;
    Ok(res.status.unwrap_or_default())
    }
}

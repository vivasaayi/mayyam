// Copyright (c) 2025 Rajan Panneer Selvam
//
// Licensed under the Business Source License 1.1 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     https://www.mariadb.com/bsl11
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.


use crate::errors::AppError;
use crate::models::cluster::KubernetesClusterConfig;
use crate::services::kubernetes::client::ClientFactory;
use k8s_openapi::api::authorization::v1::{
    ResourceAttributes, SelfSubjectAccessReview, SelfSubjectAccessReviewSpec,
    SubjectAccessReviewStatus,
};
use kube::api::PostParams;
use kube::Api;

pub struct AuthorizationService;

impl AuthorizationService {
    pub fn new() -> Self {
        Self
    }

    fn build_ssar(
        namespace: Option<String>,
        verb: String,
        group: Option<String>,
        resource: String,
        subresource: Option<String>,
        name: Option<String>,
    ) -> SelfSubjectAccessReview {
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
            spec: SelfSubjectAccessReviewSpec {
                resource_attributes: Some(attrs),
                ..Default::default()
            },
            ..Default::default()
        }
    }

    pub async fn can(
        &self,
        cluster: &KubernetesClusterConfig,
        namespace: Option<String>,
        verb: &str,
        group: Option<&str>,
        resource: &str,
        subresource: Option<&str>,
        name: Option<&str>,
    ) -> Result<SubjectAccessReviewStatus, AppError> {
        let client = ClientFactory::get_client(cluster).await?;
        let api: Api<SelfSubjectAccessReview> = Api::all(client);
        let body = Self::build_ssar(
            namespace,
            verb.to_string(),
            group.map(|s| s.to_string()),
            resource.to_string(),
            subresource.map(|s| s.to_string()),
            name.map(|s| s.to_string()),
        );
        let res = api
            .create(&PostParams::default(), &body)
            .await
            .map_err(|e| AppError::Kubernetes(e.to_string()))?;
        Ok(res.status.unwrap_or_default())
    }
}

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
use crate::services::kubernetes::job_inventory::{
    JobConditionInventoryItem, JobContainerInventoryItem, JobInventoryItem,
};
use chrono::Utc;
use k8s_openapi::api::batch::v1::Job;
use kube::api::{DeleteParams, ListParams, Patch, PatchParams};
use kube::{Api, ResourceExt};

pub struct JobsService;

fn convert_kube_job_to_job_inventory(
    job: &Job,
    cluster_id: &str,
    current_namespace: &str,
    collected_at: chrono::DateTime<Utc>,
) -> JobInventoryItem {
    let namespace = job
        .namespace()
        .unwrap_or_else(|| current_namespace.to_string());
    let spec = job.spec.as_ref();
    let status = job.status.as_ref();
    let pod_spec = spec.and_then(|spec| spec.template.spec.as_ref());
    let containers = pod_spec
        .map(|pod_spec| {
            pod_spec
                .containers
                .iter()
                .map(|container| JobContainerInventoryItem {
                    name: container.name.clone(),
                    image: container.image.clone(),
                    privileged: container
                        .security_context
                        .as_ref()
                        .and_then(|security_context| security_context.privileged),
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let conditions = status
        .and_then(|status| status.conditions.as_ref())
        .map(|conditions| {
            conditions
                .iter()
                .map(|condition| JobConditionInventoryItem {
                    type_: condition.type_.clone(),
                    status: condition.status.clone(),
                    reason: condition.reason.clone(),
                    message: condition.message.clone(),
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    JobInventoryItem {
        cluster_id: cluster_id.to_string(),
        namespace,
        name: job.name_any(),
        completions: spec.and_then(|spec| spec.completions),
        parallelism: spec.and_then(|spec| spec.parallelism),
        active: status.and_then(|status| status.active).unwrap_or(0),
        ready: status.and_then(|status| status.ready).unwrap_or(0),
        succeeded: status.and_then(|status| status.succeeded).unwrap_or(0),
        failed: status.and_then(|status| status.failed).unwrap_or(0),
        backoff_limit: spec.and_then(|spec| spec.backoff_limit),
        active_deadline_seconds: spec.and_then(|spec| spec.active_deadline_seconds),
        ttl_seconds_after_finished: spec.and_then(|spec| spec.ttl_seconds_after_finished),
        completion_mode: spec.and_then(|spec| spec.completion_mode.clone()),
        suspend: spec.and_then(|spec| spec.suspend).unwrap_or(false),
        labels: job.metadata.labels.clone().unwrap_or_default(),
        annotations: job.metadata.annotations.clone().unwrap_or_default(),
        selector: spec
            .and_then(|spec| {
                spec.selector
                    .as_ref()
                    .and_then(|selector| selector.match_labels.clone())
            })
            .unwrap_or_default(),
        pod_template_labels: spec
            .and_then(|spec| {
                spec.template
                    .metadata
                    .as_ref()
                    .and_then(|metadata| metadata.labels.clone())
            })
            .unwrap_or_default(),
        containers,
        conditions,
        service_account_name: pod_spec.and_then(|pod_spec| pod_spec.service_account_name.clone()),
        restart_policy: pod_spec.and_then(|pod_spec| pod_spec.restart_policy.clone()),
        host_network: pod_spec
            .and_then(|pod_spec| pod_spec.host_network)
            .unwrap_or(false),
        start_time: status
            .and_then(|status| status.start_time.as_ref())
            .map(|timestamp| timestamp.0),
        completion_time: status
            .and_then(|status| status.completion_time.as_ref())
            .map(|timestamp| timestamp.0),
        created_at: job
            .metadata
            .creation_timestamp
            .as_ref()
            .map(|timestamp| timestamp.0),
        collected_at,
    }
}

impl JobsService {
    pub fn new() -> Self {
        Self
    }

    async fn api(cluster: &KubernetesClusterConfig, namespace: &str) -> Result<Api<Job>, AppError> {
        let client = ClientFactory::get_client(cluster).await?;
        Ok(if namespace.is_empty() || namespace == "all" {
            Api::all(client)
        } else {
            Api::namespaced(client, namespace)
        })
    }

    pub async fn list(
        &self,
        cluster: &KubernetesClusterConfig,
        namespace: &str,
    ) -> Result<Vec<Job>, AppError> {
        let api = Self::api(cluster, namespace).await?;
        let lp = ListParams::default();
        let list = api
            .list(&lp)
            .await
            .map_err(|e| AppError::Kubernetes(e.to_string()))?;
        Ok(list.items)
    }

    pub async fn list_job_inventory(
        &self,
        cluster: &KubernetesClusterConfig,
        cluster_id: &str,
        namespace: Option<&str>,
    ) -> Result<Vec<JobInventoryItem>, AppError> {
        let namespace = namespace
            .map(str::trim)
            .filter(|namespace| !namespace.is_empty());
        let namespace_arg = namespace.unwrap_or("");
        let api = Self::api(cluster, namespace_arg).await?;
        let collected_at = Utc::now();
        let list = api
            .list(&ListParams::default())
            .await
            .map_err(|e| AppError::Kubernetes(e.to_string()))?;
        let fallback_namespace = namespace
            .filter(|namespace| *namespace != "all")
            .unwrap_or("");

        Ok(list
            .items
            .iter()
            .map(|job| {
                convert_kube_job_to_job_inventory(job, cluster_id, fallback_namespace, collected_at)
            })
            .collect())
    }

    pub async fn get(
        &self,
        cluster: &KubernetesClusterConfig,
        namespace: &str,
        name: &str,
    ) -> Result<Job, AppError> {
        let api: Api<Job> = Api::namespaced(ClientFactory::get_client(cluster).await?, namespace);
        api.get(name)
            .await
            .map_err(|e| AppError::Kubernetes(e.to_string()))
    }

    pub async fn upsert(
        &self,
        cluster: &KubernetesClusterConfig,
        namespace: &str,
        job: &Job,
    ) -> Result<Job, AppError> {
        let api: Api<Job> = Api::namespaced(ClientFactory::get_client(cluster).await?, namespace);
        let params = PatchParams::apply("mayyam").force();
        api.patch(
            job.metadata
                .name
                .as_ref()
                .ok_or_else(|| AppError::BadRequest("Job.metadata.name required".into()))?,
            &params,
            &Patch::Apply(job),
        )
        .await
        .map_err(|e| AppError::Kubernetes(e.to_string()))
    }

    pub async fn delete(
        &self,
        cluster: &KubernetesClusterConfig,
        namespace: &str,
        name: &str,
    ) -> Result<(), AppError> {
        let api: Api<Job> = Api::namespaced(ClientFactory::get_client(cluster).await?, namespace);
        api.delete(name, &DeleteParams::default())
            .await
            .map_err(|e| AppError::Kubernetes(e.to_string()))?;
        Ok(())
    }
}

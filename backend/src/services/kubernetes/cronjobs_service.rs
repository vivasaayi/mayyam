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
use crate::services::kubernetes::cronjob_inventory::{
    CronJobActiveJobInventoryItem, CronJobContainerInventoryItem, CronJobInventoryItem,
};
use chrono::Utc;
use k8s_openapi::api::batch::v1::CronJob;
use kube::api::{DeleteParams, ListParams, Patch, PatchParams};
use kube::{Api, ResourceExt};

pub struct CronJobsService;

fn convert_kube_cronjob_to_cronjob_inventory(
    cronjob: &CronJob,
    cluster_id: &str,
    current_namespace: &str,
    collected_at: chrono::DateTime<Utc>,
) -> CronJobInventoryItem {
    let namespace = cronjob
        .namespace()
        .unwrap_or_else(|| current_namespace.to_string());
    let spec = cronjob.spec.as_ref();
    let status = cronjob.status.as_ref();
    let job_template_metadata = spec.and_then(|spec| spec.job_template.metadata.as_ref());
    let job_spec = spec.and_then(|spec| spec.job_template.spec.as_ref());
    let pod_template_metadata = job_spec.and_then(|job_spec| job_spec.template.metadata.as_ref());
    let pod_spec = job_spec.and_then(|job_spec| job_spec.template.spec.as_ref());
    let containers = pod_spec
        .map(|pod_spec| {
            pod_spec
                .containers
                .iter()
                .map(|container| CronJobContainerInventoryItem {
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
    let active_jobs = status
        .and_then(|status| status.active.as_ref())
        .map(|active_jobs| {
            active_jobs
                .iter()
                .map(|active_job| CronJobActiveJobInventoryItem {
                    name: active_job.name.clone(),
                    namespace: active_job.namespace.clone(),
                    uid: active_job.uid.clone(),
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    CronJobInventoryItem {
        cluster_id: cluster_id.to_string(),
        namespace,
        name: cronjob.name_any(),
        schedule: spec.map(|spec| spec.schedule.clone()).unwrap_or_default(),
        time_zone: spec.and_then(|spec| spec.time_zone.clone()),
        concurrency_policy: spec.and_then(|spec| spec.concurrency_policy.clone()),
        suspend: spec.and_then(|spec| spec.suspend).unwrap_or(false),
        starting_deadline_seconds: spec.and_then(|spec| spec.starting_deadline_seconds),
        successful_jobs_history_limit: spec.and_then(|spec| spec.successful_jobs_history_limit),
        failed_jobs_history_limit: spec.and_then(|spec| spec.failed_jobs_history_limit),
        active_job_count: active_jobs.len(),
        active_jobs,
        last_schedule_time: status
            .and_then(|status| status.last_schedule_time.as_ref())
            .map(|timestamp| timestamp.0),
        last_successful_time: status
            .and_then(|status| status.last_successful_time.as_ref())
            .map(|timestamp| timestamp.0),
        job_completions: job_spec.and_then(|job_spec| job_spec.completions),
        job_parallelism: job_spec.and_then(|job_spec| job_spec.parallelism),
        job_backoff_limit: job_spec.and_then(|job_spec| job_spec.backoff_limit),
        job_active_deadline_seconds: job_spec.and_then(|job_spec| job_spec.active_deadline_seconds),
        labels: cronjob.metadata.labels.clone().unwrap_or_default(),
        annotations: cronjob.metadata.annotations.clone().unwrap_or_default(),
        job_template_labels: job_template_metadata
            .and_then(|metadata| metadata.labels.clone())
            .unwrap_or_default(),
        job_template_annotations: job_template_metadata
            .and_then(|metadata| metadata.annotations.clone())
            .unwrap_or_default(),
        pod_template_labels: pod_template_metadata
            .and_then(|metadata| metadata.labels.clone())
            .unwrap_or_default(),
        containers,
        service_account_name: pod_spec.and_then(|pod_spec| pod_spec.service_account_name.clone()),
        restart_policy: pod_spec.and_then(|pod_spec| pod_spec.restart_policy.clone()),
        host_network: pod_spec
            .and_then(|pod_spec| pod_spec.host_network)
            .unwrap_or(false),
        created_at: cronjob
            .metadata
            .creation_timestamp
            .as_ref()
            .map(|timestamp| timestamp.0),
        collected_at,
    }
}

impl CronJobsService {
    pub fn new() -> Self {
        Self
    }

    async fn api(
        cluster: &KubernetesClusterConfig,
        namespace: &str,
    ) -> Result<Api<CronJob>, AppError> {
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
    ) -> Result<Vec<CronJob>, AppError> {
        let api = Self::api(cluster, namespace).await?;
        let lp = ListParams::default();
        let list = api
            .list(&lp)
            .await
            .map_err(|e| AppError::Kubernetes(e.to_string()))?;
        Ok(list.items)
    }

    pub async fn list_cronjob_inventory(
        &self,
        cluster: &KubernetesClusterConfig,
        cluster_id: &str,
        namespace: Option<&str>,
    ) -> Result<Vec<CronJobInventoryItem>, AppError> {
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
            .map(|cronjob| {
                convert_kube_cronjob_to_cronjob_inventory(
                    cronjob,
                    cluster_id,
                    fallback_namespace,
                    collected_at,
                )
            })
            .collect())
    }

    pub async fn get(
        &self,
        cluster: &KubernetesClusterConfig,
        namespace: &str,
        name: &str,
    ) -> Result<CronJob, AppError> {
        let api: Api<CronJob> =
            Api::namespaced(ClientFactory::get_client(cluster).await?, namespace);
        api.get(name)
            .await
            .map_err(|e| AppError::Kubernetes(e.to_string()))
    }

    pub async fn upsert(
        &self,
        cluster: &KubernetesClusterConfig,
        namespace: &str,
        item: &CronJob,
    ) -> Result<CronJob, AppError> {
        let api: Api<CronJob> =
            Api::namespaced(ClientFactory::get_client(cluster).await?, namespace);
        let params = PatchParams::apply("mayyam").force();
        api.patch(
            item.metadata
                .name
                .as_ref()
                .ok_or_else(|| AppError::BadRequest("CronJob.metadata.name required".into()))?,
            &params,
            &Patch::Apply(item),
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
        let api: Api<CronJob> =
            Api::namespaced(ClientFactory::get_client(cluster).await?, namespace);
        api.delete(name, &DeleteParams::default())
            .await
            .map_err(|e| AppError::Kubernetes(e.to_string()))?;
        Ok(())
    }
}

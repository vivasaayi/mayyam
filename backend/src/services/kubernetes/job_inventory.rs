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

// Deterministic Kubernetes Job inventory evaluator for roadmap rows
// 02-KUBERNETES-DASHBOARD-00393/00400/00421.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::services::aws::inventory::types::{
    score_pillar, InventoryFinding, Pillar, PillarReport, Severity, COST_ALLOCATION_TAG_KEYS,
    DEFAULT_STALE_AFTER_HOURS,
};

pub const RESOURCE_TYPE: &str = "KubernetesJob";
pub const REASON_COST_OWNER_NOT_RECORDED: &str = "K8S_JOB_COST_OWNER_NOT_RECORDED";
pub const REASON_RES_FAILED_PODS: &str = "K8S_JOB_RES_FAILED_PODS";
pub const REASON_RES_BACKOFF_LIMIT_REACHED: &str = "K8S_JOB_RES_BACKOFF_LIMIT_REACHED";
pub const REASON_RES_COMPLETIONS_STALLED: &str = "K8S_JOB_RES_COMPLETIONS_STALLED";
pub const REASON_SEC_PRIVILEGED_CONTAINER: &str = "K8S_JOB_SEC_PRIVILEGED_CONTAINER";
pub const REASON_SEC_HOST_NETWORK: &str = "K8S_JOB_SEC_HOST_NETWORK";
pub const REASON_INV_STALE_DATA: &str = "K8S_JOB_INV_STALE_DATA";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobContainerInventoryItem {
    pub name: String,
    pub image: Option<String>,
    pub privileged: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobConditionInventoryItem {
    pub type_: String,
    pub status: String,
    pub reason: Option<String>,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobInventoryItem {
    pub cluster_id: String,
    pub namespace: String,
    pub name: String,
    pub completions: Option<i32>,
    pub parallelism: Option<i32>,
    pub active: i32,
    pub ready: i32,
    pub succeeded: i32,
    pub failed: i32,
    pub backoff_limit: Option<i32>,
    pub active_deadline_seconds: Option<i64>,
    pub ttl_seconds_after_finished: Option<i32>,
    pub completion_mode: Option<String>,
    pub suspend: bool,
    pub labels: BTreeMap<String, String>,
    pub annotations: BTreeMap<String, String>,
    pub selector: BTreeMap<String, String>,
    pub pod_template_labels: BTreeMap<String, String>,
    pub containers: Vec<JobContainerInventoryItem>,
    pub conditions: Vec<JobConditionInventoryItem>,
    pub service_account_name: Option<String>,
    pub restart_policy: Option<String>,
    pub host_network: bool,
    pub start_time: Option<DateTime<Utc>>,
    pub completion_time: Option<DateTime<Utc>>,
    pub created_at: Option<DateTime<Utc>>,
    pub collected_at: DateTime<Utc>,
}

pub fn evaluate_kubernetes_job_inventory(
    jobs: &[JobInventoryItem],
    pillar: Pillar,
    now: DateTime<Utc>,
) -> PillarReport {
    let mut stale_resources = 0;
    let mut findings = Vec::new();

    for job in jobs {
        if let Some(finding) = stale_finding(job, pillar, now) {
            stale_resources += 1;
            findings.push(finding);
        }

        match pillar {
            Pillar::Cost => evaluate_cost(job, pillar, &mut findings),
            Pillar::Resilience => evaluate_resilience(job, pillar, &mut findings),
            Pillar::Security => evaluate_security(job, pillar, &mut findings),
            _ => {}
        }
    }

    PillarReport {
        pillar,
        resources_evaluated: jobs.len(),
        stale_resources,
        score: score_pillar(&findings),
        findings,
    }
}

fn evaluate_cost(job: &JobInventoryItem, pillar: Pillar, findings: &mut Vec<InventoryFinding>) {
    if has_any_metadata_key(&job.labels, COST_ALLOCATION_TAG_KEYS)
        || has_any_metadata_key(&job.annotations, COST_ALLOCATION_TAG_KEYS)
        || has_any_metadata_key(&job.pod_template_labels, COST_ALLOCATION_TAG_KEYS)
    {
        return;
    }

    findings.push(finding(
        job,
        pillar,
        REASON_COST_OWNER_NOT_RECORDED,
        Severity::Medium,
        format!(
            "Kubernetes Job {}/{} has no owner, team, project, or cost-center label or annotation",
            job.namespace, job.name
        ),
        json!({
            "cluster_id": job.cluster_id,
            "namespace": job.namespace,
            "job": job.name,
            "completions": job.completions,
            "parallelism": job.parallelism,
            "ttl_seconds_after_finished": job.ttl_seconds_after_finished,
            "checked_keys": COST_ALLOCATION_TAG_KEYS,
            "checked_locations": ["labels", "annotations", "pod_template_labels"],
        }),
    ));
}

fn evaluate_resilience(
    job: &JobInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if job.failed > 0 {
        findings.push(finding(
            job,
            pillar,
            REASON_RES_FAILED_PODS,
            Severity::High,
            format!(
                "Kubernetes Job {}/{} has {} failed pods",
                job.namespace, job.name, job.failed
            ),
            json!({
                "cluster_id": job.cluster_id,
                "namespace": job.namespace,
                "job": job.name,
                "failed": job.failed,
                "succeeded": job.succeeded,
                "active": job.active,
                "ready": job.ready,
                "conditions": job.conditions,
            }),
        ));
    }

    if let Some(backoff_limit) = job.backoff_limit {
        if backoff_limit >= 0 && job.failed >= backoff_limit && job.failed > 0 {
            findings.push(finding(
                job,
                pillar,
                REASON_RES_BACKOFF_LIMIT_REACHED,
                Severity::High,
                format!(
                    "Kubernetes Job {}/{} has reached backoff limit {}",
                    job.namespace, job.name, backoff_limit
                ),
                json!({
                    "cluster_id": job.cluster_id,
                    "namespace": job.namespace,
                    "job": job.name,
                    "failed": job.failed,
                    "backoff_limit": backoff_limit,
                    "restart_policy": job.restart_policy,
                }),
            ));
        }
    }

    if let Some(completions) = job.completions {
        if completions > 0 && job.succeeded < completions && job.active == 0 && !job.suspend {
            findings.push(finding(
                job,
                pillar,
                REASON_RES_COMPLETIONS_STALLED,
                Severity::Medium,
                format!(
                    "Kubernetes Job {}/{} has {}/{} completions and no active pods",
                    job.namespace, job.name, job.succeeded, completions
                ),
                json!({
                    "cluster_id": job.cluster_id,
                    "namespace": job.namespace,
                    "job": job.name,
                    "completions": completions,
                    "succeeded": job.succeeded,
                    "active": job.active,
                    "parallelism": job.parallelism,
                    "active_deadline_seconds": job.active_deadline_seconds,
                }),
            ));
        }
    }
}

fn evaluate_security(job: &JobInventoryItem, pillar: Pillar, findings: &mut Vec<InventoryFinding>) {
    let privileged_containers = job
        .containers
        .iter()
        .filter(|container| container.privileged == Some(true))
        .map(|container| container.name.clone())
        .collect::<Vec<_>>();
    if !privileged_containers.is_empty() {
        findings.push(finding(
            job,
            pillar,
            REASON_SEC_PRIVILEGED_CONTAINER,
            Severity::High,
            format!(
                "Kubernetes Job {}/{} template has privileged containers",
                job.namespace, job.name
            ),
            json!({
                "cluster_id": job.cluster_id,
                "namespace": job.namespace,
                "job": job.name,
                "privileged_containers": privileged_containers,
                "service_account_name": job.service_account_name,
            }),
        ));
    }

    if job.host_network {
        findings.push(finding(
            job,
            pillar,
            REASON_SEC_HOST_NETWORK,
            Severity::High,
            format!(
                "Kubernetes Job {}/{} template runs with hostNetwork enabled",
                job.namespace, job.name
            ),
            json!({
                "cluster_id": job.cluster_id,
                "namespace": job.namespace,
                "job": job.name,
                "host_network": job.host_network,
            }),
        ));
    }
}

fn stale_finding(
    job: &JobInventoryItem,
    pillar: Pillar,
    now: DateTime<Utc>,
) -> Option<InventoryFinding> {
    let age_hours = (now - job.collected_at).num_hours();
    if age_hours <= DEFAULT_STALE_AFTER_HOURS {
        return None;
    }

    Some(finding(
        job,
        pillar,
        REASON_INV_STALE_DATA,
        Severity::Medium,
        format!(
            "Inventory data for Kubernetes Job {}/{} is {} hours old (threshold {} hours)",
            job.namespace, job.name, age_hours, DEFAULT_STALE_AFTER_HOURS
        ),
        json!({
            "cluster_id": job.cluster_id,
            "namespace": job.namespace,
            "job": job.name,
            "collected_at": job.collected_at,
            "age_hours": age_hours,
            "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
        }),
    ))
}

fn finding(
    job: &JobInventoryItem,
    pillar: Pillar,
    reason_code: &str,
    severity: Severity,
    message: String,
    evidence: Value,
) -> InventoryFinding {
    InventoryFinding {
        resource_id: format!("{}/{}/{}", job.cluster_id, job.namespace, job.name),
        arn: format!(
            "kubernetes://job/{}/{}/{}",
            job.cluster_id, job.namespace, job.name
        ),
        pillar,
        reason_code: reason_code.to_string(),
        severity,
        message,
        evidence,
    }
}

fn has_any_metadata_key(metadata: &BTreeMap<String, String>, wanted_keys: &[&str]) -> bool {
    wanted_keys
        .iter()
        .any(|wanted| metadata_value(metadata, wanted).is_some())
}

fn metadata_value(metadata: &BTreeMap<String, String>, wanted_key: &str) -> Option<String> {
    metadata
        .iter()
        .find(|(key, value)| key.eq_ignore_ascii_case(wanted_key) && !value.trim().is_empty())
        .map(|(_, value)| value.clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::aws::inventory::types::{Pillar, DEFAULT_STALE_AFTER_HOURS};
    use chrono::Duration;

    fn now() -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2026-06-10T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc)
    }

    fn labels(values: &[(&str, &str)]) -> BTreeMap<String, String> {
        values
            .iter()
            .map(|(key, value)| ((*key).to_string(), (*value).to_string()))
            .collect()
    }

    fn container(name: &str, privileged: Option<bool>) -> JobContainerInventoryItem {
        JobContainerInventoryItem {
            name: name.to_string(),
            image: Some("registry.local/job:1.0".to_string()),
            privileged,
        }
    }

    fn job(
        name: &str,
        metadata_labels: BTreeMap<String, String>,
        completions: Option<i32>,
        succeeded: i32,
        failed: i32,
        active: i32,
    ) -> JobInventoryItem {
        JobInventoryItem {
            cluster_id: "cluster-a".to_string(),
            namespace: "batch".to_string(),
            name: name.to_string(),
            completions,
            parallelism: Some(1),
            active,
            ready: active,
            succeeded,
            failed,
            backoff_limit: Some(3),
            active_deadline_seconds: Some(3600),
            ttl_seconds_after_finished: Some(600),
            completion_mode: Some("NonIndexed".to_string()),
            suspend: false,
            labels: metadata_labels,
            annotations: BTreeMap::new(),
            selector: labels(&[("job-name", name)]),
            pod_template_labels: labels(&[("job-name", name)]),
            containers: vec![container("worker", Some(false))],
            conditions: Vec::new(),
            service_account_name: Some("batch-worker".to_string()),
            restart_policy: Some("Never".to_string()),
            host_network: false,
            start_time: Some(now() - Duration::minutes(10)),
            completion_time: None,
            created_at: Some(now() - Duration::minutes(12)),
            collected_at: now(),
        }
    }

    fn healthy_job() -> JobInventoryItem {
        let mut item = job(
            "invoice-close",
            labels(&[("team", "finance")]),
            Some(1),
            1,
            0,
            0,
        );
        item.completion_time = Some(now() - Duration::minutes(2));
        item
    }

    #[test]
    fn cost_flags_missing_owner_and_cost_allocation_labels() {
        let report = evaluate_kubernetes_job_inventory(
            &[job("untagged", BTreeMap::new(), Some(1), 1, 0, 0)],
            Pillar::Cost,
            now(),
        );

        assert_eq!(report.resources_evaluated, 1);
        assert_eq!(report.findings.len(), 1);
        assert_eq!(
            report.findings[0].reason_code,
            REASON_COST_OWNER_NOT_RECORDED
        );
        assert_eq!(report.findings[0].pillar, Pillar::Cost);
    }

    #[test]
    fn resilience_flags_failed_backoff_and_stalled_completions() {
        let stalled = job("etl", labels(&[("team", "data")]), Some(4), 1, 3, 0);

        let report = evaluate_kubernetes_job_inventory(&[stalled], Pillar::Resilience, now());
        let reason_codes = report
            .findings
            .iter()
            .map(|finding| finding.reason_code.as_str())
            .collect::<Vec<_>>();

        assert!(reason_codes.contains(&REASON_RES_FAILED_PODS));
        assert!(reason_codes.contains(&REASON_RES_BACKOFF_LIMIT_REACHED));
        assert!(reason_codes.contains(&REASON_RES_COMPLETIONS_STALLED));
    }

    #[test]
    fn security_flags_privileged_template_and_host_network() {
        let mut exposed = healthy_job();
        exposed.host_network = true;
        exposed.containers = vec![
            container("worker", Some(false)),
            container("collector", Some(true)),
        ];

        let report = evaluate_kubernetes_job_inventory(&[exposed], Pillar::Security, now());
        let reason_codes = report
            .findings
            .iter()
            .map(|finding| finding.reason_code.as_str())
            .collect::<Vec<_>>();

        assert!(reason_codes.contains(&REASON_SEC_PRIVILEGED_CONTAINER));
        assert!(reason_codes.contains(&REASON_SEC_HOST_NETWORK));
    }

    #[test]
    fn stale_inventory_is_counted_for_any_pillar() {
        let mut stale = healthy_job();
        stale.collected_at = now() - Duration::hours(DEFAULT_STALE_AFTER_HOURS + 2);

        let report = evaluate_kubernetes_job_inventory(&[stale], Pillar::Cost, now());

        assert_eq!(report.stale_resources, 1);
        assert_eq!(report.findings[0].reason_code, REASON_INV_STALE_DATA);
    }

    #[test]
    fn healthy_job_passes_claimed_pillars() {
        for pillar in [Pillar::Cost, Pillar::Resilience, Pillar::Security] {
            let report = evaluate_kubernetes_job_inventory(&[healthy_job()], pillar, now());

            assert_eq!(report.resources_evaluated, 1);
            assert_eq!(report.stale_resources, 0);
            assert!(report.findings.is_empty());
        }
    }
}

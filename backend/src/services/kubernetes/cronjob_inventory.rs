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

// Deterministic Kubernetes CronJob inventory evaluator for roadmap rows
// 02-KUBERNETES-DASHBOARD-00442/00449/00470.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::services::aws::inventory::types::{
    score_pillar, InventoryFinding, Pillar, PillarReport, Severity, COST_ALLOCATION_TAG_KEYS,
    DEFAULT_STALE_AFTER_HOURS,
};

pub const RESOURCE_TYPE: &str = "KubernetesCronJob";
pub const REASON_COST_OWNER_NOT_RECORDED: &str = "K8S_CRONJOB_COST_OWNER_NOT_RECORDED";
pub const REASON_RES_SUSPENDED: &str = "K8S_CRONJOB_RES_SUSPENDED";
pub const REASON_RES_CONCURRENCY_OVERLAP: &str = "K8S_CRONJOB_RES_CONCURRENCY_OVERLAP";
pub const REASON_RES_NO_SUCCESSFUL_RUN: &str = "K8S_CRONJOB_RES_NO_SUCCESSFUL_RUN";
pub const REASON_SEC_PRIVILEGED_CONTAINER: &str = "K8S_CRONJOB_SEC_PRIVILEGED_CONTAINER";
pub const REASON_SEC_HOST_NETWORK: &str = "K8S_CRONJOB_SEC_HOST_NETWORK";
pub const REASON_INV_STALE_DATA: &str = "K8S_CRONJOB_INV_STALE_DATA";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CronJobContainerInventoryItem {
    pub name: String,
    pub image: Option<String>,
    pub privileged: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CronJobActiveJobInventoryItem {
    pub name: Option<String>,
    pub namespace: Option<String>,
    pub uid: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CronJobInventoryItem {
    pub cluster_id: String,
    pub namespace: String,
    pub name: String,
    pub schedule: String,
    pub time_zone: Option<String>,
    pub concurrency_policy: Option<String>,
    pub suspend: bool,
    pub starting_deadline_seconds: Option<i64>,
    pub successful_jobs_history_limit: Option<i32>,
    pub failed_jobs_history_limit: Option<i32>,
    pub active_job_count: usize,
    pub active_jobs: Vec<CronJobActiveJobInventoryItem>,
    pub last_schedule_time: Option<DateTime<Utc>>,
    pub last_successful_time: Option<DateTime<Utc>>,
    pub job_completions: Option<i32>,
    pub job_parallelism: Option<i32>,
    pub job_backoff_limit: Option<i32>,
    pub job_active_deadline_seconds: Option<i64>,
    pub labels: BTreeMap<String, String>,
    pub annotations: BTreeMap<String, String>,
    pub job_template_labels: BTreeMap<String, String>,
    pub job_template_annotations: BTreeMap<String, String>,
    pub pod_template_labels: BTreeMap<String, String>,
    pub containers: Vec<CronJobContainerInventoryItem>,
    pub service_account_name: Option<String>,
    pub restart_policy: Option<String>,
    pub host_network: bool,
    pub created_at: Option<DateTime<Utc>>,
    pub collected_at: DateTime<Utc>,
}

pub fn evaluate_kubernetes_cronjob_inventory(
    cronjobs: &[CronJobInventoryItem],
    pillar: Pillar,
    now: DateTime<Utc>,
) -> PillarReport {
    let mut stale_resources = 0;
    let mut findings = Vec::new();

    for cronjob in cronjobs {
        if let Some(finding) = stale_finding(cronjob, pillar, now) {
            stale_resources += 1;
            findings.push(finding);
        }

        match pillar {
            Pillar::Cost => evaluate_cost(cronjob, pillar, &mut findings),
            Pillar::Resilience => evaluate_resilience(cronjob, pillar, &mut findings),
            Pillar::Security => evaluate_security(cronjob, pillar, &mut findings),
            _ => {}
        }
    }

    PillarReport {
        pillar,
        resources_evaluated: cronjobs.len(),
        stale_resources,
        score: score_pillar(&findings),
        findings,
    }
}

fn evaluate_cost(
    cronjob: &CronJobInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if has_any_metadata_key(&cronjob.labels, COST_ALLOCATION_TAG_KEYS)
        || has_any_metadata_key(&cronjob.annotations, COST_ALLOCATION_TAG_KEYS)
        || has_any_metadata_key(&cronjob.job_template_labels, COST_ALLOCATION_TAG_KEYS)
        || has_any_metadata_key(&cronjob.job_template_annotations, COST_ALLOCATION_TAG_KEYS)
        || has_any_metadata_key(&cronjob.pod_template_labels, COST_ALLOCATION_TAG_KEYS)
    {
        return;
    }

    findings.push(finding(
        cronjob,
        pillar,
        REASON_COST_OWNER_NOT_RECORDED,
        Severity::Medium,
        format!(
            "Kubernetes CronJob {}/{} has no owner, team, project, or cost-center label or annotation",
            cronjob.namespace, cronjob.name
        ),
        json!({
            "cluster_id": cronjob.cluster_id,
            "namespace": cronjob.namespace,
            "cronjob": cronjob.name,
            "schedule": cronjob.schedule,
            "job_parallelism": cronjob.job_parallelism,
            "successful_jobs_history_limit": cronjob.successful_jobs_history_limit,
            "failed_jobs_history_limit": cronjob.failed_jobs_history_limit,
            "checked_keys": COST_ALLOCATION_TAG_KEYS,
            "checked_locations": [
                "labels",
                "annotations",
                "job_template_labels",
                "job_template_annotations",
                "pod_template_labels"
            ],
        }),
    ));
}

fn evaluate_resilience(
    cronjob: &CronJobInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if cronjob.suspend {
        findings.push(finding(
            cronjob,
            pillar,
            REASON_RES_SUSPENDED,
            Severity::Medium,
            format!(
                "Kubernetes CronJob {}/{} is suspended",
                cronjob.namespace, cronjob.name
            ),
            json!({
                "cluster_id": cronjob.cluster_id,
                "namespace": cronjob.namespace,
                "cronjob": cronjob.name,
                "schedule": cronjob.schedule,
                "concurrency_policy": cronjob.concurrency_policy,
                "last_schedule_time": cronjob.last_schedule_time,
                "last_successful_time": cronjob.last_successful_time,
            }),
        ));
    }

    if cronjob
        .concurrency_policy
        .as_deref()
        .map(|policy| policy.eq_ignore_ascii_case("Allow"))
        .unwrap_or(false)
        && cronjob.active_job_count > 1
    {
        findings.push(finding(
            cronjob,
            pillar,
            REASON_RES_CONCURRENCY_OVERLAP,
            Severity::High,
            format!(
                "Kubernetes CronJob {}/{} allows concurrent runs and has {} active Jobs",
                cronjob.namespace, cronjob.name, cronjob.active_job_count
            ),
            json!({
                "cluster_id": cronjob.cluster_id,
                "namespace": cronjob.namespace,
                "cronjob": cronjob.name,
                "schedule": cronjob.schedule,
                "concurrency_policy": cronjob.concurrency_policy,
                "active_job_count": cronjob.active_job_count,
                "active_jobs": cronjob.active_jobs,
            }),
        ));
    }

    if cronjob.last_schedule_time.is_some() && cronjob.last_successful_time.is_none() {
        findings.push(finding(
            cronjob,
            pillar,
            REASON_RES_NO_SUCCESSFUL_RUN,
            Severity::Medium,
            format!(
                "Kubernetes CronJob {}/{} has scheduled runs but no recorded successful run",
                cronjob.namespace, cronjob.name
            ),
            json!({
                "cluster_id": cronjob.cluster_id,
                "namespace": cronjob.namespace,
                "cronjob": cronjob.name,
                "schedule": cronjob.schedule,
                "last_schedule_time": cronjob.last_schedule_time,
                "last_successful_time": cronjob.last_successful_time,
                "job_backoff_limit": cronjob.job_backoff_limit,
                "job_active_deadline_seconds": cronjob.job_active_deadline_seconds,
            }),
        ));
    }
}

fn evaluate_security(
    cronjob: &CronJobInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    let privileged_containers = cronjob
        .containers
        .iter()
        .filter(|container| container.privileged == Some(true))
        .map(|container| container.name.clone())
        .collect::<Vec<_>>();
    if !privileged_containers.is_empty() {
        findings.push(finding(
            cronjob,
            pillar,
            REASON_SEC_PRIVILEGED_CONTAINER,
            Severity::High,
            format!(
                "Kubernetes CronJob {}/{} template has privileged containers",
                cronjob.namespace, cronjob.name
            ),
            json!({
                "cluster_id": cronjob.cluster_id,
                "namespace": cronjob.namespace,
                "cronjob": cronjob.name,
                "privileged_containers": privileged_containers,
                "service_account_name": cronjob.service_account_name,
            }),
        ));
    }

    if cronjob.host_network {
        findings.push(finding(
            cronjob,
            pillar,
            REASON_SEC_HOST_NETWORK,
            Severity::High,
            format!(
                "Kubernetes CronJob {}/{} template runs with hostNetwork enabled",
                cronjob.namespace, cronjob.name
            ),
            json!({
                "cluster_id": cronjob.cluster_id,
                "namespace": cronjob.namespace,
                "cronjob": cronjob.name,
                "host_network": cronjob.host_network,
            }),
        ));
    }
}

fn stale_finding(
    cronjob: &CronJobInventoryItem,
    pillar: Pillar,
    now: DateTime<Utc>,
) -> Option<InventoryFinding> {
    let age_hours = (now - cronjob.collected_at).num_hours();
    if age_hours <= DEFAULT_STALE_AFTER_HOURS {
        return None;
    }

    Some(finding(
        cronjob,
        pillar,
        REASON_INV_STALE_DATA,
        Severity::Medium,
        format!(
            "Inventory data for Kubernetes CronJob {}/{} is {} hours old (threshold {} hours)",
            cronjob.namespace, cronjob.name, age_hours, DEFAULT_STALE_AFTER_HOURS
        ),
        json!({
            "cluster_id": cronjob.cluster_id,
            "namespace": cronjob.namespace,
            "cronjob": cronjob.name,
            "collected_at": cronjob.collected_at,
            "age_hours": age_hours,
            "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
        }),
    ))
}

fn finding(
    cronjob: &CronJobInventoryItem,
    pillar: Pillar,
    reason_code: &str,
    severity: Severity,
    message: String,
    evidence: Value,
) -> InventoryFinding {
    InventoryFinding {
        resource_id: format!(
            "{}/{}/{}",
            cronjob.cluster_id, cronjob.namespace, cronjob.name
        ),
        arn: format!(
            "kubernetes://cronjob/{}/{}/{}",
            cronjob.cluster_id, cronjob.namespace, cronjob.name
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

    fn container(name: &str, privileged: Option<bool>) -> CronJobContainerInventoryItem {
        CronJobContainerInventoryItem {
            name: name.to_string(),
            image: Some("registry.local/cronjob:1.0".to_string()),
            privileged,
        }
    }

    fn active_job(name: &str) -> CronJobActiveJobInventoryItem {
        CronJobActiveJobInventoryItem {
            name: Some(name.to_string()),
            namespace: Some("batch".to_string()),
            uid: Some(format!("uid-{name}")),
        }
    }

    fn cronjob(name: &str, metadata_labels: BTreeMap<String, String>) -> CronJobInventoryItem {
        CronJobInventoryItem {
            cluster_id: "cluster-a".to_string(),
            namespace: "batch".to_string(),
            name: name.to_string(),
            schedule: "*/15 * * * *".to_string(),
            time_zone: Some("Etc/UTC".to_string()),
            concurrency_policy: Some("Forbid".to_string()),
            suspend: false,
            starting_deadline_seconds: Some(600),
            successful_jobs_history_limit: Some(3),
            failed_jobs_history_limit: Some(1),
            active_job_count: 0,
            active_jobs: Vec::new(),
            last_schedule_time: Some(now() - Duration::minutes(15)),
            last_successful_time: Some(now() - Duration::minutes(10)),
            job_completions: Some(1),
            job_parallelism: Some(1),
            job_backoff_limit: Some(3),
            job_active_deadline_seconds: Some(1800),
            labels: metadata_labels,
            annotations: BTreeMap::new(),
            job_template_labels: labels(&[("job-family", name)]),
            job_template_annotations: BTreeMap::new(),
            pod_template_labels: labels(&[("cronjob", name)]),
            containers: vec![container("worker", Some(false))],
            service_account_name: Some("batch-worker".to_string()),
            restart_policy: Some("OnFailure".to_string()),
            host_network: false,
            created_at: Some(now() - Duration::days(2)),
            collected_at: now(),
        }
    }

    fn healthy_cronjob() -> CronJobInventoryItem {
        cronjob("invoice-close", labels(&[("team", "finance")]))
    }

    #[test]
    fn cost_flags_missing_owner_and_cost_allocation_labels() {
        let report = evaluate_kubernetes_cronjob_inventory(
            &[cronjob("untagged", BTreeMap::new())],
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
    fn resilience_flags_suspended_overlap_and_missing_success() {
        let mut overlapping = cronjob("ledger-rollup", labels(&[("team", "finance")]));
        overlapping.suspend = true;
        overlapping.concurrency_policy = Some("Allow".to_string());
        overlapping.active_jobs =
            vec![active_job("ledger-rollup-1"), active_job("ledger-rollup-2")];
        overlapping.active_job_count = overlapping.active_jobs.len();
        overlapping.last_schedule_time = Some(now() - Duration::minutes(20));
        overlapping.last_successful_time = None;

        let report =
            evaluate_kubernetes_cronjob_inventory(&[overlapping], Pillar::Resilience, now());
        let reason_codes = report
            .findings
            .iter()
            .map(|finding| finding.reason_code.as_str())
            .collect::<Vec<_>>();

        assert!(reason_codes.contains(&REASON_RES_SUSPENDED));
        assert!(reason_codes.contains(&REASON_RES_CONCURRENCY_OVERLAP));
        assert!(reason_codes.contains(&REASON_RES_NO_SUCCESSFUL_RUN));
    }

    #[test]
    fn security_flags_privileged_template_and_host_network() {
        let mut exposed = healthy_cronjob();
        exposed.host_network = true;
        exposed.containers = vec![
            container("worker", Some(false)),
            container("collector", Some(true)),
        ];

        let report = evaluate_kubernetes_cronjob_inventory(&[exposed], Pillar::Security, now());
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
        let mut stale = healthy_cronjob();
        stale.collected_at = now() - Duration::hours(DEFAULT_STALE_AFTER_HOURS + 2);

        let report = evaluate_kubernetes_cronjob_inventory(&[stale], Pillar::Cost, now());

        assert_eq!(report.stale_resources, 1);
        assert_eq!(report.findings[0].reason_code, REASON_INV_STALE_DATA);
    }

    #[test]
    fn healthy_cronjob_passes_claimed_pillars() {
        for pillar in [Pillar::Cost, Pillar::Resilience, Pillar::Security] {
            let report = evaluate_kubernetes_cronjob_inventory(&[healthy_cronjob()], pillar, now());

            assert_eq!(report.resources_evaluated, 1);
            assert_eq!(report.stale_resources, 0);
            assert!(report.findings.is_empty());
        }
    }
}

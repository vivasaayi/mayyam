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

// Deterministic Kubernetes Pod Logs inventory evaluator for roadmap rows
// 02-KUBERNETES-DASHBOARD-01716/01723/01744.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::services::aws::inventory::types::{
    score_pillar, InventoryFinding, Pillar, PillarReport, Severity, COST_ALLOCATION_TAG_KEYS,
    DEFAULT_STALE_AFTER_HOURS,
};

pub const RESOURCE_TYPE: &str = "KubernetesPodLog";
pub const REASON_COST_HIGH_RESTART_LOG_VOLUME: &str = "K8S_POD_LOG_COST_HIGH_RESTART_LOG_VOLUME";
pub const REASON_COST_OWNER_NOT_RECORDED: &str = "K8S_POD_LOG_COST_OWNER_NOT_RECORDED";
pub const REASON_RES_CONTAINER_NOT_READY: &str = "K8S_POD_LOG_RES_CONTAINER_NOT_READY";
pub const REASON_RES_UNHEALTHY_PHASE: &str = "K8S_POD_LOG_RES_UNHEALTHY_PHASE";
pub const REASON_SEC_PRIVILEGED_LOG_TARGET: &str = "K8S_POD_LOG_SEC_PRIVILEGED_LOG_TARGET";
pub const REASON_SEC_HOST_NETWORK_LOG_TARGET: &str = "K8S_POD_LOG_SEC_HOST_NETWORK_LOG_TARGET";
pub const REASON_INV_STALE_DATA: &str = "K8S_POD_LOG_INV_STALE_DATA";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PodLogInventoryItem {
    pub cluster_id: String,
    pub namespace: String,
    pub pod_name: String,
    pub container_name: String,
    pub image: Option<String>,
    pub phase: Option<String>,
    pub ready: Option<bool>,
    pub restart_count: i32,
    pub previous_logs_available: bool,
    pub labels: BTreeMap<String, String>,
    pub annotations: BTreeMap<String, String>,
    pub node_name: Option<String>,
    pub controlled_by: Option<String>,
    pub controller_kind: Option<String>,
    pub service_account_name: Option<String>,
    pub privileged: Option<bool>,
    pub host_network: bool,
    pub created_at: Option<DateTime<Utc>>,
    pub collected_at: DateTime<Utc>,
}

pub fn evaluate_kubernetes_pod_log_inventory(
    log_targets: &[PodLogInventoryItem],
    pillar: Pillar,
    now: DateTime<Utc>,
) -> PillarReport {
    let mut stale_resources = 0;
    let mut findings = Vec::new();

    for target in log_targets {
        if let Some(finding) = stale_finding(target, pillar, now) {
            stale_resources += 1;
            findings.push(finding);
        }

        match pillar {
            Pillar::Cost => evaluate_cost(target, pillar, &mut findings),
            Pillar::Resilience => evaluate_resilience(target, pillar, &mut findings),
            Pillar::Security => evaluate_security(target, pillar, &mut findings),
            _ => {}
        }
    }

    PillarReport {
        pillar,
        resources_evaluated: log_targets.len(),
        stale_resources,
        score: score_pillar(&findings),
        findings,
    }
}

fn evaluate_cost(
    target: &PodLogInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if target.restart_count >= 10 {
        findings.push(finding(
            target,
            pillar,
            REASON_COST_HIGH_RESTART_LOG_VOLUME,
            Severity::Medium,
            format!(
                "Kubernetes Pod Logs target {}/{}/{} restarted {} times; previous logs and repeated startup output can drive log volume",
                target.namespace, target.pod_name, target.container_name, target.restart_count
            ),
            json!({
                "cluster_id": target.cluster_id,
                "namespace": target.namespace,
                "pod": target.pod_name,
                "container": target.container_name,
                "restart_count": target.restart_count,
                "previous_logs_available": target.previous_logs_available,
                "controller_kind": target.controller_kind,
                "controlled_by": target.controlled_by,
                "recommendation": "Investigate restart loops and noisy startup logging before expanding log retention or central export for this target",
            }),
        ));
    }

    if has_any_metadata_key(&target.labels, COST_ALLOCATION_TAG_KEYS)
        || has_any_metadata_key(&target.annotations, COST_ALLOCATION_TAG_KEYS)
    {
        return;
    }

    findings.push(finding(
        target,
        pillar,
        REASON_COST_OWNER_NOT_RECORDED,
        Severity::Medium,
        format!(
            "Kubernetes Pod Logs target {}/{}/{} has no owner, team, project, or cost-center label or annotation",
            target.namespace, target.pod_name, target.container_name
        ),
        json!({
            "cluster_id": target.cluster_id,
            "namespace": target.namespace,
            "pod": target.pod_name,
            "container": target.container_name,
            "checked_keys": COST_ALLOCATION_TAG_KEYS,
            "checked_locations": ["labels", "annotations"],
        }),
    ));
}

fn evaluate_resilience(
    target: &PodLogInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if target.ready == Some(false) {
        findings.push(finding(
            target,
            pillar,
            REASON_RES_CONTAINER_NOT_READY,
            Severity::High,
            format!(
                "Kubernetes Pod Logs target {}/{}/{} is not ready, so live logs may not represent a healthy serving path",
                target.namespace, target.pod_name, target.container_name
            ),
            json!({
                "cluster_id": target.cluster_id,
                "namespace": target.namespace,
                "pod": target.pod_name,
                "container": target.container_name,
                "phase": target.phase,
                "ready": target.ready,
                "restart_count": target.restart_count,
                "previous_logs_available": target.previous_logs_available,
            }),
        ));
    }

    let Some(phase) = target
        .phase
        .as_deref()
        .map(str::trim)
        .filter(|phase| !phase.is_empty())
    else {
        return;
    };
    let healthy_phase =
        phase.eq_ignore_ascii_case("Running") || phase.eq_ignore_ascii_case("Succeeded");
    if !healthy_phase {
        findings.push(finding(
            target,
            pillar,
            REASON_RES_UNHEALTHY_PHASE,
            Severity::High,
            format!(
                "Kubernetes Pod Logs target {}/{}/{} belongs to pod phase {}; expected Running or Succeeded",
                target.namespace, target.pod_name, target.container_name, phase
            ),
            json!({
                "cluster_id": target.cluster_id,
                "namespace": target.namespace,
                "pod": target.pod_name,
                "container": target.container_name,
                "phase": phase,
                "ready": target.ready,
                "restart_count": target.restart_count,
            }),
        ));
    }
}

fn evaluate_security(
    target: &PodLogInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if target.privileged == Some(true) {
        findings.push(finding(
            target,
            pillar,
            REASON_SEC_PRIVILEGED_LOG_TARGET,
            Severity::High,
            format!(
                "Kubernetes Pod Logs target {}/{}/{} runs privileged and may emit host-sensitive details",
                target.namespace, target.pod_name, target.container_name
            ),
            json!({
                "cluster_id": target.cluster_id,
                "namespace": target.namespace,
                "pod": target.pod_name,
                "container": target.container_name,
                "privileged": target.privileged,
                "service_account_name": target.service_account_name,
                "node_name": target.node_name,
                "recommendation": "Review log redaction and access controls before broadening log access for privileged workloads",
            }),
        ));
    }

    if target.host_network {
        findings.push(finding(
            target,
            pillar,
            REASON_SEC_HOST_NETWORK_LOG_TARGET,
            Severity::High,
            format!(
                "Kubernetes Pod Logs target {}/{}/{} runs with hostNetwork enabled",
                target.namespace, target.pod_name, target.container_name
            ),
            json!({
                "cluster_id": target.cluster_id,
                "namespace": target.namespace,
                "pod": target.pod_name,
                "container": target.container_name,
                "host_network": target.host_network,
                "node_name": target.node_name,
            }),
        ));
    }
}

fn stale_finding(
    target: &PodLogInventoryItem,
    pillar: Pillar,
    now: DateTime<Utc>,
) -> Option<InventoryFinding> {
    let age_hours = (now - target.collected_at).num_hours();
    if age_hours <= DEFAULT_STALE_AFTER_HOURS {
        return None;
    }

    Some(finding(
        target,
        pillar,
        REASON_INV_STALE_DATA,
        Severity::Medium,
        format!(
            "Inventory data for Kubernetes Pod Logs target {}/{}/{} is {} hours old (threshold {} hours)",
            target.namespace,
            target.pod_name,
            target.container_name,
            age_hours,
            DEFAULT_STALE_AFTER_HOURS
        ),
        json!({
            "cluster_id": target.cluster_id,
            "namespace": target.namespace,
            "pod": target.pod_name,
            "container": target.container_name,
            "collected_at": target.collected_at,
            "age_hours": age_hours,
            "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
        }),
    ))
}

fn finding(
    target: &PodLogInventoryItem,
    pillar: Pillar,
    reason_code: &str,
    severity: Severity,
    message: String,
    evidence: Value,
) -> InventoryFinding {
    InventoryFinding {
        resource_id: format!(
            "{}/{}/PodLog/{}/{}",
            target.cluster_id, target.namespace, target.pod_name, target.container_name
        ),
        arn: format!(
            "kubernetes://podlogs/{}/{}/{}/{}",
            target.cluster_id, target.namespace, target.pod_name, target.container_name
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

    fn healthy_log_target() -> PodLogInventoryItem {
        PodLogInventoryItem {
            cluster_id: "cluster-a".to_string(),
            namespace: "apps".to_string(),
            pod_name: "pod-a".to_string(),
            container_name: "app".to_string(),
            image: Some("registry.local/app:1.0.0".to_string()),
            phase: Some("Running".to_string()),
            ready: Some(true),
            restart_count: 0,
            previous_logs_available: false,
            labels: labels(&[("owner", "platform"), ("cost-center", "cc-42")]),
            annotations: BTreeMap::new(),
            node_name: Some("node-a".to_string()),
            controlled_by: Some("pod-a-rs".to_string()),
            controller_kind: Some("ReplicaSet".to_string()),
            service_account_name: Some("app".to_string()),
            privileged: Some(false),
            host_network: false,
            created_at: Some(now() - Duration::hours(2)),
            collected_at: now(),
        }
    }

    #[test]
    fn cost_flags_high_restart_log_volume() {
        let mut noisy = healthy_log_target();
        noisy.restart_count = 12;
        noisy.previous_logs_available = true;

        let report = evaluate_kubernetes_pod_log_inventory(&[noisy], Pillar::Cost, now());

        assert_eq!(report.resources_evaluated, 1);
        assert_eq!(report.findings.len(), 1);
        assert_eq!(
            report.findings[0].reason_code,
            REASON_COST_HIGH_RESTART_LOG_VOLUME
        );
        assert_eq!(report.findings[0].pillar, Pillar::Cost);
    }

    #[test]
    fn resilience_flags_unready_log_targets() {
        let mut unready = healthy_log_target();
        unready.ready = Some(false);

        let report = evaluate_kubernetes_pod_log_inventory(&[unready], Pillar::Resilience, now());

        assert_eq!(report.findings.len(), 1);
        assert_eq!(
            report.findings[0].reason_code,
            REASON_RES_CONTAINER_NOT_READY
        );
    }

    #[test]
    fn security_flags_privileged_log_targets() {
        let mut privileged = healthy_log_target();
        privileged.privileged = Some(true);

        let report = evaluate_kubernetes_pod_log_inventory(&[privileged], Pillar::Security, now());

        assert_eq!(report.findings.len(), 1);
        assert_eq!(
            report.findings[0].reason_code,
            REASON_SEC_PRIVILEGED_LOG_TARGET
        );
    }

    #[test]
    fn stale_inventory_is_counted_for_any_pillar() {
        let mut stale = healthy_log_target();
        stale.collected_at = now() - Duration::hours(DEFAULT_STALE_AFTER_HOURS + 2);

        let report = evaluate_kubernetes_pod_log_inventory(&[stale], Pillar::Cost, now());

        assert_eq!(report.stale_resources, 1);
        assert_eq!(report.findings[0].reason_code, REASON_INV_STALE_DATA);
    }

    #[test]
    fn healthy_log_targets_pass_claimed_pillars() {
        for pillar in [Pillar::Cost, Pillar::Resilience, Pillar::Security] {
            let report =
                evaluate_kubernetes_pod_log_inventory(&[healthy_log_target()], pillar, now());

            assert_eq!(report.resources_evaluated, 1);
            assert_eq!(report.stale_resources, 0);
            assert!(report.findings.is_empty());
        }
    }
}

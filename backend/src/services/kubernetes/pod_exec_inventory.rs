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

// Deterministic Kubernetes Pod Exec inventory evaluator for roadmap rows
// 02-KUBERNETES-DASHBOARD-01765/01772/01793.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::services::aws::inventory::types::{
    score_pillar, InventoryFinding, Pillar, PillarReport, Severity, COST_ALLOCATION_TAG_KEYS,
    DEFAULT_STALE_AFTER_HOURS,
};

pub const RESOURCE_TYPE: &str = "KubernetesPodExec";
pub const REASON_COST_OWNER_NOT_RECORDED: &str = "K8S_POD_EXEC_COST_OWNER_NOT_RECORDED";
pub const REASON_COST_HIGH_RESTART_DEBUG_CHURN: &str = "K8S_POD_EXEC_COST_HIGH_RESTART_DEBUG_CHURN";
pub const REASON_RES_CONTAINER_NOT_READY: &str = "K8S_POD_EXEC_RES_CONTAINER_NOT_READY";
pub const REASON_RES_UNHEALTHY_PHASE: &str = "K8S_POD_EXEC_RES_UNHEALTHY_PHASE";
pub const REASON_SEC_PRIVILEGED_EXEC_TARGET: &str = "K8S_POD_EXEC_SEC_PRIVILEGED_TARGET";
pub const REASON_SEC_HOST_NAMESPACE_EXEC_TARGET: &str = "K8S_POD_EXEC_SEC_HOST_NAMESPACE_TARGET";
pub const REASON_SEC_AUTOMOUNTED_TOKEN: &str = "K8S_POD_EXEC_SEC_AUTOMOUNTED_TOKEN";
pub const REASON_INV_STALE_DATA: &str = "K8S_POD_EXEC_INV_STALE_DATA";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PodExecInventoryItem {
    pub cluster_id: String,
    pub namespace: String,
    pub pod_name: String,
    pub container_name: String,
    pub image: Option<String>,
    pub phase: Option<String>,
    pub ready: Option<bool>,
    pub restart_count: i32,
    pub labels: BTreeMap<String, String>,
    pub annotations: BTreeMap<String, String>,
    pub node_name: Option<String>,
    pub controlled_by: Option<String>,
    pub controller_kind: Option<String>,
    pub service_account_name: Option<String>,
    pub automount_service_account_token: Option<bool>,
    pub privileged: Option<bool>,
    pub host_network: bool,
    pub host_pid: bool,
    pub host_ipc: bool,
    pub stdin: Option<bool>,
    pub tty: Option<bool>,
    pub created_at: Option<DateTime<Utc>>,
    pub collected_at: DateTime<Utc>,
}

pub fn evaluate_kubernetes_pod_exec_inventory(
    exec_targets: &[PodExecInventoryItem],
    pillar: Pillar,
    now: DateTime<Utc>,
) -> PillarReport {
    let mut stale_resources = 0;
    let mut findings = Vec::new();

    for target in exec_targets {
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
        resources_evaluated: exec_targets.len(),
        stale_resources,
        score: score_pillar(&findings),
        findings,
    }
}

fn evaluate_cost(
    target: &PodExecInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if target.restart_count >= 10 {
        findings.push(finding(
            target,
            pillar,
            REASON_COST_HIGH_RESTART_DEBUG_CHURN,
            Severity::Medium,
            format!(
                "Kubernetes Pod Exec target {}/{}/{} restarted {} times; repeated debug sessions can mask noisy workload churn",
                target.namespace, target.pod_name, target.container_name, target.restart_count
            ),
            json!({
                "cluster_id": target.cluster_id,
                "namespace": target.namespace,
                "pod": target.pod_name,
                "container": target.container_name,
                "restart_count": target.restart_count,
                "controller_kind": target.controller_kind,
                "controlled_by": target.controlled_by,
                "recommendation": "Fix restart loops instead of relying on repeated interactive debugging of this target",
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
            "Kubernetes Pod Exec target {}/{}/{} has no owner, team, project, or cost-center label or annotation",
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
    target: &PodExecInventoryItem,
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
                "Kubernetes Pod Exec target {}/{}/{} is not ready",
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
                "Kubernetes Pod Exec target {}/{}/{} belongs to pod phase {}; expected Running or Succeeded",
                target.namespace, target.pod_name, target.container_name, phase
            ),
            json!({
                "cluster_id": target.cluster_id,
                "namespace": target.namespace,
                "pod": target.pod_name,
                "container": target.container_name,
                "phase": phase,
                "ready": target.ready,
            }),
        ));
    }
}

fn evaluate_security(
    target: &PodExecInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if target.privileged == Some(true) {
        findings.push(finding(
            target,
            pillar,
            REASON_SEC_PRIVILEGED_EXEC_TARGET,
            Severity::High,
            format!(
                "Kubernetes Pod Exec target {}/{}/{} runs privileged",
                target.namespace, target.pod_name, target.container_name
            ),
            json!({
                "cluster_id": target.cluster_id,
                "namespace": target.namespace,
                "pod": target.pod_name,
                "container": target.container_name,
                "privileged": target.privileged,
                "service_account_name": target.service_account_name,
                "recommendation": "Restrict exec access or require break-glass approval for privileged containers",
            }),
        ));
    }

    if target.host_network || target.host_pid || target.host_ipc {
        findings.push(finding(
            target,
            pillar,
            REASON_SEC_HOST_NAMESPACE_EXEC_TARGET,
            Severity::High,
            format!(
                "Kubernetes Pod Exec target {}/{}/{} shares host namespaces",
                target.namespace, target.pod_name, target.container_name
            ),
            json!({
                "cluster_id": target.cluster_id,
                "namespace": target.namespace,
                "pod": target.pod_name,
                "container": target.container_name,
                "host_network": target.host_network,
                "host_pid": target.host_pid,
                "host_ipc": target.host_ipc,
                "node_name": target.node_name,
            }),
        ));
    }

    if target.automount_service_account_token != Some(false) {
        findings.push(finding(
            target,
            pillar,
            REASON_SEC_AUTOMOUNTED_TOKEN,
            Severity::Medium,
            format!(
                "Kubernetes Pod Exec target {}/{}/{} may expose a mounted service account token during interactive sessions",
                target.namespace, target.pod_name, target.container_name
            ),
            json!({
                "cluster_id": target.cluster_id,
                "namespace": target.namespace,
                "pod": target.pod_name,
                "container": target.container_name,
                "service_account_name": target.service_account_name,
                "automount_service_account_token": target.automount_service_account_token,
                "recommendation": "Set automountServiceAccountToken=false where API credentials are not required",
            }),
        ));
    }
}

fn stale_finding(
    target: &PodExecInventoryItem,
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
            "Inventory data for Kubernetes Pod Exec target {}/{}/{} is {} hours old (threshold {} hours)",
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
    target: &PodExecInventoryItem,
    pillar: Pillar,
    reason_code: &str,
    severity: Severity,
    message: String,
    evidence: Value,
) -> InventoryFinding {
    InventoryFinding {
        resource_id: format!(
            "{}/{}/PodExec/{}/{}",
            target.cluster_id, target.namespace, target.pod_name, target.container_name
        ),
        arn: format!(
            "kubernetes://podexec/{}/{}/{}/{}",
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

    fn healthy_exec_target() -> PodExecInventoryItem {
        PodExecInventoryItem {
            cluster_id: "cluster-a".to_string(),
            namespace: "apps".to_string(),
            pod_name: "pod-a".to_string(),
            container_name: "app".to_string(),
            image: Some("registry.local/app:1.0.0".to_string()),
            phase: Some("Running".to_string()),
            ready: Some(true),
            restart_count: 0,
            labels: labels(&[("owner", "platform"), ("cost-center", "cc-42")]),
            annotations: BTreeMap::new(),
            node_name: Some("node-a".to_string()),
            controlled_by: Some("pod-a-rs".to_string()),
            controller_kind: Some("ReplicaSet".to_string()),
            service_account_name: Some("app".to_string()),
            automount_service_account_token: Some(false),
            privileged: Some(false),
            host_network: false,
            host_pid: false,
            host_ipc: false,
            stdin: Some(false),
            tty: Some(false),
            created_at: Some(now() - Duration::hours(2)),
            collected_at: now(),
        }
    }

    #[test]
    fn cost_flags_high_restart_debug_churn() {
        let mut churn = healthy_exec_target();
        churn.restart_count = 12;

        let report = evaluate_kubernetes_pod_exec_inventory(&[churn], Pillar::Cost, now());

        assert_eq!(report.resources_evaluated, 1);
        assert_eq!(report.findings.len(), 1);
        assert_eq!(
            report.findings[0].reason_code,
            REASON_COST_HIGH_RESTART_DEBUG_CHURN
        );
        assert_eq!(report.findings[0].pillar, Pillar::Cost);
    }

    #[test]
    fn resilience_flags_unready_exec_targets() {
        let mut unready = healthy_exec_target();
        unready.ready = Some(false);

        let report = evaluate_kubernetes_pod_exec_inventory(&[unready], Pillar::Resilience, now());

        assert_eq!(report.findings.len(), 1);
        assert_eq!(
            report.findings[0].reason_code,
            REASON_RES_CONTAINER_NOT_READY
        );
    }

    #[test]
    fn security_flags_privileged_exec_targets() {
        let mut privileged = healthy_exec_target();
        privileged.privileged = Some(true);

        let report = evaluate_kubernetes_pod_exec_inventory(&[privileged], Pillar::Security, now());

        assert_eq!(report.findings.len(), 1);
        assert_eq!(
            report.findings[0].reason_code,
            REASON_SEC_PRIVILEGED_EXEC_TARGET
        );
    }

    #[test]
    fn stale_inventory_is_counted_for_any_pillar() {
        let mut stale = healthy_exec_target();
        stale.collected_at = now() - Duration::hours(DEFAULT_STALE_AFTER_HOURS + 2);

        let report = evaluate_kubernetes_pod_exec_inventory(&[stale], Pillar::Cost, now());

        assert_eq!(report.stale_resources, 1);
        assert_eq!(report.findings[0].reason_code, REASON_INV_STALE_DATA);
    }

    #[test]
    fn healthy_exec_targets_pass_claimed_pillars() {
        for pillar in [Pillar::Cost, Pillar::Resilience, Pillar::Security] {
            let report =
                evaluate_kubernetes_pod_exec_inventory(&[healthy_exec_target()], pillar, now());

            assert_eq!(report.resources_evaluated, 1);
            assert_eq!(report.stale_resources, 0);
            assert!(report.findings.is_empty());
        }
    }
}

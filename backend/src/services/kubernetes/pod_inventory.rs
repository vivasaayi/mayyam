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

// Deterministic Kubernetes pod inventory evaluator for roadmap rows
// 02-KUBERNETES-DASHBOARD-00148/00155/00176.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::services::aws::inventory::types::{
    score_pillar, InventoryFinding, Pillar, PillarReport, Severity, COST_ALLOCATION_TAG_KEYS,
    DEFAULT_STALE_AFTER_HOURS,
};

pub const RESOURCE_TYPE: &str = "KubernetesPod";
pub const REASON_COST_OWNER_NOT_RECORDED: &str = "K8S_POD_COST_OWNER_NOT_RECORDED";
pub const REASON_RES_MISSING_PHASE: &str = "K8S_POD_RES_MISSING_PHASE";
pub const REASON_RES_UNHEALTHY_PHASE: &str = "K8S_POD_RES_UNHEALTHY_PHASE";
pub const REASON_RES_CONTAINERS_NOT_READY: &str = "K8S_POD_RES_CONTAINERS_NOT_READY";
pub const REASON_SEC_PRIVILEGED_CONTAINER: &str = "K8S_POD_SEC_PRIVILEGED_CONTAINER";
pub const REASON_SEC_HOST_NETWORK: &str = "K8S_POD_SEC_HOST_NETWORK";
pub const REASON_INV_STALE_DATA: &str = "K8S_POD_INV_STALE_DATA";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PodContainerInventoryItem {
    pub name: String,
    pub image: Option<String>,
    pub ready: Option<bool>,
    pub restart_count: i32,
    pub privileged: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PodInventoryItem {
    pub cluster_id: String,
    pub namespace: String,
    pub name: String,
    pub phase: Option<String>,
    pub pod_ip: Option<String>,
    pub node_name: Option<String>,
    pub labels: BTreeMap<String, String>,
    pub annotations: BTreeMap<String, String>,
    pub containers: Vec<PodContainerInventoryItem>,
    pub restart_count: i32,
    pub controlled_by: Option<String>,
    pub controller_kind: Option<String>,
    pub qos_class: Option<String>,
    pub service_account_name: Option<String>,
    pub host_network: bool,
    pub created_at: Option<DateTime<Utc>>,
    pub collected_at: DateTime<Utc>,
}

pub fn evaluate_kubernetes_pod_inventory(
    pods: &[PodInventoryItem],
    pillar: Pillar,
    now: DateTime<Utc>,
) -> PillarReport {
    let mut stale_resources = 0;
    let mut findings = Vec::new();

    for pod in pods {
        if let Some(finding) = stale_finding(pod, pillar, now) {
            stale_resources += 1;
            findings.push(finding);
        }

        match pillar {
            Pillar::Cost => evaluate_cost(pod, pillar, &mut findings),
            Pillar::Resilience => evaluate_resilience(pod, pillar, &mut findings),
            Pillar::Security => evaluate_security(pod, pillar, &mut findings),
            _ => {}
        }
    }

    PillarReport {
        pillar,
        resources_evaluated: pods.len(),
        stale_resources,
        score: score_pillar(&findings),
        findings,
    }
}

fn evaluate_cost(pod: &PodInventoryItem, pillar: Pillar, findings: &mut Vec<InventoryFinding>) {
    if has_any_metadata_key(&pod.labels, COST_ALLOCATION_TAG_KEYS)
        || has_any_metadata_key(&pod.annotations, COST_ALLOCATION_TAG_KEYS)
    {
        return;
    }

    findings.push(finding(
        pod,
        pillar,
        REASON_COST_OWNER_NOT_RECORDED,
        Severity::Medium,
        format!(
            "Kubernetes pod {}/{} has no owner, team, project, or cost-center label or annotation",
            pod.namespace, pod.name
        ),
        json!({
            "cluster_id": pod.cluster_id,
            "namespace": pod.namespace,
            "pod": pod.name,
            "controller_kind": pod.controller_kind,
            "controlled_by": pod.controlled_by,
            "qos_class": pod.qos_class,
            "checked_keys": COST_ALLOCATION_TAG_KEYS,
            "checked_locations": ["labels", "annotations"],
        }),
    ));
}

fn evaluate_resilience(
    pod: &PodInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    let Some(phase) = pod
        .phase
        .as_deref()
        .map(str::trim)
        .filter(|phase| !phase.is_empty())
    else {
        findings.push(finding(
            pod,
            pillar,
            REASON_RES_MISSING_PHASE,
            Severity::Medium,
            format!(
                "Kubernetes pod {}/{} has no collected lifecycle phase",
                pod.namespace, pod.name
            ),
            json!({
                "cluster_id": pod.cluster_id,
                "namespace": pod.namespace,
                "pod": pod.name,
                "phase": pod.phase,
            }),
        ));
        return;
    };

    let healthy_terminal_or_running =
        phase.eq_ignore_ascii_case("Running") || phase.eq_ignore_ascii_case("Succeeded");
    if !healthy_terminal_or_running {
        findings.push(finding(
            pod,
            pillar,
            REASON_RES_UNHEALTHY_PHASE,
            Severity::High,
            format!(
                "Kubernetes pod {}/{} phase is {}; expected Running or Succeeded",
                pod.namespace, pod.name, phase
            ),
            json!({
                "cluster_id": pod.cluster_id,
                "namespace": pod.namespace,
                "pod": pod.name,
                "phase": phase,
                "restart_count": pod.restart_count,
                "node_name": pod.node_name,
            }),
        ));
    }

    if phase.eq_ignore_ascii_case("Running") {
        let total_containers = pod.containers.len();
        let ready_containers = pod
            .containers
            .iter()
            .filter(|container| container.ready == Some(true))
            .count();
        if ready_containers < total_containers {
            findings.push(finding(
                pod,
                pillar,
                REASON_RES_CONTAINERS_NOT_READY,
                Severity::High,
                format!(
                    "Kubernetes pod {}/{} has {}/{} ready containers",
                    pod.namespace, pod.name, ready_containers, total_containers
                ),
                json!({
                    "cluster_id": pod.cluster_id,
                    "namespace": pod.namespace,
                    "pod": pod.name,
                    "phase": phase,
                    "ready_containers": ready_containers,
                    "total_containers": total_containers,
                    "containers": pod.containers,
                }),
            ));
        }
    }
}

fn evaluate_security(pod: &PodInventoryItem, pillar: Pillar, findings: &mut Vec<InventoryFinding>) {
    let privileged_containers = pod
        .containers
        .iter()
        .filter(|container| container.privileged == Some(true))
        .map(|container| container.name.clone())
        .collect::<Vec<_>>();
    if !privileged_containers.is_empty() {
        findings.push(finding(
            pod,
            pillar,
            REASON_SEC_PRIVILEGED_CONTAINER,
            Severity::High,
            format!(
                "Kubernetes pod {}/{} has privileged containers",
                pod.namespace, pod.name
            ),
            json!({
                "cluster_id": pod.cluster_id,
                "namespace": pod.namespace,
                "pod": pod.name,
                "privileged_containers": privileged_containers,
                "service_account_name": pod.service_account_name,
            }),
        ));
    }

    if pod.host_network {
        findings.push(finding(
            pod,
            pillar,
            REASON_SEC_HOST_NETWORK,
            Severity::High,
            format!(
                "Kubernetes pod {}/{} runs with hostNetwork enabled",
                pod.namespace, pod.name
            ),
            json!({
                "cluster_id": pod.cluster_id,
                "namespace": pod.namespace,
                "pod": pod.name,
                "host_network": pod.host_network,
                "node_name": pod.node_name,
            }),
        ));
    }
}

fn stale_finding(
    pod: &PodInventoryItem,
    pillar: Pillar,
    now: DateTime<Utc>,
) -> Option<InventoryFinding> {
    let age_hours = (now - pod.collected_at).num_hours();
    if age_hours <= DEFAULT_STALE_AFTER_HOURS {
        return None;
    }

    Some(finding(
        pod,
        pillar,
        REASON_INV_STALE_DATA,
        Severity::Medium,
        format!(
            "Inventory data for Kubernetes pod {}/{} is {} hours old (threshold {} hours)",
            pod.namespace, pod.name, age_hours, DEFAULT_STALE_AFTER_HOURS
        ),
        json!({
            "cluster_id": pod.cluster_id,
            "namespace": pod.namespace,
            "pod": pod.name,
            "collected_at": pod.collected_at,
            "age_hours": age_hours,
            "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
        }),
    ))
}

fn finding(
    pod: &PodInventoryItem,
    pillar: Pillar,
    reason_code: &str,
    severity: Severity,
    message: String,
    evidence: Value,
) -> InventoryFinding {
    InventoryFinding {
        resource_id: format!("{}/{}/{}", pod.cluster_id, pod.namespace, pod.name),
        arn: format!(
            "kubernetes://pod/{}/{}/{}",
            pod.cluster_id, pod.namespace, pod.name
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

    fn container(
        name: &str,
        ready: Option<bool>,
        privileged: Option<bool>,
    ) -> PodContainerInventoryItem {
        PodContainerInventoryItem {
            name: name.to_string(),
            image: Some("registry.local/app:1.0.0".to_string()),
            ready,
            restart_count: 0,
            privileged,
        }
    }

    fn pod(
        name: &str,
        phase: Option<&str>,
        labels: BTreeMap<String, String>,
        containers: Vec<PodContainerInventoryItem>,
        host_network: bool,
        collected_at: DateTime<Utc>,
    ) -> PodInventoryItem {
        PodInventoryItem {
            cluster_id: "cluster-1".to_string(),
            namespace: "payments".to_string(),
            name: name.to_string(),
            phase: phase.map(str::to_string),
            pod_ip: Some("10.244.0.42".to_string()),
            node_name: Some("ip-10-0-0-10".to_string()),
            labels,
            annotations: BTreeMap::new(),
            restart_count: containers
                .iter()
                .map(|container| container.restart_count)
                .sum(),
            containers,
            controlled_by: Some("payments".to_string()),
            controller_kind: Some("ReplicaSet".to_string()),
            qos_class: Some("Burstable".to_string()),
            service_account_name: Some("payments".to_string()),
            host_network,
            created_at: Some(now() - Duration::hours(2)),
            collected_at,
        }
    }

    fn healthy_pod() -> PodInventoryItem {
        pod(
            "payments-6f5c9d",
            Some("Running"),
            labels(&[("owner", "platform"), ("cost-center", "cc-42")]),
            vec![container("app", Some(true), Some(false))],
            false,
            now(),
        )
    }

    fn reason_codes(report: &PillarReport) -> Vec<&str> {
        report
            .findings
            .iter()
            .map(|finding| finding.reason_code.as_str())
            .collect()
    }

    #[test]
    fn cost_flags_missing_owner_and_cost_allocation_labels() {
        let report = evaluate_kubernetes_pod_inventory(
            &[pod(
                "unowned",
                Some("Running"),
                BTreeMap::new(),
                vec![container("app", Some(true), Some(false))],
                false,
                now(),
            )],
            Pillar::Cost,
            now(),
        );

        assert_eq!(report.resources_evaluated, 1);
        assert!(reason_codes(&report).contains(&REASON_COST_OWNER_NOT_RECORDED));
    }

    #[test]
    fn resilience_flags_missing_phase_unhealthy_phase_and_unready_containers() {
        let missing_phase = pod(
            "unknown",
            None,
            labels(&[("owner", "platform")]),
            vec![container("app", Some(true), Some(false))],
            false,
            now(),
        );
        let failed = pod(
            "failed",
            Some("Failed"),
            labels(&[("owner", "platform")]),
            vec![container("app", Some(true), Some(false))],
            false,
            now(),
        );
        let unready = pod(
            "unready",
            Some("Running"),
            labels(&[("owner", "platform")]),
            vec![
                container("app", Some(true), Some(false)),
                container("sidecar", Some(false), Some(false)),
            ],
            false,
            now(),
        );

        let report = evaluate_kubernetes_pod_inventory(
            &[missing_phase, failed, unready],
            Pillar::Resilience,
            now(),
        );
        let codes = reason_codes(&report);

        assert!(codes.contains(&REASON_RES_MISSING_PHASE));
        assert!(codes.contains(&REASON_RES_UNHEALTHY_PHASE));
        assert!(codes.contains(&REASON_RES_CONTAINERS_NOT_READY));
    }

    #[test]
    fn security_flags_privileged_containers_and_host_network() {
        let exposed = pod(
            "privileged",
            Some("Running"),
            labels(&[("owner", "platform")]),
            vec![container("app", Some(true), Some(true))],
            true,
            now(),
        );

        let report = evaluate_kubernetes_pod_inventory(&[exposed], Pillar::Security, now());
        let codes = reason_codes(&report);

        assert!(codes.contains(&REASON_SEC_PRIVILEGED_CONTAINER));
        assert!(codes.contains(&REASON_SEC_HOST_NETWORK));
    }

    #[test]
    fn stale_inventory_is_counted_for_any_pillar() {
        let stale = pod(
            "stale",
            Some("Running"),
            labels(&[("owner", "platform"), ("cost-center", "cc-42")]),
            vec![container("app", Some(true), Some(false))],
            false,
            now() - Duration::hours(DEFAULT_STALE_AFTER_HOURS + 2),
        );

        let report = evaluate_kubernetes_pod_inventory(&[stale], Pillar::Cost, now());
        assert_eq!(report.stale_resources, 1);
        assert!(reason_codes(&report).contains(&REASON_INV_STALE_DATA));
    }

    #[test]
    fn healthy_pod_passes_claimed_pillars() {
        let pod = healthy_pod();

        for pillar in [Pillar::Cost, Pillar::Resilience, Pillar::Security] {
            let report =
                evaluate_kubernetes_pod_inventory(std::slice::from_ref(&pod), pillar, now());
            assert!(
                report.findings.is_empty(),
                "unexpected for {:?}: {:?}",
                pillar,
                report.findings
            );
            assert_eq!(report.score, 100);
        }
    }
}

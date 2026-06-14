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

// Deterministic Kubernetes Node Drains inventory evaluator for roadmap rows
// 02-KUBERNETES-DASHBOARD-01961/01968/01989.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::services::aws::inventory::types::{
    score_pillar, InventoryFinding, Pillar, PillarReport, Severity, COST_ALLOCATION_TAG_KEYS,
    DEFAULT_STALE_AFTER_HOURS,
};

pub const RESOURCE_TYPE: &str = "KubernetesNodeDrain";
pub const REASON_COST_OWNER_NOT_RECORDED: &str = "K8S_NODE_DRAIN_COST_OWNER_NOT_RECORDED";
pub const REASON_COST_UNSCHEDULABLE_CAPACITY: &str = "K8S_NODE_DRAIN_COST_UNSCHEDULABLE_CAPACITY";
pub const REASON_RES_ACTIVE_DRAIN: &str = "K8S_NODE_DRAIN_RES_ACTIVE_DRAIN";
pub const REASON_RES_NODE_NOT_READY_DURING_DRAIN: &str =
    "K8S_NODE_DRAIN_RES_NODE_NOT_READY_DURING_DRAIN";
pub const REASON_SEC_CONTROL_PLANE_DRAIN: &str = "K8S_NODE_DRAIN_SEC_CONTROL_PLANE_DRAIN";
pub const REASON_INV_STALE_DATA: &str = "K8S_NODE_DRAIN_INV_STALE_DATA";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeDrainInventoryItem {
    pub cluster_id: String,
    pub node_name: String,
    pub node_ready_status: Option<String>,
    pub node_unschedulable: bool,
    pub no_schedule_taints: usize,
    pub no_execute_taints: usize,
    pub taint_keys: Vec<String>,
    pub roles: Vec<String>,
    pub labels: BTreeMap<String, String>,
    pub annotations: BTreeMap<String, String>,
    pub created_at: Option<DateTime<Utc>>,
    pub collected_at: DateTime<Utc>,
}

pub fn evaluate_kubernetes_node_drains_inventory(
    drains: &[NodeDrainInventoryItem],
    pillar: Pillar,
    now: DateTime<Utc>,
) -> PillarReport {
    let mut stale_resources = 0;
    let mut findings = Vec::new();

    for drain in drains {
        if let Some(finding) = stale_finding(drain, pillar, now) {
            stale_resources += 1;
            findings.push(finding);
        }

        match pillar {
            Pillar::Cost => evaluate_cost(drain, pillar, &mut findings),
            Pillar::Resilience => evaluate_resilience(drain, pillar, &mut findings),
            Pillar::Security => evaluate_security(drain, pillar, &mut findings),
            _ => {}
        }
    }

    PillarReport {
        pillar,
        resources_evaluated: drains.len(),
        stale_resources,
        score: score_pillar(&findings),
        findings,
    }
}

fn evaluate_cost(
    drain: &NodeDrainInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if !has_any_metadata_key(&drain.labels, COST_ALLOCATION_TAG_KEYS)
        && !has_any_metadata_key(&drain.annotations, COST_ALLOCATION_TAG_KEYS)
    {
        findings.push(finding(
            drain,
            pillar,
            REASON_COST_OWNER_NOT_RECORDED,
            Severity::Medium,
            format!(
                "Kubernetes node drain state for node {} has no owner, team, project, or cost-center label or annotation",
                drain.node_name
            ),
            json!({
                "cluster_id": drain.cluster_id,
                "node": drain.node_name,
                "roles": drain.roles,
                "checked_keys": COST_ALLOCATION_TAG_KEYS,
                "checked_locations": ["labels", "annotations"],
                "node_unschedulable": drain.node_unschedulable,
                "taint_keys": drain.taint_keys,
            }),
        ));
    }

    if drain.node_unschedulable && is_ready(drain) {
        findings.push(finding(
            drain,
            pillar,
            REASON_COST_UNSCHEDULABLE_CAPACITY,
            Severity::Medium,
            format!(
                "Kubernetes node {} is Ready but cordoned or draining, leaving allocatable capacity unscheduled",
                drain.node_name
            ),
            json!({
                "cluster_id": drain.cluster_id,
                "node": drain.node_name,
                "node_ready_status": drain.node_ready_status,
                "node_unschedulable": drain.node_unschedulable,
                "no_schedule_taints": drain.no_schedule_taints,
                "no_execute_taints": drain.no_execute_taints,
                "recommendation": "Complete the drain or uncordon the node after maintenance to avoid idle capacity",
            }),
        ));
    }
}

fn evaluate_resilience(
    drain: &NodeDrainInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if !is_active_drain(drain) {
        return;
    }

    findings.push(finding(
        drain,
        pillar,
        REASON_RES_ACTIVE_DRAIN,
        Severity::Medium,
        format!(
            "Kubernetes node {} is currently cordoned or has drain taints",
            drain.node_name
        ),
        json!({
            "cluster_id": drain.cluster_id,
            "node": drain.node_name,
            "node_unschedulable": drain.node_unschedulable,
            "no_schedule_taints": drain.no_schedule_taints,
            "no_execute_taints": drain.no_execute_taints,
            "taint_keys": drain.taint_keys,
            "roles": drain.roles,
        }),
    ));

    if !is_ready(drain) {
        findings.push(finding(
            drain,
            pillar,
            REASON_RES_NODE_NOT_READY_DURING_DRAIN,
            Severity::High,
            format!(
                "Kubernetes node {} is draining and its Ready condition is not healthy",
                drain.node_name
            ),
            json!({
                "cluster_id": drain.cluster_id,
                "node": drain.node_name,
                "node_ready_status": drain.node_ready_status,
                "node_unschedulable": drain.node_unschedulable,
                "taint_keys": drain.taint_keys,
                "recommendation": "Confirm workloads were rescheduled and investigate node health before returning it to service",
            }),
        ));
    }
}

fn evaluate_security(
    drain: &NodeDrainInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if !is_active_drain(drain) || !is_control_plane_node(drain) {
        return;
    }

    findings.push(finding(
        drain,
        pillar,
        REASON_SEC_CONTROL_PLANE_DRAIN,
        Severity::High,
        format!(
            "Kubernetes control plane node {} is cordoned or draining",
            drain.node_name
        ),
        json!({
            "cluster_id": drain.cluster_id,
            "node": drain.node_name,
            "roles": drain.roles,
            "node_unschedulable": drain.node_unschedulable,
            "no_schedule_taints": drain.no_schedule_taints,
            "no_execute_taints": drain.no_execute_taints,
            "recommendation": "Treat control-plane drains as privileged maintenance and verify quorum, break-glass approval, and rollback evidence",
        }),
    ));
}

fn stale_finding(
    drain: &NodeDrainInventoryItem,
    pillar: Pillar,
    now: DateTime<Utc>,
) -> Option<InventoryFinding> {
    let age_hours = (now - drain.collected_at).num_hours();
    if age_hours <= DEFAULT_STALE_AFTER_HOURS {
        return None;
    }

    Some(finding(
        drain,
        pillar,
        REASON_INV_STALE_DATA,
        Severity::Medium,
        format!(
            "Inventory data for Kubernetes node drain state on node {} is {} hours old (threshold {} hours)",
            drain.node_name, age_hours, DEFAULT_STALE_AFTER_HOURS
        ),
        json!({
            "cluster_id": drain.cluster_id,
            "node": drain.node_name,
            "collected_at": drain.collected_at,
            "age_hours": age_hours,
            "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
        }),
    ))
}

fn finding(
    drain: &NodeDrainInventoryItem,
    pillar: Pillar,
    reason_code: &str,
    severity: Severity,
    message: String,
    evidence: Value,
) -> InventoryFinding {
    InventoryFinding {
        resource_id: format!("{}/{}", drain.cluster_id, drain.node_name),
        arn: format!(
            "kubernetes://node-drain/{}/{}",
            drain.cluster_id, drain.node_name
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

fn is_active_drain(drain: &NodeDrainInventoryItem) -> bool {
    drain.node_unschedulable || drain.no_schedule_taints > 0 || drain.no_execute_taints > 0
}

fn is_ready(drain: &NodeDrainInventoryItem) -> bool {
    drain
        .node_ready_status
        .as_deref()
        .map(str::trim)
        .filter(|status| !status.is_empty())
        .map(|status| status.eq_ignore_ascii_case("Ready"))
        .unwrap_or(false)
}

fn is_control_plane_node(drain: &NodeDrainInventoryItem) -> bool {
    drain.roles.iter().any(|role| {
        role.eq_ignore_ascii_case("control-plane") || role.eq_ignore_ascii_case("master")
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::aws::inventory::types::Pillar;
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

    fn drain(
        node_name: &str,
        ready_status: Option<&str>,
        unschedulable: bool,
        roles: Vec<&str>,
        labels: BTreeMap<String, String>,
        collected_at: DateTime<Utc>,
    ) -> NodeDrainInventoryItem {
        NodeDrainInventoryItem {
            cluster_id: "cluster-1".to_string(),
            node_name: node_name.to_string(),
            node_ready_status: ready_status.map(str::to_string),
            node_unschedulable: unschedulable,
            no_schedule_taints: usize::from(unschedulable),
            no_execute_taints: 0,
            taint_keys: if unschedulable {
                vec!["node.kubernetes.io/unschedulable:NoSchedule".to_string()]
            } else {
                Vec::new()
            },
            roles: roles.into_iter().map(str::to_string).collect(),
            labels,
            annotations: BTreeMap::new(),
            created_at: Some(now() - Duration::days(10)),
            collected_at,
        }
    }

    fn healthy_drain_state() -> NodeDrainInventoryItem {
        drain(
            "ip-10-0-0-10",
            Some("Ready"),
            false,
            vec!["worker"],
            labels(&[("owner", "platform"), ("cost-center", "cc-42")]),
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
    fn cost_flags_missing_owner_and_unschedulable_capacity() {
        let item = drain(
            "unowned-drain",
            Some("Ready"),
            true,
            vec!["worker"],
            BTreeMap::new(),
            now(),
        );

        let report = evaluate_kubernetes_node_drains_inventory(&[item], Pillar::Cost, now());
        let codes = reason_codes(&report);

        assert!(codes.contains(&REASON_COST_OWNER_NOT_RECORDED));
        assert!(codes.contains(&REASON_COST_UNSCHEDULABLE_CAPACITY));
    }

    #[test]
    fn resilience_flags_active_and_unhealthy_drains() {
        let active = drain(
            "active-drain",
            Some("Ready"),
            true,
            vec!["worker"],
            labels(&[("owner", "platform")]),
            now(),
        );
        let unhealthy = drain(
            "unhealthy-drain",
            Some("NotReady (KubeletNotReady)"),
            true,
            vec!["worker"],
            labels(&[("owner", "platform")]),
            now(),
        );

        let report = evaluate_kubernetes_node_drains_inventory(
            &[active, unhealthy],
            Pillar::Resilience,
            now(),
        );
        let codes = reason_codes(&report);

        assert!(codes.contains(&REASON_RES_ACTIVE_DRAIN));
        assert!(codes.contains(&REASON_RES_NODE_NOT_READY_DURING_DRAIN));
    }

    #[test]
    fn security_flags_control_plane_drains() {
        let item = drain(
            "control-plane-drain",
            Some("Ready"),
            true,
            vec!["control-plane"],
            labels(&[("owner", "platform")]),
            now(),
        );

        let report = evaluate_kubernetes_node_drains_inventory(&[item], Pillar::Security, now());

        assert!(reason_codes(&report).contains(&REASON_SEC_CONTROL_PLANE_DRAIN));
    }

    #[test]
    fn stale_inventory_is_counted_for_any_pillar() {
        let item = drain(
            "stale",
            Some("Ready"),
            false,
            vec!["worker"],
            labels(&[("owner", "platform")]),
            now() - Duration::hours(25),
        );

        let report = evaluate_kubernetes_node_drains_inventory(&[item], Pillar::Cost, now());

        assert_eq!(report.stale_resources, 1);
        assert!(reason_codes(&report).contains(&REASON_INV_STALE_DATA));
    }

    #[test]
    fn healthy_drain_state_passes_claimed_pillars() {
        let item = healthy_drain_state();

        for pillar in [Pillar::Cost, Pillar::Resilience, Pillar::Security] {
            let report = evaluate_kubernetes_node_drains_inventory(
                std::slice::from_ref(&item),
                pillar,
                now(),
            );
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

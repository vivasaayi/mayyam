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

// Deterministic Kubernetes node inventory evaluator for roadmap rows
// 02-KUBERNETES-DASHBOARD-00099/00106/00127.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::services::aws::inventory::types::{
    score_pillar, InventoryFinding, Pillar, PillarReport, Severity, COST_ALLOCATION_TAG_KEYS,
    DEFAULT_STALE_AFTER_HOURS,
};

pub const RESOURCE_TYPE: &str = "KubernetesNode";
pub const REASON_COST_OWNER_NOT_RECORDED: &str = "K8S_NODE_COST_OWNER_NOT_RECORDED";
pub const REASON_RES_MISSING_READY_STATUS: &str = "K8S_NODE_RES_MISSING_READY_STATUS";
pub const REASON_RES_NOT_READY: &str = "K8S_NODE_RES_NOT_READY";
pub const REASON_SEC_EXTERNAL_IP_EXPOSED: &str = "K8S_NODE_SEC_EXTERNAL_IP_EXPOSED";
pub const REASON_INV_STALE_DATA: &str = "K8S_NODE_INV_STALE_DATA";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeInventoryItem {
    pub cluster_id: String,
    pub name: String,
    pub ready_status: Option<String>,
    pub roles: Vec<String>,
    pub labels: BTreeMap<String, String>,
    pub annotations: BTreeMap<String, String>,
    pub internal_ip: Option<String>,
    pub external_ip: Option<String>,
    pub kubelet_version: Option<String>,
    pub os_image: Option<String>,
    pub kernel_version: Option<String>,
    pub container_runtime_version: Option<String>,
    pub created_at: Option<DateTime<Utc>>,
    pub collected_at: DateTime<Utc>,
}

pub fn evaluate_kubernetes_node_inventory(
    nodes: &[NodeInventoryItem],
    pillar: Pillar,
    now: DateTime<Utc>,
) -> PillarReport {
    let mut stale_resources = 0;
    let mut findings = Vec::new();

    for node in nodes {
        if let Some(finding) = stale_finding(node, pillar, now) {
            stale_resources += 1;
            findings.push(finding);
        }

        match pillar {
            Pillar::Cost => evaluate_cost(node, pillar, &mut findings),
            Pillar::Resilience => evaluate_resilience(node, pillar, &mut findings),
            Pillar::Security => evaluate_security(node, pillar, &mut findings),
            _ => {}
        }
    }

    PillarReport {
        pillar,
        resources_evaluated: nodes.len(),
        stale_resources,
        score: score_pillar(&findings),
        findings,
    }
}

fn evaluate_cost(node: &NodeInventoryItem, pillar: Pillar, findings: &mut Vec<InventoryFinding>) {
    if has_any_metadata_key(&node.labels, COST_ALLOCATION_TAG_KEYS)
        || has_any_metadata_key(&node.annotations, COST_ALLOCATION_TAG_KEYS)
    {
        return;
    }

    findings.push(finding(
        node,
        pillar,
        REASON_COST_OWNER_NOT_RECORDED,
        Severity::Medium,
        format!(
            "Kubernetes node {} has no owner, team, project, or cost-center label or annotation",
            node.name
        ),
        json!({
            "cluster_id": node.cluster_id,
            "node": node.name,
            "roles": node.roles,
            "checked_keys": COST_ALLOCATION_TAG_KEYS,
            "checked_locations": ["labels", "annotations"],
        }),
    ));
}

fn evaluate_resilience(
    node: &NodeInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    let Some(status) = node
        .ready_status
        .as_deref()
        .map(str::trim)
        .filter(|status| !status.is_empty())
    else {
        findings.push(finding(
            node,
            pillar,
            REASON_RES_MISSING_READY_STATUS,
            Severity::Medium,
            format!(
                "Kubernetes node {} has no collected Ready condition",
                node.name
            ),
            json!({
                "cluster_id": node.cluster_id,
                "node": node.name,
                "ready_status": node.ready_status,
            }),
        ));
        return;
    };

    if !status.eq_ignore_ascii_case("Ready") {
        findings.push(finding(
            node,
            pillar,
            REASON_RES_NOT_READY,
            Severity::High,
            format!(
                "Kubernetes node {} Ready condition is {}; expected Ready",
                node.name, status
            ),
            json!({
                "cluster_id": node.cluster_id,
                "node": node.name,
                "ready_status": status,
            }),
        ));
    }
}

fn evaluate_security(
    node: &NodeInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    let Some(external_ip) = node
        .external_ip
        .as_deref()
        .filter(|ip| !ip.trim().is_empty())
    else {
        return;
    };

    findings.push(finding(
        node,
        pillar,
        REASON_SEC_EXTERNAL_IP_EXPOSED,
        Severity::High,
        format!(
            "Kubernetes node {} advertises an external IP address",
            node.name
        ),
        json!({
            "cluster_id": node.cluster_id,
            "node": node.name,
            "external_ip": external_ip,
            "internal_ip": node.internal_ip,
        }),
    ));
}

fn stale_finding(
    node: &NodeInventoryItem,
    pillar: Pillar,
    now: DateTime<Utc>,
) -> Option<InventoryFinding> {
    let age_hours = (now - node.collected_at).num_hours();
    if age_hours <= DEFAULT_STALE_AFTER_HOURS {
        return None;
    }

    Some(finding(
        node,
        pillar,
        REASON_INV_STALE_DATA,
        Severity::Medium,
        format!(
            "Inventory data for Kubernetes node {} is {} hours old (threshold {} hours)",
            node.name, age_hours, DEFAULT_STALE_AFTER_HOURS
        ),
        json!({
            "cluster_id": node.cluster_id,
            "node": node.name,
            "collected_at": node.collected_at,
            "age_hours": age_hours,
            "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
        }),
    ))
}

fn finding(
    node: &NodeInventoryItem,
    pillar: Pillar,
    reason_code: &str,
    severity: Severity,
    message: String,
    evidence: Value,
) -> InventoryFinding {
    InventoryFinding {
        resource_id: format!("{}/{}", node.cluster_id, node.name),
        arn: format!("kubernetes://node/{}/{}", node.cluster_id, node.name),
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

    fn node(
        name: &str,
        ready_status: Option<&str>,
        labels: BTreeMap<String, String>,
        external_ip: Option<&str>,
        collected_at: DateTime<Utc>,
    ) -> NodeInventoryItem {
        NodeInventoryItem {
            cluster_id: "cluster-1".to_string(),
            name: name.to_string(),
            ready_status: ready_status.map(str::to_string),
            roles: vec!["worker".to_string()],
            labels,
            annotations: BTreeMap::new(),
            internal_ip: Some("10.0.0.10".to_string()),
            external_ip: external_ip.map(str::to_string),
            kubelet_version: Some("v1.30.0".to_string()),
            os_image: Some("Ubuntu 24.04 LTS".to_string()),
            kernel_version: Some("6.8.0".to_string()),
            container_runtime_version: Some("containerd://1.7.0".to_string()),
            created_at: Some(now() - Duration::days(10)),
            collected_at,
        }
    }

    fn healthy_node() -> NodeInventoryItem {
        node(
            "ip-10-0-0-10",
            Some("Ready"),
            labels(&[("owner", "platform"), ("cost-center", "cc-42")]),
            None,
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
        let report = evaluate_kubernetes_node_inventory(
            &[node("unowned", Some("Ready"), BTreeMap::new(), None, now())],
            Pillar::Cost,
            now(),
        );

        assert_eq!(report.resources_evaluated, 1);
        assert!(reason_codes(&report).contains(&REASON_COST_OWNER_NOT_RECORDED));
    }

    #[test]
    fn resilience_flags_missing_and_not_ready_status() {
        let missing = node(
            "unknown",
            None,
            labels(&[("owner", "platform")]),
            None,
            now(),
        );
        let not_ready = node(
            "not-ready",
            Some("NotReady (KubeletNotReady)"),
            labels(&[("owner", "platform")]),
            None,
            now(),
        );

        let report =
            evaluate_kubernetes_node_inventory(&[missing, not_ready], Pillar::Resilience, now());
        let codes = reason_codes(&report);

        assert!(codes.contains(&REASON_RES_MISSING_READY_STATUS));
        assert!(codes.contains(&REASON_RES_NOT_READY));
    }

    #[test]
    fn security_flags_external_node_ip() {
        let exposed = node(
            "public-node",
            Some("Ready"),
            labels(&[("owner", "platform")]),
            Some("203.0.113.10"),
            now(),
        );

        let report = evaluate_kubernetes_node_inventory(&[exposed], Pillar::Security, now());
        assert!(reason_codes(&report).contains(&REASON_SEC_EXTERNAL_IP_EXPOSED));
    }

    #[test]
    fn stale_inventory_is_counted_for_any_pillar() {
        let stale = node(
            "stale-node",
            Some("Ready"),
            labels(&[("owner", "platform"), ("cost-center", "cc-42")]),
            None,
            now() - Duration::hours(DEFAULT_STALE_AFTER_HOURS + 2),
        );

        let report = evaluate_kubernetes_node_inventory(&[stale], Pillar::Cost, now());
        assert_eq!(report.stale_resources, 1);
        assert!(reason_codes(&report).contains(&REASON_INV_STALE_DATA));
    }

    #[test]
    fn healthy_node_passes_claimed_pillars() {
        let node = healthy_node();

        for pillar in [Pillar::Cost, Pillar::Resilience, Pillar::Security] {
            let report =
                evaluate_kubernetes_node_inventory(std::slice::from_ref(&node), pillar, now());
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

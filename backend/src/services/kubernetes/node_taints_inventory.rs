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

// Deterministic Kubernetes Node Taints inventory evaluator for roadmap rows
// 02-KUBERNETES-DASHBOARD-01912/01919/01940.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::services::aws::inventory::types::{
    score_pillar, InventoryFinding, Pillar, PillarReport, Severity, COST_ALLOCATION_TAG_KEYS,
    DEFAULT_STALE_AFTER_HOURS,
};

pub const RESOURCE_TYPE: &str = "KubernetesNodeTaint";
pub const REASON_COST_OWNER_NOT_RECORDED: &str = "K8S_NODE_TAINT_COST_OWNER_NOT_RECORDED";
pub const REASON_RES_NODE_NOT_READY: &str = "K8S_NODE_TAINT_RES_NODE_NOT_READY";
pub const REASON_RES_NOEXECUTE_EVICTION: &str = "K8S_NODE_TAINT_RES_NOEXECUTE_EVICTION";
pub const REASON_SEC_WEAK_ISOLATION_EFFECT: &str = "K8S_NODE_TAINT_SEC_WEAK_ISOLATION_EFFECT";
pub const REASON_INV_STALE_DATA: &str = "K8S_NODE_TAINT_INV_STALE_DATA";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeTaintInventoryItem {
    pub cluster_id: String,
    pub node_name: String,
    pub taint_key: String,
    pub taint_value: Option<String>,
    pub effect: String,
    pub time_added: Option<DateTime<Utc>>,
    pub node_ready_status: Option<String>,
    pub node_unschedulable: bool,
    pub roles: Vec<String>,
    pub labels: BTreeMap<String, String>,
    pub annotations: BTreeMap<String, String>,
    pub created_at: Option<DateTime<Utc>>,
    pub collected_at: DateTime<Utc>,
}

pub fn evaluate_kubernetes_node_taints_inventory(
    taints: &[NodeTaintInventoryItem],
    pillar: Pillar,
    now: DateTime<Utc>,
) -> PillarReport {
    let mut stale_resources = 0;
    let mut findings = Vec::new();

    for taint in taints {
        if let Some(finding) = stale_finding(taint, pillar, now) {
            stale_resources += 1;
            findings.push(finding);
        }

        match pillar {
            Pillar::Cost => evaluate_cost(taint, pillar, &mut findings),
            Pillar::Resilience => evaluate_resilience(taint, pillar, &mut findings),
            Pillar::Security => evaluate_security(taint, pillar, &mut findings),
            _ => {}
        }
    }

    PillarReport {
        pillar,
        resources_evaluated: taints.len(),
        stale_resources,
        score: score_pillar(&findings),
        findings,
    }
}

fn evaluate_cost(
    taint: &NodeTaintInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if has_any_metadata_key(&taint.labels, COST_ALLOCATION_TAG_KEYS)
        || has_any_metadata_key(&taint.annotations, COST_ALLOCATION_TAG_KEYS)
    {
        return;
    }

    findings.push(finding(
        taint,
        pillar,
        REASON_COST_OWNER_NOT_RECORDED,
        Severity::Medium,
        format!(
            "Kubernetes node taint {} on node {} has no owner, team, project, or cost-center label or annotation",
            taint_identity(taint), taint.node_name
        ),
        json!({
            "cluster_id": taint.cluster_id,
            "node": taint.node_name,
            "taint_key": taint.taint_key,
            "taint_value": taint.taint_value,
            "effect": taint.effect,
            "roles": taint.roles,
            "checked_keys": COST_ALLOCATION_TAG_KEYS,
            "checked_locations": ["labels", "annotations"],
        }),
    ));
}

fn evaluate_resilience(
    taint: &NodeTaintInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    let ready = taint
        .node_ready_status
        .as_deref()
        .map(str::trim)
        .filter(|status| !status.is_empty())
        .map(|status| status.eq_ignore_ascii_case("Ready"))
        .unwrap_or(false);

    if !ready {
        findings.push(finding(
            taint,
            pillar,
            REASON_RES_NODE_NOT_READY,
            Severity::High,
            format!(
                "Kubernetes node taint {} is attached to node {} whose Ready condition is not healthy",
                taint_identity(taint), taint.node_name
            ),
            json!({
                "cluster_id": taint.cluster_id,
                "node": taint.node_name,
                "taint_key": taint.taint_key,
                "taint_value": taint.taint_value,
                "effect": taint.effect,
                "node_ready_status": taint.node_ready_status,
            }),
        ));
    }

    if taint.effect.eq_ignore_ascii_case("NoExecute") {
        findings.push(finding(
            taint,
            pillar,
            REASON_RES_NOEXECUTE_EVICTION,
            Severity::High,
            format!(
                "Kubernetes node taint {} on node {} can evict pods with NoExecute semantics",
                taint_identity(taint), taint.node_name
            ),
            json!({
                "cluster_id": taint.cluster_id,
                "node": taint.node_name,
                "taint_key": taint.taint_key,
                "taint_value": taint.taint_value,
                "effect": taint.effect,
                "time_added": taint.time_added,
                "recommendation": "Confirm workloads tolerate this NoExecute taint and document rollback before applying it broadly",
            }),
        ));
    }
}

fn evaluate_security(
    taint: &NodeTaintInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if !is_security_sensitive_taint(taint) || is_strong_isolation_effect(&taint.effect) {
        return;
    }

    findings.push(finding(
        taint,
        pillar,
        REASON_SEC_WEAK_ISOLATION_EFFECT,
        Severity::Medium,
        format!(
            "Kubernetes node taint {} on node {} uses weak scheduling preference for security isolation",
            taint_identity(taint), taint.node_name
        ),
        json!({
            "cluster_id": taint.cluster_id,
            "node": taint.node_name,
            "taint_key": taint.taint_key,
            "taint_value": taint.taint_value,
            "effect": taint.effect,
            "recommendation": "Use NoSchedule or NoExecute for security-sensitive isolation taints instead of PreferNoSchedule",
        }),
    ));
}

fn stale_finding(
    taint: &NodeTaintInventoryItem,
    pillar: Pillar,
    now: DateTime<Utc>,
) -> Option<InventoryFinding> {
    let age_hours = (now - taint.collected_at).num_hours();
    if age_hours <= DEFAULT_STALE_AFTER_HOURS {
        return None;
    }

    Some(finding(
        taint,
        pillar,
        REASON_INV_STALE_DATA,
        Severity::Medium,
        format!(
            "Inventory data for Kubernetes node taint {} on node {} is {} hours old (threshold {} hours)",
            taint_identity(taint), taint.node_name, age_hours, DEFAULT_STALE_AFTER_HOURS
        ),
        json!({
            "cluster_id": taint.cluster_id,
            "node": taint.node_name,
            "taint_key": taint.taint_key,
            "taint_value": taint.taint_value,
            "effect": taint.effect,
            "collected_at": taint.collected_at,
            "age_hours": age_hours,
            "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
        }),
    ))
}

fn finding(
    taint: &NodeTaintInventoryItem,
    pillar: Pillar,
    reason_code: &str,
    severity: Severity,
    message: String,
    evidence: Value,
) -> InventoryFinding {
    InventoryFinding {
        resource_id: format!(
            "{}/{}/{}:{}",
            taint.cluster_id, taint.node_name, taint.taint_key, taint.effect
        ),
        arn: format!(
            "kubernetes://node-taint/{}/{}/{}:{}",
            taint.cluster_id, taint.node_name, taint.taint_key, taint.effect
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

fn is_security_sensitive_taint(taint: &NodeTaintInventoryItem) -> bool {
    let key = taint.taint_key.to_ascii_lowercase();
    let value = taint
        .taint_value
        .as_deref()
        .unwrap_or_default()
        .to_ascii_lowercase();
    [
        "security",
        "secure",
        "restricted",
        "pci",
        "hipaa",
        "isolated",
        "isolation",
    ]
    .iter()
    .any(|needle| key.contains(needle) || value.contains(needle))
}

fn is_strong_isolation_effect(effect: &str) -> bool {
    effect.eq_ignore_ascii_case("NoSchedule") || effect.eq_ignore_ascii_case("NoExecute")
}

fn taint_identity(taint: &NodeTaintInventoryItem) -> String {
    match taint.taint_value.as_deref() {
        Some(value) if !value.trim().is_empty() => {
            format!("{}={}:{}", taint.taint_key, value, taint.effect)
        }
        _ => format!("{}:{}", taint.taint_key, taint.effect),
    }
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

    fn taint(
        node_name: &str,
        key: &str,
        value: Option<&str>,
        effect: &str,
        node_ready_status: Option<&str>,
        labels: BTreeMap<String, String>,
        collected_at: DateTime<Utc>,
    ) -> NodeTaintInventoryItem {
        NodeTaintInventoryItem {
            cluster_id: "cluster-1".to_string(),
            node_name: node_name.to_string(),
            taint_key: key.to_string(),
            taint_value: value.map(str::to_string),
            effect: effect.to_string(),
            time_added: Some(now() - Duration::hours(2)),
            node_ready_status: node_ready_status.map(str::to_string),
            node_unschedulable: false,
            roles: vec!["worker".to_string()],
            labels,
            annotations: BTreeMap::new(),
            created_at: Some(now() - Duration::days(10)),
            collected_at,
        }
    }

    fn healthy_taint() -> NodeTaintInventoryItem {
        taint(
            "ip-10-0-0-10",
            "dedicated",
            Some("gpu"),
            "NoSchedule",
            Some("Ready"),
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
    fn cost_flags_missing_owner_metadata() {
        let item = taint(
            "unowned",
            "dedicated",
            Some("batch"),
            "NoSchedule",
            Some("Ready"),
            BTreeMap::new(),
            now(),
        );

        let report = evaluate_kubernetes_node_taints_inventory(&[item], Pillar::Cost, now());

        assert_eq!(report.resources_evaluated, 1);
        assert!(reason_codes(&report).contains(&REASON_COST_OWNER_NOT_RECORDED));
    }

    #[test]
    fn resilience_flags_not_ready_and_noexecute_taints() {
        let not_ready = taint(
            "not-ready",
            "dedicated",
            Some("batch"),
            "NoSchedule",
            Some("NotReady (KubeletNotReady)"),
            labels(&[("owner", "platform")]),
            now(),
        );
        let evicting = taint(
            "evicting",
            "node.kubernetes.io/unreachable",
            None,
            "NoExecute",
            Some("Ready"),
            labels(&[("owner", "platform")]),
            now(),
        );

        let report = evaluate_kubernetes_node_taints_inventory(
            &[not_ready, evicting],
            Pillar::Resilience,
            now(),
        );
        let codes = reason_codes(&report);

        assert!(codes.contains(&REASON_RES_NODE_NOT_READY));
        assert!(codes.contains(&REASON_RES_NOEXECUTE_EVICTION));
    }

    #[test]
    fn security_flags_weak_isolation_effects() {
        let item = taint(
            "security-node",
            "security-tier",
            Some("restricted"),
            "PreferNoSchedule",
            Some("Ready"),
            labels(&[("owner", "platform")]),
            now(),
        );

        let report = evaluate_kubernetes_node_taints_inventory(&[item], Pillar::Security, now());

        assert!(reason_codes(&report).contains(&REASON_SEC_WEAK_ISOLATION_EFFECT));
    }

    #[test]
    fn stale_inventory_is_counted_for_any_pillar() {
        let item = taint(
            "stale",
            "dedicated",
            Some("gpu"),
            "NoSchedule",
            Some("Ready"),
            labels(&[("owner", "platform")]),
            now() - Duration::hours(25),
        );

        let report = evaluate_kubernetes_node_taints_inventory(&[item], Pillar::Cost, now());

        assert_eq!(report.stale_resources, 1);
        assert!(reason_codes(&report).contains(&REASON_INV_STALE_DATA));
    }

    #[test]
    fn healthy_taint_passes_claimed_pillars() {
        let item = healthy_taint();

        for pillar in [Pillar::Cost, Pillar::Resilience, Pillar::Security] {
            let report = evaluate_kubernetes_node_taints_inventory(
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

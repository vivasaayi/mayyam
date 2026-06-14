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

// Deterministic Kubernetes PodDisruptionBudget inventory evaluator for roadmap rows
// 02-KUBERNETES-DASHBOARD-01226/01233/01254.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::services::aws::inventory::types::{
    score_pillar, InventoryFinding, Pillar, PillarReport, Severity, COST_ALLOCATION_TAG_KEYS,
    DEFAULT_STALE_AFTER_HOURS,
};

pub const RESOURCE_TYPE: &str = "KubernetesPodDisruptionBudget";
pub const REASON_COST_OWNER_NOT_RECORDED: &str = "K8S_PDB_COST_OWNER_NOT_RECORDED";
pub const REASON_RES_SELECTOR_NOT_SCOPED: &str = "K8S_PDB_RES_SELECTOR_NOT_SCOPED";
pub const REASON_SEC_ALWAYS_ALLOW_UNHEALTHY_EVICTIONS: &str =
    "K8S_PDB_SEC_ALWAYS_ALLOW_UNHEALTHY_EVICTIONS";
pub const REASON_INV_STALE_DATA: &str = "K8S_PDB_INV_STALE_DATA";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PdbConditionInventoryItem {
    pub type_: String,
    pub status: String,
    pub reason: Option<String>,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PdbInventoryItem {
    pub cluster_id: String,
    pub namespace: String,
    pub name: String,
    pub labels: BTreeMap<String, String>,
    pub annotations: BTreeMap<String, String>,
    pub min_available: Option<String>,
    pub max_unavailable: Option<String>,
    pub selector_present: bool,
    pub selector_match_labels: BTreeMap<String, String>,
    pub selector_expression_count: usize,
    pub unhealthy_pod_eviction_policy: Option<String>,
    pub current_healthy: Option<i32>,
    pub desired_healthy: Option<i32>,
    pub disruptions_allowed: Option<i32>,
    pub expected_pods: Option<i32>,
    pub disrupted_pod_count: usize,
    pub conditions: Vec<PdbConditionInventoryItem>,
    pub created_at: Option<DateTime<Utc>>,
    pub collected_at: DateTime<Utc>,
}

pub fn evaluate_kubernetes_pdb_inventory(
    pdbs: &[PdbInventoryItem],
    pillar: Pillar,
    now: DateTime<Utc>,
) -> PillarReport {
    let mut stale_resources = 0;
    let mut findings = Vec::new();

    for pdb in pdbs {
        if let Some(finding) = stale_finding(pdb, pillar, now) {
            stale_resources += 1;
            findings.push(finding);
        }

        match pillar {
            Pillar::Cost => evaluate_cost(pdb, pillar, &mut findings),
            Pillar::Resilience => evaluate_resilience(pdb, pillar, &mut findings),
            Pillar::Security => evaluate_security(pdb, pillar, &mut findings),
            _ => {}
        }
    }

    PillarReport {
        pillar,
        resources_evaluated: pdbs.len(),
        stale_resources,
        score: score_pillar(&findings),
        findings,
    }
}

fn evaluate_cost(pdb: &PdbInventoryItem, pillar: Pillar, findings: &mut Vec<InventoryFinding>) {
    if has_any_metadata_key(&pdb.labels, COST_ALLOCATION_TAG_KEYS)
        || has_any_metadata_key(&pdb.annotations, COST_ALLOCATION_TAG_KEYS)
    {
        return;
    }

    findings.push(finding(
        pdb,
        pillar,
        REASON_COST_OWNER_NOT_RECORDED,
        Severity::Medium,
        format!(
            "Kubernetes PodDisruptionBudget {}/{} has no owner, team, project, or cost-center label or annotation",
            pdb.namespace, pdb.name
        ),
        json!({
            "cluster_id": pdb.cluster_id,
            "namespace": pdb.namespace,
            "name": pdb.name,
            "min_available": pdb.min_available,
            "max_unavailable": pdb.max_unavailable,
            "checked_keys": COST_ALLOCATION_TAG_KEYS,
            "checked_locations": ["labels", "annotations"],
        }),
    ));
}

fn evaluate_resilience(
    pdb: &PdbInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if pdb.selector_present
        && (!pdb.selector_match_labels.is_empty() || pdb.selector_expression_count > 0)
    {
        return;
    }

    findings.push(finding(
        pdb,
        pillar,
        REASON_RES_SELECTOR_NOT_SCOPED,
        Severity::High,
        format!(
            "Kubernetes PodDisruptionBudget {}/{} has a null or empty selector",
            pdb.namespace, pdb.name
        ),
        json!({
            "cluster_id": pdb.cluster_id,
            "namespace": pdb.namespace,
            "name": pdb.name,
            "selector_present": pdb.selector_present,
            "selector_match_labels": pdb.selector_match_labels,
            "selector_expression_count": pdb.selector_expression_count,
            "recommendation": "Set a scoped selector so the PDB protects only the intended workload pods",
        }),
    ));
}

fn evaluate_security(pdb: &PdbInventoryItem, pillar: Pillar, findings: &mut Vec<InventoryFinding>) {
    if !pdb
        .unhealthy_pod_eviction_policy
        .as_deref()
        .map(|policy| policy.eq_ignore_ascii_case("AlwaysAllow"))
        .unwrap_or(false)
    {
        return;
    }

    findings.push(finding(
        pdb,
        pillar,
        REASON_SEC_ALWAYS_ALLOW_UNHEALTHY_EVICTIONS,
        Severity::Medium,
        format!(
            "Kubernetes PodDisruptionBudget {}/{} allows unhealthy pod evictions regardless of budget",
            pdb.namespace, pdb.name
        ),
        json!({
            "cluster_id": pdb.cluster_id,
            "namespace": pdb.namespace,
            "name": pdb.name,
            "unhealthy_pod_eviction_policy": pdb.unhealthy_pod_eviction_policy,
            "current_healthy": pdb.current_healthy,
            "desired_healthy": pdb.desired_healthy,
            "disruptions_allowed": pdb.disruptions_allowed,
            "recommendation": "Use IfHealthyBudget unless the workload has an explicit recovery design for unhealthy-pod evictions",
        }),
    ));
}

fn stale_finding(
    pdb: &PdbInventoryItem,
    pillar: Pillar,
    now: DateTime<Utc>,
) -> Option<InventoryFinding> {
    let age_hours = (now - pdb.collected_at).num_hours();
    if age_hours <= DEFAULT_STALE_AFTER_HOURS {
        return None;
    }

    Some(finding(
        pdb,
        pillar,
        REASON_INV_STALE_DATA,
        Severity::Medium,
        format!(
            "Inventory data for Kubernetes PodDisruptionBudget {}/{} is {} hours old (threshold {} hours)",
            pdb.namespace, pdb.name, age_hours, DEFAULT_STALE_AFTER_HOURS
        ),
        json!({
            "cluster_id": pdb.cluster_id,
            "namespace": pdb.namespace,
            "name": pdb.name,
            "collected_at": pdb.collected_at,
            "age_hours": age_hours,
            "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
        }),
    ))
}

fn finding(
    pdb: &PdbInventoryItem,
    pillar: Pillar,
    reason_code: &str,
    severity: Severity,
    message: String,
    evidence: Value,
) -> InventoryFinding {
    InventoryFinding {
        resource_id: format!(
            "{}/{}/PodDisruptionBudget/{}",
            pdb.cluster_id, pdb.namespace, pdb.name
        ),
        arn: format!(
            "kubernetes://poddisruptionbudgets/{}/{}/{}",
            pdb.cluster_id, pdb.namespace, pdb.name
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

    fn pdb(name: &str, metadata_labels: BTreeMap<String, String>) -> PdbInventoryItem {
        PdbInventoryItem {
            cluster_id: "cluster-a".to_string(),
            namespace: "apps".to_string(),
            name: name.to_string(),
            labels: metadata_labels,
            annotations: BTreeMap::new(),
            min_available: Some("2".to_string()),
            max_unavailable: None,
            selector_present: true,
            selector_match_labels: labels(&[("app", "checkout")]),
            selector_expression_count: 0,
            unhealthy_pod_eviction_policy: Some("IfHealthyBudget".to_string()),
            current_healthy: Some(4),
            desired_healthy: Some(2),
            disruptions_allowed: Some(2),
            expected_pods: Some(4),
            disrupted_pod_count: 0,
            conditions: vec![PdbConditionInventoryItem {
                type_: "DisruptionAllowed".to_string(),
                status: "True".to_string(),
                reason: Some("SufficientPods".to_string()),
                message: None,
            }],
            created_at: Some(now() - Duration::days(3)),
            collected_at: now(),
        }
    }

    fn healthy_pdb() -> PdbInventoryItem {
        pdb("checkout-pdb", labels(&[("team", "payments")]))
    }

    #[test]
    fn cost_flags_missing_owner_and_cost_allocation_labels() {
        let report = evaluate_kubernetes_pdb_inventory(
            &[pdb("untagged", BTreeMap::new())],
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
    fn resilience_flags_null_or_empty_selectors() {
        let mut unscoped = healthy_pdb();
        unscoped.selector_present = true;
        unscoped.selector_match_labels.clear();
        unscoped.selector_expression_count = 0;

        let report = evaluate_kubernetes_pdb_inventory(&[unscoped], Pillar::Resilience, now());

        assert_eq!(report.findings.len(), 1);
        assert_eq!(
            report.findings[0].reason_code,
            REASON_RES_SELECTOR_NOT_SCOPED
        );
    }

    #[test]
    fn security_flags_always_allow_unhealthy_evictions() {
        let mut risky = healthy_pdb();
        risky.unhealthy_pod_eviction_policy = Some("AlwaysAllow".to_string());

        let report = evaluate_kubernetes_pdb_inventory(&[risky], Pillar::Security, now());

        assert_eq!(report.findings.len(), 1);
        assert_eq!(
            report.findings[0].reason_code,
            REASON_SEC_ALWAYS_ALLOW_UNHEALTHY_EVICTIONS
        );
    }

    #[test]
    fn stale_inventory_is_counted_for_any_pillar() {
        let mut stale = healthy_pdb();
        stale.collected_at = now() - Duration::hours(DEFAULT_STALE_AFTER_HOURS + 2);

        let report = evaluate_kubernetes_pdb_inventory(&[stale], Pillar::Cost, now());

        assert_eq!(report.stale_resources, 1);
        assert_eq!(report.findings[0].reason_code, REASON_INV_STALE_DATA);
    }

    #[test]
    fn healthy_pdbs_pass_claimed_pillars() {
        for pillar in [Pillar::Cost, Pillar::Resilience, Pillar::Security] {
            let report = evaluate_kubernetes_pdb_inventory(&[healthy_pdb()], pillar, now());

            assert_eq!(report.resources_evaluated, 1);
            assert_eq!(report.stale_resources, 0);
            assert!(report.findings.is_empty());
        }
    }
}

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

// Deterministic Kubernetes ResourceQuota inventory evaluator for roadmap rows
// 02-KUBERNETES-DASHBOARD-01275/01282/01303.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::services::aws::inventory::types::{
    score_pillar, InventoryFinding, Pillar, PillarReport, Severity, COST_ALLOCATION_TAG_KEYS,
    DEFAULT_STALE_AFTER_HOURS,
};

pub const RESOURCE_TYPE: &str = "KubernetesResourceQuota";
pub const REASON_COST_OWNER_NOT_RECORDED: &str = "K8S_RQ_COST_OWNER_NOT_RECORDED";
pub const REASON_RES_HARD_LIMIT_EXHAUSTED: &str = "K8S_RQ_RES_HARD_LIMIT_EXHAUSTED";
pub const REASON_SEC_SENSITIVE_OBJECT_GUARDS_MISSING: &str =
    "K8S_RQ_SEC_SENSITIVE_OBJECT_GUARDS_MISSING";
pub const REASON_INV_STALE_DATA: &str = "K8S_RQ_INV_STALE_DATA";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceQuotaScopeSelectorInventoryItem {
    pub scope_name: String,
    pub operator: String,
    pub values: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceQuotaInventoryItem {
    pub cluster_id: String,
    pub namespace: String,
    pub name: String,
    pub labels: BTreeMap<String, String>,
    pub annotations: BTreeMap<String, String>,
    pub hard: BTreeMap<String, String>,
    pub used: BTreeMap<String, String>,
    pub scopes: Vec<String>,
    pub scope_selector: Vec<ResourceQuotaScopeSelectorInventoryItem>,
    pub created_at: Option<DateTime<Utc>>,
    pub collected_at: DateTime<Utc>,
}

pub fn evaluate_kubernetes_resource_quota_inventory(
    quotas: &[ResourceQuotaInventoryItem],
    pillar: Pillar,
    now: DateTime<Utc>,
) -> PillarReport {
    let mut stale_resources = 0;
    let mut findings = Vec::new();

    for quota in quotas {
        if let Some(finding) = stale_finding(quota, pillar, now) {
            stale_resources += 1;
            findings.push(finding);
        }

        match pillar {
            Pillar::Cost => evaluate_cost(quota, pillar, &mut findings),
            Pillar::Resilience => evaluate_resilience(quota, pillar, &mut findings),
            Pillar::Security => evaluate_security(quota, pillar, &mut findings),
            _ => {}
        }
    }

    PillarReport {
        pillar,
        resources_evaluated: quotas.len(),
        stale_resources,
        score: score_pillar(&findings),
        findings,
    }
}

fn evaluate_cost(
    quota: &ResourceQuotaInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if has_any_metadata_key(&quota.labels, COST_ALLOCATION_TAG_KEYS)
        || has_any_metadata_key(&quota.annotations, COST_ALLOCATION_TAG_KEYS)
    {
        return;
    }

    findings.push(finding(
        quota,
        pillar,
        REASON_COST_OWNER_NOT_RECORDED,
        Severity::Medium,
        format!(
            "Kubernetes ResourceQuota {}/{} has no owner, team, project, or cost-center label or annotation",
            quota.namespace, quota.name
        ),
        json!({
            "cluster_id": quota.cluster_id,
            "namespace": quota.namespace,
            "name": quota.name,
            "hard": quota.hard,
            "checked_keys": COST_ALLOCATION_TAG_KEYS,
            "checked_locations": ["labels", "annotations"],
        }),
    ));
}

fn evaluate_resilience(
    quota: &ResourceQuotaInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    let exhausted_limits = quota
        .hard
        .iter()
        .filter_map(|(resource, hard)| {
            let used = quota.used.get(resource)?;
            if is_hard_limit_exhausted(used, hard) {
                Some(json!({
                    "resource": resource,
                    "used": used,
                    "hard": hard,
                }))
            } else {
                None
            }
        })
        .collect::<Vec<_>>();

    if exhausted_limits.is_empty() {
        return;
    }

    findings.push(finding(
        quota,
        pillar,
        REASON_RES_HARD_LIMIT_EXHAUSTED,
        Severity::High,
        format!(
            "Kubernetes ResourceQuota {}/{} has hard limits at or above quota usage",
            quota.namespace, quota.name
        ),
        json!({
            "cluster_id": quota.cluster_id,
            "namespace": quota.namespace,
            "name": quota.name,
            "exhausted_limits": exhausted_limits,
            "recommendation": "Raise the relevant hard quota or reduce namespace usage before new workload scheduling fails",
        }),
    ));
}

fn evaluate_security(
    quota: &ResourceQuotaInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    const SENSITIVE_GUARD_KEYS: &[&str] = &[
        "count/secrets",
        "count/configmaps",
        "services.nodeports",
        "services.loadbalancers",
    ];

    let missing_guardrails = SENSITIVE_GUARD_KEYS
        .iter()
        .filter(|key| {
            !quota
                .hard
                .keys()
                .any(|hard_key| hard_key.eq_ignore_ascii_case(key))
        })
        .copied()
        .collect::<Vec<_>>();

    if missing_guardrails.is_empty() {
        return;
    }

    findings.push(finding(
        quota,
        pillar,
        REASON_SEC_SENSITIVE_OBJECT_GUARDS_MISSING,
        Severity::Medium,
        format!(
            "Kubernetes ResourceQuota {}/{} does not limit all sensitive object families",
            quota.namespace, quota.name
        ),
        json!({
            "cluster_id": quota.cluster_id,
            "namespace": quota.namespace,
            "name": quota.name,
            "missing_guardrails": missing_guardrails,
            "hard_keys": quota.hard.keys().cloned().collect::<Vec<_>>(),
            "recommendation": "Add explicit object-count quotas for secrets, configmaps, nodeports, and load balancers where namespace policy allows them",
        }),
    ));
}

fn stale_finding(
    quota: &ResourceQuotaInventoryItem,
    pillar: Pillar,
    now: DateTime<Utc>,
) -> Option<InventoryFinding> {
    let age_hours = (now - quota.collected_at).num_hours();
    if age_hours <= DEFAULT_STALE_AFTER_HOURS {
        return None;
    }

    Some(finding(
        quota,
        pillar,
        REASON_INV_STALE_DATA,
        Severity::Medium,
        format!(
            "Inventory data for Kubernetes ResourceQuota {}/{} is {} hours old (threshold {} hours)",
            quota.namespace, quota.name, age_hours, DEFAULT_STALE_AFTER_HOURS
        ),
        json!({
            "cluster_id": quota.cluster_id,
            "namespace": quota.namespace,
            "name": quota.name,
            "collected_at": quota.collected_at,
            "age_hours": age_hours,
            "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
        }),
    ))
}

fn finding(
    quota: &ResourceQuotaInventoryItem,
    pillar: Pillar,
    reason_code: &str,
    severity: Severity,
    message: String,
    evidence: Value,
) -> InventoryFinding {
    InventoryFinding {
        resource_id: format!(
            "{}/{}/ResourceQuota/{}",
            quota.cluster_id, quota.namespace, quota.name
        ),
        arn: format!(
            "kubernetes://resourcequotas/{}/{}/{}",
            quota.cluster_id, quota.namespace, quota.name
        ),
        pillar,
        reason_code: reason_code.to_string(),
        severity,
        message,
        evidence,
    }
}

fn is_hard_limit_exhausted(used: &str, hard: &str) -> bool {
    match (parse_quantity(used), parse_quantity(hard)) {
        (Some(used), Some(hard)) if hard > 0.0 => used >= hard,
        _ => used.trim() == hard.trim() && hard.trim() != "0",
    }
}

fn parse_quantity(raw: &str) -> Option<f64> {
    let value = raw.trim();
    if value.is_empty() {
        return None;
    }

    let binary_suffixes = [
        ("Ki", 1024_f64),
        ("Mi", 1024_f64.powi(2)),
        ("Gi", 1024_f64.powi(3)),
        ("Ti", 1024_f64.powi(4)),
        ("Pi", 1024_f64.powi(5)),
        ("Ei", 1024_f64.powi(6)),
    ];
    for (suffix, multiplier) in binary_suffixes {
        if let Some(number) = value.strip_suffix(suffix) {
            return number.parse::<f64>().ok().map(|parsed| parsed * multiplier);
        }
    }

    let decimal_suffixes = [
        ("n", 0.000000001_f64),
        ("u", 0.000001_f64),
        ("m", 0.001_f64),
        ("k", 1_000_f64),
        ("M", 1_000_000_f64),
        ("G", 1_000_000_000_f64),
        ("T", 1_000_000_000_000_f64),
        ("P", 1_000_000_000_000_000_f64),
        ("E", 1_000_000_000_000_000_000_f64),
    ];
    for (suffix, multiplier) in decimal_suffixes {
        if let Some(number) = value.strip_suffix(suffix) {
            return number.parse::<f64>().ok().map(|parsed| parsed * multiplier);
        }
    }

    value.parse::<f64>().ok()
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

    fn quota(name: &str, metadata_labels: BTreeMap<String, String>) -> ResourceQuotaInventoryItem {
        ResourceQuotaInventoryItem {
            cluster_id: "cluster-a".to_string(),
            namespace: "apps".to_string(),
            name: name.to_string(),
            labels: metadata_labels,
            annotations: BTreeMap::new(),
            hard: labels(&[
                ("pods", "10"),
                ("requests.cpu", "4"),
                ("count/secrets", "20"),
                ("count/configmaps", "40"),
                ("services.nodeports", "0"),
                ("services.loadbalancers", "0"),
            ]),
            used: labels(&[
                ("pods", "4"),
                ("requests.cpu", "1500m"),
                ("count/secrets", "3"),
                ("count/configmaps", "8"),
                ("services.nodeports", "0"),
                ("services.loadbalancers", "0"),
            ]),
            scopes: vec!["NotTerminating".to_string()],
            scope_selector: Vec::new(),
            created_at: Some(now() - Duration::days(3)),
            collected_at: now(),
        }
    }

    fn healthy_quota() -> ResourceQuotaInventoryItem {
        quota("team-a-quota", labels(&[("team", "platform")]))
    }

    #[test]
    fn cost_flags_missing_owner_and_cost_allocation_labels() {
        let report = evaluate_kubernetes_resource_quota_inventory(
            &[quota("untagged", BTreeMap::new())],
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
    fn resilience_flags_hard_limits_that_are_fully_used() {
        let mut exhausted = healthy_quota();
        exhausted.used.insert("pods".to_string(), "10".to_string());

        let report =
            evaluate_kubernetes_resource_quota_inventory(&[exhausted], Pillar::Resilience, now());

        assert_eq!(report.findings.len(), 1);
        assert_eq!(
            report.findings[0].reason_code,
            REASON_RES_HARD_LIMIT_EXHAUSTED
        );
    }

    #[test]
    fn security_flags_missing_sensitive_object_guardrails() {
        let mut missing_guards = healthy_quota();
        missing_guards.hard.remove("count/secrets");
        missing_guards.hard.remove("services.nodeports");

        let report = evaluate_kubernetes_resource_quota_inventory(
            &[missing_guards],
            Pillar::Security,
            now(),
        );

        assert_eq!(report.findings.len(), 1);
        assert_eq!(
            report.findings[0].reason_code,
            REASON_SEC_SENSITIVE_OBJECT_GUARDS_MISSING
        );
    }

    #[test]
    fn stale_inventory_is_counted_for_any_pillar() {
        let mut stale = healthy_quota();
        stale.collected_at = now() - Duration::hours(DEFAULT_STALE_AFTER_HOURS + 2);

        let report = evaluate_kubernetes_resource_quota_inventory(&[stale], Pillar::Cost, now());

        assert_eq!(report.stale_resources, 1);
        assert_eq!(report.findings[0].reason_code, REASON_INV_STALE_DATA);
    }

    #[test]
    fn healthy_resource_quotas_pass_claimed_pillars() {
        for pillar in [Pillar::Cost, Pillar::Resilience, Pillar::Security] {
            let report =
                evaluate_kubernetes_resource_quota_inventory(&[healthy_quota()], pillar, now());

            assert_eq!(report.resources_evaluated, 1);
            assert_eq!(report.stale_resources, 0);
            assert!(report.findings.is_empty());
        }
    }
}

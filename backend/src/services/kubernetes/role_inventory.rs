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

// Deterministic Kubernetes Role inventory evaluator for roadmap rows
// 02-KUBERNETES-DASHBOARD-00883/00890/00911.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::services::aws::inventory::types::{
    score_pillar, InventoryFinding, Pillar, PillarReport, Severity, COST_ALLOCATION_TAG_KEYS,
    DEFAULT_STALE_AFTER_HOURS,
};

pub const RESOURCE_TYPE: &str = "KubernetesRole";
pub const REASON_COST_OWNER_NOT_RECORDED: &str = "K8S_ROLE_COST_OWNER_NOT_RECORDED";
pub const REASON_RES_EMPTY_RULES: &str = "K8S_ROLE_RES_EMPTY_RULES";
pub const REASON_SEC_WILDCARD_PERMISSION: &str = "K8S_ROLE_SEC_WILDCARD_PERMISSION";
pub const REASON_SEC_SECRET_READ: &str = "K8S_ROLE_SEC_SECRET_READ";
pub const REASON_INV_STALE_DATA: &str = "K8S_ROLE_INV_STALE_DATA";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoleOwnerReferenceInventoryItem {
    pub kind: Option<String>,
    pub name: String,
    pub controller: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoleRuleInventoryItem {
    pub api_groups: Vec<String>,
    pub resources: Vec<String>,
    pub verbs: Vec<String>,
    pub resource_names: Vec<String>,
    pub non_resource_urls: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoleInventoryItem {
    pub cluster_id: String,
    pub namespace: String,
    pub name: String,
    pub labels: BTreeMap<String, String>,
    pub annotations: BTreeMap<String, String>,
    pub rules: Vec<RoleRuleInventoryItem>,
    pub owner_references: Vec<RoleOwnerReferenceInventoryItem>,
    pub created_at: Option<DateTime<Utc>>,
    pub collected_at: DateTime<Utc>,
}

pub fn evaluate_kubernetes_role_inventory(
    roles: &[RoleInventoryItem],
    pillar: Pillar,
    now: DateTime<Utc>,
) -> PillarReport {
    let mut stale_resources = 0;
    let mut findings = Vec::new();

    for role in roles {
        if let Some(finding) = stale_finding(role, pillar, now) {
            stale_resources += 1;
            findings.push(finding);
        }

        match pillar {
            Pillar::Cost => evaluate_cost(role, pillar, &mut findings),
            Pillar::Resilience => evaluate_resilience(role, pillar, &mut findings),
            Pillar::Security => evaluate_security(role, pillar, &mut findings),
            _ => {}
        }
    }

    PillarReport {
        pillar,
        resources_evaluated: roles.len(),
        stale_resources,
        score: score_pillar(&findings),
        findings,
    }
}

fn evaluate_cost(role: &RoleInventoryItem, pillar: Pillar, findings: &mut Vec<InventoryFinding>) {
    if has_any_metadata_key(&role.labels, COST_ALLOCATION_TAG_KEYS)
        || has_any_metadata_key(&role.annotations, COST_ALLOCATION_TAG_KEYS)
    {
        return;
    }

    findings.push(finding(
        role,
        pillar,
        REASON_COST_OWNER_NOT_RECORDED,
        Severity::Medium,
        format!(
            "Kubernetes Role {}/{} has no owner, team, project, or cost-center label or annotation",
            role.namespace, role.name
        ),
        json!({
            "cluster_id": role.cluster_id,
            "namespace": role.namespace,
            "name": role.name,
            "rule_count": role.rules.len(),
            "checked_keys": COST_ALLOCATION_TAG_KEYS,
            "checked_locations": ["labels", "annotations"],
        }),
    ));
}

fn evaluate_resilience(
    role: &RoleInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if role.rules.is_empty() {
        findings.push(finding(
            role,
            pillar,
            REASON_RES_EMPTY_RULES,
            Severity::Medium,
            format!(
                "Kubernetes Role {}/{} has no RBAC rules and grants no effective permissions",
                role.namespace, role.name
            ),
            json!({
                "cluster_id": role.cluster_id,
                "namespace": role.namespace,
                "name": role.name,
                "rule_count": role.rules.len(),
                "owner_references": role.owner_references,
                "recommendation": "Remove unused empty Roles or replace them with explicit workload permissions before binding",
            }),
        ));
    }
}

fn evaluate_security(
    role: &RoleInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    let wildcard_rules = role
        .rules
        .iter()
        .filter(|rule| grants_wildcard_permission(rule))
        .cloned()
        .collect::<Vec<_>>();
    if !wildcard_rules.is_empty() {
        findings.push(finding(
            role,
            pillar,
            REASON_SEC_WILDCARD_PERMISSION,
            Severity::High,
            format!(
                "Kubernetes Role {}/{} grants wildcard RBAC permissions",
                role.namespace, role.name
            ),
            json!({
                "cluster_id": role.cluster_id,
                "namespace": role.namespace,
                "name": role.name,
                "matching_rules": wildcard_rules,
                "recommendation": "Replace wildcard apiGroups, resources, or verbs with the smallest explicit permission set",
            }),
        ));
    }

    let secret_read_rules = role
        .rules
        .iter()
        .filter(|rule| grants_secret_read(rule))
        .cloned()
        .collect::<Vec<_>>();
    if !secret_read_rules.is_empty() {
        findings.push(finding(
            role,
            pillar,
            REASON_SEC_SECRET_READ,
            Severity::High,
            format!(
                "Kubernetes Role {}/{} grants read access to Secrets",
                role.namespace, role.name
            ),
            json!({
                "cluster_id": role.cluster_id,
                "namespace": role.namespace,
                "name": role.name,
                "matching_rules": secret_read_rules,
                "recommendation": "Restrict Secret read permissions to named Secrets and workloads that require them",
                "values_redacted": true,
            }),
        ));
    }
}

fn stale_finding(
    role: &RoleInventoryItem,
    pillar: Pillar,
    now: DateTime<Utc>,
) -> Option<InventoryFinding> {
    let age_hours = (now - role.collected_at).num_hours();
    if age_hours <= DEFAULT_STALE_AFTER_HOURS {
        return None;
    }

    Some(finding(
        role,
        pillar,
        REASON_INV_STALE_DATA,
        Severity::Medium,
        format!(
            "Inventory data for Kubernetes Role {}/{} is {} hours old (threshold {} hours)",
            role.namespace, role.name, age_hours, DEFAULT_STALE_AFTER_HOURS
        ),
        json!({
            "cluster_id": role.cluster_id,
            "namespace": role.namespace,
            "name": role.name,
            "collected_at": role.collected_at,
            "age_hours": age_hours,
            "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
        }),
    ))
}

fn finding(
    role: &RoleInventoryItem,
    pillar: Pillar,
    reason_code: &str,
    severity: Severity,
    message: String,
    evidence: Value,
) -> InventoryFinding {
    InventoryFinding {
        resource_id: format!("{}/{}/Role/{}", role.cluster_id, role.namespace, role.name),
        arn: format!(
            "kubernetes://roles/{}/{}/{}",
            role.cluster_id, role.namespace, role.name
        ),
        pillar,
        reason_code: reason_code.to_string(),
        severity,
        message,
        evidence,
    }
}

fn grants_wildcard_permission(rule: &RoleRuleInventoryItem) -> bool {
    contains_wildcard(&rule.api_groups)
        || contains_wildcard(&rule.resources)
        || contains_wildcard(&rule.verbs)
        || contains_wildcard(&rule.non_resource_urls)
}

fn grants_secret_read(rule: &RoleRuleInventoryItem) -> bool {
    let grants_secret_resource =
        contains_value(&rule.resources, "secrets") || contains_wildcard(&rule.resources);
    let grants_read_verb = contains_wildcard(&rule.verbs)
        || ["get", "list", "watch"]
            .iter()
            .any(|verb| contains_value(&rule.verbs, verb));

    grants_secret_resource && grants_read_verb
}

fn contains_wildcard(values: &[String]) -> bool {
    values.iter().any(|value| value == "*")
}

fn contains_value(values: &[String], wanted: &str) -> bool {
    values
        .iter()
        .any(|value| value.eq_ignore_ascii_case(wanted))
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

    fn rule(resources: &[&str], verbs: &[&str]) -> RoleRuleInventoryItem {
        RoleRuleInventoryItem {
            api_groups: vec!["".to_string()],
            resources: resources
                .iter()
                .map(|resource| (*resource).to_string())
                .collect(),
            verbs: verbs.iter().map(|verb| (*verb).to_string()).collect(),
            resource_names: Vec::new(),
            non_resource_urls: Vec::new(),
        }
    }

    fn role(name: &str, metadata_labels: BTreeMap<String, String>) -> RoleInventoryItem {
        RoleInventoryItem {
            cluster_id: "cluster-a".to_string(),
            namespace: "apps".to_string(),
            name: name.to_string(),
            labels: metadata_labels,
            annotations: BTreeMap::new(),
            rules: vec![rule(&["configmaps"], &["get", "list"])],
            owner_references: Vec::new(),
            created_at: Some(now() - Duration::days(3)),
            collected_at: now(),
        }
    }

    fn healthy_role() -> RoleInventoryItem {
        role("reader", labels(&[("team", "payments")]))
    }

    #[test]
    fn cost_flags_missing_owner_and_cost_allocation_labels() {
        let report = evaluate_kubernetes_role_inventory(
            &[role("untagged", BTreeMap::new())],
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
    fn resilience_flags_roles_with_no_rules() {
        let mut empty = healthy_role();
        empty.rules = Vec::new();

        let report = evaluate_kubernetes_role_inventory(&[empty], Pillar::Resilience, now());

        assert_eq!(report.findings.len(), 1);
        assert_eq!(report.findings[0].reason_code, REASON_RES_EMPTY_RULES);
    }

    #[test]
    fn security_flags_wildcard_and_secret_read_permissions() {
        let mut risky = healthy_role();
        risky.rules = vec![rule(&["*"], &["*"]), rule(&["secrets"], &["get", "list"])];

        let report = evaluate_kubernetes_role_inventory(&[risky], Pillar::Security, now());
        let reason_codes = report
            .findings
            .iter()
            .map(|finding| finding.reason_code.as_str())
            .collect::<Vec<_>>();

        assert!(reason_codes.contains(&REASON_SEC_WILDCARD_PERMISSION));
        assert!(reason_codes.contains(&REASON_SEC_SECRET_READ));
    }

    #[test]
    fn stale_inventory_is_counted_for_any_pillar() {
        let mut stale = healthy_role();
        stale.collected_at = now() - Duration::hours(DEFAULT_STALE_AFTER_HOURS + 2);

        let report = evaluate_kubernetes_role_inventory(&[stale], Pillar::Cost, now());

        assert_eq!(report.stale_resources, 1);
        assert_eq!(report.findings[0].reason_code, REASON_INV_STALE_DATA);
    }

    #[test]
    fn healthy_roles_pass_claimed_pillars() {
        for pillar in [Pillar::Cost, Pillar::Resilience, Pillar::Security] {
            let report = evaluate_kubernetes_role_inventory(&[healthy_role()], pillar, now());

            assert_eq!(report.resources_evaluated, 1);
            assert_eq!(report.stale_resources, 0);
            assert!(report.findings.is_empty());
        }
    }
}

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

// Deterministic Kubernetes ClusterRoleBinding inventory evaluator for roadmap rows
// 02-KUBERNETES-DASHBOARD-01030/01037/01058.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::services::aws::inventory::types::{
    score_pillar, InventoryFinding, Pillar, PillarReport, Severity, COST_ALLOCATION_TAG_KEYS,
    DEFAULT_STALE_AFTER_HOURS,
};

pub const RESOURCE_TYPE: &str = "KubernetesClusterRoleBinding";
pub const REASON_COST_OWNER_NOT_RECORDED: &str = "K8S_CLUSTER_ROLE_BINDING_COST_OWNER_NOT_RECORDED";
pub const REASON_RES_EMPTY_SUBJECTS: &str = "K8S_CLUSTER_ROLE_BINDING_RES_EMPTY_SUBJECTS";
pub const REASON_SEC_DEFAULT_SERVICE_ACCOUNT_SUBJECT: &str =
    "K8S_CLUSTER_ROLE_BINDING_SEC_DEFAULT_SERVICE_ACCOUNT_SUBJECT";
pub const REASON_SEC_PRIVILEGED_ROLE_REFERENCE: &str =
    "K8S_CLUSTER_ROLE_BINDING_SEC_PRIVILEGED_ROLE_REFERENCE";
pub const REASON_INV_STALE_DATA: &str = "K8S_CLUSTER_ROLE_BINDING_INV_STALE_DATA";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterRoleBindingOwnerReferenceInventoryItem {
    pub kind: Option<String>,
    pub name: String,
    pub controller: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterRoleBindingRoleRefInventoryItem {
    pub api_group: String,
    pub kind: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterRoleBindingSubjectInventoryItem {
    pub api_group: Option<String>,
    pub kind: String,
    pub namespace: Option<String>,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterRoleBindingInventoryItem {
    pub cluster_id: String,
    pub name: String,
    pub labels: BTreeMap<String, String>,
    pub annotations: BTreeMap<String, String>,
    pub role_ref: ClusterRoleBindingRoleRefInventoryItem,
    pub subjects: Vec<ClusterRoleBindingSubjectInventoryItem>,
    pub owner_references: Vec<ClusterRoleBindingOwnerReferenceInventoryItem>,
    pub created_at: Option<DateTime<Utc>>,
    pub collected_at: DateTime<Utc>,
}

pub fn evaluate_kubernetes_cluster_role_binding_inventory(
    cluster_role_bindings: &[ClusterRoleBindingInventoryItem],
    pillar: Pillar,
    now: DateTime<Utc>,
) -> PillarReport {
    let mut stale_resources = 0;
    let mut findings = Vec::new();

    for cluster_role_binding in cluster_role_bindings {
        if let Some(finding) = stale_finding(cluster_role_binding, pillar, now) {
            stale_resources += 1;
            findings.push(finding);
        }

        match pillar {
            Pillar::Cost => evaluate_cost(cluster_role_binding, pillar, &mut findings),
            Pillar::Resilience => evaluate_resilience(cluster_role_binding, pillar, &mut findings),
            Pillar::Security => evaluate_security(cluster_role_binding, pillar, &mut findings),
            _ => {}
        }
    }

    PillarReport {
        pillar,
        resources_evaluated: cluster_role_bindings.len(),
        stale_resources,
        score: score_pillar(&findings),
        findings,
    }
}

fn evaluate_cost(
    cluster_role_binding: &ClusterRoleBindingInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if has_any_metadata_key(&cluster_role_binding.labels, COST_ALLOCATION_TAG_KEYS)
        || has_any_metadata_key(&cluster_role_binding.annotations, COST_ALLOCATION_TAG_KEYS)
    {
        return;
    }

    findings.push(finding(
        cluster_role_binding,
        pillar,
        REASON_COST_OWNER_NOT_RECORDED,
        Severity::Medium,
        format!(
            "Kubernetes ClusterRoleBinding {} has no owner, team, project, or cost-center label or annotation",
            cluster_role_binding.name
        ),
        json!({
            "cluster_id": cluster_role_binding.cluster_id,
            "name": cluster_role_binding.name,
            "role_ref": cluster_role_binding.role_ref,
            "subject_count": cluster_role_binding.subjects.len(),
            "checked_keys": COST_ALLOCATION_TAG_KEYS,
            "checked_locations": ["labels", "annotations"],
        }),
    ));
}

fn evaluate_resilience(
    cluster_role_binding: &ClusterRoleBindingInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if cluster_role_binding.subjects.is_empty() {
        findings.push(finding(
            cluster_role_binding,
            pillar,
            REASON_RES_EMPTY_SUBJECTS,
            Severity::Medium,
            format!(
                "Kubernetes ClusterRoleBinding {} has no subjects and grants no effective permissions",
                cluster_role_binding.name
            ),
            json!({
                "cluster_id": cluster_role_binding.cluster_id,
                "name": cluster_role_binding.name,
                "role_ref": cluster_role_binding.role_ref,
                "subject_count": cluster_role_binding.subjects.len(),
                "owner_references": cluster_role_binding.owner_references,
                "recommendation": "Remove unused ClusterRoleBindings or attach them to explicit workload subjects before relying on this cluster-wide RBAC grant",
            }),
        ));
    }
}

fn evaluate_security(
    cluster_role_binding: &ClusterRoleBindingInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    let default_service_account_subjects = cluster_role_binding
        .subjects
        .iter()
        .filter(|subject| is_default_service_account_subject(subject))
        .cloned()
        .collect::<Vec<_>>();
    if !default_service_account_subjects.is_empty() {
        findings.push(finding(
            cluster_role_binding,
            pillar,
            REASON_SEC_DEFAULT_SERVICE_ACCOUNT_SUBJECT,
            Severity::High,
            format!(
                "Kubernetes ClusterRoleBinding {} grants cluster-wide permissions to the default ServiceAccount",
                cluster_role_binding.name
            ),
            json!({
                "cluster_id": cluster_role_binding.cluster_id,
                "name": cluster_role_binding.name,
                "role_ref": cluster_role_binding.role_ref,
                "matching_subjects": default_service_account_subjects,
                "recommendation": "Bind cluster-scoped permissions to named workload ServiceAccounts instead of any namespace default ServiceAccount",
            }),
        ));
    }

    if references_privileged_role(&cluster_role_binding.role_ref) {
        findings.push(finding(
            cluster_role_binding,
            pillar,
            REASON_SEC_PRIVILEGED_ROLE_REFERENCE,
            Severity::High,
            format!(
                "Kubernetes ClusterRoleBinding {} references privileged RBAC role {}",
                cluster_role_binding.name, cluster_role_binding.role_ref.name
            ),
            json!({
                "cluster_id": cluster_role_binding.cluster_id,
                "name": cluster_role_binding.name,
                "role_ref": cluster_role_binding.role_ref,
                "subjects": cluster_role_binding.subjects,
                "recommendation": "Replace broad admin/edit/cluster-admin role references with the smallest ClusterRole required by the subject",
            }),
        ));
    }
}

fn stale_finding(
    cluster_role_binding: &ClusterRoleBindingInventoryItem,
    pillar: Pillar,
    now: DateTime<Utc>,
) -> Option<InventoryFinding> {
    let age_hours = (now - cluster_role_binding.collected_at).num_hours();
    if age_hours <= DEFAULT_STALE_AFTER_HOURS {
        return None;
    }

    Some(finding(
        cluster_role_binding,
        pillar,
        REASON_INV_STALE_DATA,
        Severity::Medium,
        format!(
            "Inventory data for Kubernetes ClusterRoleBinding {} is {} hours old (threshold {} hours)",
            cluster_role_binding.name, age_hours, DEFAULT_STALE_AFTER_HOURS
        ),
        json!({
            "cluster_id": cluster_role_binding.cluster_id,
            "name": cluster_role_binding.name,
            "collected_at": cluster_role_binding.collected_at,
            "age_hours": age_hours,
            "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
        }),
    ))
}

fn finding(
    cluster_role_binding: &ClusterRoleBindingInventoryItem,
    pillar: Pillar,
    reason_code: &str,
    severity: Severity,
    message: String,
    evidence: Value,
) -> InventoryFinding {
    InventoryFinding {
        resource_id: format!(
            "{}/ClusterRoleBinding/{}",
            cluster_role_binding.cluster_id, cluster_role_binding.name
        ),
        arn: format!(
            "kubernetes://clusterrolebindings/{}/{}",
            cluster_role_binding.cluster_id, cluster_role_binding.name
        ),
        pillar,
        reason_code: reason_code.to_string(),
        severity,
        message,
        evidence,
    }
}

fn is_default_service_account_subject(subject: &ClusterRoleBindingSubjectInventoryItem) -> bool {
    subject.kind.eq_ignore_ascii_case("ServiceAccount") && subject.name == "default"
}

fn references_privileged_role(role_ref: &ClusterRoleBindingRoleRefInventoryItem) -> bool {
    ["admin", "edit", "cluster-admin"]
        .iter()
        .any(|privileged| role_ref.name.eq_ignore_ascii_case(privileged))
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

    fn role_ref(kind: &str, name: &str) -> ClusterRoleBindingRoleRefInventoryItem {
        ClusterRoleBindingRoleRefInventoryItem {
            api_group: "rbac.authorization.k8s.io".to_string(),
            kind: kind.to_string(),
            name: name.to_string(),
        }
    }

    fn service_account_subject(name: &str) -> ClusterRoleBindingSubjectInventoryItem {
        ClusterRoleBindingSubjectInventoryItem {
            api_group: None,
            kind: "ServiceAccount".to_string(),
            namespace: Some("apps".to_string()),
            name: name.to_string(),
        }
    }

    fn cluster_role_binding(
        name: &str,
        metadata_labels: BTreeMap<String, String>,
    ) -> ClusterRoleBindingInventoryItem {
        ClusterRoleBindingInventoryItem {
            cluster_id: "cluster-a".to_string(),
            name: name.to_string(),
            labels: metadata_labels,
            annotations: BTreeMap::new(),
            role_ref: role_ref("ClusterRole", "reader"),
            subjects: vec![service_account_subject("checkout")],
            owner_references: Vec::new(),
            created_at: Some(now() - Duration::days(3)),
            collected_at: now(),
        }
    }

    fn healthy_cluster_role_binding() -> ClusterRoleBindingInventoryItem {
        cluster_role_binding("checkout-reader", labels(&[("team", "payments")]))
    }

    #[test]
    fn cost_flags_missing_owner_and_cost_allocation_labels() {
        let report = evaluate_kubernetes_cluster_role_binding_inventory(
            &[cluster_role_binding("untagged", BTreeMap::new())],
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
    fn resilience_flags_bindings_with_no_subjects() {
        let mut empty = healthy_cluster_role_binding();
        empty.subjects = Vec::new();

        let report =
            evaluate_kubernetes_cluster_role_binding_inventory(&[empty], Pillar::Resilience, now());

        assert_eq!(report.findings.len(), 1);
        assert_eq!(report.findings[0].reason_code, REASON_RES_EMPTY_SUBJECTS);
    }

    #[test]
    fn security_flags_default_service_accounts_and_privileged_role_refs() {
        let mut risky = healthy_cluster_role_binding();
        risky.role_ref = role_ref("ClusterRole", "cluster-admin");
        risky.subjects = vec![service_account_subject("default")];

        let report =
            evaluate_kubernetes_cluster_role_binding_inventory(&[risky], Pillar::Security, now());
        let reason_codes = report
            .findings
            .iter()
            .map(|finding| finding.reason_code.as_str())
            .collect::<Vec<_>>();

        assert!(reason_codes.contains(&REASON_SEC_DEFAULT_SERVICE_ACCOUNT_SUBJECT));
        assert!(reason_codes.contains(&REASON_SEC_PRIVILEGED_ROLE_REFERENCE));
    }

    #[test]
    fn stale_inventory_is_counted_for_any_pillar() {
        let mut stale = healthy_cluster_role_binding();
        stale.collected_at = now() - Duration::hours(DEFAULT_STALE_AFTER_HOURS + 2);

        let report =
            evaluate_kubernetes_cluster_role_binding_inventory(&[stale], Pillar::Cost, now());

        assert_eq!(report.stale_resources, 1);
        assert_eq!(report.findings[0].reason_code, REASON_INV_STALE_DATA);
    }

    #[test]
    fn healthy_cluster_role_bindings_pass_claimed_pillars() {
        for pillar in [Pillar::Cost, Pillar::Resilience, Pillar::Security] {
            let report = evaluate_kubernetes_cluster_role_binding_inventory(
                &[healthy_cluster_role_binding()],
                pillar,
                now(),
            );

            assert_eq!(report.resources_evaluated, 1);
            assert_eq!(report.stale_resources, 0);
            assert!(report.findings.is_empty());
        }
    }
}

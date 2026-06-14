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

// Deterministic Kubernetes namespace inventory evaluator for roadmap rows
// 02-KUBERNETES-DASHBOARD-00050/00057/00078.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::services::aws::inventory::types::{
    score_pillar, InventoryFinding, Pillar, PillarReport, Severity, COST_ALLOCATION_TAG_KEYS,
    DEFAULT_STALE_AFTER_HOURS,
};

pub const RESOURCE_TYPE: &str = "KubernetesNamespace";
pub const REASON_COST_OWNER_NOT_RECORDED: &str = "K8S_NAMESPACE_COST_OWNER_NOT_RECORDED";
pub const REASON_RES_MISSING_STATUS: &str = "K8S_NAMESPACE_RES_MISSING_STATUS";
pub const REASON_RES_TERMINATING_STATUS: &str = "K8S_NAMESPACE_RES_TERMINATING_STATUS";
pub const REASON_SEC_POD_SECURITY_NOT_ENFORCED: &str =
    "K8S_NAMESPACE_SEC_POD_SECURITY_NOT_ENFORCED";
pub const REASON_INV_STALE_DATA: &str = "K8S_NAMESPACE_INV_STALE_DATA";

const POD_SECURITY_ENFORCE_LABEL: &str = "pod-security.kubernetes.io/enforce";
const ACCEPTED_POD_SECURITY_LEVELS: &[&str] = &["baseline", "restricted"];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NamespaceInventoryItem {
    pub cluster_id: String,
    pub name: String,
    pub status: Option<String>,
    pub labels: BTreeMap<String, String>,
    pub annotations: BTreeMap<String, String>,
    pub created_at: Option<DateTime<Utc>>,
    pub collected_at: DateTime<Utc>,
}

pub fn evaluate_kubernetes_namespace_inventory(
    namespaces: &[NamespaceInventoryItem],
    pillar: Pillar,
    now: DateTime<Utc>,
) -> PillarReport {
    let mut stale_resources = 0;
    let mut findings = Vec::new();

    for namespace in namespaces {
        if let Some(finding) = stale_finding(namespace, pillar, now) {
            stale_resources += 1;
            findings.push(finding);
        }

        match pillar {
            Pillar::Cost => evaluate_cost(namespace, pillar, &mut findings),
            Pillar::Resilience => evaluate_resilience(namespace, pillar, &mut findings),
            Pillar::Security => evaluate_security(namespace, pillar, &mut findings),
            _ => {}
        }
    }

    PillarReport {
        pillar,
        resources_evaluated: namespaces.len(),
        stale_resources,
        score: score_pillar(&findings),
        findings,
    }
}

fn evaluate_cost(
    namespace: &NamespaceInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if has_any_metadata_key(&namespace.labels, COST_ALLOCATION_TAG_KEYS)
        || has_any_metadata_key(&namespace.annotations, COST_ALLOCATION_TAG_KEYS)
    {
        return;
    }

    findings.push(finding(
        namespace,
        pillar,
        REASON_COST_OWNER_NOT_RECORDED,
        Severity::Medium,
        format!(
            "Kubernetes namespace {} has no owner, team, project, or cost-center label or annotation",
            namespace.name
        ),
        json!({
            "cluster_id": namespace.cluster_id,
            "namespace": namespace.name,
            "checked_keys": COST_ALLOCATION_TAG_KEYS,
            "checked_locations": ["labels", "annotations"],
        }),
    ));
}

fn evaluate_resilience(
    namespace: &NamespaceInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    match namespace.status.as_deref() {
        None | Some("") => findings.push(finding(
            namespace,
            pillar,
            REASON_RES_MISSING_STATUS,
            Severity::Medium,
            format!(
                "Kubernetes namespace {} has no recorded status",
                namespace.name
            ),
            json!({
                "cluster_id": namespace.cluster_id,
                "namespace": namespace.name,
                "status": namespace.status,
            }),
        )),
        Some(status) if status.eq_ignore_ascii_case("terminating") => findings.push(finding(
            namespace,
            pillar,
            REASON_RES_TERMINATING_STATUS,
            Severity::High,
            format!(
                "Kubernetes namespace {} is terminating and cannot be treated as healthy inventory",
                namespace.name
            ),
            json!({
                "cluster_id": namespace.cluster_id,
                "namespace": namespace.name,
                "status": status,
            }),
        )),
        _ => {}
    }
}

fn evaluate_security(
    namespace: &NamespaceInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    let enforce_level = metadata_value(&namespace.labels, POD_SECURITY_ENFORCE_LABEL);
    let accepted = enforce_level
        .as_deref()
        .map(|level| {
            ACCEPTED_POD_SECURITY_LEVELS
                .iter()
                .any(|accepted| level.eq_ignore_ascii_case(accepted))
        })
        .unwrap_or(false);

    if accepted {
        return;
    }

    findings.push(finding(
        namespace,
        pillar,
        REASON_SEC_POD_SECURITY_NOT_ENFORCED,
        Severity::High,
        format!(
            "Kubernetes namespace {} does not enforce baseline or restricted pod security",
            namespace.name
        ),
        json!({
            "cluster_id": namespace.cluster_id,
            "namespace": namespace.name,
            "pod_security_enforce": enforce_level,
            "accepted_levels": ACCEPTED_POD_SECURITY_LEVELS,
        }),
    ));
}

fn stale_finding(
    namespace: &NamespaceInventoryItem,
    pillar: Pillar,
    now: DateTime<Utc>,
) -> Option<InventoryFinding> {
    let age_hours = (now - namespace.collected_at).num_hours();
    if age_hours <= DEFAULT_STALE_AFTER_HOURS {
        return None;
    }

    Some(finding(
        namespace,
        pillar,
        REASON_INV_STALE_DATA,
        Severity::Medium,
        format!(
            "Inventory data for Kubernetes namespace {} is {} hours old (threshold {} hours)",
            namespace.name, age_hours, DEFAULT_STALE_AFTER_HOURS
        ),
        json!({
            "cluster_id": namespace.cluster_id,
            "namespace": namespace.name,
            "collected_at": namespace.collected_at,
            "age_hours": age_hours,
            "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
        }),
    ))
}

fn finding(
    namespace: &NamespaceInventoryItem,
    pillar: Pillar,
    reason_code: &str,
    severity: Severity,
    message: String,
    evidence: Value,
) -> InventoryFinding {
    InventoryFinding {
        resource_id: format!("{}/{}", namespace.cluster_id, namespace.name),
        arn: String::new(),
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

    fn namespace(
        name: &str,
        status: Option<&str>,
        labels: BTreeMap<String, String>,
        annotations: BTreeMap<String, String>,
        collected_at: DateTime<Utc>,
    ) -> NamespaceInventoryItem {
        NamespaceInventoryItem {
            cluster_id: "cluster-1".to_string(),
            name: name.to_string(),
            status: status.map(str::to_string),
            labels,
            annotations,
            created_at: Some(now() - Duration::days(10)),
            collected_at,
        }
    }

    fn healthy_namespace() -> NamespaceInventoryItem {
        namespace(
            "payments",
            Some("Active"),
            labels(&[
                ("owner", "platform"),
                ("cost-center", "cc-42"),
                ("pod-security.kubernetes.io/enforce", "restricted"),
            ]),
            BTreeMap::new(),
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
        let ns = namespace(
            "unowned",
            Some("Active"),
            BTreeMap::new(),
            BTreeMap::new(),
            now(),
        );

        let report = evaluate_kubernetes_namespace_inventory(&[ns], Pillar::Cost, now());

        assert_eq!(report.resources_evaluated, 1);
        assert!(reason_codes(&report).contains(&REASON_COST_OWNER_NOT_RECORDED));
    }

    #[test]
    fn resilience_flags_missing_and_terminating_status() {
        let missing = namespace(
            "unknown",
            None,
            labels(&[("owner", "platform")]),
            BTreeMap::new(),
            now(),
        );
        let terminating = namespace(
            "deleting",
            Some("Terminating"),
            labels(&[("owner", "platform")]),
            BTreeMap::new(),
            now(),
        );

        let report = evaluate_kubernetes_namespace_inventory(
            &[missing, terminating],
            Pillar::Resilience,
            now(),
        );
        let codes = reason_codes(&report);

        assert!(codes.contains(&REASON_RES_MISSING_STATUS));
        assert!(codes.contains(&REASON_RES_TERMINATING_STATUS));
    }

    #[test]
    fn security_flags_missing_or_privileged_pod_security_enforcement() {
        let missing = namespace(
            "missing-policy",
            Some("Active"),
            labels(&[("owner", "platform")]),
            BTreeMap::new(),
            now(),
        );
        let privileged = namespace(
            "privileged",
            Some("Active"),
            labels(&[
                ("owner", "platform"),
                ("pod-security.kubernetes.io/enforce", "privileged"),
            ]),
            BTreeMap::new(),
            now(),
        );

        let report = evaluate_kubernetes_namespace_inventory(
            &[missing, privileged],
            Pillar::Security,
            now(),
        );

        assert_eq!(
            reason_codes(&report)
                .iter()
                .filter(|code| **code == REASON_SEC_POD_SECURITY_NOT_ENFORCED)
                .count(),
            2
        );
    }

    #[test]
    fn stale_inventory_is_counted_for_any_pillar() {
        let ns = namespace(
            "stale",
            Some("Active"),
            labels(&[
                ("owner", "platform"),
                ("pod-security.kubernetes.io/enforce", "restricted"),
            ]),
            BTreeMap::new(),
            now() - Duration::hours(25),
        );

        let report = evaluate_kubernetes_namespace_inventory(&[ns], Pillar::Cost, now());

        assert_eq!(report.stale_resources, 1);
        assert!(reason_codes(&report).contains(&REASON_INV_STALE_DATA));
    }

    #[test]
    fn healthy_namespace_passes_claimed_pillars() {
        let ns = healthy_namespace();

        for pillar in [Pillar::Cost, Pillar::Resilience, Pillar::Security] {
            let report =
                evaluate_kubernetes_namespace_inventory(std::slice::from_ref(&ns), pillar, now());
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

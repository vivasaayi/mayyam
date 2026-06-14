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

// Deterministic Kubernetes Pod Security Standards inventory evaluator for
// roadmap rows 02-KUBERNETES-DASHBOARD-01863/01870/01891.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::services::aws::inventory::types::{
    score_pillar, InventoryFinding, Pillar, PillarReport, Severity, COST_ALLOCATION_TAG_KEYS,
    DEFAULT_STALE_AFTER_HOURS,
};

pub const RESOURCE_TYPE: &str = "KubernetesPodSecurityStandard";
pub const REASON_COST_OWNER_NOT_RECORDED: &str = "K8S_PSS_COST_OWNER_NOT_RECORDED";
pub const REASON_RES_MISSING_STATUS: &str = "K8S_PSS_RES_MISSING_STATUS";
pub const REASON_RES_TERMINATING_STATUS: &str = "K8S_PSS_RES_TERMINATING_STATUS";
pub const REASON_RES_UNPINNED_ENFORCE_VERSION: &str = "K8S_PSS_RES_UNPINNED_ENFORCE_VERSION";
pub const REASON_RES_NO_ROLLOUT_GUARDRAIL: &str = "K8S_PSS_RES_NO_ROLLOUT_GUARDRAIL";
pub const REASON_SEC_ENFORCE_NOT_RESTRICTIVE: &str = "K8S_PSS_SEC_ENFORCE_NOT_RESTRICTIVE";
pub const REASON_INV_STALE_DATA: &str = "K8S_PSS_INV_STALE_DATA";

pub const POD_SECURITY_ENFORCE_LABEL: &str = "pod-security.kubernetes.io/enforce";
pub const POD_SECURITY_ENFORCE_VERSION_LABEL: &str = "pod-security.kubernetes.io/enforce-version";
pub const POD_SECURITY_AUDIT_LABEL: &str = "pod-security.kubernetes.io/audit";
pub const POD_SECURITY_AUDIT_VERSION_LABEL: &str = "pod-security.kubernetes.io/audit-version";
pub const POD_SECURITY_WARN_LABEL: &str = "pod-security.kubernetes.io/warn";
pub const POD_SECURITY_WARN_VERSION_LABEL: &str = "pod-security.kubernetes.io/warn-version";

const ACCEPTED_ENFORCE_LEVELS: &[&str] = &["baseline", "restricted"];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PodSecurityStandardsInventoryItem {
    pub cluster_id: String,
    pub namespace: String,
    pub status: Option<String>,
    pub labels: BTreeMap<String, String>,
    pub annotations: BTreeMap<String, String>,
    pub enforce_level: Option<String>,
    pub enforce_version: Option<String>,
    pub audit_level: Option<String>,
    pub audit_version: Option<String>,
    pub warn_level: Option<String>,
    pub warn_version: Option<String>,
    pub created_at: Option<DateTime<Utc>>,
    pub collected_at: DateTime<Utc>,
}

pub fn evaluate_kubernetes_pod_security_standards_inventory(
    standards: &[PodSecurityStandardsInventoryItem],
    pillar: Pillar,
    now: DateTime<Utc>,
) -> PillarReport {
    let mut stale_resources = 0;
    let mut findings = Vec::new();

    for standard in standards {
        if let Some(finding) = stale_finding(standard, pillar, now) {
            stale_resources += 1;
            findings.push(finding);
        }

        match pillar {
            Pillar::Cost => evaluate_cost(standard, pillar, &mut findings),
            Pillar::Resilience => evaluate_resilience(standard, pillar, &mut findings),
            Pillar::Security => evaluate_security(standard, pillar, &mut findings),
            _ => {}
        }
    }

    PillarReport {
        pillar,
        resources_evaluated: standards.len(),
        stale_resources,
        score: score_pillar(&findings),
        findings,
    }
}

fn evaluate_cost(
    standard: &PodSecurityStandardsInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if has_any_metadata_key(&standard.labels, COST_ALLOCATION_TAG_KEYS)
        || has_any_metadata_key(&standard.annotations, COST_ALLOCATION_TAG_KEYS)
    {
        return;
    }

    findings.push(finding(
        standard,
        pillar,
        REASON_COST_OWNER_NOT_RECORDED,
        Severity::Medium,
        format!(
            "Kubernetes Pod Security Standards policy for namespace {} has no owner, team, project, or cost-center label or annotation",
            standard.namespace
        ),
        json!({
            "cluster_id": standard.cluster_id,
            "namespace": standard.namespace,
            "checked_keys": COST_ALLOCATION_TAG_KEYS,
            "checked_locations": ["labels", "annotations"],
            "enforce_level": standard.enforce_level,
            "audit_level": standard.audit_level,
            "warn_level": standard.warn_level,
        }),
    ));
}

fn evaluate_resilience(
    standard: &PodSecurityStandardsInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    match standard.status.as_deref().map(str::trim) {
        None | Some("") => findings.push(finding(
            standard,
            pillar,
            REASON_RES_MISSING_STATUS,
            Severity::Medium,
            format!(
                "Kubernetes Pod Security Standards policy for namespace {} has no recorded namespace status",
                standard.namespace
            ),
            json!({
                "cluster_id": standard.cluster_id,
                "namespace": standard.namespace,
                "status": standard.status,
            }),
        )),
        Some(status) if status.eq_ignore_ascii_case("terminating") => findings.push(finding(
            standard,
            pillar,
            REASON_RES_TERMINATING_STATUS,
            Severity::High,
            format!(
                "Kubernetes Pod Security Standards policy for namespace {} belongs to a terminating namespace",
                standard.namespace
            ),
            json!({
                "cluster_id": standard.cluster_id,
                "namespace": standard.namespace,
                "status": status,
            }),
        )),
        _ => {}
    }

    if standard
        .enforce_level
        .as_deref()
        .map(is_non_empty)
        .unwrap_or(false)
        && is_unpinned_version(standard.enforce_version.as_deref())
    {
        findings.push(finding(
            standard,
            pillar,
            REASON_RES_UNPINNED_ENFORCE_VERSION,
            Severity::Medium,
            format!(
                "Kubernetes Pod Security Standards policy for namespace {} uses an unpinned enforce version",
                standard.namespace
            ),
            json!({
                "cluster_id": standard.cluster_id,
                "namespace": standard.namespace,
                "enforce_level": standard.enforce_level,
                "enforce_version": standard.enforce_version,
                "recommendation": "Pin pod-security enforce-version to a tested Kubernetes minor version before cluster upgrades",
            }),
        ));
    }

    if standard
        .enforce_level
        .as_deref()
        .map(is_restrictive_level)
        .unwrap_or(false)
        && !has_rollout_guardrail(standard)
    {
        findings.push(finding(
            standard,
            pillar,
            REASON_RES_NO_ROLLOUT_GUARDRAIL,
            Severity::Medium,
            format!(
                "Kubernetes Pod Security Standards policy for namespace {} enforces without audit or warn guardrails",
                standard.namespace
            ),
            json!({
                "cluster_id": standard.cluster_id,
                "namespace": standard.namespace,
                "enforce_level": standard.enforce_level,
                "audit_level": standard.audit_level,
                "warn_level": standard.warn_level,
                "recommendation": "Set audit and warn Pod Security labels during rollout so deployment breakage is visible before enforcement changes",
            }),
        ));
    }
}

fn evaluate_security(
    standard: &PodSecurityStandardsInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if standard
        .enforce_level
        .as_deref()
        .map(is_restrictive_level)
        .unwrap_or(false)
    {
        return;
    }

    findings.push(finding(
        standard,
        pillar,
        REASON_SEC_ENFORCE_NOT_RESTRICTIVE,
        Severity::High,
        format!(
            "Kubernetes Pod Security Standards policy for namespace {} does not enforce baseline or restricted",
            standard.namespace
        ),
        json!({
            "cluster_id": standard.cluster_id,
            "namespace": standard.namespace,
            "enforce_level": standard.enforce_level,
            "enforce_version": standard.enforce_version,
            "accepted_enforce_levels": ACCEPTED_ENFORCE_LEVELS,
            "recommendation": "Set pod-security.kubernetes.io/enforce to baseline or restricted and pin enforce-version",
        }),
    ));
}

fn stale_finding(
    standard: &PodSecurityStandardsInventoryItem,
    pillar: Pillar,
    now: DateTime<Utc>,
) -> Option<InventoryFinding> {
    let age_hours = (now - standard.collected_at).num_hours();
    if age_hours <= DEFAULT_STALE_AFTER_HOURS {
        return None;
    }

    Some(finding(
        standard,
        pillar,
        REASON_INV_STALE_DATA,
        Severity::Medium,
        format!(
            "Inventory data for Kubernetes Pod Security Standards policy in namespace {} is {} hours old (threshold {} hours)",
            standard.namespace, age_hours, DEFAULT_STALE_AFTER_HOURS
        ),
        json!({
            "cluster_id": standard.cluster_id,
            "namespace": standard.namespace,
            "collected_at": standard.collected_at,
            "age_hours": age_hours,
            "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
        }),
    ))
}

fn finding(
    standard: &PodSecurityStandardsInventoryItem,
    pillar: Pillar,
    reason_code: &str,
    severity: Severity,
    message: String,
    evidence: Value,
) -> InventoryFinding {
    InventoryFinding {
        resource_id: format!(
            "{}/{}/pod-security-standards",
            standard.cluster_id, standard.namespace
        ),
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

fn has_rollout_guardrail(standard: &PodSecurityStandardsInventoryItem) -> bool {
    standard
        .audit_level
        .as_deref()
        .map(is_restrictive_level)
        .unwrap_or(false)
        || standard
            .warn_level
            .as_deref()
            .map(is_restrictive_level)
            .unwrap_or(false)
}

fn is_unpinned_version(version: Option<&str>) -> bool {
    version
        .map(str::trim)
        .filter(|version| !version.is_empty())
        .map(|version| version.eq_ignore_ascii_case("latest"))
        .unwrap_or(true)
}

fn is_restrictive_level(level: &str) -> bool {
    let level = level.trim();
    ACCEPTED_ENFORCE_LEVELS
        .iter()
        .any(|accepted| level.eq_ignore_ascii_case(accepted))
}

fn is_non_empty(value: &str) -> bool {
    !value.trim().is_empty()
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

    fn standard(
        namespace: &str,
        status: Option<&str>,
        labels: BTreeMap<String, String>,
        annotations: BTreeMap<String, String>,
        collected_at: DateTime<Utc>,
    ) -> PodSecurityStandardsInventoryItem {
        PodSecurityStandardsInventoryItem {
            cluster_id: "cluster-1".to_string(),
            namespace: namespace.to_string(),
            status: status.map(str::to_string),
            enforce_level: labels.get("pod-security.kubernetes.io/enforce").cloned(),
            enforce_version: labels
                .get("pod-security.kubernetes.io/enforce-version")
                .cloned(),
            audit_level: labels.get("pod-security.kubernetes.io/audit").cloned(),
            audit_version: labels
                .get("pod-security.kubernetes.io/audit-version")
                .cloned(),
            warn_level: labels.get("pod-security.kubernetes.io/warn").cloned(),
            warn_version: labels
                .get("pod-security.kubernetes.io/warn-version")
                .cloned(),
            labels,
            annotations,
            created_at: Some(now() - Duration::days(10)),
            collected_at,
        }
    }

    fn healthy_standard() -> PodSecurityStandardsInventoryItem {
        standard(
            "payments",
            Some("Active"),
            labels(&[
                ("owner", "platform"),
                ("cost-center", "cc-42"),
                ("pod-security.kubernetes.io/enforce", "restricted"),
                ("pod-security.kubernetes.io/enforce-version", "v1.30"),
                ("pod-security.kubernetes.io/audit", "restricted"),
                ("pod-security.kubernetes.io/audit-version", "v1.30"),
                ("pod-security.kubernetes.io/warn", "restricted"),
                ("pod-security.kubernetes.io/warn-version", "v1.30"),
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
    fn cost_flags_missing_owner_metadata() {
        let standard = standard(
            "unowned",
            Some("Active"),
            labels(&[
                ("pod-security.kubernetes.io/enforce", "restricted"),
                ("pod-security.kubernetes.io/enforce-version", "v1.30"),
            ]),
            BTreeMap::new(),
            now(),
        );

        let report =
            evaluate_kubernetes_pod_security_standards_inventory(&[standard], Pillar::Cost, now());

        assert_eq!(report.resources_evaluated, 1);
        assert!(reason_codes(&report).contains(&REASON_COST_OWNER_NOT_RECORDED));
    }

    #[test]
    fn resilience_flags_status_and_rollout_risks() {
        let missing_status = standard(
            "unknown",
            None,
            labels(&[
                ("owner", "platform"),
                ("pod-security.kubernetes.io/enforce", "baseline"),
            ]),
            BTreeMap::new(),
            now(),
        );
        let terminating = standard(
            "deleting",
            Some("Terminating"),
            labels(&[
                ("owner", "platform"),
                ("pod-security.kubernetes.io/enforce", "restricted"),
                ("pod-security.kubernetes.io/enforce-version", "latest"),
            ]),
            BTreeMap::new(),
            now(),
        );

        let report = evaluate_kubernetes_pod_security_standards_inventory(
            &[missing_status, terminating],
            Pillar::Resilience,
            now(),
        );
        let codes = reason_codes(&report);

        assert!(codes.contains(&REASON_RES_MISSING_STATUS));
        assert!(codes.contains(&REASON_RES_TERMINATING_STATUS));
        assert!(codes.contains(&REASON_RES_UNPINNED_ENFORCE_VERSION));
        assert!(codes.contains(&REASON_RES_NO_ROLLOUT_GUARDRAIL));
    }

    #[test]
    fn security_flags_missing_or_privileged_enforcement() {
        let missing = standard(
            "missing-policy",
            Some("Active"),
            labels(&[("owner", "platform")]),
            BTreeMap::new(),
            now(),
        );
        let privileged = standard(
            "privileged",
            Some("Active"),
            labels(&[
                ("owner", "platform"),
                ("pod-security.kubernetes.io/enforce", "privileged"),
            ]),
            BTreeMap::new(),
            now(),
        );

        let report = evaluate_kubernetes_pod_security_standards_inventory(
            &[missing, privileged],
            Pillar::Security,
            now(),
        );

        assert_eq!(
            reason_codes(&report)
                .iter()
                .filter(|code| **code == REASON_SEC_ENFORCE_NOT_RESTRICTIVE)
                .count(),
            2
        );
    }

    #[test]
    fn stale_inventory_is_counted_for_any_pillar() {
        let standard = standard(
            "stale",
            Some("Active"),
            labels(&[
                ("owner", "platform"),
                ("pod-security.kubernetes.io/enforce", "restricted"),
                ("pod-security.kubernetes.io/enforce-version", "v1.30"),
            ]),
            BTreeMap::new(),
            now() - Duration::hours(25),
        );

        let report =
            evaluate_kubernetes_pod_security_standards_inventory(&[standard], Pillar::Cost, now());

        assert_eq!(report.stale_resources, 1);
        assert!(reason_codes(&report).contains(&REASON_INV_STALE_DATA));
    }

    #[test]
    fn healthy_standard_passes_claimed_pillars() {
        let standard = healthy_standard();

        for pillar in [Pillar::Cost, Pillar::Resilience, Pillar::Security] {
            let report = evaluate_kubernetes_pod_security_standards_inventory(
                std::slice::from_ref(&standard),
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

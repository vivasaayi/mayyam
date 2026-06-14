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

// Deterministic Kubernetes ServiceAccount inventory evaluator for roadmap rows
// 02-KUBERNETES-DASHBOARD-00834/00841/00862.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::services::aws::inventory::types::{
    score_pillar, InventoryFinding, Pillar, PillarReport, Severity, COST_ALLOCATION_TAG_KEYS,
    DEFAULT_STALE_AFTER_HOURS,
};

pub const RESOURCE_TYPE: &str = "KubernetesServiceAccount";
pub const REASON_COST_OWNER_NOT_RECORDED: &str = "K8S_SERVICE_ACCOUNT_COST_OWNER_NOT_RECORDED";
pub const REASON_RES_DEFAULT_ACCOUNT: &str = "K8S_SERVICE_ACCOUNT_RES_DEFAULT_ACCOUNT";
pub const REASON_SEC_AUTOMOUNT_TOKEN_NOT_DISABLED: &str =
    "K8S_SERVICE_ACCOUNT_SEC_AUTOMOUNT_TOKEN_NOT_DISABLED";
pub const REASON_SEC_LEGACY_TOKEN_SECRETS: &str = "K8S_SERVICE_ACCOUNT_SEC_LEGACY_TOKEN_SECRETS";
pub const REASON_INV_STALE_DATA: &str = "K8S_SERVICE_ACCOUNT_INV_STALE_DATA";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceAccountOwnerReferenceInventoryItem {
    pub kind: Option<String>,
    pub name: String,
    pub controller: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceAccountInventoryItem {
    pub cluster_id: String,
    pub namespace: String,
    pub name: String,
    pub labels: BTreeMap<String, String>,
    pub annotations: BTreeMap<String, String>,
    pub automount_service_account_token: Option<bool>,
    pub image_pull_secret_names: Vec<String>,
    pub secret_names: Vec<String>,
    pub owner_references: Vec<ServiceAccountOwnerReferenceInventoryItem>,
    pub created_at: Option<DateTime<Utc>>,
    pub collected_at: DateTime<Utc>,
}

pub fn evaluate_kubernetes_service_account_inventory(
    service_accounts: &[ServiceAccountInventoryItem],
    pillar: Pillar,
    now: DateTime<Utc>,
) -> PillarReport {
    let mut stale_resources = 0;
    let mut findings = Vec::new();

    for service_account in service_accounts {
        if let Some(finding) = stale_finding(service_account, pillar, now) {
            stale_resources += 1;
            findings.push(finding);
        }

        match pillar {
            Pillar::Cost => evaluate_cost(service_account, pillar, &mut findings),
            Pillar::Resilience => evaluate_resilience(service_account, pillar, &mut findings),
            Pillar::Security => evaluate_security(service_account, pillar, &mut findings),
            _ => {}
        }
    }

    PillarReport {
        pillar,
        resources_evaluated: service_accounts.len(),
        stale_resources,
        score: score_pillar(&findings),
        findings,
    }
}

fn evaluate_cost(
    service_account: &ServiceAccountInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if has_any_metadata_key(&service_account.labels, COST_ALLOCATION_TAG_KEYS)
        || has_any_metadata_key(&service_account.annotations, COST_ALLOCATION_TAG_KEYS)
    {
        return;
    }

    findings.push(finding(
        service_account,
        pillar,
        REASON_COST_OWNER_NOT_RECORDED,
        Severity::Medium,
        format!(
            "Kubernetes ServiceAccount {}/{} has no owner, team, project, or cost-center label or annotation",
            service_account.namespace, service_account.name
        ),
        json!({
            "cluster_id": service_account.cluster_id,
            "namespace": service_account.namespace,
            "name": service_account.name,
            "image_pull_secret_names": service_account.image_pull_secret_names,
            "secret_names": service_account.secret_names,
            "checked_keys": COST_ALLOCATION_TAG_KEYS,
            "checked_locations": ["labels", "annotations"],
        }),
    ));
}

fn evaluate_resilience(
    service_account: &ServiceAccountInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if service_account.name == "default" {
        findings.push(finding(
            service_account,
            pillar,
            REASON_RES_DEFAULT_ACCOUNT,
            Severity::Medium,
            format!(
                "Kubernetes namespace {} relies on the default ServiceAccount instead of a workload-specific identity",
                service_account.namespace
            ),
            json!({
                "cluster_id": service_account.cluster_id,
                "namespace": service_account.namespace,
                "name": service_account.name,
                "owner_references": service_account.owner_references,
                "recommendation": "Create named ServiceAccounts per workload so credentials, image pull dependencies, and ownership can be changed without affecting the namespace default",
            }),
        ));
    }
}

fn evaluate_security(
    service_account: &ServiceAccountInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    let token_automount_effective = service_account
        .automount_service_account_token
        .unwrap_or(true);
    if token_automount_effective {
        findings.push(finding(
            service_account,
            pillar,
            REASON_SEC_AUTOMOUNT_TOKEN_NOT_DISABLED,
            Severity::High,
            format!(
                "Kubernetes ServiceAccount {}/{} does not explicitly disable service account token automount",
                service_account.namespace, service_account.name
            ),
            json!({
                "cluster_id": service_account.cluster_id,
                "namespace": service_account.namespace,
                "name": service_account.name,
                "automount_service_account_token": service_account.automount_service_account_token,
                "effective_automount_service_account_token": token_automount_effective,
                "recommendation": "Set automountServiceAccountToken=false by default and opt in only for workloads that need API access",
            }),
        ));
    }

    if !service_account.secret_names.is_empty() {
        findings.push(finding(
            service_account,
            pillar,
            REASON_SEC_LEGACY_TOKEN_SECRETS,
            Severity::High,
            format!(
                "Kubernetes ServiceAccount {}/{} references legacy token secrets",
                service_account.namespace, service_account.name
            ),
            json!({
                "cluster_id": service_account.cluster_id,
                "namespace": service_account.namespace,
                "name": service_account.name,
                "secret_names": service_account.secret_names,
                "values_redacted": true,
                "recommendation": "Prefer projected bound service account tokens with short expiration",
            }),
        ));
    }
}

fn stale_finding(
    service_account: &ServiceAccountInventoryItem,
    pillar: Pillar,
    now: DateTime<Utc>,
) -> Option<InventoryFinding> {
    let age_hours = (now - service_account.collected_at).num_hours();
    if age_hours <= DEFAULT_STALE_AFTER_HOURS {
        return None;
    }

    Some(finding(
        service_account,
        pillar,
        REASON_INV_STALE_DATA,
        Severity::Medium,
        format!(
            "Inventory data for Kubernetes ServiceAccount {}/{} is {} hours old (threshold {} hours)",
            service_account.namespace, service_account.name, age_hours, DEFAULT_STALE_AFTER_HOURS
        ),
        json!({
            "cluster_id": service_account.cluster_id,
            "namespace": service_account.namespace,
            "name": service_account.name,
            "collected_at": service_account.collected_at,
            "age_hours": age_hours,
            "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
        }),
    ))
}

fn finding(
    service_account: &ServiceAccountInventoryItem,
    pillar: Pillar,
    reason_code: &str,
    severity: Severity,
    message: String,
    evidence: Value,
) -> InventoryFinding {
    InventoryFinding {
        resource_id: format!(
            "{}/{}/ServiceAccount/{}",
            service_account.cluster_id, service_account.namespace, service_account.name
        ),
        arn: format!(
            "kubernetes://serviceaccounts/{}/{}/{}",
            service_account.cluster_id, service_account.namespace, service_account.name
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

    fn service_account(
        name: &str,
        metadata_labels: BTreeMap<String, String>,
    ) -> ServiceAccountInventoryItem {
        ServiceAccountInventoryItem {
            cluster_id: "cluster-a".to_string(),
            namespace: "apps".to_string(),
            name: name.to_string(),
            labels: metadata_labels,
            annotations: BTreeMap::new(),
            automount_service_account_token: Some(false),
            image_pull_secret_names: vec!["registry-creds".to_string()],
            secret_names: Vec::new(),
            owner_references: Vec::new(),
            created_at: Some(now() - Duration::days(3)),
            collected_at: now(),
        }
    }

    fn healthy_service_account() -> ServiceAccountInventoryItem {
        service_account("checkout", labels(&[("team", "payments")]))
    }

    #[test]
    fn cost_flags_missing_owner_and_cost_allocation_labels() {
        let report = evaluate_kubernetes_service_account_inventory(
            &[service_account("untagged", BTreeMap::new())],
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
    fn resilience_flags_default_service_accounts() {
        let report = evaluate_kubernetes_service_account_inventory(
            &[service_account("default", labels(&[("team", "platform")]))],
            Pillar::Resilience,
            now(),
        );

        assert_eq!(report.findings.len(), 1);
        assert_eq!(report.findings[0].reason_code, REASON_RES_DEFAULT_ACCOUNT);
    }

    #[test]
    fn security_flags_automount_and_legacy_token_secrets() {
        let mut exposed = healthy_service_account();
        exposed.automount_service_account_token = None;
        exposed.secret_names = vec!["checkout-token".to_string()];

        let report =
            evaluate_kubernetes_service_account_inventory(&[exposed], Pillar::Security, now());
        let reason_codes = report
            .findings
            .iter()
            .map(|finding| finding.reason_code.as_str())
            .collect::<Vec<_>>();

        assert!(reason_codes.contains(&REASON_SEC_AUTOMOUNT_TOKEN_NOT_DISABLED));
        assert!(reason_codes.contains(&REASON_SEC_LEGACY_TOKEN_SECRETS));
    }

    #[test]
    fn stale_inventory_is_counted_for_any_pillar() {
        let mut stale = healthy_service_account();
        stale.collected_at = now() - Duration::hours(DEFAULT_STALE_AFTER_HOURS + 2);

        let report = evaluate_kubernetes_service_account_inventory(&[stale], Pillar::Cost, now());

        assert_eq!(report.stale_resources, 1);
        assert_eq!(report.findings[0].reason_code, REASON_INV_STALE_DATA);
    }

    #[test]
    fn healthy_service_accounts_pass_claimed_pillars() {
        for pillar in [Pillar::Cost, Pillar::Resilience, Pillar::Security] {
            let report = evaluate_kubernetes_service_account_inventory(
                &[healthy_service_account()],
                pillar,
                now(),
            );

            assert_eq!(report.resources_evaluated, 1);
            assert_eq!(report.stale_resources, 0);
            assert!(report.findings.is_empty());
        }
    }
}

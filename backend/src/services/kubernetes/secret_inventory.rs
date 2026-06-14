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

// Deterministic Kubernetes Secret inventory evaluator for roadmap rows
// 02-KUBERNETES-DASHBOARD-00785/00792/00813.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::services::aws::inventory::types::{
    score_pillar, InventoryFinding, Pillar, PillarReport, Severity, COST_ALLOCATION_TAG_KEYS,
    DEFAULT_STALE_AFTER_HOURS,
};

pub const RESOURCE_TYPE: &str = "KubernetesSecret";
pub const REASON_COST_OWNER_NOT_RECORDED: &str = "K8S_SECRET_COST_OWNER_NOT_RECORDED";
pub const REASON_RES_MUTABLE_DATA: &str = "K8S_SECRET_RES_MUTABLE_DATA";
pub const REASON_RES_EMPTY_DATA: &str = "K8S_SECRET_RES_EMPTY_DATA";
pub const REASON_SEC_SERVICE_ACCOUNT_TOKEN_SECRET: &str =
    "K8S_SECRET_SEC_SERVICE_ACCOUNT_TOKEN_SECRET";
pub const REASON_INV_STALE_DATA: &str = "K8S_SECRET_INV_STALE_DATA";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecretOwnerReferenceInventoryItem {
    pub kind: Option<String>,
    pub name: String,
    pub controller: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecretInventoryItem {
    pub cluster_id: String,
    pub namespace: String,
    pub name: String,
    pub secret_type: Option<String>,
    pub labels: BTreeMap<String, String>,
    pub annotations: BTreeMap<String, String>,
    pub data_keys: Vec<String>,
    pub string_data_keys: Vec<String>,
    pub total_data_bytes: usize,
    pub immutable: bool,
    pub owner_references: Vec<SecretOwnerReferenceInventoryItem>,
    pub created_at: Option<DateTime<Utc>>,
    pub collected_at: DateTime<Utc>,
}

pub fn evaluate_kubernetes_secret_inventory(
    secrets: &[SecretInventoryItem],
    pillar: Pillar,
    now: DateTime<Utc>,
) -> PillarReport {
    let mut stale_resources = 0;
    let mut findings = Vec::new();

    for secret in secrets {
        if let Some(finding) = stale_finding(secret, pillar, now) {
            stale_resources += 1;
            findings.push(finding);
        }

        match pillar {
            Pillar::Cost => evaluate_cost(secret, pillar, &mut findings),
            Pillar::Resilience => evaluate_resilience(secret, pillar, &mut findings),
            Pillar::Security => evaluate_security(secret, pillar, &mut findings),
            _ => {}
        }
    }

    PillarReport {
        pillar,
        resources_evaluated: secrets.len(),
        stale_resources,
        score: score_pillar(&findings),
        findings,
    }
}

fn evaluate_cost(
    secret: &SecretInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if has_any_metadata_key(&secret.labels, COST_ALLOCATION_TAG_KEYS)
        || has_any_metadata_key(&secret.annotations, COST_ALLOCATION_TAG_KEYS)
    {
        return;
    }

    findings.push(finding(
        secret,
        pillar,
        REASON_COST_OWNER_NOT_RECORDED,
        Severity::Medium,
        format!(
            "Kubernetes Secret {}/{} has no owner, team, project, or cost-center label or annotation",
            secret.namespace, secret.name
        ),
        json!({
            "cluster_id": secret.cluster_id,
            "namespace": secret.namespace,
            "name": secret.name,
            "secret_type": secret.secret_type,
            "data_keys": secret.data_keys,
            "string_data_keys": secret.string_data_keys,
            "total_data_bytes": secret.total_data_bytes,
            "checked_keys": COST_ALLOCATION_TAG_KEYS,
            "checked_locations": ["labels", "annotations"],
            "values_redacted": true,
        }),
    ));
}

fn evaluate_resilience(
    secret: &SecretInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if !secret.immutable {
        findings.push(finding(
            secret,
            pillar,
            REASON_RES_MUTABLE_DATA,
            Severity::Medium,
            format!(
                "Kubernetes Secret {}/{} is mutable and can drift without a rollout boundary",
                secret.namespace, secret.name
            ),
            json!({
                "cluster_id": secret.cluster_id,
                "namespace": secret.namespace,
                "name": secret.name,
                "secret_type": secret.secret_type,
                "immutable": secret.immutable,
                "owner_references": secret.owner_references,
                "values_redacted": true,
            }),
        ));
    }

    if secret.data_keys.is_empty() && secret.string_data_keys.is_empty() {
        findings.push(finding(
            secret,
            pillar,
            REASON_RES_EMPTY_DATA,
            Severity::Medium,
            format!(
                "Kubernetes Secret {}/{} has no data or stringData keys",
                secret.namespace, secret.name
            ),
            json!({
                "cluster_id": secret.cluster_id,
                "namespace": secret.namespace,
                "name": secret.name,
                "secret_type": secret.secret_type,
                "total_data_bytes": secret.total_data_bytes,
                "values_redacted": true,
            }),
        ));
    }
}

fn evaluate_security(
    secret: &SecretInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    let is_service_account_token = secret
        .secret_type
        .as_deref()
        .map(|secret_type| secret_type.eq_ignore_ascii_case("kubernetes.io/service-account-token"))
        .unwrap_or(false);
    if is_service_account_token {
        findings.push(finding(
            secret,
            pillar,
            REASON_SEC_SERVICE_ACCOUNT_TOKEN_SECRET,
            Severity::High,
            format!(
                "Kubernetes Secret {}/{} is a legacy service account token secret",
                secret.namespace, secret.name
            ),
            json!({
                "cluster_id": secret.cluster_id,
                "namespace": secret.namespace,
                "name": secret.name,
                "secret_type": secret.secret_type,
                "data_keys": secret.data_keys,
                "recommendation": "Prefer projected bound service account tokens with short expiration",
                "values_redacted": true,
            }),
        ));
    }
}

fn stale_finding(
    secret: &SecretInventoryItem,
    pillar: Pillar,
    now: DateTime<Utc>,
) -> Option<InventoryFinding> {
    let age_hours = (now - secret.collected_at).num_hours();
    if age_hours <= DEFAULT_STALE_AFTER_HOURS {
        return None;
    }

    Some(finding(
        secret,
        pillar,
        REASON_INV_STALE_DATA,
        Severity::Medium,
        format!(
            "Inventory data for Kubernetes Secret {}/{} is {} hours old (threshold {} hours)",
            secret.namespace, secret.name, age_hours, DEFAULT_STALE_AFTER_HOURS
        ),
        json!({
            "cluster_id": secret.cluster_id,
            "namespace": secret.namespace,
            "name": secret.name,
            "secret_type": secret.secret_type,
            "collected_at": secret.collected_at,
            "age_hours": age_hours,
            "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
            "values_redacted": true,
        }),
    ))
}

fn finding(
    secret: &SecretInventoryItem,
    pillar: Pillar,
    reason_code: &str,
    severity: Severity,
    message: String,
    evidence: Value,
) -> InventoryFinding {
    InventoryFinding {
        resource_id: format!(
            "{}/{}/Secret/{}",
            secret.cluster_id, secret.namespace, secret.name
        ),
        arn: format!(
            "kubernetes://secrets/{}/{}/{}",
            secret.cluster_id, secret.namespace, secret.name
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

    fn owner(kind: &str, name: &str) -> SecretOwnerReferenceInventoryItem {
        SecretOwnerReferenceInventoryItem {
            kind: Some(kind.to_string()),
            name: name.to_string(),
            controller: Some(true),
        }
    }

    fn secret(name: &str, metadata_labels: BTreeMap<String, String>) -> SecretInventoryItem {
        SecretInventoryItem {
            cluster_id: "cluster-a".to_string(),
            namespace: "apps".to_string(),
            name: name.to_string(),
            secret_type: Some("kubernetes.io/tls".to_string()),
            labels: metadata_labels,
            annotations: BTreeMap::new(),
            data_keys: vec!["tls.crt".to_string(), "tls.key".to_string()],
            string_data_keys: Vec::new(),
            total_data_bytes: 512,
            immutable: true,
            owner_references: vec![owner("Deployment", "checkout")],
            created_at: Some(now() - Duration::days(3)),
            collected_at: now(),
        }
    }

    fn healthy_secret() -> SecretInventoryItem {
        secret("checkout-tls", labels(&[("team", "payments")]))
    }

    #[test]
    fn cost_flags_missing_owner_and_cost_allocation_labels() {
        let report = evaluate_kubernetes_secret_inventory(
            &[secret("untagged", BTreeMap::new())],
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
    fn resilience_flags_mutable_and_empty_secrets() {
        let mut risky = healthy_secret();
        risky.immutable = false;
        risky.data_keys = Vec::new();
        risky.string_data_keys = Vec::new();
        risky.total_data_bytes = 0;

        let report = evaluate_kubernetes_secret_inventory(&[risky], Pillar::Resilience, now());
        let reason_codes = report
            .findings
            .iter()
            .map(|finding| finding.reason_code.as_str())
            .collect::<Vec<_>>();

        assert!(reason_codes.contains(&REASON_RES_MUTABLE_DATA));
        assert!(reason_codes.contains(&REASON_RES_EMPTY_DATA));
    }

    #[test]
    fn security_flags_legacy_service_account_token_secrets() {
        let mut exposed = healthy_secret();
        exposed.secret_type = Some("kubernetes.io/service-account-token".to_string());
        exposed.data_keys = vec!["token".to_string(), "ca.crt".to_string()];
        exposed.total_data_bytes = 1024;

        let report = evaluate_kubernetes_secret_inventory(&[exposed], Pillar::Security, now());
        let reason_codes = report
            .findings
            .iter()
            .map(|finding| finding.reason_code.as_str())
            .collect::<Vec<_>>();

        assert!(reason_codes.contains(&REASON_SEC_SERVICE_ACCOUNT_TOKEN_SECRET));
    }

    #[test]
    fn stale_inventory_is_counted_for_any_pillar() {
        let mut stale = healthy_secret();
        stale.collected_at = now() - Duration::hours(DEFAULT_STALE_AFTER_HOURS + 2);

        let report = evaluate_kubernetes_secret_inventory(&[stale], Pillar::Cost, now());

        assert_eq!(report.stale_resources, 1);
        assert_eq!(report.findings[0].reason_code, REASON_INV_STALE_DATA);
    }

    #[test]
    fn healthy_secrets_pass_claimed_pillars() {
        for pillar in [Pillar::Cost, Pillar::Resilience, Pillar::Security] {
            let report = evaluate_kubernetes_secret_inventory(&[healthy_secret()], pillar, now());

            assert_eq!(report.resources_evaluated, 1);
            assert_eq!(report.stale_resources, 0);
            assert!(report.findings.is_empty());
        }
    }
}

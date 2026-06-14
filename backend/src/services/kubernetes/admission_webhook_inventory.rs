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

// Deterministic Kubernetes Admission Webhook inventory evaluator for roadmap rows
// 02-KUBERNETES-DASHBOARD-01814/01821/01842.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::services::aws::inventory::types::{
    score_pillar, InventoryFinding, Pillar, PillarReport, Severity, COST_ALLOCATION_TAG_KEYS,
    DEFAULT_STALE_AFTER_HOURS,
};

pub const RESOURCE_TYPE: &str = "KubernetesAdmissionWebhook";
pub const REASON_COST_OWNER_NOT_RECORDED: &str = "K8S_ADMISSION_WEBHOOK_COST_OWNER_NOT_RECORDED";
pub const REASON_COST_BROAD_SCOPE: &str = "K8S_ADMISSION_WEBHOOK_COST_BROAD_SCOPE";
pub const REASON_RES_FAIL_CLOSED_LONG_TIMEOUT: &str =
    "K8S_ADMISSION_WEBHOOK_RES_FAIL_CLOSED_LONG_TIMEOUT";
pub const REASON_RES_NO_REVIEW_VERSIONS: &str = "K8S_ADMISSION_WEBHOOK_RES_NO_REVIEW_VERSIONS";
pub const REASON_SEC_INSECURE_CLIENT: &str = "K8S_ADMISSION_WEBHOOK_SEC_INSECURE_CLIENT";
pub const REASON_SEC_UNSAFE_SIDE_EFFECTS: &str = "K8S_ADMISSION_WEBHOOK_SEC_UNSAFE_SIDE_EFFECTS";
pub const REASON_INV_STALE_DATA: &str = "K8S_ADMISSION_WEBHOOK_INV_STALE_DATA";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdmissionWebhookInventoryItem {
    pub cluster_id: String,
    pub configuration_name: String,
    pub webhook_name: String,
    pub webhook_type: String,
    pub labels: BTreeMap<String, String>,
    pub annotations: BTreeMap<String, String>,
    pub failure_policy: Option<String>,
    pub match_policy: Option<String>,
    pub side_effects: Option<String>,
    pub timeout_seconds: Option<i32>,
    pub admission_review_versions: Vec<String>,
    pub namespace_selector_present: bool,
    pub object_selector_present: bool,
    pub rules_count: usize,
    pub operations: Vec<String>,
    pub api_groups: Vec<String>,
    pub api_versions: Vec<String>,
    pub resources: Vec<String>,
    pub scope: Option<String>,
    pub client_service_namespace: Option<String>,
    pub client_service_name: Option<String>,
    pub client_service_path: Option<String>,
    pub client_service_port: Option<i32>,
    pub client_url_host: Option<String>,
    pub client_url_scheme: Option<String>,
    pub ca_bundle_present: bool,
    pub created_at: Option<DateTime<Utc>>,
    pub collected_at: DateTime<Utc>,
}

pub fn evaluate_kubernetes_admission_webhook_inventory(
    webhooks: &[AdmissionWebhookInventoryItem],
    pillar: Pillar,
    now: DateTime<Utc>,
) -> PillarReport {
    let mut stale_resources = 0;
    let mut findings = Vec::new();

    for webhook in webhooks {
        if let Some(finding) = stale_finding(webhook, pillar, now) {
            stale_resources += 1;
            findings.push(finding);
        }

        match pillar {
            Pillar::Cost => evaluate_cost(webhook, pillar, &mut findings),
            Pillar::Resilience => evaluate_resilience(webhook, pillar, &mut findings),
            Pillar::Security => evaluate_security(webhook, pillar, &mut findings),
            _ => {}
        }
    }

    PillarReport {
        pillar,
        resources_evaluated: webhooks.len(),
        stale_resources,
        score: score_pillar(&findings),
        findings,
    }
}

fn evaluate_cost(
    webhook: &AdmissionWebhookInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if is_broad_scope(webhook) {
        findings.push(finding(
            webhook,
            pillar,
            REASON_COST_BROAD_SCOPE,
            Severity::Medium,
            format!(
                "Kubernetes Admission Webhook {}/{} applies broadly without namespace or object selectors",
                webhook.configuration_name, webhook.webhook_name
            ),
            json!({
                "cluster_id": webhook.cluster_id,
                "configuration_name": webhook.configuration_name,
                "webhook_name": webhook.webhook_name,
                "webhook_type": webhook.webhook_type,
                "operations": webhook.operations,
                "api_groups": webhook.api_groups,
                "api_versions": webhook.api_versions,
                "resources": webhook.resources,
                "namespace_selector_present": webhook.namespace_selector_present,
                "object_selector_present": webhook.object_selector_present,
                "rules_count": webhook.rules_count,
                "recommendation": "Scope admission webhooks to the smallest namespace, object, operation, and resource set to reduce admission latency and control-plane load",
            }),
        ));
    }

    if has_any_metadata_key(&webhook.labels, COST_ALLOCATION_TAG_KEYS)
        || has_any_metadata_key(&webhook.annotations, COST_ALLOCATION_TAG_KEYS)
    {
        return;
    }

    findings.push(finding(
        webhook,
        pillar,
        REASON_COST_OWNER_NOT_RECORDED,
        Severity::Medium,
        format!(
            "Kubernetes Admission Webhook {}/{} has no owner, team, project, or cost-center label or annotation",
            webhook.configuration_name, webhook.webhook_name
        ),
        json!({
            "cluster_id": webhook.cluster_id,
            "configuration_name": webhook.configuration_name,
            "webhook_name": webhook.webhook_name,
            "webhook_type": webhook.webhook_type,
            "checked_keys": COST_ALLOCATION_TAG_KEYS,
            "checked_locations": ["labels", "annotations"],
        }),
    ));
}

fn evaluate_resilience(
    webhook: &AdmissionWebhookInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    let timeout_seconds = webhook.timeout_seconds.unwrap_or(10);
    if is_fail_closed(webhook) && timeout_seconds >= 10 {
        findings.push(finding(
            webhook,
            pillar,
            REASON_RES_FAIL_CLOSED_LONG_TIMEOUT,
            Severity::High,
            format!(
                "Kubernetes Admission Webhook {}/{} fails closed with a {} second timeout",
                webhook.configuration_name, webhook.webhook_name, timeout_seconds
            ),
            json!({
                "cluster_id": webhook.cluster_id,
                "configuration_name": webhook.configuration_name,
                "webhook_name": webhook.webhook_name,
                "webhook_type": webhook.webhook_type,
                "failure_policy": webhook.failure_policy,
                "timeout_seconds": webhook.timeout_seconds,
                "client_service_namespace": webhook.client_service_namespace,
                "client_service_name": webhook.client_service_name,
                "client_url_host": webhook.client_url_host,
                "recommendation": "Use short timeouts and validate webhook endpoint availability before fail-closed enforcement",
            }),
        ));
    }

    if webhook.admission_review_versions.is_empty() {
        findings.push(finding(
            webhook,
            pillar,
            REASON_RES_NO_REVIEW_VERSIONS,
            Severity::Medium,
            format!(
                "Kubernetes Admission Webhook {}/{} does not advertise admissionReviewVersions",
                webhook.configuration_name, webhook.webhook_name
            ),
            json!({
                "cluster_id": webhook.cluster_id,
                "configuration_name": webhook.configuration_name,
                "webhook_name": webhook.webhook_name,
                "webhook_type": webhook.webhook_type,
                "admission_review_versions": webhook.admission_review_versions,
                "recommendation": "Publish supported admissionReviewVersions, including v1 for current Kubernetes clusters",
            }),
        ));
    }
}

fn evaluate_security(
    webhook: &AdmissionWebhookInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if has_insecure_client(webhook) {
        findings.push(finding(
            webhook,
            pillar,
            REASON_SEC_INSECURE_CLIENT,
            Severity::High,
            format!(
                "Kubernetes Admission Webhook {}/{} uses an insecure or unverifiable client endpoint",
                webhook.configuration_name, webhook.webhook_name
            ),
            json!({
                "cluster_id": webhook.cluster_id,
                "configuration_name": webhook.configuration_name,
                "webhook_name": webhook.webhook_name,
                "webhook_type": webhook.webhook_type,
                "client_service_namespace": webhook.client_service_namespace,
                "client_service_name": webhook.client_service_name,
                "client_url_host": webhook.client_url_host,
                "client_url_scheme": webhook.client_url_scheme,
                "ca_bundle_present": webhook.ca_bundle_present,
                "recommendation": "Use HTTPS webhook endpoints and provide CA bundle evidence for external admission webhooks",
            }),
        ));
    }

    if has_unsafe_side_effects(webhook) {
        findings.push(finding(
            webhook,
            pillar,
            REASON_SEC_UNSAFE_SIDE_EFFECTS,
            Severity::Medium,
            format!(
                "Kubernetes Admission Webhook {}/{} reports unsafe side effects for admission review",
                webhook.configuration_name, webhook.webhook_name
            ),
            json!({
                "cluster_id": webhook.cluster_id,
                "configuration_name": webhook.configuration_name,
                "webhook_name": webhook.webhook_name,
                "webhook_type": webhook.webhook_type,
                "side_effects": webhook.side_effects,
                "recommendation": "Set sideEffects to None or NoneOnDryRun and keep webhook handlers idempotent",
            }),
        ));
    }
}

fn stale_finding(
    webhook: &AdmissionWebhookInventoryItem,
    pillar: Pillar,
    now: DateTime<Utc>,
) -> Option<InventoryFinding> {
    let age_hours = (now - webhook.collected_at).num_hours();
    if age_hours <= DEFAULT_STALE_AFTER_HOURS {
        return None;
    }

    Some(finding(
        webhook,
        pillar,
        REASON_INV_STALE_DATA,
        Severity::Medium,
        format!(
            "Inventory data for Kubernetes Admission Webhook {}/{} is {} hours old (threshold {} hours)",
            webhook.configuration_name,
            webhook.webhook_name,
            age_hours,
            DEFAULT_STALE_AFTER_HOURS
        ),
        json!({
            "cluster_id": webhook.cluster_id,
            "configuration_name": webhook.configuration_name,
            "webhook_name": webhook.webhook_name,
            "webhook_type": webhook.webhook_type,
            "collected_at": webhook.collected_at,
            "age_hours": age_hours,
            "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
        }),
    ))
}

fn finding(
    webhook: &AdmissionWebhookInventoryItem,
    pillar: Pillar,
    reason_code: &str,
    severity: Severity,
    message: String,
    evidence: Value,
) -> InventoryFinding {
    InventoryFinding {
        resource_id: format!(
            "{}/AdmissionWebhook/{}/{}",
            webhook.cluster_id, webhook.configuration_name, webhook.webhook_name
        ),
        arn: format!(
            "kubernetes://admissionwebhooks/{}/{}/{}",
            webhook.cluster_id, webhook.configuration_name, webhook.webhook_name
        ),
        pillar,
        reason_code: reason_code.to_string(),
        severity,
        message,
        evidence,
    }
}

fn is_broad_scope(webhook: &AdmissionWebhookInventoryItem) -> bool {
    if webhook.namespace_selector_present || webhook.object_selector_present {
        return false;
    }

    let mutating_all_operations = contains_wildcard(&webhook.operations)
        || webhook
            .operations
            .iter()
            .any(|operation| matches_normalized(operation, &["CREATE", "UPDATE", "DELETE"]));
    let all_resources = contains_wildcard(&webhook.resources)
        || webhook
            .resources
            .iter()
            .any(|resource| resource.trim() == "*/*");
    mutating_all_operations && all_resources
}

fn is_fail_closed(webhook: &AdmissionWebhookInventoryItem) -> bool {
    webhook
        .failure_policy
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.eq_ignore_ascii_case("Fail"))
        .unwrap_or(true)
}

fn has_insecure_client(webhook: &AdmissionWebhookInventoryItem) -> bool {
    webhook
        .client_url_scheme
        .as_deref()
        .map(|scheme| !scheme.eq_ignore_ascii_case("https"))
        .unwrap_or(false)
        || (webhook.client_url_host.is_some() && !webhook.ca_bundle_present)
}

fn has_unsafe_side_effects(webhook: &AdmissionWebhookInventoryItem) -> bool {
    webhook
        .side_effects
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| {
            !value.eq_ignore_ascii_case("None") && !value.eq_ignore_ascii_case("NoneOnDryRun")
        })
        .unwrap_or(true)
}

fn contains_wildcard(values: &[String]) -> bool {
    values.iter().any(|value| value.trim() == "*")
}

fn matches_normalized(value: &str, wanted_values: &[&str]) -> bool {
    wanted_values
        .iter()
        .any(|wanted| value.trim().eq_ignore_ascii_case(wanted))
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

    fn healthy_webhook() -> AdmissionWebhookInventoryItem {
        AdmissionWebhookInventoryItem {
            cluster_id: "cluster-a".to_string(),
            configuration_name: "platform-validators".to_string(),
            webhook_name: "validate.platform.example.com".to_string(),
            webhook_type: "Validating".to_string(),
            labels: labels(&[("owner", "platform"), ("cost-center", "cc-42")]),
            annotations: BTreeMap::new(),
            failure_policy: Some("Ignore".to_string()),
            match_policy: Some("Equivalent".to_string()),
            side_effects: Some("None".to_string()),
            timeout_seconds: Some(5),
            admission_review_versions: vec!["v1".to_string()],
            namespace_selector_present: true,
            object_selector_present: false,
            rules_count: 1,
            operations: vec!["CREATE".to_string()],
            api_groups: vec!["apps".to_string()],
            api_versions: vec!["v1".to_string()],
            resources: vec!["deployments".to_string()],
            scope: Some("Namespaced".to_string()),
            client_service_namespace: Some("platform".to_string()),
            client_service_name: Some("webhook".to_string()),
            client_service_path: Some("/validate".to_string()),
            client_service_port: Some(443),
            client_url_host: None,
            client_url_scheme: None,
            ca_bundle_present: true,
            created_at: Some(now() - Duration::hours(2)),
            collected_at: now(),
        }
    }

    #[test]
    fn cost_flags_missing_owner_metadata() {
        let mut webhook = healthy_webhook();
        webhook.labels.clear();

        let report =
            evaluate_kubernetes_admission_webhook_inventory(&[webhook], Pillar::Cost, now());

        assert_eq!(report.resources_evaluated, 1);
        assert_eq!(report.findings.len(), 1);
        assert_eq!(
            report.findings[0].reason_code,
            REASON_COST_OWNER_NOT_RECORDED
        );
    }

    #[test]
    fn cost_flags_broad_unscoped_webhooks() {
        let mut webhook = healthy_webhook();
        webhook.namespace_selector_present = false;
        webhook.object_selector_present = false;
        webhook.operations = vec!["*".to_string()];
        webhook.api_groups = vec!["*".to_string()];
        webhook.api_versions = vec!["*".to_string()];
        webhook.resources = vec!["*".to_string()];

        let report =
            evaluate_kubernetes_admission_webhook_inventory(&[webhook], Pillar::Cost, now());

        assert_eq!(report.findings.len(), 1);
        assert_eq!(report.findings[0].reason_code, REASON_COST_BROAD_SCOPE);
    }

    #[test]
    fn resilience_flags_fail_closed_long_timeout() {
        let mut webhook = healthy_webhook();
        webhook.failure_policy = Some("Fail".to_string());
        webhook.timeout_seconds = Some(30);

        let report =
            evaluate_kubernetes_admission_webhook_inventory(&[webhook], Pillar::Resilience, now());

        assert_eq!(report.findings.len(), 1);
        assert_eq!(
            report.findings[0].reason_code,
            REASON_RES_FAIL_CLOSED_LONG_TIMEOUT
        );
    }

    #[test]
    fn security_flags_insecure_client_urls() {
        let mut webhook = healthy_webhook();
        webhook.client_service_namespace = None;
        webhook.client_service_name = None;
        webhook.client_url_scheme = Some("http".to_string());
        webhook.client_url_host = Some("webhook.example.com".to_string());
        webhook.ca_bundle_present = false;

        let report =
            evaluate_kubernetes_admission_webhook_inventory(&[webhook], Pillar::Security, now());

        assert_eq!(report.findings.len(), 1);
        assert_eq!(report.findings[0].reason_code, REASON_SEC_INSECURE_CLIENT);
    }

    #[test]
    fn stale_inventory_is_counted_for_any_pillar() {
        let mut webhook = healthy_webhook();
        webhook.collected_at = now() - Duration::hours(DEFAULT_STALE_AFTER_HOURS + 2);

        let report =
            evaluate_kubernetes_admission_webhook_inventory(&[webhook], Pillar::Cost, now());

        assert_eq!(report.stale_resources, 1);
        assert_eq!(report.findings[0].reason_code, REASON_INV_STALE_DATA);
    }

    #[test]
    fn healthy_webhooks_pass_claimed_pillars() {
        for pillar in [Pillar::Cost, Pillar::Resilience, Pillar::Security] {
            let report = evaluate_kubernetes_admission_webhook_inventory(
                &[healthy_webhook()],
                pillar,
                now(),
            );

            assert_eq!(report.resources_evaluated, 1);
            assert_eq!(report.stale_resources, 0);
            assert!(report.findings.is_empty());
        }
    }
}

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

// Deterministic Kafka SASL inventory evaluator for roadmap rows
// 04-KAFKA-DASHBOARD-MANAGEMENT-00687/00694/00715.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::config::KafkaClusterConfig;
use crate::services::aws::inventory::types::{
    score_pillar, InventoryFinding, Pillar, PillarReport, Severity, COST_ALLOCATION_TAG_KEYS,
    DEFAULT_STALE_AFTER_HOURS,
};

pub const RESOURCE_TYPE: &str = "KafkaSasl";
pub const REASON_COST_OWNER_NOT_RECORDED: &str = "KAFKA_SASL_COST_OWNER_NOT_RECORDED";
pub const REASON_COST_NO_SASL_EVIDENCE: &str = "KAFKA_SASL_COST_NO_EVIDENCE";
pub const REASON_COST_HIGH_AUTH_LISTENER_COUNT: &str = "KAFKA_SASL_COST_HIGH_AUTH_LISTENER_COUNT";
pub const REASON_RES_NO_SASL_EVIDENCE: &str = "KAFKA_SASL_RES_NO_EVIDENCE";
pub const REASON_RES_NO_AUDIT_EVIDENCE: &str = "KAFKA_SASL_RES_NO_AUDIT_EVIDENCE";
pub const REASON_RES_NO_MECHANISM: &str = "KAFKA_SASL_RES_NO_MECHANISM";
pub const REASON_SEC_NO_SASL_EVIDENCE: &str = "KAFKA_SASL_SEC_NO_EVIDENCE";
pub const REASON_SEC_NO_MECHANISM: &str = "KAFKA_SASL_SEC_NO_MECHANISM";
pub const REASON_SEC_MISSING_CREDENTIALS: &str = "KAFKA_SASL_SEC_MISSING_CREDENTIALS";
pub const REASON_SEC_PLAINTEXT: &str = "KAFKA_SASL_SEC_PLAINTEXT_PROTOCOL";
pub const REASON_INV_STALE_DATA: &str = "KAFKA_SASL_INV_STALE_DATA";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KafkaSaslInventoryItem {
    pub sasl_id: String,
    pub cluster_name: String,
    pub mechanism: Option<String>,
    pub username_present: bool,
    pub credentials_present: bool,
    pub owner: Option<String>,
    pub labels: BTreeMap<String, String>,
    pub authenticated_listener_count: Option<i32>,
    pub credential_rotation_days: Option<i64>,
    pub audit_evidence: bool,
    pub has_sasl_evidence: bool,
    pub security_protocol: String,
    pub collected_at: DateTime<Utc>,
}

pub fn evaluate_kafka_sasl_inventory(
    items: &[KafkaSaslInventoryItem],
    pillar: Pillar,
    now: DateTime<Utc>,
) -> PillarReport {
    let mut stale_resources = 0;
    let mut findings = Vec::new();

    for item in items {
        if let Some(finding) = stale_finding(item, pillar, now) {
            stale_resources += 1;
            findings.push(finding);
        }

        match pillar {
            Pillar::Cost => evaluate_cost(item, pillar, &mut findings),
            Pillar::Resilience => evaluate_resilience(item, pillar, &mut findings),
            Pillar::Security => evaluate_security(item, pillar, &mut findings),
            _ => {}
        }
    }

    PillarReport {
        pillar,
        resources_evaluated: items.len(),
        stale_resources,
        score: score_pillar(&findings),
        findings,
    }
}

pub fn sasl_inventory_item_from_config(
    cluster: &KafkaClusterConfig,
    collected_at: DateTime<Utc>,
) -> KafkaSaslInventoryItem {
    let protocol = cluster.security_protocol.trim();
    let mechanism = cluster.sasl_mechanism.as_ref().and_then(|value| {
        let trimmed = value.trim();
        (!trimmed.is_empty()).then(|| trimmed.to_string())
    });
    let username_present = cluster
        .sasl_username
        .as_deref()
        .is_some_and(|value| !value.trim().is_empty());
    let password_present = cluster
        .sasl_password
        .as_deref()
        .is_some_and(|value| !value.trim().is_empty());
    let has_sasl_evidence = protocol.starts_with("SASL") || mechanism.is_some() || username_present;

    KafkaSaslInventoryItem {
        sasl_id: format!("{}:sasl", cluster.name),
        cluster_name: cluster.name.clone(),
        mechanism,
        username_present,
        credentials_present: username_present && password_present,
        owner: None,
        labels: BTreeMap::new(),
        authenticated_listener_count: None,
        credential_rotation_days: None,
        audit_evidence: false,
        has_sasl_evidence,
        security_protocol: cluster.security_protocol.clone(),
        collected_at,
    }
}

fn evaluate_cost(
    item: &KafkaSaslInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if !has_owner_metadata(item) {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_OWNER_NOT_RECORDED,
            Severity::Medium,
            format!(
                "Kafka SASL inventory for cluster {} has no owner, team, project, or cost-center metadata",
                item.cluster_name
            ),
            json!({
                "sasl_id": item.sasl_id,
                "cluster_name": item.cluster_name,
                "checked_keys": COST_ALLOCATION_TAG_KEYS,
            }),
        ));
    }

    if !item.has_sasl_evidence {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_NO_SASL_EVIDENCE,
            Severity::High,
            format!(
                "Kafka SASL inventory for cluster {} has no SASL evidence",
                item.cluster_name
            ),
            json!({
                "sasl_id": item.sasl_id,
                "cluster_name": item.cluster_name,
                "recommendation": "Collect SASL mechanism, principal, listener, credential rotation, ownership, and audit evidence before estimating authentication operating cost",
            }),
        ));
    }

    if item
        .authenticated_listener_count
        .is_some_and(|count| count >= 10)
    {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_HIGH_AUTH_LISTENER_COUNT,
            Severity::Medium,
            format!(
                "Kafka SASL inventory {} has high authenticated listener volume",
                item.sasl_id
            ),
            json!({
                "sasl_id": item.sasl_id,
                "authenticated_listener_count": item.authenticated_listener_count,
                "recommendation": "Review listener sprawl, duplicate authentication paths, and credential management cost before adding more SASL endpoints",
            }),
        ));
    }
}

fn evaluate_resilience(
    item: &KafkaSaslInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if !item.has_sasl_evidence {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_NO_SASL_EVIDENCE,
            Severity::High,
            format!(
                "Kafka SASL inventory for cluster {} has no resilience evidence",
                item.cluster_name
            ),
            json!({
                "sasl_id": item.sasl_id,
                "cluster_name": item.cluster_name,
                "recommendation": "Collect SASL mechanism, listener, credential, and audit evidence before relying on authentication posture during recovery",
            }),
        ));
    }

    if item.has_sasl_evidence && !item.audit_evidence {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_NO_AUDIT_EVIDENCE,
            Severity::High,
            format!("Kafka SASL inventory {} has no audit evidence", item.sasl_id),
            json!({
                "sasl_id": item.sasl_id,
                "audit_evidence": item.audit_evidence,
                "recommendation": "Enable authentication and credential-change audit evidence before accepting SASL recovery posture",
            }),
        ));
    }

    if item.has_sasl_evidence && item.mechanism.as_deref().unwrap_or("").trim().is_empty() {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_NO_MECHANISM,
            Severity::High,
            format!(
                "Kafka SASL inventory for cluster {} has no mechanism recorded",
                item.cluster_name
            ),
            json!({
                "sasl_id": item.sasl_id,
                "mechanism": item.mechanism,
                "recommendation": "Record SASL mechanism so recovery runbooks can validate compatible clients and fallback paths",
            }),
        ));
    }
}

fn evaluate_security(
    item: &KafkaSaslInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if !item.has_sasl_evidence {
        findings.push(finding(
            item,
            pillar,
            REASON_SEC_NO_SASL_EVIDENCE,
            Severity::High,
            format!(
                "Kafka SASL inventory for cluster {} has no SASL evidence for security review",
                item.cluster_name
            ),
            json!({
                "sasl_id": item.sasl_id,
                "cluster_name": item.cluster_name,
                "recommendation": "Collect SASL mechanism, principals, credential state, listener protocols, and audit evidence before assessing authentication posture",
            }),
        ));
    }

    if item.has_sasl_evidence && item.mechanism.as_deref().unwrap_or("").trim().is_empty() {
        findings.push(finding(
            item,
            pillar,
            REASON_SEC_NO_MECHANISM,
            Severity::High,
            format!(
                "Kafka SASL inventory for cluster {} has no SASL mechanism",
                item.cluster_name
            ),
            json!({
                "sasl_id": item.sasl_id,
                "mechanism": item.mechanism,
                "recommendation": "Record and validate the SASL mechanism before accepting authentication posture",
            }),
        ));
    }

    if item.has_sasl_evidence && !item.credentials_present {
        findings.push(finding(
            item,
            pillar,
            REASON_SEC_MISSING_CREDENTIALS,
            Severity::High,
            format!(
                "Kafka SASL inventory for cluster {} has incomplete credential evidence",
                item.cluster_name
            ),
            json!({
                "sasl_id": item.sasl_id,
                "username_present": item.username_present,
                "credentials_present": item.credentials_present,
                "recommendation": "Verify SASL credentials are present, scoped, rotated, and stored outside static configuration where possible",
            }),
        ));
    }

    if item.security_protocol.eq_ignore_ascii_case("PLAINTEXT") {
        findings.push(finding(
            item,
            pillar,
            REASON_SEC_PLAINTEXT,
            Severity::High,
            format!(
                "Kafka SASL inventory for cluster {} uses PLAINTEXT transport",
                item.cluster_name
            ),
            json!({
                "sasl_id": item.sasl_id,
                "security_protocol": item.security_protocol,
                "recommendation": "Use SASL_SSL or SSL so authentication material and payloads are not sent in clear text",
            }),
        ));
    }
}

fn stale_finding(
    item: &KafkaSaslInventoryItem,
    pillar: Pillar,
    now: DateTime<Utc>,
) -> Option<InventoryFinding> {
    let age_hours = now
        .signed_duration_since(item.collected_at)
        .num_hours()
        .max(0);

    if age_hours <= DEFAULT_STALE_AFTER_HOURS {
        return None;
    }

    Some(finding(
        item,
        pillar,
        REASON_INV_STALE_DATA,
        Severity::High,
        format!(
            "Kafka SASL inventory for cluster {} is {} hour(s) old",
            item.cluster_name, age_hours
        ),
        json!({
            "sasl_id": item.sasl_id,
            "cluster_name": item.cluster_name,
            "age_hours": age_hours,
            "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
            "recommendation": "Refresh Kafka SASL inventory before acting on this posture report",
        }),
    ))
}

fn has_owner_metadata(item: &KafkaSaslInventoryItem) -> bool {
    item.owner
        .as_deref()
        .is_some_and(|owner| !owner.trim().is_empty())
        || COST_ALLOCATION_TAG_KEYS.iter().any(|key| {
            item.labels
                .get(*key)
                .or_else(|| item.labels.get(&key.to_ascii_lowercase()))
                .is_some_and(|value| !value.trim().is_empty())
        })
}

fn finding(
    item: &KafkaSaslInventoryItem,
    pillar: Pillar,
    reason_code: &'static str,
    severity: Severity,
    message: String,
    evidence: Value,
) -> InventoryFinding {
    InventoryFinding {
        resource_id: item.sasl_id.clone(),
        arn: format!("kafka:sasl/{}", item.sasl_id),
        pillar,
        severity,
        reason_code: reason_code.to_string(),
        message,
        evidence,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    fn sasl(now: DateTime<Utc>) -> KafkaSaslInventoryItem {
        KafkaSaslInventoryItem {
            sasl_id: "prod:sasl".to_string(),
            cluster_name: "prod".to_string(),
            mechanism: Some("SCRAM-SHA-512".to_string()),
            username_present: true,
            credentials_present: true,
            owner: Some("platform".to_string()),
            labels: BTreeMap::new(),
            authenticated_listener_count: Some(2),
            credential_rotation_days: Some(60),
            audit_evidence: true,
            has_sasl_evidence: true,
            security_protocol: "SASL_SSL".to_string(),
            collected_at: now,
        }
    }

    #[test]
    fn healthy_sasl_passes_claimed_pillars() {
        let now = Utc::now();
        let item = sasl(now);

        for pillar in [Pillar::Cost, Pillar::Resilience, Pillar::Security] {
            let report = evaluate_kafka_sasl_inventory(std::slice::from_ref(&item), pillar, now);
            assert_eq!(report.resources_evaluated, 1);
            assert!(report.findings.is_empty());
            assert_eq!(report.stale_resources, 0);
            assert_eq!(report.score, 100);
        }
    }

    #[test]
    fn cost_flags_missing_owner_missing_evidence_and_listener_sprawl() {
        let now = Utc::now();
        let mut item = sasl(now);
        item.owner = None;
        item.has_sasl_evidence = false;
        item.authenticated_listener_count = Some(12);

        let report = evaluate_kafka_sasl_inventory(&[item], Pillar::Cost, now);

        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_COST_OWNER_NOT_RECORDED));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_COST_NO_SASL_EVIDENCE));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_COST_HIGH_AUTH_LISTENER_COUNT));
    }

    #[test]
    fn resilience_flags_missing_evidence_audit_and_mechanism() {
        let now = Utc::now();
        let mut missing_evidence = sasl(now);
        missing_evidence.has_sasl_evidence = false;
        let mut incomplete = sasl(now);
        incomplete.audit_evidence = false;
        incomplete.mechanism = None;

        let report =
            evaluate_kafka_sasl_inventory(&[missing_evidence, incomplete], Pillar::Resilience, now);

        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_RES_NO_SASL_EVIDENCE));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_RES_NO_AUDIT_EVIDENCE));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_RES_NO_MECHANISM));
    }

    #[test]
    fn security_flags_missing_evidence_mechanism_credentials_and_plaintext() {
        let now = Utc::now();
        let mut missing_evidence = sasl(now);
        missing_evidence.has_sasl_evidence = false;
        let mut insecure = sasl(now);
        insecure.mechanism = None;
        insecure.credentials_present = false;
        insecure.security_protocol = "PLAINTEXT".to_string();

        let report =
            evaluate_kafka_sasl_inventory(&[missing_evidence, insecure], Pillar::Security, now);

        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_SEC_NO_SASL_EVIDENCE));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_SEC_NO_MECHANISM));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_SEC_MISSING_CREDENTIALS));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_SEC_PLAINTEXT));
    }

    #[test]
    fn stale_sasl_inventory_is_counted_for_any_pillar() {
        let now = Utc::now();
        let mut item = sasl(now);
        item.collected_at = now - Duration::hours(DEFAULT_STALE_AFTER_HOURS + 2);

        let report = evaluate_kafka_sasl_inventory(&[item], Pillar::Security, now);

        assert_eq!(report.stale_resources, 1);
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_INV_STALE_DATA));
    }
}

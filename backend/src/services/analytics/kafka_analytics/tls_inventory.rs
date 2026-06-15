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

// Deterministic Kafka TLS inventory evaluator for roadmap rows
// 04-KAFKA-DASHBOARD-MANAGEMENT-00736/00743/00764.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::config::KafkaClusterConfig;
use crate::services::aws::inventory::types::{
    score_pillar, InventoryFinding, Pillar, PillarReport, Severity, COST_ALLOCATION_TAG_KEYS,
    DEFAULT_STALE_AFTER_HOURS,
};

pub const RESOURCE_TYPE: &str = "KafkaTls";
pub const REASON_COST_OWNER_NOT_RECORDED: &str = "KAFKA_TLS_COST_OWNER_NOT_RECORDED";
pub const REASON_COST_NO_TLS_EVIDENCE: &str = "KAFKA_TLS_COST_NO_EVIDENCE";
pub const REASON_COST_HIGH_ENCRYPTED_LISTENER_COUNT: &str =
    "KAFKA_TLS_COST_HIGH_ENCRYPTED_LISTENER_COUNT";
pub const REASON_RES_NO_TLS_EVIDENCE: &str = "KAFKA_TLS_RES_NO_EVIDENCE";
pub const REASON_RES_CERT_EXPIRING: &str = "KAFKA_TLS_RES_CERT_EXPIRING_SOON";
pub const REASON_RES_NO_ROTATION_EVIDENCE: &str = "KAFKA_TLS_RES_NO_ROTATION_EVIDENCE";
pub const REASON_SEC_NO_TLS_EVIDENCE: &str = "KAFKA_TLS_SEC_NO_EVIDENCE";
pub const REASON_SEC_UNENCRYPTED_PROTOCOL: &str = "KAFKA_TLS_SEC_UNENCRYPTED_PROTOCOL";
pub const REASON_SEC_CERT_EXPIRED: &str = "KAFKA_TLS_SEC_CERT_EXPIRED";
pub const REASON_SEC_MUTUAL_TLS_DISABLED: &str = "KAFKA_TLS_SEC_MUTUAL_TLS_DISABLED";
pub const REASON_INV_STALE_DATA: &str = "KAFKA_TLS_INV_STALE_DATA";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KafkaTlsInventoryItem {
    pub tls_id: String,
    pub cluster_name: String,
    pub certificate_subject: Option<String>,
    pub certificate_issuer: Option<String>,
    pub days_to_expiry: Option<i64>,
    pub owner: Option<String>,
    pub labels: BTreeMap<String, String>,
    pub encrypted_listener_count: Option<i32>,
    pub mutual_tls_enabled: Option<bool>,
    pub certificate_rotation_days: Option<i64>,
    pub has_tls_evidence: bool,
    pub security_protocol: String,
    pub collected_at: DateTime<Utc>,
}

pub fn evaluate_kafka_tls_inventory(
    items: &[KafkaTlsInventoryItem],
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

pub fn tls_inventory_item_from_config(
    cluster: &KafkaClusterConfig,
    collected_at: DateTime<Utc>,
) -> KafkaTlsInventoryItem {
    let protocol = cluster.security_protocol.to_ascii_uppercase();
    let has_tls_evidence = protocol.contains("SSL") || protocol.contains("TLS");

    KafkaTlsInventoryItem {
        tls_id: format!("{}:tls", cluster.name),
        cluster_name: cluster.name.clone(),
        certificate_subject: None,
        certificate_issuer: None,
        days_to_expiry: None,
        owner: None,
        labels: BTreeMap::new(),
        encrypted_listener_count: None,
        mutual_tls_enabled: None,
        certificate_rotation_days: None,
        has_tls_evidence,
        security_protocol: cluster.security_protocol.clone(),
        collected_at,
    }
}

fn evaluate_cost(
    item: &KafkaTlsInventoryItem,
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
                "Kafka TLS inventory for cluster {} has no owner, team, project, or cost-center metadata",
                item.cluster_name
            ),
            json!({
                "tls_id": item.tls_id,
                "cluster_name": item.cluster_name,
                "checked_keys": COST_ALLOCATION_TAG_KEYS,
            }),
        ));
    }

    if !item.has_tls_evidence {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_NO_TLS_EVIDENCE,
            Severity::High,
            format!(
                "Kafka TLS inventory for cluster {} has no TLS evidence",
                item.cluster_name
            ),
            json!({
                "tls_id": item.tls_id,
                "cluster_name": item.cluster_name,
                "recommendation": "Collect TLS listeners, certificate metadata, ownership, labels, rotation state, and mTLS posture before estimating encryption operating cost",
            }),
        ));
    }

    if item
        .encrypted_listener_count
        .is_some_and(|count| count >= 10)
    {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_HIGH_ENCRYPTED_LISTENER_COUNT,
            Severity::Medium,
            format!(
                "Kafka TLS inventory {} has high encrypted listener volume",
                item.tls_id
            ),
            json!({
                "tls_id": item.tls_id,
                "encrypted_listener_count": item.encrypted_listener_count,
                "recommendation": "Review listener sprawl, duplicate encryption paths, and certificate-management cost before adding more TLS endpoints",
            }),
        ));
    }
}

fn evaluate_resilience(
    item: &KafkaTlsInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if !item.has_tls_evidence {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_NO_TLS_EVIDENCE,
            Severity::High,
            format!(
                "Kafka TLS inventory for cluster {} has no resilience evidence",
                item.cluster_name
            ),
            json!({
                "tls_id": item.tls_id,
                "cluster_name": item.cluster_name,
                "recommendation": "Collect TLS listener, certificate, expiry, and rotation evidence before relying on encryption posture during recovery",
            }),
        ));
    }

    if item.has_tls_evidence
        && item
            .days_to_expiry
            .is_some_and(|days| (0..=14).contains(&days))
    {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_CERT_EXPIRING,
            Severity::High,
            format!("Kafka TLS certificate {} expires soon", item.tls_id),
            json!({
                "tls_id": item.tls_id,
                "days_to_expiry": item.days_to_expiry,
                "recommendation": "Rotate or renew Kafka TLS certificates before the recovery window is at risk",
            }),
        ));
    }

    if item.has_tls_evidence && item.certificate_rotation_days.is_none() {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_NO_ROTATION_EVIDENCE,
            Severity::Medium,
            format!(
                "Kafka TLS inventory for cluster {} has no certificate rotation evidence",
                item.cluster_name
            ),
            json!({
                "tls_id": item.tls_id,
                "certificate_rotation_days": item.certificate_rotation_days,
                "recommendation": "Record certificate rotation cadence and last rotation evidence for incident readiness",
            }),
        ));
    }
}

fn evaluate_security(
    item: &KafkaTlsInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if !item.has_tls_evidence {
        findings.push(finding(
            item,
            pillar,
            REASON_SEC_NO_TLS_EVIDENCE,
            Severity::High,
            format!(
                "Kafka TLS inventory for cluster {} has no TLS evidence for security review",
                item.cluster_name
            ),
            json!({
                "tls_id": item.tls_id,
                "cluster_name": item.cluster_name,
                "recommendation": "Collect listener protocol, certificate, expiry, issuer, and mTLS evidence before assessing encryption posture",
            }),
        ));
    }

    if !uses_encrypted_protocol(&item.security_protocol) {
        findings.push(finding(
            item,
            pillar,
            REASON_SEC_UNENCRYPTED_PROTOCOL,
            Severity::High,
            format!(
                "Kafka TLS inventory for cluster {} does not use an encrypted protocol",
                item.cluster_name
            ),
            json!({
                "tls_id": item.tls_id,
                "security_protocol": item.security_protocol,
                "recommendation": "Use SSL or SASL_SSL for Kafka client listeners",
            }),
        ));
    }

    if item.has_tls_evidence && item.days_to_expiry.is_some_and(|days| days < 0) {
        findings.push(finding(
            item,
            pillar,
            REASON_SEC_CERT_EXPIRED,
            Severity::High,
            format!("Kafka TLS certificate {} is expired", item.tls_id),
            json!({
                "tls_id": item.tls_id,
                "days_to_expiry": item.days_to_expiry,
                "recommendation": "Replace expired TLS certificates before accepting Kafka security posture",
            }),
        ));
    }

    if item.has_tls_evidence && item.mutual_tls_enabled == Some(false) {
        findings.push(finding(
            item,
            pillar,
            REASON_SEC_MUTUAL_TLS_DISABLED,
            Severity::Medium,
            format!("Kafka TLS inventory {} has mutual TLS disabled", item.tls_id),
            json!({
                "tls_id": item.tls_id,
                "mutual_tls_enabled": item.mutual_tls_enabled,
                "recommendation": "Enable or explicitly risk-accept mTLS for client authentication where supported",
            }),
        ));
    }
}

fn stale_finding(
    item: &KafkaTlsInventoryItem,
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
            "Kafka TLS inventory for cluster {} is {} hour(s) old",
            item.cluster_name, age_hours
        ),
        json!({
            "tls_id": item.tls_id,
            "cluster_name": item.cluster_name,
            "age_hours": age_hours,
            "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
            "recommendation": "Refresh Kafka TLS inventory before acting on this posture report",
        }),
    ))
}

fn uses_encrypted_protocol(protocol: &str) -> bool {
    let protocol = protocol.to_ascii_uppercase();
    protocol.contains("SSL") || protocol.contains("TLS")
}

fn has_owner_metadata(item: &KafkaTlsInventoryItem) -> bool {
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
    item: &KafkaTlsInventoryItem,
    pillar: Pillar,
    reason_code: &'static str,
    severity: Severity,
    message: String,
    evidence: Value,
) -> InventoryFinding {
    InventoryFinding {
        resource_id: item.tls_id.clone(),
        arn: format!("kafka:tls/{}", item.tls_id),
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

    fn tls(now: DateTime<Utc>) -> KafkaTlsInventoryItem {
        KafkaTlsInventoryItem {
            tls_id: "prod:tls".to_string(),
            cluster_name: "prod".to_string(),
            certificate_subject: Some("CN=prod.kafka".to_string()),
            certificate_issuer: Some("CN=mayyam-ca".to_string()),
            days_to_expiry: Some(90),
            owner: Some("platform".to_string()),
            labels: BTreeMap::new(),
            encrypted_listener_count: Some(2),
            mutual_tls_enabled: Some(true),
            certificate_rotation_days: Some(60),
            has_tls_evidence: true,
            security_protocol: "SSL".to_string(),
            collected_at: now,
        }
    }

    #[test]
    fn healthy_tls_passes_claimed_pillars() {
        let now = Utc::now();
        let item = tls(now);

        for pillar in [Pillar::Cost, Pillar::Resilience, Pillar::Security] {
            let report = evaluate_kafka_tls_inventory(std::slice::from_ref(&item), pillar, now);
            assert_eq!(report.resources_evaluated, 1);
            assert!(report.findings.is_empty());
            assert_eq!(report.stale_resources, 0);
            assert_eq!(report.score, 100);
        }
    }

    #[test]
    fn cost_flags_missing_owner_missing_evidence_and_listener_sprawl() {
        let now = Utc::now();
        let mut item = tls(now);
        item.owner = None;
        item.has_tls_evidence = false;
        item.encrypted_listener_count = Some(12);

        let report = evaluate_kafka_tls_inventory(&[item], Pillar::Cost, now);

        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_COST_OWNER_NOT_RECORDED));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_COST_NO_TLS_EVIDENCE));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_COST_HIGH_ENCRYPTED_LISTENER_COUNT));
    }

    #[test]
    fn resilience_flags_missing_evidence_expiry_and_rotation_gap() {
        let now = Utc::now();
        let mut missing_evidence = tls(now);
        missing_evidence.has_tls_evidence = false;
        let mut expiring = tls(now);
        expiring.days_to_expiry = Some(7);
        expiring.certificate_rotation_days = None;

        let report =
            evaluate_kafka_tls_inventory(&[missing_evidence, expiring], Pillar::Resilience, now);

        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_RES_NO_TLS_EVIDENCE));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_RES_CERT_EXPIRING));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_RES_NO_ROTATION_EVIDENCE));
    }

    #[test]
    fn security_flags_missing_evidence_unencrypted_expired_and_no_mtls() {
        let now = Utc::now();
        let mut missing_evidence = tls(now);
        missing_evidence.has_tls_evidence = false;
        let mut insecure = tls(now);
        insecure.security_protocol = "PLAINTEXT".to_string();
        insecure.days_to_expiry = Some(-1);
        insecure.mutual_tls_enabled = Some(false);

        let report =
            evaluate_kafka_tls_inventory(&[missing_evidence, insecure], Pillar::Security, now);

        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_SEC_NO_TLS_EVIDENCE));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_SEC_UNENCRYPTED_PROTOCOL));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_SEC_CERT_EXPIRED));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_SEC_MUTUAL_TLS_DISABLED));
    }

    #[test]
    fn stale_tls_inventory_is_counted_for_any_pillar() {
        let now = Utc::now();
        let mut item = tls(now);
        item.collected_at = now - Duration::hours(DEFAULT_STALE_AFTER_HOURS + 2);

        let report = evaluate_kafka_tls_inventory(&[item], Pillar::Security, now);

        assert_eq!(report.stale_resources, 1);
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_INV_STALE_DATA));
    }
}

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

// Deterministic poison message inventory evaluator for roadmap rows
// 04-KAFKA-DASHBOARD-MANAGEMENT-01569/01576/01597.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::config::KafkaClusterConfig;
use crate::services::aws::inventory::types::{
    score_pillar, InventoryFinding, Pillar, PillarReport, Severity, COST_ALLOCATION_TAG_KEYS,
    DEFAULT_STALE_AFTER_HOURS,
};

pub const RESOURCE_TYPE: &str = "KafkaPoisonMessage";
pub const REASON_COST_OWNER_NOT_RECORDED: &str = "KAFKA_POISON_MESSAGE_COST_OWNER_NOT_RECORDED";
pub const REASON_COST_NO_EVIDENCE: &str = "KAFKA_POISON_MESSAGE_COST_NO_EVIDENCE";
pub const REASON_COST_HIGH_FAILURE_VOLUME: &str = "KAFKA_POISON_MESSAGE_COST_HIGH_FAILURE_VOLUME";
pub const REASON_RES_NO_EVIDENCE: &str = "KAFKA_POISON_MESSAGE_RES_NO_EVIDENCE";
pub const REASON_RES_NO_QUARANTINE_EVIDENCE: &str =
    "KAFKA_POISON_MESSAGE_RES_NO_QUARANTINE_EVIDENCE";
pub const REASON_RES_NO_CLASSIFICATION_EVIDENCE: &str =
    "KAFKA_POISON_MESSAGE_RES_NO_CLASSIFICATION_EVIDENCE";
pub const REASON_RES_NO_REPLAY_POLICY_EVIDENCE: &str =
    "KAFKA_POISON_MESSAGE_RES_NO_REPLAY_POLICY_EVIDENCE";
pub const REASON_SEC_NO_EVIDENCE: &str = "KAFKA_POISON_MESSAGE_SEC_NO_EVIDENCE";
pub const REASON_SEC_NO_ACL_EVIDENCE: &str = "KAFKA_POISON_MESSAGE_SEC_NO_ACL_EVIDENCE";
pub const REASON_SEC_PLAINTEXT_CONNECTION: &str = "KAFKA_POISON_MESSAGE_SEC_PLAINTEXT_CONNECTION";
pub const REASON_INV_STALE_DATA: &str = "KAFKA_POISON_MESSAGE_INV_STALE_DATA";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoisonMessageInventoryItem {
    pub poison_message_id: String,
    pub cluster_name: String,
    pub source_topic: Option<String>,
    pub dead_letter_topic: Option<String>,
    pub failure_class: Option<String>,
    pub failed_message_count: Option<u64>,
    pub owner: Option<String>,
    pub labels: BTreeMap<String, String>,
    pub quarantine_evidence: bool,
    pub classification_evidence: bool,
    pub replay_policy_evidence: bool,
    pub acl_evidence: bool,
    pub plaintext_connection: bool,
    pub has_poison_message_evidence: bool,
    pub collected_at: DateTime<Utc>,
}

pub fn evaluate_poison_message_inventory(
    items: &[PoisonMessageInventoryItem],
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

pub fn poison_message_inventory_item_from_config(
    cluster: &KafkaClusterConfig,
    collected_at: DateTime<Utc>,
) -> PoisonMessageInventoryItem {
    PoisonMessageInventoryItem {
        poison_message_id: format!("{}:poison-message", cluster.name),
        cluster_name: cluster.name.clone(),
        source_topic: None,
        dead_letter_topic: None,
        failure_class: None,
        failed_message_count: None,
        owner: None,
        labels: BTreeMap::new(),
        quarantine_evidence: false,
        classification_evidence: false,
        replay_policy_evidence: false,
        acl_evidence: false,
        plaintext_connection: cluster.security_protocol.eq_ignore_ascii_case("PLAINTEXT"),
        has_poison_message_evidence: false,
        collected_at,
    }
}

fn evaluate_cost(
    item: &PoisonMessageInventoryItem,
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
                "Kafka poison message inventory for cluster {} has no owner, team, project, or cost-center metadata",
                item.cluster_name
            ),
            json!({
                "poison_message_id": item.poison_message_id,
                "cluster_name": item.cluster_name,
                "checked_keys": COST_ALLOCATION_TAG_KEYS,
            }),
        ));
    }

    if !item.has_poison_message_evidence {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_NO_EVIDENCE,
            Severity::High,
            format!(
                "Kafka poison message inventory for cluster {} has no poison message evidence",
                item.cluster_name
            ),
            json!({
                "poison_message_id": item.poison_message_id,
                "cluster_name": item.cluster_name,
                "recommendation": "Collect source topic, failure class, failure count, owner, quarantine, replay policy, and ACL evidence before estimating poison message cost posture",
            }),
        ));
    }

    if item
        .failed_message_count
        .is_some_and(|failed_message_count| failed_message_count > 10_000)
    {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_HIGH_FAILURE_VOLUME,
            Severity::Medium,
            format!(
                "Kafka poison message {} has high failure volume",
                item.poison_message_id
            ),
            json!({
                "poison_message_id": item.poison_message_id,
                "failed_message_count": item.failed_message_count,
                "recommendation": "Review poison-message volume, DLQ storage, and replay effort before accepting operational cost posture",
            }),
        ));
    }
}

fn evaluate_resilience(
    item: &PoisonMessageInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if !item.has_poison_message_evidence {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_NO_EVIDENCE,
            Severity::High,
            format!(
                "Kafka poison message inventory for cluster {} has no resilience evidence",
                item.cluster_name
            ),
            json!({
                "poison_message_id": item.poison_message_id,
                "cluster_name": item.cluster_name,
                "recommendation": "Collect quarantine, classification, replay, consumer isolation, and validation evidence before accepting poison message resilience posture",
            }),
        ));
    }

    if item.has_poison_message_evidence && !item.quarantine_evidence {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_NO_QUARANTINE_EVIDENCE,
            Severity::High,
            format!(
                "Kafka poison message {} has no quarantine evidence",
                item.poison_message_id
            ),
            json!({
                "poison_message_id": item.poison_message_id,
                "dead_letter_topic": item.dead_letter_topic,
                "recommendation": "Record DLQ or quarantine evidence before accepting poison-message resilience posture",
            }),
        ));
    }

    if item.has_poison_message_evidence && !item.classification_evidence {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_NO_CLASSIFICATION_EVIDENCE,
            Severity::High,
            format!(
                "Kafka poison message {} has no failure classification evidence",
                item.poison_message_id
            ),
            json!({
                "poison_message_id": item.poison_message_id,
                "failure_class": item.failure_class,
                "recommendation": "Classify poison-message failure modes before replaying or discarding messages",
            }),
        ));
    }

    if item.has_poison_message_evidence && !item.replay_policy_evidence {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_NO_REPLAY_POLICY_EVIDENCE,
            Severity::High,
            format!(
                "Kafka poison message {} has no replay policy evidence",
                item.poison_message_id
            ),
            json!({
                "poison_message_id": item.poison_message_id,
                "recommendation": "Record replay, discard, and operator approval policy before accepting poison-message recovery posture",
            }),
        ));
    }
}

fn evaluate_security(
    item: &PoisonMessageInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if !item.has_poison_message_evidence {
        findings.push(finding(
            item,
            pillar,
            REASON_SEC_NO_EVIDENCE,
            Severity::High,
            format!(
                "Kafka poison message inventory for cluster {} has no security evidence",
                item.cluster_name
            ),
            json!({
                "poison_message_id": item.poison_message_id,
                "cluster_name": item.cluster_name,
                "recommendation": "Collect quarantine access, replay identity, topic authorization, audit, and transport evidence before accepting security posture",
            }),
        ));
    }

    if item.has_poison_message_evidence && !item.acl_evidence {
        findings.push(finding(
            item,
            pillar,
            REASON_SEC_NO_ACL_EVIDENCE,
            Severity::High,
            format!(
                "Kafka poison message {} has no ACL evidence",
                item.poison_message_id
            ),
            json!({
                "poison_message_id": item.poison_message_id,
                "source_topic": item.source_topic,
                "dead_letter_topic": item.dead_letter_topic,
                "recommendation": "Record source topic, quarantine, replay, and operator ACL evidence for poison-message handling",
            }),
        ));
    }

    if item.plaintext_connection {
        findings.push(finding(
            item,
            pillar,
            REASON_SEC_PLAINTEXT_CONNECTION,
            Severity::High,
            format!(
                "Kafka poison message {} uses PLAINTEXT or unverified transport",
                item.poison_message_id
            ),
            json!({
                "poison_message_id": item.poison_message_id,
                "plaintext_connection": item.plaintext_connection,
                "recommendation": "Use encrypted Kafka transport before accepting poison-message security posture",
            }),
        ));
    }
}

fn stale_finding(
    item: &PoisonMessageInventoryItem,
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
            "Kafka poison message inventory for cluster {} is {} hour(s) old",
            item.cluster_name, age_hours
        ),
        json!({
            "poison_message_id": item.poison_message_id,
            "cluster_name": item.cluster_name,
            "age_hours": age_hours,
            "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
            "recommendation": "Refresh Kafka poison message inventory before acting on this posture report",
        }),
    ))
}

fn has_owner_metadata(item: &PoisonMessageInventoryItem) -> bool {
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
    item: &PoisonMessageInventoryItem,
    pillar: Pillar,
    reason_code: &'static str,
    severity: Severity,
    message: String,
    evidence: Value,
) -> InventoryFinding {
    InventoryFinding {
        resource_id: item.poison_message_id.clone(),
        arn: format!("kafka:poison-message/{}", item.poison_message_id),
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

    fn poison_message(now: DateTime<Utc>) -> PoisonMessageInventoryItem {
        PoisonMessageInventoryItem {
            poison_message_id: "prod:poison-message:orders".to_string(),
            cluster_name: "prod".to_string(),
            source_topic: Some("orders".to_string()),
            dead_letter_topic: Some("orders.dlq".to_string()),
            failure_class: Some("schema_validation".to_string()),
            failed_message_count: Some(25),
            owner: Some("platform".to_string()),
            labels: BTreeMap::new(),
            quarantine_evidence: true,
            classification_evidence: true,
            replay_policy_evidence: true,
            acl_evidence: true,
            plaintext_connection: false,
            has_poison_message_evidence: true,
            collected_at: now,
        }
    }

    #[test]
    fn healthy_poison_message_passes_claimed_pillars() {
        let now = Utc::now();
        let item = poison_message(now);

        for pillar in [Pillar::Cost, Pillar::Resilience, Pillar::Security] {
            let report =
                evaluate_poison_message_inventory(std::slice::from_ref(&item), pillar, now);
            assert_eq!(report.resources_evaluated, 1);
            assert!(report.findings.is_empty());
            assert_eq!(report.stale_resources, 0);
            assert_eq!(report.score, 100);
        }
    }

    #[test]
    fn cost_flags_missing_owner_missing_evidence_and_high_failure_volume() {
        let now = Utc::now();
        let mut missing_evidence = poison_message(now);
        missing_evidence.owner = None;
        missing_evidence.has_poison_message_evidence = false;
        let mut high_volume = poison_message(now);
        high_volume.failed_message_count = Some(10_001);

        let report =
            evaluate_poison_message_inventory(&[missing_evidence, high_volume], Pillar::Cost, now);

        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_COST_OWNER_NOT_RECORDED));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_COST_NO_EVIDENCE));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_COST_HIGH_FAILURE_VOLUME));
    }

    #[test]
    fn resilience_flags_missing_evidence_quarantine_classification_and_replay_policy() {
        let now = Utc::now();
        let mut missing_evidence = poison_message(now);
        missing_evidence.has_poison_message_evidence = false;
        let mut risky = poison_message(now);
        risky.quarantine_evidence = false;
        risky.classification_evidence = false;
        risky.replay_policy_evidence = false;

        let report =
            evaluate_poison_message_inventory(&[missing_evidence, risky], Pillar::Resilience, now);

        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_RES_NO_EVIDENCE));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_RES_NO_QUARANTINE_EVIDENCE));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_RES_NO_CLASSIFICATION_EVIDENCE));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_RES_NO_REPLAY_POLICY_EVIDENCE));
    }

    #[test]
    fn security_flags_missing_evidence_acl_and_plaintext() {
        let now = Utc::now();
        let mut missing_evidence = poison_message(now);
        missing_evidence.has_poison_message_evidence = false;
        let mut insecure = poison_message(now);
        insecure.acl_evidence = false;
        insecure.plaintext_connection = true;

        let report =
            evaluate_poison_message_inventory(&[missing_evidence, insecure], Pillar::Security, now);

        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_SEC_NO_EVIDENCE));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_SEC_NO_ACL_EVIDENCE));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_SEC_PLAINTEXT_CONNECTION));
    }

    #[test]
    fn stale_poison_message_inventory_is_counted_for_any_pillar() {
        let now = Utc::now();
        let mut item = poison_message(now);
        item.collected_at = now - Duration::hours(DEFAULT_STALE_AFTER_HOURS + 2);

        let report = evaluate_poison_message_inventory(&[item], Pillar::Security, now);

        assert_eq!(report.stale_resources, 1);
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_INV_STALE_DATA));
    }
}

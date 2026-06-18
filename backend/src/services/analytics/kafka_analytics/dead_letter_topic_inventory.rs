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

// Deterministic dead-letter topic inventory evaluator for roadmap rows
// 04-KAFKA-DASHBOARD-MANAGEMENT-01520/01527/01548.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::config::KafkaClusterConfig;
use crate::services::aws::inventory::types::{
    score_pillar, InventoryFinding, Pillar, PillarReport, Severity, COST_ALLOCATION_TAG_KEYS,
    DEFAULT_STALE_AFTER_HOURS,
};

pub const RESOURCE_TYPE: &str = "KafkaDeadLetterTopic";
pub const REASON_COST_OWNER_NOT_RECORDED: &str = "KAFKA_DEAD_LETTER_TOPIC_COST_OWNER_NOT_RECORDED";
pub const REASON_COST_NO_EVIDENCE: &str = "KAFKA_DEAD_LETTER_TOPIC_COST_NO_EVIDENCE";
pub const REASON_COST_LONG_RETENTION: &str = "KAFKA_DEAD_LETTER_TOPIC_COST_LONG_RETENTION";
pub const REASON_RES_NO_EVIDENCE: &str = "KAFKA_DEAD_LETTER_TOPIC_RES_NO_EVIDENCE";
pub const REASON_RES_NO_ROUTING_EVIDENCE: &str = "KAFKA_DEAD_LETTER_TOPIC_RES_NO_ROUTING_EVIDENCE";
pub const REASON_RES_NO_REPLAY_EVIDENCE: &str = "KAFKA_DEAD_LETTER_TOPIC_RES_NO_REPLAY_EVIDENCE";
pub const REASON_RES_NO_ALERT_EVIDENCE: &str = "KAFKA_DEAD_LETTER_TOPIC_RES_NO_ALERT_EVIDENCE";
pub const REASON_SEC_NO_EVIDENCE: &str = "KAFKA_DEAD_LETTER_TOPIC_SEC_NO_EVIDENCE";
pub const REASON_SEC_NO_ACL_EVIDENCE: &str = "KAFKA_DEAD_LETTER_TOPIC_SEC_NO_ACL_EVIDENCE";
pub const REASON_SEC_PLAINTEXT_CONNECTION: &str =
    "KAFKA_DEAD_LETTER_TOPIC_SEC_PLAINTEXT_CONNECTION";
pub const REASON_INV_STALE_DATA: &str = "KAFKA_DEAD_LETTER_TOPIC_INV_STALE_DATA";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeadLetterTopicInventoryItem {
    pub dlq_id: String,
    pub cluster_name: String,
    pub source_topic: Option<String>,
    pub dead_letter_topic: Option<String>,
    pub retention_ms: Option<i64>,
    pub owner: Option<String>,
    pub labels: BTreeMap<String, String>,
    pub routing_evidence: bool,
    pub replay_evidence: bool,
    pub alert_evidence: bool,
    pub acl_evidence: bool,
    pub plaintext_connection: bool,
    pub has_dead_letter_evidence: bool,
    pub collected_at: DateTime<Utc>,
}

pub fn evaluate_dead_letter_topic_inventory(
    items: &[DeadLetterTopicInventoryItem],
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

pub fn dead_letter_topic_inventory_item_from_config(
    cluster: &KafkaClusterConfig,
    collected_at: DateTime<Utc>,
) -> DeadLetterTopicInventoryItem {
    DeadLetterTopicInventoryItem {
        dlq_id: format!("{}:dead-letter-topic", cluster.name),
        cluster_name: cluster.name.clone(),
        source_topic: None,
        dead_letter_topic: None,
        retention_ms: None,
        owner: None,
        labels: BTreeMap::new(),
        routing_evidence: false,
        replay_evidence: false,
        alert_evidence: false,
        acl_evidence: false,
        plaintext_connection: cluster.security_protocol.eq_ignore_ascii_case("PLAINTEXT"),
        has_dead_letter_evidence: false,
        collected_at,
    }
}

fn evaluate_cost(
    item: &DeadLetterTopicInventoryItem,
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
                "Kafka dead-letter topic inventory for cluster {} has no owner, team, project, or cost-center metadata",
                item.cluster_name
            ),
            json!({
                "dlq_id": item.dlq_id,
                "cluster_name": item.cluster_name,
                "checked_keys": COST_ALLOCATION_TAG_KEYS,
            }),
        ));
    }

    if !item.has_dead_letter_evidence {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_NO_EVIDENCE,
            Severity::High,
            format!(
                "Kafka dead-letter topic inventory for cluster {} has no DLQ evidence",
                item.cluster_name
            ),
            json!({
                "dlq_id": item.dlq_id,
                "cluster_name": item.cluster_name,
                "recommendation": "Collect source topic, dead-letter topic, retention, owner, routing, replay, alert, and ACL evidence before estimating DLQ cost posture",
            }),
        ));
    }

    if item
        .retention_ms
        .is_some_and(|retention_ms| retention_ms > 30_i64 * 24 * 60 * 60 * 1000)
    {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_LONG_RETENTION,
            Severity::Medium,
            format!("Kafka dead-letter topic {} has long retention", item.dlq_id),
            json!({
                "dlq_id": item.dlq_id,
                "retention_ms": item.retention_ms,
                "recommendation": "Review DLQ retention against replay requirements and storage cost before accepting the policy",
            }),
        ));
    }
}

fn evaluate_resilience(
    item: &DeadLetterTopicInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if !item.has_dead_letter_evidence {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_NO_EVIDENCE,
            Severity::High,
            format!(
                "Kafka dead-letter topic inventory for cluster {} has no resilience evidence",
                item.cluster_name
            ),
            json!({
                "dlq_id": item.dlq_id,
                "cluster_name": item.cluster_name,
                "recommendation": "Collect failure routing, replay procedure, alerting, ownership, and retention evidence before accepting DLQ resilience posture",
            }),
        ));
    }

    if item.has_dead_letter_evidence && !item.routing_evidence {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_NO_ROUTING_EVIDENCE,
            Severity::High,
            format!(
                "Kafka dead-letter topic {} has no failure routing evidence",
                item.dlq_id
            ),
            json!({
                "dlq_id": item.dlq_id,
                "source_topic": item.source_topic,
                "dead_letter_topic": item.dead_letter_topic,
                "recommendation": "Record producer, consumer, or connector failure-routing evidence before accepting DLQ posture",
            }),
        ));
    }

    if item.has_dead_letter_evidence && !item.replay_evidence {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_NO_REPLAY_EVIDENCE,
            Severity::High,
            format!(
                "Kafka dead-letter topic {} has no replay evidence",
                item.dlq_id
            ),
            json!({
                "dlq_id": item.dlq_id,
                "recommendation": "Record DLQ triage and replay procedure evidence before accepting recovery posture",
            }),
        ));
    }

    if item.has_dead_letter_evidence && !item.alert_evidence {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_NO_ALERT_EVIDENCE,
            Severity::Medium,
            format!("Kafka dead-letter topic {} has no alert evidence", item.dlq_id),
            json!({
                "dlq_id": item.dlq_id,
                "recommendation": "Add alert evidence for DLQ growth, age, and replay backlog before accepting operational posture",
            }),
        ));
    }
}

fn evaluate_security(
    item: &DeadLetterTopicInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if !item.has_dead_letter_evidence {
        findings.push(finding(
            item,
            pillar,
            REASON_SEC_NO_EVIDENCE,
            Severity::High,
            format!(
                "Kafka dead-letter topic inventory for cluster {} has no security evidence",
                item.cluster_name
            ),
            json!({
                "dlq_id": item.dlq_id,
                "cluster_name": item.cluster_name,
                "recommendation": "Collect DLQ topic authorization, replay identity, audit, approval, and transport evidence before accepting security posture",
            }),
        ));
    }

    if item.has_dead_letter_evidence && !item.acl_evidence {
        findings.push(finding(
            item,
            pillar,
            REASON_SEC_NO_ACL_EVIDENCE,
            Severity::High,
            format!("Kafka dead-letter topic {} has no ACL evidence", item.dlq_id),
            json!({
                "dlq_id": item.dlq_id,
                "dead_letter_topic": item.dead_letter_topic,
                "recommendation": "Record read, write, replay, and admin ACL evidence for DLQ topics",
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
                "Kafka dead-letter topic {} uses PLAINTEXT or unverified transport",
                item.dlq_id
            ),
            json!({
                "dlq_id": item.dlq_id,
                "plaintext_connection": item.plaintext_connection,
                "recommendation": "Use encrypted Kafka transport before accepting DLQ security posture",
            }),
        ));
    }
}

fn stale_finding(
    item: &DeadLetterTopicInventoryItem,
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
            "Kafka dead-letter topic inventory for cluster {} is {} hour(s) old",
            item.cluster_name, age_hours
        ),
        json!({
            "dlq_id": item.dlq_id,
            "cluster_name": item.cluster_name,
            "age_hours": age_hours,
            "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
            "recommendation": "Refresh Kafka dead-letter topic inventory before acting on this posture report",
        }),
    ))
}

fn has_owner_metadata(item: &DeadLetterTopicInventoryItem) -> bool {
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
    item: &DeadLetterTopicInventoryItem,
    pillar: Pillar,
    reason_code: &'static str,
    severity: Severity,
    message: String,
    evidence: Value,
) -> InventoryFinding {
    InventoryFinding {
        resource_id: item.dlq_id.clone(),
        arn: format!("kafka:dead-letter-topic/{}", item.dlq_id),
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

    fn dlq(now: DateTime<Utc>) -> DeadLetterTopicInventoryItem {
        DeadLetterTopicInventoryItem {
            dlq_id: "prod:dead-letter-topic:orders".to_string(),
            cluster_name: "prod".to_string(),
            source_topic: Some("orders".to_string()),
            dead_letter_topic: Some("orders.dlq".to_string()),
            retention_ms: Some(7_i64 * 24 * 60 * 60 * 1000),
            owner: Some("platform".to_string()),
            labels: BTreeMap::new(),
            routing_evidence: true,
            replay_evidence: true,
            alert_evidence: true,
            acl_evidence: true,
            plaintext_connection: false,
            has_dead_letter_evidence: true,
            collected_at: now,
        }
    }

    #[test]
    fn healthy_dead_letter_topic_passes_claimed_pillars() {
        let now = Utc::now();
        let item = dlq(now);

        for pillar in [Pillar::Cost, Pillar::Resilience, Pillar::Security] {
            let report =
                evaluate_dead_letter_topic_inventory(std::slice::from_ref(&item), pillar, now);
            assert_eq!(report.resources_evaluated, 1);
            assert!(report.findings.is_empty());
            assert_eq!(report.stale_resources, 0);
            assert_eq!(report.score, 100);
        }
    }

    #[test]
    fn cost_flags_missing_owner_missing_evidence_and_long_retention() {
        let now = Utc::now();
        let mut missing_evidence = dlq(now);
        missing_evidence.owner = None;
        missing_evidence.has_dead_letter_evidence = false;
        let mut long_retention = dlq(now);
        long_retention.retention_ms = Some(31_i64 * 24 * 60 * 60 * 1000);

        let report = evaluate_dead_letter_topic_inventory(
            &[missing_evidence, long_retention],
            Pillar::Cost,
            now,
        );

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
            .any(|finding| finding.reason_code == REASON_COST_LONG_RETENTION));
    }

    #[test]
    fn resilience_flags_missing_evidence_routing_replay_and_alerts() {
        let now = Utc::now();
        let mut missing_evidence = dlq(now);
        missing_evidence.has_dead_letter_evidence = false;
        let mut risky = dlq(now);
        risky.routing_evidence = false;
        risky.replay_evidence = false;
        risky.alert_evidence = false;

        let report = evaluate_dead_letter_topic_inventory(
            &[missing_evidence, risky],
            Pillar::Resilience,
            now,
        );

        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_RES_NO_EVIDENCE));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_RES_NO_ROUTING_EVIDENCE));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_RES_NO_REPLAY_EVIDENCE));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_RES_NO_ALERT_EVIDENCE));
    }

    #[test]
    fn security_flags_missing_evidence_acl_and_plaintext() {
        let now = Utc::now();
        let mut missing_evidence = dlq(now);
        missing_evidence.has_dead_letter_evidence = false;
        let mut insecure = dlq(now);
        insecure.acl_evidence = false;
        insecure.plaintext_connection = true;

        let report = evaluate_dead_letter_topic_inventory(
            &[missing_evidence, insecure],
            Pillar::Security,
            now,
        );

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
    fn stale_dead_letter_topic_inventory_is_counted_for_any_pillar() {
        let now = Utc::now();
        let mut item = dlq(now);
        item.collected_at = now - Duration::hours(DEFAULT_STALE_AFTER_HOURS + 2);

        let report = evaluate_dead_letter_topic_inventory(&[item], Pillar::Security, now);

        assert_eq!(report.stale_resources, 1);
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_INV_STALE_DATA));
    }
}

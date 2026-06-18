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

// Deterministic message replay inventory evaluator for roadmap rows
// 04-KAFKA-DASHBOARD-MANAGEMENT-01471/01478/01499.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::config::KafkaClusterConfig;
use crate::services::aws::inventory::types::{
    score_pillar, InventoryFinding, Pillar, PillarReport, Severity, COST_ALLOCATION_TAG_KEYS,
    DEFAULT_STALE_AFTER_HOURS,
};

pub const RESOURCE_TYPE: &str = "KafkaMessageReplay";
pub const REASON_COST_OWNER_NOT_RECORDED: &str = "KAFKA_MESSAGE_REPLAY_COST_OWNER_NOT_RECORDED";
pub const REASON_COST_NO_EVIDENCE: &str = "KAFKA_MESSAGE_REPLAY_COST_NO_EVIDENCE";
pub const REASON_COST_LARGE_REPLAY_WINDOW: &str = "KAFKA_MESSAGE_REPLAY_COST_LARGE_REPLAY_WINDOW";
pub const REASON_RES_NO_EVIDENCE: &str = "KAFKA_MESSAGE_REPLAY_RES_NO_EVIDENCE";
pub const REASON_RES_NO_REPLAY_PLAN: &str = "KAFKA_MESSAGE_REPLAY_RES_NO_REPLAY_PLAN";
pub const REASON_RES_NO_IDEMPOTENCY_EVIDENCE: &str =
    "KAFKA_MESSAGE_REPLAY_RES_NO_IDEMPOTENCY_EVIDENCE";
pub const REASON_RES_NO_THROTTLE_EVIDENCE: &str = "KAFKA_MESSAGE_REPLAY_RES_NO_THROTTLE_EVIDENCE";
pub const REASON_SEC_NO_EVIDENCE: &str = "KAFKA_MESSAGE_REPLAY_SEC_NO_EVIDENCE";
pub const REASON_SEC_NO_ACL_EVIDENCE: &str = "KAFKA_MESSAGE_REPLAY_SEC_NO_ACL_EVIDENCE";
pub const REASON_SEC_PLAINTEXT_CONNECTION: &str = "KAFKA_MESSAGE_REPLAY_SEC_PLAINTEXT_CONNECTION";
pub const REASON_INV_STALE_DATA: &str = "KAFKA_MESSAGE_REPLAY_INV_STALE_DATA";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageReplayInventoryItem {
    pub replay_id: String,
    pub cluster_name: String,
    pub source_topic: Option<String>,
    pub target_topic: Option<String>,
    pub consumer_group: Option<String>,
    pub start_offset: Option<i64>,
    pub end_offset: Option<i64>,
    pub replay_window_minutes: Option<u32>,
    pub owner: Option<String>,
    pub labels: BTreeMap<String, String>,
    pub replay_plan_evidence: bool,
    pub idempotency_evidence: bool,
    pub throttle_evidence: bool,
    pub acl_evidence: bool,
    pub plaintext_connection: bool,
    pub has_replay_evidence: bool,
    pub collected_at: DateTime<Utc>,
}

pub fn evaluate_message_replay_inventory(
    items: &[MessageReplayInventoryItem],
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

pub fn message_replay_inventory_item_from_config(
    cluster: &KafkaClusterConfig,
    collected_at: DateTime<Utc>,
) -> MessageReplayInventoryItem {
    MessageReplayInventoryItem {
        replay_id: format!("{}:message-replay", cluster.name),
        cluster_name: cluster.name.clone(),
        source_topic: None,
        target_topic: None,
        consumer_group: None,
        start_offset: None,
        end_offset: None,
        replay_window_minutes: None,
        owner: None,
        labels: BTreeMap::new(),
        replay_plan_evidence: false,
        idempotency_evidence: false,
        throttle_evidence: false,
        acl_evidence: false,
        plaintext_connection: cluster.security_protocol.eq_ignore_ascii_case("PLAINTEXT"),
        has_replay_evidence: false,
        collected_at,
    }
}

fn evaluate_cost(
    item: &MessageReplayInventoryItem,
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
                "Kafka message replay inventory for cluster {} has no owner, team, project, or cost-center metadata",
                item.cluster_name
            ),
            json!({
                "replay_id": item.replay_id,
                "cluster_name": item.cluster_name,
                "checked_keys": COST_ALLOCATION_TAG_KEYS,
            }),
        ));
    }

    if !item.has_replay_evidence {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_NO_EVIDENCE,
            Severity::High,
            format!(
                "Kafka message replay inventory for cluster {} has no replay evidence",
                item.cluster_name
            ),
            json!({
                "replay_id": item.replay_id,
                "cluster_name": item.cluster_name,
                "recommendation": "Collect topic, offset range, owner, replay plan, throttle limits, and idempotency evidence before estimating replay cost posture",
            }),
        ));
    }

    if item
        .replay_window_minutes
        .is_some_and(|window_minutes| window_minutes > 240)
    {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_LARGE_REPLAY_WINDOW,
            Severity::Medium,
            format!(
                "Kafka message replay {} has a large replay window",
                item.replay_id
            ),
            json!({
                "replay_id": item.replay_id,
                "replay_window_minutes": item.replay_window_minutes,
                "recommendation": "Review replay capacity, throttling, and consumer impact before accepting an extended replay window",
            }),
        ));
    }
}

fn evaluate_resilience(
    item: &MessageReplayInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if !item.has_replay_evidence {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_NO_EVIDENCE,
            Severity::High,
            format!(
                "Kafka message replay inventory for cluster {} has no resilience evidence",
                item.cluster_name
            ),
            json!({
                "replay_id": item.replay_id,
                "cluster_name": item.cluster_name,
                "recommendation": "Collect replay scope, offsets, ordering expectations, backpressure controls, and validation evidence before accepting replay resilience posture",
            }),
        ));
    }

    if item.has_replay_evidence && !item.replay_plan_evidence {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_NO_REPLAY_PLAN,
            Severity::High,
            format!(
                "Kafka message replay {} has no replay plan evidence",
                item.replay_id
            ),
            json!({
                "replay_id": item.replay_id,
                "source_topic": item.source_topic,
                "target_topic": item.target_topic,
                "recommendation": "Record replay plan, offset range, validation, and rollback evidence before running message replay",
            }),
        ));
    }

    if item.has_replay_evidence && !item.idempotency_evidence {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_NO_IDEMPOTENCY_EVIDENCE,
            Severity::High,
            format!(
                "Kafka message replay {} has no idempotency evidence",
                item.replay_id
            ),
            json!({
                "replay_id": item.replay_id,
                "consumer_group": item.consumer_group,
                "recommendation": "Prove downstream consumers are idempotent or isolated before replaying messages",
            }),
        ));
    }

    if item.has_replay_evidence && !item.throttle_evidence {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_NO_THROTTLE_EVIDENCE,
            Severity::Medium,
            format!(
                "Kafka message replay {} has no throttle evidence",
                item.replay_id
            ),
            json!({
                "replay_id": item.replay_id,
                "recommendation": "Record replay rate limits and backpressure controls before replaying production messages",
            }),
        ));
    }
}

fn evaluate_security(
    item: &MessageReplayInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if !item.has_replay_evidence {
        findings.push(finding(
            item,
            pillar,
            REASON_SEC_NO_EVIDENCE,
            Severity::High,
            format!(
                "Kafka message replay inventory for cluster {} has no security evidence",
                item.cluster_name
            ),
            json!({
                "replay_id": item.replay_id,
                "cluster_name": item.cluster_name,
                "recommendation": "Collect replay identity, topic authorization, audit, approval, and transport evidence before accepting replay security posture",
            }),
        ));
    }

    if item.has_replay_evidence && !item.acl_evidence {
        findings.push(finding(
            item,
            pillar,
            REASON_SEC_NO_ACL_EVIDENCE,
            Severity::High,
            format!(
                "Kafka message replay {} has no ACL evidence",
                item.replay_id
            ),
            json!({
                "replay_id": item.replay_id,
                "source_topic": item.source_topic,
                "target_topic": item.target_topic,
                "recommendation": "Record source, target, and consumer-group authorization evidence before message replay",
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
                "Kafka message replay {} uses PLAINTEXT or unverified transport",
                item.replay_id
            ),
            json!({
                "replay_id": item.replay_id,
                "plaintext_connection": item.plaintext_connection,
                "recommendation": "Use encrypted Kafka transport for message replay operations",
            }),
        ));
    }
}

fn stale_finding(
    item: &MessageReplayInventoryItem,
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
            "Kafka message replay inventory for cluster {} is {} hour(s) old",
            item.cluster_name, age_hours
        ),
        json!({
            "replay_id": item.replay_id,
            "cluster_name": item.cluster_name,
            "age_hours": age_hours,
            "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
            "recommendation": "Refresh Kafka message replay inventory before acting on this posture report",
        }),
    ))
}

fn has_owner_metadata(item: &MessageReplayInventoryItem) -> bool {
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
    item: &MessageReplayInventoryItem,
    pillar: Pillar,
    reason_code: &'static str,
    severity: Severity,
    message: String,
    evidence: Value,
) -> InventoryFinding {
    InventoryFinding {
        resource_id: item.replay_id.clone(),
        arn: format!("kafka:message-replay/{}", item.replay_id),
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

    fn replay(now: DateTime<Utc>) -> MessageReplayInventoryItem {
        MessageReplayInventoryItem {
            replay_id: "prod:message-replay:orders".to_string(),
            cluster_name: "prod".to_string(),
            source_topic: Some("orders".to_string()),
            target_topic: Some("orders-replay".to_string()),
            consumer_group: Some("orders-reprocessor".to_string()),
            start_offset: Some(100),
            end_offset: Some(500),
            replay_window_minutes: Some(30),
            owner: Some("platform".to_string()),
            labels: BTreeMap::new(),
            replay_plan_evidence: true,
            idempotency_evidence: true,
            throttle_evidence: true,
            acl_evidence: true,
            plaintext_connection: false,
            has_replay_evidence: true,
            collected_at: now,
        }
    }

    #[test]
    fn healthy_message_replay_passes_claimed_pillars() {
        let now = Utc::now();
        let item = replay(now);

        for pillar in [Pillar::Cost, Pillar::Resilience, Pillar::Security] {
            let report =
                evaluate_message_replay_inventory(std::slice::from_ref(&item), pillar, now);
            assert_eq!(report.resources_evaluated, 1);
            assert!(report.findings.is_empty());
            assert_eq!(report.stale_resources, 0);
            assert_eq!(report.score, 100);
        }
    }

    #[test]
    fn cost_flags_missing_owner_missing_evidence_and_large_window() {
        let now = Utc::now();
        let mut missing_evidence = replay(now);
        missing_evidence.owner = None;
        missing_evidence.has_replay_evidence = false;
        let mut large_window = replay(now);
        large_window.replay_window_minutes = Some(241);

        let report =
            evaluate_message_replay_inventory(&[missing_evidence, large_window], Pillar::Cost, now);

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
            .any(|finding| finding.reason_code == REASON_COST_LARGE_REPLAY_WINDOW));
    }

    #[test]
    fn resilience_flags_missing_evidence_plan_idempotency_and_throttle() {
        let now = Utc::now();
        let mut missing_evidence = replay(now);
        missing_evidence.has_replay_evidence = false;
        let mut risky = replay(now);
        risky.replay_plan_evidence = false;
        risky.idempotency_evidence = false;
        risky.throttle_evidence = false;

        let report =
            evaluate_message_replay_inventory(&[missing_evidence, risky], Pillar::Resilience, now);

        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_RES_NO_EVIDENCE));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_RES_NO_REPLAY_PLAN));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_RES_NO_IDEMPOTENCY_EVIDENCE));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_RES_NO_THROTTLE_EVIDENCE));
    }

    #[test]
    fn security_flags_missing_evidence_acl_and_plaintext() {
        let now = Utc::now();
        let mut missing_evidence = replay(now);
        missing_evidence.has_replay_evidence = false;
        let mut insecure = replay(now);
        insecure.acl_evidence = false;
        insecure.plaintext_connection = true;

        let report =
            evaluate_message_replay_inventory(&[missing_evidence, insecure], Pillar::Security, now);

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
    fn stale_message_replay_inventory_is_counted_for_any_pillar() {
        let now = Utc::now();
        let mut item = replay(now);
        item.collected_at = now - Duration::hours(DEFAULT_STALE_AFTER_HOURS + 2);

        let report = evaluate_message_replay_inventory(&[item], Pillar::Security, now);

        assert_eq!(report.stale_resources, 1);
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_INV_STALE_DATA));
    }
}

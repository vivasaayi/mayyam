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

// Deterministic Kafka consumer group inventory evaluator for roadmap rows
// 04-KAFKA-DASHBOARD-MANAGEMENT-00344/00351/00372.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::config::KafkaClusterConfig;
use crate::services::aws::inventory::types::{
    score_pillar, InventoryFinding, Pillar, PillarReport, Severity, COST_ALLOCATION_TAG_KEYS,
    DEFAULT_STALE_AFTER_HOURS,
};

pub const RESOURCE_TYPE: &str = "KafkaConsumerGroup";
pub const REASON_COST_OWNER_NOT_RECORDED: &str = "KAFKA_CONSUMER_GROUP_COST_OWNER_NOT_RECORDED";
pub const REASON_COST_NO_GROUP_EVIDENCE: &str = "KAFKA_CONSUMER_GROUP_COST_NO_EVIDENCE";
pub const REASON_COST_HIGH_LAG: &str = "KAFKA_CONSUMER_GROUP_COST_HIGH_LAG";
pub const REASON_RES_NO_GROUP_EVIDENCE: &str = "KAFKA_CONSUMER_GROUP_RES_NO_EVIDENCE";
pub const REASON_RES_NO_ACTIVE_MEMBERS: &str = "KAFKA_CONSUMER_GROUP_RES_NO_ACTIVE_MEMBERS";
pub const REASON_RES_NO_COMMITTED_OFFSETS: &str = "KAFKA_CONSUMER_GROUP_RES_NO_COMMITTED_OFFSETS";
pub const REASON_SEC_NO_GROUP_EVIDENCE: &str = "KAFKA_CONSUMER_GROUP_SEC_NO_EVIDENCE";
pub const REASON_SEC_NO_ACL_EVIDENCE: &str = "KAFKA_CONSUMER_GROUP_SEC_NO_ACL_EVIDENCE";
pub const REASON_SEC_PLAINTEXT: &str = "KAFKA_CONSUMER_GROUP_SEC_PLAINTEXT_PROTOCOL";
pub const REASON_INV_STALE_DATA: &str = "KAFKA_CONSUMER_GROUP_INV_STALE_DATA";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KafkaConsumerGroupInventoryItem {
    pub consumer_group_id: String,
    pub cluster_name: String,
    pub group_id: String,
    pub owner: Option<String>,
    pub labels: BTreeMap<String, String>,
    pub member_count: Option<i32>,
    pub committed_offset_count: Option<i32>,
    pub total_lag: Option<i64>,
    pub has_group_evidence: bool,
    pub has_acl_evidence: bool,
    pub security_protocol: String,
    pub collected_at: DateTime<Utc>,
}

pub fn evaluate_kafka_consumer_group_inventory(
    items: &[KafkaConsumerGroupInventoryItem],
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

pub fn consumer_group_inventory_item_from_config(
    cluster: &KafkaClusterConfig,
    collected_at: DateTime<Utc>,
) -> KafkaConsumerGroupInventoryItem {
    KafkaConsumerGroupInventoryItem {
        consumer_group_id: format!("{}:consumer-groups", cluster.name),
        cluster_name: cluster.name.clone(),
        group_id: "<uncollected>".to_string(),
        owner: None,
        labels: BTreeMap::new(),
        member_count: None,
        committed_offset_count: None,
        total_lag: None,
        has_group_evidence: false,
        has_acl_evidence: false,
        security_protocol: cluster.security_protocol.clone(),
        collected_at,
    }
}

fn evaluate_cost(
    item: &KafkaConsumerGroupInventoryItem,
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
                "Kafka consumer group inventory for cluster {} has no owner, team, project, or cost-center metadata",
                item.cluster_name
            ),
            json!({
                "consumer_group_id": item.consumer_group_id,
                "cluster_name": item.cluster_name,
                "checked_keys": COST_ALLOCATION_TAG_KEYS,
            }),
        ));
    }

    if !item.has_group_evidence {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_NO_GROUP_EVIDENCE,
            Severity::High,
            format!(
                "Kafka consumer group inventory for cluster {} has no group evidence",
                item.cluster_name
            ),
            json!({
                "consumer_group_id": item.consumer_group_id,
                "cluster_name": item.cluster_name,
                "recommendation": "Collect group membership, committed offsets, lag, ownership, and labels before estimating consumer-driven broker cost",
            }),
        ));
    }

    if item.total_lag.is_some_and(|lag| lag >= 100_000) {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_HIGH_LAG,
            Severity::Medium,
            format!(
                "Kafka consumer group {} has total lag {}",
                item.group_id,
                item.total_lag.unwrap_or_default()
            ),
            json!({
                "consumer_group_id": item.consumer_group_id,
                "total_lag": item.total_lag,
                "recommendation": "Validate whether lag requires consumer scaling or broker capacity before adding spend",
            }),
        ));
    }
}

fn evaluate_resilience(
    item: &KafkaConsumerGroupInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if !item.has_group_evidence {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_NO_GROUP_EVIDENCE,
            Severity::High,
            format!(
                "Kafka consumer group inventory for cluster {} has no resilience evidence",
                item.cluster_name
            ),
            json!({
                "consumer_group_id": item.consumer_group_id,
                "cluster_name": item.cluster_name,
                "recommendation": "Collect group state, active members, committed offsets, and lag before evaluating consumer recovery readiness",
            }),
        ));
    }

    if item.has_group_evidence && item.member_count.unwrap_or_default() == 0 {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_NO_ACTIVE_MEMBERS,
            Severity::High,
            format!("Kafka consumer group {} has no active members", item.group_id),
            json!({
                "consumer_group_id": item.consumer_group_id,
                "member_count": item.member_count,
                "recommendation": "Verify consumer availability and restart or rebalance the group before accepting it as resilient",
            }),
        ));
    }

    if item.has_group_evidence && item.committed_offset_count.unwrap_or_default() == 0 {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_NO_COMMITTED_OFFSETS,
            Severity::High,
            format!(
                "Kafka consumer group {} has no committed offset evidence",
                item.group_id
            ),
            json!({
                "consumer_group_id": item.consumer_group_id,
                "committed_offset_count": item.committed_offset_count,
                "recommendation": "Collect committed offsets before using the group for recovery or migration decisions",
            }),
        ));
    }
}

fn evaluate_security(
    item: &KafkaConsumerGroupInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if !item.has_group_evidence {
        findings.push(finding(
            item,
            pillar,
            REASON_SEC_NO_GROUP_EVIDENCE,
            Severity::Medium,
            format!(
                "Kafka consumer group inventory for cluster {} has no group evidence for security review",
                item.cluster_name
            ),
            json!({
                "consumer_group_id": item.consumer_group_id,
                "cluster_name": item.cluster_name,
                "recommendation": "Collect group IDs, topic subscriptions, and principals before assessing consumer access",
            }),
        ));
    }

    if item.has_group_evidence && !item.has_acl_evidence {
        findings.push(finding(
            item,
            pillar,
            REASON_SEC_NO_ACL_EVIDENCE,
            Severity::Medium,
            format!(
                "Kafka consumer group {} has no ACL or principal evidence",
                item.group_id
            ),
            json!({
                "consumer_group_id": item.consumer_group_id,
                "has_acl_evidence": item.has_acl_evidence,
                "recommendation": "Collect consumer group ACL bindings and principals before accepting access posture",
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
                "Kafka consumer group inventory for cluster {} is associated with PLAINTEXT client protocol",
                item.cluster_name
            ),
            json!({
                "consumer_group_id": item.consumer_group_id,
                "security_protocol": item.security_protocol,
                "recommendation": "Use SSL or SASL_SSL and validate consumer group ACLs before treating consumer group inventory as secure",
            }),
        ));
    }
}

fn stale_finding(
    item: &KafkaConsumerGroupInventoryItem,
    pillar: Pillar,
    now: DateTime<Utc>,
) -> Option<InventoryFinding> {
    let age_hours = (now - item.collected_at).num_hours();
    if age_hours <= DEFAULT_STALE_AFTER_HOURS {
        return None;
    }

    Some(finding(
        item,
        pillar,
        REASON_INV_STALE_DATA,
        Severity::Medium,
        format!(
            "Kafka consumer group inventory for {} is {} hours old (threshold {} hours)",
            item.consumer_group_id, age_hours, DEFAULT_STALE_AFTER_HOURS
        ),
        json!({
            "consumer_group_id": item.consumer_group_id,
            "collected_at": item.collected_at,
            "age_hours": age_hours,
            "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
        }),
    ))
}

fn has_owner_metadata(item: &KafkaConsumerGroupInventoryItem) -> bool {
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
    item: &KafkaConsumerGroupInventoryItem,
    pillar: Pillar,
    reason_code: &str,
    severity: Severity,
    message: String,
    evidence: Value,
) -> InventoryFinding {
    InventoryFinding {
        resource_id: item.consumer_group_id.clone(),
        arn: format!("kafka:consumer-group/{}", item.consumer_group_id),
        pillar,
        reason_code: reason_code.to_string(),
        severity,
        message,
        evidence,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    fn now() -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2026-06-15T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc)
    }

    fn item() -> KafkaConsumerGroupInventoryItem {
        KafkaConsumerGroupInventoryItem {
            consumer_group_id: "orders:checkout".to_string(),
            cluster_name: "orders".to_string(),
            group_id: "checkout".to_string(),
            owner: None,
            labels: BTreeMap::new(),
            member_count: Some(2),
            committed_offset_count: Some(8),
            total_lag: Some(0),
            has_group_evidence: true,
            has_acl_evidence: true,
            security_protocol: "PLAINTEXT".to_string(),
            collected_at: now(),
        }
    }

    #[test]
    fn cost_flags_missing_owner_missing_evidence_and_high_lag() {
        let mut item = item();
        item.has_group_evidence = false;
        item.total_lag = Some(150_000);

        let report = evaluate_kafka_consumer_group_inventory(&[item], Pillar::Cost, now());

        assert!(report
            .findings
            .iter()
            .any(|f| f.reason_code == REASON_COST_OWNER_NOT_RECORDED));
        assert!(report
            .findings
            .iter()
            .any(|f| f.reason_code == REASON_COST_NO_GROUP_EVIDENCE));
        assert!(report
            .findings
            .iter()
            .any(|f| f.reason_code == REASON_COST_HIGH_LAG));
    }

    #[test]
    fn resilience_flags_missing_evidence_members_and_offsets() {
        let mut missing = item();
        missing.has_group_evidence = false;
        let mut unhealthy = item();
        unhealthy.member_count = Some(0);
        unhealthy.committed_offset_count = Some(0);

        let report = evaluate_kafka_consumer_group_inventory(
            &[missing, unhealthy],
            Pillar::Resilience,
            now(),
        );

        assert!(report
            .findings
            .iter()
            .any(|f| f.reason_code == REASON_RES_NO_GROUP_EVIDENCE));
        assert!(report
            .findings
            .iter()
            .any(|f| f.reason_code == REASON_RES_NO_ACTIVE_MEMBERS));
        assert!(report
            .findings
            .iter()
            .any(|f| f.reason_code == REASON_RES_NO_COMMITTED_OFFSETS));
    }

    #[test]
    fn security_flags_missing_evidence_missing_acl_and_plaintext() {
        let mut missing = item();
        missing.has_group_evidence = false;
        let mut no_acl = item();
        no_acl.has_acl_evidence = false;

        let report =
            evaluate_kafka_consumer_group_inventory(&[missing, no_acl], Pillar::Security, now());

        assert!(report
            .findings
            .iter()
            .any(|f| f.reason_code == REASON_SEC_NO_GROUP_EVIDENCE));
        assert!(report
            .findings
            .iter()
            .any(|f| f.reason_code == REASON_SEC_NO_ACL_EVIDENCE));
        assert!(report
            .findings
            .iter()
            .any(|f| f.reason_code == REASON_SEC_PLAINTEXT));
    }

    #[test]
    fn stale_consumer_group_inventory_is_counted_for_any_pillar() {
        let mut item = item();
        item.owner = Some("platform".to_string());
        item.security_protocol = "SSL".to_string();
        item.collected_at = now() - Duration::hours(DEFAULT_STALE_AFTER_HOURS + 2);

        let report = evaluate_kafka_consumer_group_inventory(&[item], Pillar::Cost, now());

        assert_eq!(report.stale_resources, 1);
        assert!(report
            .findings
            .iter()
            .any(|f| f.reason_code == REASON_INV_STALE_DATA));
    }

    #[test]
    fn healthy_consumer_group_passes_claimed_pillars() {
        let mut item = item();
        item.owner = Some("platform".to_string());
        item.security_protocol = "SSL".to_string();

        for pillar in [Pillar::Cost, Pillar::Resilience, Pillar::Security] {
            let report = evaluate_kafka_consumer_group_inventory(&[item.clone()], pillar, now());
            assert_eq!(report.score, 100);
            assert!(report.findings.is_empty());
        }
    }
}

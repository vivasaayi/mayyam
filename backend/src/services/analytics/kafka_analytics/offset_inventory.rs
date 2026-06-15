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

// Deterministic Kafka offset inventory evaluator for roadmap rows
// 04-KAFKA-DASHBOARD-MANAGEMENT-00393/00400/00421.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::config::KafkaClusterConfig;
use crate::services::aws::inventory::types::{
    score_pillar, InventoryFinding, Pillar, PillarReport, Severity, COST_ALLOCATION_TAG_KEYS,
    DEFAULT_STALE_AFTER_HOURS,
};

pub const RESOURCE_TYPE: &str = "KafkaOffset";
pub const REASON_COST_OWNER_NOT_RECORDED: &str = "KAFKA_OFFSET_COST_OWNER_NOT_RECORDED";
pub const REASON_COST_NO_OFFSET_EVIDENCE: &str = "KAFKA_OFFSET_COST_NO_EVIDENCE";
pub const REASON_COST_HIGH_LAG: &str = "KAFKA_OFFSET_COST_HIGH_LAG";
pub const REASON_RES_NO_OFFSET_EVIDENCE: &str = "KAFKA_OFFSET_RES_NO_EVIDENCE";
pub const REASON_RES_COMMIT_NOT_RECORDED: &str = "KAFKA_OFFSET_RES_COMMIT_NOT_RECORDED";
pub const REASON_RES_OFFSET_BEHIND_END: &str = "KAFKA_OFFSET_RES_BEHIND_END_OFFSET";
pub const REASON_SEC_NO_OFFSET_EVIDENCE: &str = "KAFKA_OFFSET_SEC_NO_EVIDENCE";
pub const REASON_SEC_NO_PRINCIPAL_EVIDENCE: &str = "KAFKA_OFFSET_SEC_NO_PRINCIPAL_EVIDENCE";
pub const REASON_SEC_PLAINTEXT: &str = "KAFKA_OFFSET_SEC_PLAINTEXT_PROTOCOL";
pub const REASON_INV_STALE_DATA: &str = "KAFKA_OFFSET_INV_STALE_DATA";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KafkaOffsetInventoryItem {
    pub offset_id: String,
    pub cluster_name: String,
    pub group_id: String,
    pub topic_name: String,
    pub partition: Option<i32>,
    pub committed_offset: Option<i64>,
    pub end_offset: Option<i64>,
    pub lag: Option<i64>,
    pub owner: Option<String>,
    pub labels: BTreeMap<String, String>,
    pub principal: Option<String>,
    pub has_offset_evidence: bool,
    pub security_protocol: String,
    pub collected_at: DateTime<Utc>,
}

pub fn evaluate_kafka_offset_inventory(
    items: &[KafkaOffsetInventoryItem],
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

pub fn offset_inventory_item_from_config(
    cluster: &KafkaClusterConfig,
    collected_at: DateTime<Utc>,
) -> KafkaOffsetInventoryItem {
    KafkaOffsetInventoryItem {
        offset_id: format!("{}:offsets", cluster.name),
        cluster_name: cluster.name.clone(),
        group_id: "<uncollected>".to_string(),
        topic_name: "<uncollected>".to_string(),
        partition: None,
        committed_offset: None,
        end_offset: None,
        lag: None,
        owner: None,
        labels: BTreeMap::new(),
        principal: None,
        has_offset_evidence: false,
        security_protocol: cluster.security_protocol.clone(),
        collected_at,
    }
}

fn evaluate_cost(
    item: &KafkaOffsetInventoryItem,
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
                "Kafka offset inventory for cluster {} has no owner, team, project, or cost-center metadata",
                item.cluster_name
            ),
            json!({
                "offset_id": item.offset_id,
                "cluster_name": item.cluster_name,
                "checked_keys": COST_ALLOCATION_TAG_KEYS,
            }),
        ));
    }

    if !item.has_offset_evidence {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_NO_OFFSET_EVIDENCE,
            Severity::High,
            format!(
                "Kafka offset inventory for cluster {} has no offset evidence",
                item.cluster_name
            ),
            json!({
                "offset_id": item.offset_id,
                "cluster_name": item.cluster_name,
                "recommendation": "Collect committed offsets, end offsets, lag, ownership, and labels before estimating offset-driven consumer scaling cost",
            }),
        ));
    }

    if item.lag.is_some_and(|lag| lag >= 100_000) {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_HIGH_LAG,
            Severity::Medium,
            format!(
                "Kafka offset {} has lag {}",
                item.offset_id,
                item.lag.unwrap_or_default()
            ),
            json!({
                "offset_id": item.offset_id,
                "lag": item.lag,
                "recommendation": "Validate lag trend and consumer throughput before adding broker or consumer capacity",
            }),
        ));
    }
}

fn evaluate_resilience(
    item: &KafkaOffsetInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if !item.has_offset_evidence {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_NO_OFFSET_EVIDENCE,
            Severity::High,
            format!(
                "Kafka offset inventory for cluster {} has no resilience evidence",
                item.cluster_name
            ),
            json!({
                "offset_id": item.offset_id,
                "cluster_name": item.cluster_name,
                "recommendation": "Collect committed offsets, end offsets, lag, and partition identity before evaluating recovery readiness",
            }),
        ));
    }

    if item.has_offset_evidence && item.committed_offset.is_none() {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_COMMIT_NOT_RECORDED,
            Severity::High,
            format!("Kafka offset {} has no committed offset", item.offset_id),
            json!({
                "offset_id": item.offset_id,
                "committed_offset": item.committed_offset,
                "recommendation": "Collect committed offsets before using this group/topic partition for recovery or migration decisions",
            }),
        ));
    }

    if item
        .committed_offset
        .zip(item.end_offset)
        .is_some_and(|(committed, end)| committed < end)
    {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_OFFSET_BEHIND_END,
            Severity::Medium,
            format!(
                "Kafka offset {} is behind the partition end offset",
                item.offset_id
            ),
            json!({
                "offset_id": item.offset_id,
                "committed_offset": item.committed_offset,
                "end_offset": item.end_offset,
                "lag": item.lag,
                "recommendation": "Verify lag is within recovery objectives before accepting consumer offset posture",
            }),
        ));
    }
}

fn evaluate_security(
    item: &KafkaOffsetInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if !item.has_offset_evidence {
        findings.push(finding(
            item,
            pillar,
            REASON_SEC_NO_OFFSET_EVIDENCE,
            Severity::Medium,
            format!(
                "Kafka offset inventory for cluster {} has no offset evidence for security review",
                item.cluster_name
            ),
            json!({
                "offset_id": item.offset_id,
                "cluster_name": item.cluster_name,
                "recommendation": "Collect consumer group, topic, partition, and principal evidence before assessing offset access",
            }),
        ));
    }

    if item.has_offset_evidence && item.principal.as_deref().unwrap_or("").trim().is_empty() {
        findings.push(finding(
            item,
            pillar,
            REASON_SEC_NO_PRINCIPAL_EVIDENCE,
            Severity::Medium,
            format!("Kafka offset {} has no principal evidence", item.offset_id),
            json!({
                "offset_id": item.offset_id,
                "principal": item.principal,
                "recommendation": "Map offset commits to authenticated principals before accepting consumer access posture",
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
                "Kafka offset inventory for cluster {} uses PLAINTEXT transport",
                item.cluster_name
            ),
            json!({
                "offset_id": item.offset_id,
                "security_protocol": item.security_protocol,
                "recommendation": "Use encrypted Kafka listener protocols before exposing offset and consumer group evidence",
            }),
        ));
    }
}

fn stale_finding(
    item: &KafkaOffsetInventoryItem,
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
            "Kafka offset inventory for cluster {} is {} hour(s) old",
            item.cluster_name, age_hours
        ),
        json!({
            "offset_id": item.offset_id,
            "cluster_name": item.cluster_name,
            "age_hours": age_hours,
            "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
            "recommendation": "Refresh Kafka offset inventory before acting on this posture report",
        }),
    ))
}

fn has_owner_metadata(item: &KafkaOffsetInventoryItem) -> bool {
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
    item: &KafkaOffsetInventoryItem,
    pillar: Pillar,
    reason_code: &'static str,
    severity: Severity,
    message: String,
    evidence: Value,
) -> InventoryFinding {
    InventoryFinding {
        resource_id: item.offset_id.clone(),
        arn: format!("kafka:offset/{}", item.offset_id),
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

    fn healthy_offset(now: DateTime<Utc>) -> KafkaOffsetInventoryItem {
        KafkaOffsetInventoryItem {
            offset_id: "prod:orders:checkout:0".to_string(),
            cluster_name: "prod".to_string(),
            group_id: "checkout".to_string(),
            topic_name: "orders".to_string(),
            partition: Some(0),
            committed_offset: Some(990),
            end_offset: Some(1_000),
            lag: Some(10),
            owner: Some("payments".to_string()),
            labels: BTreeMap::new(),
            principal: Some("User:checkout".to_string()),
            has_offset_evidence: true,
            security_protocol: "SASL_SSL".to_string(),
            collected_at: now,
        }
    }

    #[test]
    fn healthy_offset_passes_claimed_pillars() {
        let now = Utc::now();
        let item = healthy_offset(now);

        for pillar in [Pillar::Cost, Pillar::Security] {
            let report = evaluate_kafka_offset_inventory(std::slice::from_ref(&item), pillar, now);
            assert_eq!(report.resources_evaluated, 1);
            assert!(report.findings.is_empty());
            assert_eq!(report.stale_resources, 0);
            assert_eq!(report.score, 100);
        }
    }

    #[test]
    fn resilience_flags_offset_behind_end_offset() {
        let now = Utc::now();
        let item = healthy_offset(now);

        let report = evaluate_kafka_offset_inventory(&[item], Pillar::Resilience, now);

        assert!(report.findings.iter().any(|finding| {
            finding.reason_code == REASON_RES_OFFSET_BEHIND_END
                && finding.evidence["lag"] == json!(10)
        }));
    }

    #[test]
    fn cost_flags_missing_owner_missing_evidence_and_high_lag() {
        let now = Utc::now();
        let mut item = healthy_offset(now);
        item.owner = None;
        item.lag = Some(250_000);
        item.has_offset_evidence = false;

        let report = evaluate_kafka_offset_inventory(&[item], Pillar::Cost, now);

        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_COST_OWNER_NOT_RECORDED));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_COST_NO_OFFSET_EVIDENCE));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_COST_HIGH_LAG));
    }

    #[test]
    fn resilience_flags_missing_evidence_and_commit() {
        let now = Utc::now();
        let mut missing_evidence = healthy_offset(now);
        missing_evidence.has_offset_evidence = false;
        let mut missing_commit = healthy_offset(now);
        missing_commit.committed_offset = None;

        let report = evaluate_kafka_offset_inventory(
            &[missing_evidence, missing_commit],
            Pillar::Resilience,
            now,
        );

        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_RES_NO_OFFSET_EVIDENCE));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_RES_COMMIT_NOT_RECORDED));
    }

    #[test]
    fn security_flags_missing_evidence_missing_principal_and_plaintext() {
        let now = Utc::now();
        let mut item = healthy_offset(now);
        item.has_offset_evidence = false;
        item.principal = None;
        item.security_protocol = "PLAINTEXT".to_string();

        let report = evaluate_kafka_offset_inventory(&[item], Pillar::Security, now);

        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_SEC_NO_OFFSET_EVIDENCE));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_SEC_PLAINTEXT));
    }

    #[test]
    fn stale_offset_inventory_is_counted_for_any_pillar() {
        let now = Utc::now();
        let mut item = healthy_offset(now);
        item.collected_at = now - Duration::hours(DEFAULT_STALE_AFTER_HOURS + 2);

        let report = evaluate_kafka_offset_inventory(&[item], Pillar::Security, now);

        assert_eq!(report.stale_resources, 1);
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_INV_STALE_DATA));
    }
}

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

// Deterministic Kafka consumer inventory evaluator for roadmap rows
// 04-KAFKA-DASHBOARD-MANAGEMENT-00540/00547/00568.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::config::KafkaClusterConfig;
use crate::services::aws::inventory::types::{
    score_pillar, InventoryFinding, Pillar, PillarReport, Severity, COST_ALLOCATION_TAG_KEYS,
    DEFAULT_STALE_AFTER_HOURS,
};

pub const RESOURCE_TYPE: &str = "KafkaConsumer";
pub const REASON_COST_OWNER_NOT_RECORDED: &str = "KAFKA_CONSUMER_COST_OWNER_NOT_RECORDED";
pub const REASON_COST_NO_CONSUMER_EVIDENCE: &str = "KAFKA_CONSUMER_COST_NO_EVIDENCE";
pub const REASON_COST_HIGH_LAG: &str = "KAFKA_CONSUMER_COST_HIGH_LAG";
pub const REASON_RES_NO_CONSUMER_EVIDENCE: &str = "KAFKA_CONSUMER_RES_NO_EVIDENCE";
pub const REASON_RES_NO_ASSIGNMENTS: &str = "KAFKA_CONSUMER_RES_NO_ASSIGNMENTS";
pub const REASON_RES_NO_HEARTBEAT: &str = "KAFKA_CONSUMER_RES_NO_HEARTBEAT";
pub const REASON_RES_HIGH_ERROR_RATE: &str = "KAFKA_CONSUMER_RES_HIGH_ERROR_RATE";
pub const REASON_SEC_NO_CONSUMER_EVIDENCE: &str = "KAFKA_CONSUMER_SEC_NO_EVIDENCE";
pub const REASON_SEC_NO_PRINCIPAL_EVIDENCE: &str = "KAFKA_CONSUMER_SEC_NO_PRINCIPAL_EVIDENCE";
pub const REASON_SEC_PLAINTEXT: &str = "KAFKA_CONSUMER_SEC_PLAINTEXT_PROTOCOL";
pub const REASON_INV_STALE_DATA: &str = "KAFKA_CONSUMER_INV_STALE_DATA";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KafkaConsumerInventoryItem {
    pub consumer_id: String,
    pub cluster_name: String,
    pub client_id: String,
    pub group_id: String,
    pub topic_names: Vec<String>,
    pub owner: Option<String>,
    pub labels: BTreeMap<String, String>,
    pub assignment_count: Option<i32>,
    pub records_per_second: Option<f64>,
    pub total_lag: Option<i64>,
    pub error_rate: Option<f64>,
    pub last_heartbeat_seconds_ago: Option<i64>,
    pub principal: Option<String>,
    pub has_consumer_evidence: bool,
    pub security_protocol: String,
    pub collected_at: DateTime<Utc>,
}

pub fn evaluate_kafka_consumer_inventory(
    items: &[KafkaConsumerInventoryItem],
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

pub fn consumer_inventory_item_from_config(
    cluster: &KafkaClusterConfig,
    collected_at: DateTime<Utc>,
) -> KafkaConsumerInventoryItem {
    KafkaConsumerInventoryItem {
        consumer_id: format!("{}:consumers", cluster.name),
        cluster_name: cluster.name.clone(),
        client_id: "<uncollected>".to_string(),
        group_id: "<uncollected>".to_string(),
        topic_names: Vec::new(),
        owner: None,
        labels: BTreeMap::new(),
        assignment_count: None,
        records_per_second: None,
        total_lag: None,
        error_rate: None,
        last_heartbeat_seconds_ago: None,
        principal: None,
        has_consumer_evidence: false,
        security_protocol: cluster.security_protocol.clone(),
        collected_at,
    }
}

fn evaluate_cost(
    item: &KafkaConsumerInventoryItem,
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
                "Kafka consumer inventory for cluster {} has no owner, team, project, or cost-center metadata",
                item.cluster_name
            ),
            json!({
                "consumer_id": item.consumer_id,
                "cluster_name": item.cluster_name,
                "checked_keys": COST_ALLOCATION_TAG_KEYS,
            }),
        ));
    }

    if !item.has_consumer_evidence {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_NO_CONSUMER_EVIDENCE,
            Severity::High,
            format!(
                "Kafka consumer inventory for cluster {} has no consumer evidence",
                item.cluster_name
            ),
            json!({
                "consumer_id": item.consumer_id,
                "cluster_name": item.cluster_name,
                "recommendation": "Collect consumer client IDs, group IDs, assignments, lag, throughput, ownership, and labels before estimating consumer-driven cost",
            }),
        ));
    }

    if item.total_lag.is_some_and(|lag| lag >= 100_000) {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_HIGH_LAG,
            Severity::Medium,
            format!("Kafka consumer {} has high lag", item.client_id),
            json!({
                "consumer_id": item.consumer_id,
                "total_lag": item.total_lag,
                "records_per_second": item.records_per_second,
                "recommendation": "Validate lag trend and consumer throughput before adding consumer or broker capacity",
            }),
        ));
    }
}

fn evaluate_resilience(
    item: &KafkaConsumerInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if !item.has_consumer_evidence {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_NO_CONSUMER_EVIDENCE,
            Severity::High,
            format!(
                "Kafka consumer inventory for cluster {} has no resilience evidence",
                item.cluster_name
            ),
            json!({
                "consumer_id": item.consumer_id,
                "cluster_name": item.cluster_name,
                "recommendation": "Collect assignments, lag, heartbeat, and error-rate evidence before evaluating consumer resilience",
            }),
        ));
    }

    if item.has_consumer_evidence && item.assignment_count.unwrap_or_default() == 0 {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_NO_ASSIGNMENTS,
            Severity::High,
            format!("Kafka consumer {} has no assigned partitions", item.client_id),
            json!({
                "consumer_id": item.consumer_id,
                "assignment_count": item.assignment_count,
                "recommendation": "Verify group membership and partition assignment before accepting consumer recovery posture",
            }),
        ));
    }

    if item
        .last_heartbeat_seconds_ago
        .is_some_and(|seconds| seconds >= 60)
    {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_NO_HEARTBEAT,
            Severity::High,
            format!("Kafka consumer {} has stale heartbeat evidence", item.client_id),
            json!({
                "consumer_id": item.consumer_id,
                "last_heartbeat_seconds_ago": item.last_heartbeat_seconds_ago,
                "recommendation": "Investigate consumer process health and rebalance state before accepting resilience posture",
            }),
        ));
    }

    if item.error_rate.is_some_and(|rate| rate >= 0.05) {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_HIGH_ERROR_RATE,
            Severity::High,
            format!("Kafka consumer {} has elevated error rate", item.client_id),
            json!({
                "consumer_id": item.consumer_id,
                "error_rate": item.error_rate,
                "recommendation": "Investigate deserialization, processing, offset commit, and broker errors before accepting consumer resilience",
            }),
        ));
    }
}

fn evaluate_security(
    item: &KafkaConsumerInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if !item.has_consumer_evidence {
        findings.push(finding(
            item,
            pillar,
            REASON_SEC_NO_CONSUMER_EVIDENCE,
            Severity::Medium,
            format!(
                "Kafka consumer inventory for cluster {} has no consumer evidence for security review",
                item.cluster_name
            ),
            json!({
                "consumer_id": item.consumer_id,
                "cluster_name": item.cluster_name,
                "recommendation": "Collect consumer client IDs, read topics, principals, and ACL bindings before assessing consumer access",
            }),
        ));
    }

    if item.has_consumer_evidence && item.principal.as_deref().unwrap_or("").trim().is_empty() {
        findings.push(finding(
            item,
            pillar,
            REASON_SEC_NO_PRINCIPAL_EVIDENCE,
            Severity::Medium,
            format!("Kafka consumer {} has no principal evidence", item.client_id),
            json!({
                "consumer_id": item.consumer_id,
                "principal": item.principal,
                "recommendation": "Map consumer clients to authenticated principals before accepting read-access posture",
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
                "Kafka consumer inventory for cluster {} uses PLAINTEXT transport",
                item.cluster_name
            ),
            json!({
                "consumer_id": item.consumer_id,
                "security_protocol": item.security_protocol,
                "recommendation": "Use encrypted Kafka listener protocols before accepting consumer read-access posture",
            }),
        ));
    }
}

fn stale_finding(
    item: &KafkaConsumerInventoryItem,
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
            "Kafka consumer inventory for cluster {} is {} hour(s) old",
            item.cluster_name, age_hours
        ),
        json!({
            "consumer_id": item.consumer_id,
            "cluster_name": item.cluster_name,
            "age_hours": age_hours,
            "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
            "recommendation": "Refresh Kafka consumer inventory before acting on this posture report",
        }),
    ))
}

fn has_owner_metadata(item: &KafkaConsumerInventoryItem) -> bool {
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
    item: &KafkaConsumerInventoryItem,
    pillar: Pillar,
    reason_code: &'static str,
    severity: Severity,
    message: String,
    evidence: Value,
) -> InventoryFinding {
    InventoryFinding {
        resource_id: item.consumer_id.clone(),
        arn: format!("kafka:consumer/{}", item.consumer_id),
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

    fn consumer(now: DateTime<Utc>) -> KafkaConsumerInventoryItem {
        KafkaConsumerInventoryItem {
            consumer_id: "prod:checkout-consumer".to_string(),
            cluster_name: "prod".to_string(),
            client_id: "checkout-consumer".to_string(),
            group_id: "checkout".to_string(),
            topic_names: vec!["orders".to_string()],
            owner: Some("payments".to_string()),
            labels: BTreeMap::new(),
            assignment_count: Some(4),
            records_per_second: Some(100.0),
            total_lag: Some(0),
            error_rate: Some(0.0),
            last_heartbeat_seconds_ago: Some(5),
            principal: Some("User:checkout-consumer".to_string()),
            has_consumer_evidence: true,
            security_protocol: "SASL_SSL".to_string(),
            collected_at: now,
        }
    }

    #[test]
    fn healthy_consumer_passes_claimed_pillars() {
        let now = Utc::now();
        let item = consumer(now);

        for pillar in [Pillar::Cost, Pillar::Resilience, Pillar::Security] {
            let report =
                evaluate_kafka_consumer_inventory(std::slice::from_ref(&item), pillar, now);
            assert_eq!(report.resources_evaluated, 1);
            assert!(report.findings.is_empty());
            assert_eq!(report.stale_resources, 0);
            assert_eq!(report.score, 100);
        }
    }

    #[test]
    fn cost_flags_missing_owner_missing_evidence_and_high_lag() {
        let now = Utc::now();
        let mut item = consumer(now);
        item.owner = None;
        item.has_consumer_evidence = false;
        item.total_lag = Some(150_000);

        let report = evaluate_kafka_consumer_inventory(&[item], Pillar::Cost, now);

        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_COST_OWNER_NOT_RECORDED));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_COST_NO_CONSUMER_EVIDENCE));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_COST_HIGH_LAG));
    }

    #[test]
    fn resilience_flags_missing_evidence_assignments_heartbeat_and_errors() {
        let now = Utc::now();
        let mut missing_evidence = consumer(now);
        missing_evidence.has_consumer_evidence = false;
        let mut unhealthy = consumer(now);
        unhealthy.assignment_count = Some(0);
        unhealthy.last_heartbeat_seconds_ago = Some(120);
        unhealthy.error_rate = Some(0.08);

        let report = evaluate_kafka_consumer_inventory(
            &[missing_evidence, unhealthy],
            Pillar::Resilience,
            now,
        );

        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_RES_NO_CONSUMER_EVIDENCE));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_RES_NO_ASSIGNMENTS));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_RES_NO_HEARTBEAT));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_RES_HIGH_ERROR_RATE));
    }

    #[test]
    fn security_flags_missing_evidence_missing_principal_and_plaintext() {
        let now = Utc::now();
        let mut missing_evidence = consumer(now);
        missing_evidence.has_consumer_evidence = false;
        let mut plaintext = consumer(now);
        plaintext.principal = None;
        plaintext.security_protocol = "PLAINTEXT".to_string();

        let report = evaluate_kafka_consumer_inventory(
            &[missing_evidence, plaintext],
            Pillar::Security,
            now,
        );

        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_SEC_NO_CONSUMER_EVIDENCE));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_SEC_NO_PRINCIPAL_EVIDENCE));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_SEC_PLAINTEXT));
    }

    #[test]
    fn stale_consumer_inventory_is_counted_for_any_pillar() {
        let now = Utc::now();
        let mut item = consumer(now);
        item.collected_at = now - Duration::hours(DEFAULT_STALE_AFTER_HOURS + 2);

        let report = evaluate_kafka_consumer_inventory(&[item], Pillar::Security, now);

        assert_eq!(report.stale_resources, 1);
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_INV_STALE_DATA));
    }
}

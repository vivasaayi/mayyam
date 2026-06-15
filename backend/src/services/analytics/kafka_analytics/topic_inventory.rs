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

// Deterministic Kafka topic inventory evaluator for roadmap rows
// 04-KAFKA-DASHBOARD-MANAGEMENT-00148/00155/00176.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::config::KafkaClusterConfig;
use crate::services::aws::inventory::types::{
    score_pillar, InventoryFinding, Pillar, PillarReport, Severity, COST_ALLOCATION_TAG_KEYS,
    DEFAULT_STALE_AFTER_HOURS,
};

pub const RESOURCE_TYPE: &str = "KafkaTopic";
pub const REASON_COST_OWNER_NOT_RECORDED: &str = "KAFKA_TOPIC_COST_OWNER_NOT_RECORDED";
pub const REASON_COST_NO_TOPIC_EVIDENCE: &str = "KAFKA_TOPIC_COST_NO_EVIDENCE";
pub const REASON_COST_HIGH_PARTITION_COUNT: &str = "KAFKA_TOPIC_COST_HIGH_PARTITION_COUNT";
pub const REASON_RES_NO_TOPIC_EVIDENCE: &str = "KAFKA_TOPIC_RES_NO_EVIDENCE";
pub const REASON_RES_UNDER_REPLICATED: &str = "KAFKA_TOPIC_RES_UNDER_REPLICATED";
pub const REASON_SEC_NO_TOPIC_EVIDENCE: &str = "KAFKA_TOPIC_SEC_NO_EVIDENCE";
pub const REASON_SEC_PLAINTEXT: &str = "KAFKA_TOPIC_SEC_PLAINTEXT_PROTOCOL";
pub const REASON_INV_STALE_DATA: &str = "KAFKA_TOPIC_INV_STALE_DATA";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KafkaTopicInventoryItem {
    pub topic_id: String,
    pub cluster_name: String,
    pub topic_name: String,
    pub owner: Option<String>,
    pub labels: BTreeMap<String, String>,
    pub partition_count: Option<i32>,
    pub replication_factor: Option<i32>,
    pub security_protocol: String,
    pub has_topic_evidence: bool,
    pub collected_at: DateTime<Utc>,
}

pub fn evaluate_kafka_topic_inventory(
    items: &[KafkaTopicInventoryItem],
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

pub fn topic_inventory_item_from_config(
    cluster: &KafkaClusterConfig,
    collected_at: DateTime<Utc>,
) -> KafkaTopicInventoryItem {
    KafkaTopicInventoryItem {
        topic_id: format!("{}:topics", cluster.name),
        cluster_name: cluster.name.clone(),
        topic_name: "<uncollected>".to_string(),
        owner: None,
        labels: BTreeMap::new(),
        partition_count: None,
        replication_factor: None,
        security_protocol: cluster.security_protocol.clone(),
        has_topic_evidence: false,
        collected_at,
    }
}

fn evaluate_cost(
    item: &KafkaTopicInventoryItem,
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
                "Kafka topic inventory for cluster {} has no owner, team, project, or cost-center metadata",
                item.cluster_name
            ),
            json!({
                "topic_id": item.topic_id,
                "cluster_name": item.cluster_name,
                "topic_name": item.topic_name,
                "checked_keys": COST_ALLOCATION_TAG_KEYS,
            }),
        ));
    }

    if !item.has_topic_evidence {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_NO_TOPIC_EVIDENCE,
            Severity::High,
            format!(
                "Kafka topic inventory for cluster {} has no topic evidence",
                item.cluster_name
            ),
            json!({
                "topic_id": item.topic_id,
                "cluster_name": item.cluster_name,
                "recommendation": "Collect topic names, partitions, replication factors, configs, and ownership before attributing topic-level spend",
            }),
        ));
    }

    if item.partition_count.unwrap_or_default() >= 100 {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_HIGH_PARTITION_COUNT,
            Severity::Medium,
            format!(
                "Kafka topic {} has {} partition(s), which can increase broker overhead",
                item.topic_name,
                item.partition_count.unwrap_or_default()
            ),
            json!({
                "topic_id": item.topic_id,
                "partition_count": item.partition_count,
                "recommendation": "Validate partition count against throughput and consumer parallelism before adding broker capacity",
            }),
        ));
    }
}

fn evaluate_resilience(
    item: &KafkaTopicInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if !item.has_topic_evidence {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_NO_TOPIC_EVIDENCE,
            Severity::High,
            format!(
                "Kafka topic inventory for cluster {} has no resilience evidence",
                item.cluster_name
            ),
            json!({
                "topic_id": item.topic_id,
                "cluster_name": item.cluster_name,
                "recommendation": "Collect topic partitions, replica assignments, replication factor, and ISR state before evaluating topic resilience",
            }),
        ));
    }

    if item.replication_factor.is_some_and(|replicas| replicas < 3) {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_UNDER_REPLICATED,
            Severity::High,
            format!(
                "Kafka topic {} has replication factor {}",
                item.topic_name,
                item.replication_factor.unwrap_or_default()
            ),
            json!({
                "topic_id": item.topic_id,
                "replication_factor": item.replication_factor,
                "recommendation": "Use replication factor 3 or better for production topics unless the topic is explicitly non-critical",
            }),
        ));
    }
}

fn evaluate_security(
    item: &KafkaTopicInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if !item.has_topic_evidence {
        findings.push(finding(
            item,
            pillar,
            REASON_SEC_NO_TOPIC_EVIDENCE,
            Severity::Medium,
            format!(
                "Kafka topic inventory for cluster {} has no topic evidence for security review",
                item.cluster_name
            ),
            json!({
                "topic_id": item.topic_id,
                "cluster_name": item.cluster_name,
                "recommendation": "Collect topic inventory and ACL bindings before assessing sensitive-topic exposure",
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
                "Kafka topic inventory for cluster {} is associated with PLAINTEXT client protocol",
                item.cluster_name
            ),
            json!({
                "topic_id": item.topic_id,
                "security_protocol": item.security_protocol,
                "recommendation": "Use SSL or SASL_SSL and validate topic ACLs before treating topic inventory as secure",
            }),
        ));
    }
}

fn stale_finding(
    item: &KafkaTopicInventoryItem,
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
            "Kafka topic inventory for {} is {} hours old (threshold {} hours)",
            item.topic_id, age_hours, DEFAULT_STALE_AFTER_HOURS
        ),
        json!({
            "topic_id": item.topic_id,
            "collected_at": item.collected_at,
            "age_hours": age_hours,
            "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
        }),
    ))
}

fn has_owner_metadata(item: &KafkaTopicInventoryItem) -> bool {
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
    item: &KafkaTopicInventoryItem,
    pillar: Pillar,
    reason_code: &str,
    severity: Severity,
    message: String,
    evidence: Value,
) -> InventoryFinding {
    InventoryFinding {
        resource_id: item.topic_id.clone(),
        arn: format!("kafka:topic/{}", item.topic_id),
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

    fn item() -> KafkaTopicInventoryItem {
        KafkaTopicInventoryItem {
            topic_id: "orders:events".to_string(),
            cluster_name: "orders".to_string(),
            topic_name: "events".to_string(),
            owner: None,
            labels: BTreeMap::new(),
            partition_count: Some(12),
            replication_factor: Some(1),
            security_protocol: "PLAINTEXT".to_string(),
            has_topic_evidence: true,
            collected_at: now(),
        }
    }

    #[test]
    fn cost_flags_missing_owner_missing_evidence_and_high_partition_count() {
        let mut item = item();
        item.has_topic_evidence = false;
        item.partition_count = Some(128);

        let report = evaluate_kafka_topic_inventory(&[item], Pillar::Cost, now());

        assert!(report
            .findings
            .iter()
            .any(|f| f.reason_code == REASON_COST_OWNER_NOT_RECORDED));
        assert!(report
            .findings
            .iter()
            .any(|f| f.reason_code == REASON_COST_NO_TOPIC_EVIDENCE));
        assert!(report
            .findings
            .iter()
            .any(|f| f.reason_code == REASON_COST_HIGH_PARTITION_COUNT));
    }

    #[test]
    fn resilience_flags_missing_evidence_and_under_replicated_topics() {
        let mut missing = item();
        missing.has_topic_evidence = false;
        let under_replicated = item();

        let report =
            evaluate_kafka_topic_inventory(&[missing, under_replicated], Pillar::Resilience, now());

        assert!(report
            .findings
            .iter()
            .any(|f| f.reason_code == REASON_RES_NO_TOPIC_EVIDENCE));
        assert!(report
            .findings
            .iter()
            .any(|f| f.reason_code == REASON_RES_UNDER_REPLICATED));
    }

    #[test]
    fn security_flags_missing_evidence_and_plaintext_protocol() {
        let mut item = item();
        item.has_topic_evidence = false;

        let report = evaluate_kafka_topic_inventory(&[item], Pillar::Security, now());

        assert!(report
            .findings
            .iter()
            .any(|f| f.reason_code == REASON_SEC_NO_TOPIC_EVIDENCE));
        assert!(report
            .findings
            .iter()
            .any(|f| f.reason_code == REASON_SEC_PLAINTEXT));
    }

    #[test]
    fn stale_topic_inventory_is_counted_for_any_pillar() {
        let mut item = item();
        item.owner = Some("platform".to_string());
        item.replication_factor = Some(3);
        item.security_protocol = "SSL".to_string();
        item.collected_at = now() - Duration::hours(DEFAULT_STALE_AFTER_HOURS + 2);

        let report = evaluate_kafka_topic_inventory(&[item], Pillar::Cost, now());

        assert_eq!(report.stale_resources, 1);
        assert!(report
            .findings
            .iter()
            .any(|f| f.reason_code == REASON_INV_STALE_DATA));
    }

    #[test]
    fn healthy_topic_passes_claimed_pillars() {
        let mut item = item();
        item.owner = Some("platform".to_string());
        item.replication_factor = Some(3);
        item.security_protocol = "SSL".to_string();

        for pillar in [Pillar::Cost, Pillar::Resilience, Pillar::Security] {
            let report = evaluate_kafka_topic_inventory(&[item.clone()], pillar, now());
            assert_eq!(report.score, 100);
            assert!(report.findings.is_empty());
        }
    }
}

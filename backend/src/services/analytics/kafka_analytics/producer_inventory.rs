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

// Deterministic Kafka producer inventory evaluator for roadmap rows
// 04-KAFKA-DASHBOARD-MANAGEMENT-00491/00498/00519.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::config::KafkaClusterConfig;
use crate::services::aws::inventory::types::{
    score_pillar, InventoryFinding, Pillar, PillarReport, Severity, COST_ALLOCATION_TAG_KEYS,
    DEFAULT_STALE_AFTER_HOURS,
};

pub const RESOURCE_TYPE: &str = "KafkaProducer";
pub const REASON_COST_OWNER_NOT_RECORDED: &str = "KAFKA_PRODUCER_COST_OWNER_NOT_RECORDED";
pub const REASON_COST_NO_PRODUCER_EVIDENCE: &str = "KAFKA_PRODUCER_COST_NO_EVIDENCE";
pub const REASON_COST_HIGH_THROUGHPUT: &str = "KAFKA_PRODUCER_COST_HIGH_THROUGHPUT";
pub const REASON_RES_NO_PRODUCER_EVIDENCE: &str = "KAFKA_PRODUCER_RES_NO_EVIDENCE";
pub const REASON_RES_IDEMPOTENCE_DISABLED: &str = "KAFKA_PRODUCER_RES_IDEMPOTENCE_DISABLED";
pub const REASON_RES_ACKS_NOT_ALL: &str = "KAFKA_PRODUCER_RES_ACKS_NOT_ALL";
pub const REASON_RES_HIGH_ERROR_RATE: &str = "KAFKA_PRODUCER_RES_HIGH_ERROR_RATE";
pub const REASON_SEC_NO_PRODUCER_EVIDENCE: &str = "KAFKA_PRODUCER_SEC_NO_EVIDENCE";
pub const REASON_SEC_NO_PRINCIPAL_EVIDENCE: &str = "KAFKA_PRODUCER_SEC_NO_PRINCIPAL_EVIDENCE";
pub const REASON_SEC_PLAINTEXT: &str = "KAFKA_PRODUCER_SEC_PLAINTEXT_PROTOCOL";
pub const REASON_INV_STALE_DATA: &str = "KAFKA_PRODUCER_INV_STALE_DATA";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KafkaProducerInventoryItem {
    pub producer_id: String,
    pub cluster_name: String,
    pub client_id: String,
    pub topic_names: Vec<String>,
    pub owner: Option<String>,
    pub labels: BTreeMap<String, String>,
    pub records_per_second: Option<f64>,
    pub bytes_per_second: Option<f64>,
    pub error_rate: Option<f64>,
    pub idempotence_enabled: Option<bool>,
    pub acks: Option<String>,
    pub principal: Option<String>,
    pub has_producer_evidence: bool,
    pub security_protocol: String,
    pub collected_at: DateTime<Utc>,
}

pub fn evaluate_kafka_producer_inventory(
    items: &[KafkaProducerInventoryItem],
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

pub fn producer_inventory_item_from_config(
    cluster: &KafkaClusterConfig,
    collected_at: DateTime<Utc>,
) -> KafkaProducerInventoryItem {
    KafkaProducerInventoryItem {
        producer_id: format!("{}:producers", cluster.name),
        cluster_name: cluster.name.clone(),
        client_id: "<uncollected>".to_string(),
        topic_names: Vec::new(),
        owner: None,
        labels: BTreeMap::new(),
        records_per_second: None,
        bytes_per_second: None,
        error_rate: None,
        idempotence_enabled: None,
        acks: None,
        principal: None,
        has_producer_evidence: false,
        security_protocol: cluster.security_protocol.clone(),
        collected_at,
    }
}

fn evaluate_cost(
    item: &KafkaProducerInventoryItem,
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
                "Kafka producer inventory for cluster {} has no owner, team, project, or cost-center metadata",
                item.cluster_name
            ),
            json!({
                "producer_id": item.producer_id,
                "cluster_name": item.cluster_name,
                "checked_keys": COST_ALLOCATION_TAG_KEYS,
            }),
        ));
    }

    if !item.has_producer_evidence {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_NO_PRODUCER_EVIDENCE,
            Severity::High,
            format!(
                "Kafka producer inventory for cluster {} has no producer evidence",
                item.cluster_name
            ),
            json!({
                "producer_id": item.producer_id,
                "cluster_name": item.cluster_name,
                "recommendation": "Collect producer client IDs, topics, throughput, ownership, and labels before estimating producer-driven broker cost",
            }),
        ));
    }

    if item
        .bytes_per_second
        .is_some_and(|bytes| bytes >= 100_000_000.0)
        || item
            .records_per_second
            .is_some_and(|records| records >= 100_000.0)
    {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_HIGH_THROUGHPUT,
            Severity::Medium,
            format!("Kafka producer {} has high write throughput", item.client_id),
            json!({
                "producer_id": item.producer_id,
                "records_per_second": item.records_per_second,
                "bytes_per_second": item.bytes_per_second,
                "recommendation": "Validate batching, compression, and topic partitioning before increasing broker capacity for producer throughput",
            }),
        ));
    }
}

fn evaluate_resilience(
    item: &KafkaProducerInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if !item.has_producer_evidence {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_NO_PRODUCER_EVIDENCE,
            Severity::High,
            format!(
                "Kafka producer inventory for cluster {} has no resilience evidence",
                item.cluster_name
            ),
            json!({
                "producer_id": item.producer_id,
                "cluster_name": item.cluster_name,
                "recommendation": "Collect producer configuration, error rate, acknowledgements, and idempotence evidence before evaluating write resilience",
            }),
        ));
    }

    if item.has_producer_evidence && item.idempotence_enabled == Some(false) {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_IDEMPOTENCE_DISABLED,
            Severity::High,
            format!("Kafka producer {} has idempotence disabled", item.client_id),
            json!({
                "producer_id": item.producer_id,
                "idempotence_enabled": item.idempotence_enabled,
                "recommendation": "Enable idempotent producers for workloads that require duplicate-safe retries",
            }),
        ));
    }

    if item
        .acks
        .as_deref()
        .is_some_and(|acks| !acks.eq_ignore_ascii_case("all"))
    {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_ACKS_NOT_ALL,
            Severity::Medium,
            format!("Kafka producer {} does not require all acknowledgements", item.client_id),
            json!({
                "producer_id": item.producer_id,
                "acks": item.acks,
                "recommendation": "Use acks=all for critical producers unless the workload has an explicit durability exception",
            }),
        ));
    }

    if item.error_rate.is_some_and(|rate| rate >= 0.05) {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_HIGH_ERROR_RATE,
            Severity::High,
            format!("Kafka producer {} has elevated error rate", item.client_id),
            json!({
                "producer_id": item.producer_id,
                "error_rate": item.error_rate,
                "recommendation": "Investigate producer retries, broker throttling, authentication failures, and topic availability before accepting resilience posture",
            }),
        ));
    }
}

fn evaluate_security(
    item: &KafkaProducerInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if !item.has_producer_evidence {
        findings.push(finding(
            item,
            pillar,
            REASON_SEC_NO_PRODUCER_EVIDENCE,
            Severity::Medium,
            format!(
                "Kafka producer inventory for cluster {} has no producer evidence for security review",
                item.cluster_name
            ),
            json!({
                "producer_id": item.producer_id,
                "cluster_name": item.cluster_name,
                "recommendation": "Collect producer client IDs, write topics, principals, and ACL bindings before assessing producer access",
            }),
        ));
    }

    if item.has_producer_evidence && item.principal.as_deref().unwrap_or("").trim().is_empty() {
        findings.push(finding(
            item,
            pillar,
            REASON_SEC_NO_PRINCIPAL_EVIDENCE,
            Severity::Medium,
            format!("Kafka producer {} has no principal evidence", item.client_id),
            json!({
                "producer_id": item.producer_id,
                "principal": item.principal,
                "recommendation": "Map producer clients to authenticated principals before accepting write-access posture",
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
                "Kafka producer inventory for cluster {} uses PLAINTEXT transport",
                item.cluster_name
            ),
            json!({
                "producer_id": item.producer_id,
                "security_protocol": item.security_protocol,
                "recommendation": "Use encrypted Kafka listener protocols before accepting producer write-access posture",
            }),
        ));
    }
}

fn stale_finding(
    item: &KafkaProducerInventoryItem,
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
            "Kafka producer inventory for cluster {} is {} hour(s) old",
            item.cluster_name, age_hours
        ),
        json!({
            "producer_id": item.producer_id,
            "cluster_name": item.cluster_name,
            "age_hours": age_hours,
            "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
            "recommendation": "Refresh Kafka producer inventory before acting on this posture report",
        }),
    ))
}

fn has_owner_metadata(item: &KafkaProducerInventoryItem) -> bool {
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
    item: &KafkaProducerInventoryItem,
    pillar: Pillar,
    reason_code: &'static str,
    severity: Severity,
    message: String,
    evidence: Value,
) -> InventoryFinding {
    InventoryFinding {
        resource_id: item.producer_id.clone(),
        arn: format!("kafka:producer/{}", item.producer_id),
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

    fn producer(now: DateTime<Utc>) -> KafkaProducerInventoryItem {
        KafkaProducerInventoryItem {
            producer_id: "prod:checkout-producer".to_string(),
            cluster_name: "prod".to_string(),
            client_id: "checkout-producer".to_string(),
            topic_names: vec!["orders".to_string()],
            owner: Some("payments".to_string()),
            labels: BTreeMap::new(),
            records_per_second: Some(100.0),
            bytes_per_second: Some(1_000_000.0),
            error_rate: Some(0.0),
            idempotence_enabled: Some(true),
            acks: Some("all".to_string()),
            principal: Some("User:checkout-producer".to_string()),
            has_producer_evidence: true,
            security_protocol: "SASL_SSL".to_string(),
            collected_at: now,
        }
    }

    #[test]
    fn healthy_producer_passes_claimed_pillars() {
        let now = Utc::now();
        let item = producer(now);

        for pillar in [Pillar::Cost, Pillar::Resilience, Pillar::Security] {
            let report =
                evaluate_kafka_producer_inventory(std::slice::from_ref(&item), pillar, now);
            assert_eq!(report.resources_evaluated, 1);
            assert!(report.findings.is_empty());
            assert_eq!(report.stale_resources, 0);
            assert_eq!(report.score, 100);
        }
    }

    #[test]
    fn cost_flags_missing_owner_missing_evidence_and_high_throughput() {
        let now = Utc::now();
        let mut item = producer(now);
        item.owner = None;
        item.has_producer_evidence = false;
        item.records_per_second = Some(150_000.0);

        let report = evaluate_kafka_producer_inventory(&[item], Pillar::Cost, now);

        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_COST_OWNER_NOT_RECORDED));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_COST_NO_PRODUCER_EVIDENCE));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_COST_HIGH_THROUGHPUT));
    }

    #[test]
    fn resilience_flags_missing_evidence_idempotence_acks_and_errors() {
        let now = Utc::now();
        let mut missing_evidence = producer(now);
        missing_evidence.has_producer_evidence = false;
        let mut risky = producer(now);
        risky.idempotence_enabled = Some(false);
        risky.acks = Some("1".to_string());
        risky.error_rate = Some(0.08);

        let report =
            evaluate_kafka_producer_inventory(&[missing_evidence, risky], Pillar::Resilience, now);

        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_RES_NO_PRODUCER_EVIDENCE));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_RES_IDEMPOTENCE_DISABLED));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_RES_ACKS_NOT_ALL));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_RES_HIGH_ERROR_RATE));
    }

    #[test]
    fn security_flags_missing_evidence_missing_principal_and_plaintext() {
        let now = Utc::now();
        let mut missing_evidence = producer(now);
        missing_evidence.has_producer_evidence = false;
        let mut plaintext = producer(now);
        plaintext.principal = None;
        plaintext.security_protocol = "PLAINTEXT".to_string();

        let report = evaluate_kafka_producer_inventory(
            &[missing_evidence, plaintext],
            Pillar::Security,
            now,
        );

        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_SEC_NO_PRODUCER_EVIDENCE));
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
    fn stale_producer_inventory_is_counted_for_any_pillar() {
        let now = Utc::now();
        let mut item = producer(now);
        item.collected_at = now - Duration::hours(DEFAULT_STALE_AFTER_HOURS + 2);

        let report = evaluate_kafka_producer_inventory(&[item], Pillar::Security, now);

        assert_eq!(report.stale_resources, 1);
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_INV_STALE_DATA));
    }
}

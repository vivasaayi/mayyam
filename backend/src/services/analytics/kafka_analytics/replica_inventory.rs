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

// Deterministic Kafka replica inventory evaluator for roadmap rows
// 04-KAFKA-DASHBOARD-MANAGEMENT-00246/00253/00274.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::config::KafkaClusterConfig;
use crate::services::aws::inventory::types::{
    score_pillar, InventoryFinding, Pillar, PillarReport, Severity, COST_ALLOCATION_TAG_KEYS,
    DEFAULT_STALE_AFTER_HOURS,
};

pub const RESOURCE_TYPE: &str = "KafkaReplica";
pub const REASON_COST_OWNER_NOT_RECORDED: &str = "KAFKA_REPLICA_COST_OWNER_NOT_RECORDED";
pub const REASON_COST_NO_REPLICA_EVIDENCE: &str = "KAFKA_REPLICA_COST_NO_EVIDENCE";
pub const REASON_COST_HIGH_REPLICA_COUNT: &str = "KAFKA_REPLICA_COST_HIGH_REPLICA_COUNT";
pub const REASON_RES_NO_REPLICA_EVIDENCE: &str = "KAFKA_REPLICA_RES_NO_EVIDENCE";
pub const REASON_RES_NOT_IN_SYNC: &str = "KAFKA_REPLICA_RES_NOT_IN_SYNC";
pub const REASON_RES_NO_BROKER_ASSIGNMENT: &str = "KAFKA_REPLICA_RES_NO_BROKER_ASSIGNMENT";
pub const REASON_SEC_NO_REPLICA_EVIDENCE: &str = "KAFKA_REPLICA_SEC_NO_EVIDENCE";
pub const REASON_SEC_PLAINTEXT: &str = "KAFKA_REPLICA_SEC_PLAINTEXT_PROTOCOL";
pub const REASON_INV_STALE_DATA: &str = "KAFKA_REPLICA_INV_STALE_DATA";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KafkaReplicaInventoryItem {
    pub replica_id: String,
    pub cluster_name: String,
    pub topic_name: String,
    pub partition: Option<i32>,
    pub broker_id: Option<i32>,
    pub replica_count: Option<i32>,
    pub in_sync: Option<bool>,
    pub owner: Option<String>,
    pub labels: BTreeMap<String, String>,
    pub security_protocol: String,
    pub has_replica_evidence: bool,
    pub collected_at: DateTime<Utc>,
}

pub fn evaluate_kafka_replica_inventory(
    items: &[KafkaReplicaInventoryItem],
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

pub fn replica_inventory_item_from_config(
    cluster: &KafkaClusterConfig,
    collected_at: DateTime<Utc>,
) -> KafkaReplicaInventoryItem {
    KafkaReplicaInventoryItem {
        replica_id: format!("{}:replicas", cluster.name),
        cluster_name: cluster.name.clone(),
        topic_name: "<uncollected>".to_string(),
        partition: None,
        broker_id: None,
        replica_count: None,
        in_sync: None,
        owner: None,
        labels: BTreeMap::new(),
        security_protocol: cluster.security_protocol.clone(),
        has_replica_evidence: false,
        collected_at,
    }
}

fn evaluate_cost(
    item: &KafkaReplicaInventoryItem,
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
                "Kafka replica inventory for cluster {} has no owner, team, project, or cost-center metadata",
                item.cluster_name
            ),
            json!({
                "replica_id": item.replica_id,
                "cluster_name": item.cluster_name,
                "checked_keys": COST_ALLOCATION_TAG_KEYS,
            }),
        ));
    }

    if !item.has_replica_evidence {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_NO_REPLICA_EVIDENCE,
            Severity::High,
            format!(
                "Kafka replica inventory for cluster {} has no replica evidence",
                item.cluster_name
            ),
            json!({
                "replica_id": item.replica_id,
                "cluster_name": item.cluster_name,
                "recommendation": "Collect replica assignments, broker IDs, ISR state, ownership, and labels before attributing replica-level cost",
            }),
        ));
    }

    if item.replica_count.is_some_and(|replicas| replicas >= 4) {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_HIGH_REPLICA_COUNT,
            Severity::Medium,
            format!(
                "Kafka replica set {} has {} replica(s), which can increase storage and broker overhead",
                item.replica_id,
                item.replica_count.unwrap_or_default()
            ),
            json!({
                "replica_id": item.replica_id,
                "replica_count": item.replica_count,
                "recommendation": "Validate replica count against durability requirements before expanding broker capacity",
            }),
        ));
    }
}

fn evaluate_resilience(
    item: &KafkaReplicaInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if !item.has_replica_evidence {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_NO_REPLICA_EVIDENCE,
            Severity::High,
            format!(
                "Kafka replica inventory for cluster {} has no resilience evidence",
                item.cluster_name
            ),
            json!({
                "replica_id": item.replica_id,
                "cluster_name": item.cluster_name,
                "recommendation": "Collect replica assignment and ISR evidence before evaluating replica resilience",
            }),
        ));
    }

    if item.has_replica_evidence && item.broker_id.is_none() {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_NO_BROKER_ASSIGNMENT,
            Severity::High,
            format!("Kafka replica {} has no recorded broker assignment", item.replica_id),
            json!({
                "replica_id": item.replica_id,
                "broker_id": item.broker_id,
                "recommendation": "Collect broker assignment for every replica before accepting placement as resilient",
            }),
        ));
    }

    if item.in_sync == Some(false) {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_NOT_IN_SYNC,
            Severity::High,
            format!("Kafka replica {} is not in sync", item.replica_id),
            json!({
                "replica_id": item.replica_id,
                "in_sync": item.in_sync,
                "recommendation": "Investigate lagging or offline replicas before treating the partition as protected",
            }),
        ));
    }
}

fn evaluate_security(
    item: &KafkaReplicaInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if !item.has_replica_evidence {
        findings.push(finding(
            item,
            pillar,
            REASON_SEC_NO_REPLICA_EVIDENCE,
            Severity::Medium,
            format!(
                "Kafka replica inventory for cluster {} has no replica evidence for security review",
                item.cluster_name
            ),
            json!({
                "replica_id": item.replica_id,
                "cluster_name": item.cluster_name,
                "recommendation": "Collect replica placement and topic classification before assessing sensitive replica exposure",
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
                "Kafka replica inventory for cluster {} is associated with PLAINTEXT client protocol",
                item.cluster_name
            ),
            json!({
                "replica_id": item.replica_id,
                "security_protocol": item.security_protocol,
                "recommendation": "Use SSL or SASL_SSL and validate topic ACLs before treating replica inventory as secure",
            }),
        ));
    }
}

fn stale_finding(
    item: &KafkaReplicaInventoryItem,
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
            "Kafka replica inventory for {} is {} hours old (threshold {} hours)",
            item.replica_id, age_hours, DEFAULT_STALE_AFTER_HOURS
        ),
        json!({
            "replica_id": item.replica_id,
            "collected_at": item.collected_at,
            "age_hours": age_hours,
            "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
        }),
    ))
}

fn has_owner_metadata(item: &KafkaReplicaInventoryItem) -> bool {
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
    item: &KafkaReplicaInventoryItem,
    pillar: Pillar,
    reason_code: &str,
    severity: Severity,
    message: String,
    evidence: Value,
) -> InventoryFinding {
    InventoryFinding {
        resource_id: item.replica_id.clone(),
        arn: format!("kafka:replica/{}", item.replica_id),
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

    fn item() -> KafkaReplicaInventoryItem {
        KafkaReplicaInventoryItem {
            replica_id: "orders:events:0:broker-1".to_string(),
            cluster_name: "orders".to_string(),
            topic_name: "events".to_string(),
            partition: Some(0),
            broker_id: Some(1),
            replica_count: Some(3),
            in_sync: Some(true),
            owner: None,
            labels: BTreeMap::new(),
            security_protocol: "PLAINTEXT".to_string(),
            has_replica_evidence: true,
            collected_at: now(),
        }
    }

    #[test]
    fn cost_flags_missing_owner_missing_evidence_and_high_replica_count() {
        let mut item = item();
        item.has_replica_evidence = false;
        item.replica_count = Some(4);

        let report = evaluate_kafka_replica_inventory(&[item], Pillar::Cost, now());

        assert!(report
            .findings
            .iter()
            .any(|f| f.reason_code == REASON_COST_OWNER_NOT_RECORDED));
        assert!(report
            .findings
            .iter()
            .any(|f| f.reason_code == REASON_COST_NO_REPLICA_EVIDENCE));
        assert!(report
            .findings
            .iter()
            .any(|f| f.reason_code == REASON_COST_HIGH_REPLICA_COUNT));
    }

    #[test]
    fn resilience_flags_missing_evidence_missing_broker_and_out_of_sync() {
        let mut missing = item();
        missing.has_replica_evidence = false;
        let mut unhealthy = item();
        unhealthy.broker_id = None;
        unhealthy.in_sync = Some(false);

        let report =
            evaluate_kafka_replica_inventory(&[missing, unhealthy], Pillar::Resilience, now());

        assert!(report
            .findings
            .iter()
            .any(|f| f.reason_code == REASON_RES_NO_REPLICA_EVIDENCE));
        assert!(report
            .findings
            .iter()
            .any(|f| f.reason_code == REASON_RES_NO_BROKER_ASSIGNMENT));
        assert!(report
            .findings
            .iter()
            .any(|f| f.reason_code == REASON_RES_NOT_IN_SYNC));
    }

    #[test]
    fn security_flags_missing_evidence_and_plaintext_protocol() {
        let mut item = item();
        item.has_replica_evidence = false;

        let report = evaluate_kafka_replica_inventory(&[item], Pillar::Security, now());

        assert!(report
            .findings
            .iter()
            .any(|f| f.reason_code == REASON_SEC_NO_REPLICA_EVIDENCE));
        assert!(report
            .findings
            .iter()
            .any(|f| f.reason_code == REASON_SEC_PLAINTEXT));
    }

    #[test]
    fn stale_replica_inventory_is_counted_for_any_pillar() {
        let mut item = item();
        item.owner = Some("platform".to_string());
        item.security_protocol = "SSL".to_string();
        item.collected_at = now() - Duration::hours(DEFAULT_STALE_AFTER_HOURS + 2);

        let report = evaluate_kafka_replica_inventory(&[item], Pillar::Cost, now());

        assert_eq!(report.stale_resources, 1);
        assert!(report
            .findings
            .iter()
            .any(|f| f.reason_code == REASON_INV_STALE_DATA));
    }

    #[test]
    fn healthy_replica_passes_claimed_pillars() {
        let mut item = item();
        item.owner = Some("platform".to_string());
        item.security_protocol = "SSL".to_string();

        for pillar in [Pillar::Cost, Pillar::Resilience, Pillar::Security] {
            let report = evaluate_kafka_replica_inventory(&[item.clone()], pillar, now());
            assert_eq!(report.score, 100);
            assert!(report.findings.is_empty());
        }
    }
}

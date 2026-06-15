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

// Deterministic Kafka ISR health inventory evaluator for roadmap rows
// 04-KAFKA-DASHBOARD-MANAGEMENT-00295/00302/00323.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::config::KafkaClusterConfig;
use crate::services::aws::inventory::types::{
    score_pillar, InventoryFinding, Pillar, PillarReport, Severity, COST_ALLOCATION_TAG_KEYS,
    DEFAULT_STALE_AFTER_HOURS,
};

pub const RESOURCE_TYPE: &str = "KafkaIsrHealth";
pub const REASON_COST_OWNER_NOT_RECORDED: &str = "KAFKA_ISR_COST_OWNER_NOT_RECORDED";
pub const REASON_COST_NO_ISR_EVIDENCE: &str = "KAFKA_ISR_COST_NO_EVIDENCE";
pub const REASON_RES_NO_ISR_EVIDENCE: &str = "KAFKA_ISR_RES_NO_EVIDENCE";
pub const REASON_RES_ISR_BELOW_REPLICA_COUNT: &str = "KAFKA_ISR_RES_BELOW_REPLICA_COUNT";
pub const REASON_RES_OFFLINE_PARTITION: &str = "KAFKA_ISR_RES_OFFLINE_PARTITION";
pub const REASON_SEC_NO_ISR_EVIDENCE: &str = "KAFKA_ISR_SEC_NO_EVIDENCE";
pub const REASON_SEC_PLAINTEXT: &str = "KAFKA_ISR_SEC_PLAINTEXT_PROTOCOL";
pub const REASON_INV_STALE_DATA: &str = "KAFKA_ISR_INV_STALE_DATA";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KafkaIsrHealthInventoryItem {
    pub isr_id: String,
    pub cluster_name: String,
    pub topic_name: String,
    pub partition: Option<i32>,
    pub replica_count: Option<i32>,
    pub in_sync_replica_count: Option<i32>,
    pub offline_replica_count: Option<i32>,
    pub owner: Option<String>,
    pub labels: BTreeMap<String, String>,
    pub security_protocol: String,
    pub has_isr_evidence: bool,
    pub collected_at: DateTime<Utc>,
}

pub fn evaluate_kafka_isr_health_inventory(
    items: &[KafkaIsrHealthInventoryItem],
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

pub fn isr_health_inventory_item_from_config(
    cluster: &KafkaClusterConfig,
    collected_at: DateTime<Utc>,
) -> KafkaIsrHealthInventoryItem {
    KafkaIsrHealthInventoryItem {
        isr_id: format!("{}:isr-health", cluster.name),
        cluster_name: cluster.name.clone(),
        topic_name: "<uncollected>".to_string(),
        partition: None,
        replica_count: None,
        in_sync_replica_count: None,
        offline_replica_count: None,
        owner: None,
        labels: BTreeMap::new(),
        security_protocol: cluster.security_protocol.clone(),
        has_isr_evidence: false,
        collected_at,
    }
}

fn evaluate_cost(
    item: &KafkaIsrHealthInventoryItem,
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
                "Kafka ISR health inventory for cluster {} has no owner, team, project, or cost-center metadata",
                item.cluster_name
            ),
            json!({
                "isr_id": item.isr_id,
                "cluster_name": item.cluster_name,
                "checked_keys": COST_ALLOCATION_TAG_KEYS,
            }),
        ));
    }

    if !item.has_isr_evidence {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_NO_ISR_EVIDENCE,
            Severity::High,
            format!(
                "Kafka ISR health inventory for cluster {} has no ISR evidence",
                item.cluster_name
            ),
            json!({
                "isr_id": item.isr_id,
                "cluster_name": item.cluster_name,
                "recommendation": "Collect ISR and replica health before estimating broker capacity or remediation spend",
            }),
        ));
    }
}

fn evaluate_resilience(
    item: &KafkaIsrHealthInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if !item.has_isr_evidence {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_NO_ISR_EVIDENCE,
            Severity::High,
            format!(
                "Kafka ISR health inventory for cluster {} has no resilience evidence",
                item.cluster_name
            ),
            json!({
                "isr_id": item.isr_id,
                "cluster_name": item.cluster_name,
                "recommendation": "Collect replica count, ISR count, and offline replica evidence before evaluating partition resilience",
            }),
        ));
    }

    if item
        .replica_count
        .zip(item.in_sync_replica_count)
        .is_some_and(|(replicas, isr)| isr < replicas)
    {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_ISR_BELOW_REPLICA_COUNT,
            Severity::High,
            format!("Kafka ISR health {} is below replica count", item.isr_id),
            json!({
                "isr_id": item.isr_id,
                "replica_count": item.replica_count,
                "in_sync_replica_count": item.in_sync_replica_count,
                "recommendation": "Investigate under-replicated partitions before accepting the cluster as resilient",
            }),
        ));
    }

    if item
        .offline_replica_count
        .is_some_and(|offline| offline > 0)
    {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_OFFLINE_PARTITION,
            Severity::High,
            format!(
                "Kafka ISR health {} has {} offline replica(s)",
                item.isr_id,
                item.offline_replica_count.unwrap_or_default()
            ),
            json!({
                "isr_id": item.isr_id,
                "offline_replica_count": item.offline_replica_count,
                "recommendation": "Restore offline replicas and verify ISR recovery before closing the resilience finding",
            }),
        ));
    }
}

fn evaluate_security(
    item: &KafkaIsrHealthInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if !item.has_isr_evidence {
        findings.push(finding(
            item,
            pillar,
            REASON_SEC_NO_ISR_EVIDENCE,
            Severity::Medium,
            format!(
                "Kafka ISR health inventory for cluster {} has no ISR evidence for security review",
                item.cluster_name
            ),
            json!({
                "isr_id": item.isr_id,
                "cluster_name": item.cluster_name,
                "recommendation": "Collect ISR health and topic classification before assessing sensitive data replica exposure",
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
                "Kafka ISR health inventory for cluster {} is associated with PLAINTEXT client protocol",
                item.cluster_name
            ),
            json!({
                "isr_id": item.isr_id,
                "security_protocol": item.security_protocol,
                "recommendation": "Use SSL or SASL_SSL before treating ISR evidence as secure",
            }),
        ));
    }
}

fn stale_finding(
    item: &KafkaIsrHealthInventoryItem,
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
            "Kafka ISR health inventory for {} is {} hours old (threshold {} hours)",
            item.isr_id, age_hours, DEFAULT_STALE_AFTER_HOURS
        ),
        json!({
            "isr_id": item.isr_id,
            "collected_at": item.collected_at,
            "age_hours": age_hours,
            "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
        }),
    ))
}

fn has_owner_metadata(item: &KafkaIsrHealthInventoryItem) -> bool {
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
    item: &KafkaIsrHealthInventoryItem,
    pillar: Pillar,
    reason_code: &str,
    severity: Severity,
    message: String,
    evidence: Value,
) -> InventoryFinding {
    InventoryFinding {
        resource_id: item.isr_id.clone(),
        arn: format!("kafka:isr-health/{}", item.isr_id),
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

    fn item() -> KafkaIsrHealthInventoryItem {
        KafkaIsrHealthInventoryItem {
            isr_id: "orders:events:0:isr".to_string(),
            cluster_name: "orders".to_string(),
            topic_name: "events".to_string(),
            partition: Some(0),
            replica_count: Some(3),
            in_sync_replica_count: Some(3),
            offline_replica_count: Some(0),
            owner: None,
            labels: BTreeMap::new(),
            security_protocol: "PLAINTEXT".to_string(),
            has_isr_evidence: true,
            collected_at: now(),
        }
    }

    #[test]
    fn cost_flags_missing_owner_and_missing_evidence() {
        let mut item = item();
        item.has_isr_evidence = false;

        let report = evaluate_kafka_isr_health_inventory(&[item], Pillar::Cost, now());

        assert!(report
            .findings
            .iter()
            .any(|f| f.reason_code == REASON_COST_OWNER_NOT_RECORDED));
        assert!(report
            .findings
            .iter()
            .any(|f| f.reason_code == REASON_COST_NO_ISR_EVIDENCE));
    }

    #[test]
    fn resilience_flags_missing_evidence_low_isr_and_offline_replicas() {
        let mut missing = item();
        missing.has_isr_evidence = false;
        let mut unhealthy = item();
        unhealthy.in_sync_replica_count = Some(1);
        unhealthy.offline_replica_count = Some(1);

        let report =
            evaluate_kafka_isr_health_inventory(&[missing, unhealthy], Pillar::Resilience, now());

        assert!(report
            .findings
            .iter()
            .any(|f| f.reason_code == REASON_RES_NO_ISR_EVIDENCE));
        assert!(report
            .findings
            .iter()
            .any(|f| f.reason_code == REASON_RES_ISR_BELOW_REPLICA_COUNT));
        assert!(report
            .findings
            .iter()
            .any(|f| f.reason_code == REASON_RES_OFFLINE_PARTITION));
    }

    #[test]
    fn security_flags_missing_evidence_and_plaintext_protocol() {
        let mut item = item();
        item.has_isr_evidence = false;

        let report = evaluate_kafka_isr_health_inventory(&[item], Pillar::Security, now());

        assert!(report
            .findings
            .iter()
            .any(|f| f.reason_code == REASON_SEC_NO_ISR_EVIDENCE));
        assert!(report
            .findings
            .iter()
            .any(|f| f.reason_code == REASON_SEC_PLAINTEXT));
    }

    #[test]
    fn stale_isr_health_inventory_is_counted_for_any_pillar() {
        let mut item = item();
        item.owner = Some("platform".to_string());
        item.security_protocol = "SSL".to_string();
        item.collected_at = now() - Duration::hours(DEFAULT_STALE_AFTER_HOURS + 2);

        let report = evaluate_kafka_isr_health_inventory(&[item], Pillar::Cost, now());

        assert_eq!(report.stale_resources, 1);
        assert!(report
            .findings
            .iter()
            .any(|f| f.reason_code == REASON_INV_STALE_DATA));
    }

    #[test]
    fn healthy_isr_health_passes_claimed_pillars() {
        let mut item = item();
        item.owner = Some("platform".to_string());
        item.security_protocol = "SSL".to_string();

        for pillar in [Pillar::Cost, Pillar::Resilience, Pillar::Security] {
            let report = evaluate_kafka_isr_health_inventory(&[item.clone()], pillar, now());
            assert_eq!(report.score, 100);
            assert!(report.findings.is_empty());
        }
    }
}

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

// Deterministic broker disk inventory evaluator for roadmap rows
// 04-KAFKA-DASHBOARD-MANAGEMENT-01618/01625/01646.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::config::KafkaClusterConfig;
use crate::services::aws::inventory::types::{
    score_pillar, InventoryFinding, Pillar, PillarReport, Severity, COST_ALLOCATION_TAG_KEYS,
    DEFAULT_STALE_AFTER_HOURS,
};

pub const RESOURCE_TYPE: &str = "KafkaBrokerDisk";
pub const REASON_COST_OWNER_NOT_RECORDED: &str = "KAFKA_BROKER_DISK_COST_OWNER_NOT_RECORDED";
pub const REASON_COST_NO_EVIDENCE: &str = "KAFKA_BROKER_DISK_COST_NO_EVIDENCE";
pub const REASON_COST_HIGH_UTILIZATION: &str = "KAFKA_BROKER_DISK_COST_HIGH_UTILIZATION";
pub const REASON_RES_NO_EVIDENCE: &str = "KAFKA_BROKER_DISK_RES_NO_EVIDENCE";
pub const REASON_RES_NO_ALERT_EVIDENCE: &str = "KAFKA_BROKER_DISK_RES_NO_ALERT_EVIDENCE";
pub const REASON_RES_NO_REBALANCE_EVIDENCE: &str = "KAFKA_BROKER_DISK_RES_NO_REBALANCE_EVIDENCE";
pub const REASON_RES_NO_RETENTION_EVIDENCE: &str = "KAFKA_BROKER_DISK_RES_NO_RETENTION_EVIDENCE";
pub const REASON_SEC_NO_EVIDENCE: &str = "KAFKA_BROKER_DISK_SEC_NO_EVIDENCE";
pub const REASON_SEC_NO_ENCRYPTION_EVIDENCE: &str = "KAFKA_BROKER_DISK_SEC_NO_ENCRYPTION_EVIDENCE";
pub const REASON_SEC_PLAINTEXT_CONNECTION: &str = "KAFKA_BROKER_DISK_SEC_PLAINTEXT_CONNECTION";
pub const REASON_INV_STALE_DATA: &str = "KAFKA_BROKER_DISK_INV_STALE_DATA";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrokerDiskInventoryItem {
    pub disk_id: String,
    pub cluster_name: String,
    pub broker_id: Option<i32>,
    pub log_dir: Option<String>,
    pub disk_used_bytes: Option<u64>,
    pub disk_capacity_bytes: Option<u64>,
    pub disk_usage_percent: Option<f64>,
    pub owner: Option<String>,
    pub labels: BTreeMap<String, String>,
    pub alert_evidence: bool,
    pub rebalance_evidence: bool,
    pub retention_evidence: bool,
    pub encryption_evidence: bool,
    pub plaintext_connection: bool,
    pub has_disk_evidence: bool,
    pub collected_at: DateTime<Utc>,
}

pub fn evaluate_broker_disk_inventory(
    items: &[BrokerDiskInventoryItem],
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

pub fn broker_disk_inventory_item_from_config(
    cluster: &KafkaClusterConfig,
    collected_at: DateTime<Utc>,
) -> BrokerDiskInventoryItem {
    BrokerDiskInventoryItem {
        disk_id: format!("{}:broker-disk", cluster.name),
        cluster_name: cluster.name.clone(),
        broker_id: None,
        log_dir: None,
        disk_used_bytes: None,
        disk_capacity_bytes: None,
        disk_usage_percent: None,
        owner: None,
        labels: BTreeMap::new(),
        alert_evidence: false,
        rebalance_evidence: false,
        retention_evidence: false,
        encryption_evidence: false,
        plaintext_connection: cluster.security_protocol.eq_ignore_ascii_case("PLAINTEXT"),
        has_disk_evidence: false,
        collected_at,
    }
}

fn evaluate_cost(
    item: &BrokerDiskInventoryItem,
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
                "Kafka broker disk inventory for cluster {} has no owner, team, project, or cost-center metadata",
                item.cluster_name
            ),
            json!({
                "disk_id": item.disk_id,
                "cluster_name": item.cluster_name,
                "checked_keys": COST_ALLOCATION_TAG_KEYS,
            }),
        ));
    }

    if !item.has_disk_evidence {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_NO_EVIDENCE,
            Severity::High,
            format!(
                "Kafka broker disk inventory for cluster {} has no disk evidence",
                item.cluster_name
            ),
            json!({
                "disk_id": item.disk_id,
                "cluster_name": item.cluster_name,
                "recommendation": "Collect broker disk usage, capacity, owner, retention, alert, rebalance, and encryption evidence before estimating broker disk cost posture",
            }),
        ));
    }

    if item
        .disk_usage_percent
        .is_some_and(|usage_percent| usage_percent >= 85.0)
    {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_HIGH_UTILIZATION,
            Severity::Medium,
            format!(
                "Kafka broker disk {} has high disk utilization",
                item.disk_id
            ),
            json!({
                "disk_id": item.disk_id,
                "disk_usage_percent": item.disk_usage_percent,
                "disk_used_bytes": item.disk_used_bytes,
                "disk_capacity_bytes": item.disk_capacity_bytes,
                "recommendation": "Review retention, compaction, partition placement, and storage expansion before accepting disk cost posture",
            }),
        ));
    }
}

fn evaluate_resilience(
    item: &BrokerDiskInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if !item.has_disk_evidence {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_NO_EVIDENCE,
            Severity::High,
            format!(
                "Kafka broker disk inventory for cluster {} has no resilience evidence",
                item.cluster_name
            ),
            json!({
                "disk_id": item.disk_id,
                "cluster_name": item.cluster_name,
                "recommendation": "Collect broker disk usage, alerting, retention, replica placement, and rebalance evidence before accepting disk resilience posture",
            }),
        ));
    }

    if item.has_disk_evidence && !item.alert_evidence {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_NO_ALERT_EVIDENCE,
            Severity::High,
            format!("Kafka broker disk {} has no alert evidence", item.disk_id),
            json!({
                "disk_id": item.disk_id,
                "recommendation": "Record alert evidence for disk usage, log-dir failures, and growth rate before accepting resilience posture",
            }),
        ));
    }

    if item.has_disk_evidence && !item.rebalance_evidence {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_NO_REBALANCE_EVIDENCE,
            Severity::High,
            format!("Kafka broker disk {} has no rebalance evidence", item.disk_id),
            json!({
                "disk_id": item.disk_id,
                "broker_id": item.broker_id,
                "recommendation": "Record partition movement or broker expansion evidence for disk pressure recovery",
            }),
        ));
    }

    if item.has_disk_evidence && !item.retention_evidence {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_NO_RETENTION_EVIDENCE,
            Severity::Medium,
            format!("Kafka broker disk {} has no retention evidence", item.disk_id),
            json!({
                "disk_id": item.disk_id,
                "recommendation": "Record topic retention and compaction policy evidence before accepting disk growth posture",
            }),
        ));
    }
}

fn evaluate_security(
    item: &BrokerDiskInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if !item.has_disk_evidence {
        findings.push(finding(
            item,
            pillar,
            REASON_SEC_NO_EVIDENCE,
            Severity::High,
            format!(
                "Kafka broker disk inventory for cluster {} has no security evidence",
                item.cluster_name
            ),
            json!({
                "disk_id": item.disk_id,
                "cluster_name": item.cluster_name,
                "recommendation": "Collect disk encryption, host access, topic ACL, audit, and transport evidence before accepting broker disk security posture",
            }),
        ));
    }

    if item.has_disk_evidence && !item.encryption_evidence {
        findings.push(finding(
            item,
            pillar,
            REASON_SEC_NO_ENCRYPTION_EVIDENCE,
            Severity::High,
            format!(
                "Kafka broker disk {} has no encryption-at-rest evidence",
                item.disk_id
            ),
            json!({
                "disk_id": item.disk_id,
                "log_dir": item.log_dir,
                "recommendation": "Record volume, filesystem, or managed-service encryption evidence for Kafka broker disks",
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
                "Kafka broker disk {} belongs to a PLAINTEXT or unverified Kafka transport cluster",
                item.disk_id
            ),
            json!({
                "disk_id": item.disk_id,
                "plaintext_connection": item.plaintext_connection,
                "recommendation": "Use encrypted Kafka transport before accepting broker disk security posture",
            }),
        ));
    }
}

fn stale_finding(
    item: &BrokerDiskInventoryItem,
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
            "Kafka broker disk inventory for cluster {} is {} hour(s) old",
            item.cluster_name, age_hours
        ),
        json!({
            "disk_id": item.disk_id,
            "cluster_name": item.cluster_name,
            "age_hours": age_hours,
            "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
            "recommendation": "Refresh Kafka broker disk inventory before acting on this posture report",
        }),
    ))
}

fn has_owner_metadata(item: &BrokerDiskInventoryItem) -> bool {
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
    item: &BrokerDiskInventoryItem,
    pillar: Pillar,
    reason_code: &'static str,
    severity: Severity,
    message: String,
    evidence: Value,
) -> InventoryFinding {
    InventoryFinding {
        resource_id: item.disk_id.clone(),
        arn: format!("kafka:broker-disk/{}", item.disk_id),
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

    fn disk(now: DateTime<Utc>) -> BrokerDiskInventoryItem {
        BrokerDiskInventoryItem {
            disk_id: "prod:broker-disk:1".to_string(),
            cluster_name: "prod".to_string(),
            broker_id: Some(1),
            log_dir: Some("/var/lib/kafka/data".to_string()),
            disk_used_bytes: Some(500),
            disk_capacity_bytes: Some(1000),
            disk_usage_percent: Some(50.0),
            owner: Some("platform".to_string()),
            labels: BTreeMap::new(),
            alert_evidence: true,
            rebalance_evidence: true,
            retention_evidence: true,
            encryption_evidence: true,
            plaintext_connection: false,
            has_disk_evidence: true,
            collected_at: now,
        }
    }

    #[test]
    fn healthy_broker_disk_passes_claimed_pillars() {
        let now = Utc::now();
        let item = disk(now);

        for pillar in [Pillar::Cost, Pillar::Resilience, Pillar::Security] {
            let report = evaluate_broker_disk_inventory(std::slice::from_ref(&item), pillar, now);
            assert_eq!(report.resources_evaluated, 1);
            assert!(report.findings.is_empty());
            assert_eq!(report.stale_resources, 0);
            assert_eq!(report.score, 100);
        }
    }

    #[test]
    fn cost_flags_missing_owner_missing_evidence_and_high_utilization() {
        let now = Utc::now();
        let mut missing_evidence = disk(now);
        missing_evidence.owner = None;
        missing_evidence.has_disk_evidence = false;
        let mut high_usage = disk(now);
        high_usage.disk_usage_percent = Some(85.0);

        let report =
            evaluate_broker_disk_inventory(&[missing_evidence, high_usage], Pillar::Cost, now);

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
            .any(|finding| finding.reason_code == REASON_COST_HIGH_UTILIZATION));
    }

    #[test]
    fn resilience_flags_missing_evidence_alert_rebalance_and_retention() {
        let now = Utc::now();
        let mut missing_evidence = disk(now);
        missing_evidence.has_disk_evidence = false;
        let mut risky = disk(now);
        risky.alert_evidence = false;
        risky.rebalance_evidence = false;
        risky.retention_evidence = false;

        let report =
            evaluate_broker_disk_inventory(&[missing_evidence, risky], Pillar::Resilience, now);

        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_RES_NO_EVIDENCE));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_RES_NO_ALERT_EVIDENCE));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_RES_NO_REBALANCE_EVIDENCE));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_RES_NO_RETENTION_EVIDENCE));
    }

    #[test]
    fn security_flags_missing_evidence_encryption_and_plaintext() {
        let now = Utc::now();
        let mut missing_evidence = disk(now);
        missing_evidence.has_disk_evidence = false;
        let mut insecure = disk(now);
        insecure.encryption_evidence = false;
        insecure.plaintext_connection = true;

        let report =
            evaluate_broker_disk_inventory(&[missing_evidence, insecure], Pillar::Security, now);

        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_SEC_NO_EVIDENCE));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_SEC_NO_ENCRYPTION_EVIDENCE));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_SEC_PLAINTEXT_CONNECTION));
    }

    #[test]
    fn stale_broker_disk_inventory_is_counted_for_any_pillar() {
        let now = Utc::now();
        let mut item = disk(now);
        item.collected_at = now - Duration::hours(DEFAULT_STALE_AFTER_HOURS + 2);

        let report = evaluate_broker_disk_inventory(&[item], Pillar::Security, now);

        assert_eq!(report.stale_resources, 1);
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_INV_STALE_DATA));
    }
}

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

// Deterministic network throughput inventory evaluator for roadmap rows
// 04-KAFKA-DASHBOARD-MANAGEMENT-01667/01674/01695.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::config::KafkaClusterConfig;
use crate::services::aws::inventory::types::{
    score_pillar, InventoryFinding, Pillar, PillarReport, Severity, COST_ALLOCATION_TAG_KEYS,
    DEFAULT_STALE_AFTER_HOURS,
};

pub const RESOURCE_TYPE: &str = "KafkaNetworkThroughput";
pub const REASON_COST_OWNER_NOT_RECORDED: &str = "KAFKA_NETWORK_THROUGHPUT_COST_OWNER_NOT_RECORDED";
pub const REASON_COST_NO_EVIDENCE: &str = "KAFKA_NETWORK_THROUGHPUT_COST_NO_EVIDENCE";
pub const REASON_COST_HIGH_EGRESS: &str = "KAFKA_NETWORK_THROUGHPUT_COST_HIGH_EGRESS";
pub const REASON_RES_NO_EVIDENCE: &str = "KAFKA_NETWORK_THROUGHPUT_RES_NO_EVIDENCE";
pub const REASON_RES_NO_ALERT_EVIDENCE: &str = "KAFKA_NETWORK_THROUGHPUT_RES_NO_ALERT_EVIDENCE";
pub const REASON_RES_NO_CAPACITY_EVIDENCE: &str =
    "KAFKA_NETWORK_THROUGHPUT_RES_NO_CAPACITY_EVIDENCE";
pub const REASON_RES_HIGH_UTILIZATION: &str = "KAFKA_NETWORK_THROUGHPUT_RES_HIGH_UTILIZATION";
pub const REASON_SEC_NO_EVIDENCE: &str = "KAFKA_NETWORK_THROUGHPUT_SEC_NO_EVIDENCE";
pub const REASON_SEC_NO_ENCRYPTION_EVIDENCE: &str =
    "KAFKA_NETWORK_THROUGHPUT_SEC_NO_ENCRYPTION_EVIDENCE";
pub const REASON_SEC_PLAINTEXT_CONNECTION: &str =
    "KAFKA_NETWORK_THROUGHPUT_SEC_PLAINTEXT_CONNECTION";
pub const REASON_INV_STALE_DATA: &str = "KAFKA_NETWORK_THROUGHPUT_INV_STALE_DATA";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkThroughputInventoryItem {
    pub throughput_id: String,
    pub cluster_name: String,
    pub broker_id: Option<i32>,
    pub bytes_in_per_sec: Option<f64>,
    pub bytes_out_per_sec: Option<f64>,
    pub network_capacity_bytes_per_sec: Option<f64>,
    pub network_utilization_percent: Option<f64>,
    pub cross_zone_bytes_per_sec: Option<f64>,
    pub owner: Option<String>,
    pub labels: BTreeMap<String, String>,
    pub alert_evidence: bool,
    pub capacity_evidence: bool,
    pub encryption_evidence: bool,
    pub plaintext_connection: bool,
    pub has_throughput_evidence: bool,
    pub collected_at: DateTime<Utc>,
}

pub fn evaluate_network_throughput_inventory(
    items: &[NetworkThroughputInventoryItem],
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

pub fn network_throughput_inventory_item_from_config(
    cluster: &KafkaClusterConfig,
    collected_at: DateTime<Utc>,
) -> NetworkThroughputInventoryItem {
    NetworkThroughputInventoryItem {
        throughput_id: format!("{}:network-throughput", cluster.name),
        cluster_name: cluster.name.clone(),
        broker_id: None,
        bytes_in_per_sec: None,
        bytes_out_per_sec: None,
        network_capacity_bytes_per_sec: None,
        network_utilization_percent: None,
        cross_zone_bytes_per_sec: None,
        owner: None,
        labels: BTreeMap::new(),
        alert_evidence: false,
        capacity_evidence: false,
        encryption_evidence: false,
        plaintext_connection: cluster.security_protocol.eq_ignore_ascii_case("PLAINTEXT"),
        has_throughput_evidence: false,
        collected_at,
    }
}

fn evaluate_cost(
    item: &NetworkThroughputInventoryItem,
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
                "Kafka network throughput inventory for cluster {} has no owner, team, project, or cost-center metadata",
                item.cluster_name
            ),
            json!({
                "throughput_id": item.throughput_id,
                "cluster_name": item.cluster_name,
                "checked_keys": COST_ALLOCATION_TAG_KEYS,
            }),
        ));
    }

    if !item.has_throughput_evidence {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_NO_EVIDENCE,
            Severity::High,
            format!(
                "Kafka network throughput inventory for cluster {} has no throughput evidence",
                item.cluster_name
            ),
            json!({
                "throughput_id": item.throughput_id,
                "cluster_name": item.cluster_name,
                "recommendation": "Collect broker network bytes in/out, capacity, cross-zone egress, owner, alert, and encryption evidence before estimating network throughput cost posture",
            }),
        ));
    }

    if item
        .cross_zone_bytes_per_sec
        .is_some_and(|cross_zone_bytes_per_sec| cross_zone_bytes_per_sec > 0.0)
    {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_HIGH_EGRESS,
            Severity::Medium,
            format!(
                "Kafka network throughput {} has cross-zone or external egress evidence",
                item.throughput_id
            ),
            json!({
                "throughput_id": item.throughput_id,
                "cross_zone_bytes_per_sec": item.cross_zone_bytes_per_sec,
                "recommendation": "Review client placement, broker racks, replication traffic, and cross-zone egress charges before accepting network cost posture",
            }),
        ));
    }
}

fn evaluate_resilience(
    item: &NetworkThroughputInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if !item.has_throughput_evidence {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_NO_EVIDENCE,
            Severity::High,
            format!(
                "Kafka network throughput inventory for cluster {} has no resilience evidence",
                item.cluster_name
            ),
            json!({
                "throughput_id": item.throughput_id,
                "cluster_name": item.cluster_name,
                "recommendation": "Collect broker network throughput, saturation, alerting, capacity headroom, and client traffic evidence before accepting resilience posture",
            }),
        ));
    }

    if item.has_throughput_evidence && !item.alert_evidence {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_NO_ALERT_EVIDENCE,
            Severity::High,
            format!(
                "Kafka network throughput {} has no alert evidence",
                item.throughput_id
            ),
            json!({
                "throughput_id": item.throughput_id,
                "recommendation": "Record alerts for broker network saturation, request queueing, client throttling, and replication traffic before accepting resilience posture",
            }),
        ));
    }

    if item.has_throughput_evidence && !item.capacity_evidence {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_NO_CAPACITY_EVIDENCE,
            Severity::High,
            format!(
                "Kafka network throughput {} has no capacity evidence",
                item.throughput_id
            ),
            json!({
                "throughput_id": item.throughput_id,
                "network_capacity_bytes_per_sec": item.network_capacity_bytes_per_sec,
                "recommendation": "Record broker NIC, managed-service throughput, partition placement, and expected peak traffic evidence before accepting capacity posture",
            }),
        ));
    }

    if item
        .network_utilization_percent
        .is_some_and(|utilization_percent| utilization_percent >= 80.0)
    {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_HIGH_UTILIZATION,
            Severity::High,
            format!(
                "Kafka network throughput {} has high utilization",
                item.throughput_id
            ),
            json!({
                "throughput_id": item.throughput_id,
                "network_utilization_percent": item.network_utilization_percent,
                "bytes_in_per_sec": item.bytes_in_per_sec,
                "bytes_out_per_sec": item.bytes_out_per_sec,
                "network_capacity_bytes_per_sec": item.network_capacity_bytes_per_sec,
                "recommendation": "Add network headroom, rebalance traffic, or throttle producers before accepting resilience posture",
            }),
        ));
    }
}

fn evaluate_security(
    item: &NetworkThroughputInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if !item.has_throughput_evidence {
        findings.push(finding(
            item,
            pillar,
            REASON_SEC_NO_EVIDENCE,
            Severity::High,
            format!(
                "Kafka network throughput inventory for cluster {} has no security evidence",
                item.cluster_name
            ),
            json!({
                "throughput_id": item.throughput_id,
                "cluster_name": item.cluster_name,
                "recommendation": "Collect transport encryption, listener exposure, client identity, ACL, and throughput anomaly evidence before accepting network security posture",
            }),
        ));
    }

    if item.has_throughput_evidence && !item.encryption_evidence {
        findings.push(finding(
            item,
            pillar,
            REASON_SEC_NO_ENCRYPTION_EVIDENCE,
            Severity::High,
            format!(
                "Kafka network throughput {} has no encryption evidence",
                item.throughput_id
            ),
            json!({
                "throughput_id": item.throughput_id,
                "recommendation": "Record TLS or managed-service transport encryption evidence for Kafka broker and client network traffic",
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
                "Kafka network throughput {} belongs to a PLAINTEXT or unverified Kafka transport cluster",
                item.throughput_id
            ),
            json!({
                "throughput_id": item.throughput_id,
                "plaintext_connection": item.plaintext_connection,
                "recommendation": "Use encrypted Kafka transport before accepting network throughput security posture",
            }),
        ));
    }
}

fn stale_finding(
    item: &NetworkThroughputInventoryItem,
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
            "Kafka network throughput inventory for cluster {} is {} hour(s) old",
            item.cluster_name, age_hours
        ),
        json!({
            "throughput_id": item.throughput_id,
            "cluster_name": item.cluster_name,
            "age_hours": age_hours,
            "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
            "recommendation": "Refresh Kafka network throughput inventory before acting on this posture report",
        }),
    ))
}

fn has_owner_metadata(item: &NetworkThroughputInventoryItem) -> bool {
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
    item: &NetworkThroughputInventoryItem,
    pillar: Pillar,
    reason_code: &'static str,
    severity: Severity,
    message: String,
    evidence: Value,
) -> InventoryFinding {
    InventoryFinding {
        resource_id: item.throughput_id.clone(),
        arn: format!("kafka:network-throughput/{}", item.throughput_id),
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

    fn throughput(now: DateTime<Utc>) -> NetworkThroughputInventoryItem {
        NetworkThroughputInventoryItem {
            throughput_id: "prod:network-throughput:1".to_string(),
            cluster_name: "prod".to_string(),
            broker_id: Some(1),
            bytes_in_per_sec: Some(1000.0),
            bytes_out_per_sec: Some(1200.0),
            network_capacity_bytes_per_sec: Some(10_000.0),
            network_utilization_percent: Some(22.0),
            cross_zone_bytes_per_sec: None,
            owner: Some("platform".to_string()),
            labels: BTreeMap::new(),
            alert_evidence: true,
            capacity_evidence: true,
            encryption_evidence: true,
            plaintext_connection: false,
            has_throughput_evidence: true,
            collected_at: now,
        }
    }

    #[test]
    fn healthy_network_throughput_passes_claimed_pillars() {
        let now = Utc::now();
        let item = throughput(now);

        for pillar in [Pillar::Cost, Pillar::Resilience, Pillar::Security] {
            let report =
                evaluate_network_throughput_inventory(std::slice::from_ref(&item), pillar, now);
            assert_eq!(report.resources_evaluated, 1);
            assert!(report.findings.is_empty());
            assert_eq!(report.stale_resources, 0);
            assert_eq!(report.score, 100);
        }
    }

    #[test]
    fn cost_flags_missing_owner_missing_evidence_and_cross_zone_egress() {
        let now = Utc::now();
        let mut missing_evidence = throughput(now);
        missing_evidence.owner = None;
        missing_evidence.has_throughput_evidence = false;
        let mut high_egress = throughput(now);
        high_egress.cross_zone_bytes_per_sec = Some(1024.0);

        let report = evaluate_network_throughput_inventory(
            &[missing_evidence, high_egress],
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
            .any(|finding| finding.reason_code == REASON_COST_HIGH_EGRESS));
    }

    #[test]
    fn resilience_flags_missing_evidence_alert_capacity_and_high_utilization() {
        let now = Utc::now();
        let mut missing_evidence = throughput(now);
        missing_evidence.has_throughput_evidence = false;
        let mut risky = throughput(now);
        risky.alert_evidence = false;
        risky.capacity_evidence = false;
        risky.network_utilization_percent = Some(80.0);

        let report = evaluate_network_throughput_inventory(
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
            .any(|finding| finding.reason_code == REASON_RES_NO_ALERT_EVIDENCE));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_RES_NO_CAPACITY_EVIDENCE));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_RES_HIGH_UTILIZATION));
    }

    #[test]
    fn security_flags_missing_evidence_encryption_and_plaintext_transport() {
        let now = Utc::now();
        let mut missing_evidence = throughput(now);
        missing_evidence.has_throughput_evidence = false;
        let mut risky = throughput(now);
        risky.encryption_evidence = false;
        risky.plaintext_connection = true;

        let report = evaluate_network_throughput_inventory(
            &[missing_evidence, risky],
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
            .any(|finding| finding.reason_code == REASON_SEC_NO_ENCRYPTION_EVIDENCE));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_SEC_PLAINTEXT_CONNECTION));
    }

    #[test]
    fn config_item_produces_missing_evidence_and_plaintext_security_findings() {
        let now = Utc::now();
        let cluster = KafkaClusterConfig {
            name: "orders".to_string(),
            bootstrap_servers: vec!["broker-1:9092".to_string()],
            sasl_username: None,
            sasl_password: None,
            sasl_mechanism: None,
            security_protocol: "PLAINTEXT".to_string(),
        };

        let item = network_throughput_inventory_item_from_config(&cluster, now);
        let report = evaluate_network_throughput_inventory(&[item], Pillar::Security, now);

        assert_eq!(report.resources_evaluated, 1);
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_SEC_NO_EVIDENCE));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_SEC_PLAINTEXT_CONNECTION));
    }

    #[test]
    fn stale_inventory_is_reported_per_pillar() {
        let now = Utc::now();
        let mut item = throughput(now);
        item.collected_at = now - Duration::hours(DEFAULT_STALE_AFTER_HOURS + 1);

        let report = evaluate_network_throughput_inventory(&[item], Pillar::Cost, now);

        assert_eq!(report.stale_resources, 1);
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_INV_STALE_DATA));
    }
}

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

// Deterministic Kafka controller quorum inventory evaluator for roadmap rows
// 04-KAFKA-DASHBOARD-MANAGEMENT-00099/00106/00127.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::config::KafkaClusterConfig;
use crate::services::aws::inventory::types::{
    score_pillar, InventoryFinding, Pillar, PillarReport, Severity, COST_ALLOCATION_TAG_KEYS,
    DEFAULT_STALE_AFTER_HOURS,
};

pub const RESOURCE_TYPE: &str = "KafkaControllerQuorum";
pub const REASON_COST_OWNER_NOT_RECORDED: &str = "KAFKA_CONTROLLER_QUORUM_COST_OWNER_NOT_RECORDED";
pub const REASON_COST_NO_QUORUM_EVIDENCE: &str = "KAFKA_CONTROLLER_QUORUM_COST_NO_EVIDENCE";
pub const REASON_RES_NO_QUORUM_EVIDENCE: &str = "KAFKA_CONTROLLER_QUORUM_RES_NO_EVIDENCE";
pub const REASON_RES_UNDERSIZED_QUORUM: &str = "KAFKA_CONTROLLER_QUORUM_RES_UNDERSIZED";
pub const REASON_SEC_NO_QUORUM_EVIDENCE: &str = "KAFKA_CONTROLLER_QUORUM_SEC_NO_EVIDENCE";
pub const REASON_SEC_PLAINTEXT: &str = "KAFKA_CONTROLLER_QUORUM_SEC_PLAINTEXT_PROTOCOL";
pub const REASON_INV_STALE_DATA: &str = "KAFKA_CONTROLLER_QUORUM_INV_STALE_DATA";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KafkaControllerQuorumInventoryItem {
    pub quorum_id: String,
    pub cluster_name: String,
    pub owner: Option<String>,
    pub labels: BTreeMap<String, String>,
    pub voter_count: usize,
    pub voter_endpoints: Vec<String>,
    pub security_protocol: String,
    pub collected_at: DateTime<Utc>,
}

pub fn evaluate_kafka_controller_quorum_inventory(
    items: &[KafkaControllerQuorumInventoryItem],
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

pub fn controller_quorum_item_from_config(
    cluster: &KafkaClusterConfig,
    collected_at: DateTime<Utc>,
) -> KafkaControllerQuorumInventoryItem {
    KafkaControllerQuorumInventoryItem {
        quorum_id: format!("{}:controller-quorum", cluster.name),
        cluster_name: cluster.name.clone(),
        owner: None,
        labels: BTreeMap::new(),
        voter_count: cluster.bootstrap_servers.len(),
        voter_endpoints: cluster.bootstrap_servers.clone(),
        security_protocol: cluster.security_protocol.clone(),
        collected_at,
    }
}

fn evaluate_cost(
    item: &KafkaControllerQuorumInventoryItem,
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
                "Kafka controller quorum for cluster {} has no owner, team, project, or cost-center metadata",
                item.cluster_name
            ),
            json!({
                "quorum_id": item.quorum_id,
                "cluster_name": item.cluster_name,
                "checked_keys": COST_ALLOCATION_TAG_KEYS,
            }),
        ));
    }

    if item.voter_count == 0 {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_NO_QUORUM_EVIDENCE,
            Severity::High,
            format!(
                "Kafka controller quorum for cluster {} has no voter endpoint evidence",
                item.cluster_name
            ),
            json!({
                "quorum_id": item.quorum_id,
                "voter_count": item.voter_count,
                "recommendation": "Collect controller quorum or broker endpoint evidence before attributing controller overhead or dependency edges",
            }),
        ));
    }
}

fn evaluate_resilience(
    item: &KafkaControllerQuorumInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if item.voter_count == 0 {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_NO_QUORUM_EVIDENCE,
            Severity::High,
            format!(
                "Kafka controller quorum for cluster {} has no voter evidence",
                item.cluster_name
            ),
            json!({
                "quorum_id": item.quorum_id,
                "voter_count": item.voter_count,
                "recommendation": "Collect controller quorum state before evaluating leader election and failover readiness",
            }),
        ));
    } else if item.voter_count < 3 {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_UNDERSIZED_QUORUM,
            Severity::High,
            format!(
                "Kafka controller quorum for cluster {} has only {} voter endpoint(s)",
                item.cluster_name, item.voter_count
            ),
            json!({
                "quorum_id": item.quorum_id,
                "voter_count": item.voter_count,
                "recommendation": "Use at least three controller voters or broker endpoints across failure domains for quorum resilience",
            }),
        ));
    }
}

fn evaluate_security(
    item: &KafkaControllerQuorumInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if item.voter_count == 0 {
        findings.push(finding(
            item,
            pillar,
            REASON_SEC_NO_QUORUM_EVIDENCE,
            Severity::Medium,
            format!(
                "Kafka controller quorum for cluster {} has no endpoint evidence for security review",
                item.cluster_name
            ),
            json!({
                "quorum_id": item.quorum_id,
                "voter_count": item.voter_count,
                "recommendation": "Collect controller quorum or broker endpoint evidence before assessing listener exposure",
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
                "Kafka controller quorum for cluster {} is represented by PLAINTEXT client protocol evidence",
                item.cluster_name
            ),
            json!({
                "quorum_id": item.quorum_id,
                "security_protocol": item.security_protocol,
                "recommendation": "Use SSL or SASL_SSL for Kafka listeners and verify controller quorum traffic is isolated and encrypted where supported",
            }),
        ));
    }
}

fn stale_finding(
    item: &KafkaControllerQuorumInventoryItem,
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
            "Kafka controller quorum inventory for {} is {} hours old (threshold {} hours)",
            item.cluster_name, age_hours, DEFAULT_STALE_AFTER_HOURS
        ),
        json!({
            "quorum_id": item.quorum_id,
            "collected_at": item.collected_at,
            "age_hours": age_hours,
            "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
        }),
    ))
}

fn has_owner_metadata(item: &KafkaControllerQuorumInventoryItem) -> bool {
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
    item: &KafkaControllerQuorumInventoryItem,
    pillar: Pillar,
    reason_code: &str,
    severity: Severity,
    message: String,
    evidence: Value,
) -> InventoryFinding {
    InventoryFinding {
        resource_id: item.quorum_id.clone(),
        arn: format!("kafka:controller-quorum/{}", item.quorum_id),
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

    fn item() -> KafkaControllerQuorumInventoryItem {
        KafkaControllerQuorumInventoryItem {
            quorum_id: "orders:controller-quorum".to_string(),
            cluster_name: "orders".to_string(),
            owner: None,
            labels: BTreeMap::new(),
            voter_count: 1,
            voter_endpoints: vec!["broker-1:9092".to_string()],
            security_protocol: "PLAINTEXT".to_string(),
            collected_at: now(),
        }
    }

    #[test]
    fn cost_flags_missing_owner_and_missing_quorum_evidence() {
        let mut item = item();
        item.voter_count = 0;
        item.voter_endpoints.clear();

        let report = evaluate_kafka_controller_quorum_inventory(&[item], Pillar::Cost, now());

        assert!(report
            .findings
            .iter()
            .any(|f| f.reason_code == REASON_COST_OWNER_NOT_RECORDED));
        assert!(report
            .findings
            .iter()
            .any(|f| f.reason_code == REASON_COST_NO_QUORUM_EVIDENCE));
    }

    #[test]
    fn resilience_flags_missing_and_undersized_quorum() {
        let mut missing = item();
        missing.voter_count = 0;
        let undersized = item();

        let report = evaluate_kafka_controller_quorum_inventory(
            &[missing, undersized],
            Pillar::Resilience,
            now(),
        );

        assert!(report
            .findings
            .iter()
            .any(|f| f.reason_code == REASON_RES_NO_QUORUM_EVIDENCE));
        assert!(report
            .findings
            .iter()
            .any(|f| f.reason_code == REASON_RES_UNDERSIZED_QUORUM));
    }

    #[test]
    fn security_flags_plaintext_and_missing_quorum_evidence() {
        let mut item = item();
        item.voter_count = 0;

        let report = evaluate_kafka_controller_quorum_inventory(&[item], Pillar::Security, now());

        assert!(report
            .findings
            .iter()
            .any(|f| f.reason_code == REASON_SEC_NO_QUORUM_EVIDENCE));
        assert!(report
            .findings
            .iter()
            .any(|f| f.reason_code == REASON_SEC_PLAINTEXT));
    }

    #[test]
    fn stale_controller_quorum_inventory_is_counted_for_any_pillar() {
        let mut item = item();
        item.owner = Some("platform".to_string());
        item.security_protocol = "SSL".to_string();
        item.voter_count = 3;
        item.collected_at = now() - Duration::hours(DEFAULT_STALE_AFTER_HOURS + 2);

        let report = evaluate_kafka_controller_quorum_inventory(&[item], Pillar::Cost, now());

        assert_eq!(report.stale_resources, 1);
        assert!(report
            .findings
            .iter()
            .any(|f| f.reason_code == REASON_INV_STALE_DATA));
    }

    #[test]
    fn healthy_controller_quorum_passes_claimed_pillars() {
        let mut item = item();
        item.owner = Some("platform".to_string());
        item.security_protocol = "SSL".to_string();
        item.voter_count = 3;
        item.voter_endpoints = vec![
            "broker-1:9092".to_string(),
            "broker-2:9092".to_string(),
            "broker-3:9092".to_string(),
        ];

        for pillar in [Pillar::Cost, Pillar::Resilience, Pillar::Security] {
            let report = evaluate_kafka_controller_quorum_inventory(&[item.clone()], pillar, now());
            assert_eq!(report.score, 100);
            assert!(report.findings.is_empty());
        }
    }
}

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

// Deterministic Kafka cluster inventory evaluator for roadmap rows
// 04-KAFKA-DASHBOARD-MANAGEMENT-00001/00008/00029.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::config::KafkaClusterConfig;
use crate::services::aws::inventory::types::{
    score_pillar, InventoryFinding, Pillar, PillarReport, Severity, COST_ALLOCATION_TAG_KEYS,
    DEFAULT_STALE_AFTER_HOURS,
};

pub const RESOURCE_TYPE: &str = "KafkaCluster";
pub const REASON_COST_OWNER_NOT_RECORDED: &str = "KAFKA_CLUSTER_COST_OWNER_NOT_RECORDED";
pub const REASON_COST_BOOTSTRAP_MISSING: &str = "KAFKA_CLUSTER_COST_BOOTSTRAP_MISSING";
pub const REASON_RES_BOOTSTRAP_MISSING: &str = "KAFKA_CLUSTER_RES_BOOTSTRAP_MISSING";
pub const REASON_RES_STATUS_UNKNOWN: &str = "KAFKA_CLUSTER_RES_STATUS_UNKNOWN";
pub const REASON_RES_STATUS_UNHEALTHY: &str = "KAFKA_CLUSTER_RES_STATUS_UNHEALTHY";
pub const REASON_SEC_BOOTSTRAP_MISSING: &str = "KAFKA_CLUSTER_SEC_BOOTSTRAP_MISSING";
pub const REASON_SEC_PLAINTEXT: &str = "KAFKA_CLUSTER_SEC_PLAINTEXT_PROTOCOL";
pub const REASON_SEC_SASL_MISSING_MECHANISM: &str = "KAFKA_CLUSTER_SEC_SASL_MISSING_MECHANISM";
pub const REASON_INV_STALE_DATA: &str = "KAFKA_CLUSTER_INV_STALE_DATA";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KafkaClusterInventoryItem {
    pub cluster_id: String,
    pub name: String,
    pub owner: Option<String>,
    pub labels: BTreeMap<String, String>,
    pub bootstrap_servers: Vec<String>,
    pub security_protocol: String,
    pub sasl_mechanism: Option<String>,
    pub status: Option<String>,
    pub collected_at: DateTime<Utc>,
}

pub fn evaluate_kafka_cluster_inventory(
    items: &[KafkaClusterInventoryItem],
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

pub fn cluster_inventory_item_from_config(
    cluster: &KafkaClusterConfig,
    collected_at: DateTime<Utc>,
) -> KafkaClusterInventoryItem {
    KafkaClusterInventoryItem {
        cluster_id: cluster.name.clone(),
        name: cluster.name.clone(),
        owner: None,
        labels: BTreeMap::new(),
        bootstrap_servers: cluster.bootstrap_servers.clone(),
        security_protocol: cluster.security_protocol.clone(),
        sasl_mechanism: cluster.sasl_mechanism.clone(),
        status: None,
        collected_at,
    }
}

fn evaluate_cost(
    item: &KafkaClusterInventoryItem,
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
                "Kafka cluster {} has no owner, team, project, or cost-center metadata",
                item.name
            ),
            json!({
                "cluster_id": item.cluster_id,
                "name": item.name,
                "owner": item.owner,
                "checked_keys": COST_ALLOCATION_TAG_KEYS,
                "checked_locations": ["owner", "labels"],
            }),
        ));
    }

    if item.bootstrap_servers.is_empty() {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_BOOTSTRAP_MISSING,
            Severity::High,
            format!(
                "Kafka cluster {} has no bootstrap server inventory, so cost posture cannot be tied to a runtime cluster",
                item.name
            ),
            json!({
                "cluster_id": item.cluster_id,
                "bootstrap_server_count": item.bootstrap_servers.len(),
                "recommendation": "Collect bootstrap server inventory before attributing spend, ownership, or dependency edges for this Kafka cluster",
            }),
        ));
    }
}

fn evaluate_resilience(
    item: &KafkaClusterInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if item.bootstrap_servers.is_empty() {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_BOOTSTRAP_MISSING,
            Severity::High,
            format!(
                "Kafka cluster {} has no bootstrap server inventory for resilience checks",
                item.name
            ),
            json!({
                "cluster_id": item.cluster_id,
                "bootstrap_server_count": item.bootstrap_servers.len(),
                "recommendation": "Collect bootstrap servers and broker metadata before evaluating cluster reachability and dependency health",
            }),
        ));
    }

    match normalized_status(item.status.as_deref()) {
        None => findings.push(finding(
            item,
            pillar,
            REASON_RES_STATUS_UNKNOWN,
            Severity::Medium,
            format!("Kafka cluster {} has no last known health status", item.name),
            json!({
                "cluster_id": item.cluster_id,
                "status": item.status,
                "recommendation": "Run a Kafka health check and persist freshness so operators can distinguish unknown state from healthy state",
            }),
        )),
        Some(status) if !is_healthy_status(status) => findings.push(finding(
            item,
            pillar,
            REASON_RES_STATUS_UNHEALTHY,
            Severity::High,
            format!(
                "Kafka cluster {} last status is '{}' rather than healthy",
                item.name, status
            ),
            json!({
                "cluster_id": item.cluster_id,
                "status": status,
                "recommendation": "Investigate broker reachability, controller health, quorum state, and client connectivity before using this cluster for production traffic",
            }),
        )),
        _ => {}
    }
}

fn evaluate_security(
    item: &KafkaClusterInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if item.bootstrap_servers.is_empty() {
        findings.push(finding(
            item,
            pillar,
            REASON_SEC_BOOTSTRAP_MISSING,
            Severity::Medium,
            format!(
                "Kafka cluster {} has no bootstrap server inventory for security review",
                item.name
            ),
            json!({
                "cluster_id": item.cluster_id,
                "bootstrap_server_count": item.bootstrap_servers.len(),
                "recommendation": "Collect bootstrap server inventory before assessing listener exposure and authentication posture",
            }),
        ));
    }

    let protocol = item.security_protocol.to_ascii_uppercase();
    if protocol == "PLAINTEXT" {
        findings.push(finding(
            item,
            pillar,
            REASON_SEC_PLAINTEXT,
            Severity::High,
            format!(
                "Kafka cluster {} uses PLAINTEXT client security protocol",
                item.name
            ),
            json!({
                "cluster_id": item.cluster_id,
                "security_protocol": item.security_protocol,
                "recommendation": "Use SSL or SASL_SSL for client connections so credentials and payloads are not sent in clear text",
            }),
        ));
    }

    if protocol.starts_with("SASL") && item.sasl_mechanism.as_deref().unwrap_or("").is_empty() {
        findings.push(finding(
            item,
            pillar,
            REASON_SEC_SASL_MISSING_MECHANISM,
            Severity::Medium,
            format!(
                "Kafka cluster {} uses SASL but has no mechanism recorded",
                item.name
            ),
            json!({
                "cluster_id": item.cluster_id,
                "security_protocol": item.security_protocol,
                "sasl_mechanism": item.sasl_mechanism,
                "recommendation": "Record the SASL mechanism so authentication posture can be audited and drift can be detected",
            }),
        ));
    }
}

fn stale_finding(
    item: &KafkaClusterInventoryItem,
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
            "Kafka cluster inventory for {} is {} hours old (threshold {} hours)",
            item.name, age_hours, DEFAULT_STALE_AFTER_HOURS
        ),
        json!({
            "cluster_id": item.cluster_id,
            "collected_at": item.collected_at,
            "age_hours": age_hours,
            "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
        }),
    ))
}

fn has_owner_metadata(item: &KafkaClusterInventoryItem) -> bool {
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

fn normalized_status(status: Option<&str>) -> Option<&str> {
    status.map(str::trim).filter(|status| !status.is_empty())
}

fn is_healthy_status(status: &str) -> bool {
    matches!(
        status.to_ascii_lowercase().as_str(),
        "healthy" | "active" | "running" | "available" | "connected"
    )
}

fn finding(
    item: &KafkaClusterInventoryItem,
    pillar: Pillar,
    reason_code: &str,
    severity: Severity,
    message: String,
    evidence: Value,
) -> InventoryFinding {
    InventoryFinding {
        resource_id: item.cluster_id.clone(),
        arn: format!("kafka:cluster/{}", item.cluster_id),
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

    fn item() -> KafkaClusterInventoryItem {
        KafkaClusterInventoryItem {
            cluster_id: "orders".to_string(),
            name: "orders".to_string(),
            owner: None,
            labels: BTreeMap::new(),
            bootstrap_servers: vec!["broker-1:9092".to_string()],
            security_protocol: "PLAINTEXT".to_string(),
            sasl_mechanism: None,
            status: None,
            collected_at: now(),
        }
    }

    #[test]
    fn cost_flags_missing_owner_and_bootstrap_inventory() {
        let mut item = item();
        item.bootstrap_servers.clear();

        let report = evaluate_kafka_cluster_inventory(&[item], Pillar::Cost, now());

        assert_eq!(report.resources_evaluated, 1);
        assert_eq!(report.findings.len(), 2);
        assert!(report
            .findings
            .iter()
            .any(|f| f.reason_code == REASON_COST_OWNER_NOT_RECORDED));
        assert!(report
            .findings
            .iter()
            .any(|f| f.reason_code == REASON_COST_BOOTSTRAP_MISSING));
    }

    #[test]
    fn resilience_flags_missing_bootstrap_unknown_status_and_unhealthy_status() {
        let mut missing = item();
        missing.bootstrap_servers.clear();

        let mut unhealthy = item();
        unhealthy.cluster_id = "payments".to_string();
        unhealthy.name = "payments".to_string();
        unhealthy.status = Some("degraded".to_string());

        let report =
            evaluate_kafka_cluster_inventory(&[missing, unhealthy], Pillar::Resilience, now());

        assert_eq!(report.resources_evaluated, 2);
        assert!(report
            .findings
            .iter()
            .any(|f| f.reason_code == REASON_RES_BOOTSTRAP_MISSING));
        assert!(report
            .findings
            .iter()
            .any(|f| f.reason_code == REASON_RES_STATUS_UNKNOWN));
        assert!(report
            .findings
            .iter()
            .any(|f| f.reason_code == REASON_RES_STATUS_UNHEALTHY));
    }

    #[test]
    fn security_flags_plaintext_and_missing_sasl_mechanism() {
        let plaintext = item();
        let mut sasl = item();
        sasl.cluster_id = "secure".to_string();
        sasl.name = "secure".to_string();
        sasl.security_protocol = "SASL_SSL".to_string();

        let report = evaluate_kafka_cluster_inventory(&[plaintext, sasl], Pillar::Security, now());

        assert_eq!(report.resources_evaluated, 2);
        assert!(report
            .findings
            .iter()
            .any(|f| f.reason_code == REASON_SEC_PLAINTEXT));
        assert!(report
            .findings
            .iter()
            .any(|f| f.reason_code == REASON_SEC_SASL_MISSING_MECHANISM));
    }

    #[test]
    fn stale_cluster_inventory_is_counted_for_any_pillar() {
        let mut item = item();
        item.owner = Some("platform".to_string());
        item.status = Some("healthy".to_string());
        item.security_protocol = "SSL".to_string();
        item.collected_at = now() - Duration::hours(DEFAULT_STALE_AFTER_HOURS + 2);

        let report = evaluate_kafka_cluster_inventory(&[item], Pillar::Cost, now());

        assert_eq!(report.stale_resources, 1);
        assert!(report
            .findings
            .iter()
            .any(|f| f.reason_code == REASON_INV_STALE_DATA));
    }

    #[test]
    fn healthy_cluster_passes_claimed_pillars() {
        let mut item = item();
        item.owner = Some("platform".to_string());
        item.security_protocol = "SSL".to_string();
        item.status = Some("healthy".to_string());

        for pillar in [Pillar::Cost, Pillar::Resilience, Pillar::Security] {
            let report = evaluate_kafka_cluster_inventory(&[item.clone()], pillar, now());
            assert_eq!(report.score, 100);
            assert!(report.findings.is_empty());
        }
    }
}

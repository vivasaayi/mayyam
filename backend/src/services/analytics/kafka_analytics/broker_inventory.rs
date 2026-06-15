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

// Deterministic Kafka broker inventory evaluator for roadmap rows
// 04-KAFKA-DASHBOARD-MANAGEMENT-00050/00057/00078.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::config::KafkaClusterConfig;
use crate::services::aws::inventory::types::{
    score_pillar, InventoryFinding, Pillar, PillarReport, Severity, COST_ALLOCATION_TAG_KEYS,
    DEFAULT_STALE_AFTER_HOURS,
};

pub const RESOURCE_TYPE: &str = "KafkaBroker";
pub const REASON_COST_OWNER_NOT_RECORDED: &str = "KAFKA_BROKER_COST_OWNER_NOT_RECORDED";
pub const REASON_COST_ENDPOINT_INCOMPLETE: &str = "KAFKA_BROKER_COST_ENDPOINT_INCOMPLETE";
pub const REASON_RES_ENDPOINT_INCOMPLETE: &str = "KAFKA_BROKER_RES_ENDPOINT_INCOMPLETE";
pub const REASON_RES_SINGLE_BROKER: &str = "KAFKA_BROKER_RES_SINGLE_BROKER_CLUSTER";
pub const REASON_SEC_ENDPOINT_INCOMPLETE: &str = "KAFKA_BROKER_SEC_ENDPOINT_INCOMPLETE";
pub const REASON_SEC_PLAINTEXT: &str = "KAFKA_BROKER_SEC_PLAINTEXT_PROTOCOL";
pub const REASON_SEC_PUBLIC_BINDING_REVIEW: &str = "KAFKA_BROKER_SEC_PUBLIC_BINDING_REVIEW";
pub const REASON_INV_STALE_DATA: &str = "KAFKA_BROKER_INV_STALE_DATA";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KafkaBrokerInventoryItem {
    pub broker_id: String,
    pub cluster_name: String,
    pub host: String,
    pub port: Option<u16>,
    pub owner: Option<String>,
    pub labels: BTreeMap<String, String>,
    pub security_protocol: String,
    pub cluster_broker_count: usize,
    pub collected_at: DateTime<Utc>,
}

pub fn evaluate_kafka_broker_inventory(
    items: &[KafkaBrokerInventoryItem],
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

pub fn broker_inventory_items_from_config(
    cluster: &KafkaClusterConfig,
    collected_at: DateTime<Utc>,
) -> Vec<KafkaBrokerInventoryItem> {
    let broker_count = cluster.bootstrap_servers.len();
    cluster
        .bootstrap_servers
        .iter()
        .enumerate()
        .map(|(index, endpoint)| {
            let (host, port) = parse_endpoint(endpoint);
            KafkaBrokerInventoryItem {
                broker_id: format!("{}:{}", cluster.name, endpoint),
                cluster_name: cluster.name.clone(),
                host: host.unwrap_or_else(|| format!("broker-{}", index + 1)),
                port,
                owner: None,
                labels: BTreeMap::new(),
                security_protocol: cluster.security_protocol.clone(),
                cluster_broker_count: broker_count,
                collected_at,
            }
        })
        .collect()
}

fn evaluate_cost(
    item: &KafkaBrokerInventoryItem,
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
                "Kafka broker {} has no owner, team, project, or cost-center metadata",
                item.broker_id
            ),
            json!({
                "broker_id": item.broker_id,
                "cluster_name": item.cluster_name,
                "checked_keys": COST_ALLOCATION_TAG_KEYS,
            }),
        ));
    }

    if !has_complete_endpoint(item) {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_ENDPOINT_INCOMPLETE,
            Severity::High,
            format!(
                "Kafka broker {} has incomplete endpoint inventory",
                item.broker_id
            ),
            json!({
                "broker_id": item.broker_id,
                "host": item.host,
                "port": item.port,
                "recommendation": "Collect host and port for each broker before attributing broker spend or dependency edges",
            }),
        ));
    }
}

fn evaluate_resilience(
    item: &KafkaBrokerInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if !has_complete_endpoint(item) {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_ENDPOINT_INCOMPLETE,
            Severity::High,
            format!(
                "Kafka broker {} has incomplete endpoint inventory for resilience checks",
                item.broker_id
            ),
            json!({
                "broker_id": item.broker_id,
                "host": item.host,
                "port": item.port,
                "recommendation": "Collect broker endpoints before evaluating reachability, rack spread, or controller failover dependencies",
            }),
        ));
    }

    if item.cluster_broker_count < 3 {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_SINGLE_BROKER,
            Severity::High,
            format!(
                "Kafka cluster {} has only {} configured broker endpoint(s)",
                item.cluster_name, item.cluster_broker_count
            ),
            json!({
                "cluster_name": item.cluster_name,
                "cluster_broker_count": item.cluster_broker_count,
                "recommendation": "Use at least three brokers across failure domains for production Kafka resilience",
            }),
        ));
    }
}

fn evaluate_security(
    item: &KafkaBrokerInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if !has_complete_endpoint(item) {
        findings.push(finding(
            item,
            pillar,
            REASON_SEC_ENDPOINT_INCOMPLETE,
            Severity::Medium,
            format!(
                "Kafka broker {} has incomplete endpoint inventory for security review",
                item.broker_id
            ),
            json!({
                "broker_id": item.broker_id,
                "host": item.host,
                "port": item.port,
                "recommendation": "Collect broker endpoints before assessing listener exposure and firewall posture",
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
                "Kafka broker {} is reached through PLAINTEXT client protocol",
                item.broker_id
            ),
            json!({
                "broker_id": item.broker_id,
                "security_protocol": item.security_protocol,
                "recommendation": "Use SSL or SASL_SSL so broker client traffic is encrypted and authenticated",
            }),
        ));
    }

    if is_public_binding(&item.host) {
        findings.push(finding(
            item,
            pillar,
            REASON_SEC_PUBLIC_BINDING_REVIEW,
            Severity::Medium,
            format!(
                "Kafka broker {} appears to use a public or wildcard host binding",
                item.broker_id
            ),
            json!({
                "broker_id": item.broker_id,
                "host": item.host,
                "recommendation": "Verify the broker listener is restricted to approved networks and protected by TLS and authentication",
            }),
        ));
    }
}

fn stale_finding(
    item: &KafkaBrokerInventoryItem,
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
            "Kafka broker inventory for {} is {} hours old (threshold {} hours)",
            item.broker_id, age_hours, DEFAULT_STALE_AFTER_HOURS
        ),
        json!({
            "broker_id": item.broker_id,
            "collected_at": item.collected_at,
            "age_hours": age_hours,
            "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
        }),
    ))
}

fn parse_endpoint(endpoint: &str) -> (Option<String>, Option<u16>) {
    let trimmed = endpoint.trim();
    let Some((host, port)) = trimmed.rsplit_once(':') else {
        return ((!trimmed.is_empty()).then(|| trimmed.to_string()), None);
    };
    let host = (!host.trim().is_empty()).then(|| host.trim().to_string());
    let port = port.trim().parse::<u16>().ok();
    (host, port)
}

fn has_owner_metadata(item: &KafkaBrokerInventoryItem) -> bool {
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

fn has_complete_endpoint(item: &KafkaBrokerInventoryItem) -> bool {
    !item.host.trim().is_empty() && item.port.is_some()
}

fn is_public_binding(host: &str) -> bool {
    matches!(host, "0.0.0.0" | "::" | "*") || host.ends_with(".amazonaws.com")
}

fn finding(
    item: &KafkaBrokerInventoryItem,
    pillar: Pillar,
    reason_code: &str,
    severity: Severity,
    message: String,
    evidence: Value,
) -> InventoryFinding {
    InventoryFinding {
        resource_id: item.broker_id.clone(),
        arn: format!("kafka:broker/{}", item.broker_id),
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

    fn item() -> KafkaBrokerInventoryItem {
        KafkaBrokerInventoryItem {
            broker_id: "orders:broker-1:9092".to_string(),
            cluster_name: "orders".to_string(),
            host: "broker-1".to_string(),
            port: Some(9092),
            owner: None,
            labels: BTreeMap::new(),
            security_protocol: "PLAINTEXT".to_string(),
            cluster_broker_count: 1,
            collected_at: now(),
        }
    }

    #[test]
    fn cost_flags_missing_owner_and_incomplete_endpoint() {
        let mut item = item();
        item.port = None;

        let report = evaluate_kafka_broker_inventory(&[item], Pillar::Cost, now());

        assert_eq!(report.resources_evaluated, 1);
        assert!(report
            .findings
            .iter()
            .any(|f| f.reason_code == REASON_COST_OWNER_NOT_RECORDED));
        assert!(report
            .findings
            .iter()
            .any(|f| f.reason_code == REASON_COST_ENDPOINT_INCOMPLETE));
    }

    #[test]
    fn resilience_flags_incomplete_endpoint_and_small_cluster() {
        let mut item = item();
        item.port = None;

        let report = evaluate_kafka_broker_inventory(&[item], Pillar::Resilience, now());

        assert!(report
            .findings
            .iter()
            .any(|f| f.reason_code == REASON_RES_ENDPOINT_INCOMPLETE));
        assert!(report
            .findings
            .iter()
            .any(|f| f.reason_code == REASON_RES_SINGLE_BROKER));
    }

    #[test]
    fn security_flags_plaintext_and_public_binding() {
        let mut item = item();
        item.host = "0.0.0.0".to_string();

        let report = evaluate_kafka_broker_inventory(&[item], Pillar::Security, now());

        assert!(report
            .findings
            .iter()
            .any(|f| f.reason_code == REASON_SEC_PLAINTEXT));
        assert!(report
            .findings
            .iter()
            .any(|f| f.reason_code == REASON_SEC_PUBLIC_BINDING_REVIEW));
    }

    #[test]
    fn stale_broker_inventory_is_counted_for_any_pillar() {
        let mut item = item();
        item.owner = Some("platform".to_string());
        item.security_protocol = "SSL".to_string();
        item.cluster_broker_count = 3;
        item.collected_at = now() - Duration::hours(DEFAULT_STALE_AFTER_HOURS + 2);

        let report = evaluate_kafka_broker_inventory(&[item], Pillar::Cost, now());

        assert_eq!(report.stale_resources, 1);
        assert!(report
            .findings
            .iter()
            .any(|f| f.reason_code == REASON_INV_STALE_DATA));
    }

    #[test]
    fn healthy_brokers_pass_claimed_pillars() {
        let mut item = item();
        item.owner = Some("platform".to_string());
        item.security_protocol = "SSL".to_string();
        item.cluster_broker_count = 3;

        for pillar in [Pillar::Cost, Pillar::Resilience, Pillar::Security] {
            let report = evaluate_kafka_broker_inventory(&[item.clone()], pillar, now());
            assert_eq!(report.score, 100);
            assert!(report.findings.is_empty());
        }
    }
}

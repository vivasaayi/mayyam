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

// Deterministic Kafka connector inventory evaluator for roadmap rows
// 04-KAFKA-DASHBOARD-MANAGEMENT-01030/01037/01058.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::config::KafkaClusterConfig;
use crate::services::aws::inventory::types::{
    score_pillar, InventoryFinding, Pillar, PillarReport, Severity, COST_ALLOCATION_TAG_KEYS,
    DEFAULT_STALE_AFTER_HOURS,
};

pub const RESOURCE_TYPE: &str = "KafkaConnector";
pub const REASON_COST_OWNER_NOT_RECORDED: &str = "KAFKA_CONNECTOR_COST_OWNER_NOT_RECORDED";
pub const REASON_COST_NO_EVIDENCE: &str = "KAFKA_CONNECTOR_COST_NO_EVIDENCE";
pub const REASON_COST_HIGH_TASK_COUNT: &str = "KAFKA_CONNECTOR_COST_HIGH_TASK_COUNT";
pub const REASON_RES_NO_EVIDENCE: &str = "KAFKA_CONNECTOR_RES_NO_EVIDENCE";
pub const REASON_RES_NO_AUDIT_EVIDENCE: &str = "KAFKA_CONNECTOR_RES_NO_AUDIT_EVIDENCE";
pub const REASON_RES_FAILED_TASKS: &str = "KAFKA_CONNECTOR_RES_FAILED_TASKS";
pub const REASON_SEC_NO_EVIDENCE: &str = "KAFKA_CONNECTOR_SEC_NO_EVIDENCE";
pub const REASON_SEC_UNREDACTED_SECRETS: &str = "KAFKA_CONNECTOR_SEC_UNREDACTED_SECRETS";
pub const REASON_SEC_PLAINTEXT_CONNECTION: &str = "KAFKA_CONNECTOR_SEC_PLAINTEXT_CONNECTION";
pub const REASON_INV_STALE_DATA: &str = "KAFKA_CONNECTOR_INV_STALE_DATA";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KafkaConnectorInventoryItem {
    pub connector_id: String,
    pub connect_cluster_id: String,
    pub cluster_name: String,
    pub connector_name: String,
    pub connector_class: Option<String>,
    pub connector_type: Option<String>,
    pub task_count: Option<u32>,
    pub failed_task_count: Option<u32>,
    pub owner: Option<String>,
    pub labels: BTreeMap<String, String>,
    pub audit_evidence: bool,
    pub secrets_redacted: bool,
    pub plaintext_connection: bool,
    pub has_connector_evidence: bool,
    pub collected_at: DateTime<Utc>,
}

pub fn evaluate_kafka_connector_inventory(
    items: &[KafkaConnectorInventoryItem],
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

pub fn connector_inventory_item_from_config(
    cluster: &KafkaClusterConfig,
    collected_at: DateTime<Utc>,
) -> KafkaConnectorInventoryItem {
    KafkaConnectorInventoryItem {
        connector_id: format!("{}:connectors", cluster.name),
        connect_cluster_id: format!("{}:kafka-connect", cluster.name),
        cluster_name: cluster.name.clone(),
        connector_name: "<uncollected>".to_string(),
        connector_class: None,
        connector_type: None,
        task_count: None,
        failed_task_count: None,
        owner: None,
        labels: BTreeMap::new(),
        audit_evidence: false,
        secrets_redacted: false,
        plaintext_connection: cluster.security_protocol.eq_ignore_ascii_case("PLAINTEXT"),
        has_connector_evidence: false,
        collected_at,
    }
}

fn evaluate_cost(
    item: &KafkaConnectorInventoryItem,
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
                "Kafka connector inventory for cluster {} has no owner, team, project, or cost-center metadata",
                item.cluster_name
            ),
            json!({
                "connector_id": item.connector_id,
                "cluster_name": item.cluster_name,
                "checked_keys": COST_ALLOCATION_TAG_KEYS,
            }),
        ));
    }

    if !item.has_connector_evidence {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_NO_EVIDENCE,
            Severity::High,
            format!(
                "Kafka connector inventory for cluster {} has no connector evidence",
                item.cluster_name
            ),
            json!({
                "connector_id": item.connector_id,
                "cluster_name": item.cluster_name,
                "recommendation": "Collect connector name, class, type, task counts, state, ownership, labels, secret redaction, and audit evidence before estimating connector cost posture",
            }),
        ));
    }

    if item.task_count.is_some_and(|task_count| task_count >= 100) {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_HIGH_TASK_COUNT,
            Severity::Medium,
            format!("Kafka connector {} has a high task count", item.connector_id),
            json!({
                "connector_id": item.connector_id,
                "task_count": item.task_count,
                "connector_class": item.connector_class,
                "recommendation": "Review connector task parallelism against throughput needs before scaling Connect workers",
            }),
        ));
    }
}

fn evaluate_resilience(
    item: &KafkaConnectorInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if !item.has_connector_evidence {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_NO_EVIDENCE,
            Severity::High,
            format!(
                "Kafka connector inventory for cluster {} has no resilience evidence",
                item.cluster_name
            ),
            json!({
                "connector_id": item.connector_id,
                "cluster_name": item.cluster_name,
                "recommendation": "Collect connector status, task state, restart policy, and audit evidence before accepting integration recovery posture",
            }),
        ));
    }

    if item.has_connector_evidence && !item.audit_evidence {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_NO_AUDIT_EVIDENCE,
            Severity::High,
            format!("Kafka connector {} has no audit evidence", item.connector_id),
            json!({
                "connector_id": item.connector_id,
                "audit_evidence": item.audit_evidence,
                "recommendation": "Enable or collect connector config and task lifecycle audit evidence before accepting recovery posture",
            }),
        ));
    }

    if item
        .failed_task_count
        .is_some_and(|failed_task_count| failed_task_count > 0)
    {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_FAILED_TASKS,
            Severity::High,
            format!("Kafka connector {} has failed tasks", item.connector_id),
            json!({
                "connector_id": item.connector_id,
                "failed_task_count": item.failed_task_count,
                "task_count": item.task_count,
                "recommendation": "Resolve failed connector tasks before accepting integration availability posture",
            }),
        ));
    }
}

fn evaluate_security(
    item: &KafkaConnectorInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if !item.has_connector_evidence {
        findings.push(finding(
            item,
            pillar,
            REASON_SEC_NO_EVIDENCE,
            Severity::High,
            format!(
                "Kafka connector inventory for cluster {} has no security evidence",
                item.cluster_name
            ),
            json!({
                "connector_id": item.connector_id,
                "cluster_name": item.cluster_name,
                "recommendation": "Collect connector config, secret redaction, endpoint transport, ownership, and audit evidence before accepting security posture",
            }),
        ));
    }

    if item.has_connector_evidence && !item.secrets_redacted {
        findings.push(finding(
            item,
            pillar,
            REASON_SEC_UNREDACTED_SECRETS,
            Severity::High,
            format!(
                "Kafka connector {} has unredacted or unverified secret handling",
                item.connector_id
            ),
            json!({
                "connector_id": item.connector_id,
                "secrets_redacted": item.secrets_redacted,
                "recommendation": "Store connector secrets in a secrets provider and redact sensitive config values from inventory evidence",
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
                "Kafka connector {} uses PLAINTEXT or unverified transport",
                item.connector_id
            ),
            json!({
                "connector_id": item.connector_id,
                "plaintext_connection": item.plaintext_connection,
                "recommendation": "Use encrypted connector source, sink, and Kafka transport before accepting security posture",
            }),
        ));
    }
}

fn stale_finding(
    item: &KafkaConnectorInventoryItem,
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
            "Kafka connector inventory for cluster {} is {} hour(s) old",
            item.cluster_name, age_hours
        ),
        json!({
            "connector_id": item.connector_id,
            "cluster_name": item.cluster_name,
            "age_hours": age_hours,
            "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
            "recommendation": "Refresh Kafka connector inventory before acting on this posture report",
        }),
    ))
}

fn has_owner_metadata(item: &KafkaConnectorInventoryItem) -> bool {
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
    item: &KafkaConnectorInventoryItem,
    pillar: Pillar,
    reason_code: &'static str,
    severity: Severity,
    message: String,
    evidence: Value,
) -> InventoryFinding {
    InventoryFinding {
        resource_id: item.connector_id.clone(),
        arn: format!("kafka:connector/{}", item.connector_id),
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

    fn connector(now: DateTime<Utc>) -> KafkaConnectorInventoryItem {
        KafkaConnectorInventoryItem {
            connector_id: "prod:connector:orders-sink".to_string(),
            connect_cluster_id: "prod:kafka-connect".to_string(),
            cluster_name: "prod".to_string(),
            connector_name: "orders-sink".to_string(),
            connector_class: Some("io.confluent.connect.jdbc.JdbcSinkConnector".to_string()),
            connector_type: Some("sink".to_string()),
            task_count: Some(8),
            failed_task_count: Some(0),
            owner: Some("orders".to_string()),
            labels: BTreeMap::new(),
            audit_evidence: true,
            secrets_redacted: true,
            plaintext_connection: false,
            has_connector_evidence: true,
            collected_at: now,
        }
    }

    #[test]
    fn healthy_connector_passes_claimed_pillars() {
        let now = Utc::now();
        let item = connector(now);

        for pillar in [Pillar::Cost, Pillar::Resilience, Pillar::Security] {
            let report =
                evaluate_kafka_connector_inventory(std::slice::from_ref(&item), pillar, now);
            assert_eq!(report.resources_evaluated, 1);
            assert!(report.findings.is_empty());
            assert_eq!(report.stale_resources, 0);
            assert_eq!(report.score, 100);
        }
    }

    #[test]
    fn cost_flags_missing_owner_missing_evidence_and_high_task_count() {
        let now = Utc::now();
        let mut missing_evidence = connector(now);
        missing_evidence.owner = None;
        missing_evidence.has_connector_evidence = false;
        let mut large = connector(now);
        large.task_count = Some(100);

        let report =
            evaluate_kafka_connector_inventory(&[missing_evidence, large], Pillar::Cost, now);

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
            .any(|finding| finding.reason_code == REASON_COST_HIGH_TASK_COUNT));
    }

    #[test]
    fn resilience_flags_missing_evidence_audit_and_failed_tasks() {
        let now = Utc::now();
        let mut missing_evidence = connector(now);
        missing_evidence.has_connector_evidence = false;
        let mut risky = connector(now);
        risky.audit_evidence = false;
        risky.failed_task_count = Some(3);

        let report =
            evaluate_kafka_connector_inventory(&[missing_evidence, risky], Pillar::Resilience, now);

        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_RES_NO_EVIDENCE));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_RES_NO_AUDIT_EVIDENCE));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_RES_FAILED_TASKS));
    }

    #[test]
    fn security_flags_missing_evidence_unredacted_secrets_and_plaintext() {
        let now = Utc::now();
        let mut missing_evidence = connector(now);
        missing_evidence.has_connector_evidence = false;
        let mut insecure = connector(now);
        insecure.secrets_redacted = false;
        insecure.plaintext_connection = true;

        let report = evaluate_kafka_connector_inventory(
            &[missing_evidence, insecure],
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
            .any(|finding| finding.reason_code == REASON_SEC_UNREDACTED_SECRETS));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_SEC_PLAINTEXT_CONNECTION));
    }

    #[test]
    fn stale_connector_inventory_is_counted_for_any_pillar() {
        let now = Utc::now();
        let mut item = connector(now);
        item.collected_at = now - Duration::hours(DEFAULT_STALE_AFTER_HOURS + 2);

        let report = evaluate_kafka_connector_inventory(&[item], Pillar::Security, now);

        assert_eq!(report.stale_resources, 1);
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_INV_STALE_DATA));
    }
}

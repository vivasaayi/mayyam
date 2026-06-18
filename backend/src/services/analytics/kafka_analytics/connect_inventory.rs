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

// Deterministic Kafka Connect inventory evaluator for roadmap rows
// 04-KAFKA-DASHBOARD-MANAGEMENT-00981/00988/01009.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::config::KafkaClusterConfig;
use crate::services::aws::inventory::types::{
    score_pillar, InventoryFinding, Pillar, PillarReport, Severity, COST_ALLOCATION_TAG_KEYS,
    DEFAULT_STALE_AFTER_HOURS,
};

pub const RESOURCE_TYPE: &str = "KafkaConnect";
pub const REASON_COST_OWNER_NOT_RECORDED: &str = "KAFKA_CONNECT_COST_OWNER_NOT_RECORDED";
pub const REASON_COST_NO_EVIDENCE: &str = "KAFKA_CONNECT_COST_NO_EVIDENCE";
pub const REASON_COST_HIGH_TASK_COUNT: &str = "KAFKA_CONNECT_COST_HIGH_TASK_COUNT";
pub const REASON_RES_NO_EVIDENCE: &str = "KAFKA_CONNECT_RES_NO_EVIDENCE";
pub const REASON_RES_NO_AUDIT_EVIDENCE: &str = "KAFKA_CONNECT_RES_NO_AUDIT_EVIDENCE";
pub const REASON_RES_FAILED_TASKS: &str = "KAFKA_CONNECT_RES_FAILED_TASKS";
pub const REASON_RES_SINGLE_WORKER: &str = "KAFKA_CONNECT_RES_SINGLE_WORKER";
pub const REASON_SEC_NO_EVIDENCE: &str = "KAFKA_CONNECT_SEC_NO_EVIDENCE";
pub const REASON_SEC_AUTH_DISABLED: &str = "KAFKA_CONNECT_SEC_AUTH_DISABLED";
pub const REASON_SEC_TLS_DISABLED: &str = "KAFKA_CONNECT_SEC_TLS_DISABLED";
pub const REASON_INV_STALE_DATA: &str = "KAFKA_CONNECT_INV_STALE_DATA";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KafkaConnectInventoryItem {
    pub connect_cluster_id: String,
    pub cluster_name: String,
    pub endpoint: Option<String>,
    pub worker_count: Option<u32>,
    pub connector_count: Option<u32>,
    pub task_count: Option<u32>,
    pub failed_task_count: Option<u32>,
    pub owner: Option<String>,
    pub labels: BTreeMap<String, String>,
    pub audit_evidence: bool,
    pub auth_enabled: bool,
    pub tls_enabled: bool,
    pub has_connect_evidence: bool,
    pub collected_at: DateTime<Utc>,
}

pub fn evaluate_kafka_connect_inventory(
    items: &[KafkaConnectInventoryItem],
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

pub fn connect_inventory_item_from_config(
    cluster: &KafkaClusterConfig,
    collected_at: DateTime<Utc>,
) -> KafkaConnectInventoryItem {
    KafkaConnectInventoryItem {
        connect_cluster_id: format!("{}:kafka-connect", cluster.name),
        cluster_name: cluster.name.clone(),
        endpoint: None,
        worker_count: None,
        connector_count: None,
        task_count: None,
        failed_task_count: None,
        owner: None,
        labels: BTreeMap::new(),
        audit_evidence: false,
        auth_enabled: false,
        tls_enabled: !cluster.security_protocol.eq_ignore_ascii_case("PLAINTEXT"),
        has_connect_evidence: false,
        collected_at,
    }
}

fn evaluate_cost(
    item: &KafkaConnectInventoryItem,
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
                "Kafka Connect inventory for cluster {} has no owner, team, project, or cost-center metadata",
                item.cluster_name
            ),
            json!({
                "connect_cluster_id": item.connect_cluster_id,
                "cluster_name": item.cluster_name,
                "checked_keys": COST_ALLOCATION_TAG_KEYS,
            }),
        ));
    }

    if !item.has_connect_evidence {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_NO_EVIDENCE,
            Severity::High,
            format!(
                "Kafka Connect inventory for cluster {} has no Connect evidence",
                item.cluster_name
            ),
            json!({
                "connect_cluster_id": item.connect_cluster_id,
                "cluster_name": item.cluster_name,
                "recommendation": "Collect Kafka Connect endpoint, workers, connectors, tasks, ownership, labels, auth, TLS, and audit evidence before estimating cost posture",
            }),
        ));
    }

    if item
        .task_count
        .is_some_and(|task_count| task_count >= 1_000)
    {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_HIGH_TASK_COUNT,
            Severity::Medium,
            format!(
                "Kafka Connect cluster {} has a high task count",
                item.connect_cluster_id
            ),
            json!({
                "connect_cluster_id": item.connect_cluster_id,
                "connector_count": item.connector_count,
                "task_count": item.task_count,
                "worker_count": item.worker_count,
                "recommendation": "Review task parallelism and connector consolidation before scaling Connect worker capacity",
            }),
        ));
    }
}

fn evaluate_resilience(
    item: &KafkaConnectInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if !item.has_connect_evidence {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_NO_EVIDENCE,
            Severity::High,
            format!(
                "Kafka Connect inventory for cluster {} has no resilience evidence",
                item.cluster_name
            ),
            json!({
                "connect_cluster_id": item.connect_cluster_id,
                "cluster_name": item.cluster_name,
                "recommendation": "Collect Connect worker, connector, task, status, and audit evidence before relying on integration recovery posture",
            }),
        ));
    }

    if item.has_connect_evidence && !item.audit_evidence {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_NO_AUDIT_EVIDENCE,
            Severity::High,
            format!(
                "Kafka Connect cluster {} has no audit evidence",
                item.connect_cluster_id
            ),
            json!({
                "connect_cluster_id": item.connect_cluster_id,
                "audit_evidence": item.audit_evidence,
                "recommendation": "Enable or collect Kafka Connect connector and task change audit evidence before accepting recovery posture",
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
            format!(
                "Kafka Connect cluster {} has failed tasks",
                item.connect_cluster_id
            ),
            json!({
                "connect_cluster_id": item.connect_cluster_id,
                "failed_task_count": item.failed_task_count,
                "task_count": item.task_count,
                "recommendation": "Resolve failed Connect tasks before accepting integration availability posture",
            }),
        ));
    }

    if item
        .worker_count
        .is_some_and(|worker_count| worker_count <= 1)
    {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_SINGLE_WORKER,
            Severity::Medium,
            format!(
                "Kafka Connect cluster {} has one or fewer workers",
                item.connect_cluster_id
            ),
            json!({
                "connect_cluster_id": item.connect_cluster_id,
                "worker_count": item.worker_count,
                "recommendation": "Run multiple Connect workers for production connector availability and rolling maintenance",
            }),
        ));
    }
}

fn evaluate_security(
    item: &KafkaConnectInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if !item.has_connect_evidence {
        findings.push(finding(
            item,
            pillar,
            REASON_SEC_NO_EVIDENCE,
            Severity::High,
            format!(
                "Kafka Connect inventory for cluster {} has no security evidence",
                item.cluster_name
            ),
            json!({
                "connect_cluster_id": item.connect_cluster_id,
                "cluster_name": item.cluster_name,
                "recommendation": "Collect Kafka Connect auth, TLS, endpoint, connector, task, and audit evidence before accepting security posture",
            }),
        ));
    }

    if item.has_connect_evidence && !item.auth_enabled {
        findings.push(finding(
            item,
            pillar,
            REASON_SEC_AUTH_DISABLED,
            Severity::High,
            format!(
                "Kafka Connect cluster {} has authentication disabled or unverified",
                item.connect_cluster_id
            ),
            json!({
                "connect_cluster_id": item.connect_cluster_id,
                "auth_enabled": item.auth_enabled,
                "recommendation": "Require authenticated Kafka Connect API access before allowing connector inspection or mutation",
            }),
        ));
    }

    if !item.tls_enabled {
        findings.push(finding(
            item,
            pillar,
            REASON_SEC_TLS_DISABLED,
            Severity::High,
            format!(
                "Kafka Connect cluster {} has TLS disabled or unverified",
                item.connect_cluster_id
            ),
            json!({
                "connect_cluster_id": item.connect_cluster_id,
                "tls_enabled": item.tls_enabled,
                "endpoint": item.endpoint,
                "recommendation": "Use HTTPS/TLS for Kafka Connect API and worker traffic before accepting security posture",
            }),
        ));
    }
}

fn stale_finding(
    item: &KafkaConnectInventoryItem,
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
            "Kafka Connect inventory for cluster {} is {} hour(s) old",
            item.cluster_name, age_hours
        ),
        json!({
            "connect_cluster_id": item.connect_cluster_id,
            "cluster_name": item.cluster_name,
            "age_hours": age_hours,
            "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
            "recommendation": "Refresh Kafka Connect inventory before acting on this posture report",
        }),
    ))
}

fn has_owner_metadata(item: &KafkaConnectInventoryItem) -> bool {
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
    item: &KafkaConnectInventoryItem,
    pillar: Pillar,
    reason_code: &'static str,
    severity: Severity,
    message: String,
    evidence: Value,
) -> InventoryFinding {
    InventoryFinding {
        resource_id: item.connect_cluster_id.clone(),
        arn: format!("kafka:connect/{}", item.connect_cluster_id),
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

    fn connect(now: DateTime<Utc>) -> KafkaConnectInventoryItem {
        KafkaConnectInventoryItem {
            connect_cluster_id: "prod:kafka-connect".to_string(),
            cluster_name: "prod".to_string(),
            endpoint: Some("https://connect.example.com".to_string()),
            worker_count: Some(3),
            connector_count: Some(12),
            task_count: Some(48),
            failed_task_count: Some(0),
            owner: Some("integration-platform".to_string()),
            labels: BTreeMap::new(),
            audit_evidence: true,
            auth_enabled: true,
            tls_enabled: true,
            has_connect_evidence: true,
            collected_at: now,
        }
    }

    #[test]
    fn healthy_kafka_connect_passes_claimed_pillars() {
        let now = Utc::now();
        let item = connect(now);

        for pillar in [Pillar::Cost, Pillar::Resilience, Pillar::Security] {
            let report = evaluate_kafka_connect_inventory(std::slice::from_ref(&item), pillar, now);
            assert_eq!(report.resources_evaluated, 1);
            assert!(report.findings.is_empty());
            assert_eq!(report.stale_resources, 0);
            assert_eq!(report.score, 100);
        }
    }

    #[test]
    fn cost_flags_missing_owner_missing_evidence_and_high_task_count() {
        let now = Utc::now();
        let mut missing_evidence = connect(now);
        missing_evidence.owner = None;
        missing_evidence.has_connect_evidence = false;
        let mut large = connect(now);
        large.task_count = Some(1_000);

        let report =
            evaluate_kafka_connect_inventory(&[missing_evidence, large], Pillar::Cost, now);

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
    fn resilience_flags_missing_evidence_audit_failed_tasks_and_single_worker() {
        let now = Utc::now();
        let mut missing_evidence = connect(now);
        missing_evidence.has_connect_evidence = false;
        let mut risky = connect(now);
        risky.audit_evidence = false;
        risky.failed_task_count = Some(2);
        risky.worker_count = Some(1);

        let report =
            evaluate_kafka_connect_inventory(&[missing_evidence, risky], Pillar::Resilience, now);

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
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_RES_SINGLE_WORKER));
    }

    #[test]
    fn security_flags_missing_evidence_disabled_auth_and_tls() {
        let now = Utc::now();
        let mut missing_evidence = connect(now);
        missing_evidence.has_connect_evidence = false;
        let mut insecure = connect(now);
        insecure.auth_enabled = false;
        insecure.tls_enabled = false;

        let report =
            evaluate_kafka_connect_inventory(&[missing_evidence, insecure], Pillar::Security, now);

        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_SEC_NO_EVIDENCE));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_SEC_AUTH_DISABLED));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_SEC_TLS_DISABLED));
    }

    #[test]
    fn stale_kafka_connect_inventory_is_counted_for_any_pillar() {
        let now = Utc::now();
        let mut item = connect(now);
        item.collected_at = now - Duration::hours(DEFAULT_STALE_AFTER_HOURS + 2);

        let report = evaluate_kafka_connect_inventory(&[item], Pillar::Security, now);

        assert_eq!(report.stale_resources, 1);
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_INV_STALE_DATA));
    }
}

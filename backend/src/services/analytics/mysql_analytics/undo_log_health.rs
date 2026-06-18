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

// Deterministic MySQL undo log health evaluator for roadmap rows
// 03-MYSQL-AI-TRIAGER-00345/00352/00373.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::services::analytics::mysql_analytics::mysql_telemetry::MySqlTelemetrySnapshot;
use crate::services::aws::inventory::types::{
    score_pillar, InventoryFinding, Pillar, PillarReport, Severity, DEFAULT_STALE_AFTER_HOURS,
};

pub const RESOURCE_TYPE: &str = "MySqlUndoLogHealth";
pub const REASON_COST_NO_UNDO_LOG_METRICS: &str = "MYSQL_UNDO_LOG_HEALTH_COST_NO_METRICS";
pub const REASON_COST_UNDO_PRESSURE_SPEND_REVIEW: &str =
    "MYSQL_UNDO_LOG_HEALTH_COST_UNDO_PRESSURE_SPEND_REVIEW";
pub const REASON_RES_NO_UNDO_LOG_METRICS: &str = "MYSQL_UNDO_LOG_HEALTH_RES_NO_METRICS";
pub const REASON_RES_TRANSACTION_PRESSURE: &str = "MYSQL_UNDO_LOG_HEALTH_RES_TRANSACTION_PRESSURE";
pub const REASON_RES_WRITE_PRESSURE: &str = "MYSQL_UNDO_LOG_HEALTH_RES_WRITE_PRESSURE";
pub const REASON_SEC_VERSION_MISSING: &str = "MYSQL_UNDO_LOG_HEALTH_SEC_VERSION_MISSING";
pub const REASON_SEC_NO_UNDO_LOG_METRICS: &str = "MYSQL_UNDO_LOG_HEALTH_SEC_NO_METRICS";
pub const REASON_SEC_UNDO_LOG_REVIEW: &str = "MYSQL_UNDO_LOG_HEALTH_SEC_UNDO_LOG_REVIEW";
pub const REASON_INV_STALE_DATA: &str = "MYSQL_UNDO_LOG_HEALTH_STALE_DATA";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UndoLogHealthItem {
    pub connection_id: String,
    pub connection_name: String,
    pub server_version: Option<String>,
    pub undo_log_metric_count: usize,
    pub row_lock_waits: i64,
    pub row_lock_time_ms: i64,
    pub deadlocks: i64,
    pub write_operations: i64,
    pub qps_since_start: f64,
    pub collected_at: DateTime<Utc>,
}

pub fn evaluate_mysql_undo_log_health(
    items: &[UndoLogHealthItem],
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

pub fn undo_log_health_item_from_telemetry(
    connection_id: &str,
    connection_name: &str,
    snapshot: &MySqlTelemetrySnapshot,
) -> UndoLogHealthItem {
    UndoLogHealthItem {
        connection_id: connection_id.to_string(),
        connection_name: connection_name.to_string(),
        server_version: snapshot.server.version.clone(),
        undo_log_metric_count: 3,
        row_lock_waits: snapshot.innodb.row_lock_waits,
        row_lock_time_ms: snapshot.innodb.row_lock_time_ms,
        deadlocks: snapshot.innodb.deadlocks,
        write_operations: snapshot.workload.com_insert
            + snapshot.workload.com_update
            + snapshot.workload.com_delete,
        qps_since_start: snapshot.workload.qps_since_start,
        collected_at: snapshot.collected_at,
    }
}

fn evaluate_cost(item: &UndoLogHealthItem, pillar: Pillar, findings: &mut Vec<InventoryFinding>) {
    if !has_undo_log_metrics(item) {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_NO_UNDO_LOG_METRICS,
            Severity::High,
            format!(
                "MySQL undo log health for {} has no undo log evidence",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "undo_log_metric_count": item.undo_log_metric_count,
                "recommendation": "Collect row-lock, deadlock, and write-workload counters before making undo-retention, purge, or capacity-spend recommendations",
            }),
        ));
    }

    if has_undo_pressure_spend_pressure(item) {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_UNDO_PRESSURE_SPEND_REVIEW,
            Severity::Medium,
            format!(
                "MySQL undo log health for {} needs spend review before scaling",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "row_lock_waits": item.row_lock_waits,
                "row_lock_time_ms": item.row_lock_time_ms,
                "deadlocks": item.deadlocks,
                "write_operations": item.write_operations,
                "qps_since_start": item.qps_since_start,
                "recommendation": "Review long transactions, purge lag symptoms, row-lock pressure, and write spikes before adding capacity or storage spend",
            }),
        ));
    }
}

fn evaluate_resilience(
    item: &UndoLogHealthItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if !has_undo_log_metrics(item) {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_NO_UNDO_LOG_METRICS,
            Severity::High,
            format!(
                "MySQL undo log health for {} has no resilience evidence",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "undo_log_metric_count": item.undo_log_metric_count,
                "recommendation": "Collect row-lock wait, lock time, deadlock, and write counters so transaction stalls are not treated as generic saturation",
            }),
        ));
    }

    if has_transaction_pressure(item) {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_TRANSACTION_PRESSURE,
            Severity::High,
            format!(
                "MySQL undo log health for {} shows transaction or lock pressure",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "row_lock_waits": item.row_lock_waits,
                "row_lock_time_ms": item.row_lock_time_ms,
                "deadlocks": item.deadlocks,
                "recommendation": "Investigate long-running transactions, purge lag symptoms, row-lock waits, and deadlocks before failover or broad incident escalation",
            }),
        ));
    }

    if item.write_operations >= 100_000 || item.qps_since_start >= 500.0 {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_WRITE_PRESSURE,
            Severity::Medium,
            format!(
                "MySQL undo log health for {} shows sustained write pressure",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "write_operations": item.write_operations,
                "qps_since_start": item.qps_since_start,
                "recommendation": "Correlate write pressure with transaction, lock, and purge evidence before resilience actions",
            }),
        ));
    }
}

fn evaluate_security(
    item: &UndoLogHealthItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if item
        .server_version
        .as_deref()
        .map(str::trim)
        .filter(|version| !version.is_empty())
        .is_none()
    {
        findings.push(finding(
            item,
            pillar,
            REASON_SEC_VERSION_MISSING,
            Severity::Medium,
            format!(
                "MySQL undo log health for {} has no server version evidence",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "server_version": item.server_version,
                "recommendation": "Record MySQL server version with undo-log health evidence so version-specific transaction and purge guidance can be mapped",
            }),
        ));
    }

    if !has_undo_log_metrics(item) {
        findings.push(finding(
            item,
            pillar,
            REASON_SEC_NO_UNDO_LOG_METRICS,
            Severity::High,
            format!(
                "MySQL undo log health for {} has no scoped evidence for security review",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "undo_log_metric_count": item.undo_log_metric_count,
                "recommendation": "Collect scoped undo-log and transaction evidence so incident review does not require ad hoc privileged diagnostics",
            }),
        ));
    }

    if has_transaction_pressure(item) || item.write_operations >= 100_000 {
        findings.push(finding(
            item,
            pillar,
            REASON_SEC_UNDO_LOG_REVIEW,
            Severity::Medium,
            format!(
                "MySQL undo log health for {} should be reviewed before exporting incident evidence",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "row_lock_waits": item.row_lock_waits,
                "row_lock_time_ms": item.row_lock_time_ms,
                "deadlocks": item.deadlocks,
                "write_operations": item.write_operations,
                "recommendation": "Review transaction and lock evidence with scoped credentials and redact workload-sensitive write evidence before sharing outside the database team",
            }),
        ));
    }
}

fn stale_finding(
    item: &UndoLogHealthItem,
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
            "Undo log health data for {} is {} hours old (threshold {} hours)",
            item.connection_name, age_hours, DEFAULT_STALE_AFTER_HOURS
        ),
        json!({
            "connection_id": item.connection_id,
            "collected_at": item.collected_at,
            "age_hours": age_hours,
            "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
        }),
    ))
}

fn finding(
    item: &UndoLogHealthItem,
    pillar: Pillar,
    reason_code: &str,
    severity: Severity,
    message: String,
    evidence: Value,
) -> InventoryFinding {
    InventoryFinding {
        resource_id: item.connection_id.clone(),
        arn: format!("mysql-undo-log-health:{}", item.connection_id),
        pillar,
        reason_code: reason_code.to_string(),
        severity,
        message,
        evidence,
    }
}

fn has_undo_log_metrics(item: &UndoLogHealthItem) -> bool {
    item.undo_log_metric_count > 0
}

fn has_transaction_pressure(item: &UndoLogHealthItem) -> bool {
    has_undo_log_metrics(item)
        && (item.row_lock_waits > 0 || item.row_lock_time_ms > 0 || item.deadlocks > 0)
}

fn has_undo_pressure_spend_pressure(item: &UndoLogHealthItem) -> bool {
    has_transaction_pressure(item) && (item.write_operations > 0 || item.qps_since_start > 0.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    fn item() -> UndoLogHealthItem {
        UndoLogHealthItem {
            connection_id: "conn-1".to_string(),
            connection_name: "orders-mysql".to_string(),
            server_version: Some("8.0.36".to_string()),
            undo_log_metric_count: 3,
            row_lock_waits: 0,
            row_lock_time_ms: 0,
            deadlocks: 0,
            write_operations: 2_500,
            qps_since_start: 25.0,
            collected_at: Utc::now(),
        }
    }

    #[test]
    fn cost_flags_missing_metrics_and_undo_pressure_spend_review() {
        let now = Utc::now();
        let missing = UndoLogHealthItem {
            undo_log_metric_count: 0,
            ..item()
        };
        let pressured = UndoLogHealthItem {
            row_lock_waits: 10,
            row_lock_time_ms: 4_000,
            deadlocks: 1,
            write_operations: 80_000,
            qps_since_start: 150.0,
            ..item()
        };

        let report = evaluate_mysql_undo_log_health(&[missing, pressured], Pillar::Cost, now);

        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_COST_NO_UNDO_LOG_METRICS));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_COST_UNDO_PRESSURE_SPEND_REVIEW));
    }

    #[test]
    fn resilience_flags_missing_metrics_transaction_pressure_and_write_pressure() {
        let now = Utc::now();
        let missing = UndoLogHealthItem {
            undo_log_metric_count: 0,
            ..item()
        };
        let pressured = UndoLogHealthItem {
            row_lock_waits: 12,
            row_lock_time_ms: 8_000,
            deadlocks: 1,
            write_operations: 150_000,
            qps_since_start: 650.0,
            ..item()
        };

        let report = evaluate_mysql_undo_log_health(&[missing, pressured], Pillar::Resilience, now);

        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_RES_NO_UNDO_LOG_METRICS));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_RES_TRANSACTION_PRESSURE));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_RES_WRITE_PRESSURE));
    }

    #[test]
    fn security_flags_missing_version_missing_metrics_and_undo_review() {
        let now = Utc::now();
        let missing = UndoLogHealthItem {
            server_version: None,
            undo_log_metric_count: 0,
            ..item()
        };
        let pressured = UndoLogHealthItem {
            row_lock_waits: 5,
            ..item()
        };

        let report = evaluate_mysql_undo_log_health(&[missing, pressured], Pillar::Security, now);

        assert_eq!(report.findings.len(), 3);
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_SEC_VERSION_MISSING));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_SEC_NO_UNDO_LOG_METRICS));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_SEC_UNDO_LOG_REVIEW));
    }

    #[test]
    fn stale_undo_log_health_data_is_counted_for_any_pillar() {
        let now = Utc::now();
        let mut item = item();
        item.collected_at = now - Duration::hours(DEFAULT_STALE_AFTER_HOURS + 2);

        let report = evaluate_mysql_undo_log_health(&[item], Pillar::Resilience, now);

        assert_eq!(report.stale_resources, 1);
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_INV_STALE_DATA));
    }

    #[test]
    fn healthy_undo_log_health_passes_claimed_pillars() {
        let now = Utc::now();
        let items = vec![item()];

        for pillar in [Pillar::Cost, Pillar::Resilience, Pillar::Security] {
            let report = evaluate_mysql_undo_log_health(&items, pillar, now);
            assert_eq!(report.score, 100);
            assert!(report.findings.is_empty());
        }
    }
}

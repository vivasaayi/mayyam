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

// Deterministic MySQL redo log health evaluator for roadmap rows
// 03-MYSQL-AI-TRIAGER-00296/00303/00324.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::services::analytics::mysql_analytics::mysql_telemetry::MySqlTelemetrySnapshot;
use crate::services::aws::inventory::types::{
    score_pillar, InventoryFinding, Pillar, PillarReport, Severity, DEFAULT_STALE_AFTER_HOURS,
};

pub const RESOURCE_TYPE: &str = "MySqlRedoLogHealth";
pub const REASON_COST_NO_REDO_LOG_METRICS: &str = "MYSQL_REDO_LOG_HEALTH_COST_NO_METRICS";
pub const REASON_COST_WRITE_STALL_SPEND_REVIEW: &str =
    "MYSQL_REDO_LOG_HEALTH_COST_WRITE_STALL_SPEND_REVIEW";
pub const REASON_RES_NO_REDO_LOG_METRICS: &str = "MYSQL_REDO_LOG_HEALTH_RES_NO_METRICS";
pub const REASON_RES_REDO_LOG_WAITS: &str = "MYSQL_REDO_LOG_HEALTH_RES_WAITS";
pub const REASON_RES_WRITE_PRESSURE: &str = "MYSQL_REDO_LOG_HEALTH_RES_WRITE_PRESSURE";
pub const REASON_SEC_VERSION_MISSING: &str = "MYSQL_REDO_LOG_HEALTH_SEC_VERSION_MISSING";
pub const REASON_SEC_NO_REDO_LOG_METRICS: &str = "MYSQL_REDO_LOG_HEALTH_SEC_NO_METRICS";
pub const REASON_SEC_REDO_LOG_REVIEW: &str = "MYSQL_REDO_LOG_HEALTH_SEC_REDO_LOG_REVIEW";
pub const REASON_INV_STALE_DATA: &str = "MYSQL_REDO_LOG_HEALTH_STALE_DATA";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedoLogHealthItem {
    pub connection_id: String,
    pub connection_name: String,
    pub server_version: Option<String>,
    pub redo_log_metric_count: usize,
    pub log_waits: i64,
    pub write_operations: i64,
    pub qps_since_start: f64,
    pub collected_at: DateTime<Utc>,
}

pub fn evaluate_mysql_redo_log_health(
    items: &[RedoLogHealthItem],
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

pub fn redo_log_health_item_from_telemetry(
    connection_id: &str,
    connection_name: &str,
    snapshot: &MySqlTelemetrySnapshot,
) -> RedoLogHealthItem {
    RedoLogHealthItem {
        connection_id: connection_id.to_string(),
        connection_name: connection_name.to_string(),
        server_version: snapshot.server.version.clone(),
        redo_log_metric_count: 1,
        log_waits: snapshot.innodb.log_waits,
        write_operations: snapshot.workload.com_insert
            + snapshot.workload.com_update
            + snapshot.workload.com_delete,
        qps_since_start: snapshot.workload.qps_since_start,
        collected_at: snapshot.collected_at,
    }
}

fn evaluate_cost(item: &RedoLogHealthItem, pillar: Pillar, findings: &mut Vec<InventoryFinding>) {
    if !has_redo_log_metrics(item) {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_NO_REDO_LOG_METRICS,
            Severity::High,
            format!(
                "MySQL redo log health for {} has no redo log metrics",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "redo_log_metric_count": item.redo_log_metric_count,
                "recommendation": "Collect Innodb_log_waits and write workload counters before changing database spend",
            }),
        ));
    }

    if has_write_stall_spend_pressure(item) {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_WRITE_STALL_SPEND_REVIEW,
            Severity::Medium,
            format!(
                "MySQL redo log health for {} needs spend review before scaling",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "log_waits": item.log_waits,
                "write_operations": item.write_operations,
                "qps_since_start": item.qps_since_start,
                "recommendation": "Review redo log sizing, flush behavior, and write bursts before adding compute or storage capacity",
            }),
        ));
    }
}

fn evaluate_resilience(
    item: &RedoLogHealthItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if !has_redo_log_metrics(item) {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_NO_REDO_LOG_METRICS,
            Severity::High,
            format!(
                "MySQL redo log health for {} has no resilience evidence",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "redo_log_metric_count": item.redo_log_metric_count,
                "recommendation": "Collect redo log waits and write workload counters so write stalls can be separated from query, lock, or storage symptoms",
            }),
        ));
    }

    if item.log_waits > 0 {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_REDO_LOG_WAITS,
            Severity::High,
            format!(
                "MySQL redo log waits are present for {}",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "log_waits": item.log_waits,
                "write_operations": item.write_operations,
                "recommendation": "Investigate redo log waits before treating write latency as generic saturation or initiating failover",
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
                "MySQL redo log health for {} shows sustained write pressure",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "write_operations": item.write_operations,
                "qps_since_start": item.qps_since_start,
                "recommendation": "Correlate write pressure with checkpoint, lock, and storage telemetry before resilience actions",
            }),
        ));
    }
}

fn evaluate_security(
    item: &RedoLogHealthItem,
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
                "MySQL redo log health for {} has no server version evidence",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "server_version": item.server_version,
                "recommendation": "Record MySQL server version with redo-log health evidence so version-specific security guidance can be mapped",
            }),
        ));
    }

    if !has_redo_log_metrics(item) {
        findings.push(finding(
            item,
            pillar,
            REASON_SEC_NO_REDO_LOG_METRICS,
            Severity::High,
            format!(
                "MySQL redo log health for {} has no redo-log evidence for security review",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "redo_log_metric_count": item.redo_log_metric_count,
                "recommendation": "Collect scoped redo-log evidence so incident review does not require ad hoc privileged diagnostics",
            }),
        ));
    }

    if item.log_waits > 0 || item.write_operations >= 100_000 {
        findings.push(finding(
            item,
            pillar,
            REASON_SEC_REDO_LOG_REVIEW,
            Severity::Medium,
            format!(
                "MySQL redo log health for {} should be reviewed before exporting incident evidence",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "log_waits": item.log_waits,
                "write_operations": item.write_operations,
                "recommendation": "Review redo-log wait and write evidence with scoped credentials and redact workload-sensitive details before sharing outside the database team",
            }),
        ));
    }
}

fn stale_finding(
    item: &RedoLogHealthItem,
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
            "Redo log health data for {} is {} hours old (threshold {} hours)",
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
    item: &RedoLogHealthItem,
    pillar: Pillar,
    reason_code: &str,
    severity: Severity,
    message: String,
    evidence: Value,
) -> InventoryFinding {
    InventoryFinding {
        resource_id: item.connection_id.clone(),
        arn: format!("mysql-redo-log-health:{}", item.connection_id),
        pillar,
        reason_code: reason_code.to_string(),
        severity,
        message,
        evidence,
    }
}

fn has_redo_log_metrics(item: &RedoLogHealthItem) -> bool {
    item.redo_log_metric_count > 0
}

fn has_write_stall_spend_pressure(item: &RedoLogHealthItem) -> bool {
    has_redo_log_metrics(item)
        && item.log_waits > 0
        && (item.write_operations > 0 || item.qps_since_start > 0.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    fn item() -> RedoLogHealthItem {
        RedoLogHealthItem {
            connection_id: "conn-1".to_string(),
            connection_name: "orders-mysql".to_string(),
            server_version: Some("8.0.36".to_string()),
            redo_log_metric_count: 1,
            log_waits: 0,
            write_operations: 2_500,
            qps_since_start: 25.0,
            collected_at: Utc::now(),
        }
    }

    #[test]
    fn cost_flags_missing_metrics_and_write_stall_spend_review() {
        let now = Utc::now();
        let missing = RedoLogHealthItem {
            redo_log_metric_count: 0,
            ..item()
        };
        let stalled = RedoLogHealthItem {
            log_waits: 3,
            write_operations: 80_000,
            qps_since_start: 150.0,
            ..item()
        };

        let report = evaluate_mysql_redo_log_health(&[missing, stalled], Pillar::Cost, now);

        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_COST_NO_REDO_LOG_METRICS));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_COST_WRITE_STALL_SPEND_REVIEW));
    }

    #[test]
    fn resilience_flags_missing_metrics_redo_waits_and_write_pressure() {
        let now = Utc::now();
        let missing = RedoLogHealthItem {
            redo_log_metric_count: 0,
            ..item()
        };
        let pressured = RedoLogHealthItem {
            log_waits: 2,
            write_operations: 150_000,
            qps_since_start: 650.0,
            ..item()
        };

        let report = evaluate_mysql_redo_log_health(&[missing, pressured], Pillar::Resilience, now);

        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_RES_NO_REDO_LOG_METRICS));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_RES_REDO_LOG_WAITS));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_RES_WRITE_PRESSURE));
    }

    #[test]
    fn security_flags_missing_version_missing_metrics_and_redo_review() {
        let now = Utc::now();
        let mut item = item();
        item.server_version = None;
        item.redo_log_metric_count = 0;
        item.log_waits = 1;

        let report = evaluate_mysql_redo_log_health(&[item], Pillar::Security, now);

        assert_eq!(report.findings.len(), 3);
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_SEC_VERSION_MISSING));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_SEC_NO_REDO_LOG_METRICS));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_SEC_REDO_LOG_REVIEW));
    }

    #[test]
    fn stale_redo_log_health_data_is_counted_for_any_pillar() {
        let now = Utc::now();
        let mut item = item();
        item.collected_at = now - Duration::hours(DEFAULT_STALE_AFTER_HOURS + 1);

        let report = evaluate_mysql_redo_log_health(&[item], Pillar::Cost, now);

        assert_eq!(report.stale_resources, 1);
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_INV_STALE_DATA));
    }

    #[test]
    fn healthy_redo_log_health_passes_claimed_pillars() {
        let now = Utc::now();
        let items = vec![item()];

        for pillar in [Pillar::Cost, Pillar::Resilience, Pillar::Security] {
            let report = evaluate_mysql_redo_log_health(&items, pillar, now);
            assert_eq!(report.score, 100);
            assert!(report.findings.is_empty());
        }
    }
}

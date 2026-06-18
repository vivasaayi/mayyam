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

// Deterministic MySQL binary log health evaluator for roadmap rows
// 03-MYSQL-AI-TRIAGER-00394/00401/00422.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::services::analytics::mysql_analytics::mysql_telemetry::MySqlTelemetrySnapshot;
use crate::services::aws::inventory::types::{
    score_pillar, InventoryFinding, Pillar, PillarReport, Severity, DEFAULT_STALE_AFTER_HOURS,
};

pub const RESOURCE_TYPE: &str = "MySqlBinaryLogHealth";
pub const REASON_COST_NO_BINARY_LOG_METRICS: &str = "MYSQL_BINARY_LOG_HEALTH_COST_NO_METRICS";
pub const REASON_COST_WRITE_SPEND_REVIEW: &str = "MYSQL_BINARY_LOG_HEALTH_COST_WRITE_SPEND_REVIEW";
pub const REASON_RES_BINARY_LOG_DISABLED: &str = "MYSQL_BINARY_LOG_HEALTH_RES_DISABLED";
pub const REASON_RES_NO_BINARY_LOG_METRICS: &str = "MYSQL_BINARY_LOG_HEALTH_RES_NO_METRICS";
pub const REASON_RES_RETENTION_MISSING: &str = "MYSQL_BINARY_LOG_HEALTH_RES_RETENTION_MISSING";
pub const REASON_RES_WRITE_LOG_PRESSURE: &str = "MYSQL_BINARY_LOG_HEALTH_RES_WRITE_LOG_PRESSURE";
pub const REASON_SEC_VERSION_MISSING: &str = "MYSQL_BINARY_LOG_HEALTH_SEC_VERSION_MISSING";
pub const REASON_SEC_NO_BINARY_LOG_METRICS: &str = "MYSQL_BINARY_LOG_HEALTH_SEC_NO_METRICS";
pub const REASON_SEC_BINARY_LOG_REVIEW: &str = "MYSQL_BINARY_LOG_HEALTH_SEC_REVIEW";
pub const REASON_INV_STALE_DATA: &str = "MYSQL_BINARY_LOG_HEALTH_STALE_DATA";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BinaryLogHealthItem {
    pub connection_id: String,
    pub connection_name: String,
    pub server_version: Option<String>,
    pub log_bin: Option<String>,
    pub binlog_expire_logs_seconds: Option<i64>,
    pub expire_logs_days: Option<i64>,
    pub gtid_mode: Option<String>,
    pub binary_log_metric_count: usize,
    pub write_operations: i64,
    pub qps_since_start: f64,
    pub read_write_ratio: Option<f64>,
    pub collected_at: DateTime<Utc>,
}

pub fn evaluate_mysql_binary_log_health(
    items: &[BinaryLogHealthItem],
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

pub fn binary_log_health_item_from_telemetry(
    connection_id: &str,
    connection_name: &str,
    snapshot: &MySqlTelemetrySnapshot,
) -> BinaryLogHealthItem {
    BinaryLogHealthItem {
        connection_id: connection_id.to_string(),
        connection_name: connection_name.to_string(),
        server_version: snapshot.server.version.clone(),
        log_bin: snapshot.server.log_bin.clone(),
        binlog_expire_logs_seconds: snapshot.server.binlog_expire_logs_seconds,
        expire_logs_days: snapshot.server.expire_logs_days,
        gtid_mode: snapshot.server.gtid_mode.clone(),
        binary_log_metric_count: 2,
        write_operations: snapshot.workload.com_insert
            + snapshot.workload.com_update
            + snapshot.workload.com_delete,
        qps_since_start: snapshot.workload.qps_since_start,
        read_write_ratio: snapshot.workload.read_write_ratio,
        collected_at: snapshot.collected_at,
    }
}

fn evaluate_cost(item: &BinaryLogHealthItem, pillar: Pillar, findings: &mut Vec<InventoryFinding>) {
    if !has_binary_log_metrics(item) {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_NO_BINARY_LOG_METRICS,
            Severity::High,
            format!(
                "MySQL binary log health for {} has no binary log evidence",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "binary_log_metric_count": item.binary_log_metric_count,
                "recommendation": "Collect binary-log configuration and write workload evidence before changing log-retention, storage, replication, or backup spend",
            }),
        ));
    }

    if has_write_log_pressure(item) {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_WRITE_SPEND_REVIEW,
            Severity::Medium,
            format!(
                "MySQL binary log health for {} needs spend review",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "write_operations": item.write_operations,
                "qps_since_start": item.qps_since_start,
                "read_write_ratio": item.read_write_ratio,
                "recommendation": "Review binary log retention, replica fanout, PITR requirements, and write spikes before increasing storage or instance spend",
            }),
        ));
    }
}

fn evaluate_resilience(
    item: &BinaryLogHealthItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if !is_binary_logging_enabled(item) {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_BINARY_LOG_DISABLED,
            Severity::High,
            format!(
                "MySQL binary logging is not enabled for {}",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "log_bin": item.log_bin,
                "recommendation": "Enable binary logging where PITR, replication, audit, or deterministic recovery workflows depend on it",
            }),
        ));
    }

    if !has_binary_log_metrics(item) {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_NO_BINARY_LOG_METRICS,
            Severity::High,
            format!(
                "MySQL binary log health for {} has no resilience evidence",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "binary_log_metric_count": item.binary_log_metric_count,
                "recommendation": "Collect binary-log and write workload evidence so PITR, replication, and failover readiness can be evaluated deterministically",
            }),
        ));
    }

    if item.binlog_expire_logs_seconds.is_none() && item.expire_logs_days.is_none() {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_RETENTION_MISSING,
            Severity::Medium,
            format!(
                "MySQL binary log retention is not recorded for {}",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "binlog_expire_logs_seconds": item.binlog_expire_logs_seconds,
                "expire_logs_days": item.expire_logs_days,
                "recommendation": "Record binary log retention so PITR and replica recovery windows can be checked against policy",
            }),
        ));
    }

    if has_write_log_pressure(item) {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_WRITE_LOG_PRESSURE,
            Severity::High,
            format!(
                "MySQL binary log write pressure is elevated for {}",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "write_operations": item.write_operations,
                "qps_since_start": item.qps_since_start,
                "read_write_ratio": item.read_write_ratio,
                "gtid_mode": item.gtid_mode,
                "recommendation": "Validate binary log durability, retention headroom, replica apply capacity, and PITR coverage before treating write pressure as generic saturation",
            }),
        ));
    }
}

fn evaluate_security(
    item: &BinaryLogHealthItem,
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
                "MySQL binary log health for {} has no server version evidence",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "server_version": item.server_version,
                "recommendation": "Record MySQL server version with binary-log evidence so version-specific replication and audit guidance can be mapped",
            }),
        ));
    }

    if !has_binary_log_metrics(item) {
        findings.push(finding(
            item,
            pillar,
            REASON_SEC_NO_BINARY_LOG_METRICS,
            Severity::High,
            format!(
                "MySQL binary log health for {} has no scoped evidence for security review",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "binary_log_metric_count": item.binary_log_metric_count,
                "recommendation": "Collect scoped binary-log evidence so audit, PITR, and incident review do not require ad hoc privileged diagnostics",
            }),
        ));
    }

    if has_write_log_pressure(item) || !is_gtid_enabled(item) {
        findings.push(finding(
            item,
            pillar,
            REASON_SEC_BINARY_LOG_REVIEW,
            Severity::Medium,
            format!(
                "MySQL binary log evidence for {} should be reviewed before export",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "write_operations": item.write_operations,
                "qps_since_start": item.qps_since_start,
                "read_write_ratio": item.read_write_ratio,
                "gtid_mode": item.gtid_mode,
                "recommendation": "Review binary-log evidence with scoped credentials and redact workload-sensitive write metadata before sharing outside the database team",
            }),
        ));
    }
}

fn stale_finding(
    item: &BinaryLogHealthItem,
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
            "Binary log health data for {} is {} hours old (threshold {} hours)",
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
    item: &BinaryLogHealthItem,
    pillar: Pillar,
    reason_code: &str,
    severity: Severity,
    message: String,
    evidence: Value,
) -> InventoryFinding {
    InventoryFinding {
        resource_id: item.connection_id.clone(),
        arn: format!("mysql-binary-log-health:{}", item.connection_id),
        pillar,
        reason_code: reason_code.to_string(),
        severity,
        message,
        evidence,
    }
}

fn has_binary_log_metrics(item: &BinaryLogHealthItem) -> bool {
    item.binary_log_metric_count > 0
}

fn has_write_log_pressure(item: &BinaryLogHealthItem) -> bool {
    has_binary_log_metrics(item)
        && (item.write_operations >= 25_000
            || item.qps_since_start >= 80.0
            || (item.write_operations > 0
                && item
                    .read_write_ratio
                    .map(|ratio| ratio > 0.0 && ratio <= 1.0)
                    .unwrap_or(false)))
}

fn is_binary_logging_enabled(item: &BinaryLogHealthItem) -> bool {
    item.log_bin
        .as_deref()
        .map(str::trim)
        .map(|value| matches!(value.to_ascii_lowercase().as_str(), "on" | "1" | "true"))
        .unwrap_or(false)
}

fn is_gtid_enabled(item: &BinaryLogHealthItem) -> bool {
    item.gtid_mode
        .as_deref()
        .map(str::trim)
        .map(|value| value.eq_ignore_ascii_case("on"))
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    fn item() -> BinaryLogHealthItem {
        BinaryLogHealthItem {
            connection_id: "conn-1".to_string(),
            connection_name: "orders-mysql".to_string(),
            server_version: Some("8.0.36".to_string()),
            log_bin: Some("ON".to_string()),
            binlog_expire_logs_seconds: Some(604_800),
            expire_logs_days: None,
            gtid_mode: Some("ON".to_string()),
            binary_log_metric_count: 2,
            write_operations: 2_500,
            qps_since_start: 25.0,
            read_write_ratio: Some(4.0),
            collected_at: Utc::now(),
        }
    }

    #[test]
    fn cost_flags_missing_metrics_and_write_spend_review() {
        let now = Utc::now();
        let missing = BinaryLogHealthItem {
            binary_log_metric_count: 0,
            ..item()
        };
        let write_heavy = BinaryLogHealthItem {
            write_operations: 80_000,
            qps_since_start: 150.0,
            read_write_ratio: Some(0.5),
            ..item()
        };

        let report = evaluate_mysql_binary_log_health(&[missing, write_heavy], Pillar::Cost, now);

        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_COST_NO_BINARY_LOG_METRICS));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_COST_WRITE_SPEND_REVIEW));
    }

    #[test]
    fn resilience_flags_disabled_missing_metrics_missing_retention_and_write_pressure() {
        let now = Utc::now();
        let missing = BinaryLogHealthItem {
            log_bin: Some("OFF".to_string()),
            binary_log_metric_count: 0,
            binlog_expire_logs_seconds: None,
            expire_logs_days: None,
            ..item()
        };
        let write_heavy = BinaryLogHealthItem {
            write_operations: 50_000,
            qps_since_start: 120.0,
            read_write_ratio: Some(0.6),
            ..item()
        };

        let report =
            evaluate_mysql_binary_log_health(&[missing, write_heavy], Pillar::Resilience, now);

        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_RES_BINARY_LOG_DISABLED));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_RES_NO_BINARY_LOG_METRICS));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_RES_RETENTION_MISSING));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_RES_WRITE_LOG_PRESSURE));
    }

    #[test]
    fn security_flags_missing_version_missing_metrics_and_binary_log_review() {
        let now = Utc::now();
        let missing = BinaryLogHealthItem {
            server_version: None,
            binary_log_metric_count: 0,
            ..item()
        };
        let review = BinaryLogHealthItem {
            gtid_mode: Some("OFF".to_string()),
            ..item()
        };

        let report = evaluate_mysql_binary_log_health(&[missing, review], Pillar::Security, now);

        assert_eq!(report.findings.len(), 3);
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_SEC_VERSION_MISSING));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_SEC_NO_BINARY_LOG_METRICS));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_SEC_BINARY_LOG_REVIEW));
    }

    #[test]
    fn stale_binary_log_health_data_is_counted_for_any_pillar() {
        let now = Utc::now();
        let mut item = item();
        item.collected_at = now - Duration::hours(DEFAULT_STALE_AFTER_HOURS + 2);

        let report = evaluate_mysql_binary_log_health(&[item], Pillar::Resilience, now);

        assert_eq!(report.stale_resources, 1);
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_INV_STALE_DATA));
    }

    #[test]
    fn healthy_binary_log_health_passes_claimed_pillars() {
        let now = Utc::now();
        let items = vec![item()];

        for pillar in [Pillar::Cost, Pillar::Resilience, Pillar::Security] {
            let report = evaluate_mysql_binary_log_health(&items, pillar, now);
            assert_eq!(report.score, 100);
            assert!(report.findings.is_empty());
        }
    }
}

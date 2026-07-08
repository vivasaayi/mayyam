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

use chrono::{DateTime, Utc};
use serde::Serialize;
use serde_json::{json, Value};

use crate::services::analytics::mysql_analytics::mysql_telemetry::{
    MySqlFinding, MySqlFindingCategory, MySqlFindingSeverity, MySqlTelemetrySnapshot,
};
use crate::services::analytics::mysql_analytics::slow_query_log_health::{
    REASON_COST_NO_DIGEST_COVERAGE, REASON_COST_SLOW_QUERY_PRESSURE,
    REASON_RES_HIGH_PRIORITY_FINDINGS, REASON_RES_LOG_DISABLED, REASON_RES_THRESHOLD_HIGH,
};
use crate::services::aws::inventory::types::DEFAULT_STALE_AFTER_HOURS;

pub const RESOURCE_TYPE: &str = "MySqlSlowQueryAlert";
const REASON_SLOW_QUERY_DIGEST_SPIKE: &str = "MYSQL_SLOW_QUERY_ALERT_DIGEST_SPIKE";
const THRESHOLD_LONG_QUERY_TIME_SECONDS: f64 = 5.0;
const THRESHOLD_SLOW_QUERIES: i64 = 100;
const THRESHOLD_TOP_DIGEST_TOTAL_TIME_MS: f64 = 60_000.0;
const THRESHOLD_TOP_DIGEST_MAX_TIME_MS: f64 = 10_000.0;
const THRESHOLD_ROWS_EXAMINED_PER_ROW_SENT: f64 = 1_000.0;

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct MySqlSlowQueryAlert {
    pub connection_id: String,
    pub connection_name: String,
    pub status: &'static str,
    pub severity: &'static str,
    pub reason_code: String,
    pub summary: String,
    pub evidence: Value,
    pub collected_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct MySqlSlowQueryAlertWorkflow {
    pub workflow_id: &'static str,
    pub read_only_mode: bool,
    pub status: &'static str,
    pub evidence_reason_codes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct MySqlSlowQueryAlertHealth {
    pub workflow_id: &'static str,
    pub status: &'static str,
    pub active_alerts: usize,
    pub insufficient_data_alerts: usize,
    pub highest_severity: &'static str,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct MySqlSlowQueryAlertReporting {
    pub workflow_id: &'static str,
    pub report_id: &'static str,
    pub total_alerts: usize,
    pub impacted_connections: Vec<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct MySqlSlowQueryAlertBundle {
    pub resource_type: &'static str,
    pub resources_evaluated: usize,
    pub active_alerts: usize,
    pub insufficient_data_alerts: usize,
    pub alerts: Vec<MySqlSlowQueryAlert>,
    pub notification_workflow: MySqlSlowQueryAlertWorkflow,
    pub ai_triage_workflow: MySqlSlowQueryAlertWorkflow,
    pub agentic_investigation_workflow: MySqlSlowQueryAlertWorkflow,
    pub automation_workflow: MySqlSlowQueryAlertWorkflow,
    pub health_workflow: MySqlSlowQueryAlertHealth,
    pub reporting: MySqlSlowQueryAlertReporting,
}

pub fn build_mysql_slow_query_alert_bundle(
    items: &[(&str, &str, &MySqlTelemetrySnapshot)],
) -> MySqlSlowQueryAlertBundle {
    let mut alerts = Vec::new();

    for (connection_id, connection_name, snapshot) in items {
        let top_digest = snapshot
            .statements
            .iter()
            .max_by(|left, right| left.total_time_ms.total_cmp(&right.total_time_ms));
        let high_priority_findings = snapshot
            .findings
            .iter()
            .filter(|finding| {
                matches!(
                    finding.severity,
                    MySqlFindingSeverity::Critical | MySqlFindingSeverity::High
                )
            })
            .count();

        if !slow_query_log_enabled(&snapshot.server.slow_query_log_enabled) {
            alerts.push(MySqlSlowQueryAlert {
                connection_id: (*connection_id).to_string(),
                connection_name: (*connection_name).to_string(),
                status: "insufficient_data",
                severity: "high",
                reason_code: REASON_RES_LOG_DISABLED.to_string(),
                summary: format!(
                    "Slow query alerting cannot evaluate {} because slow query logging is disabled",
                    connection_name
                ),
                evidence: json!({
                    "slow_query_log_enabled": snapshot.server.slow_query_log_enabled,
                    "recommendation": "Enable slow_query_log before using Mayyam slow-query alerts for incident response",
                }),
                collected_at: snapshot.collected_at,
            });
        }

        if snapshot.statements.is_empty() {
            alerts.push(MySqlSlowQueryAlert {
                connection_id: (*connection_id).to_string(),
                connection_name: (*connection_name).to_string(),
                status: "insufficient_data",
                severity: "medium",
                reason_code: REASON_COST_NO_DIGEST_COVERAGE.to_string(),
                summary: format!(
                    "Slow query alerting for {} has no statement digest evidence",
                    connection_name
                ),
                evidence: json!({
                    "statement_digest_count": snapshot.statements.len(),
                    "recommendation": "Collect statement digest evidence so Mayyam can attribute slow-query pressure to a query family",
                }),
                collected_at: snapshot.collected_at,
            });
        }

        if snapshot
            .server
            .long_query_time_seconds
            .is_some_and(|seconds| seconds > THRESHOLD_LONG_QUERY_TIME_SECONDS)
        {
            alerts.push(MySqlSlowQueryAlert {
                connection_id: (*connection_id).to_string(),
                connection_name: (*connection_name).to_string(),
                status: "firing",
                severity: "medium",
                reason_code: REASON_RES_THRESHOLD_HIGH.to_string(),
                summary: format!(
                    "Slow query threshold for {} is too high for incident alerting",
                    connection_name
                ),
                evidence: json!({
                    "long_query_time_seconds": snapshot.server.long_query_time_seconds,
                    "recommended_max_seconds": THRESHOLD_LONG_QUERY_TIME_SECONDS,
                }),
                collected_at: snapshot.collected_at,
            });
        }

        if snapshot.workload.slow_queries >= THRESHOLD_SLOW_QUERIES
            || top_digest.is_some_and(|digest| {
                digest.total_time_ms >= THRESHOLD_TOP_DIGEST_TOTAL_TIME_MS
                    || digest.max_time_ms >= THRESHOLD_TOP_DIGEST_MAX_TIME_MS
                    || digest.rows_examined_per_row_sent.unwrap_or_default()
                        >= THRESHOLD_ROWS_EXAMINED_PER_ROW_SENT
            })
        {
            alerts.push(MySqlSlowQueryAlert {
                connection_id: (*connection_id).to_string(),
                connection_name: (*connection_name).to_string(),
                status: "firing",
                severity: "high",
                reason_code: REASON_COST_SLOW_QUERY_PRESSURE.to_string(),
                summary: format!(
                    "Slow query pressure is elevated on {}",
                    connection_name
                ),
                evidence: json!({
                    "slow_queries": snapshot.workload.slow_queries,
                    "top_digest_total_time_ms": top_digest.map(|digest| digest.total_time_ms),
                    "top_digest_max_time_ms": top_digest.map(|digest| digest.max_time_ms),
                    "top_digest_rows_examined_per_row_sent": top_digest.and_then(|digest| digest.rows_examined_per_row_sent),
                }),
                collected_at: snapshot.collected_at,
            });
        }

        if high_priority_findings > 0 {
            alerts.push(MySqlSlowQueryAlert {
                connection_id: (*connection_id).to_string(),
                connection_name: (*connection_name).to_string(),
                status: "firing",
                severity: "high",
                reason_code: REASON_RES_HIGH_PRIORITY_FINDINGS.to_string(),
                summary: format!(
                    "Slow query evidence for {} includes high-priority database findings",
                    connection_name
                ),
                evidence: json!({
                    "high_priority_findings": high_priority_findings,
                    "categories": high_priority_finding_categories(&snapshot.findings),
                }),
                collected_at: snapshot.collected_at,
            });
        }

        if top_digest.is_some_and(|digest| {
            digest.execution_count >= 25
                && digest.avg_time_ms >= 1_000.0
                && digest.max_time_ms >= 5_000.0
        }) {
            let digest = top_digest.expect("checked above");
            alerts.push(MySqlSlowQueryAlert {
                connection_id: (*connection_id).to_string(),
                connection_name: (*connection_name).to_string(),
                status: "firing",
                severity: "medium",
                reason_code: REASON_SLOW_QUERY_DIGEST_SPIKE.to_string(),
                summary: format!(
                    "A single query family is repeatedly spiking latency on {}",
                    connection_name
                ),
                evidence: json!({
                    "digest_text": digest.digest_text,
                    "execution_count": digest.execution_count,
                    "avg_time_ms": digest.avg_time_ms,
                    "max_time_ms": digest.max_time_ms,
                }),
                collected_at: snapshot.collected_at,
            });
        }
    }

    alerts.sort_by(|left, right| {
        left.connection_name
            .cmp(&right.connection_name)
            .then(left.reason_code.cmp(&right.reason_code))
    });

    let active_alerts = alerts
        .iter()
        .filter(|alert| alert.status == "firing")
        .count();
    let insufficient_data_alerts = alerts
        .iter()
        .filter(|alert| alert.status == "insufficient_data")
        .count();
    let evidence_reason_codes = alerts
        .iter()
        .map(|alert| alert.reason_code.clone())
        .collect::<Vec<_>>();
    let workflow_status = if active_alerts > 0 {
        "action_required"
    } else if insufficient_data_alerts > 0 {
        "insufficient_data"
    } else {
        "healthy"
    };

    let highest_severity = highest_severity(&alerts);
    let total_alerts = alerts.len();
    let impacted_connections = unique_connections(&alerts);

    MySqlSlowQueryAlertBundle {
        resource_type: RESOURCE_TYPE,
        resources_evaluated: items.len(),
        active_alerts,
        insufficient_data_alerts,
        alerts,
        notification_workflow: MySqlSlowQueryAlertWorkflow {
            workflow_id: "mysql_slow_query_notification",
            read_only_mode: true,
            status: workflow_status,
            evidence_reason_codes: evidence_reason_codes.clone(),
        },
        ai_triage_workflow: MySqlSlowQueryAlertWorkflow {
            workflow_id: "mysql_slow_query_ai_triage",
            read_only_mode: true,
            status: workflow_status,
            evidence_reason_codes: evidence_reason_codes.clone(),
        },
        agentic_investigation_workflow: MySqlSlowQueryAlertWorkflow {
            workflow_id: "mysql_slow_query_agentic_investigation",
            read_only_mode: true,
            status: workflow_status,
            evidence_reason_codes: evidence_reason_codes.clone(),
        },
        automation_workflow: MySqlSlowQueryAlertWorkflow {
            workflow_id: "mysql_slow_query_automation",
            read_only_mode: true,
            status: workflow_status,
            evidence_reason_codes: evidence_reason_codes.clone(),
        },
        health_workflow: MySqlSlowQueryAlertHealth {
            workflow_id: "mysql_slow_query_health",
            status: workflow_status,
            active_alerts,
            insufficient_data_alerts,
            highest_severity,
        },
        reporting: MySqlSlowQueryAlertReporting {
            workflow_id: "mysql_slow_query_reporting",
            report_id: "mysql_slow_query_alert_summary",
            total_alerts,
            impacted_connections,
        },
    }
}

pub fn stale_after_hours() -> i64 {
    DEFAULT_STALE_AFTER_HOURS
}

fn slow_query_log_enabled(value: &Option<String>) -> bool {
    value
        .as_deref()
        .map(str::trim)
        .map(|value| value.eq_ignore_ascii_case("on") || value.eq_ignore_ascii_case("1"))
        .unwrap_or(false)
}

fn high_priority_finding_categories(findings: &[MySqlFinding]) -> Vec<&'static str> {
    let mut categories = findings
        .iter()
        .filter(|finding| {
            matches!(
                finding.severity,
                MySqlFindingSeverity::Critical | MySqlFindingSeverity::High
            )
        })
        .map(|finding| match finding.category {
            MySqlFindingCategory::Workload => "workload",
            MySqlFindingCategory::Query => "query",
            MySqlFindingCategory::Index => "index",
            MySqlFindingCategory::Locking => "locking",
            MySqlFindingCategory::Connection => "connection",
            MySqlFindingCategory::InnoDb => "innodb",
            MySqlFindingCategory::Storage => "storage",
            MySqlFindingCategory::Configuration => "configuration",
        })
        .collect::<Vec<_>>();
    categories.sort_unstable();
    categories.dedup();
    categories
}

fn unique_connections(alerts: &[MySqlSlowQueryAlert]) -> Vec<String> {
    let mut connections = alerts
        .iter()
        .map(|alert| alert.connection_name.clone())
        .collect::<Vec<_>>();
    connections.sort();
    connections.dedup();
    connections
}

fn highest_severity(alerts: &[MySqlSlowQueryAlert]) -> &'static str {
    if alerts.iter().any(|alert| alert.severity == "high") {
        "high"
    } else if alerts.iter().any(|alert| alert.severity == "medium") {
        "medium"
    } else {
        "info"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::analytics::mysql_analytics::mysql_telemetry::{
        MySqlConnectionSnapshot, MySqlInnoDbSnapshot, MySqlLockSnapshot, MySqlServerContext,
        MySqlStatementDigest, MySqlWorkloadSnapshot,
    };

    #[test]
    fn slow_query_alert_bundle_reports_pressure_and_missing_evidence() {
        let snapshot = telemetry_snapshot(
            Some("OFF"),
            Some(12.0),
            180,
            vec![MySqlStatementDigest {
                digest: Some("abc".to_string()),
                schema_name: Some("app".to_string()),
                digest_text: "SELECT * FROM orders WHERE user_id = ?".to_string(),
                execution_count: 42,
                total_time_ms: 180_000.0,
                avg_time_ms: 2_400.0,
                max_time_ms: 12_500.0,
                rows_examined: 55_000,
                rows_sent: 30,
                rows_examined_per_row_sent: Some(1_833.0),
                no_index_used_count: 12,
                no_good_index_used_count: 6,
                first_seen: None,
                last_seen: None,
            }],
            vec![MySqlFinding {
                severity: MySqlFindingSeverity::High,
                category: MySqlFindingCategory::Query,
                title: "High latency digest".to_string(),
                evidence: vec!["digest avg time above threshold".to_string()],
                impact: "Requests can back up".to_string(),
                recommendation: "Tune the query plan".to_string(),
                validation_query: None,
            }],
        );

        let bundle = build_mysql_slow_query_alert_bundle(&[("conn-1", "Orders MySQL", &snapshot)]);

        assert_eq!(bundle.resource_type, RESOURCE_TYPE);
        assert_eq!(bundle.resources_evaluated, 1);
        assert_eq!(bundle.active_alerts, 4);
        assert_eq!(bundle.insufficient_data_alerts, 1);
        assert_eq!(bundle.alerts.len(), 5);
        assert_eq!(bundle.notification_workflow.status, "action_required");
        assert_eq!(bundle.health_workflow.highest_severity, "high");
        assert!(bundle
            .alerts
            .iter()
            .any(|alert| alert.reason_code == REASON_RES_LOG_DISABLED));
        assert!(bundle
            .alerts
            .iter()
            .any(|alert| alert.reason_code == REASON_COST_SLOW_QUERY_PRESSURE));
        assert!(bundle
            .alerts
            .iter()
            .any(|alert| alert.reason_code == REASON_SLOW_QUERY_DIGEST_SPIKE));
    }

    fn telemetry_snapshot(
        slow_query_log_enabled: Option<&str>,
        long_query_time_seconds: Option<f64>,
        slow_queries: i64,
        statements: Vec<MySqlStatementDigest>,
        findings: Vec<MySqlFinding>,
    ) -> MySqlTelemetrySnapshot {
        MySqlTelemetrySnapshot {
            collected_at: Utc::now(),
            server: MySqlServerContext {
                version: Some("8.0.36".to_string()),
                uptime_seconds: 86_400,
                have_ssl: Some("YES".to_string()),
                require_secure_transport: Some("ON".to_string()),
                log_bin: Some("ON".to_string()),
                binlog_expire_logs_seconds: Some(86_400),
                expire_logs_days: None,
                gtid_mode: Some("ON".to_string()),
                performance_schema_enabled: Some("ON".to_string()),
                slow_query_log_enabled: slow_query_log_enabled.map(str::to_string),
                long_query_time_seconds,
                sys_schema_available: true,
            },
            workload: MySqlWorkloadSnapshot {
                questions: 0,
                queries: 0,
                com_select: 0,
                com_insert: 0,
                com_update: 0,
                com_delete: 0,
                slow_queries,
                created_tmp_tables: 0,
                created_tmp_disk_tables: 0,
                created_tmp_files: 0,
                tmp_disk_table_pct: None,
                sort_merge_passes: 0,
                sort_range: 0,
                sort_rows: 0,
                sort_scan: 0,
                sort_merge_pass_pct: None,
                select_full_join: 0,
                select_full_range_join: 0,
                select_range_check: 0,
                full_join_select_pct: None,
                ssl_accepts: 0,
                ssl_finished_accepts: 0,
                ssl_accept_pct: None,
                qps_since_start: 0.0,
                read_write_ratio: None,
            },
            connections: MySqlConnectionSnapshot {
                max_connections: 0,
                max_used_connections: 0,
                threads_connected: 0,
                threads_running: 0,
                threads_cached: 0,
                connection_usage_pct: None,
                peak_connection_usage_pct: None,
                aborted_clients: 0,
                aborted_connects: 0,
                connection_errors: Default::default(),
            },
            innodb: MySqlInnoDbSnapshot {
                buffer_pool_hit_ratio: None,
                buffer_pool_pages_total: 0,
                buffer_pool_pages_free: 0,
                buffer_pool_pages_dirty: 0,
                buffer_pool_dirty_pct: None,
                buffer_pool_free_pct: None,
                log_waits: 0,
                row_lock_waits: 0,
                row_lock_time_ms: 0,
                deadlocks: 0,
            },
            statements,
            tables: Vec::new(),
            partitions: Vec::new(),
            indexes: Vec::new(),
            privileges: Vec::new(),
            waits: Vec::new(),
            locks: MySqlLockSnapshot {
                blocked_processes: 0,
                pending_metadata_locks: None,
                data_lock_waits: None,
            },
            findings,
        }
    }
}

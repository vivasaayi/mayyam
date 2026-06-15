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

// Deterministic MySQL slow query log health evaluator for roadmap rows
// 03-MYSQL-AI-TRIAGER-00100/00107/00128.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::services::analytics::mysql_analytics::mysql_telemetry::{
    MySqlFindingSeverity, MySqlTelemetrySnapshot,
};
use crate::services::aws::inventory::types::{
    score_pillar, InventoryFinding, Pillar, PillarReport, Severity, DEFAULT_STALE_AFTER_HOURS,
};

pub const RESOURCE_TYPE: &str = "MySqlSlowQueryLogHealth";
pub const REASON_COST_LOG_DISABLED: &str = "MYSQL_SLOW_QUERY_LOG_HEALTH_COST_DISABLED";
pub const REASON_COST_NO_DIGEST_COVERAGE: &str = "MYSQL_SLOW_QUERY_LOG_HEALTH_COST_NO_DIGESTS";
pub const REASON_COST_SLOW_QUERY_PRESSURE: &str =
    "MYSQL_SLOW_QUERY_LOG_HEALTH_COST_SLOW_QUERY_PRESSURE";
pub const REASON_RES_LOG_DISABLED: &str = "MYSQL_SLOW_QUERY_LOG_HEALTH_RES_DISABLED";
pub const REASON_RES_THRESHOLD_HIGH: &str = "MYSQL_SLOW_QUERY_LOG_HEALTH_RES_THRESHOLD_HIGH";
pub const REASON_RES_HIGH_PRIORITY_FINDINGS: &str =
    "MYSQL_SLOW_QUERY_LOG_HEALTH_RES_HIGH_PRIORITY_FINDINGS";
pub const REASON_SEC_LOG_DISABLED: &str = "MYSQL_SLOW_QUERY_LOG_HEALTH_SEC_DISABLED";
pub const REASON_SEC_VERSION_MISSING: &str = "MYSQL_SLOW_QUERY_LOG_HEALTH_SEC_VERSION_MISSING";
pub const REASON_SEC_EXCESSIVE_CAPTURE: &str = "MYSQL_SLOW_QUERY_LOG_HEALTH_SEC_EXCESSIVE_CAPTURE";
pub const REASON_INV_STALE_DATA: &str = "MYSQL_SLOW_QUERY_LOG_HEALTH_STALE_DATA";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlowQueryLogHealthItem {
    pub connection_id: String,
    pub connection_name: String,
    pub server_version: Option<String>,
    pub slow_query_log_enabled: Option<String>,
    pub long_query_time_seconds: Option<f64>,
    pub slow_queries: i64,
    pub statement_digest_count: usize,
    pub top_digest_total_time_ms: f64,
    pub top_digest_max_time_ms: f64,
    pub max_rows_examined_per_row_sent: Option<f64>,
    pub high_priority_findings: usize,
    pub collected_at: DateTime<Utc>,
}

pub fn evaluate_mysql_slow_query_log_health(
    items: &[SlowQueryLogHealthItem],
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

pub fn slow_query_log_health_item_from_telemetry(
    connection_id: &str,
    connection_name: &str,
    snapshot: &MySqlTelemetrySnapshot,
) -> SlowQueryLogHealthItem {
    let top_digest = snapshot
        .statements
        .iter()
        .max_by(|a, b| a.total_time_ms.total_cmp(&b.total_time_ms));
    let max_rows_examined_per_row_sent = snapshot
        .statements
        .iter()
        .filter_map(|digest| digest.rows_examined_per_row_sent)
        .max_by(|a, b| a.total_cmp(b));
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

    SlowQueryLogHealthItem {
        connection_id: connection_id.to_string(),
        connection_name: connection_name.to_string(),
        server_version: snapshot.server.version.clone(),
        slow_query_log_enabled: snapshot.server.slow_query_log_enabled.clone(),
        long_query_time_seconds: snapshot.server.long_query_time_seconds,
        slow_queries: snapshot.workload.slow_queries,
        statement_digest_count: snapshot.statements.len(),
        top_digest_total_time_ms: top_digest.map(|digest| digest.total_time_ms).unwrap_or(0.0),
        top_digest_max_time_ms: top_digest.map(|digest| digest.max_time_ms).unwrap_or(0.0),
        max_rows_examined_per_row_sent,
        high_priority_findings,
        collected_at: snapshot.collected_at,
    }
}

fn evaluate_cost(
    item: &SlowQueryLogHealthItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if !is_slow_query_log_enabled(item) {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_LOG_DISABLED,
            Severity::High,
            format!(
                "Slow query log health telemetry is disabled for {}",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "slow_query_log_enabled": item.slow_query_log_enabled,
                "recommendation": "Enable slow query logging before estimating query-level cost or savings opportunities",
            }),
        ));
    }

    if item.statement_digest_count == 0 {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_NO_DIGEST_COVERAGE,
            Severity::Medium,
            format!(
                "Slow query log health for {} has no statement digest coverage",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "statement_digest_count": item.statement_digest_count,
                "recommendation": "Collect normalized statement digests so slow-query cost can be attributed by query family",
            }),
        ));
    }

    if item.slow_queries >= 100
        || item.top_digest_total_time_ms >= 60_000.0
        || item.max_rows_examined_per_row_sent.unwrap_or(0.0) >= 1_000.0
    {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_SLOW_QUERY_PRESSURE,
            Severity::High,
            format!(
                "Slow query log health for {} shows expensive query pressure",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "slow_queries": item.slow_queries,
                "top_digest_total_time_ms": item.top_digest_total_time_ms,
                "max_rows_examined_per_row_sent": item.max_rows_examined_per_row_sent,
                "recommendation": "Review slow-query families before scaling compute, storage, or I/O capacity",
            }),
        ));
    }
}

fn evaluate_resilience(
    item: &SlowQueryLogHealthItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if !is_slow_query_log_enabled(item) {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_LOG_DISABLED,
            Severity::High,
            format!(
                "Slow query log health telemetry is disabled for {}",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "slow_query_log_enabled": item.slow_query_log_enabled,
                "recommendation": "Enable slow query logging so incident triage can separate query regressions from database-wide saturation",
            }),
        ));
    }

    if long_query_time_is_high(item) {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_THRESHOLD_HIGH,
            Severity::Medium,
            format!(
                "Slow query log health for {} uses a high long_query_time threshold",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "long_query_time_seconds": item.long_query_time_seconds,
                "recommended_max_seconds": 5.0,
                "recommendation": "Tune long_query_time low enough to catch latency regressions before incident windows depend on ad hoc capture",
            }),
        ));
    }

    if item.high_priority_findings > 0 {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_HIGH_PRIORITY_FINDINGS,
            Severity::Medium,
            format!(
                "Slow query log health for {} includes high-priority telemetry findings",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "high_priority_findings": item.high_priority_findings,
            }),
        ));
    }
}

fn evaluate_security(
    item: &SlowQueryLogHealthItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if !is_slow_query_log_enabled(item) {
        findings.push(finding(
            item,
            pillar,
            REASON_SEC_LOG_DISABLED,
            Severity::High,
            format!(
                "Slow query log security telemetry is disabled for {}",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "slow_query_log_enabled": item.slow_query_log_enabled,
                "recommendation": "Enable slow-query evidence with appropriate access controls before relying on query-shape diagnostics for security reviews",
            }),
        ));
    }

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
                "Slow query log health for {} has no server version evidence",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "server_version": item.server_version,
                "recommendation": "Record server version with slow-query health evidence so advisories and compatibility risks can be mapped",
            }),
        ));
    }

    if is_slow_query_log_enabled(item)
        && item
            .long_query_time_seconds
            .map(|seconds| seconds < 0.1)
            .unwrap_or(false)
    {
        findings.push(finding(
            item,
            pillar,
            REASON_SEC_EXCESSIVE_CAPTURE,
            Severity::Medium,
            format!(
                "Slow query log health for {} may capture excessive query detail",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "long_query_time_seconds": item.long_query_time_seconds,
                "recommendation": "Verify retention, access controls, and literal redaction before operating with sub-100ms slow-query capture",
            }),
        ));
    }
}

fn stale_finding(
    item: &SlowQueryLogHealthItem,
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
            "Slow query log health data for {} is {} hours old (threshold {} hours)",
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
    item: &SlowQueryLogHealthItem,
    pillar: Pillar,
    reason_code: &str,
    severity: Severity,
    message: String,
    evidence: Value,
) -> InventoryFinding {
    InventoryFinding {
        resource_id: item.connection_id.clone(),
        arn: format!("mysql-slow-query-log-health:{}", item.connection_id),
        pillar,
        reason_code: reason_code.to_string(),
        severity,
        message,
        evidence,
    }
}

fn is_slow_query_log_enabled(item: &SlowQueryLogHealthItem) -> bool {
    item.slow_query_log_enabled
        .as_deref()
        .map(str::trim)
        .map(|value| {
            value.eq_ignore_ascii_case("on")
                || value.eq_ignore_ascii_case("1")
                || value.eq_ignore_ascii_case("true")
                || value.eq_ignore_ascii_case("yes")
        })
        .unwrap_or(false)
}

fn long_query_time_is_high(item: &SlowQueryLogHealthItem) -> bool {
    item.long_query_time_seconds
        .map(|seconds| seconds > 5.0)
        .unwrap_or(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    fn item() -> SlowQueryLogHealthItem {
        SlowQueryLogHealthItem {
            connection_id: "conn-1".to_string(),
            connection_name: "prod-mysql".to_string(),
            server_version: Some("8.0.36".to_string()),
            slow_query_log_enabled: Some("ON".to_string()),
            long_query_time_seconds: Some(2.0),
            slow_queries: 2,
            statement_digest_count: 10,
            top_digest_total_time_ms: 500.0,
            top_digest_max_time_ms: 50.0,
            max_rows_examined_per_row_sent: Some(10.0),
            high_priority_findings: 0,
            collected_at: Utc::now(),
        }
    }

    #[test]
    fn cost_flags_disabled_log_missing_coverage_and_slow_pressure() {
        let now = Utc::now();
        let mut item = item();
        item.slow_query_log_enabled = Some("OFF".to_string());
        item.statement_digest_count = 0;
        item.slow_queries = 250;

        let report = evaluate_mysql_slow_query_log_health(&[item], Pillar::Cost, now);

        assert_eq!(report.resources_evaluated, 1);
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_COST_LOG_DISABLED));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_COST_NO_DIGEST_COVERAGE));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_COST_SLOW_QUERY_PRESSURE));
    }

    #[test]
    fn resilience_flags_disabled_log_high_threshold_and_high_priority_findings() {
        let now = Utc::now();
        let mut item = item();
        item.slow_query_log_enabled = None;
        item.long_query_time_seconds = Some(12.0);
        item.high_priority_findings = 2;

        let report = evaluate_mysql_slow_query_log_health(&[item], Pillar::Resilience, now);

        assert_eq!(report.findings.len(), 3);
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_RES_LOG_DISABLED));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_RES_THRESHOLD_HIGH));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_RES_HIGH_PRIORITY_FINDINGS));
    }

    #[test]
    fn security_flags_disabled_log_missing_version_and_excessive_capture() {
        let now = Utc::now();
        let mut disabled = item();
        disabled.slow_query_log_enabled = Some("OFF".to_string());
        disabled.server_version = None;

        let mut excessive = item();
        excessive.long_query_time_seconds = Some(0.05);

        let report =
            evaluate_mysql_slow_query_log_health(&[disabled, excessive], Pillar::Security, now);

        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_SEC_LOG_DISABLED));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_SEC_VERSION_MISSING));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_SEC_EXCESSIVE_CAPTURE));
    }

    #[test]
    fn stale_health_data_is_counted_for_any_pillar() {
        let now = Utc::now();
        let mut item = item();
        item.collected_at = now - Duration::hours(DEFAULT_STALE_AFTER_HOURS + 2);

        let report = evaluate_mysql_slow_query_log_health(&[item], Pillar::Cost, now);

        assert_eq!(report.stale_resources, 1);
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_INV_STALE_DATA));
    }

    #[test]
    fn healthy_slow_query_log_health_passes_claimed_pillars() {
        let now = Utc::now();
        let items = vec![item()];

        for pillar in [Pillar::Cost, Pillar::Resilience, Pillar::Security] {
            let report = evaluate_mysql_slow_query_log_health(&items, pillar, now);
            assert_eq!(report.score, 100);
            assert!(report.findings.is_empty());
        }
    }
}

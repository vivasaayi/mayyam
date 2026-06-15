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

// Deterministic Performance Schema health evaluator for roadmap rows
// 03-MYSQL-AI-TRIAGER-00002/00009/00030.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::services::analytics::mysql_analytics::mysql_telemetry::{
    MySqlFindingSeverity, MySqlTelemetrySnapshot,
};
use crate::services::aws::inventory::types::{
    score_pillar, InventoryFinding, Pillar, PillarReport, Severity, DEFAULT_STALE_AFTER_HOURS,
};

pub const RESOURCE_TYPE: &str = "MySqlPerformanceSchemaHealth";
pub const REASON_COST_TELEMETRY_DISABLED: &str = "MYSQL_PERF_SCHEMA_HEALTH_COST_DISABLED";
pub const REASON_COST_EXPENSIVE_DIGEST: &str = "MYSQL_PERF_SCHEMA_HEALTH_COST_EXPENSIVE_DIGEST";
pub const REASON_COST_ROWS_EXAMINED_WASTE: &str =
    "MYSQL_PERF_SCHEMA_HEALTH_COST_ROWS_EXAMINED_WASTE";
pub const REASON_RES_TELEMETRY_DISABLED: &str = "MYSQL_PERF_SCHEMA_HEALTH_RES_DISABLED";
pub const REASON_RES_WAIT_PRESSURE: &str = "MYSQL_PERF_SCHEMA_HEALTH_RES_WAIT_PRESSURE";
pub const REASON_RES_HIGH_PRIORITY_FINDINGS: &str =
    "MYSQL_PERF_SCHEMA_HEALTH_RES_HIGH_PRIORITY_FINDINGS";
pub const REASON_SEC_TELEMETRY_DISABLED: &str = "MYSQL_PERF_SCHEMA_HEALTH_SEC_DISABLED";
pub const REASON_SEC_VERSION_MISSING: &str = "MYSQL_PERF_SCHEMA_HEALTH_SEC_VERSION_MISSING";
pub const REASON_INV_STALE_DATA: &str = "MYSQL_PERF_SCHEMA_HEALTH_STALE_DATA";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceSchemaHealthItem {
    pub connection_id: String,
    pub connection_name: String,
    pub server_version: Option<String>,
    pub performance_schema_enabled: Option<String>,
    pub statement_digest_count: usize,
    pub top_digest_total_time_ms: f64,
    pub top_digest_max_time_ms: f64,
    pub max_rows_examined_per_row_sent: Option<f64>,
    pub top_wait_event: Option<String>,
    pub top_wait_total_ms: f64,
    pub top_wait_avg_ms: f64,
    pub high_priority_findings: usize,
    pub collected_at: DateTime<Utc>,
}

pub fn evaluate_mysql_performance_schema_health(
    items: &[PerformanceSchemaHealthItem],
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

pub fn performance_schema_health_item_from_telemetry(
    connection_id: &str,
    connection_name: &str,
    snapshot: &MySqlTelemetrySnapshot,
) -> PerformanceSchemaHealthItem {
    let top_digest = snapshot
        .statements
        .iter()
        .max_by(|a, b| a.total_time_ms.total_cmp(&b.total_time_ms));
    let max_rows_examined_per_row_sent = snapshot
        .statements
        .iter()
        .filter_map(|digest| digest.rows_examined_per_row_sent)
        .max_by(|a, b| a.total_cmp(b));
    let top_wait = snapshot
        .waits
        .iter()
        .max_by(|a, b| a.total_wait_ms.total_cmp(&b.total_wait_ms));
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

    PerformanceSchemaHealthItem {
        connection_id: connection_id.to_string(),
        connection_name: connection_name.to_string(),
        server_version: snapshot.server.version.clone(),
        performance_schema_enabled: snapshot.server.performance_schema_enabled.clone(),
        statement_digest_count: snapshot.statements.len(),
        top_digest_total_time_ms: top_digest.map(|digest| digest.total_time_ms).unwrap_or(0.0),
        top_digest_max_time_ms: top_digest.map(|digest| digest.max_time_ms).unwrap_or(0.0),
        max_rows_examined_per_row_sent,
        top_wait_event: top_wait.map(|wait| wait.event_name.clone()),
        top_wait_total_ms: top_wait.map(|wait| wait.total_wait_ms).unwrap_or(0.0),
        top_wait_avg_ms: top_wait.map(|wait| wait.avg_wait_ms).unwrap_or(0.0),
        high_priority_findings,
        collected_at: snapshot.collected_at,
    }
}

fn evaluate_cost(
    item: &PerformanceSchemaHealthItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if !is_performance_schema_enabled(item) {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_TELEMETRY_DISABLED,
            Severity::High,
            format!(
                "Performance Schema health telemetry is disabled for {}",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "performance_schema_enabled": item.performance_schema_enabled,
                "recommendation": "Enable Performance Schema before estimating query-level cost or savings opportunities",
            }),
        ));
    }

    if item.top_digest_total_time_ms >= 60_000.0 || item.top_digest_max_time_ms >= 5_000.0 {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_EXPENSIVE_DIGEST,
            Severity::High,
            format!(
                "Performance Schema health for {} shows an expensive statement digest",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "top_digest_total_time_ms": item.top_digest_total_time_ms,
                "top_digest_max_time_ms": item.top_digest_max_time_ms,
                "recommendation": "Review the highest-cost digest before recommending instance or storage spend changes",
            }),
        ));
    }

    if item.max_rows_examined_per_row_sent.unwrap_or(0.0) >= 1_000.0 {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_ROWS_EXAMINED_WASTE,
            Severity::Medium,
            format!(
                "Performance Schema health for {} shows high rows-examined waste",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "max_rows_examined_per_row_sent": item.max_rows_examined_per_row_sent,
                "recommendation": "Use digest evidence to quantify wasted CPU and I/O before capacity recommendations",
            }),
        ));
    }
}

fn evaluate_resilience(
    item: &PerformanceSchemaHealthItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if !is_performance_schema_enabled(item) {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_TELEMETRY_DISABLED,
            Severity::High,
            format!(
                "Performance Schema health telemetry is disabled for {}",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "performance_schema_enabled": item.performance_schema_enabled,
                "recommendation": "Enable wait and statement summaries so incident triage can separate lock, I/O, and query symptoms",
            }),
        ));
    }

    if item.top_wait_total_ms >= 30_000.0 || item.top_wait_avg_ms >= 250.0 {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_WAIT_PRESSURE,
            Severity::High,
            format!(
                "Performance Schema health for {} shows wait-event pressure",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "top_wait_event": item.top_wait_event,
                "top_wait_total_ms": item.top_wait_total_ms,
                "top_wait_avg_ms": item.top_wait_avg_ms,
                "recommendation": "Investigate the dominant wait event before failover or automated remediation decisions",
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
                "Performance Schema health for {} includes high-priority telemetry findings",
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
    item: &PerformanceSchemaHealthItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if !is_performance_schema_enabled(item) {
        findings.push(finding(
            item,
            pillar,
            REASON_SEC_TELEMETRY_DISABLED,
            Severity::High,
            format!(
                "Performance Schema security telemetry is disabled for {}",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "performance_schema_enabled": item.performance_schema_enabled,
                "recommendation": "Enable Performance Schema evidence so security-sensitive query and privilege diagnostics are auditable",
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
                "Performance Schema health for {} has no server version evidence",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "server_version": item.server_version,
                "recommendation": "Record server version with health evidence so advisories and compatibility risks can be mapped",
            }),
        ));
    }
}

fn stale_finding(
    item: &PerformanceSchemaHealthItem,
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
            "Performance Schema health data for {} is {} hours old (threshold {} hours)",
            item.connection_name, age_hours, DEFAULT_STALE_AFTER_HOURS
        ),
        json!({
            "collected_at": item.collected_at,
            "age_hours": age_hours,
            "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
        }),
    ))
}

fn finding(
    item: &PerformanceSchemaHealthItem,
    pillar: Pillar,
    reason_code: &str,
    severity: Severity,
    message: String,
    evidence: Value,
) -> InventoryFinding {
    InventoryFinding {
        resource_id: item.connection_id.clone(),
        arn: format!("mysql-performance-schema-health:{}", item.connection_id),
        pillar,
        reason_code: reason_code.to_string(),
        severity,
        message,
        evidence,
    }
}

fn is_performance_schema_enabled(item: &PerformanceSchemaHealthItem) -> bool {
    item.performance_schema_enabled
        .as_deref()
        .map(|value| {
            let normalized = value.trim().to_ascii_lowercase();
            normalized == "on" || normalized == "1" || normalized == "yes" || normalized == "true"
        })
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    fn item() -> PerformanceSchemaHealthItem {
        PerformanceSchemaHealthItem {
            connection_id: "conn-1".to_string(),
            connection_name: "prod-mysql".to_string(),
            server_version: Some("8.0.36".to_string()),
            performance_schema_enabled: Some("ON".to_string()),
            statement_digest_count: 4,
            top_digest_total_time_ms: 1_000.0,
            top_digest_max_time_ms: 100.0,
            max_rows_examined_per_row_sent: Some(10.0),
            top_wait_event: Some("wait/io/table/sql/handler".to_string()),
            top_wait_total_ms: 100.0,
            top_wait_avg_ms: 1.0,
            high_priority_findings: 0,
            collected_at: Utc::now(),
        }
    }

    #[test]
    fn cost_flags_disabled_telemetry_and_expensive_digest() {
        let now = Utc::now();
        let mut item = item();
        item.performance_schema_enabled = Some("OFF".to_string());
        item.top_digest_total_time_ms = 90_000.0;
        item.max_rows_examined_per_row_sent = Some(2_000.0);

        let report = evaluate_mysql_performance_schema_health(&[item], Pillar::Cost, now);

        assert_eq!(report.resources_evaluated, 1);
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_COST_TELEMETRY_DISABLED));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_COST_EXPENSIVE_DIGEST));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_COST_ROWS_EXAMINED_WASTE));
    }

    #[test]
    fn resilience_flags_wait_pressure_and_high_priority_findings() {
        let now = Utc::now();
        let mut item = item();
        item.top_wait_total_ms = 60_000.0;
        item.top_wait_avg_ms = 500.0;
        item.high_priority_findings = 2;

        let report = evaluate_mysql_performance_schema_health(&[item], Pillar::Resilience, now);

        assert_eq!(report.findings.len(), 2);
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_RES_WAIT_PRESSURE));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_RES_HIGH_PRIORITY_FINDINGS));
    }

    #[test]
    fn security_flags_disabled_telemetry_and_missing_version() {
        let now = Utc::now();
        let mut item = item();
        item.performance_schema_enabled = None;
        item.server_version = None;

        let report = evaluate_mysql_performance_schema_health(&[item], Pillar::Security, now);

        assert_eq!(report.findings.len(), 2);
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_SEC_TELEMETRY_DISABLED));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_SEC_VERSION_MISSING));
    }

    #[test]
    fn stale_health_data_is_counted_for_any_pillar() {
        let now = Utc::now();
        let mut item = item();
        item.collected_at = now - Duration::hours(DEFAULT_STALE_AFTER_HOURS + 2);

        let report = evaluate_mysql_performance_schema_health(&[item], Pillar::Cost, now);

        assert_eq!(report.stale_resources, 1);
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_INV_STALE_DATA));
    }

    #[test]
    fn healthy_performance_schema_health_passes_claimed_pillars() {
        let now = Utc::now();
        let items = vec![item()];

        for pillar in [Pillar::Cost, Pillar::Resilience, Pillar::Security] {
            let report = evaluate_mysql_performance_schema_health(&items, pillar, now);
            assert_eq!(report.score, 100);
            assert!(report.findings.is_empty());
        }
    }
}

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

// Deterministic MySQL digest statistics health evaluator for roadmap rows
// 03-MYSQL-AI-TRIAGER-00149/00156/00177.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::services::analytics::mysql_analytics::mysql_telemetry::{
    MySqlFindingSeverity, MySqlStatementDigest, MySqlTelemetrySnapshot,
};
use crate::services::aws::inventory::types::{
    score_pillar, InventoryFinding, Pillar, PillarReport, Severity, DEFAULT_STALE_AFTER_HOURS,
};

pub const RESOURCE_TYPE: &str = "MySqlDigestStatisticsHealth";
pub const REASON_COST_NO_DIGEST_COVERAGE: &str =
    "MYSQL_DIGEST_STATS_HEALTH_COST_NO_DIGEST_COVERAGE";
pub const REASON_COST_EXPENSIVE_DIGESTS: &str = "MYSQL_DIGEST_STATS_HEALTH_COST_EXPENSIVE_DIGESTS";
pub const REASON_COST_ROWS_EXAMINED_WASTE: &str =
    "MYSQL_DIGEST_STATS_HEALTH_COST_ROWS_EXAMINED_WASTE";
pub const REASON_RES_NO_DIGEST_COVERAGE: &str = "MYSQL_DIGEST_STATS_HEALTH_RES_NO_DIGEST_COVERAGE";
pub const REASON_RES_LATENCY_OUTLIERS: &str = "MYSQL_DIGEST_STATS_HEALTH_RES_LATENCY_OUTLIERS";
pub const REASON_RES_HIGH_PRIORITY_FINDINGS: &str =
    "MYSQL_DIGEST_STATS_HEALTH_RES_HIGH_PRIORITY_FINDINGS";
pub const REASON_SEC_VERSION_MISSING: &str = "MYSQL_DIGEST_STATS_HEALTH_SEC_VERSION_MISSING";
pub const REASON_SEC_NO_DIGEST_COVERAGE: &str = "MYSQL_DIGEST_STATS_HEALTH_SEC_NO_DIGEST_COVERAGE";
pub const REASON_SEC_UNNORMALIZED_TEXT: &str = "MYSQL_DIGEST_STATS_HEALTH_SEC_UNNORMALIZED_TEXT";
pub const REASON_INV_STALE_DATA: &str = "MYSQL_DIGEST_STATS_HEALTH_STALE_DATA";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DigestStatisticsHealthItem {
    pub connection_id: String,
    pub connection_name: String,
    pub server_version: Option<String>,
    pub digest_count: usize,
    pub total_execution_count: i64,
    pub total_time_ms: f64,
    pub max_avg_time_ms: f64,
    pub max_time_ms: f64,
    pub max_rows_examined_per_row_sent: Option<f64>,
    pub no_index_digest_count: usize,
    pub full_scan_digest_count: usize,
    pub unnormalized_digest_count: usize,
    pub high_priority_findings: usize,
    pub collected_at: DateTime<Utc>,
}

pub fn evaluate_mysql_digest_statistics_health(
    items: &[DigestStatisticsHealthItem],
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

pub fn digest_statistics_health_item_from_telemetry(
    connection_id: &str,
    connection_name: &str,
    snapshot: &MySqlTelemetrySnapshot,
) -> DigestStatisticsHealthItem {
    let total_execution_count = snapshot
        .statements
        .iter()
        .map(|digest| digest.execution_count)
        .sum();
    let total_time_ms = snapshot
        .statements
        .iter()
        .map(|digest| digest.total_time_ms)
        .sum();
    let max_avg_time_ms = snapshot
        .statements
        .iter()
        .map(|digest| digest.avg_time_ms)
        .fold(0.0, f64::max);
    let max_time_ms = snapshot
        .statements
        .iter()
        .map(|digest| digest.max_time_ms)
        .fold(0.0, f64::max);
    let max_rows_examined_per_row_sent = snapshot
        .statements
        .iter()
        .filter_map(|digest| digest.rows_examined_per_row_sent)
        .max_by(|a, b| a.total_cmp(b));
    let no_index_digest_count = snapshot
        .statements
        .iter()
        .filter(|digest| digest.no_index_used_count > 0 || digest.no_good_index_used_count > 0)
        .count();
    let full_scan_digest_count = snapshot
        .statements
        .iter()
        .filter(|digest| is_full_scan_digest(digest))
        .count();
    let unnormalized_digest_count = snapshot
        .statements
        .iter()
        .filter(|digest| has_unnormalized_digest_text(&digest.digest_text))
        .count();
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

    DigestStatisticsHealthItem {
        connection_id: connection_id.to_string(),
        connection_name: connection_name.to_string(),
        server_version: snapshot.server.version.clone(),
        digest_count: snapshot.statements.len(),
        total_execution_count,
        total_time_ms,
        max_avg_time_ms,
        max_time_ms,
        max_rows_examined_per_row_sent,
        no_index_digest_count,
        full_scan_digest_count,
        unnormalized_digest_count,
        high_priority_findings,
        collected_at: snapshot.collected_at,
    }
}

fn evaluate_cost(
    item: &DigestStatisticsHealthItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if item.digest_count == 0 {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_NO_DIGEST_COVERAGE,
            Severity::High,
            format!(
                "Digest statistics health for {} has no statement digest coverage",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "digest_count": item.digest_count,
                "total_execution_count": item.total_execution_count,
                "recommendation": "Collect statement digest statistics before estimating query-level cost or savings opportunities",
            }),
        ));
    }

    if item.total_time_ms >= 60_000.0
        || item.no_index_digest_count > 0
        || item.full_scan_digest_count > 0
    {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_EXPENSIVE_DIGESTS,
            Severity::High,
            format!(
                "Digest statistics health for {} shows expensive query families",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "total_time_ms": item.total_time_ms,
                "no_index_digest_count": item.no_index_digest_count,
                "full_scan_digest_count": item.full_scan_digest_count,
                "recommendation": "Prioritize the highest-cost digest families before scaling compute or storage",
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
                "Digest statistics health for {} shows high rows-examined waste",
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
    item: &DigestStatisticsHealthItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if item.digest_count == 0 {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_NO_DIGEST_COVERAGE,
            Severity::High,
            format!(
                "Digest statistics health for {} has no query-family evidence",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "digest_count": item.digest_count,
                "recommendation": "Collect digest statistics so incident response can separate query regressions from database-wide saturation",
            }),
        ));
    }

    if item.max_avg_time_ms >= 1_000.0 || item.max_time_ms >= 5_000.0 {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_LATENCY_OUTLIERS,
            Severity::High,
            format!(
                "Digest statistics health for {} shows latency outliers",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "max_avg_time_ms": item.max_avg_time_ms,
                "max_time_ms": item.max_time_ms,
                "recommendation": "Investigate high-latency digest families before failover, memory, or index changes",
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
                "Digest statistics health for {} includes high-priority telemetry findings",
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
    item: &DigestStatisticsHealthItem,
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
                "Digest statistics health for {} has no server version evidence",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "server_version": item.server_version,
                "recommendation": "Record server version with digest health evidence so advisories and compatibility risks can be mapped",
            }),
        ));
    }

    if item.digest_count == 0 {
        findings.push(finding(
            item,
            pillar,
            REASON_SEC_NO_DIGEST_COVERAGE,
            Severity::High,
            format!(
                "Digest statistics health for {} has no normalized statement evidence",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "digest_count": item.digest_count,
                "recommendation": "Collect normalized statement digest evidence before relying on query-shape diagnostics for security reviews",
            }),
        ));
    }

    if item.unnormalized_digest_count > 0 {
        findings.push(finding(
            item,
            pillar,
            REASON_SEC_UNNORMALIZED_TEXT,
            Severity::Medium,
            format!(
                "Digest statistics health for {} may include unnormalized query text",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "unnormalized_digest_count": item.unnormalized_digest_count,
                "recommendation": "Verify digest text is normalized before storing or exporting query evidence",
            }),
        ));
    }
}

fn stale_finding(
    item: &DigestStatisticsHealthItem,
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
            "Digest statistics health data for {} is {} hours old (threshold {} hours)",
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
    item: &DigestStatisticsHealthItem,
    pillar: Pillar,
    reason_code: &str,
    severity: Severity,
    message: String,
    evidence: Value,
) -> InventoryFinding {
    InventoryFinding {
        resource_id: item.connection_id.clone(),
        arn: format!("mysql-digest-statistics-health:{}", item.connection_id),
        pillar,
        reason_code: reason_code.to_string(),
        severity,
        message,
        evidence,
    }
}

fn is_full_scan_digest(digest: &MySqlStatementDigest) -> bool {
    digest.rows_examined >= 100_000
        && digest
            .rows_examined_per_row_sent
            .map(|ratio| ratio >= 100.0)
            .unwrap_or(false)
}

fn has_unnormalized_digest_text(text: &str) -> bool {
    text.chars().any(|ch| ch == '\'' || ch == '"')
        || text
            .split_whitespace()
            .any(|token| token.chars().any(|ch| ch.is_ascii_digit()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    fn item() -> DigestStatisticsHealthItem {
        DigestStatisticsHealthItem {
            connection_id: "conn-1".to_string(),
            connection_name: "prod-mysql".to_string(),
            server_version: Some("8.0.36".to_string()),
            digest_count: 5,
            total_execution_count: 1_000,
            total_time_ms: 1_000.0,
            max_avg_time_ms: 10.0,
            max_time_ms: 100.0,
            max_rows_examined_per_row_sent: Some(10.0),
            no_index_digest_count: 0,
            full_scan_digest_count: 0,
            unnormalized_digest_count: 0,
            high_priority_findings: 0,
            collected_at: Utc::now(),
        }
    }

    #[test]
    fn cost_flags_missing_coverage_expensive_digests_and_rows_waste() {
        let now = Utc::now();
        let mut item = item();
        item.digest_count = 0;
        item.total_time_ms = 90_000.0;
        item.no_index_digest_count = 2;
        item.max_rows_examined_per_row_sent = Some(2_000.0);

        let report = evaluate_mysql_digest_statistics_health(&[item], Pillar::Cost, now);

        assert_eq!(report.resources_evaluated, 1);
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_COST_NO_DIGEST_COVERAGE));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_COST_EXPENSIVE_DIGESTS));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_COST_ROWS_EXAMINED_WASTE));
    }

    #[test]
    fn resilience_flags_missing_coverage_latency_and_high_priority_findings() {
        let now = Utc::now();
        let mut item = item();
        item.digest_count = 0;
        item.max_avg_time_ms = 2_000.0;
        item.max_time_ms = 10_000.0;
        item.high_priority_findings = 3;

        let report = evaluate_mysql_digest_statistics_health(&[item], Pillar::Resilience, now);

        assert_eq!(report.findings.len(), 3);
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_RES_NO_DIGEST_COVERAGE));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_RES_LATENCY_OUTLIERS));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_RES_HIGH_PRIORITY_FINDINGS));
    }

    #[test]
    fn security_flags_missing_version_missing_coverage_and_unnormalized_text() {
        let now = Utc::now();
        let mut item = item();
        item.server_version = None;
        item.digest_count = 0;
        item.unnormalized_digest_count = 2;

        let report = evaluate_mysql_digest_statistics_health(&[item], Pillar::Security, now);

        assert_eq!(report.findings.len(), 3);
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_SEC_VERSION_MISSING));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_SEC_NO_DIGEST_COVERAGE));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_SEC_UNNORMALIZED_TEXT));
    }

    #[test]
    fn stale_health_data_is_counted_for_any_pillar() {
        let now = Utc::now();
        let mut item = item();
        item.collected_at = now - Duration::hours(DEFAULT_STALE_AFTER_HOURS + 2);

        let report = evaluate_mysql_digest_statistics_health(&[item], Pillar::Cost, now);

        assert_eq!(report.stale_resources, 1);
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_INV_STALE_DATA));
    }

    #[test]
    fn healthy_digest_statistics_health_passes_claimed_pillars() {
        let now = Utc::now();
        let items = vec![item()];

        for pillar in [Pillar::Cost, Pillar::Resilience, Pillar::Security] {
            let report = evaluate_mysql_digest_statistics_health(&items, pillar, now);
            assert_eq!(report.score, 100);
            assert!(report.findings.is_empty());
        }
    }
}

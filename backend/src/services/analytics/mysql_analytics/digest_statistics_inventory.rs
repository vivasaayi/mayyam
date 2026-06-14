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

// Deterministic MySQL digest statistics inventory evaluator for roadmap rows
// 03-MYSQL-AI-TRIAGER-00148/00155/00176.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::services::analytics::mysql_analytics::mysql_telemetry::{
    MySqlStatementDigest, MySqlTelemetrySnapshot,
};
use crate::services::aws::inventory::types::{
    score_pillar, InventoryFinding, Pillar, PillarReport, Severity, COST_ALLOCATION_TAG_KEYS,
    DEFAULT_STALE_AFTER_HOURS,
};

pub const RESOURCE_TYPE: &str = "MySqlDigestStatistics";
pub const REASON_COST_OWNER_NOT_RECORDED: &str = "MYSQL_DIGEST_STATS_COST_OWNER_NOT_RECORDED";
pub const REASON_COST_NO_DIGEST_COVERAGE: &str = "MYSQL_DIGEST_STATS_COST_NO_DIGEST_COVERAGE";
pub const REASON_COST_EXPENSIVE_DIGESTS: &str = "MYSQL_DIGEST_STATS_COST_EXPENSIVE_DIGESTS";
pub const REASON_RES_NO_DIGEST_COVERAGE: &str = "MYSQL_DIGEST_STATS_RES_NO_DIGEST_COVERAGE";
pub const REASON_RES_LATENCY_OUTLIERS: &str = "MYSQL_DIGEST_STATS_RES_LATENCY_OUTLIERS";
pub const REASON_SEC_VERSION_NOT_RECORDED: &str = "MYSQL_DIGEST_STATS_SEC_VERSION_NOT_RECORDED";
pub const REASON_SEC_NO_DIGEST_COVERAGE: &str = "MYSQL_DIGEST_STATS_SEC_NO_DIGEST_COVERAGE";
pub const REASON_SEC_UNNORMALIZED_TEXT: &str = "MYSQL_DIGEST_STATS_SEC_UNNORMALIZED_TEXT";
pub const REASON_INV_STALE_DATA: &str = "MYSQL_DIGEST_STATS_INV_STALE_DATA";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DigestStatisticsInventoryItem {
    pub connection_id: String,
    pub connection_name: String,
    pub owner: Option<String>,
    pub labels: BTreeMap<String, String>,
    pub server_version: Option<String>,
    pub digest_count: usize,
    pub total_execution_count: i64,
    pub total_time_ms: f64,
    pub max_avg_time_ms: f64,
    pub max_rows_examined_per_row_sent: Option<f64>,
    pub no_index_digest_count: usize,
    pub full_scan_digest_count: usize,
    pub unnormalized_digest_count: usize,
    pub collected_at: DateTime<Utc>,
}

pub fn evaluate_mysql_digest_statistics_inventory(
    items: &[DigestStatisticsInventoryItem],
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

pub fn digest_statistics_item_from_telemetry(
    connection_id: &str,
    connection_name: &str,
    owner: Option<String>,
    labels: BTreeMap<String, String>,
    snapshot: &MySqlTelemetrySnapshot,
) -> DigestStatisticsInventoryItem {
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
    let max_rows_examined_per_row_sent = snapshot
        .statements
        .iter()
        .filter_map(|digest| digest.rows_examined_per_row_sent)
        .fold(None, |current, value| {
            Some(current.map_or(value, |existing: f64| existing.max(value)))
        });
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

    DigestStatisticsInventoryItem {
        connection_id: connection_id.to_string(),
        connection_name: connection_name.to_string(),
        owner,
        labels,
        server_version: snapshot.server.version.clone(),
        digest_count: snapshot.statements.len(),
        total_execution_count,
        total_time_ms,
        max_avg_time_ms,
        max_rows_examined_per_row_sent,
        no_index_digest_count,
        full_scan_digest_count,
        unnormalized_digest_count,
        collected_at: snapshot.collected_at,
    }
}

fn evaluate_cost(
    item: &DigestStatisticsInventoryItem,
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
                "MySQL digest statistics inventory for connection {} has no owner, team, project, or cost-center metadata",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "connection_name": item.connection_name,
                "owner": item.owner,
                "checked_keys": COST_ALLOCATION_TAG_KEYS,
                "checked_locations": ["owner", "labels"],
            }),
        ));
    }

    if item.digest_count == 0 {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_NO_DIGEST_COVERAGE,
            Severity::High,
            format!(
                "MySQL digest statistics inventory for connection {} has no statement digest coverage",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "connection_name": item.connection_name,
                "digest_count": item.digest_count,
                "total_execution_count": item.total_execution_count,
                "recommendation": "Collect performance_schema statement digest statistics so expensive SQL families can be attributed before cost recommendations are made",
            }),
        ));
    }

    if has_expensive_digest_evidence(item) {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_EXPENSIVE_DIGESTS,
            Severity::Medium,
            format!(
                "MySQL digest statistics show expensive or wasteful query families for connection {}",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "connection_name": item.connection_name,
                "total_time_ms": item.total_time_ms,
                "max_rows_examined_per_row_sent": item.max_rows_examined_per_row_sent,
                "no_index_digest_count": item.no_index_digest_count,
                "full_scan_digest_count": item.full_scan_digest_count,
                "recommendation": "Prioritize top digest families by total time, rows examined, and missing-index counters before scaling database capacity",
            }),
        ));
    }
}

fn evaluate_resilience(
    item: &DigestStatisticsInventoryItem,
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
                "MySQL digest statistics inventory for connection {} has no query-family evidence for resilience triage",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "connection_name": item.connection_name,
                "digest_count": item.digest_count,
                "recommendation": "Collect statement digest statistics so latency incidents can be separated between isolated query regressions and database-wide saturation",
            }),
        ));
    }

    if item.max_avg_time_ms >= 1_000.0 {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_LATENCY_OUTLIERS,
            Severity::High,
            format!(
                "MySQL digest statistics show latency outliers for connection {}",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "connection_name": item.connection_name,
                "max_avg_time_ms": item.max_avg_time_ms,
                "threshold_ms": 1000.0,
                "recommendation": "Investigate high-average-time digest families and verify whether they align with incident windows before changing capacity or failover posture",
            }),
        ));
    }
}

fn evaluate_security(
    item: &DigestStatisticsInventoryItem,
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
            REASON_SEC_VERSION_NOT_RECORDED,
            Severity::Medium,
            format!(
                "MySQL digest statistics inventory for connection {} has no recorded server version",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "connection_name": item.connection_name,
                "server_version": item.server_version,
                "recommendation": "Record MySQL server version with digest statistics so version-specific security guidance can be mapped deterministically",
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
                "MySQL digest statistics inventory for connection {} has no normalized statement digest evidence",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "connection_name": item.connection_name,
                "digest_count": item.digest_count,
                "recommendation": "Collect normalized statement digests so risky query shapes can be reviewed without exposing raw SQL literals",
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
                "MySQL digest statistics for connection {} include digest text that may not be normalized",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "connection_name": item.connection_name,
                "unnormalized_digest_count": item.unnormalized_digest_count,
                "recommendation": "Verify digest text is normalized before storing or exporting query evidence, and avoid capturing literal SQL values in inventory records",
            }),
        ));
    }
}

fn stale_finding(
    item: &DigestStatisticsInventoryItem,
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
            "Inventory data for MySQL digest statistics connection {} is {} hours old (threshold {} hours)",
            item.connection_name, age_hours, DEFAULT_STALE_AFTER_HOURS
        ),
        json!({
            "connection_id": item.connection_id,
            "connection_name": item.connection_name,
            "collected_at": item.collected_at,
            "age_hours": age_hours,
            "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
        }),
    ))
}

fn finding(
    item: &DigestStatisticsInventoryItem,
    pillar: Pillar,
    reason_code: &str,
    severity: Severity,
    message: String,
    evidence: Value,
) -> InventoryFinding {
    InventoryFinding {
        resource_id: item.connection_id.clone(),
        arn: format!("mysql://digest-statistics/{}", item.connection_id),
        pillar,
        reason_code: reason_code.to_string(),
        severity,
        message,
        evidence,
    }
}

fn has_owner_metadata(item: &DigestStatisticsInventoryItem) -> bool {
    item.owner
        .as_deref()
        .map(str::trim)
        .filter(|owner| !owner.is_empty())
        .is_some()
        || has_any_metadata_key(&item.labels, COST_ALLOCATION_TAG_KEYS)
}

fn has_any_metadata_key(metadata: &BTreeMap<String, String>, wanted_keys: &[&str]) -> bool {
    wanted_keys
        .iter()
        .any(|wanted| metadata_value(metadata, wanted).is_some())
}

fn metadata_value(metadata: &BTreeMap<String, String>, wanted_key: &str) -> Option<String> {
    metadata
        .iter()
        .find(|(key, value)| key.eq_ignore_ascii_case(wanted_key) && !value.trim().is_empty())
        .map(|(_, value)| value.clone())
}

fn has_expensive_digest_evidence(item: &DigestStatisticsInventoryItem) -> bool {
    item.total_time_ms >= 60_000.0
        || item
            .max_rows_examined_per_row_sent
            .map(|ratio| ratio >= 1_000.0)
            .unwrap_or(false)
        || item.no_index_digest_count > 0
        || item.full_scan_digest_count > 0
}

fn is_full_scan_digest(digest: &MySqlStatementDigest) -> bool {
    digest.rows_examined >= 100_000
        && digest
            .rows_examined_per_row_sent
            .map(|ratio| ratio >= 1_000.0)
            .unwrap_or(false)
}

fn has_unnormalized_digest_text(text: &str) -> bool {
    text.contains('\'') || text.contains('"')
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::aws::inventory::types::Pillar;
    use chrono::Duration;

    fn now() -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2026-06-12T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc)
    }

    fn labels(values: &[(&str, &str)]) -> BTreeMap<String, String> {
        values
            .iter()
            .map(|(key, value)| ((*key).to_string(), (*value).to_string()))
            .collect()
    }

    fn item(
        owner: Option<&str>,
        server_version: Option<&str>,
        digest_count: usize,
        total_time_ms: f64,
        max_avg_time_ms: f64,
        max_rows_examined_per_row_sent: Option<f64>,
        no_index_digest_count: usize,
        full_scan_digest_count: usize,
        unnormalized_digest_count: usize,
        labels: BTreeMap<String, String>,
        collected_at: DateTime<Utc>,
    ) -> DigestStatisticsInventoryItem {
        DigestStatisticsInventoryItem {
            connection_id: "mysql-1".to_string(),
            connection_name: "orders-mysql".to_string(),
            owner: owner.map(str::to_string),
            labels,
            server_version: server_version.map(str::to_string),
            digest_count,
            total_execution_count: 500,
            total_time_ms,
            max_avg_time_ms,
            max_rows_examined_per_row_sent,
            no_index_digest_count,
            full_scan_digest_count,
            unnormalized_digest_count,
            collected_at,
        }
    }

    fn healthy_item() -> DigestStatisticsInventoryItem {
        item(
            Some("database-platform"),
            Some("8.0.36"),
            12,
            20_000.0,
            125.0,
            Some(20.0),
            0,
            0,
            0,
            labels(&[("cost-center", "cc-42")]),
            now(),
        )
    }

    fn reason_codes(report: &PillarReport) -> Vec<&str> {
        report
            .findings
            .iter()
            .map(|finding| finding.reason_code.as_str())
            .collect()
    }

    #[test]
    fn cost_flags_missing_owner_missing_coverage_and_expensive_digests() {
        let target = item(
            Some(""),
            Some("8.0.36"),
            0,
            90_000.0,
            250.0,
            Some(2_500.0),
            2,
            1,
            0,
            BTreeMap::new(),
            now(),
        );

        let report = evaluate_mysql_digest_statistics_inventory(&[target], Pillar::Cost, now());
        let codes = reason_codes(&report);

        assert!(codes.contains(&REASON_COST_OWNER_NOT_RECORDED));
        assert!(codes.contains(&REASON_COST_NO_DIGEST_COVERAGE));
        assert!(codes.contains(&REASON_COST_EXPENSIVE_DIGESTS));
    }

    #[test]
    fn resilience_flags_missing_coverage_and_latency_outliers() {
        let target = item(
            Some("database-platform"),
            Some("8.0.36"),
            0,
            30_000.0,
            2_500.0,
            Some(10.0),
            0,
            0,
            0,
            labels(&[("owner", "database-platform")]),
            now(),
        );

        let report =
            evaluate_mysql_digest_statistics_inventory(&[target], Pillar::Resilience, now());
        let codes = reason_codes(&report);

        assert!(codes.contains(&REASON_RES_NO_DIGEST_COVERAGE));
        assert!(codes.contains(&REASON_RES_LATENCY_OUTLIERS));
    }

    #[test]
    fn security_flags_missing_version_missing_coverage_and_unnormalized_text() {
        let target = item(
            Some("database-platform"),
            None,
            0,
            10_000.0,
            50.0,
            Some(5.0),
            0,
            0,
            2,
            labels(&[("owner", "database-platform")]),
            now(),
        );

        let report = evaluate_mysql_digest_statistics_inventory(&[target], Pillar::Security, now());
        let codes = reason_codes(&report);

        assert!(codes.contains(&REASON_SEC_VERSION_NOT_RECORDED));
        assert!(codes.contains(&REASON_SEC_NO_DIGEST_COVERAGE));
        assert!(codes.contains(&REASON_SEC_UNNORMALIZED_TEXT));
    }

    #[test]
    fn stale_inventory_is_counted_for_any_pillar() {
        let target = DigestStatisticsInventoryItem {
            collected_at: now() - Duration::hours(25),
            ..healthy_item()
        };

        let report = evaluate_mysql_digest_statistics_inventory(&[target], Pillar::Cost, now());

        assert_eq!(report.stale_resources, 1);
        assert!(reason_codes(&report).contains(&REASON_INV_STALE_DATA));
    }

    #[test]
    fn healthy_digest_statistics_pass_claimed_pillars() {
        let target = healthy_item();

        for pillar in [Pillar::Cost, Pillar::Resilience, Pillar::Security] {
            let report = evaluate_mysql_digest_statistics_inventory(
                std::slice::from_ref(&target),
                pillar,
                now(),
            );
            assert!(
                report.findings.is_empty(),
                "unexpected for {:?}: {:?}",
                pillar,
                report.findings
            );
            assert_eq!(report.score, 100);
        }
    }
}

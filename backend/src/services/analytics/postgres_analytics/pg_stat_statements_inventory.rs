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

// Deterministic PostgreSQL pg_stat_statements inventory evaluator for roadmap
// rows 05-POSTGRES-00050/00057/00078.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::services::aws::inventory::types::{
    score_pillar, InventoryFinding, Pillar, PillarReport, Severity, COST_ALLOCATION_TAG_KEYS,
    DEFAULT_STALE_AFTER_HOURS,
};

pub const RESOURCE_TYPE: &str = "PostgresPgStatStatements";
pub const REASON_COST_OWNER_NOT_RECORDED: &str =
    "POSTGRES_PG_STAT_STATEMENTS_COST_OWNER_NOT_RECORDED";
pub const REASON_COST_STATEMENT_COVERAGE_MISSING: &str =
    "POSTGRES_PG_STAT_STATEMENTS_COST_STATEMENT_COVERAGE_MISSING";
pub const REASON_COST_TEMP_IO_PRESENT: &str = "POSTGRES_PG_STAT_STATEMENTS_COST_TEMP_IO_PRESENT";
pub const REASON_RES_STATEMENT_COVERAGE_UNAVAILABLE: &str =
    "POSTGRES_PG_STAT_STATEMENTS_RES_STATEMENT_COVERAGE_UNAVAILABLE";
pub const REASON_RES_SLOW_STATEMENT_MEAN: &str =
    "POSTGRES_PG_STAT_STATEMENTS_RES_SLOW_STATEMENT_MEAN";
pub const REASON_SEC_VERSION_NOT_RECORDED: &str =
    "POSTGRES_PG_STAT_STATEMENTS_SEC_VERSION_NOT_RECORDED";
pub const REASON_SEC_QUERY_TEXT_VISIBLE: &str =
    "POSTGRES_PG_STAT_STATEMENTS_SEC_QUERY_TEXT_VISIBLE";
pub const REASON_INV_STALE_DATA: &str = "POSTGRES_PG_STAT_STATEMENTS_INV_STALE_DATA";

const SLOW_STATEMENT_MEAN_MS: f64 = 1_000.0;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PgStatStatementsInventoryItem {
    pub connection_id: String,
    pub connection_name: String,
    pub owner: Option<String>,
    pub labels: BTreeMap<String, String>,
    pub server_version: Option<String>,
    pub extension_available: bool,
    pub statements_tracked: usize,
    pub total_calls: i64,
    pub total_exec_time_ms: f64,
    pub max_mean_exec_time_ms: Option<f64>,
    pub shared_blks_read: i64,
    pub shared_blks_hit: i64,
    pub temp_blks_written: i64,
    pub query_text_visible: bool,
    pub missing_evidence_reason: Option<String>,
    pub collected_at: DateTime<Utc>,
}

pub fn evaluate_postgres_pg_stat_statements_inventory(
    items: &[PgStatStatementsInventoryItem],
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

fn evaluate_cost(
    item: &PgStatStatementsInventoryItem,
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
                "PostgreSQL pg_stat_statements inventory for connection {} has no owner, team, project, or cost-center metadata",
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

    if !item.extension_available || item.statements_tracked == 0 {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_STATEMENT_COVERAGE_MISSING,
            Severity::High,
            format!(
                "PostgreSQL pg_stat_statements inventory for connection {} has no statement cost coverage",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "connection_name": item.connection_name,
                "extension_available": item.extension_available,
                "statements_tracked": item.statements_tracked,
                "missing_evidence_reason": item.missing_evidence_reason,
                "recommendation": "Enable and collect pg_stat_statements aggregate evidence before attributing SQL workload cost",
            }),
        ));
    }

    if item.temp_blks_written > 0 {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_TEMP_IO_PRESENT,
            Severity::Low,
            format!(
                "PostgreSQL pg_stat_statements inventory for connection {} shows temporary block writes",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "connection_name": item.connection_name,
                "temp_blks_written": item.temp_blks_written,
                "total_calls": item.total_calls,
                "recommendation": "Review top SQL by temporary I/O for sort, hash, and work_mem right-sizing opportunities",
            }),
        ));
    }
}

fn evaluate_resilience(
    item: &PgStatStatementsInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if !item.extension_available {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_STATEMENT_COVERAGE_UNAVAILABLE,
            Severity::High,
            format!(
                "PostgreSQL pg_stat_statements inventory for connection {} is unavailable",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "connection_name": item.connection_name,
                "extension_available": item.extension_available,
                "missing_evidence_reason": item.missing_evidence_reason,
                "recommendation": "Install or grant access to pg_stat_statements so slow query and workload regression triage has deterministic evidence",
            }),
        ));
    }

    if item
        .max_mean_exec_time_ms
        .map(|value| value >= SLOW_STATEMENT_MEAN_MS)
        .unwrap_or(false)
    {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_SLOW_STATEMENT_MEAN,
            Severity::High,
            format!(
                "PostgreSQL pg_stat_statements inventory for connection {} has a statement mean execution time above {:.0} ms",
                item.connection_name, SLOW_STATEMENT_MEAN_MS
            ),
            json!({
                "connection_id": item.connection_id,
                "connection_name": item.connection_name,
                "max_mean_exec_time_ms": item.max_mean_exec_time_ms,
                "threshold_ms": SLOW_STATEMENT_MEAN_MS,
                "total_calls": item.total_calls,
                "recommendation": "Triage the slowest mean-execution statements before they create latency or pool saturation incidents",
            }),
        ));
    }
}

fn evaluate_security(
    item: &PgStatStatementsInventoryItem,
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
                "PostgreSQL pg_stat_statements inventory for connection {} has no recorded server version",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "connection_name": item.connection_name,
                "server_version": item.server_version,
                "recommendation": "Record PostgreSQL server version with pg_stat_statements evidence so extension posture and advisories can be mapped deterministically",
            }),
        ));
    }

    if item.query_text_visible {
        findings.push(finding(
            item,
            pillar,
            REASON_SEC_QUERY_TEXT_VISIBLE,
            Severity::Low,
            format!(
                "PostgreSQL pg_stat_statements inventory for connection {} exposes normalized query text",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "connection_name": item.connection_name,
                "query_text_visible": item.query_text_visible,
                "recommendation": "Limit pg_stat_statements access to trusted operators because normalized query text can still reveal schema and workload details",
            }),
        ));
    }
}

fn stale_finding(
    item: &PgStatStatementsInventoryItem,
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
            "Inventory data for PostgreSQL pg_stat_statements connection {} is {} hours old (threshold {} hours)",
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
    item: &PgStatStatementsInventoryItem,
    pillar: Pillar,
    reason_code: &str,
    severity: Severity,
    message: String,
    evidence: Value,
) -> InventoryFinding {
    InventoryFinding {
        resource_id: item.connection_id.clone(),
        arn: format!("postgres://pg-stat-statements/{}", item.connection_id),
        pillar,
        reason_code: reason_code.to_string(),
        severity,
        message,
        evidence,
    }
}

fn has_owner_metadata(item: &PgStatStatementsInventoryItem) -> bool {
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

#[cfg(test)]
mod tests {
    use super::*;
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
        extension_available: bool,
        statements_tracked: usize,
        temp_blks_written: i64,
        max_mean_exec_time_ms: Option<f64>,
        query_text_visible: bool,
        labels: BTreeMap<String, String>,
        collected_at: DateTime<Utc>,
    ) -> PgStatStatementsInventoryItem {
        PgStatStatementsInventoryItem {
            connection_id: "postgres-1".to_string(),
            connection_name: "orders-postgres".to_string(),
            owner: owner.map(str::to_string),
            labels,
            server_version: server_version.map(str::to_string),
            extension_available,
            statements_tracked,
            total_calls: 1_200,
            total_exec_time_ms: 42_000.0,
            max_mean_exec_time_ms,
            shared_blks_read: 200,
            shared_blks_hit: 10_000,
            temp_blks_written,
            query_text_visible,
            missing_evidence_reason: (!extension_available)
                .then(|| "pg_stat_statements extension is not available".to_string()),
            collected_at,
        }
    }

    fn healthy_item() -> PgStatStatementsInventoryItem {
        item(
            Some("database-platform"),
            Some("PostgreSQL 16.2"),
            true,
            24,
            0,
            Some(20.0),
            false,
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
    fn cost_flags_missing_owner_and_statement_coverage() {
        let target = item(
            Some(""),
            Some("PostgreSQL 16.2"),
            false,
            0,
            0,
            None,
            false,
            BTreeMap::new(),
            now(),
        );

        let report = evaluate_postgres_pg_stat_statements_inventory(&[target], Pillar::Cost, now());
        let codes = reason_codes(&report);

        assert!(codes.contains(&REASON_COST_OWNER_NOT_RECORDED));
        assert!(codes.contains(&REASON_COST_STATEMENT_COVERAGE_MISSING));
    }

    #[test]
    fn cost_flags_temp_io_for_workload_right_sizing() {
        let target = item(
            Some("database-platform"),
            Some("PostgreSQL 16.2"),
            true,
            12,
            44,
            Some(25.0),
            false,
            labels(&[("owner", "database-platform")]),
            now(),
        );

        let report = evaluate_postgres_pg_stat_statements_inventory(&[target], Pillar::Cost, now());

        assert!(reason_codes(&report).contains(&REASON_COST_TEMP_IO_PRESENT));
    }

    #[test]
    fn resilience_flags_missing_extension_and_slow_mean_statement() {
        let unavailable = item(
            Some("database-platform"),
            Some("PostgreSQL 16.2"),
            false,
            0,
            0,
            None,
            false,
            labels(&[("owner", "database-platform")]),
            now(),
        );
        let slow = item(
            Some("database-platform"),
            Some("PostgreSQL 16.2"),
            true,
            10,
            0,
            Some(1_200.0),
            false,
            labels(&[("owner", "database-platform")]),
            now(),
        );

        let report = evaluate_postgres_pg_stat_statements_inventory(
            &[unavailable, slow],
            Pillar::Resilience,
            now(),
        );
        let codes = reason_codes(&report);

        assert!(codes.contains(&REASON_RES_STATEMENT_COVERAGE_UNAVAILABLE));
        assert!(codes.contains(&REASON_RES_SLOW_STATEMENT_MEAN));
    }

    #[test]
    fn security_flags_missing_version_and_visible_query_text() {
        let target = item(
            Some("database-platform"),
            None,
            true,
            12,
            0,
            Some(25.0),
            true,
            labels(&[("owner", "database-platform")]),
            now(),
        );

        let report =
            evaluate_postgres_pg_stat_statements_inventory(&[target], Pillar::Security, now());
        let codes = reason_codes(&report);

        assert!(codes.contains(&REASON_SEC_VERSION_NOT_RECORDED));
        assert!(codes.contains(&REASON_SEC_QUERY_TEXT_VISIBLE));
    }

    #[test]
    fn stale_inventory_is_counted_for_any_pillar() {
        let target = item(
            Some("database-platform"),
            Some("PostgreSQL 16.2"),
            true,
            12,
            0,
            Some(25.0),
            false,
            labels(&[("owner", "database-platform")]),
            now() - Duration::hours(25),
        );

        let report = evaluate_postgres_pg_stat_statements_inventory(&[target], Pillar::Cost, now());

        assert_eq!(report.stale_resources, 1);
        assert!(reason_codes(&report).contains(&REASON_INV_STALE_DATA));
    }

    #[test]
    fn healthy_pg_stat_statements_passes_claimed_pillars() {
        let target = healthy_item();

        for pillar in [Pillar::Cost, Pillar::Resilience, Pillar::Security] {
            let report = evaluate_postgres_pg_stat_statements_inventory(
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

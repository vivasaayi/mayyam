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

// Deterministic PostgreSQL pg_stat_wal inventory evaluator for roadmap rows
// 05-POSTGRES-00197/00204/00225.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::services::aws::inventory::types::{
    score_pillar, InventoryFinding, Pillar, PillarReport, Severity, COST_ALLOCATION_TAG_KEYS,
    DEFAULT_STALE_AFTER_HOURS,
};

pub const RESOURCE_TYPE: &str = "PostgresPgStatWal";
pub const REASON_COST_OWNER_NOT_RECORDED: &str = "POSTGRES_PG_STAT_WAL_COST_OWNER_NOT_RECORDED";
pub const REASON_COST_WAL_COVERAGE_MISSING: &str = "POSTGRES_PG_STAT_WAL_COST_WAL_COVERAGE_MISSING";
pub const REASON_COST_HIGH_WAL_VOLUME: &str = "POSTGRES_PG_STAT_WAL_COST_HIGH_WAL_VOLUME";
pub const REASON_RES_WAL_COVERAGE_UNAVAILABLE: &str =
    "POSTGRES_PG_STAT_WAL_RES_WAL_COVERAGE_UNAVAILABLE";
pub const REASON_RES_WAL_BUFFER_PRESSURE: &str = "POSTGRES_PG_STAT_WAL_RES_WAL_BUFFER_PRESSURE";
pub const REASON_RES_WAL_SYNC_PRESSURE: &str = "POSTGRES_PG_STAT_WAL_RES_WAL_SYNC_PRESSURE";
pub const REASON_SEC_VERSION_NOT_RECORDED: &str = "POSTGRES_PG_STAT_WAL_SEC_VERSION_NOT_RECORDED";
pub const REASON_SEC_WAL_LEVEL_WEAK_OR_MISSING: &str =
    "POSTGRES_PG_STAT_WAL_SEC_WAL_LEVEL_WEAK_OR_MISSING";
pub const REASON_SEC_ARCHIVE_EVIDENCE_MISSING: &str =
    "POSTGRES_PG_STAT_WAL_SEC_ARCHIVE_EVIDENCE_MISSING";
pub const REASON_INV_STALE_DATA: &str = "POSTGRES_PG_STAT_WAL_INV_STALE_DATA";

const HIGH_WAL_BYTES_THRESHOLD: i64 = 10 * 1024 * 1024 * 1024;
const WAL_BUFFERS_FULL_THRESHOLD: i64 = 10;
const WAL_BUFFERS_FULL_RATIO_THRESHOLD: f64 = 0.01;
const WAL_SYNC_OPS_THRESHOLD: i64 = 100;
const WAL_SYNC_TIME_THRESHOLD_MS: f64 = 1_000.0;
const WAL_WRITE_TIME_THRESHOLD_MS: f64 = 1_000.0;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PgStatWalInventoryItem {
    pub connection_id: String,
    pub connection_name: String,
    pub owner: Option<String>,
    pub labels: BTreeMap<String, String>,
    pub wal_stats_available: bool,
    pub missing_evidence_reason: Option<String>,
    pub wal_records: i64,
    pub wal_fpi: i64,
    pub wal_bytes: i64,
    pub wal_buffers_full: i64,
    pub wal_write: i64,
    pub wal_sync: i64,
    pub wal_write_time_ms: f64,
    pub wal_sync_time_ms: f64,
    pub stats_reset: Option<DateTime<Utc>>,
    pub archive_mode: Option<String>,
    pub max_wal_size_mb: Option<i64>,
    pub min_wal_size_mb: Option<i64>,
    pub wal_level: Option<String>,
    pub server_version: Option<String>,
    pub collected_at: DateTime<Utc>,
}

pub fn evaluate_postgres_pg_stat_wal_inventory(
    items: &[PgStatWalInventoryItem],
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
    item: &PgStatWalInventoryItem,
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
                "PostgreSQL pg_stat_wal inventory for connection {} has no owner, team, project, or cost-center metadata",
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

    if !item.wal_stats_available || !has_wal_evidence(item) {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_WAL_COVERAGE_MISSING,
            Severity::High,
            format!(
                "PostgreSQL pg_stat_wal inventory for connection {} has no WAL cost coverage",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "connection_name": item.connection_name,
                "wal_stats_available": item.wal_stats_available,
                "missing_evidence_reason": item.missing_evidence_reason,
                "recommendation": "Collect pg_stat_wal counters before attributing write amplification, storage churn, or replication/archive cost",
            }),
        ));
    }

    if item.wal_bytes >= HIGH_WAL_BYTES_THRESHOLD {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_HIGH_WAL_VOLUME,
            Severity::Medium,
            format!(
                "PostgreSQL pg_stat_wal inventory for connection {} shows high WAL volume",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "connection_name": item.connection_name,
                "wal_records": item.wal_records,
                "wal_fpi": item.wal_fpi,
                "wal_bytes": item.wal_bytes,
                "threshold_bytes": HIGH_WAL_BYTES_THRESHOLD,
                "max_wal_size_mb": item.max_wal_size_mb,
                "min_wal_size_mb": item.min_wal_size_mb,
                "stats_reset": item.stats_reset,
                "recommendation": "Review high WAL generation for write amplification, checkpoint cadence, full-page-image volume, and storage or archive right-sizing opportunities",
            }),
        ));
    }
}

fn evaluate_resilience(
    item: &PgStatWalInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if !item.wal_stats_available {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_WAL_COVERAGE_UNAVAILABLE,
            Severity::High,
            format!(
                "PostgreSQL pg_stat_wal inventory for connection {} is unavailable",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "connection_name": item.connection_name,
                "wal_stats_available": item.wal_stats_available,
                "missing_evidence_reason": item.missing_evidence_reason,
                "recommendation": "Collect pg_stat_wal evidence so WAL generation, buffer pressure, and sync behavior have deterministic resilience signals",
            }),
        ));
    }

    if item.wal_buffers_full >= WAL_BUFFERS_FULL_THRESHOLD
        || wal_buffers_full_ratio(item)
            .map(|ratio| ratio >= WAL_BUFFERS_FULL_RATIO_THRESHOLD)
            .unwrap_or(false)
    {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_WAL_BUFFER_PRESSURE,
            Severity::Medium,
            format!(
                "PostgreSQL pg_stat_wal inventory for connection {} shows WAL buffer pressure",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "connection_name": item.connection_name,
                "wal_records": item.wal_records,
                "wal_buffers_full": item.wal_buffers_full,
                "wal_buffers_full_ratio": wal_buffers_full_ratio(item),
                "wal_buffers_full_threshold": WAL_BUFFERS_FULL_THRESHOLD,
                "wal_buffers_full_ratio_threshold": WAL_BUFFERS_FULL_RATIO_THRESHOLD,
                "recommendation": "Investigate WAL buffer sizing, checkpoint cadence, and write burst patterns before buffer pressure creates commit latency",
            }),
        ));
    }

    if item.wal_sync >= WAL_SYNC_OPS_THRESHOLD
        || item.wal_sync_time_ms >= WAL_SYNC_TIME_THRESHOLD_MS
        || item.wal_write_time_ms >= WAL_WRITE_TIME_THRESHOLD_MS
    {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_WAL_SYNC_PRESSURE,
            Severity::High,
            format!(
                "PostgreSQL pg_stat_wal inventory for connection {} shows WAL sync or write latency pressure",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "connection_name": item.connection_name,
                "wal_write": item.wal_write,
                "wal_sync": item.wal_sync,
                "wal_write_time_ms": item.wal_write_time_ms,
                "wal_sync_time_ms": item.wal_sync_time_ms,
                "sync_ops_threshold": WAL_SYNC_OPS_THRESHOLD,
                "sync_time_threshold_ms": WAL_SYNC_TIME_THRESHOLD_MS,
                "write_time_threshold_ms": WAL_WRITE_TIME_THRESHOLD_MS,
                "recommendation": "Investigate storage latency, synchronous commit behavior, checkpoint pressure, and WAL device saturation before commits become availability incidents",
            }),
        ));
    }
}

fn evaluate_security(
    item: &PgStatWalInventoryItem,
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
                "PostgreSQL pg_stat_wal inventory for connection {} has no recorded server version",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "connection_name": item.connection_name,
                "server_version": item.server_version,
                "recommendation": "Record PostgreSQL server version with pg_stat_wal evidence so version support and advisory checks can be mapped deterministically",
            }),
        ));
    }

    if weak_or_missing_wal_level(item) {
        findings.push(finding(
            item,
            pillar,
            REASON_SEC_WAL_LEVEL_WEAK_OR_MISSING,
            Severity::Medium,
            format!(
                "PostgreSQL pg_stat_wal inventory for connection {} has weak or missing WAL level evidence",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "connection_name": item.connection_name,
                "wal_level": item.wal_level,
                "recommendation": "Record wal_level and prefer replica or logical evidence where recovery, auditing, or downstream change-capture controls depend on WAL contents",
            }),
        ));
    }

    if item
        .archive_mode
        .as_deref()
        .map(str::trim)
        .filter(|archive_mode| !archive_mode.is_empty())
        .is_none()
    {
        findings.push(finding(
            item,
            pillar,
            REASON_SEC_ARCHIVE_EVIDENCE_MISSING,
            Severity::Medium,
            format!(
                "PostgreSQL pg_stat_wal inventory for connection {} has no WAL archive-mode evidence",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "connection_name": item.connection_name,
                "archive_mode": item.archive_mode,
                "recommendation": "Record archive_mode with WAL inventory so recovery, retention, and forensic readiness posture can be scored from evidence",
            }),
        ));
    }
}

fn stale_finding(
    item: &PgStatWalInventoryItem,
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
            "Inventory data for PostgreSQL pg_stat_wal connection {} is {} hours old (threshold {} hours)",
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
    item: &PgStatWalInventoryItem,
    pillar: Pillar,
    reason_code: &str,
    severity: Severity,
    message: String,
    evidence: Value,
) -> InventoryFinding {
    InventoryFinding {
        resource_id: item.connection_id.clone(),
        arn: format!("postgres://pg-stat-wal/{}", item.connection_id),
        pillar,
        reason_code: reason_code.to_string(),
        severity,
        message,
        evidence,
    }
}

fn has_owner_metadata(item: &PgStatWalInventoryItem) -> bool {
    item.owner
        .as_deref()
        .map(str::trim)
        .filter(|owner| !owner.is_empty())
        .is_some()
        || has_any_metadata_key(&item.labels, COST_ALLOCATION_TAG_KEYS)
}

fn has_wal_evidence(item: &PgStatWalInventoryItem) -> bool {
    [
        item.wal_records,
        item.wal_fpi,
        item.wal_bytes,
        item.wal_buffers_full,
        item.wal_write,
        item.wal_sync,
    ]
    .iter()
    .any(|value| *value > 0)
        || item.wal_write_time_ms > 0.0
        || item.wal_sync_time_ms > 0.0
}

fn wal_buffers_full_ratio(item: &PgStatWalInventoryItem) -> Option<f64> {
    if item.wal_records <= 0 {
        return None;
    }

    Some(item.wal_buffers_full.max(0) as f64 / item.wal_records as f64)
}

fn weak_or_missing_wal_level(item: &PgStatWalInventoryItem) -> bool {
    match item
        .wal_level
        .as_deref()
        .map(str::trim)
        .filter(|wal_level| !wal_level.is_empty())
    {
        Some(wal_level) => wal_level.eq_ignore_ascii_case("minimal"),
        None => true,
    }
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
        labels: BTreeMap<String, String>,
        wal_stats_available: bool,
        wal_records: i64,
        wal_bytes: i64,
        wal_buffers_full: i64,
        wal_sync: i64,
        wal_sync_time_ms: f64,
        archive_mode: Option<&str>,
        wal_level: Option<&str>,
        server_version: Option<&str>,
        collected_at: DateTime<Utc>,
    ) -> PgStatWalInventoryItem {
        PgStatWalInventoryItem {
            connection_id: "postgres-1".to_string(),
            connection_name: "orders-postgres".to_string(),
            owner: owner.map(str::to_string),
            labels,
            wal_stats_available,
            missing_evidence_reason: (!wal_stats_available)
                .then(|| "pg_stat_wal row was not collected".to_string()),
            wal_records,
            wal_fpi: wal_records / 20,
            wal_bytes,
            wal_buffers_full,
            wal_write: wal_records / 10,
            wal_sync,
            wal_write_time_ms: if wal_sync_time_ms >= WAL_WRITE_TIME_THRESHOLD_MS {
                1_200.0
            } else if wal_records > 0 {
                20.0
            } else {
                0.0
            },
            wal_sync_time_ms,
            stats_reset: Some(now() - Duration::hours(2)),
            archive_mode: archive_mode.map(str::to_string),
            max_wal_size_mb: Some(4_096),
            min_wal_size_mb: Some(1_024),
            wal_level: wal_level.map(str::to_string),
            server_version: server_version.map(str::to_string),
            collected_at,
        }
    }

    fn healthy_item() -> PgStatWalInventoryItem {
        item(
            Some("database-platform"),
            labels(&[("cost-center", "cc-42")]),
            true,
            10_000,
            512 * 1024 * 1024,
            0,
            10,
            20.0,
            Some("on"),
            Some("replica"),
            Some("PostgreSQL 16.2"),
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
    fn cost_flags_missing_owner_and_wal_coverage() {
        let target = item(
            Some(""),
            BTreeMap::new(),
            false,
            0,
            0,
            0,
            0,
            0.0,
            Some("on"),
            Some("replica"),
            Some("PostgreSQL 16.2"),
            now(),
        );

        let report = evaluate_postgres_pg_stat_wal_inventory(&[target], Pillar::Cost, now());
        let codes = reason_codes(&report);

        assert!(codes.contains(&REASON_COST_OWNER_NOT_RECORDED));
        assert!(codes.contains(&REASON_COST_WAL_COVERAGE_MISSING));
    }

    #[test]
    fn cost_flags_high_wal_volume() {
        let target = item(
            Some("database-platform"),
            labels(&[("owner", "database-platform")]),
            true,
            1_000_000,
            12 * 1024 * 1024 * 1024,
            0,
            10,
            20.0,
            Some("on"),
            Some("replica"),
            Some("PostgreSQL 16.2"),
            now(),
        );

        let report = evaluate_postgres_pg_stat_wal_inventory(&[target], Pillar::Cost, now());

        assert!(reason_codes(&report).contains(&REASON_COST_HIGH_WAL_VOLUME));
    }

    #[test]
    fn resilience_flags_unavailable_buffer_and_sync_pressure() {
        let unavailable = item(
            Some("database-platform"),
            labels(&[("owner", "database-platform")]),
            false,
            0,
            0,
            0,
            0,
            0.0,
            Some("on"),
            Some("replica"),
            Some("PostgreSQL 16.2"),
            now(),
        );
        let pressured = item(
            Some("database-platform"),
            labels(&[("owner", "database-platform")]),
            true,
            1_000,
            512 * 1024 * 1024,
            20,
            150,
            1_500.0,
            Some("on"),
            Some("replica"),
            Some("PostgreSQL 16.2"),
            now(),
        );

        let report = evaluate_postgres_pg_stat_wal_inventory(
            &[unavailable, pressured],
            Pillar::Resilience,
            now(),
        );
        let codes = reason_codes(&report);

        assert!(codes.contains(&REASON_RES_WAL_COVERAGE_UNAVAILABLE));
        assert!(codes.contains(&REASON_RES_WAL_BUFFER_PRESSURE));
        assert!(codes.contains(&REASON_RES_WAL_SYNC_PRESSURE));
    }

    #[test]
    fn security_flags_missing_version_weak_wal_level_and_missing_archive_evidence() {
        let target = item(
            Some("database-platform"),
            labels(&[("owner", "database-platform")]),
            true,
            10_000,
            512 * 1024 * 1024,
            0,
            10,
            20.0,
            None,
            Some("minimal"),
            None,
            now(),
        );

        let report = evaluate_postgres_pg_stat_wal_inventory(&[target], Pillar::Security, now());
        let codes = reason_codes(&report);

        assert!(codes.contains(&REASON_SEC_VERSION_NOT_RECORDED));
        assert!(codes.contains(&REASON_SEC_WAL_LEVEL_WEAK_OR_MISSING));
        assert!(codes.contains(&REASON_SEC_ARCHIVE_EVIDENCE_MISSING));
    }

    #[test]
    fn stale_inventory_is_counted_for_any_pillar() {
        let target = PgStatWalInventoryItem {
            collected_at: now() - Duration::hours(25),
            ..healthy_item()
        };

        let report = evaluate_postgres_pg_stat_wal_inventory(&[target], Pillar::Cost, now());

        assert_eq!(report.stale_resources, 1);
        assert!(reason_codes(&report).contains(&REASON_INV_STALE_DATA));
    }

    #[test]
    fn healthy_pg_stat_wal_passes_claimed_pillars() {
        let target = healthy_item();

        for pillar in [Pillar::Cost, Pillar::Resilience, Pillar::Security] {
            let report = evaluate_postgres_pg_stat_wal_inventory(
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

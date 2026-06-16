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

// Deterministic PostgreSQL pg_stat_database inventory evaluator for roadmap
// rows 05-POSTGRES-00099/00106/00127.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::services::aws::inventory::types::{
    score_pillar, InventoryFinding, Pillar, PillarReport, Severity, COST_ALLOCATION_TAG_KEYS,
    DEFAULT_STALE_AFTER_HOURS,
};

pub const RESOURCE_TYPE: &str = "PostgresPgStatDatabase";
pub const REASON_COST_OWNER_NOT_RECORDED: &str =
    "POSTGRES_PG_STAT_DATABASE_COST_OWNER_NOT_RECORDED";
pub const REASON_COST_DATABASE_STATS_COVERAGE_MISSING: &str =
    "POSTGRES_PG_STAT_DATABASE_COST_DATABASE_STATS_COVERAGE_MISSING";
pub const REASON_COST_TEMP_IO_PRESENT: &str = "POSTGRES_PG_STAT_DATABASE_COST_TEMP_IO_PRESENT";
pub const REASON_RES_CONNECTION_PRESSURE: &str =
    "POSTGRES_PG_STAT_DATABASE_RES_CONNECTION_PRESSURE";
pub const REASON_RES_DATABASE_STATS_COVERAGE_MISSING: &str =
    "POSTGRES_PG_STAT_DATABASE_RES_DATABASE_STATS_COVERAGE_MISSING";
pub const REASON_RES_TRANSACTION_ROLLBACK_PRESSURE: &str =
    "POSTGRES_PG_STAT_DATABASE_RES_TRANSACTION_ROLLBACK_PRESSURE";
pub const REASON_RES_DATABASE_CONTENTION: &str =
    "POSTGRES_PG_STAT_DATABASE_RES_DATABASE_CONTENTION";
pub const REASON_SEC_VERSION_NOT_RECORDED: &str =
    "POSTGRES_PG_STAT_DATABASE_SEC_VERSION_NOT_RECORDED";
pub const REASON_SEC_EXPOSURE_EVIDENCE_MISSING: &str =
    "POSTGRES_PG_STAT_DATABASE_SEC_EXPOSURE_EVIDENCE_MISSING";
pub const REASON_SEC_PUBLIC_EXPOSURE_WITHOUT_SSL: &str =
    "POSTGRES_PG_STAT_DATABASE_SEC_PUBLIC_EXPOSURE_WITHOUT_SSL";
pub const REASON_INV_STALE_DATA: &str = "POSTGRES_PG_STAT_DATABASE_INV_STALE_DATA";

const CONNECTION_PRESSURE_THRESHOLD_PCT: f64 = 80.0;
const ROLLBACK_PRESSURE_THRESHOLD_PCT: f64 = 10.0;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PgStatDatabaseExposureEvidence {
    pub publicly_accessible: Option<bool>,
    pub ssl_enforced: Option<bool>,
    pub allowed_source_count: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PgStatDatabaseInventoryItem {
    pub connection_id: String,
    pub connection_name: String,
    pub datname: String,
    pub owner: Option<String>,
    pub labels: BTreeMap<String, String>,
    pub stats_available: bool,
    pub missing_evidence_reason: Option<String>,
    pub numbackends: i64,
    pub max_connections: Option<i64>,
    pub xact_commit: i64,
    pub xact_rollback: i64,
    pub blks_read: i64,
    pub blks_hit: i64,
    pub tup_returned: i64,
    pub tup_fetched: i64,
    pub tup_inserted: i64,
    pub tup_updated: i64,
    pub tup_deleted: i64,
    pub deadlocks: i64,
    pub temp_files: i64,
    pub temp_bytes: i64,
    pub conflicts: i64,
    pub checksum_failures: Option<i64>,
    pub database_size_bytes: Option<i64>,
    pub server_version: Option<String>,
    pub exposure: Option<PgStatDatabaseExposureEvidence>,
    pub collected_at: DateTime<Utc>,
}

pub fn evaluate_postgres_pg_stat_database_inventory(
    items: &[PgStatDatabaseInventoryItem],
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
    item: &PgStatDatabaseInventoryItem,
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
                "PostgreSQL pg_stat_database inventory for database {} on connection {} has no owner, team, project, or cost-center metadata",
                item.datname, item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "connection_name": item.connection_name,
                "datname": item.datname,
                "owner": item.owner,
                "checked_keys": COST_ALLOCATION_TAG_KEYS,
                "checked_locations": ["owner", "labels"],
            }),
        ));
    }

    if !item.stats_available {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_DATABASE_STATS_COVERAGE_MISSING,
            Severity::High,
            format!(
                "PostgreSQL pg_stat_database inventory for database {} on connection {} has no database statistics coverage",
                item.datname, item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "connection_name": item.connection_name,
                "datname": item.datname,
                "stats_available": item.stats_available,
                "missing_evidence_reason": item.missing_evidence_reason,
                "recommendation": "Collect pg_stat_database rows before attributing database-level cost, storage, cache, or temporary I/O posture",
            }),
        ));
    }

    if item.temp_files > 0 || item.temp_bytes > 0 {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_TEMP_IO_PRESENT,
            Severity::Low,
            format!(
                "PostgreSQL database {} on connection {} has temporary file I/O",
                item.datname, item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "connection_name": item.connection_name,
                "datname": item.datname,
                "temp_files": item.temp_files,
                "temp_bytes": item.temp_bytes,
                "database_size_bytes": item.database_size_bytes,
                "recommendation": "Review workload memory settings and query patterns that spill to temporary files before scaling compute or storage",
            }),
        ));
    }
}

fn evaluate_resilience(
    item: &PgStatDatabaseInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if !item.stats_available {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_DATABASE_STATS_COVERAGE_MISSING,
            Severity::High,
            format!(
                "PostgreSQL pg_stat_database inventory for database {} on connection {} is unavailable",
                item.datname, item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "connection_name": item.connection_name,
                "datname": item.datname,
                "stats_available": item.stats_available,
                "missing_evidence_reason": item.missing_evidence_reason,
                "recommendation": "Collect pg_stat_database evidence before relying on transaction, conflict, deadlock, or connection pressure signals",
            }),
        ));
    }

    if let Some(max_connections) = item.max_connections {
        if max_connections > 0 {
            let usage_pct = (item.numbackends as f64 / max_connections as f64) * 100.0;
            if usage_pct >= CONNECTION_PRESSURE_THRESHOLD_PCT {
                findings.push(finding(
                    item,
                    pillar,
                    REASON_RES_CONNECTION_PRESSURE,
                    Severity::High,
                    format!(
                        "PostgreSQL database {} on connection {} is using {:.1}% of max_connections",
                        item.datname, item.connection_name, usage_pct
                    ),
                    json!({
                        "connection_id": item.connection_id,
                        "connection_name": item.connection_name,
                        "datname": item.datname,
                        "numbackends": item.numbackends,
                        "max_connections": max_connections,
                        "usage_pct": usage_pct,
                        "threshold_pct": CONNECTION_PRESSURE_THRESHOLD_PCT,
                        "recommendation": "Triage connection pool sizing and per-database session concentration before connection exhaustion affects availability",
                    }),
                ));
            }
        }
    }

    let total_xacts = item.xact_commit + item.xact_rollback;
    if total_xacts > 0 {
        let rollback_pct = (item.xact_rollback as f64 / total_xacts as f64) * 100.0;
        if rollback_pct >= ROLLBACK_PRESSURE_THRESHOLD_PCT {
            findings.push(finding(
                item,
                pillar,
                REASON_RES_TRANSACTION_ROLLBACK_PRESSURE,
                Severity::Medium,
                format!(
                    "PostgreSQL database {} on connection {} has {:.1}% rolled back transactions",
                    item.datname, item.connection_name, rollback_pct
                ),
                json!({
                    "connection_id": item.connection_id,
                    "connection_name": item.connection_name,
                    "datname": item.datname,
                    "xact_commit": item.xact_commit,
                    "xact_rollback": item.xact_rollback,
                    "rollback_pct": rollback_pct,
                    "threshold_pct": ROLLBACK_PRESSURE_THRESHOLD_PCT,
                    "recommendation": "Investigate application errors, serialization failures, and retry storms that can reduce database availability",
                }),
            ));
        }
    }

    if item.deadlocks > 0 || item.conflicts > 0 || item.checksum_failures.unwrap_or(0) > 0 {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_DATABASE_CONTENTION,
            Severity::High,
            format!(
                "PostgreSQL database {} on connection {} has deadlock, conflict, or checksum failure symptoms",
                item.datname, item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "connection_name": item.connection_name,
                "datname": item.datname,
                "deadlocks": item.deadlocks,
                "conflicts": item.conflicts,
                "checksum_failures": item.checksum_failures,
                "recommendation": "Investigate deadlocks, recovery conflicts, and checksum failures before they become availability or data-integrity incidents",
            }),
        ));
    }
}

fn evaluate_security(
    item: &PgStatDatabaseInventoryItem,
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
                "PostgreSQL pg_stat_database inventory for database {} on connection {} has no recorded server version",
                item.datname, item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "connection_name": item.connection_name,
                "datname": item.datname,
                "server_version": item.server_version,
                "recommendation": "Record PostgreSQL server version with pg_stat_database evidence so vulnerability and support posture can be mapped deterministically",
            }),
        ));
    }

    if item.exposure.is_none() {
        findings.push(finding(
            item,
            pillar,
            REASON_SEC_EXPOSURE_EVIDENCE_MISSING,
            Severity::Medium,
            format!(
                "PostgreSQL pg_stat_database inventory for database {} on connection {} has no database exposure evidence",
                item.datname, item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "connection_name": item.connection_name,
                "datname": item.datname,
                "exposure": item.exposure,
                "recommendation": "Attach network exposure and TLS enforcement evidence to database inventory before scoring access posture",
            }),
        ));
    }

    if let Some(exposure) = &item.exposure {
        if exposure.publicly_accessible == Some(true) && exposure.ssl_enforced == Some(false) {
            findings.push(finding(
                item,
                pillar,
                REASON_SEC_PUBLIC_EXPOSURE_WITHOUT_SSL,
                Severity::High,
                format!(
                    "PostgreSQL database {} on connection {} is publicly reachable without enforced SSL evidence",
                    item.datname, item.connection_name
                ),
                json!({
                    "connection_id": item.connection_id,
                    "connection_name": item.connection_name,
                    "datname": item.datname,
                    "publicly_accessible": exposure.publicly_accessible,
                    "ssl_enforced": exposure.ssl_enforced,
                    "allowed_source_count": exposure.allowed_source_count,
                    "recommendation": "Restrict public reachability or enforce TLS before accepting this database security posture",
                }),
            ));
        }
    }
}

fn stale_finding(
    item: &PgStatDatabaseInventoryItem,
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
            "Inventory data for PostgreSQL pg_stat_database database {} on connection {} is {} hours old (threshold {} hours)",
            item.datname, item.connection_name, age_hours, DEFAULT_STALE_AFTER_HOURS
        ),
        json!({
            "connection_id": item.connection_id,
            "connection_name": item.connection_name,
            "datname": item.datname,
            "collected_at": item.collected_at,
            "age_hours": age_hours,
            "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
        }),
    ))
}

fn finding(
    item: &PgStatDatabaseInventoryItem,
    pillar: Pillar,
    reason_code: &str,
    severity: Severity,
    message: String,
    evidence: Value,
) -> InventoryFinding {
    InventoryFinding {
        resource_id: format!("{}/{}", item.connection_id, item.datname),
        arn: format!(
            "postgres://pg-stat-database/{}/{}",
            item.connection_id, item.datname
        ),
        pillar,
        reason_code: reason_code.to_string(),
        severity,
        message,
        evidence,
    }
}

fn has_owner_metadata(item: &PgStatDatabaseInventoryItem) -> bool {
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

    fn exposure(
        publicly_accessible: Option<bool>,
        ssl_enforced: Option<bool>,
    ) -> PgStatDatabaseExposureEvidence {
        PgStatDatabaseExposureEvidence {
            publicly_accessible,
            ssl_enforced,
            allowed_source_count: Some(2),
        }
    }

    fn item(
        owner: Option<&str>,
        server_version: Option<&str>,
        stats_available: bool,
        numbackends: i64,
        max_connections: Option<i64>,
        xact_commit: i64,
        xact_rollback: i64,
        deadlocks: i64,
        conflicts: i64,
        checksum_failures: Option<i64>,
        temp_files: i64,
        temp_bytes: i64,
        exposure: Option<PgStatDatabaseExposureEvidence>,
        labels: BTreeMap<String, String>,
        collected_at: DateTime<Utc>,
    ) -> PgStatDatabaseInventoryItem {
        PgStatDatabaseInventoryItem {
            connection_id: "postgres-1".to_string(),
            connection_name: "orders-postgres".to_string(),
            datname: "orders".to_string(),
            owner: owner.map(str::to_string),
            labels,
            stats_available,
            missing_evidence_reason: (!stats_available)
                .then(|| "pg_stat_database row was not collected".to_string()),
            numbackends,
            max_connections,
            xact_commit,
            xact_rollback,
            blks_read: 120,
            blks_hit: 12_000,
            tup_returned: 500_000,
            tup_fetched: 300_000,
            tup_inserted: 1_200,
            tup_updated: 700,
            tup_deleted: 200,
            deadlocks,
            temp_files,
            temp_bytes,
            conflicts,
            checksum_failures,
            database_size_bytes: Some(48 * 1024 * 1024 * 1024),
            server_version: server_version.map(str::to_string),
            exposure,
            collected_at,
        }
    }

    fn healthy_item() -> PgStatDatabaseInventoryItem {
        item(
            Some("database-platform"),
            Some("PostgreSQL 16.2"),
            true,
            12,
            Some(200),
            10_000,
            50,
            0,
            0,
            Some(0),
            0,
            0,
            Some(exposure(Some(false), Some(true))),
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
    fn cost_flags_missing_owner_and_database_stats_coverage() {
        let target = item(
            Some(""),
            Some("PostgreSQL 16.2"),
            false,
            0,
            Some(100),
            0,
            0,
            0,
            0,
            Some(0),
            0,
            0,
            Some(exposure(Some(false), Some(true))),
            BTreeMap::new(),
            now(),
        );

        let report = evaluate_postgres_pg_stat_database_inventory(&[target], Pillar::Cost, now());
        let codes = reason_codes(&report);

        assert!(codes.contains(&REASON_COST_OWNER_NOT_RECORDED));
        assert!(codes.contains(&REASON_COST_DATABASE_STATS_COVERAGE_MISSING));
    }

    #[test]
    fn cost_flags_temp_io_for_right_sizing() {
        let target = item(
            Some("database-platform"),
            Some("PostgreSQL 16.2"),
            true,
            10,
            Some(100),
            1_000,
            5,
            0,
            0,
            Some(0),
            2,
            64 * 1024 * 1024,
            Some(exposure(Some(false), Some(true))),
            labels(&[("owner", "database-platform")]),
            now(),
        );

        let report = evaluate_postgres_pg_stat_database_inventory(&[target], Pillar::Cost, now());

        assert!(reason_codes(&report).contains(&REASON_COST_TEMP_IO_PRESENT));
    }

    #[test]
    fn resilience_flags_connection_rollback_and_contention_symptoms() {
        let target = item(
            Some("database-platform"),
            Some("PostgreSQL 16.2"),
            true,
            85,
            Some(100),
            900,
            100,
            1,
            2,
            Some(1),
            0,
            0,
            Some(exposure(Some(false), Some(true))),
            labels(&[("owner", "database-platform")]),
            now(),
        );

        let report =
            evaluate_postgres_pg_stat_database_inventory(&[target], Pillar::Resilience, now());
        let codes = reason_codes(&report);

        assert!(codes.contains(&REASON_RES_CONNECTION_PRESSURE));
        assert!(codes.contains(&REASON_RES_TRANSACTION_ROLLBACK_PRESSURE));
        assert!(codes.contains(&REASON_RES_DATABASE_CONTENTION));
    }

    #[test]
    fn resilience_flags_missing_database_stats_coverage() {
        let target = item(
            Some("database-platform"),
            Some("PostgreSQL 16.2"),
            false,
            0,
            Some(100),
            0,
            0,
            0,
            0,
            Some(0),
            0,
            0,
            Some(exposure(Some(false), Some(true))),
            labels(&[("owner", "database-platform")]),
            now(),
        );

        let report =
            evaluate_postgres_pg_stat_database_inventory(&[target], Pillar::Resilience, now());

        assert!(reason_codes(&report).contains(&REASON_RES_DATABASE_STATS_COVERAGE_MISSING));
    }

    #[test]
    fn security_flags_missing_version_and_exposure_evidence() {
        let target = item(
            Some("database-platform"),
            None,
            true,
            10,
            Some(100),
            1_000,
            5,
            0,
            0,
            Some(0),
            0,
            0,
            None,
            labels(&[("owner", "database-platform")]),
            now(),
        );

        let report =
            evaluate_postgres_pg_stat_database_inventory(&[target], Pillar::Security, now());
        let codes = reason_codes(&report);

        assert!(codes.contains(&REASON_SEC_VERSION_NOT_RECORDED));
        assert!(codes.contains(&REASON_SEC_EXPOSURE_EVIDENCE_MISSING));
    }

    #[test]
    fn security_flags_public_exposure_without_ssl() {
        let target = item(
            Some("database-platform"),
            Some("PostgreSQL 16.2"),
            true,
            10,
            Some(100),
            1_000,
            5,
            0,
            0,
            Some(0),
            0,
            0,
            Some(exposure(Some(true), Some(false))),
            labels(&[("owner", "database-platform")]),
            now(),
        );

        let report =
            evaluate_postgres_pg_stat_database_inventory(&[target], Pillar::Security, now());

        assert!(reason_codes(&report).contains(&REASON_SEC_PUBLIC_EXPOSURE_WITHOUT_SSL));
    }

    #[test]
    fn stale_inventory_is_counted_for_any_pillar() {
        let target = item(
            Some("database-platform"),
            Some("PostgreSQL 16.2"),
            true,
            10,
            Some(100),
            1_000,
            5,
            0,
            0,
            Some(0),
            0,
            0,
            Some(exposure(Some(false), Some(true))),
            labels(&[("owner", "database-platform")]),
            now() - Duration::hours(25),
        );

        let report = evaluate_postgres_pg_stat_database_inventory(&[target], Pillar::Cost, now());

        assert_eq!(report.stale_resources, 1);
        assert!(reason_codes(&report).contains(&REASON_INV_STALE_DATA));
    }

    #[test]
    fn healthy_pg_stat_database_passes_claimed_pillars() {
        let target = healthy_item();

        for pillar in [Pillar::Cost, Pillar::Resilience, Pillar::Security] {
            let report = evaluate_postgres_pg_stat_database_inventory(
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

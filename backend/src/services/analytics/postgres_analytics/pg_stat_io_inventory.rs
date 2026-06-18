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

// Deterministic PostgreSQL pg_stat_io inventory evaluator for roadmap rows
// 05-POSTGRES-00148/00155/00176.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::services::aws::inventory::types::{
    score_pillar, InventoryFinding, Pillar, PillarReport, Severity, COST_ALLOCATION_TAG_KEYS,
    DEFAULT_STALE_AFTER_HOURS,
};

pub const RESOURCE_TYPE: &str = "PostgresPgStatIo";
pub const REASON_COST_OWNER_NOT_RECORDED: &str = "POSTGRES_PG_STAT_IO_COST_OWNER_NOT_RECORDED";
pub const REASON_COST_IO_COVERAGE_MISSING: &str = "POSTGRES_PG_STAT_IO_COST_IO_COVERAGE_MISSING";
pub const REASON_COST_HIGH_IO_BYTES: &str = "POSTGRES_PG_STAT_IO_COST_HIGH_IO_BYTES";
pub const REASON_COST_TEMP_IO_PRESENT: &str = "POSTGRES_PG_STAT_IO_COST_TEMP_IO_PRESENT";
pub const REASON_RES_IO_COVERAGE_UNAVAILABLE: &str =
    "POSTGRES_PG_STAT_IO_RES_IO_COVERAGE_UNAVAILABLE";
pub const REASON_RES_LOW_CACHE_REUSE: &str = "POSTGRES_PG_STAT_IO_RES_LOW_CACHE_REUSE";
pub const REASON_RES_DURABILITY_PRESSURE: &str = "POSTGRES_PG_STAT_IO_RES_DURABILITY_PRESSURE";
pub const REASON_SEC_VERSION_NOT_RECORDED: &str = "POSTGRES_PG_STAT_IO_SEC_VERSION_NOT_RECORDED";
pub const REASON_SEC_SECURITY_EVIDENCE_MISSING: &str =
    "POSTGRES_PG_STAT_IO_SEC_SECURITY_EVIDENCE_MISSING";
pub const REASON_INV_STALE_DATA: &str = "POSTGRES_PG_STAT_IO_INV_STALE_DATA";

const HIGH_IO_BYTES_THRESHOLD: i64 = 10 * 1024 * 1024 * 1024;
const LOW_CACHE_REUSE_RATIO: f64 = 0.90;
const DURABILITY_OPS_THRESHOLD: i64 = 100;
const HIGH_WRITE_LATENCY_MS: f64 = 1_000.0;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PgStatIoInventoryItem {
    pub connection_id: String,
    pub connection_name: String,
    pub owner: Option<String>,
    pub labels: BTreeMap<String, String>,
    pub pg_stat_io_available: bool,
    pub backend_type: String,
    pub context: String,
    pub object: String,
    pub reads: i64,
    pub read_time_ms: f64,
    pub writes: i64,
    pub write_time_ms: f64,
    pub writebacks: i64,
    pub writeback_time_ms: f64,
    pub extends: i64,
    pub extend_time_ms: f64,
    pub op_bytes: i64,
    pub hits: i64,
    pub evictions: i64,
    pub reuses: i64,
    pub fsyncs: i64,
    pub fsync_time_ms: f64,
    pub stats_reset: Option<DateTime<Utc>>,
    pub server_version: Option<String>,
    pub security_evidence_recorded: bool,
    pub missing_evidence_reason: Option<String>,
    pub collected_at: DateTime<Utc>,
}

pub fn evaluate_postgres_pg_stat_io_inventory(
    items: &[PgStatIoInventoryItem],
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
    item: &PgStatIoInventoryItem,
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
                "PostgreSQL pg_stat_io inventory for connection {} has no owner, team, project, or cost-center metadata",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "connection_name": item.connection_name,
                "backend_type": item.backend_type,
                "context": item.context,
                "object": item.object,
                "owner": item.owner,
                "checked_keys": COST_ALLOCATION_TAG_KEYS,
                "checked_locations": ["owner", "labels"],
            }),
        ));
    }

    if !item.pg_stat_io_available || !has_io_evidence(item) {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_IO_COVERAGE_MISSING,
            Severity::High,
            format!(
                "PostgreSQL pg_stat_io inventory for connection {} has no I/O cost coverage",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "connection_name": item.connection_name,
                "pg_stat_io_available": item.pg_stat_io_available,
                "missing_evidence_reason": item.missing_evidence_reason,
                "recommendation": "Collect pg_stat_io aggregate counters before attributing PostgreSQL storage and I/O cost",
            }),
        ));
    }

    let total_io_bytes = total_io_bytes(item);
    if total_io_bytes >= HIGH_IO_BYTES_THRESHOLD {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_HIGH_IO_BYTES,
            Severity::Medium,
            format!(
                "PostgreSQL pg_stat_io inventory for connection {} shows high read/write/extend I/O volume",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "connection_name": item.connection_name,
                "backend_type": item.backend_type,
                "context": item.context,
                "object": item.object,
                "reads": item.reads,
                "writes": item.writes,
                "extends": item.extends,
                "op_bytes": item.op_bytes,
                "total_io_bytes": total_io_bytes,
                "threshold_bytes": HIGH_IO_BYTES_THRESHOLD,
                "recommendation": "Review high-volume pg_stat_io dimensions for indexing, cache sizing, query shape, and storage tier right-sizing opportunities",
            }),
        ));
    }

    if is_temp_io(item) && (item.reads > 0 || item.writes > 0 || item.extends > 0) {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_TEMP_IO_PRESENT,
            Severity::Low,
            format!(
                "PostgreSQL pg_stat_io inventory for connection {} shows temporary object I/O",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "connection_name": item.connection_name,
                "backend_type": item.backend_type,
                "context": item.context,
                "object": item.object,
                "reads": item.reads,
                "writes": item.writes,
                "extends": item.extends,
                "op_bytes": item.op_bytes,
                "total_io_bytes": total_io_bytes,
                "recommendation": "Investigate temporary I/O for sort/hash memory tuning and query plan improvements before it becomes persistent storage spend",
            }),
        ));
    }
}

fn evaluate_resilience(
    item: &PgStatIoInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if !item.pg_stat_io_available {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_IO_COVERAGE_UNAVAILABLE,
            Severity::High,
            format!(
                "PostgreSQL pg_stat_io inventory for connection {} is unavailable",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "connection_name": item.connection_name,
                "pg_stat_io_available": item.pg_stat_io_available,
                "missing_evidence_reason": item.missing_evidence_reason,
                "recommendation": "Collect pg_stat_io from PostgreSQL 16 or compatible sources so I/O saturation and durability symptoms have deterministic evidence",
            }),
        ));
    }

    if cache_reuse_ratio(item)
        .map(|ratio| ratio < LOW_CACHE_REUSE_RATIO)
        .unwrap_or(false)
    {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_LOW_CACHE_REUSE,
            Severity::Medium,
            format!(
                "PostgreSQL pg_stat_io inventory for connection {} has cache reuse below {:.0}%",
                item.connection_name,
                LOW_CACHE_REUSE_RATIO * 100.0
            ),
            json!({
                "connection_id": item.connection_id,
                "connection_name": item.connection_name,
                "backend_type": item.backend_type,
                "context": item.context,
                "object": item.object,
                "hits": item.hits,
                "reads": item.reads,
                "reuses": item.reuses,
                "evictions": item.evictions,
                "cache_reuse_ratio": cache_reuse_ratio(item),
                "threshold": LOW_CACHE_REUSE_RATIO,
                "recommendation": "Triage cache churn and read amplification before they create latency spikes or storage throttling incidents",
            }),
        ));
    }

    if item.fsyncs >= DURABILITY_OPS_THRESHOLD
        || item.evictions >= DURABILITY_OPS_THRESHOLD
        || item.writebacks >= DURABILITY_OPS_THRESHOLD
        || item.write_time_ms >= HIGH_WRITE_LATENCY_MS
        || item.fsync_time_ms >= HIGH_WRITE_LATENCY_MS
        || item.writeback_time_ms >= HIGH_WRITE_LATENCY_MS
    {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_DURABILITY_PRESSURE,
            Severity::High,
            format!(
                "PostgreSQL pg_stat_io inventory for connection {} shows fsync, eviction, writeback, or write-latency pressure",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "connection_name": item.connection_name,
                "backend_type": item.backend_type,
                "context": item.context,
                "object": item.object,
                "writes": item.writes,
                "write_time_ms": item.write_time_ms,
                "writebacks": item.writebacks,
                "writeback_time_ms": item.writeback_time_ms,
                "evictions": item.evictions,
                "fsyncs": item.fsyncs,
                "fsync_time_ms": item.fsync_time_ms,
                "ops_threshold": DURABILITY_OPS_THRESHOLD,
                "latency_threshold_ms": HIGH_WRITE_LATENCY_MS,
                "recommendation": "Investigate checkpoint, background writer, storage latency, and dirty-page pressure before durability work threatens availability",
            }),
        ));
    }
}

fn evaluate_security(
    item: &PgStatIoInventoryItem,
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
                "PostgreSQL pg_stat_io inventory for connection {} has no recorded server version",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "connection_name": item.connection_name,
                "server_version": item.server_version,
                "recommendation": "Record PostgreSQL server version with pg_stat_io evidence so version support and advisory checks can be mapped deterministically",
            }),
        ));
    }

    if !item.security_evidence_recorded {
        findings.push(finding(
            item,
            pillar,
            REASON_SEC_SECURITY_EVIDENCE_MISSING,
            Severity::Medium,
            format!(
                "PostgreSQL pg_stat_io inventory for connection {} has no recorded access-control evidence",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "connection_name": item.connection_name,
                "security_evidence_recorded": item.security_evidence_recorded,
                "missing_evidence_reason": item.missing_evidence_reason,
                "recommendation": "Record collector role, RBAC scope, or equivalent access-control evidence before relying on pg_stat_io inventory for security posture",
            }),
        ));
    }
}

fn stale_finding(
    item: &PgStatIoInventoryItem,
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
            "Inventory data for PostgreSQL pg_stat_io connection {} is {} hours old (threshold {} hours)",
            item.connection_name, age_hours, DEFAULT_STALE_AFTER_HOURS
        ),
        json!({
            "connection_id": item.connection_id,
            "connection_name": item.connection_name,
            "backend_type": item.backend_type,
            "context": item.context,
            "object": item.object,
            "collected_at": item.collected_at,
            "age_hours": age_hours,
            "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
        }),
    ))
}

fn finding(
    item: &PgStatIoInventoryItem,
    pillar: Pillar,
    reason_code: &str,
    severity: Severity,
    message: String,
    evidence: Value,
) -> InventoryFinding {
    InventoryFinding {
        resource_id: item.connection_id.clone(),
        arn: format!(
            "postgres://pg-stat-io/{}/{}/{}",
            item.connection_id, item.backend_type, item.context
        ),
        pillar,
        reason_code: reason_code.to_string(),
        severity,
        message,
        evidence,
    }
}

fn has_owner_metadata(item: &PgStatIoInventoryItem) -> bool {
    item.owner
        .as_deref()
        .map(str::trim)
        .filter(|owner| !owner.is_empty())
        .is_some()
        || has_any_metadata_key(&item.labels, COST_ALLOCATION_TAG_KEYS)
}

fn has_io_evidence(item: &PgStatIoInventoryItem) -> bool {
    [
        item.reads,
        item.writes,
        item.writebacks,
        item.extends,
        item.hits,
        item.evictions,
        item.reuses,
        item.fsyncs,
    ]
    .iter()
    .any(|value| *value > 0)
}

fn total_io_bytes(item: &PgStatIoInventoryItem) -> i64 {
    item.reads
        .saturating_add(item.writes)
        .saturating_add(item.extends)
        .saturating_mul(item.op_bytes.max(0))
}

fn cache_reuse_ratio(item: &PgStatIoInventoryItem) -> Option<f64> {
    let cache_events = item
        .hits
        .saturating_add(item.reads)
        .saturating_add(item.reuses)
        .saturating_add(item.evictions);
    if cache_events <= 0 {
        return None;
    }

    Some((item.hits.saturating_add(item.reuses)) as f64 / cache_events as f64)
}

fn is_temp_io(item: &PgStatIoInventoryItem) -> bool {
    item.context.eq_ignore_ascii_case("temp")
        || item.object.eq_ignore_ascii_case("temp relation")
        || item.object.eq_ignore_ascii_case("temporary relation")
        || item.object.to_ascii_lowercase().contains("temp")
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
        pg_stat_io_available: bool,
        backend_type: &str,
        context: &str,
        object: &str,
        reads: i64,
        writes: i64,
        hits: i64,
        evictions: i64,
        fsyncs: i64,
        server_version: Option<&str>,
        security_evidence_recorded: bool,
        collected_at: DateTime<Utc>,
    ) -> PgStatIoInventoryItem {
        PgStatIoInventoryItem {
            connection_id: "postgres-1".to_string(),
            connection_name: "orders-postgres".to_string(),
            owner: owner.map(str::to_string),
            labels,
            pg_stat_io_available,
            backend_type: backend_type.to_string(),
            context: context.to_string(),
            object: object.to_string(),
            reads,
            read_time_ms: 12.0,
            writes,
            write_time_ms: if fsyncs > 0 { 1_200.0 } else { 20.0 },
            writebacks: if fsyncs > 0 { 125 } else { 0 },
            writeback_time_ms: if fsyncs > 0 { 1_100.0 } else { 0.0 },
            extends: 0,
            extend_time_ms: 0.0,
            op_bytes: 8_192,
            hits,
            evictions,
            reuses: hits / 2,
            fsyncs,
            fsync_time_ms: if fsyncs > 0 { 1_500.0 } else { 0.0 },
            stats_reset: Some(now() - Duration::hours(2)),
            server_version: server_version.map(str::to_string),
            security_evidence_recorded,
            missing_evidence_reason: (!pg_stat_io_available)
                .then(|| "pg_stat_io is not available on this connection".to_string()),
            collected_at,
        }
    }

    fn healthy_item() -> PgStatIoInventoryItem {
        item(
            Some("database-platform"),
            labels(&[("cost-center", "cc-42")]),
            true,
            "client backend",
            "normal",
            "relation",
            100,
            20,
            8_000,
            2,
            0,
            Some("PostgreSQL 16.2"),
            true,
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
    fn cost_flags_missing_owner_and_io_coverage() {
        let target = item(
            Some(""),
            BTreeMap::new(),
            false,
            "",
            "",
            "",
            0,
            0,
            0,
            0,
            0,
            Some("PostgreSQL 16.2"),
            true,
            now(),
        );

        let report = evaluate_postgres_pg_stat_io_inventory(&[target], Pillar::Cost, now());
        let codes = reason_codes(&report);

        assert!(codes.contains(&REASON_COST_OWNER_NOT_RECORDED));
        assert!(codes.contains(&REASON_COST_IO_COVERAGE_MISSING));
    }

    #[test]
    fn cost_flags_high_io_and_temp_io() {
        let target = item(
            Some("database-platform"),
            labels(&[("owner", "database-platform")]),
            true,
            "client backend",
            "temp",
            "temp relation",
            900_000,
            500_000,
            10_000,
            0,
            0,
            Some("PostgreSQL 16.2"),
            true,
            now(),
        );

        let report = evaluate_postgres_pg_stat_io_inventory(&[target], Pillar::Cost, now());
        let codes = reason_codes(&report);

        assert!(codes.contains(&REASON_COST_HIGH_IO_BYTES));
        assert!(codes.contains(&REASON_COST_TEMP_IO_PRESENT));
    }

    #[test]
    fn resilience_flags_unavailable_low_cache_reuse_and_durability_pressure() {
        let unavailable = item(
            Some("database-platform"),
            labels(&[("owner", "database-platform")]),
            false,
            "",
            "",
            "",
            0,
            0,
            0,
            0,
            0,
            Some("PostgreSQL 16.2"),
            true,
            now(),
        );
        let pressured = item(
            Some("database-platform"),
            labels(&[("owner", "database-platform")]),
            true,
            "background writer",
            "normal",
            "relation",
            1_000,
            400,
            50,
            200,
            150,
            Some("PostgreSQL 16.2"),
            true,
            now(),
        );

        let report = evaluate_postgres_pg_stat_io_inventory(
            &[unavailable, pressured],
            Pillar::Resilience,
            now(),
        );
        let codes = reason_codes(&report);

        assert!(codes.contains(&REASON_RES_IO_COVERAGE_UNAVAILABLE));
        assert!(codes.contains(&REASON_RES_LOW_CACHE_REUSE));
        assert!(codes.contains(&REASON_RES_DURABILITY_PRESSURE));
    }

    #[test]
    fn security_flags_missing_version_and_security_evidence() {
        let target = item(
            Some("database-platform"),
            labels(&[("owner", "database-platform")]),
            true,
            "client backend",
            "normal",
            "relation",
            100,
            20,
            8_000,
            0,
            0,
            None,
            false,
            now(),
        );

        let report = evaluate_postgres_pg_stat_io_inventory(&[target], Pillar::Security, now());
        let codes = reason_codes(&report);

        assert!(codes.contains(&REASON_SEC_VERSION_NOT_RECORDED));
        assert!(codes.contains(&REASON_SEC_SECURITY_EVIDENCE_MISSING));
    }

    #[test]
    fn stale_inventory_is_counted_for_any_pillar() {
        let target = PgStatIoInventoryItem {
            collected_at: now() - Duration::hours(25),
            ..healthy_item()
        };

        let report = evaluate_postgres_pg_stat_io_inventory(&[target], Pillar::Cost, now());

        assert_eq!(report.stale_resources, 1);
        assert!(reason_codes(&report).contains(&REASON_INV_STALE_DATA));
    }

    #[test]
    fn healthy_pg_stat_io_passes_claimed_pillars() {
        let target = healthy_item();

        for pillar in [Pillar::Cost, Pillar::Resilience, Pillar::Security] {
            let report = evaluate_postgres_pg_stat_io_inventory(
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

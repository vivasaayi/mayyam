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

// Deterministic PostgreSQL pg_locks inventory evaluator for roadmap rows
// 05-POSTGRES-00246/00253/00274.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::services::aws::inventory::types::{
    score_pillar, InventoryFinding, Pillar, PillarReport, Severity, COST_ALLOCATION_TAG_KEYS,
    DEFAULT_STALE_AFTER_HOURS,
};

pub const RESOURCE_TYPE: &str = "PostgresPgLocks";
pub const REASON_COST_OWNER_NOT_RECORDED: &str = "POSTGRES_PG_LOCKS_COST_OWNER_NOT_RECORDED";
pub const REASON_COST_LOCK_COVERAGE_MISSING: &str = "POSTGRES_PG_LOCKS_COST_LOCK_COVERAGE_MISSING";
pub const REASON_COST_HIGH_LOCK_VOLUME: &str = "POSTGRES_PG_LOCKS_COST_HIGH_LOCK_VOLUME";
pub const REASON_RES_LOCK_COVERAGE_UNAVAILABLE: &str =
    "POSTGRES_PG_LOCKS_RES_LOCK_COVERAGE_UNAVAILABLE";
pub const REASON_RES_WAITING_OR_BLOCKED_LOCKS: &str =
    "POSTGRES_PG_LOCKS_RES_WAITING_OR_BLOCKED_LOCKS";
pub const REASON_RES_LOCK_TABLE_PRESSURE: &str = "POSTGRES_PG_LOCKS_RES_LOCK_TABLE_PRESSURE";
pub const REASON_SEC_VERSION_NOT_RECORDED: &str = "POSTGRES_PG_LOCKS_SEC_VERSION_NOT_RECORDED";
pub const REASON_SEC_SECURITY_EVIDENCE_MISSING: &str =
    "POSTGRES_PG_LOCKS_SEC_SECURITY_EVIDENCE_MISSING";
pub const REASON_INV_STALE_DATA: &str = "POSTGRES_PG_LOCKS_INV_STALE_DATA";

const HIGH_LOCK_VOLUME_THRESHOLD: i64 = 1_000;
const HIGH_LOCK_CAPACITY_USAGE_PCT: f64 = 50.0;
const LOCK_TABLE_PRESSURE_THRESHOLD_PCT: f64 = 80.0;
const LONG_WAIT_SECONDS_THRESHOLD: i64 = 30;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PgLocksInventoryItem {
    pub connection_id: String,
    pub connection_name: String,
    pub owner: Option<String>,
    pub labels: BTreeMap<String, String>,
    pub locks_available: bool,
    pub missing_evidence_reason: Option<String>,
    pub total_locks: i64,
    pub granted_locks: i64,
    pub waiting_locks: i64,
    pub exclusive_locks: i64,
    pub transactionid_locks: i64,
    pub relation_locks: i64,
    pub blocked_sessions: i64,
    pub oldest_wait_seconds: Option<i64>,
    pub max_locks_per_transaction: Option<i64>,
    pub max_connections: Option<i64>,
    pub deadlock_timeout_ms: Option<i64>,
    pub server_version: Option<String>,
    pub security_evidence_recorded: bool,
    pub collected_at: DateTime<Utc>,
}

pub fn evaluate_postgres_pg_locks_inventory(
    items: &[PgLocksInventoryItem],
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
    item: &PgLocksInventoryItem,
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
                "PostgreSQL pg_locks inventory for connection {} has no owner, team, project, or cost-center metadata",
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

    if !item.locks_available || !has_lock_evidence(item) {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_LOCK_COVERAGE_MISSING,
            Severity::High,
            format!(
                "PostgreSQL pg_locks inventory for connection {} has no lock cost coverage",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "connection_name": item.connection_name,
                "locks_available": item.locks_available,
                "missing_evidence_reason": item.missing_evidence_reason,
                "recommendation": "Collect pg_locks and pg_stat_activity lock evidence before attributing connection, transaction, or capacity waste",
            }),
        ));
    }

    if high_lock_volume(item) {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_HIGH_LOCK_VOLUME,
            Severity::Medium,
            format!(
                "PostgreSQL pg_locks inventory for connection {} shows high lock volume that can hide capacity waste",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "connection_name": item.connection_name,
                "total_locks": item.total_locks,
                "granted_locks": item.granted_locks,
                "waiting_locks": item.waiting_locks,
                "exclusive_locks": item.exclusive_locks,
                "transactionid_locks": item.transactionid_locks,
                "relation_locks": item.relation_locks,
                "lock_capacity": lock_capacity(item),
                "capacity_usage_pct": lock_capacity_usage_pct(item),
                "lock_count_threshold": HIGH_LOCK_VOLUME_THRESHOLD,
                "capacity_usage_threshold_pct": HIGH_LOCK_CAPACITY_USAGE_PCT,
                "recommendation": "Review transaction duration, lock-heavy migrations, and query patterns before scaling connections or compute to mask avoidable lock churn",
            }),
        ));
    }
}

fn evaluate_resilience(
    item: &PgLocksInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if !item.locks_available {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_LOCK_COVERAGE_UNAVAILABLE,
            Severity::High,
            format!(
                "PostgreSQL pg_locks inventory for connection {} is unavailable",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "connection_name": item.connection_name,
                "locks_available": item.locks_available,
                "missing_evidence_reason": item.missing_evidence_reason,
                "recommendation": "Collect pg_locks evidence so lock waits, blockers, and lock-table pressure have deterministic resilience signals",
            }),
        ));
    }

    if item.waiting_locks > 0
        || item.blocked_sessions > 0
        || item
            .oldest_wait_seconds
            .map(|seconds| seconds >= LONG_WAIT_SECONDS_THRESHOLD)
            .unwrap_or(false)
        || wait_exceeds_deadlock_timeout(item)
    {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_WAITING_OR_BLOCKED_LOCKS,
            Severity::High,
            format!(
                "PostgreSQL pg_locks inventory for connection {} has waiting locks or blocked sessions",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "connection_name": item.connection_name,
                "waiting_locks": item.waiting_locks,
                "blocked_sessions": item.blocked_sessions,
                "oldest_wait_seconds": item.oldest_wait_seconds,
                "deadlock_timeout_ms": item.deadlock_timeout_ms,
                "long_wait_threshold_seconds": LONG_WAIT_SECONDS_THRESHOLD,
                "recommendation": "Triage blocking statements, transaction age, and lock acquisition order before lock waits become user-visible outages",
            }),
        ));
    }

    if lock_capacity_usage_pct(item)
        .map(|usage_pct| usage_pct >= LOCK_TABLE_PRESSURE_THRESHOLD_PCT)
        .unwrap_or(false)
    {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_LOCK_TABLE_PRESSURE,
            Severity::High,
            format!(
                "PostgreSQL pg_locks inventory for connection {} is near lock-table capacity",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "connection_name": item.connection_name,
                "total_locks": item.total_locks,
                "max_locks_per_transaction": item.max_locks_per_transaction,
                "max_connections": item.max_connections,
                "lock_capacity": lock_capacity(item),
                "capacity_usage_pct": lock_capacity_usage_pct(item),
                "threshold_pct": LOCK_TABLE_PRESSURE_THRESHOLD_PCT,
                "recommendation": "Reduce lock amplification or tune max_locks_per_transaction before lock table exhaustion affects availability",
            }),
        ));
    }
}

fn evaluate_security(
    item: &PgLocksInventoryItem,
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
                "PostgreSQL pg_locks inventory for connection {} has no recorded server version",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "connection_name": item.connection_name,
                "server_version": item.server_version,
                "recommendation": "Record PostgreSQL server version with pg_locks evidence so advisory, support, and lock-behavior posture can be mapped deterministically",
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
                "PostgreSQL pg_locks inventory for connection {} has no recorded access-control evidence",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "connection_name": item.connection_name,
                "security_evidence_recorded": item.security_evidence_recorded,
                "missing_evidence_reason": item.missing_evidence_reason,
                "recommendation": "Record collector role, RBAC scope, or equivalent access-control evidence before relying on pg_locks inventory for security posture",
            }),
        ));
    }
}

fn stale_finding(
    item: &PgLocksInventoryItem,
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
            "Inventory data for PostgreSQL pg_locks connection {} is {} hours old (threshold {} hours)",
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
    item: &PgLocksInventoryItem,
    pillar: Pillar,
    reason_code: &str,
    severity: Severity,
    message: String,
    evidence: Value,
) -> InventoryFinding {
    InventoryFinding {
        resource_id: item.connection_id.clone(),
        arn: format!("postgres://pg-locks/{}", item.connection_id),
        pillar,
        reason_code: reason_code.to_string(),
        severity,
        message,
        evidence,
    }
}

fn has_owner_metadata(item: &PgLocksInventoryItem) -> bool {
    item.owner
        .as_deref()
        .map(str::trim)
        .filter(|owner| !owner.is_empty())
        .is_some()
        || has_any_metadata_key(&item.labels, COST_ALLOCATION_TAG_KEYS)
}

fn has_lock_evidence(item: &PgLocksInventoryItem) -> bool {
    [
        item.total_locks,
        item.granted_locks,
        item.waiting_locks,
        item.exclusive_locks,
        item.transactionid_locks,
        item.relation_locks,
        item.blocked_sessions,
    ]
    .iter()
    .any(|value| *value > 0)
}

fn high_lock_volume(item: &PgLocksInventoryItem) -> bool {
    item.total_locks >= HIGH_LOCK_VOLUME_THRESHOLD
        || lock_capacity_usage_pct(item)
            .map(|usage_pct| usage_pct >= HIGH_LOCK_CAPACITY_USAGE_PCT)
            .unwrap_or(false)
}

fn wait_exceeds_deadlock_timeout(item: &PgLocksInventoryItem) -> bool {
    match (item.oldest_wait_seconds, item.deadlock_timeout_ms) {
        (Some(wait_seconds), Some(timeout_ms)) if timeout_ms > 0 => {
            wait_seconds.saturating_mul(1_000) >= timeout_ms
        }
        _ => false,
    }
}

fn lock_capacity_usage_pct(item: &PgLocksInventoryItem) -> Option<f64> {
    let capacity = lock_capacity(item)?;
    if capacity <= 0 {
        return None;
    }

    Some((item.total_locks.max(0) as f64 / capacity as f64) * 100.0)
}

fn lock_capacity(item: &PgLocksInventoryItem) -> Option<i64> {
    let max_locks_per_transaction = item.max_locks_per_transaction?;
    let max_connections = item.max_connections?;
    if max_locks_per_transaction <= 0 || max_connections <= 0 {
        return None;
    }

    Some(max_locks_per_transaction.saturating_mul(max_connections))
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
        locks_available: bool,
        total_locks: i64,
        waiting_locks: i64,
        blocked_sessions: i64,
        oldest_wait_seconds: Option<i64>,
        max_locks_per_transaction: Option<i64>,
        max_connections: Option<i64>,
        server_version: Option<&str>,
        security_evidence_recorded: bool,
        collected_at: DateTime<Utc>,
    ) -> PgLocksInventoryItem {
        PgLocksInventoryItem {
            connection_id: "postgres-1".to_string(),
            connection_name: "orders-postgres".to_string(),
            owner: owner.map(str::to_string),
            labels,
            locks_available,
            missing_evidence_reason: (!locks_available)
                .then(|| "pg_locks rows were not collected".to_string()),
            total_locks,
            granted_locks: total_locks.saturating_sub(waiting_locks).max(0),
            waiting_locks,
            exclusive_locks: total_locks / 5,
            transactionid_locks: total_locks / 4,
            relation_locks: total_locks / 2,
            blocked_sessions,
            oldest_wait_seconds,
            max_locks_per_transaction,
            max_connections,
            deadlock_timeout_ms: Some(1_000),
            server_version: server_version.map(str::to_string),
            security_evidence_recorded,
            collected_at,
        }
    }

    fn healthy_item() -> PgLocksInventoryItem {
        item(
            Some("database-platform"),
            labels(&[("cost-center", "cc-42")]),
            true,
            120,
            0,
            0,
            None,
            Some(64),
            Some(200),
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
    fn cost_flags_missing_owner_and_lock_coverage() {
        let target = item(
            Some(""),
            BTreeMap::new(),
            false,
            0,
            0,
            0,
            None,
            Some(64),
            Some(100),
            Some("PostgreSQL 16.2"),
            true,
            now(),
        );

        let report = evaluate_postgres_pg_locks_inventory(&[target], Pillar::Cost, now());
        let codes = reason_codes(&report);

        assert!(codes.contains(&REASON_COST_OWNER_NOT_RECORDED));
        assert!(codes.contains(&REASON_COST_LOCK_COVERAGE_MISSING));
    }

    #[test]
    fn cost_flags_high_lock_volume() {
        let target = item(
            Some("database-platform"),
            labels(&[("owner", "database-platform")]),
            true,
            1_100,
            0,
            0,
            None,
            Some(64),
            Some(100),
            Some("PostgreSQL 16.2"),
            true,
            now(),
        );

        let report = evaluate_postgres_pg_locks_inventory(&[target], Pillar::Cost, now());

        assert!(reason_codes(&report).contains(&REASON_COST_HIGH_LOCK_VOLUME));
    }

    #[test]
    fn resilience_flags_unavailable_waiting_and_lock_table_pressure() {
        let unavailable = item(
            Some("database-platform"),
            labels(&[("owner", "database-platform")]),
            false,
            0,
            0,
            0,
            None,
            Some(64),
            Some(100),
            Some("PostgreSQL 16.2"),
            true,
            now(),
        );
        let pressured = item(
            Some("database-platform"),
            labels(&[("owner", "database-platform")]),
            true,
            5_200,
            4,
            2,
            Some(45),
            Some(64),
            Some(100),
            Some("PostgreSQL 16.2"),
            true,
            now(),
        );

        let report = evaluate_postgres_pg_locks_inventory(
            &[unavailable, pressured],
            Pillar::Resilience,
            now(),
        );
        let codes = reason_codes(&report);

        assert!(codes.contains(&REASON_RES_LOCK_COVERAGE_UNAVAILABLE));
        assert!(codes.contains(&REASON_RES_WAITING_OR_BLOCKED_LOCKS));
        assert!(codes.contains(&REASON_RES_LOCK_TABLE_PRESSURE));
    }

    #[test]
    fn security_flags_missing_version_and_security_evidence() {
        let target = item(
            Some("database-platform"),
            labels(&[("owner", "database-platform")]),
            true,
            100,
            0,
            0,
            None,
            Some(64),
            Some(100),
            None,
            false,
            now(),
        );

        let report = evaluate_postgres_pg_locks_inventory(&[target], Pillar::Security, now());
        let codes = reason_codes(&report);

        assert!(codes.contains(&REASON_SEC_VERSION_NOT_RECORDED));
        assert!(codes.contains(&REASON_SEC_SECURITY_EVIDENCE_MISSING));
    }

    #[test]
    fn stale_inventory_is_counted_for_any_pillar() {
        let target = PgLocksInventoryItem {
            collected_at: now() - Duration::hours(25),
            ..healthy_item()
        };

        let report = evaluate_postgres_pg_locks_inventory(&[target], Pillar::Cost, now());

        assert_eq!(report.stale_resources, 1);
        assert!(reason_codes(&report).contains(&REASON_INV_STALE_DATA));
    }

    #[test]
    fn healthy_pg_locks_passes_claimed_pillars() {
        let target = healthy_item();

        for pillar in [Pillar::Cost, Pillar::Resilience, Pillar::Security] {
            let report =
                evaluate_postgres_pg_locks_inventory(std::slice::from_ref(&target), pillar, now());
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

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

// Deterministic PostgreSQL pg_stat_activity inventory evaluator for roadmap rows
// 05-POSTGRES-00001/00008/00029.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::services::aws::inventory::types::{
    score_pillar, InventoryFinding, Pillar, PillarReport, Severity, COST_ALLOCATION_TAG_KEYS,
    DEFAULT_STALE_AFTER_HOURS,
};

pub const RESOURCE_TYPE: &str = "PostgresPgStatActivity";
pub const REASON_COST_OWNER_NOT_RECORDED: &str =
    "POSTGRES_PG_STAT_ACTIVITY_COST_OWNER_NOT_RECORDED";
pub const REASON_COST_ACTIVITY_COVERAGE_MISSING: &str =
    "POSTGRES_PG_STAT_ACTIVITY_COST_ACTIVITY_COVERAGE_MISSING";
pub const REASON_RES_CONNECTION_PRESSURE: &str =
    "POSTGRES_PG_STAT_ACTIVITY_RES_CONNECTION_PRESSURE";
pub const REASON_RES_BLOCKED_SESSIONS: &str = "POSTGRES_PG_STAT_ACTIVITY_RES_BLOCKED_SESSIONS";
pub const REASON_SEC_VERSION_NOT_RECORDED: &str =
    "POSTGRES_PG_STAT_ACTIVITY_SEC_VERSION_NOT_RECORDED";
pub const REASON_SEC_SSL_COVERAGE_UNKNOWN: &str =
    "POSTGRES_PG_STAT_ACTIVITY_SEC_SSL_COVERAGE_UNKNOWN";
pub const REASON_INV_STALE_DATA: &str = "POSTGRES_PG_STAT_ACTIVITY_INV_STALE_DATA";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PgStatActivityInventoryItem {
    pub connection_id: String,
    pub connection_name: String,
    pub owner: Option<String>,
    pub labels: BTreeMap<String, String>,
    pub server_version: Option<String>,
    pub total_sessions: usize,
    pub active_sessions: usize,
    pub idle_sessions: usize,
    pub idle_in_transaction_sessions: usize,
    pub blocked_sessions: usize,
    pub max_connections: Option<usize>,
    pub ssl_sessions: Option<usize>,
    pub collected_at: DateTime<Utc>,
}

pub fn evaluate_postgres_pg_stat_activity_inventory(
    items: &[PgStatActivityInventoryItem],
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
    item: &PgStatActivityInventoryItem,
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
                "PostgreSQL pg_stat_activity inventory for connection {} has no owner, team, project, or cost-center metadata",
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

    if item.total_sessions == 0 {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_ACTIVITY_COVERAGE_MISSING,
            Severity::High,
            format!(
                "PostgreSQL pg_stat_activity inventory for connection {} has no session coverage",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "connection_name": item.connection_name,
                "total_sessions": item.total_sessions,
                "recommendation": "Collect pg_stat_activity session evidence before attributing idle or active connection cost",
            }),
        ));
    }
}

fn evaluate_resilience(
    item: &PgStatActivityInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if let Some(max_connections) = item.max_connections {
        if max_connections > 0 {
            let usage_pct = (item.total_sessions as f64 / max_connections as f64) * 100.0;
            if usage_pct >= 80.0 {
                findings.push(finding(
                    item,
                    pillar,
                    REASON_RES_CONNECTION_PRESSURE,
                    Severity::High,
                    format!(
                        "PostgreSQL connection {} is using {:.1}% of max_connections",
                        item.connection_name, usage_pct
                    ),
                    json!({
                        "connection_id": item.connection_id,
                        "connection_name": item.connection_name,
                        "total_sessions": item.total_sessions,
                        "max_connections": max_connections,
                        "usage_pct": usage_pct,
                        "recommendation": "Triage connection pool sizing and long-lived sessions before connection exhaustion affects availability",
                    }),
                ));
            }
        }
    }

    if item.blocked_sessions > 0 || item.idle_in_transaction_sessions > 0 {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_BLOCKED_SESSIONS,
            Severity::Medium,
            format!(
                "PostgreSQL pg_stat_activity inventory for connection {} has blocked or idle-in-transaction sessions",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "connection_name": item.connection_name,
                "blocked_sessions": item.blocked_sessions,
                "idle_in_transaction_sessions": item.idle_in_transaction_sessions,
                "recommendation": "Investigate blocked sessions and idle transactions before they create lock queues or vacuum lag",
            }),
        ));
    }
}

fn evaluate_security(
    item: &PgStatActivityInventoryItem,
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
                "PostgreSQL pg_stat_activity inventory for connection {} has no recorded server version",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "connection_name": item.connection_name,
                "server_version": item.server_version,
                "recommendation": "Record PostgreSQL server version with pg_stat_activity evidence so advisories can be mapped deterministically",
            }),
        ));
    }

    if item.ssl_sessions.is_none() {
        findings.push(finding(
            item,
            pillar,
            REASON_SEC_SSL_COVERAGE_UNKNOWN,
            Severity::Medium,
            format!(
                "PostgreSQL pg_stat_activity inventory for connection {} has no SSL session coverage",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "connection_name": item.connection_name,
                "ssl_sessions": item.ssl_sessions,
                "recommendation": "Join pg_stat_activity with pg_stat_ssl or equivalent connection security evidence before relying on session inventory for security posture",
            }),
        ));
    }
}

fn stale_finding(
    item: &PgStatActivityInventoryItem,
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
            "Inventory data for PostgreSQL pg_stat_activity connection {} is {} hours old (threshold {} hours)",
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
    item: &PgStatActivityInventoryItem,
    pillar: Pillar,
    reason_code: &str,
    severity: Severity,
    message: String,
    evidence: Value,
) -> InventoryFinding {
    InventoryFinding {
        resource_id: item.connection_id.clone(),
        arn: format!("postgres://pg-stat-activity/{}", item.connection_id),
        pillar,
        reason_code: reason_code.to_string(),
        severity,
        message,
        evidence,
    }
}

fn has_owner_metadata(item: &PgStatActivityInventoryItem) -> bool {
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
        total_sessions: usize,
        idle_in_transaction_sessions: usize,
        blocked_sessions: usize,
        max_connections: Option<usize>,
        ssl_sessions: Option<usize>,
        labels: BTreeMap<String, String>,
        collected_at: DateTime<Utc>,
    ) -> PgStatActivityInventoryItem {
        PgStatActivityInventoryItem {
            connection_id: "postgres-1".to_string(),
            connection_name: "orders-postgres".to_string(),
            owner: owner.map(str::to_string),
            labels,
            server_version: server_version.map(str::to_string),
            total_sessions,
            active_sessions: total_sessions.saturating_sub(idle_in_transaction_sessions),
            idle_sessions: idle_in_transaction_sessions,
            idle_in_transaction_sessions,
            blocked_sessions,
            max_connections,
            ssl_sessions,
            collected_at,
        }
    }

    fn healthy_item() -> PgStatActivityInventoryItem {
        item(
            Some("database-platform"),
            Some("PostgreSQL 16.2"),
            12,
            0,
            0,
            Some(200),
            Some(12),
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
    fn cost_flags_missing_owner_and_activity_coverage() {
        let target = item(
            Some(""),
            Some("PostgreSQL 16.2"),
            0,
            0,
            0,
            Some(100),
            Some(0),
            BTreeMap::new(),
            now(),
        );

        let report = evaluate_postgres_pg_stat_activity_inventory(&[target], Pillar::Cost, now());
        let codes = reason_codes(&report);

        assert!(codes.contains(&REASON_COST_OWNER_NOT_RECORDED));
        assert!(codes.contains(&REASON_COST_ACTIVITY_COVERAGE_MISSING));
    }

    #[test]
    fn resilience_flags_connection_pressure_and_blocking() {
        let target = item(
            Some("database-platform"),
            Some("PostgreSQL 16.2"),
            90,
            2,
            1,
            Some(100),
            Some(90),
            labels(&[("owner", "database-platform")]),
            now(),
        );

        let report =
            evaluate_postgres_pg_stat_activity_inventory(&[target], Pillar::Resilience, now());
        let codes = reason_codes(&report);

        assert!(codes.contains(&REASON_RES_CONNECTION_PRESSURE));
        assert!(codes.contains(&REASON_RES_BLOCKED_SESSIONS));
    }

    #[test]
    fn security_flags_missing_version_and_ssl_coverage() {
        let target = item(
            Some("database-platform"),
            None,
            10,
            0,
            0,
            Some(100),
            None,
            labels(&[("owner", "database-platform")]),
            now(),
        );

        let report =
            evaluate_postgres_pg_stat_activity_inventory(&[target], Pillar::Security, now());
        let codes = reason_codes(&report);

        assert!(codes.contains(&REASON_SEC_VERSION_NOT_RECORDED));
        assert!(codes.contains(&REASON_SEC_SSL_COVERAGE_UNKNOWN));
    }

    #[test]
    fn stale_inventory_is_counted_for_any_pillar() {
        let target = item(
            Some("database-platform"),
            Some("PostgreSQL 16.2"),
            10,
            0,
            0,
            Some(100),
            Some(10),
            labels(&[("owner", "database-platform")]),
            now() - Duration::hours(25),
        );

        let report = evaluate_postgres_pg_stat_activity_inventory(&[target], Pillar::Cost, now());

        assert_eq!(report.stale_resources, 1);
        assert!(reason_codes(&report).contains(&REASON_INV_STALE_DATA));
    }

    #[test]
    fn healthy_pg_stat_activity_passes_claimed_pillars() {
        let target = healthy_item();

        for pillar in [Pillar::Cost, Pillar::Resilience, Pillar::Security] {
            let report = evaluate_postgres_pg_stat_activity_inventory(
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

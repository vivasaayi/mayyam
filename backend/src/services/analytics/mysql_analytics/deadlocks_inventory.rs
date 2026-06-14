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

// Deterministic deadlocks inventory evaluator for roadmap rows
// 03-MYSQL-AI-TRIAGER-00736/00743/00764.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::services::analytics::mysql_analytics::mysql_telemetry::MySqlTelemetrySnapshot;
use crate::services::aws::inventory::types::{
    score_pillar, InventoryFinding, Pillar, PillarReport, Severity, COST_ALLOCATION_TAG_KEYS,
    DEFAULT_STALE_AFTER_HOURS,
};

pub const RESOURCE_TYPE: &str = "MySqlDeadlocks";
pub const REASON_COST_OWNER_NOT_RECORDED: &str = "MYSQL_DEADLOCKS_COST_OWNER_NOT_RECORDED";
pub const REASON_COST_NO_DEADLOCK_METRICS: &str = "MYSQL_DEADLOCKS_COST_NO_METRICS";
pub const REASON_COST_DEADLOCK_WASTE: &str = "MYSQL_DEADLOCKS_COST_DEADLOCK_WASTE";
pub const REASON_RES_NO_DEADLOCK_METRICS: &str = "MYSQL_DEADLOCKS_RES_NO_METRICS";
pub const REASON_RES_DEADLOCKS_DETECTED: &str = "MYSQL_DEADLOCKS_RES_DEADLOCKS_DETECTED";
pub const REASON_RES_LOCK_WAIT_PRESSURE: &str = "MYSQL_DEADLOCKS_RES_LOCK_WAIT_PRESSURE";
pub const REASON_SEC_NO_DEADLOCK_METRICS: &str = "MYSQL_DEADLOCKS_SEC_NO_METRICS";
pub const REASON_SEC_DEADLOCK_REVIEW: &str = "MYSQL_DEADLOCKS_SEC_DEADLOCK_REVIEW";
pub const REASON_INV_STALE_DATA: &str = "MYSQL_DEADLOCKS_INV_STALE_DATA";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeadlocksInventoryItem {
    pub connection_id: String,
    pub connection_name: String,
    pub owner: Option<String>,
    pub labels: BTreeMap<String, String>,
    pub deadlock_metric_count: usize,
    pub deadlocks: i64,
    pub row_lock_waits: i64,
    pub row_lock_time_ms: i64,
    pub blocked_processes: i64,
    pub pending_metadata_locks: Option<i64>,
    pub collected_at: DateTime<Utc>,
}

pub fn evaluate_mysql_deadlocks_inventory(
    items: &[DeadlocksInventoryItem],
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

pub fn deadlocks_item_from_telemetry(
    connection_id: &str,
    connection_name: &str,
    owner: Option<String>,
    labels: BTreeMap<String, String>,
    snapshot: &MySqlTelemetrySnapshot,
) -> DeadlocksInventoryItem {
    DeadlocksInventoryItem {
        connection_id: connection_id.to_string(),
        connection_name: connection_name.to_string(),
        owner,
        labels,
        deadlock_metric_count: 4 + usize::from(snapshot.locks.pending_metadata_locks.is_some()),
        deadlocks: snapshot.innodb.deadlocks,
        row_lock_waits: snapshot.innodb.row_lock_waits,
        row_lock_time_ms: snapshot.innodb.row_lock_time_ms,
        blocked_processes: snapshot.locks.blocked_processes,
        pending_metadata_locks: snapshot.locks.pending_metadata_locks,
        collected_at: snapshot.collected_at,
    }
}

fn evaluate_cost(
    item: &DeadlocksInventoryItem,
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
                "Deadlock inventory for {} has no owner, team, project, or cost-center metadata",
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

    if !has_deadlock_metrics(item) {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_NO_DEADLOCK_METRICS,
            Severity::High,
            format!(
                "Deadlock inventory for {} has no collected deadlock metrics",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "connection_name": item.connection_name,
                "deadlock_metric_count": item.deadlock_metric_count,
                "recommendation": "Collect InnoDB deadlock, row-lock wait, blocked process, and metadata-lock evidence before estimating transaction retry waste or scaling spend",
            }),
        ));
    }

    if has_deadlock_waste(item) {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_DEADLOCK_WASTE,
            Severity::Medium,
            format!(
                "Deadlock activity for {} is creating retry or wait waste",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "connection_name": item.connection_name,
                "deadlocks": item.deadlocks,
                "row_lock_waits": item.row_lock_waits,
                "row_lock_time_ms": item.row_lock_time_ms,
                "blocked_processes": item.blocked_processes,
                "pending_metadata_locks": pending_metadata_locks(item),
                "recommendation": "Fix transaction ordering, long-running transactions, and lock hot spots before scaling capacity to absorb deadlock retries",
            }),
        ));
    }
}

fn evaluate_resilience(
    item: &DeadlocksInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if !has_deadlock_metrics(item) {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_NO_DEADLOCK_METRICS,
            Severity::High,
            format!(
                "Deadlock inventory for {} has no resilience evidence",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "connection_name": item.connection_name,
                "deadlock_metric_count": item.deadlock_metric_count,
                "recommendation": "Collect deadlock and row-lock wait counters so transaction failure risk can be evaluated deterministically",
            }),
        ));
    }

    if has_deadlocks(item) {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_DEADLOCKS_DETECTED,
            Severity::High,
            format!("Deadlocks are occurring for {}", item.connection_name),
            json!({
                "connection_id": item.connection_id,
                "connection_name": item.connection_name,
                "deadlocks": item.deadlocks,
                "blocked_processes": item.blocked_processes,
                "pending_metadata_locks": pending_metadata_locks(item),
                "recommendation": "Inspect deadlock traces, transaction ordering, affected statements, and retry behavior before broad incident escalation or failover",
            }),
        ));
    }

    if has_lock_wait_pressure(item) {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_LOCK_WAIT_PRESSURE,
            Severity::Medium,
            format!(
                "Lock wait pressure is present alongside deadlock evidence for {}",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "connection_name": item.connection_name,
                "row_lock_waits": item.row_lock_waits,
                "row_lock_time_ms": item.row_lock_time_ms,
                "blocked_processes": item.blocked_processes,
                "recommendation": "Correlate row-lock waits, blocked sessions, and recent workload changes before retrying migrations or high-write jobs",
            }),
        ));
    }
}

fn evaluate_security(
    item: &DeadlocksInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if !has_deadlock_metrics(item) {
        findings.push(finding(
            item,
            pillar,
            REASON_SEC_NO_DEADLOCK_METRICS,
            Severity::High,
            format!(
                "Deadlock inventory for {} has no scoped security evidence",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "connection_name": item.connection_name,
                "deadlock_metric_count": item.deadlock_metric_count,
                "recommendation": "Collect deadlock and lock-wait evidence so unusual write conflicts can be reviewed without ad hoc privileged diagnostics",
            }),
        ));
    }

    if needs_deadlock_review(item) {
        findings.push(finding(
            item,
            pillar,
            REASON_SEC_DEADLOCK_REVIEW,
            Severity::Medium,
            format!(
                "Deadlock activity for {} should be reviewed",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "connection_name": item.connection_name,
                "deadlocks": item.deadlocks,
                "row_lock_waits": item.row_lock_waits,
                "blocked_processes": item.blocked_processes,
                "recommendation": "Review recent write paths, migration actors, privileged sessions, and workload ownership before treating deadlocks as expected application behavior",
            }),
        ));
    }
}

fn stale_finding(
    item: &DeadlocksInventoryItem,
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
            "Inventory data for deadlocks resource {} is {} hours old (threshold {} hours)",
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
    item: &DeadlocksInventoryItem,
    pillar: Pillar,
    reason_code: &str,
    severity: Severity,
    message: String,
    evidence: Value,
) -> InventoryFinding {
    InventoryFinding {
        resource_id: item.connection_id.clone(),
        arn: format!("mysql://deadlocks/{}", item.connection_id),
        pillar,
        reason_code: reason_code.to_string(),
        severity,
        message,
        evidence,
    }
}

fn has_owner_metadata(item: &DeadlocksInventoryItem) -> bool {
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

fn has_deadlock_metrics(item: &DeadlocksInventoryItem) -> bool {
    item.deadlock_metric_count > 0
}

fn pending_metadata_locks(item: &DeadlocksInventoryItem) -> i64 {
    item.pending_metadata_locks.unwrap_or(0)
}

fn has_deadlocks(item: &DeadlocksInventoryItem) -> bool {
    has_deadlock_metrics(item) && item.deadlocks > 0
}

fn has_lock_wait_pressure(item: &DeadlocksInventoryItem) -> bool {
    has_deadlock_metrics(item)
        && (item.row_lock_waits >= 10
            || item.row_lock_time_ms >= 10_000
            || item.blocked_processes > 0
            || pending_metadata_locks(item) > 0)
}

fn has_deadlock_waste(item: &DeadlocksInventoryItem) -> bool {
    has_deadlock_metrics(item)
        && (item.deadlocks > 0 || item.row_lock_time_ms >= 60_000 || item.blocked_processes > 0)
}

fn needs_deadlock_review(item: &DeadlocksInventoryItem) -> bool {
    has_deadlock_metrics(item) && (item.deadlocks > 0 || item.blocked_processes > 0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{DateTime, Duration, Utc};
    use std::collections::BTreeMap;

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

    #[allow(clippy::too_many_arguments)]
    fn item(
        owner: Option<&str>,
        deadlock_metric_count: usize,
        deadlocks: i64,
        row_lock_waits: i64,
        row_lock_time_ms: i64,
        blocked_processes: i64,
        pending_metadata_locks: Option<i64>,
        labels: BTreeMap<String, String>,
        collected_at: DateTime<Utc>,
    ) -> DeadlocksInventoryItem {
        DeadlocksInventoryItem {
            connection_id: "mysql-1".to_string(),
            connection_name: "orders-mysql".to_string(),
            owner: owner.map(str::to_string),
            labels,
            deadlock_metric_count,
            deadlocks,
            row_lock_waits,
            row_lock_time_ms,
            blocked_processes,
            pending_metadata_locks,
            collected_at,
        }
    }

    fn healthy_item() -> DeadlocksInventoryItem {
        item(
            Some("database-platform"),
            4,
            0,
            0,
            0,
            0,
            Some(0),
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
    fn cost_flags_missing_owner_missing_metrics_and_deadlock_waste() {
        let missing_metrics = item(Some(""), 0, 0, 0, 0, 0, None, BTreeMap::new(), now());
        let deadlock_pressure = item(
            Some("database-platform"),
            4,
            3,
            40,
            90_000,
            2,
            Some(1),
            labels(&[("owner", "database-platform")]),
            now(),
        );

        let report = evaluate_mysql_deadlocks_inventory(
            &[missing_metrics, deadlock_pressure],
            Pillar::Cost,
            now(),
        );
        let codes = reason_codes(&report);

        assert!(codes.contains(&REASON_COST_OWNER_NOT_RECORDED));
        assert!(codes.contains(&REASON_COST_NO_DEADLOCK_METRICS));
        assert!(codes.contains(&REASON_COST_DEADLOCK_WASTE));
    }

    #[test]
    fn resilience_flags_missing_metrics_and_deadlock_pressure() {
        let missing_metrics = item(
            Some("database-platform"),
            0,
            0,
            0,
            0,
            0,
            None,
            labels(&[("owner", "database-platform")]),
            now(),
        );
        let deadlock_pressure = item(
            Some("database-platform"),
            4,
            2,
            30,
            45_000,
            1,
            Some(1),
            labels(&[("owner", "database-platform")]),
            now(),
        );

        let report = evaluate_mysql_deadlocks_inventory(
            &[missing_metrics, deadlock_pressure],
            Pillar::Resilience,
            now(),
        );
        let codes = reason_codes(&report);

        assert!(codes.contains(&REASON_RES_NO_DEADLOCK_METRICS));
        assert!(codes.contains(&REASON_RES_DEADLOCKS_DETECTED));
        assert!(codes.contains(&REASON_RES_LOCK_WAIT_PRESSURE));
    }

    #[test]
    fn security_flags_missing_metrics_and_unreviewed_deadlocks() {
        let missing_metrics = item(
            Some("database-platform"),
            0,
            0,
            0,
            0,
            0,
            None,
            labels(&[("owner", "database-platform")]),
            now(),
        );
        let deadlock_pressure = item(
            Some("database-platform"),
            4,
            1,
            10,
            12_000,
            1,
            Some(0),
            labels(&[("owner", "database-platform")]),
            now(),
        );

        let report = evaluate_mysql_deadlocks_inventory(
            &[missing_metrics, deadlock_pressure],
            Pillar::Security,
            now(),
        );
        let codes = reason_codes(&report);

        assert!(codes.contains(&REASON_SEC_NO_DEADLOCK_METRICS));
        assert!(codes.contains(&REASON_SEC_DEADLOCK_REVIEW));
    }

    #[test]
    fn stale_inventory_is_counted_for_any_pillar() {
        let stale = item(
            Some("database-platform"),
            4,
            0,
            0,
            0,
            0,
            Some(0),
            labels(&[("owner", "database-platform")]),
            now() - Duration::hours(49),
        );

        let report = evaluate_mysql_deadlocks_inventory(&[stale], Pillar::Resilience, now());

        assert_eq!(report.stale_resources, 1);
        assert!(reason_codes(&report).contains(&REASON_INV_STALE_DATA));
    }

    #[test]
    fn healthy_deadlocks_pass_claimed_pillars() {
        for pillar in [Pillar::Cost, Pillar::Resilience, Pillar::Security] {
            let report = evaluate_mysql_deadlocks_inventory(&[healthy_item()], pillar, now());

            assert_eq!(report.resources_evaluated, 1);
            assert_eq!(report.stale_resources, 0);
            assert!(report.findings.is_empty());
            assert!(report.score >= 99);
        }
    }
}

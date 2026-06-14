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

// Deterministic metadata locks inventory evaluator for roadmap rows
// 03-MYSQL-AI-TRIAGER-00687/00694/00715.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::services::analytics::mysql_analytics::mysql_telemetry::MySqlTelemetrySnapshot;
use crate::services::aws::inventory::types::{
    score_pillar, InventoryFinding, Pillar, PillarReport, Severity, COST_ALLOCATION_TAG_KEYS,
    DEFAULT_STALE_AFTER_HOURS,
};

pub const RESOURCE_TYPE: &str = "MySqlMetadataLocks";
pub const REASON_COST_OWNER_NOT_RECORDED: &str = "MYSQL_METADATA_LOCKS_COST_OWNER_NOT_RECORDED";
pub const REASON_COST_NO_LOCK_METRICS: &str = "MYSQL_METADATA_LOCKS_COST_NO_LOCK_METRICS";
pub const REASON_COST_BLOCKING_WASTE: &str = "MYSQL_METADATA_LOCKS_COST_BLOCKING_WASTE";
pub const REASON_RES_NO_LOCK_METRICS: &str = "MYSQL_METADATA_LOCKS_RES_NO_LOCK_METRICS";
pub const REASON_RES_PENDING_LOCKS: &str = "MYSQL_METADATA_LOCKS_RES_PENDING_LOCKS";
pub const REASON_RES_BLOCKED_PROCESSES: &str = "MYSQL_METADATA_LOCKS_RES_BLOCKED_PROCESSES";
pub const REASON_SEC_NO_LOCK_METRICS: &str = "MYSQL_METADATA_LOCKS_SEC_NO_LOCK_METRICS";
pub const REASON_SEC_DDL_LOCK_REVIEW: &str = "MYSQL_METADATA_LOCKS_SEC_DDL_LOCK_REVIEW";
pub const REASON_INV_STALE_DATA: &str = "MYSQL_METADATA_LOCKS_INV_STALE_DATA";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetadataLocksInventoryItem {
    pub connection_id: String,
    pub connection_name: String,
    pub owner: Option<String>,
    pub labels: BTreeMap<String, String>,
    pub lock_metric_count: usize,
    pub pending_metadata_locks: Option<i64>,
    pub blocked_processes: i64,
    pub data_lock_waits: Option<i64>,
    pub threads_running: i64,
    pub collected_at: DateTime<Utc>,
}

pub fn evaluate_mysql_metadata_locks_inventory(
    items: &[MetadataLocksInventoryItem],
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

pub fn metadata_locks_item_from_telemetry(
    connection_id: &str,
    connection_name: &str,
    owner: Option<String>,
    labels: BTreeMap<String, String>,
    snapshot: &MySqlTelemetrySnapshot,
) -> MetadataLocksInventoryItem {
    MetadataLocksInventoryItem {
        connection_id: connection_id.to_string(),
        connection_name: connection_name.to_string(),
        owner,
        labels,
        lock_metric_count: 1
            + usize::from(snapshot.locks.pending_metadata_locks.is_some())
            + usize::from(snapshot.locks.data_lock_waits.is_some()),
        pending_metadata_locks: snapshot.locks.pending_metadata_locks,
        blocked_processes: snapshot.locks.blocked_processes,
        data_lock_waits: snapshot.locks.data_lock_waits,
        threads_running: snapshot.connections.threads_running,
        collected_at: snapshot.collected_at,
    }
}

fn evaluate_cost(
    item: &MetadataLocksInventoryItem,
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
                "Metadata lock inventory for {} has no owner, team, project, or cost-center metadata",
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

    if !has_lock_metrics(item) {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_NO_LOCK_METRICS,
            Severity::High,
            format!(
                "Metadata lock inventory for {} has no collected lock metrics",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "connection_name": item.connection_name,
                "lock_metric_count": item.lock_metric_count,
                "recommendation": "Collect pending metadata lock, blocked process, data lock wait, and running thread evidence before estimating lock-related toil or capacity waste",
            }),
        ));
    }

    if has_blocking_waste(item) {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_BLOCKING_WASTE,
            Severity::Medium,
            format!(
                "Metadata locks for {} are causing blocking waste",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "connection_name": item.connection_name,
                "pending_metadata_locks": pending_metadata_locks(item),
                "blocked_processes": item.blocked_processes,
                "data_lock_waits": item.data_lock_waits,
                "threads_running": item.threads_running,
                "recommendation": "Identify DDL, long transactions, and blocked sessions before scaling resources to compensate for lock-driven throughput loss",
            }),
        ));
    }
}

fn evaluate_resilience(
    item: &MetadataLocksInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if !has_lock_metrics(item) {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_NO_LOCK_METRICS,
            Severity::High,
            format!(
                "Metadata lock inventory for {} has no resilience evidence",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "connection_name": item.connection_name,
                "lock_metric_count": item.lock_metric_count,
                "recommendation": "Collect metadata lock and blocked process evidence so schema-change blast radius and availability risk can be evaluated deterministically",
            }),
        ));
    }

    if has_pending_metadata_locks(item) {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_PENDING_LOCKS,
            Severity::High,
            format!(
                "Metadata locks are pending for {}",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "connection_name": item.connection_name,
                "pending_metadata_locks": pending_metadata_locks(item),
                "threads_running": item.threads_running,
                "recommendation": "Inspect waiting metadata locks, active DDL, and long transactions before continuing schema changes or deployments",
            }),
        ));
    }

    if has_blocked_processes(item) {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_BLOCKED_PROCESSES,
            Severity::High,
            format!(
                "Metadata locks are blocking processes for {}",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "connection_name": item.connection_name,
                "blocked_processes": item.blocked_processes,
                "data_lock_waits": item.data_lock_waits,
                "recommendation": "Identify blockers and waiters, pause risky migrations, and verify application impact before retrying DDL or lock-heavy maintenance",
            }),
        ));
    }
}

fn evaluate_security(
    item: &MetadataLocksInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if !has_lock_metrics(item) {
        findings.push(finding(
            item,
            pillar,
            REASON_SEC_NO_LOCK_METRICS,
            Severity::High,
            format!(
                "Metadata lock inventory for {} has no scoped security evidence",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "connection_name": item.connection_name,
                "lock_metric_count": item.lock_metric_count,
                "recommendation": "Collect metadata lock evidence so unexpected DDL or privileged maintenance activity can be reviewed without ad hoc privileged diagnostics",
            }),
        ));
    }

    if needs_ddl_lock_review(item) {
        findings.push(finding(
            item,
            pillar,
            REASON_SEC_DDL_LOCK_REVIEW,
            Severity::Medium,
            format!(
                "Metadata lock activity for {} should be reviewed",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "connection_name": item.connection_name,
                "pending_metadata_locks": pending_metadata_locks(item),
                "blocked_processes": item.blocked_processes,
                "data_lock_waits": item.data_lock_waits,
                "recommendation": "Review recent DDL, migration actor, change approval, and privileged session evidence before treating lock pressure as normal workload behavior",
            }),
        ));
    }
}

fn stale_finding(
    item: &MetadataLocksInventoryItem,
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
            "Inventory data for metadata locks resource {} is {} hours old (threshold {} hours)",
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
    item: &MetadataLocksInventoryItem,
    pillar: Pillar,
    reason_code: &str,
    severity: Severity,
    message: String,
    evidence: Value,
) -> InventoryFinding {
    InventoryFinding {
        resource_id: item.connection_id.clone(),
        arn: format!("mysql://metadata-locks/{}", item.connection_id),
        pillar,
        reason_code: reason_code.to_string(),
        severity,
        message,
        evidence,
    }
}

fn has_owner_metadata(item: &MetadataLocksInventoryItem) -> bool {
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

fn has_lock_metrics(item: &MetadataLocksInventoryItem) -> bool {
    item.lock_metric_count > 0
}

fn pending_metadata_locks(item: &MetadataLocksInventoryItem) -> i64 {
    item.pending_metadata_locks.unwrap_or(0)
}

fn data_lock_waits(item: &MetadataLocksInventoryItem) -> i64 {
    item.data_lock_waits.unwrap_or(0)
}

fn has_blocking_waste(item: &MetadataLocksInventoryItem) -> bool {
    has_lock_metrics(item)
        && (pending_metadata_locks(item) >= 10
            || item.blocked_processes >= 2
            || data_lock_waits(item) >= 5)
}

fn has_pending_metadata_locks(item: &MetadataLocksInventoryItem) -> bool {
    has_lock_metrics(item) && pending_metadata_locks(item) > 0
}

fn has_blocked_processes(item: &MetadataLocksInventoryItem) -> bool {
    has_lock_metrics(item) && (item.blocked_processes > 0 || data_lock_waits(item) > 0)
}

fn needs_ddl_lock_review(item: &MetadataLocksInventoryItem) -> bool {
    has_lock_metrics(item) && (pending_metadata_locks(item) > 0 || item.blocked_processes > 0)
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

    #[allow(clippy::too_many_arguments)]
    fn item(
        owner: Option<&str>,
        lock_metric_count: usize,
        pending_metadata_locks: Option<i64>,
        blocked_processes: i64,
        data_lock_waits: Option<i64>,
        threads_running: i64,
        labels: BTreeMap<String, String>,
        collected_at: DateTime<Utc>,
    ) -> MetadataLocksInventoryItem {
        MetadataLocksInventoryItem {
            connection_id: "mysql-1".to_string(),
            connection_name: "orders-mysql".to_string(),
            owner: owner.map(str::to_string),
            labels,
            lock_metric_count,
            pending_metadata_locks,
            blocked_processes,
            data_lock_waits,
            threads_running,
            collected_at,
        }
    }

    fn healthy_item() -> MetadataLocksInventoryItem {
        item(
            Some("database-platform"),
            3,
            Some(0),
            0,
            Some(0),
            4,
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
    fn cost_flags_missing_owner_missing_metrics_and_blocking_waste() {
        let missing_metrics = item(Some(""), 0, None, 0, None, 0, BTreeMap::new(), now());
        let blocking = item(
            Some("database-platform"),
            3,
            Some(12),
            4,
            Some(6),
            40,
            labels(&[("owner", "database-platform")]),
            now(),
        );

        let report = evaluate_mysql_metadata_locks_inventory(
            &[missing_metrics, blocking],
            Pillar::Cost,
            now(),
        );
        let codes = reason_codes(&report);

        assert!(codes.contains(&REASON_COST_OWNER_NOT_RECORDED));
        assert!(codes.contains(&REASON_COST_NO_LOCK_METRICS));
        assert!(codes.contains(&REASON_COST_BLOCKING_WASTE));
    }

    #[test]
    fn resilience_flags_missing_metrics_pending_locks_and_blocked_processes() {
        let missing_metrics = item(
            Some("database-platform"),
            0,
            None,
            0,
            None,
            0,
            labels(&[("owner", "database-platform")]),
            now(),
        );
        let blocking = item(
            Some("database-platform"),
            3,
            Some(5),
            2,
            Some(3),
            20,
            labels(&[("owner", "database-platform")]),
            now(),
        );

        let report = evaluate_mysql_metadata_locks_inventory(
            &[missing_metrics, blocking],
            Pillar::Resilience,
            now(),
        );
        let codes = reason_codes(&report);

        assert!(codes.contains(&REASON_RES_NO_LOCK_METRICS));
        assert!(codes.contains(&REASON_RES_PENDING_LOCKS));
        assert!(codes.contains(&REASON_RES_BLOCKED_PROCESSES));
    }

    #[test]
    fn security_flags_missing_metrics_and_ddl_lock_review() {
        let missing_metrics = item(
            Some("database-platform"),
            0,
            None,
            0,
            None,
            0,
            labels(&[("owner", "database-platform")]),
            now(),
        );
        let ddl_pressure = item(
            Some("database-platform"),
            3,
            Some(3),
            1,
            Some(1),
            8,
            labels(&[("owner", "database-platform")]),
            now(),
        );

        let report = evaluate_mysql_metadata_locks_inventory(
            &[missing_metrics, ddl_pressure],
            Pillar::Security,
            now(),
        );
        let codes = reason_codes(&report);

        assert!(codes.contains(&REASON_SEC_NO_LOCK_METRICS));
        assert!(codes.contains(&REASON_SEC_DDL_LOCK_REVIEW));
    }

    #[test]
    fn stale_inventory_is_counted_for_any_pillar() {
        let stale = item(
            Some("database-platform"),
            3,
            Some(0),
            0,
            Some(0),
            4,
            labels(&[("owner", "database-platform")]),
            now() - Duration::hours(49),
        );

        let report = evaluate_mysql_metadata_locks_inventory(&[stale], Pillar::Resilience, now());

        assert_eq!(report.stale_resources, 1);
        assert!(reason_codes(&report).contains(&REASON_INV_STALE_DATA));
    }

    #[test]
    fn healthy_metadata_locks_pass_claimed_pillars() {
        for pillar in [Pillar::Cost, Pillar::Resilience, Pillar::Security] {
            let report = evaluate_mysql_metadata_locks_inventory(&[healthy_item()], pillar, now());

            assert_eq!(report.resources_evaluated, 1);
            assert_eq!(report.stale_resources, 0);
            assert!(report.findings.is_empty());
            assert!(report.score >= 99);
        }
    }
}

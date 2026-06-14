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

// Deterministic MySQL undo log inventory evaluator for roadmap rows
// 03-MYSQL-AI-TRIAGER-00344/00351/00372.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::services::analytics::mysql_analytics::mysql_telemetry::MySqlTelemetrySnapshot;
use crate::services::aws::inventory::types::{
    score_pillar, InventoryFinding, Pillar, PillarReport, Severity, COST_ALLOCATION_TAG_KEYS,
    DEFAULT_STALE_AFTER_HOURS,
};

pub const RESOURCE_TYPE: &str = "MySqlUndoLog";
pub const REASON_COST_OWNER_NOT_RECORDED: &str = "MYSQL_UNDO_LOG_COST_OWNER_NOT_RECORDED";
pub const REASON_COST_NO_UNDO_LOG_METRICS: &str = "MYSQL_UNDO_LOG_COST_NO_METRICS";
pub const REASON_COST_UNDO_PRESSURE_SPEND_REVIEW: &str =
    "MYSQL_UNDO_LOG_COST_UNDO_PRESSURE_SPEND_REVIEW";
pub const REASON_RES_NO_UNDO_LOG_METRICS: &str = "MYSQL_UNDO_LOG_RES_NO_METRICS";
pub const REASON_RES_UNDO_LOCK_PRESSURE: &str = "MYSQL_UNDO_LOG_RES_LOCK_PRESSURE";
pub const REASON_SEC_VERSION_NOT_RECORDED: &str = "MYSQL_UNDO_LOG_SEC_VERSION_NOT_RECORDED";
pub const REASON_SEC_NO_UNDO_LOG_METRICS: &str = "MYSQL_UNDO_LOG_SEC_NO_METRICS";
pub const REASON_SEC_UNDO_LOG_REVIEW: &str = "MYSQL_UNDO_LOG_SEC_UNDO_LOG_REVIEW";
pub const REASON_INV_STALE_DATA: &str = "MYSQL_UNDO_LOG_INV_STALE_DATA";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UndoLogInventoryItem {
    pub connection_id: String,
    pub connection_name: String,
    pub owner: Option<String>,
    pub labels: BTreeMap<String, String>,
    pub server_version: Option<String>,
    pub undo_log_metric_count: usize,
    pub row_lock_waits: i64,
    pub row_lock_time_ms: i64,
    pub deadlocks: i64,
    pub write_operations: i64,
    pub qps_since_start: f64,
    pub collected_at: DateTime<Utc>,
}

pub fn evaluate_mysql_undo_log_inventory(
    items: &[UndoLogInventoryItem],
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

pub fn undo_log_item_from_telemetry(
    connection_id: &str,
    connection_name: &str,
    owner: Option<String>,
    labels: BTreeMap<String, String>,
    snapshot: &MySqlTelemetrySnapshot,
) -> UndoLogInventoryItem {
    UndoLogInventoryItem {
        connection_id: connection_id.to_string(),
        connection_name: connection_name.to_string(),
        owner,
        labels,
        server_version: snapshot.server.version.clone(),
        undo_log_metric_count: 3,
        row_lock_waits: snapshot.innodb.row_lock_waits,
        row_lock_time_ms: snapshot.innodb.row_lock_time_ms,
        deadlocks: snapshot.innodb.deadlocks,
        write_operations: snapshot.workload.com_insert
            + snapshot.workload.com_update
            + snapshot.workload.com_delete,
        qps_since_start: snapshot.workload.qps_since_start,
        collected_at: snapshot.collected_at,
    }
}

fn evaluate_cost(
    item: &UndoLogInventoryItem,
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
                "MySQL undo log inventory for connection {} has no owner, team, project, or cost-center metadata",
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

    if !has_undo_log_metrics(item) {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_NO_UNDO_LOG_METRICS,
            Severity::High,
            format!(
                "MySQL undo log inventory for connection {} has no undo log evidence",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "connection_name": item.connection_name,
                "undo_log_metric_count": item.undo_log_metric_count,
                "recommendation": "Collect InnoDB row-lock, deadlock, and write-workload counters before making undo-retention, purge, or capacity-spend recommendations",
            }),
        ));
    }

    if has_undo_pressure_spend_pressure(item) {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_UNDO_PRESSURE_SPEND_REVIEW,
            Severity::Medium,
            format!(
                "MySQL undo log pressure for connection {} needs spend review before scaling",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "connection_name": item.connection_name,
                "row_lock_waits": item.row_lock_waits,
                "row_lock_time_ms": item.row_lock_time_ms,
                "deadlocks": item.deadlocks,
                "write_operations": item.write_operations,
                "qps_since_start": item.qps_since_start,
                "recommendation": "Review long transactions, purge lag indicators, row-lock pressure, and write spikes before increasing database capacity or storage spend",
            }),
        ));
    }
}

fn evaluate_resilience(
    item: &UndoLogInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if !has_undo_log_metrics(item) {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_NO_UNDO_LOG_METRICS,
            Severity::High,
            format!(
                "MySQL undo log inventory for connection {} has no resilience evidence",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "connection_name": item.connection_name,
                "undo_log_metric_count": item.undo_log_metric_count,
                "recommendation": "Collect row-lock wait, lock time, deadlock, and write workload counters so transaction stalls are not treated as generic saturation",
            }),
        ));
    }

    if has_undo_lock_pressure(item) {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_UNDO_LOCK_PRESSURE,
            Severity::High,
            format!(
                "MySQL undo log evidence for connection {} shows lock or transaction pressure",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "connection_name": item.connection_name,
                "row_lock_waits": item.row_lock_waits,
                "row_lock_time_ms": item.row_lock_time_ms,
                "deadlocks": item.deadlocks,
                "write_operations": item.write_operations,
                "qps_since_start": item.qps_since_start,
                "recommendation": "Investigate long-running transactions, purge lag symptoms, row-lock waits, and deadlocks before failover or broad incident escalation",
            }),
        ));
    }
}

fn evaluate_security(
    item: &UndoLogInventoryItem,
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
                "MySQL undo log inventory for connection {} has no recorded server version",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "connection_name": item.connection_name,
                "server_version": item.server_version,
                "recommendation": "Record MySQL server version with undo-log evidence so version-specific transaction and purge guidance can be mapped deterministically",
            }),
        ));
    }

    if !has_undo_log_metrics(item) {
        findings.push(finding(
            item,
            pillar,
            REASON_SEC_NO_UNDO_LOG_METRICS,
            Severity::High,
            format!(
                "MySQL undo log inventory for connection {} has no scoped evidence for security review",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "connection_name": item.connection_name,
                "undo_log_metric_count": item.undo_log_metric_count,
                "recommendation": "Collect scoped undo-log and transaction pressure evidence so incident review does not require ad hoc privileged diagnostics",
            }),
        ));
    }

    if has_undo_lock_pressure(item) {
        findings.push(finding(
            item,
            pillar,
            REASON_SEC_UNDO_LOG_REVIEW,
            Severity::Medium,
            format!(
                "MySQL undo log pressure for connection {} should be reviewed before exporting incident evidence",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "connection_name": item.connection_name,
                "row_lock_waits": item.row_lock_waits,
                "row_lock_time_ms": item.row_lock_time_ms,
                "deadlocks": item.deadlocks,
                "write_operations": item.write_operations,
                "qps_since_start": item.qps_since_start,
                "recommendation": "Review transaction and lock evidence with scoped credentials and redact workload-sensitive write evidence before sharing outside the database team",
            }),
        ));
    }
}

fn stale_finding(
    item: &UndoLogInventoryItem,
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
            "Inventory data for MySQL undo log connection {} is {} hours old (threshold {} hours)",
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
    item: &UndoLogInventoryItem,
    pillar: Pillar,
    reason_code: &str,
    severity: Severity,
    message: String,
    evidence: Value,
) -> InventoryFinding {
    InventoryFinding {
        resource_id: item.connection_id.clone(),
        arn: format!("mysql://undo-log/{}", item.connection_id),
        pillar,
        reason_code: reason_code.to_string(),
        severity,
        message,
        evidence,
    }
}

fn has_owner_metadata(item: &UndoLogInventoryItem) -> bool {
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

fn has_undo_log_metrics(item: &UndoLogInventoryItem) -> bool {
    item.undo_log_metric_count > 0
}

fn has_undo_lock_pressure(item: &UndoLogInventoryItem) -> bool {
    has_undo_log_metrics(item)
        && (item.row_lock_waits > 0 || item.row_lock_time_ms > 0 || item.deadlocks > 0)
}

fn has_undo_pressure_spend_pressure(item: &UndoLogInventoryItem) -> bool {
    has_undo_lock_pressure(item) && (item.write_operations > 0 || item.qps_since_start > 0.0)
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
        undo_log_metric_count: usize,
        row_lock_waits: i64,
        row_lock_time_ms: i64,
        deadlocks: i64,
        write_operations: i64,
        qps_since_start: f64,
        labels: BTreeMap<String, String>,
        collected_at: DateTime<Utc>,
    ) -> UndoLogInventoryItem {
        UndoLogInventoryItem {
            connection_id: "mysql-1".to_string(),
            connection_name: "orders-mysql".to_string(),
            owner: owner.map(str::to_string),
            labels,
            server_version: server_version.map(str::to_string),
            undo_log_metric_count,
            row_lock_waits,
            row_lock_time_ms,
            deadlocks,
            write_operations,
            qps_since_start,
            collected_at,
        }
    }

    fn healthy_item() -> UndoLogInventoryItem {
        item(
            Some("database-platform"),
            Some("8.0.36"),
            3,
            0,
            0,
            0,
            2_500,
            25.0,
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
    fn cost_flags_missing_owner_missing_metrics_and_undo_pressure_review() {
        let missing_metrics = item(
            Some(""),
            Some("8.0.36"),
            0,
            0,
            0,
            0,
            0,
            0.0,
            BTreeMap::new(),
            now(),
        );
        let undo_pressure = item(
            Some("database-platform"),
            Some("8.0.36"),
            3,
            24,
            12_000,
            1,
            80_000,
            150.0,
            labels(&[("owner", "database-platform")]),
            now(),
        );

        let report = evaluate_mysql_undo_log_inventory(
            &[missing_metrics, undo_pressure],
            Pillar::Cost,
            now(),
        );
        let codes = reason_codes(&report);

        assert!(codes.contains(&REASON_COST_OWNER_NOT_RECORDED));
        assert!(codes.contains(&REASON_COST_NO_UNDO_LOG_METRICS));
        assert!(codes.contains(&REASON_COST_UNDO_PRESSURE_SPEND_REVIEW));
    }

    #[test]
    fn resilience_flags_missing_metrics_and_undo_lock_pressure() {
        let missing_metrics = item(
            Some("database-platform"),
            Some("8.0.36"),
            0,
            0,
            0,
            0,
            0,
            0.0,
            labels(&[("owner", "database-platform")]),
            now(),
        );
        let undo_pressure = item(
            Some("database-platform"),
            Some("8.0.36"),
            3,
            12,
            8_000,
            1,
            50_000,
            120.0,
            labels(&[("owner", "database-platform")]),
            now(),
        );

        let report = evaluate_mysql_undo_log_inventory(
            &[missing_metrics, undo_pressure],
            Pillar::Resilience,
            now(),
        );
        let codes = reason_codes(&report);

        assert!(codes.contains(&REASON_RES_NO_UNDO_LOG_METRICS));
        assert!(codes.contains(&REASON_RES_UNDO_LOCK_PRESSURE));
    }

    #[test]
    fn security_flags_missing_version_missing_metrics_and_undo_log_review() {
        let missing_metrics = item(
            Some("database-platform"),
            None,
            0,
            0,
            0,
            0,
            0,
            0.0,
            labels(&[("owner", "database-platform")]),
            now(),
        );
        let undo_pressure = item(
            Some("database-platform"),
            Some("8.0.36"),
            3,
            5,
            2_500,
            1,
            25_000,
            80.0,
            labels(&[("owner", "database-platform")]),
            now(),
        );

        let report = evaluate_mysql_undo_log_inventory(
            &[missing_metrics, undo_pressure],
            Pillar::Security,
            now(),
        );
        let codes = reason_codes(&report);

        assert!(codes.contains(&REASON_SEC_VERSION_NOT_RECORDED));
        assert!(codes.contains(&REASON_SEC_NO_UNDO_LOG_METRICS));
        assert!(codes.contains(&REASON_SEC_UNDO_LOG_REVIEW));
    }

    #[test]
    fn stale_inventory_is_counted_for_any_pillar() {
        let stale = item(
            Some("database-platform"),
            Some("8.0.36"),
            3,
            0,
            0,
            0,
            100,
            10.0,
            labels(&[("owner", "database-platform")]),
            now() - Duration::hours(49),
        );

        let report = evaluate_mysql_undo_log_inventory(&[stale], Pillar::Resilience, now());

        assert_eq!(report.stale_resources, 1);
        assert!(reason_codes(&report).contains(&REASON_INV_STALE_DATA));
    }

    #[test]
    fn healthy_undo_log_passes_claimed_pillars() {
        for pillar in [Pillar::Cost, Pillar::Resilience, Pillar::Security] {
            let report = evaluate_mysql_undo_log_inventory(&[healthy_item()], pillar, now());

            assert_eq!(report.resources_evaluated, 1);
            assert_eq!(report.stale_resources, 0);
            assert!(report.findings.is_empty());
            assert!(report.score >= 99);
        }
    }
}

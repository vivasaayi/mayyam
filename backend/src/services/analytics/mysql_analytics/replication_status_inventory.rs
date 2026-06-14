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

// Deterministic MySQL replication status inventory evaluator for roadmap rows
// 03-MYSQL-AI-TRIAGER-00442/00449/00470.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::services::analytics::mysql_analytics::mysql_telemetry::MySqlTelemetrySnapshot;
use crate::services::aws::inventory::types::{
    score_pillar, InventoryFinding, Pillar, PillarReport, Severity, COST_ALLOCATION_TAG_KEYS,
    DEFAULT_STALE_AFTER_HOURS,
};

pub const RESOURCE_TYPE: &str = "MySqlReplicationStatus";
pub const REASON_COST_OWNER_NOT_RECORDED: &str = "MYSQL_REPLICATION_STATUS_COST_OWNER_NOT_RECORDED";
pub const REASON_COST_NO_REPLICATION_METRICS: &str = "MYSQL_REPLICATION_STATUS_COST_NO_METRICS";
pub const REASON_COST_REPLICA_LAG_SPEND_REVIEW: &str =
    "MYSQL_REPLICATION_STATUS_COST_REPLICA_LAG_SPEND_REVIEW";
pub const REASON_RES_NO_REPLICATION_METRICS: &str = "MYSQL_REPLICATION_STATUS_RES_NO_METRICS";
pub const REASON_RES_REPLICA_LAG_OR_THREAD_STOPPED: &str =
    "MYSQL_REPLICATION_STATUS_RES_REPLICA_LAG_OR_THREAD_STOPPED";
pub const REASON_SEC_VERSION_NOT_RECORDED: &str =
    "MYSQL_REPLICATION_STATUS_SEC_VERSION_NOT_RECORDED";
pub const REASON_SEC_NO_REPLICATION_METRICS: &str = "MYSQL_REPLICATION_STATUS_SEC_NO_METRICS";
pub const REASON_SEC_REPLICATION_STATUS_REVIEW: &str =
    "MYSQL_REPLICATION_STATUS_SEC_REPLICATION_STATUS_REVIEW";
pub const REASON_INV_STALE_DATA: &str = "MYSQL_REPLICATION_STATUS_INV_STALE_DATA";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplicationStatusInventoryItem {
    pub connection_id: String,
    pub connection_name: String,
    pub owner: Option<String>,
    pub labels: BTreeMap<String, String>,
    pub server_version: Option<String>,
    pub replication_metric_count: usize,
    pub replica_lag_seconds: Option<i64>,
    pub replica_io_running: Option<bool>,
    pub replica_sql_running: Option<bool>,
    pub write_operations: i64,
    pub qps_since_start: f64,
    pub collected_at: DateTime<Utc>,
}

pub fn evaluate_mysql_replication_status_inventory(
    items: &[ReplicationStatusInventoryItem],
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

pub fn replication_status_item_from_telemetry(
    connection_id: &str,
    connection_name: &str,
    owner: Option<String>,
    labels: BTreeMap<String, String>,
    snapshot: &MySqlTelemetrySnapshot,
) -> ReplicationStatusInventoryItem {
    ReplicationStatusInventoryItem {
        connection_id: connection_id.to_string(),
        connection_name: connection_name.to_string(),
        owner,
        labels,
        server_version: snapshot.server.version.clone(),
        replication_metric_count: 0,
        replica_lag_seconds: None,
        replica_io_running: None,
        replica_sql_running: None,
        write_operations: snapshot.workload.com_insert
            + snapshot.workload.com_update
            + snapshot.workload.com_delete,
        qps_since_start: snapshot.workload.qps_since_start,
        collected_at: snapshot.collected_at,
    }
}

fn evaluate_cost(
    item: &ReplicationStatusInventoryItem,
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
                "MySQL replication status inventory for connection {} has no owner, team, project, or cost-center metadata",
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

    if !has_replication_metrics(item) {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_NO_REPLICATION_METRICS,
            Severity::High,
            format!(
                "MySQL replication status inventory for connection {} has no replication status evidence",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "connection_name": item.connection_name,
                "replication_metric_count": item.replication_metric_count,
                "recommendation": "Collect SHOW REPLICA STATUS or provider replica evidence before making replica capacity, storage, backup, or cross-region spend recommendations",
            }),
        ));
    }

    if has_replication_spend_pressure(item) {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_REPLICA_LAG_SPEND_REVIEW,
            Severity::Medium,
            format!(
                "MySQL replication lag for connection {} needs spend review before scaling",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "connection_name": item.connection_name,
                "replica_lag_seconds": item.replica_lag_seconds,
                "replica_io_running": item.replica_io_running,
                "replica_sql_running": item.replica_sql_running,
                "write_operations": item.write_operations,
                "qps_since_start": item.qps_since_start,
                "recommendation": "Review replica apply capacity, write spikes, retention, cross-region topology, and read-routing assumptions before increasing replica or primary spend",
            }),
        ));
    }
}

fn evaluate_resilience(
    item: &ReplicationStatusInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if !has_replication_metrics(item) {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_NO_REPLICATION_METRICS,
            Severity::High,
            format!(
                "MySQL replication status inventory for connection {} has no resilience evidence",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "connection_name": item.connection_name,
                "replication_metric_count": item.replication_metric_count,
                "recommendation": "Collect replica lag and IO/SQL thread status so failover readiness and recovery point risk can be evaluated deterministically",
            }),
        ));
    }

    if has_replication_resilience_risk(item) {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_REPLICA_LAG_OR_THREAD_STOPPED,
            Severity::High,
            format!(
                "MySQL replication status for connection {} shows lag or stopped replica threads",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "connection_name": item.connection_name,
                "replica_lag_seconds": item.replica_lag_seconds,
                "replica_io_running": item.replica_io_running,
                "replica_sql_running": item.replica_sql_running,
                "write_operations": item.write_operations,
                "qps_since_start": item.qps_since_start,
                "recommendation": "Validate replica IO and SQL threads, replication delay, relay log growth, and failover eligibility before relying on this replica for recovery",
            }),
        ));
    }
}

fn evaluate_security(
    item: &ReplicationStatusInventoryItem,
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
                "MySQL replication status inventory for connection {} has no recorded server version",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "connection_name": item.connection_name,
                "server_version": item.server_version,
                "recommendation": "Record MySQL server version with replication status so version-specific replication, TLS, and privilege guidance can be mapped deterministically",
            }),
        ));
    }

    if !has_replication_metrics(item) {
        findings.push(finding(
            item,
            pillar,
            REASON_SEC_NO_REPLICATION_METRICS,
            Severity::High,
            format!(
                "MySQL replication status inventory for connection {} has no scoped evidence for security review",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "connection_name": item.connection_name,
                "replication_metric_count": item.replication_metric_count,
                "recommendation": "Collect scoped replication status evidence so replica privileges, topology, lag, and incident exposure can be reviewed without ad hoc privileged diagnostics",
            }),
        ));
    }

    if has_replication_security_review(item) {
        findings.push(finding(
            item,
            pillar,
            REASON_SEC_REPLICATION_STATUS_REVIEW,
            Severity::Medium,
            format!(
                "MySQL replication status evidence for connection {} should be reviewed",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "connection_name": item.connection_name,
                "replica_lag_seconds": item.replica_lag_seconds,
                "replica_io_running": item.replica_io_running,
                "replica_sql_running": item.replica_sql_running,
                "recommendation": "Review replication users, TLS settings, external replica exposure, and incident data-retention implications before sharing replication status evidence outside the database team",
            }),
        ));
    }
}

fn stale_finding(
    item: &ReplicationStatusInventoryItem,
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
            "Inventory data for MySQL replication status connection {} is {} hours old (threshold {} hours)",
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
    item: &ReplicationStatusInventoryItem,
    pillar: Pillar,
    reason_code: &str,
    severity: Severity,
    message: String,
    evidence: Value,
) -> InventoryFinding {
    InventoryFinding {
        resource_id: item.connection_id.clone(),
        arn: format!("mysql://replication-status/{}", item.connection_id),
        pillar,
        reason_code: reason_code.to_string(),
        severity,
        message,
        evidence,
    }
}

fn has_owner_metadata(item: &ReplicationStatusInventoryItem) -> bool {
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

fn has_replication_metrics(item: &ReplicationStatusInventoryItem) -> bool {
    item.replication_metric_count > 0
}

fn replica_thread_stopped(item: &ReplicationStatusInventoryItem) -> bool {
    item.replica_io_running == Some(false) || item.replica_sql_running == Some(false)
}

fn lag_seconds(item: &ReplicationStatusInventoryItem) -> i64 {
    item.replica_lag_seconds.unwrap_or(0).max(0)
}

fn has_replication_spend_pressure(item: &ReplicationStatusInventoryItem) -> bool {
    has_replication_metrics(item)
        && (lag_seconds(item) >= 300
            || ((item.write_operations >= 25_000 || item.qps_since_start >= 80.0)
                && (lag_seconds(item) >= 60 || replica_thread_stopped(item))))
}

fn has_replication_resilience_risk(item: &ReplicationStatusInventoryItem) -> bool {
    has_replication_metrics(item) && (lag_seconds(item) >= 60 || replica_thread_stopped(item))
}

fn has_replication_security_review(item: &ReplicationStatusInventoryItem) -> bool {
    has_replication_resilience_risk(item)
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
        replication_metric_count: usize,
        replica_lag_seconds: Option<i64>,
        replica_io_running: Option<bool>,
        replica_sql_running: Option<bool>,
        write_operations: i64,
        qps_since_start: f64,
        labels: BTreeMap<String, String>,
        collected_at: DateTime<Utc>,
    ) -> ReplicationStatusInventoryItem {
        ReplicationStatusInventoryItem {
            connection_id: "mysql-1".to_string(),
            connection_name: "orders-mysql".to_string(),
            owner: owner.map(str::to_string),
            labels,
            server_version: server_version.map(str::to_string),
            replication_metric_count,
            replica_lag_seconds,
            replica_io_running,
            replica_sql_running,
            write_operations,
            qps_since_start,
            collected_at,
        }
    }

    fn healthy_item() -> ReplicationStatusInventoryItem {
        item(
            Some("database-platform"),
            Some("8.0.36"),
            3,
            Some(2),
            Some(true),
            Some(true),
            1_000,
            10.0,
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
    fn cost_flags_missing_owner_missing_metrics_and_replica_lag_spend_review() {
        let missing_metrics = item(
            Some(""),
            Some("8.0.36"),
            0,
            None,
            None,
            None,
            0,
            0.0,
            BTreeMap::new(),
            now(),
        );
        let lagged_replica = item(
            Some("database-platform"),
            Some("8.0.36"),
            3,
            Some(600),
            Some(true),
            Some(true),
            80_000,
            150.0,
            labels(&[("owner", "database-platform")]),
            now(),
        );

        let report = evaluate_mysql_replication_status_inventory(
            &[missing_metrics, lagged_replica],
            Pillar::Cost,
            now(),
        );
        let codes = reason_codes(&report);

        assert!(codes.contains(&REASON_COST_OWNER_NOT_RECORDED));
        assert!(codes.contains(&REASON_COST_NO_REPLICATION_METRICS));
        assert!(codes.contains(&REASON_COST_REPLICA_LAG_SPEND_REVIEW));
    }

    #[test]
    fn resilience_flags_missing_metrics_lag_and_stopped_threads() {
        let missing_metrics = item(
            Some("database-platform"),
            Some("8.0.36"),
            0,
            None,
            None,
            None,
            0,
            0.0,
            labels(&[("owner", "database-platform")]),
            now(),
        );
        let stopped_thread = item(
            Some("database-platform"),
            Some("8.0.36"),
            3,
            Some(120),
            Some(false),
            Some(true),
            50_000,
            100.0,
            labels(&[("owner", "database-platform")]),
            now(),
        );

        let report = evaluate_mysql_replication_status_inventory(
            &[missing_metrics, stopped_thread],
            Pillar::Resilience,
            now(),
        );
        let codes = reason_codes(&report);

        assert!(codes.contains(&REASON_RES_NO_REPLICATION_METRICS));
        assert!(codes.contains(&REASON_RES_REPLICA_LAG_OR_THREAD_STOPPED));
    }

    #[test]
    fn security_flags_missing_version_missing_metrics_and_replication_review() {
        let missing_metrics = item(
            Some("database-platform"),
            None,
            0,
            None,
            None,
            None,
            0,
            0.0,
            labels(&[("owner", "database-platform")]),
            now(),
        );
        let stopped_thread = item(
            Some("database-platform"),
            Some("8.0.36"),
            3,
            Some(90),
            Some(true),
            Some(false),
            40_000,
            95.0,
            labels(&[("owner", "database-platform")]),
            now(),
        );

        let report = evaluate_mysql_replication_status_inventory(
            &[missing_metrics, stopped_thread],
            Pillar::Security,
            now(),
        );
        let codes = reason_codes(&report);

        assert!(codes.contains(&REASON_SEC_VERSION_NOT_RECORDED));
        assert!(codes.contains(&REASON_SEC_NO_REPLICATION_METRICS));
        assert!(codes.contains(&REASON_SEC_REPLICATION_STATUS_REVIEW));
    }

    #[test]
    fn stale_inventory_is_counted_for_any_pillar() {
        let stale = item(
            Some("database-platform"),
            Some("8.0.36"),
            3,
            Some(3),
            Some(true),
            Some(true),
            100,
            10.0,
            labels(&[("owner", "database-platform")]),
            now() - Duration::hours(49),
        );

        let report =
            evaluate_mysql_replication_status_inventory(&[stale], Pillar::Resilience, now());

        assert_eq!(report.stale_resources, 1);
        assert!(reason_codes(&report).contains(&REASON_INV_STALE_DATA));
    }

    #[test]
    fn healthy_replication_status_passes_claimed_pillars() {
        for pillar in [Pillar::Cost, Pillar::Resilience, Pillar::Security] {
            let report =
                evaluate_mysql_replication_status_inventory(&[healthy_item()], pillar, now());

            assert_eq!(report.resources_evaluated, 1);
            assert_eq!(report.stale_resources, 0);
            assert!(report.findings.is_empty());
            assert!(report.score >= 99);
        }
    }
}

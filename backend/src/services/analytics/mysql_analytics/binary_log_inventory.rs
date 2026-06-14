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

// Deterministic MySQL binary log inventory evaluator for roadmap rows
// 03-MYSQL-AI-TRIAGER-00393/00400/00421.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::services::analytics::mysql_analytics::mysql_telemetry::MySqlTelemetrySnapshot;
use crate::services::aws::inventory::types::{
    score_pillar, InventoryFinding, Pillar, PillarReport, Severity, COST_ALLOCATION_TAG_KEYS,
    DEFAULT_STALE_AFTER_HOURS,
};

pub const RESOURCE_TYPE: &str = "MySqlBinaryLog";
pub const REASON_COST_OWNER_NOT_RECORDED: &str = "MYSQL_BINARY_LOG_COST_OWNER_NOT_RECORDED";
pub const REASON_COST_NO_BINARY_LOG_METRICS: &str = "MYSQL_BINARY_LOG_COST_NO_METRICS";
pub const REASON_COST_WRITE_SPEND_REVIEW: &str = "MYSQL_BINARY_LOG_COST_WRITE_SPEND_REVIEW";
pub const REASON_RES_NO_BINARY_LOG_METRICS: &str = "MYSQL_BINARY_LOG_RES_NO_METRICS";
pub const REASON_RES_WRITE_LOG_PRESSURE: &str = "MYSQL_BINARY_LOG_RES_WRITE_LOG_PRESSURE";
pub const REASON_SEC_VERSION_NOT_RECORDED: &str = "MYSQL_BINARY_LOG_SEC_VERSION_NOT_RECORDED";
pub const REASON_SEC_NO_BINARY_LOG_METRICS: &str = "MYSQL_BINARY_LOG_SEC_NO_METRICS";
pub const REASON_SEC_BINARY_LOG_REVIEW: &str = "MYSQL_BINARY_LOG_SEC_BINARY_LOG_REVIEW";
pub const REASON_INV_STALE_DATA: &str = "MYSQL_BINARY_LOG_INV_STALE_DATA";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BinaryLogInventoryItem {
    pub connection_id: String,
    pub connection_name: String,
    pub owner: Option<String>,
    pub labels: BTreeMap<String, String>,
    pub server_version: Option<String>,
    pub binary_log_metric_count: usize,
    pub write_operations: i64,
    pub qps_since_start: f64,
    pub read_write_ratio: Option<f64>,
    pub collected_at: DateTime<Utc>,
}

pub fn evaluate_mysql_binary_log_inventory(
    items: &[BinaryLogInventoryItem],
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

pub fn binary_log_item_from_telemetry(
    connection_id: &str,
    connection_name: &str,
    owner: Option<String>,
    labels: BTreeMap<String, String>,
    snapshot: &MySqlTelemetrySnapshot,
) -> BinaryLogInventoryItem {
    BinaryLogInventoryItem {
        connection_id: connection_id.to_string(),
        connection_name: connection_name.to_string(),
        owner,
        labels,
        server_version: snapshot.server.version.clone(),
        binary_log_metric_count: 2,
        write_operations: snapshot.workload.com_insert
            + snapshot.workload.com_update
            + snapshot.workload.com_delete,
        qps_since_start: snapshot.workload.qps_since_start,
        read_write_ratio: snapshot.workload.read_write_ratio,
        collected_at: snapshot.collected_at,
    }
}

fn evaluate_cost(
    item: &BinaryLogInventoryItem,
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
                "MySQL binary log inventory for connection {} has no owner, team, project, or cost-center metadata",
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

    if !has_binary_log_metrics(item) {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_NO_BINARY_LOG_METRICS,
            Severity::High,
            format!(
                "MySQL binary log inventory for connection {} has no binary log evidence",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "connection_name": item.connection_name,
                "binary_log_metric_count": item.binary_log_metric_count,
                "recommendation": "Collect write workload and binary-log related evidence before making log-retention, storage, replication, or backup-spend recommendations",
            }),
        ));
    }

    if has_write_log_pressure(item) {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_WRITE_SPEND_REVIEW,
            Severity::Medium,
            format!(
                "MySQL binary log write pressure for connection {} needs spend review",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "connection_name": item.connection_name,
                "write_operations": item.write_operations,
                "qps_since_start": item.qps_since_start,
                "read_write_ratio": item.read_write_ratio,
                "recommendation": "Review binary log retention, replica fanout, backup/PITR requirements, and write spikes before increasing storage or instance spend",
            }),
        ));
    }
}

fn evaluate_resilience(
    item: &BinaryLogInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if !has_binary_log_metrics(item) {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_NO_BINARY_LOG_METRICS,
            Severity::High,
            format!(
                "MySQL binary log inventory for connection {} has no resilience evidence",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "connection_name": item.connection_name,
                "binary_log_metric_count": item.binary_log_metric_count,
                "recommendation": "Collect binary-log and write workload evidence so PITR, replication, and failover readiness can be evaluated deterministically",
            }),
        ));
    }

    if has_write_log_pressure(item) {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_WRITE_LOG_PRESSURE,
            Severity::High,
            format!(
                "MySQL binary log write pressure is elevated for connection {}",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "connection_name": item.connection_name,
                "write_operations": item.write_operations,
                "qps_since_start": item.qps_since_start,
                "read_write_ratio": item.read_write_ratio,
                "recommendation": "Validate binary log durability, retention headroom, replica apply capacity, and PITR coverage before treating write pressure as generic database saturation",
            }),
        ));
    }
}

fn evaluate_security(
    item: &BinaryLogInventoryItem,
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
                "MySQL binary log inventory for connection {} has no recorded server version",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "connection_name": item.connection_name,
                "server_version": item.server_version,
                "recommendation": "Record MySQL server version with binary-log evidence so version-specific replication and audit guidance can be mapped deterministically",
            }),
        ));
    }

    if !has_binary_log_metrics(item) {
        findings.push(finding(
            item,
            pillar,
            REASON_SEC_NO_BINARY_LOG_METRICS,
            Severity::High,
            format!(
                "MySQL binary log inventory for connection {} has no scoped evidence for security review",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "connection_name": item.connection_name,
                "binary_log_metric_count": item.binary_log_metric_count,
                "recommendation": "Collect scoped binary-log inventory evidence so audit, PITR, and incident review do not require ad hoc privileged diagnostics",
            }),
        ));
    }

    if has_write_log_pressure(item) {
        findings.push(finding(
            item,
            pillar,
            REASON_SEC_BINARY_LOG_REVIEW,
            Severity::Medium,
            format!(
                "MySQL binary log evidence for connection {} should be reviewed before export",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "connection_name": item.connection_name,
                "write_operations": item.write_operations,
                "qps_since_start": item.qps_since_start,
                "read_write_ratio": item.read_write_ratio,
                "recommendation": "Review binary-log evidence with scoped credentials and redact workload-sensitive write metadata before sharing outside the database team",
            }),
        ));
    }
}

fn stale_finding(
    item: &BinaryLogInventoryItem,
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
            "Inventory data for MySQL binary log connection {} is {} hours old (threshold {} hours)",
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
    item: &BinaryLogInventoryItem,
    pillar: Pillar,
    reason_code: &str,
    severity: Severity,
    message: String,
    evidence: Value,
) -> InventoryFinding {
    InventoryFinding {
        resource_id: item.connection_id.clone(),
        arn: format!("mysql://binary-log/{}", item.connection_id),
        pillar,
        reason_code: reason_code.to_string(),
        severity,
        message,
        evidence,
    }
}

fn has_owner_metadata(item: &BinaryLogInventoryItem) -> bool {
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

fn has_binary_log_metrics(item: &BinaryLogInventoryItem) -> bool {
    item.binary_log_metric_count > 0
}

fn has_write_log_pressure(item: &BinaryLogInventoryItem) -> bool {
    has_binary_log_metrics(item)
        && (item.write_operations >= 25_000
            || item.qps_since_start >= 80.0
            || (item.write_operations > 0
                && item
                    .read_write_ratio
                    .map(|ratio| ratio > 0.0 && ratio <= 1.0)
                    .unwrap_or(false)))
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
        binary_log_metric_count: usize,
        write_operations: i64,
        qps_since_start: f64,
        read_write_ratio: Option<f64>,
        labels: BTreeMap<String, String>,
        collected_at: DateTime<Utc>,
    ) -> BinaryLogInventoryItem {
        BinaryLogInventoryItem {
            connection_id: "mysql-1".to_string(),
            connection_name: "orders-mysql".to_string(),
            owner: owner.map(str::to_string),
            labels,
            server_version: server_version.map(str::to_string),
            binary_log_metric_count,
            write_operations,
            qps_since_start,
            read_write_ratio,
            collected_at,
        }
    }

    fn healthy_item() -> BinaryLogInventoryItem {
        item(
            Some("database-platform"),
            Some("8.0.36"),
            2,
            2_500,
            25.0,
            Some(4.0),
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
    fn cost_flags_missing_owner_missing_metrics_and_write_spend_review() {
        let missing_metrics = item(
            Some(""),
            Some("8.0.36"),
            0,
            0,
            0.0,
            None,
            BTreeMap::new(),
            now(),
        );
        let write_heavy = item(
            Some("database-platform"),
            Some("8.0.36"),
            2,
            80_000,
            150.0,
            Some(0.5),
            labels(&[("owner", "database-platform")]),
            now(),
        );

        let report = evaluate_mysql_binary_log_inventory(
            &[missing_metrics, write_heavy],
            Pillar::Cost,
            now(),
        );
        let codes = reason_codes(&report);

        assert!(codes.contains(&REASON_COST_OWNER_NOT_RECORDED));
        assert!(codes.contains(&REASON_COST_NO_BINARY_LOG_METRICS));
        assert!(codes.contains(&REASON_COST_WRITE_SPEND_REVIEW));
    }

    #[test]
    fn resilience_flags_missing_metrics_and_write_log_pressure() {
        let missing_metrics = item(
            Some("database-platform"),
            Some("8.0.36"),
            0,
            0,
            0.0,
            None,
            labels(&[("owner", "database-platform")]),
            now(),
        );
        let write_heavy = item(
            Some("database-platform"),
            Some("8.0.36"),
            2,
            50_000,
            120.0,
            Some(0.6),
            labels(&[("owner", "database-platform")]),
            now(),
        );

        let report = evaluate_mysql_binary_log_inventory(
            &[missing_metrics, write_heavy],
            Pillar::Resilience,
            now(),
        );
        let codes = reason_codes(&report);

        assert!(codes.contains(&REASON_RES_NO_BINARY_LOG_METRICS));
        assert!(codes.contains(&REASON_RES_WRITE_LOG_PRESSURE));
    }

    #[test]
    fn security_flags_missing_version_missing_metrics_and_binary_log_review() {
        let missing_metrics = item(
            Some("database-platform"),
            None,
            0,
            0,
            0.0,
            None,
            labels(&[("owner", "database-platform")]),
            now(),
        );
        let write_heavy = item(
            Some("database-platform"),
            Some("8.0.36"),
            2,
            25_000,
            80.0,
            Some(0.7),
            labels(&[("owner", "database-platform")]),
            now(),
        );

        let report = evaluate_mysql_binary_log_inventory(
            &[missing_metrics, write_heavy],
            Pillar::Security,
            now(),
        );
        let codes = reason_codes(&report);

        assert!(codes.contains(&REASON_SEC_VERSION_NOT_RECORDED));
        assert!(codes.contains(&REASON_SEC_NO_BINARY_LOG_METRICS));
        assert!(codes.contains(&REASON_SEC_BINARY_LOG_REVIEW));
    }

    #[test]
    fn stale_inventory_is_counted_for_any_pillar() {
        let stale = item(
            Some("database-platform"),
            Some("8.0.36"),
            2,
            100,
            10.0,
            Some(4.0),
            labels(&[("owner", "database-platform")]),
            now() - Duration::hours(49),
        );

        let report = evaluate_mysql_binary_log_inventory(&[stale], Pillar::Resilience, now());

        assert_eq!(report.stale_resources, 1);
        assert!(reason_codes(&report).contains(&REASON_INV_STALE_DATA));
    }

    #[test]
    fn healthy_binary_log_passes_claimed_pillars() {
        for pillar in [Pillar::Cost, Pillar::Resilience, Pillar::Security] {
            let report = evaluate_mysql_binary_log_inventory(&[healthy_item()], pillar, now());

            assert_eq!(report.resources_evaluated, 1);
            assert_eq!(report.stale_resources, 0);
            assert!(report.findings.is_empty());
            assert!(report.score >= 99);
        }
    }
}

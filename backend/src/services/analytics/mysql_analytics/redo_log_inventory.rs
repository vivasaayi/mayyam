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

// Deterministic MySQL redo log inventory evaluator for roadmap rows
// 03-MYSQL-AI-TRIAGER-00295/00302/00323.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::services::analytics::mysql_analytics::mysql_telemetry::MySqlTelemetrySnapshot;
use crate::services::aws::inventory::types::{
    score_pillar, InventoryFinding, Pillar, PillarReport, Severity, COST_ALLOCATION_TAG_KEYS,
    DEFAULT_STALE_AFTER_HOURS,
};

pub const RESOURCE_TYPE: &str = "MySqlRedoLog";
pub const REASON_COST_OWNER_NOT_RECORDED: &str = "MYSQL_REDO_LOG_COST_OWNER_NOT_RECORDED";
pub const REASON_COST_NO_REDO_LOG_METRICS: &str = "MYSQL_REDO_LOG_COST_NO_METRICS";
pub const REASON_COST_LOG_WAIT_SPEND_REVIEW: &str = "MYSQL_REDO_LOG_COST_LOG_WAIT_SPEND_REVIEW";
pub const REASON_RES_NO_REDO_LOG_METRICS: &str = "MYSQL_REDO_LOG_RES_NO_METRICS";
pub const REASON_RES_REDO_LOG_WAITS: &str = "MYSQL_REDO_LOG_RES_WAITS";
pub const REASON_SEC_VERSION_NOT_RECORDED: &str = "MYSQL_REDO_LOG_SEC_VERSION_NOT_RECORDED";
pub const REASON_SEC_NO_REDO_LOG_METRICS: &str = "MYSQL_REDO_LOG_SEC_NO_METRICS";
pub const REASON_SEC_REDO_LOG_REVIEW: &str = "MYSQL_REDO_LOG_SEC_REDO_LOG_REVIEW";
pub const REASON_INV_STALE_DATA: &str = "MYSQL_REDO_LOG_INV_STALE_DATA";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedoLogInventoryItem {
    pub connection_id: String,
    pub connection_name: String,
    pub owner: Option<String>,
    pub labels: BTreeMap<String, String>,
    pub server_version: Option<String>,
    pub redo_log_metric_count: usize,
    pub log_waits: i64,
    pub write_operations: i64,
    pub qps_since_start: f64,
    pub collected_at: DateTime<Utc>,
}

pub fn evaluate_mysql_redo_log_inventory(
    items: &[RedoLogInventoryItem],
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

pub fn redo_log_item_from_telemetry(
    connection_id: &str,
    connection_name: &str,
    owner: Option<String>,
    labels: BTreeMap<String, String>,
    snapshot: &MySqlTelemetrySnapshot,
) -> RedoLogInventoryItem {
    RedoLogInventoryItem {
        connection_id: connection_id.to_string(),
        connection_name: connection_name.to_string(),
        owner,
        labels,
        server_version: snapshot.server.version.clone(),
        redo_log_metric_count: 1,
        log_waits: snapshot.innodb.log_waits,
        write_operations: snapshot.workload.com_insert
            + snapshot.workload.com_update
            + snapshot.workload.com_delete,
        qps_since_start: snapshot.workload.qps_since_start,
        collected_at: snapshot.collected_at,
    }
}

fn evaluate_cost(
    item: &RedoLogInventoryItem,
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
                "MySQL redo log inventory for connection {} has no owner, team, project, or cost-center metadata",
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

    if !has_redo_log_metrics(item) {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_NO_REDO_LOG_METRICS,
            Severity::High,
            format!(
                "MySQL redo log inventory for connection {} has no redo log metrics",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "connection_name": item.connection_name,
                "redo_log_metric_count": item.redo_log_metric_count,
                "recommendation": "Collect Innodb_log_waits and write workload counters before making redo-log sizing or instance-spend recommendations",
            }),
        ));
    }

    if has_log_wait_spend_pressure(item) {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_LOG_WAIT_SPEND_REVIEW,
            Severity::Medium,
            format!(
                "MySQL redo log waits for connection {} need spend review before scaling",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "connection_name": item.connection_name,
                "log_waits": item.log_waits,
                "write_operations": item.write_operations,
                "qps_since_start": item.qps_since_start,
                "recommendation": "Review redo log sizing, flush settings, and write spikes before increasing database capacity or storage spend",
            }),
        ));
    }
}

fn evaluate_resilience(
    item: &RedoLogInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if !has_redo_log_metrics(item) {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_NO_REDO_LOG_METRICS,
            Severity::High,
            format!(
                "MySQL redo log inventory for connection {} has no resilience evidence",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "connection_name": item.connection_name,
                "redo_log_metric_count": item.redo_log_metric_count,
                "recommendation": "Collect redo log waits and write workload counters so write stalls can be separated from query, lock, or storage symptoms",
            }),
        ));
    }

    if item.log_waits > 0 {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_REDO_LOG_WAITS,
            Severity::High,
            format!(
                "MySQL redo log waits are present for connection {}",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "connection_name": item.connection_name,
                "log_waits": item.log_waits,
                "write_operations": item.write_operations,
                "qps_since_start": item.qps_since_start,
                "recommendation": "Investigate redo log waits before treating write latency as generic database saturation or initiating failover",
            }),
        ));
    }
}

fn evaluate_security(
    item: &RedoLogInventoryItem,
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
                "MySQL redo log inventory for connection {} has no recorded server version",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "connection_name": item.connection_name,
                "server_version": item.server_version,
                "recommendation": "Record MySQL server version with redo-log evidence so version-specific security guidance can be mapped deterministically",
            }),
        ));
    }

    if !has_redo_log_metrics(item) {
        findings.push(finding(
            item,
            pillar,
            REASON_SEC_NO_REDO_LOG_METRICS,
            Severity::High,
            format!(
                "MySQL redo log inventory for connection {} has no redo-log evidence for security review",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "connection_name": item.connection_name,
                "redo_log_metric_count": item.redo_log_metric_count,
                "recommendation": "Collect scoped redo-log evidence so incident review does not require ad hoc privileged diagnostics",
            }),
        ));
    }

    if item.log_waits > 0 {
        findings.push(finding(
            item,
            pillar,
            REASON_SEC_REDO_LOG_REVIEW,
            Severity::Medium,
            format!(
                "MySQL redo log waits for connection {} should be reviewed before exporting incident evidence",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "connection_name": item.connection_name,
                "log_waits": item.log_waits,
                "write_operations": item.write_operations,
                "qps_since_start": item.qps_since_start,
                "recommendation": "Review redo-log wait evidence with scoped credentials and redact workload-sensitive write evidence before sharing outside the database team",
            }),
        ));
    }
}

fn stale_finding(
    item: &RedoLogInventoryItem,
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
            "Inventory data for MySQL redo log connection {} is {} hours old (threshold {} hours)",
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
    item: &RedoLogInventoryItem,
    pillar: Pillar,
    reason_code: &str,
    severity: Severity,
    message: String,
    evidence: Value,
) -> InventoryFinding {
    InventoryFinding {
        resource_id: item.connection_id.clone(),
        arn: format!("mysql://redo-log/{}", item.connection_id),
        pillar,
        reason_code: reason_code.to_string(),
        severity,
        message,
        evidence,
    }
}

fn has_owner_metadata(item: &RedoLogInventoryItem) -> bool {
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

fn has_redo_log_metrics(item: &RedoLogInventoryItem) -> bool {
    item.redo_log_metric_count > 0
}

fn has_log_wait_spend_pressure(item: &RedoLogInventoryItem) -> bool {
    has_redo_log_metrics(item)
        && item.log_waits > 0
        && (item.write_operations > 0 || item.qps_since_start > 0.0)
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
        redo_log_metric_count: usize,
        log_waits: i64,
        write_operations: i64,
        qps_since_start: f64,
        labels: BTreeMap<String, String>,
        collected_at: DateTime<Utc>,
    ) -> RedoLogInventoryItem {
        RedoLogInventoryItem {
            connection_id: "mysql-1".to_string(),
            connection_name: "orders-mysql".to_string(),
            owner: owner.map(str::to_string),
            labels,
            server_version: server_version.map(str::to_string),
            redo_log_metric_count,
            log_waits,
            write_operations,
            qps_since_start,
            collected_at,
        }
    }

    fn healthy_item() -> RedoLogInventoryItem {
        item(
            Some("database-platform"),
            Some("8.0.36"),
            1,
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
    fn cost_flags_missing_owner_missing_metrics_and_log_wait_spend_review() {
        let missing_metrics = item(
            Some(""),
            Some("8.0.36"),
            0,
            0,
            0,
            0.0,
            BTreeMap::new(),
            now(),
        );
        let log_waits = item(
            Some("database-platform"),
            Some("8.0.36"),
            1,
            4,
            80_000,
            150.0,
            labels(&[("owner", "database-platform")]),
            now(),
        );

        let report =
            evaluate_mysql_redo_log_inventory(&[missing_metrics, log_waits], Pillar::Cost, now());
        let codes = reason_codes(&report);

        assert!(codes.contains(&REASON_COST_OWNER_NOT_RECORDED));
        assert!(codes.contains(&REASON_COST_NO_REDO_LOG_METRICS));
        assert!(codes.contains(&REASON_COST_LOG_WAIT_SPEND_REVIEW));
    }

    #[test]
    fn resilience_flags_missing_metrics_and_redo_log_waits() {
        let missing_metrics = item(
            Some("database-platform"),
            Some("8.0.36"),
            0,
            0,
            0,
            0.0,
            labels(&[("owner", "database-platform")]),
            now(),
        );
        let log_waits = item(
            Some("database-platform"),
            Some("8.0.36"),
            1,
            2,
            50_000,
            120.0,
            labels(&[("owner", "database-platform")]),
            now(),
        );

        let report = evaluate_mysql_redo_log_inventory(
            &[missing_metrics, log_waits],
            Pillar::Resilience,
            now(),
        );
        let codes = reason_codes(&report);

        assert!(codes.contains(&REASON_RES_NO_REDO_LOG_METRICS));
        assert!(codes.contains(&REASON_RES_REDO_LOG_WAITS));
    }

    #[test]
    fn security_flags_missing_version_missing_metrics_and_redo_log_review() {
        let missing_metrics = item(
            Some("database-platform"),
            None,
            0,
            0,
            0,
            0.0,
            labels(&[("owner", "database-platform")]),
            now(),
        );
        let log_waits = item(
            Some("database-platform"),
            Some("8.0.36"),
            1,
            1,
            25_000,
            80.0,
            labels(&[("owner", "database-platform")]),
            now(),
        );

        let report = evaluate_mysql_redo_log_inventory(
            &[missing_metrics, log_waits],
            Pillar::Security,
            now(),
        );
        let codes = reason_codes(&report);

        assert!(codes.contains(&REASON_SEC_VERSION_NOT_RECORDED));
        assert!(codes.contains(&REASON_SEC_NO_REDO_LOG_METRICS));
        assert!(codes.contains(&REASON_SEC_REDO_LOG_REVIEW));
    }

    #[test]
    fn stale_inventory_is_counted_for_any_pillar() {
        let target = item(
            Some("database-platform"),
            Some("8.0.36"),
            1,
            0,
            2_500,
            25.0,
            labels(&[("owner", "database-platform")]),
            now() - Duration::hours(DEFAULT_STALE_AFTER_HOURS + 2),
        );

        let report = evaluate_mysql_redo_log_inventory(&[target], Pillar::Cost, now());

        assert_eq!(report.stale_resources, 1);
        assert_eq!(reason_codes(&report), vec![REASON_INV_STALE_DATA]);
    }

    #[test]
    fn healthy_redo_log_passes_claimed_pillars() {
        for pillar in [Pillar::Cost, Pillar::Resilience, Pillar::Security] {
            let item = healthy_item();
            let report = evaluate_mysql_redo_log_inventory(&[item], pillar, now());

            assert_eq!(report.resources_evaluated, 1);
            assert_eq!(report.stale_resources, 0);
            assert!(report.findings.is_empty());
            assert_eq!(report.score, 100);
        }
    }
}

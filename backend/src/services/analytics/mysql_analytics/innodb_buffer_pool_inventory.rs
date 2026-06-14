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

// Deterministic MySQL InnoDB buffer pool inventory evaluator for roadmap rows
// 03-MYSQL-AI-TRIAGER-00246/00253/00274.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::services::analytics::mysql_analytics::mysql_telemetry::MySqlTelemetrySnapshot;
use crate::services::aws::inventory::types::{
    score_pillar, InventoryFinding, Pillar, PillarReport, Severity, COST_ALLOCATION_TAG_KEYS,
    DEFAULT_STALE_AFTER_HOURS,
};

pub const RESOURCE_TYPE: &str = "MySqlInnoDbBufferPool";
pub const REASON_COST_OWNER_NOT_RECORDED: &str = "MYSQL_INNODB_BUFFER_POOL_COST_OWNER_NOT_RECORDED";
pub const REASON_COST_NO_BUFFER_POOL_METRICS: &str = "MYSQL_INNODB_BUFFER_POOL_COST_NO_METRICS";
pub const REASON_COST_RIGHTSIZE_REVIEW: &str = "MYSQL_INNODB_BUFFER_POOL_COST_RIGHTSIZE_REVIEW";
pub const REASON_RES_NO_BUFFER_POOL_METRICS: &str = "MYSQL_INNODB_BUFFER_POOL_RES_NO_METRICS";
pub const REASON_RES_DIRTY_OR_HEADROOM_PRESSURE: &str =
    "MYSQL_INNODB_BUFFER_POOL_RES_DIRTY_OR_HEADROOM";
pub const REASON_SEC_VERSION_NOT_RECORDED: &str =
    "MYSQL_INNODB_BUFFER_POOL_SEC_VERSION_NOT_RECORDED";
pub const REASON_SEC_NO_BUFFER_POOL_METRICS: &str = "MYSQL_INNODB_BUFFER_POOL_SEC_NO_METRICS";
pub const REASON_SEC_PRESSURE_REVIEW: &str = "MYSQL_INNODB_BUFFER_POOL_SEC_PRESSURE_REVIEW";
pub const REASON_INV_STALE_DATA: &str = "MYSQL_INNODB_BUFFER_POOL_INV_STALE_DATA";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InnoDbBufferPoolInventoryItem {
    pub connection_id: String,
    pub connection_name: String,
    pub owner: Option<String>,
    pub labels: BTreeMap<String, String>,
    pub server_version: Option<String>,
    pub buffer_pool_hit_ratio: Option<f64>,
    pub buffer_pool_pages_total: i64,
    pub buffer_pool_pages_free: i64,
    pub buffer_pool_pages_dirty: i64,
    pub buffer_pool_dirty_pct: Option<f64>,
    pub buffer_pool_free_pct: Option<f64>,
    pub log_waits: i64,
    pub row_lock_waits: i64,
    pub row_lock_time_ms: i64,
    pub deadlocks: i64,
    pub collected_at: DateTime<Utc>,
}

pub fn evaluate_mysql_innodb_buffer_pool_inventory(
    items: &[InnoDbBufferPoolInventoryItem],
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

pub fn innodb_buffer_pool_item_from_telemetry(
    connection_id: &str,
    connection_name: &str,
    owner: Option<String>,
    labels: BTreeMap<String, String>,
    snapshot: &MySqlTelemetrySnapshot,
) -> InnoDbBufferPoolInventoryItem {
    InnoDbBufferPoolInventoryItem {
        connection_id: connection_id.to_string(),
        connection_name: connection_name.to_string(),
        owner,
        labels,
        server_version: snapshot.server.version.clone(),
        buffer_pool_hit_ratio: snapshot.innodb.buffer_pool_hit_ratio,
        buffer_pool_pages_total: snapshot.innodb.buffer_pool_pages_total,
        buffer_pool_pages_free: snapshot.innodb.buffer_pool_pages_free,
        buffer_pool_pages_dirty: snapshot.innodb.buffer_pool_pages_dirty,
        buffer_pool_dirty_pct: snapshot.innodb.buffer_pool_dirty_pct,
        buffer_pool_free_pct: snapshot.innodb.buffer_pool_free_pct,
        log_waits: snapshot.innodb.log_waits,
        row_lock_waits: snapshot.innodb.row_lock_waits,
        row_lock_time_ms: snapshot.innodb.row_lock_time_ms,
        deadlocks: snapshot.innodb.deadlocks,
        collected_at: snapshot.collected_at,
    }
}

fn evaluate_cost(
    item: &InnoDbBufferPoolInventoryItem,
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
                "MySQL InnoDB buffer pool inventory for connection {} has no owner, team, project, or cost-center metadata",
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

    if !has_buffer_pool_metrics(item) {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_NO_BUFFER_POOL_METRICS,
            Severity::High,
            format!(
                "MySQL InnoDB buffer pool inventory for connection {} has no buffer pool metrics",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "connection_name": item.connection_name,
                "buffer_pool_pages_total": item.buffer_pool_pages_total,
                "buffer_pool_hit_ratio": item.buffer_pool_hit_ratio,
                "recommendation": "Collect InnoDB buffer pool status counters before making cost or instance-size recommendations",
            }),
        ));
    }

    if has_cost_rightsize_pressure(item) {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_RIGHTSIZE_REVIEW,
            Severity::Medium,
            format!(
                "MySQL InnoDB buffer pool evidence for connection {} needs sizing review",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "connection_name": item.connection_name,
                "buffer_pool_hit_ratio": item.buffer_pool_hit_ratio,
                "buffer_pool_free_pct": item.buffer_pool_free_pct,
                "buffer_pool_pages_total": item.buffer_pool_pages_total,
                "buffer_pool_pages_free": item.buffer_pool_pages_free,
                "recommendation": "Compare working-set fit, free buffer pool pages, and disk-read pressure before increasing or decreasing database memory spend",
            }),
        ));
    }
}

fn evaluate_resilience(
    item: &InnoDbBufferPoolInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if !has_buffer_pool_metrics(item) {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_NO_BUFFER_POOL_METRICS,
            Severity::High,
            format!(
                "MySQL InnoDB buffer pool inventory for connection {} has no resilience evidence",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "connection_name": item.connection_name,
                "buffer_pool_pages_total": item.buffer_pool_pages_total,
                "recommendation": "Collect InnoDB buffer pool and lock counters so incidents can distinguish cache pressure, checkpoint pressure, and storage latency",
            }),
        ));
    }

    if has_resilience_pressure(item) {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_DIRTY_OR_HEADROOM_PRESSURE,
            Severity::High,
            format!(
                "MySQL InnoDB buffer pool evidence for connection {} shows dirty-page or headroom pressure",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "connection_name": item.connection_name,
                "buffer_pool_dirty_pct": item.buffer_pool_dirty_pct,
                "buffer_pool_free_pct": item.buffer_pool_free_pct,
                "buffer_pool_pages_dirty": item.buffer_pool_pages_dirty,
                "buffer_pool_pages_free": item.buffer_pool_pages_free,
                "log_waits": item.log_waits,
                "row_lock_waits": item.row_lock_waits,
                "deadlocks": item.deadlocks,
                "recommendation": "Investigate dirty-page pressure, low free-page headroom, redo-log waits, and lock waits before treating the instance as generally unavailable",
            }),
        ));
    }
}

fn evaluate_security(
    item: &InnoDbBufferPoolInventoryItem,
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
                "MySQL InnoDB buffer pool inventory for connection {} has no recorded server version",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "connection_name": item.connection_name,
                "server_version": item.server_version,
                "recommendation": "Record MySQL server version with InnoDB evidence so version-specific security guidance can be mapped deterministically",
            }),
        ));
    }

    if !has_buffer_pool_metrics(item) {
        findings.push(finding(
            item,
            pillar,
            REASON_SEC_NO_BUFFER_POOL_METRICS,
            Severity::High,
            format!(
                "MySQL InnoDB buffer pool inventory for connection {} has no buffer pool evidence for security review",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "connection_name": item.connection_name,
                "buffer_pool_pages_total": item.buffer_pool_pages_total,
                "recommendation": "Collect scoped InnoDB buffer pool evidence so incident review does not require ad hoc privileged diagnostics",
            }),
        ));
    }

    if has_security_review_pressure(item) {
        findings.push(finding(
            item,
            pillar,
            REASON_SEC_PRESSURE_REVIEW,
            Severity::Medium,
            format!(
                "MySQL InnoDB buffer pool pressure for connection {} should be reviewed before exporting incident evidence",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "connection_name": item.connection_name,
                "buffer_pool_hit_ratio": item.buffer_pool_hit_ratio,
                "buffer_pool_dirty_pct": item.buffer_pool_dirty_pct,
                "buffer_pool_free_pct": item.buffer_pool_free_pct,
                "log_waits": item.log_waits,
                "row_lock_waits": item.row_lock_waits,
                "deadlocks": item.deadlocks,
                "recommendation": "Review high InnoDB pressure with scoped credentials and redact workload-sensitive evidence before sharing outside the database team",
            }),
        ));
    }
}

fn stale_finding(
    item: &InnoDbBufferPoolInventoryItem,
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
            "Inventory data for MySQL InnoDB buffer pool connection {} is {} hours old (threshold {} hours)",
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
    item: &InnoDbBufferPoolInventoryItem,
    pillar: Pillar,
    reason_code: &str,
    severity: Severity,
    message: String,
    evidence: Value,
) -> InventoryFinding {
    InventoryFinding {
        resource_id: item.connection_id.clone(),
        arn: format!("mysql://innodb-buffer-pool/{}", item.connection_id),
        pillar,
        reason_code: reason_code.to_string(),
        severity,
        message,
        evidence,
    }
}

fn has_owner_metadata(item: &InnoDbBufferPoolInventoryItem) -> bool {
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

fn has_buffer_pool_metrics(item: &InnoDbBufferPoolInventoryItem) -> bool {
    item.buffer_pool_pages_total > 0
        || item.buffer_pool_hit_ratio.is_some()
        || item.buffer_pool_dirty_pct.is_some()
        || item.buffer_pool_free_pct.is_some()
}

fn has_cost_rightsize_pressure(item: &InnoDbBufferPoolInventoryItem) -> bool {
    has_buffer_pool_metrics(item)
        && (item
            .buffer_pool_hit_ratio
            .is_some_and(|hit_ratio| hit_ratio < 0.95)
            || item
                .buffer_pool_free_pct
                .is_some_and(|free_pct| free_pct >= 50.0))
}

fn has_resilience_pressure(item: &InnoDbBufferPoolInventoryItem) -> bool {
    has_buffer_pool_metrics(item)
        && (item
            .buffer_pool_dirty_pct
            .is_some_and(|dirty_pct| dirty_pct >= 50.0)
            || item
                .buffer_pool_free_pct
                .is_some_and(|free_pct| free_pct <= 5.0)
            || item.log_waits > 0
            || item.row_lock_waits > 0
            || item.deadlocks > 0)
}

fn has_security_review_pressure(item: &InnoDbBufferPoolInventoryItem) -> bool {
    has_buffer_pool_metrics(item)
        && (item
            .buffer_pool_hit_ratio
            .is_some_and(|hit_ratio| hit_ratio < 0.95)
            || item
                .buffer_pool_dirty_pct
                .is_some_and(|dirty_pct| dirty_pct >= 50.0)
            || item.log_waits > 0
            || item.row_lock_waits > 0
            || item.deadlocks > 0)
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
        hit_ratio: Option<f64>,
        pages_total: i64,
        pages_free: i64,
        pages_dirty: i64,
        dirty_pct: Option<f64>,
        free_pct: Option<f64>,
        log_waits: i64,
        row_lock_waits: i64,
        deadlocks: i64,
        labels: BTreeMap<String, String>,
        collected_at: DateTime<Utc>,
    ) -> InnoDbBufferPoolInventoryItem {
        InnoDbBufferPoolInventoryItem {
            connection_id: "mysql-1".to_string(),
            connection_name: "orders-mysql".to_string(),
            owner: owner.map(str::to_string),
            labels,
            server_version: server_version.map(str::to_string),
            buffer_pool_hit_ratio: hit_ratio,
            buffer_pool_pages_total: pages_total,
            buffer_pool_pages_free: pages_free,
            buffer_pool_pages_dirty: pages_dirty,
            buffer_pool_dirty_pct: dirty_pct,
            buffer_pool_free_pct: free_pct,
            log_waits,
            row_lock_waits,
            row_lock_time_ms: row_lock_waits * 250,
            deadlocks,
            collected_at,
        }
    }

    fn healthy_item() -> InnoDbBufferPoolInventoryItem {
        item(
            Some("database-platform"),
            Some("8.0.36"),
            Some(0.995),
            1000,
            150,
            25,
            Some(2.5),
            Some(15.0),
            0,
            0,
            0,
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
    fn cost_flags_missing_owner_missing_metrics_and_rightsize_pressure() {
        let missing_metrics = item(
            Some(""),
            Some("8.0.36"),
            None,
            0,
            0,
            0,
            None,
            None,
            0,
            0,
            0,
            BTreeMap::new(),
            now(),
        );
        let oversized_or_miss_heavy = item(
            Some("database-platform"),
            Some("8.0.36"),
            Some(0.90),
            1000,
            650,
            25,
            Some(2.5),
            Some(65.0),
            0,
            0,
            0,
            labels(&[("owner", "database-platform")]),
            now(),
        );

        let report = evaluate_mysql_innodb_buffer_pool_inventory(
            &[missing_metrics, oversized_or_miss_heavy],
            Pillar::Cost,
            now(),
        );
        let codes = reason_codes(&report);

        assert!(codes.contains(&REASON_COST_OWNER_NOT_RECORDED));
        assert!(codes.contains(&REASON_COST_NO_BUFFER_POOL_METRICS));
        assert!(codes.contains(&REASON_COST_RIGHTSIZE_REVIEW));
    }

    #[test]
    fn resilience_flags_missing_metrics_and_dirty_or_low_free_pressure() {
        let missing_metrics = item(
            Some("database-platform"),
            Some("8.0.36"),
            None,
            0,
            0,
            0,
            None,
            None,
            0,
            0,
            0,
            labels(&[("owner", "database-platform")]),
            now(),
        );
        let checkpoint_pressure = item(
            Some("database-platform"),
            Some("8.0.36"),
            Some(0.99),
            1000,
            20,
            650,
            Some(65.0),
            Some(2.0),
            2,
            5,
            1,
            labels(&[("owner", "database-platform")]),
            now(),
        );

        let report = evaluate_mysql_innodb_buffer_pool_inventory(
            &[missing_metrics, checkpoint_pressure],
            Pillar::Resilience,
            now(),
        );
        let codes = reason_codes(&report);

        assert!(codes.contains(&REASON_RES_NO_BUFFER_POOL_METRICS));
        assert!(codes.contains(&REASON_RES_DIRTY_OR_HEADROOM_PRESSURE));
    }

    #[test]
    fn security_flags_missing_version_missing_metrics_and_pressure_review() {
        let missing_metrics = item(
            Some("database-platform"),
            None,
            None,
            0,
            0,
            0,
            None,
            None,
            0,
            0,
            0,
            labels(&[("owner", "database-platform")]),
            now(),
        );
        let pressure = item(
            Some("database-platform"),
            Some("8.0.36"),
            Some(0.91),
            1000,
            10,
            600,
            Some(60.0),
            Some(1.0),
            1,
            2,
            0,
            labels(&[("owner", "database-platform")]),
            now(),
        );

        let report = evaluate_mysql_innodb_buffer_pool_inventory(
            &[missing_metrics, pressure],
            Pillar::Security,
            now(),
        );
        let codes = reason_codes(&report);

        assert!(codes.contains(&REASON_SEC_VERSION_NOT_RECORDED));
        assert!(codes.contains(&REASON_SEC_NO_BUFFER_POOL_METRICS));
        assert!(codes.contains(&REASON_SEC_PRESSURE_REVIEW));
    }

    #[test]
    fn stale_inventory_is_counted_for_any_pillar() {
        let target = item(
            Some("database-platform"),
            Some("8.0.36"),
            Some(0.99),
            1000,
            150,
            25,
            Some(2.5),
            Some(15.0),
            0,
            0,
            0,
            labels(&[("owner", "database-platform")]),
            now() - Duration::hours(DEFAULT_STALE_AFTER_HOURS + 2),
        );

        let report = evaluate_mysql_innodb_buffer_pool_inventory(&[target], Pillar::Cost, now());

        assert_eq!(report.stale_resources, 1);
        assert_eq!(reason_codes(&report), vec![REASON_INV_STALE_DATA]);
    }

    #[test]
    fn healthy_buffer_pool_passes_claimed_pillars() {
        for pillar in [Pillar::Cost, Pillar::Resilience, Pillar::Security] {
            let item = healthy_item();
            let report = evaluate_mysql_innodb_buffer_pool_inventory(&[item], pillar, now());

            assert_eq!(report.resources_evaluated, 1);
            assert_eq!(report.stale_resources, 0);
            assert!(report.findings.is_empty());
            assert_eq!(report.score, 100);
        }
    }
}

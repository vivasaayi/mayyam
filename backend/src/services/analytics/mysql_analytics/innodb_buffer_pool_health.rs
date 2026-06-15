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

// Deterministic MySQL InnoDB buffer pool health evaluator for roadmap rows
// 03-MYSQL-AI-TRIAGER-00247/00254/00275.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::services::analytics::mysql_analytics::mysql_telemetry::MySqlTelemetrySnapshot;
use crate::services::aws::inventory::types::{
    score_pillar, InventoryFinding, Pillar, PillarReport, Severity, DEFAULT_STALE_AFTER_HOURS,
};

pub const RESOURCE_TYPE: &str = "MySqlInnoDbBufferPoolHealth";
pub const REASON_COST_NO_METRICS: &str = "MYSQL_INNODB_BUFFER_POOL_HEALTH_COST_NO_METRICS";
pub const REASON_COST_RIGHTSIZE_PRESSURE: &str =
    "MYSQL_INNODB_BUFFER_POOL_HEALTH_COST_RIGHTSIZE_PRESSURE";
pub const REASON_RES_NO_METRICS: &str = "MYSQL_INNODB_BUFFER_POOL_HEALTH_RES_NO_METRICS";
pub const REASON_RES_DIRTY_OR_HEADROOM_PRESSURE: &str =
    "MYSQL_INNODB_BUFFER_POOL_HEALTH_RES_DIRTY_OR_HEADROOM_PRESSURE";
pub const REASON_RES_LOCK_OR_DEADLOCK_PRESSURE: &str =
    "MYSQL_INNODB_BUFFER_POOL_HEALTH_RES_LOCK_OR_DEADLOCK_PRESSURE";
pub const REASON_SEC_VERSION_MISSING: &str = "MYSQL_INNODB_BUFFER_POOL_HEALTH_SEC_VERSION_MISSING";
pub const REASON_SEC_NO_METRICS: &str = "MYSQL_INNODB_BUFFER_POOL_HEALTH_SEC_NO_METRICS";
pub const REASON_SEC_PRESSURE_REVIEW: &str = "MYSQL_INNODB_BUFFER_POOL_HEALTH_SEC_PRESSURE_REVIEW";
pub const REASON_INV_STALE_DATA: &str = "MYSQL_INNODB_BUFFER_POOL_HEALTH_STALE_DATA";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InnoDbBufferPoolHealthItem {
    pub connection_id: String,
    pub connection_name: String,
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

pub fn evaluate_mysql_innodb_buffer_pool_health(
    items: &[InnoDbBufferPoolHealthItem],
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

pub fn innodb_buffer_pool_health_item_from_telemetry(
    connection_id: &str,
    connection_name: &str,
    snapshot: &MySqlTelemetrySnapshot,
) -> InnoDbBufferPoolHealthItem {
    InnoDbBufferPoolHealthItem {
        connection_id: connection_id.to_string(),
        connection_name: connection_name.to_string(),
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
    item: &InnoDbBufferPoolHealthItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if !has_buffer_pool_metrics(item) {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_NO_METRICS,
            Severity::High,
            format!(
                "InnoDB buffer pool health for {} has no buffer pool metrics",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "buffer_pool_pages_total": item.buffer_pool_pages_total,
                "buffer_pool_hit_ratio": item.buffer_pool_hit_ratio,
                "recommendation": "Collect InnoDB buffer pool status before making cost or instance-size recommendations",
            }),
        ));
    }

    if has_cost_rightsize_pressure(item) {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_RIGHTSIZE_PRESSURE,
            Severity::Medium,
            format!(
                "InnoDB buffer pool health for {} needs sizing review",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "buffer_pool_hit_ratio": item.buffer_pool_hit_ratio,
                "buffer_pool_free_pct": item.buffer_pool_free_pct,
                "buffer_pool_pages_total": item.buffer_pool_pages_total,
                "buffer_pool_pages_free": item.buffer_pool_pages_free,
                "recommendation": "Compare working-set fit, free pages, and disk-read pressure before changing database memory spend",
            }),
        ));
    }
}

fn evaluate_resilience(
    item: &InnoDbBufferPoolHealthItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if !has_buffer_pool_metrics(item) {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_NO_METRICS,
            Severity::High,
            format!(
                "InnoDB buffer pool health for {} has no resilience evidence",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "buffer_pool_pages_total": item.buffer_pool_pages_total,
                "recommendation": "Collect buffer pool and lock counters so incidents can distinguish cache, checkpoint, and storage pressure",
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
                "InnoDB buffer pool health for {} shows dirty-page or headroom pressure",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "buffer_pool_dirty_pct": item.buffer_pool_dirty_pct,
                "buffer_pool_free_pct": item.buffer_pool_free_pct,
                "log_waits": item.log_waits,
                "recommendation": "Investigate dirty page, free page, and log wait pressure before failover or memory remediation",
            }),
        ));
    }

    if item.row_lock_waits > 0 || item.deadlocks > 0 {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_LOCK_OR_DEADLOCK_PRESSURE,
            Severity::Medium,
            format!(
                "InnoDB buffer pool health for {} includes lock or deadlock pressure",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "row_lock_waits": item.row_lock_waits,
                "row_lock_time_ms": item.row_lock_time_ms,
                "deadlocks": item.deadlocks,
            }),
        ));
    }
}

fn evaluate_security(
    item: &InnoDbBufferPoolHealthItem,
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
            REASON_SEC_VERSION_MISSING,
            Severity::Medium,
            format!(
                "InnoDB buffer pool health for {} has no server version evidence",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "server_version": item.server_version,
                "recommendation": "Record server version with InnoDB health evidence so version-specific security guidance can be mapped",
            }),
        ));
    }

    if !has_buffer_pool_metrics(item) {
        findings.push(finding(
            item,
            pillar,
            REASON_SEC_NO_METRICS,
            Severity::High,
            format!(
                "InnoDB buffer pool health for {} has no metrics for security review",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "buffer_pool_pages_total": item.buffer_pool_pages_total,
                "recommendation": "Collect InnoDB metrics so privileged incident diagnostics are not required during security review",
            }),
        ));
    }

    if item.deadlocks > 0 || item.row_lock_waits >= 100 {
        findings.push(finding(
            item,
            pillar,
            REASON_SEC_PRESSURE_REVIEW,
            Severity::Medium,
            format!(
                "InnoDB buffer pool health for {} includes lock pressure that should be reviewed",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "row_lock_waits": item.row_lock_waits,
                "deadlocks": item.deadlocks,
                "recommendation": "Review lock pressure for administrative or metadata-lock patterns before exporting evidence",
            }),
        ));
    }
}

fn stale_finding(
    item: &InnoDbBufferPoolHealthItem,
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
            "InnoDB buffer pool health data for {} is {} hours old (threshold {} hours)",
            item.connection_name, age_hours, DEFAULT_STALE_AFTER_HOURS
        ),
        json!({
            "connection_id": item.connection_id,
            "collected_at": item.collected_at,
            "age_hours": age_hours,
            "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
        }),
    ))
}

fn finding(
    item: &InnoDbBufferPoolHealthItem,
    pillar: Pillar,
    reason_code: &str,
    severity: Severity,
    message: String,
    evidence: Value,
) -> InventoryFinding {
    InventoryFinding {
        resource_id: item.connection_id.clone(),
        arn: format!("mysql-innodb-buffer-pool-health:{}", item.connection_id),
        pillar,
        reason_code: reason_code.to_string(),
        severity,
        message,
        evidence,
    }
}

fn has_buffer_pool_metrics(item: &InnoDbBufferPoolHealthItem) -> bool {
    item.buffer_pool_pages_total > 0 || item.buffer_pool_hit_ratio.is_some()
}

fn has_cost_rightsize_pressure(item: &InnoDbBufferPoolHealthItem) -> bool {
    item.buffer_pool_hit_ratio
        .map(|ratio| ratio < 0.99)
        .unwrap_or(false)
        || item
            .buffer_pool_free_pct
            .map(|free_pct| free_pct > 70.0)
            .unwrap_or(false)
}

fn has_resilience_pressure(item: &InnoDbBufferPoolHealthItem) -> bool {
    item.buffer_pool_dirty_pct
        .map(|dirty_pct| dirty_pct >= 70.0)
        .unwrap_or(false)
        || item
            .buffer_pool_free_pct
            .map(|free_pct| free_pct <= 5.0)
            .unwrap_or(false)
        || item.log_waits > 0
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    fn item() -> InnoDbBufferPoolHealthItem {
        InnoDbBufferPoolHealthItem {
            connection_id: "conn-1".to_string(),
            connection_name: "prod-mysql".to_string(),
            server_version: Some("8.0.36".to_string()),
            buffer_pool_hit_ratio: Some(0.999),
            buffer_pool_pages_total: 10_000,
            buffer_pool_pages_free: 2_000,
            buffer_pool_pages_dirty: 50,
            buffer_pool_dirty_pct: Some(0.5),
            buffer_pool_free_pct: Some(20.0),
            log_waits: 0,
            row_lock_waits: 0,
            row_lock_time_ms: 0,
            deadlocks: 0,
            collected_at: Utc::now(),
        }
    }

    #[test]
    fn cost_flags_missing_metrics_and_rightsize_pressure() {
        let now = Utc::now();
        let missing = InnoDbBufferPoolHealthItem {
            buffer_pool_hit_ratio: None,
            buffer_pool_pages_total: 0,
            ..item()
        };
        let oversized = InnoDbBufferPoolHealthItem {
            buffer_pool_free_pct: Some(80.0),
            ..item()
        };

        let report =
            evaluate_mysql_innodb_buffer_pool_health(&[missing, oversized], Pillar::Cost, now);

        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_COST_NO_METRICS));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_COST_RIGHTSIZE_PRESSURE));
    }

    #[test]
    fn resilience_flags_metric_gap_dirty_pressure_and_lock_pressure() {
        let now = Utc::now();
        let mut item = item();
        item.buffer_pool_dirty_pct = Some(80.0);
        item.buffer_pool_free_pct = Some(2.0);
        item.log_waits = 3;
        item.row_lock_waits = 5;
        item.deadlocks = 1;

        let report = evaluate_mysql_innodb_buffer_pool_health(&[item], Pillar::Resilience, now);

        assert_eq!(report.findings.len(), 2);
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_RES_DIRTY_OR_HEADROOM_PRESSURE));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_RES_LOCK_OR_DEADLOCK_PRESSURE));
    }

    #[test]
    fn security_flags_missing_version_missing_metrics_and_lock_review() {
        let now = Utc::now();
        let mut item = item();
        item.server_version = None;
        item.buffer_pool_hit_ratio = None;
        item.buffer_pool_pages_total = 0;
        item.row_lock_waits = 150;

        let report = evaluate_mysql_innodb_buffer_pool_health(&[item], Pillar::Security, now);

        assert_eq!(report.findings.len(), 3);
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_SEC_VERSION_MISSING));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_SEC_NO_METRICS));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_SEC_PRESSURE_REVIEW));
    }

    #[test]
    fn stale_health_data_is_counted_for_any_pillar() {
        let now = Utc::now();
        let mut item = item();
        item.collected_at = now - Duration::hours(DEFAULT_STALE_AFTER_HOURS + 2);

        let report = evaluate_mysql_innodb_buffer_pool_health(&[item], Pillar::Cost, now);

        assert_eq!(report.stale_resources, 1);
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_INV_STALE_DATA));
    }

    #[test]
    fn healthy_innodb_buffer_pool_health_passes_claimed_pillars() {
        let now = Utc::now();
        let items = vec![item()];

        for pillar in [Pillar::Cost, Pillar::Resilience, Pillar::Security] {
            let report = evaluate_mysql_innodb_buffer_pool_health(&items, pillar, now);
            assert_eq!(report.score, 100);
            assert!(report.findings.is_empty());
        }
    }
}

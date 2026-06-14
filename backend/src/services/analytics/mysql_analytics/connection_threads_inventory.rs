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

// Deterministic connection threads inventory evaluator for roadmap rows
// 03-MYSQL-AI-TRIAGER-00638/00645/00666.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::services::analytics::mysql_analytics::mysql_telemetry::MySqlTelemetrySnapshot;
use crate::services::aws::inventory::types::{
    score_pillar, InventoryFinding, Pillar, PillarReport, Severity, COST_ALLOCATION_TAG_KEYS,
    DEFAULT_STALE_AFTER_HOURS,
};

pub const RESOURCE_TYPE: &str = "MySqlConnectionThreads";
pub const REASON_COST_OWNER_NOT_RECORDED: &str = "MYSQL_CONNECTION_THREADS_COST_OWNER_NOT_RECORDED";
pub const REASON_COST_NO_THREAD_METRICS: &str = "MYSQL_CONNECTION_THREADS_COST_NO_THREAD_METRICS";
pub const REASON_COST_IDLE_POOL_PRESSURE: &str = "MYSQL_CONNECTION_THREADS_COST_IDLE_POOL_PRESSURE";
pub const REASON_RES_NO_THREAD_METRICS: &str = "MYSQL_CONNECTION_THREADS_RES_NO_THREAD_METRICS";
pub const REASON_RES_CONNECTION_SATURATION: &str =
    "MYSQL_CONNECTION_THREADS_RES_CONNECTION_SATURATION";
pub const REASON_RES_CONNECTION_ERRORS: &str = "MYSQL_CONNECTION_THREADS_RES_CONNECTION_ERRORS";
pub const REASON_SEC_NO_THREAD_METRICS: &str = "MYSQL_CONNECTION_THREADS_SEC_NO_THREAD_METRICS";
pub const REASON_SEC_ABORTED_CONNECTS: &str = "MYSQL_CONNECTION_THREADS_SEC_ABORTED_CONNECTS";
pub const REASON_INV_STALE_DATA: &str = "MYSQL_CONNECTION_THREADS_INV_STALE_DATA";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionThreadsInventoryItem {
    pub connection_id: String,
    pub connection_name: String,
    pub owner: Option<String>,
    pub labels: BTreeMap<String, String>,
    pub thread_metric_count: usize,
    pub max_connections: i64,
    pub max_used_connections: i64,
    pub threads_connected: i64,
    pub threads_running: i64,
    pub threads_cached: i64,
    pub connection_usage_pct: Option<f64>,
    pub peak_connection_usage_pct: Option<f64>,
    pub aborted_clients: i64,
    pub aborted_connects: i64,
    pub connection_errors_total: i64,
    pub collected_at: DateTime<Utc>,
}

pub fn evaluate_mysql_connection_threads_inventory(
    items: &[ConnectionThreadsInventoryItem],
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

pub fn connection_threads_item_from_telemetry(
    connection_id: &str,
    connection_name: &str,
    owner: Option<String>,
    labels: BTreeMap<String, String>,
    snapshot: &MySqlTelemetrySnapshot,
) -> ConnectionThreadsInventoryItem {
    let connection_errors_total = snapshot.connections.connection_errors.values().sum();
    ConnectionThreadsInventoryItem {
        connection_id: connection_id.to_string(),
        connection_name: connection_name.to_string(),
        owner,
        labels,
        thread_metric_count: 8 + snapshot.connections.connection_errors.len(),
        max_connections: snapshot.connections.max_connections,
        max_used_connections: snapshot.connections.max_used_connections,
        threads_connected: snapshot.connections.threads_connected,
        threads_running: snapshot.connections.threads_running,
        threads_cached: snapshot.connections.threads_cached,
        connection_usage_pct: snapshot.connections.connection_usage_pct,
        peak_connection_usage_pct: snapshot.connections.peak_connection_usage_pct,
        aborted_clients: snapshot.connections.aborted_clients,
        aborted_connects: snapshot.connections.aborted_connects,
        connection_errors_total,
        collected_at: snapshot.collected_at,
    }
}

fn evaluate_cost(
    item: &ConnectionThreadsInventoryItem,
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
                "Connection thread inventory for {} has no owner, team, project, or cost-center metadata",
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

    if !has_thread_metrics(item) {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_NO_THREAD_METRICS,
            Severity::High,
            format!(
                "Connection thread inventory for {} has no collected thread metrics",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "connection_name": item.connection_name,
                "thread_metric_count": item.thread_metric_count,
                "recommendation": "Collect max_connections, Threads_connected, Threads_running, Max_used_connections, aborted connection counts, and connection errors before tuning pool size or capacity",
            }),
        ));
    }

    if has_idle_pool_pressure(item) {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_IDLE_POOL_PRESSURE,
            Severity::Medium,
            format!(
                "Connection pool for {} has high connected threads with low active work",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "connection_name": item.connection_name,
                "max_connections": item.max_connections,
                "threads_connected": item.threads_connected,
                "threads_running": item.threads_running,
                "connection_usage_pct": usage_pct(item),
                "active_thread_ratio": active_thread_ratio(item),
                "recommendation": "Tune application pool limits, idle timeout, and connection reuse before raising max_connections or scaling the database only for idle sessions",
            }),
        ));
    }
}

fn evaluate_resilience(
    item: &ConnectionThreadsInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if !has_thread_metrics(item) {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_NO_THREAD_METRICS,
            Severity::High,
            format!(
                "Connection thread inventory for {} has no resilience evidence",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "connection_name": item.connection_name,
                "thread_metric_count": item.thread_metric_count,
                "recommendation": "Collect current and peak connection usage, running threads, and connection error counters so saturation risk can be evaluated deterministically",
            }),
        ));
    }

    if has_connection_saturation(item) {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_CONNECTION_SATURATION,
            Severity::High,
            format!(
                "Connection threads for {} are near max_connections",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "connection_name": item.connection_name,
                "max_connections": item.max_connections,
                "max_used_connections": item.max_used_connections,
                "threads_connected": item.threads_connected,
                "threads_running": item.threads_running,
                "connection_usage_pct": usage_pct(item),
                "peak_connection_usage_pct": peak_usage_pct(item),
                "recommendation": "Inspect connection leaks, pool sizing, slow queries, and memory headroom before increasing max_connections",
            }),
        ));
    }

    if has_connection_errors(item) {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_CONNECTION_ERRORS,
            Severity::Medium,
            format!(
                "Connection thread inventory for {} has connection errors",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "connection_name": item.connection_name,
                "aborted_clients": item.aborted_clients,
                "aborted_connects": item.aborted_connects,
                "connection_errors_total": item.connection_errors_total,
                "recommendation": "Review max connection errors, network resets, client timeouts, and pool retry behavior before treating the connection tier as stable",
            }),
        ));
    }
}

fn evaluate_security(
    item: &ConnectionThreadsInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if !has_thread_metrics(item) {
        findings.push(finding(
            item,
            pillar,
            REASON_SEC_NO_THREAD_METRICS,
            Severity::High,
            format!(
                "Connection thread inventory for {} has no security evidence",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "connection_name": item.connection_name,
                "thread_metric_count": item.thread_metric_count,
                "recommendation": "Collect aborted connect counts and connection error counters so unusual client churn can be triaged without ad hoc privileged diagnostics",
            }),
        ));
    }

    if has_aborted_connect_pressure(item) {
        findings.push(finding(
            item,
            pillar,
            REASON_SEC_ABORTED_CONNECTS,
            Severity::Medium,
            format!(
                "Connection thread inventory for {} has elevated aborted connects",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "connection_name": item.connection_name,
                "aborted_clients": item.aborted_clients,
                "aborted_connects": item.aborted_connects,
                "connection_errors_total": item.connection_errors_total,
                "recommendation": "Review source clients, authentication failures, network paths, and connection rate controls before dismissing aborted connects as benign pool churn",
            }),
        ));
    }
}

fn stale_finding(
    item: &ConnectionThreadsInventoryItem,
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
            "Inventory data for connection thread resource {} is {} hours old (threshold {} hours)",
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
    item: &ConnectionThreadsInventoryItem,
    pillar: Pillar,
    reason_code: &str,
    severity: Severity,
    message: String,
    evidence: Value,
) -> InventoryFinding {
    InventoryFinding {
        resource_id: item.connection_id.clone(),
        arn: format!("mysql://connection-threads/{}", item.connection_id),
        pillar,
        reason_code: reason_code.to_string(),
        severity,
        message,
        evidence,
    }
}

fn has_owner_metadata(item: &ConnectionThreadsInventoryItem) -> bool {
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

fn has_thread_metrics(item: &ConnectionThreadsInventoryItem) -> bool {
    item.thread_metric_count > 0
}

fn has_idle_pool_pressure(item: &ConnectionThreadsInventoryItem) -> bool {
    has_thread_metrics(item)
        && item.threads_connected >= 50
        && usage_pct(item).unwrap_or(0.0) >= 70.0
        && active_thread_ratio(item).unwrap_or(1.0) <= 0.10
}

fn has_connection_saturation(item: &ConnectionThreadsInventoryItem) -> bool {
    has_thread_metrics(item)
        && (usage_pct(item).unwrap_or(0.0) >= 90.0
            || peak_usage_pct(item).unwrap_or(0.0) >= 95.0
            || pct(item.max_used_connections, item.max_connections).unwrap_or(0.0) >= 90.0)
}

fn has_connection_errors(item: &ConnectionThreadsInventoryItem) -> bool {
    has_thread_metrics(item)
        && (item.connection_errors_total > 0
            || item.aborted_clients > 0
            || item.aborted_connects > 0)
}

fn has_aborted_connect_pressure(item: &ConnectionThreadsInventoryItem) -> bool {
    has_thread_metrics(item) && (item.aborted_connects >= 10 || item.connection_errors_total >= 3)
}

fn usage_pct(item: &ConnectionThreadsInventoryItem) -> Option<f64> {
    item.connection_usage_pct
        .or_else(|| pct(item.threads_connected, item.max_connections))
}

fn peak_usage_pct(item: &ConnectionThreadsInventoryItem) -> Option<f64> {
    item.peak_connection_usage_pct
        .or_else(|| pct(item.max_used_connections, item.max_connections))
}

fn active_thread_ratio(item: &ConnectionThreadsInventoryItem) -> Option<f64> {
    if item.threads_connected <= 0 {
        return None;
    }
    Some(item.threads_running as f64 / item.threads_connected as f64)
}

fn pct(numerator: i64, denominator: i64) -> Option<f64> {
    if denominator <= 0 {
        return None;
    }
    Some((numerator as f64 / denominator as f64) * 100.0)
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
        thread_metric_count: usize,
        max_connections: i64,
        max_used_connections: i64,
        threads_connected: i64,
        threads_running: i64,
        connection_usage_pct: Option<f64>,
        peak_connection_usage_pct: Option<f64>,
        aborted_clients: i64,
        aborted_connects: i64,
        connection_errors_total: i64,
        labels: BTreeMap<String, String>,
        collected_at: DateTime<Utc>,
    ) -> ConnectionThreadsInventoryItem {
        ConnectionThreadsInventoryItem {
            connection_id: "mysql-1".to_string(),
            connection_name: "orders-mysql".to_string(),
            owner: owner.map(str::to_string),
            labels,
            thread_metric_count,
            max_connections,
            max_used_connections,
            threads_connected,
            threads_running,
            threads_cached: 10,
            connection_usage_pct,
            peak_connection_usage_pct,
            aborted_clients,
            aborted_connects,
            connection_errors_total,
            collected_at,
        }
    }

    fn healthy_item() -> ConnectionThreadsInventoryItem {
        item(
            Some("database-platform"),
            8,
            400,
            120,
            50,
            5,
            Some(12.5),
            Some(30.0),
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
    fn cost_flags_missing_owner_missing_metrics_and_idle_pool_pressure() {
        let missing_metrics = item(
            Some(""),
            0,
            0,
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
        let idle_pressure = item(
            Some("database-platform"),
            8,
            200,
            190,
            180,
            3,
            Some(90.0),
            Some(95.0),
            0,
            0,
            0,
            labels(&[("owner", "database-platform")]),
            now(),
        );

        let report = evaluate_mysql_connection_threads_inventory(
            &[missing_metrics, idle_pressure],
            Pillar::Cost,
            now(),
        );
        let codes = reason_codes(&report);

        assert!(codes.contains(&REASON_COST_OWNER_NOT_RECORDED));
        assert!(codes.contains(&REASON_COST_NO_THREAD_METRICS));
        assert!(codes.contains(&REASON_COST_IDLE_POOL_PRESSURE));
    }

    #[test]
    fn resilience_flags_missing_metrics_saturation_and_connection_errors() {
        let missing_metrics = item(
            Some("database-platform"),
            0,
            0,
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
        let saturated = item(
            Some("database-platform"),
            8,
            200,
            198,
            190,
            80,
            Some(95.0),
            Some(99.0),
            3,
            2,
            4,
            labels(&[("owner", "database-platform")]),
            now(),
        );

        let report = evaluate_mysql_connection_threads_inventory(
            &[missing_metrics, saturated],
            Pillar::Resilience,
            now(),
        );
        let codes = reason_codes(&report);

        assert!(codes.contains(&REASON_RES_NO_THREAD_METRICS));
        assert!(codes.contains(&REASON_RES_CONNECTION_SATURATION));
        assert!(codes.contains(&REASON_RES_CONNECTION_ERRORS));
    }

    #[test]
    fn security_flags_missing_metrics_and_aborted_connects() {
        let missing_metrics = item(
            Some("database-platform"),
            0,
            0,
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
        let aborted_connects = item(
            Some("database-platform"),
            8,
            200,
            100,
            40,
            8,
            Some(20.0),
            Some(50.0),
            12,
            25,
            3,
            labels(&[("owner", "database-platform")]),
            now(),
        );

        let report = evaluate_mysql_connection_threads_inventory(
            &[missing_metrics, aborted_connects],
            Pillar::Security,
            now(),
        );
        let codes = reason_codes(&report);

        assert!(codes.contains(&REASON_SEC_NO_THREAD_METRICS));
        assert!(codes.contains(&REASON_SEC_ABORTED_CONNECTS));
    }

    #[test]
    fn stale_inventory_is_counted_for_any_pillar() {
        let stale = item(
            Some("database-platform"),
            8,
            200,
            100,
            40,
            8,
            Some(20.0),
            Some(50.0),
            0,
            0,
            0,
            labels(&[("owner", "database-platform")]),
            now() - Duration::hours(49),
        );

        let report =
            evaluate_mysql_connection_threads_inventory(&[stale], Pillar::Resilience, now());

        assert_eq!(report.stale_resources, 1);
        assert!(reason_codes(&report).contains(&REASON_INV_STALE_DATA));
    }

    #[test]
    fn healthy_connection_threads_pass_claimed_pillars() {
        for pillar in [Pillar::Cost, Pillar::Resilience, Pillar::Security] {
            let report =
                evaluate_mysql_connection_threads_inventory(&[healthy_item()], pillar, now());

            assert_eq!(report.resources_evaluated, 1);
            assert_eq!(report.stale_resources, 0);
            assert!(report.findings.is_empty());
            assert!(report.score >= 99);
        }
    }
}

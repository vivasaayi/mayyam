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

// Deterministic MySQL wait events inventory evaluator for roadmap rows
// 03-MYSQL-AI-TRIAGER-00197/00204/00225.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::services::analytics::mysql_analytics::mysql_telemetry::{
    MySqlTelemetrySnapshot, MySqlWaitTelemetry,
};
use crate::services::aws::inventory::types::{
    score_pillar, InventoryFinding, Pillar, PillarReport, Severity, COST_ALLOCATION_TAG_KEYS,
    DEFAULT_STALE_AFTER_HOURS,
};

pub const RESOURCE_TYPE: &str = "MySqlWaitEvents";
pub const REASON_COST_OWNER_NOT_RECORDED: &str = "MYSQL_WAIT_EVENTS_COST_OWNER_NOT_RECORDED";
pub const REASON_COST_NO_WAIT_COVERAGE: &str = "MYSQL_WAIT_EVENTS_COST_NO_WAIT_COVERAGE";
pub const REASON_COST_HEAVY_WAITS: &str = "MYSQL_WAIT_EVENTS_COST_HEAVY_WAITS";
pub const REASON_RES_NO_WAIT_COVERAGE: &str = "MYSQL_WAIT_EVENTS_RES_NO_WAIT_COVERAGE";
pub const REASON_RES_LOCK_OR_SYNC_WAITS: &str = "MYSQL_WAIT_EVENTS_RES_LOCK_OR_SYNC_WAITS";
pub const REASON_SEC_VERSION_NOT_RECORDED: &str = "MYSQL_WAIT_EVENTS_SEC_VERSION_NOT_RECORDED";
pub const REASON_SEC_NO_WAIT_COVERAGE: &str = "MYSQL_WAIT_EVENTS_SEC_NO_WAIT_COVERAGE";
pub const REASON_SEC_LOCK_WAIT_REVIEW: &str = "MYSQL_WAIT_EVENTS_SEC_LOCK_WAIT_REVIEW";
pub const REASON_INV_STALE_DATA: &str = "MYSQL_WAIT_EVENTS_INV_STALE_DATA";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WaitEventsInventoryItem {
    pub connection_id: String,
    pub connection_name: String,
    pub owner: Option<String>,
    pub labels: BTreeMap<String, String>,
    pub server_version: Option<String>,
    pub wait_event_count: usize,
    pub total_wait_ms: f64,
    pub max_avg_wait_ms: f64,
    pub io_wait_event_count: usize,
    pub lock_wait_event_count: usize,
    pub sync_wait_event_count: usize,
    pub high_wait_event_count: usize,
    pub collected_at: DateTime<Utc>,
}

pub fn evaluate_mysql_wait_events_inventory(
    items: &[WaitEventsInventoryItem],
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

pub fn wait_events_item_from_telemetry(
    connection_id: &str,
    connection_name: &str,
    owner: Option<String>,
    labels: BTreeMap<String, String>,
    snapshot: &MySqlTelemetrySnapshot,
) -> WaitEventsInventoryItem {
    let total_wait_ms = snapshot.waits.iter().map(|event| event.total_wait_ms).sum();
    let max_avg_wait_ms = snapshot
        .waits
        .iter()
        .map(|event| event.avg_wait_ms)
        .fold(0.0, f64::max);
    let io_wait_event_count = snapshot
        .waits
        .iter()
        .filter(|event| is_io_wait(event))
        .count();
    let lock_wait_event_count = snapshot
        .waits
        .iter()
        .filter(|event| is_lock_wait(event))
        .count();
    let sync_wait_event_count = snapshot
        .waits
        .iter()
        .filter(|event| is_sync_wait(event))
        .count();
    let high_wait_event_count = snapshot
        .waits
        .iter()
        .filter(|event| is_high_wait(event))
        .count();

    WaitEventsInventoryItem {
        connection_id: connection_id.to_string(),
        connection_name: connection_name.to_string(),
        owner,
        labels,
        server_version: snapshot.server.version.clone(),
        wait_event_count: snapshot.waits.len(),
        total_wait_ms,
        max_avg_wait_ms,
        io_wait_event_count,
        lock_wait_event_count,
        sync_wait_event_count,
        high_wait_event_count,
        collected_at: snapshot.collected_at,
    }
}

fn evaluate_cost(
    item: &WaitEventsInventoryItem,
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
                "MySQL wait events inventory for connection {} has no owner, team, project, or cost-center metadata",
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

    if item.wait_event_count == 0 {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_NO_WAIT_COVERAGE,
            Severity::High,
            format!(
                "MySQL wait events inventory for connection {} has no wait-event coverage",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "connection_name": item.connection_name,
                "wait_event_count": item.wait_event_count,
                "recommendation": "Collect Performance Schema wait events so cost recommendations can distinguish slow SQL from storage, lock, or synchronization waits",
            }),
        ));
    }

    if has_heavy_waits(item) {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_HEAVY_WAITS,
            Severity::Medium,
            format!(
                "MySQL wait events show material wait time for connection {}",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "connection_name": item.connection_name,
                "total_wait_ms": item.total_wait_ms,
                "max_avg_wait_ms": item.max_avg_wait_ms,
                "io_wait_event_count": item.io_wait_event_count,
                "high_wait_event_count": item.high_wait_event_count,
                "recommendation": "Review top wait classes before scaling database capacity so spend changes target the actual bottleneck",
            }),
        ));
    }
}

fn evaluate_resilience(
    item: &WaitEventsInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if item.wait_event_count == 0 {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_NO_WAIT_COVERAGE,
            Severity::High,
            format!(
                "MySQL wait events inventory for connection {} has no evidence for resilience triage",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "connection_name": item.connection_name,
                "wait_event_count": item.wait_event_count,
                "recommendation": "Collect wait events so incidents can be triaged by lock, IO, and synchronization bottlenecks instead of relying on database-wide symptoms",
            }),
        ));
    }

    if has_lock_or_sync_pressure(item) {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_LOCK_OR_SYNC_WAITS,
            Severity::High,
            format!(
                "MySQL wait events show lock or synchronization pressure for connection {}",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "connection_name": item.connection_name,
                "lock_wait_event_count": item.lock_wait_event_count,
                "sync_wait_event_count": item.sync_wait_event_count,
                "high_wait_event_count": item.high_wait_event_count,
                "max_avg_wait_ms": item.max_avg_wait_ms,
                "recommendation": "Investigate high lock or synchronization waits before treating the database as generally unhealthy or triggering failover action",
            }),
        ));
    }
}

fn evaluate_security(
    item: &WaitEventsInventoryItem,
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
                "MySQL wait events inventory for connection {} has no recorded server version",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "connection_name": item.connection_name,
                "server_version": item.server_version,
                "recommendation": "Record MySQL server version with wait-event evidence so version-specific security guidance can be mapped deterministically",
            }),
        ));
    }

    if item.wait_event_count == 0 {
        findings.push(finding(
            item,
            pillar,
            REASON_SEC_NO_WAIT_COVERAGE,
            Severity::High,
            format!(
                "MySQL wait events inventory for connection {} has no wait-class evidence for security review",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "connection_name": item.connection_name,
                "wait_event_count": item.wait_event_count,
                "recommendation": "Collect wait events so metadata-lock or synchronization patterns can be reviewed without requiring privileged ad hoc queries during incidents",
            }),
        ));
    }

    if item.lock_wait_event_count > 0 && item.high_wait_event_count > 0 {
        findings.push(finding(
            item,
            pillar,
            REASON_SEC_LOCK_WAIT_REVIEW,
            Severity::Medium,
            format!(
                "MySQL wait events for connection {} include high lock waits that should be reviewed",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "connection_name": item.connection_name,
                "lock_wait_event_count": item.lock_wait_event_count,
                "high_wait_event_count": item.high_wait_event_count,
                "recommendation": "Review high lock waits for administrative or metadata-lock patterns before exporting evidence outside the scoped database team",
            }),
        ));
    }
}

fn stale_finding(
    item: &WaitEventsInventoryItem,
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
            "Inventory data for MySQL wait events connection {} is {} hours old (threshold {} hours)",
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
    item: &WaitEventsInventoryItem,
    pillar: Pillar,
    reason_code: &str,
    severity: Severity,
    message: String,
    evidence: Value,
) -> InventoryFinding {
    InventoryFinding {
        resource_id: item.connection_id.clone(),
        arn: format!("mysql://wait-events/{}", item.connection_id),
        pillar,
        reason_code: reason_code.to_string(),
        severity,
        message,
        evidence,
    }
}

fn has_owner_metadata(item: &WaitEventsInventoryItem) -> bool {
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

fn has_heavy_waits(item: &WaitEventsInventoryItem) -> bool {
    item.total_wait_ms >= 10_000.0 || item.high_wait_event_count > 0
}

fn has_lock_or_sync_pressure(item: &WaitEventsInventoryItem) -> bool {
    item.high_wait_event_count > 0
        && (item.lock_wait_event_count > 0 || item.sync_wait_event_count > 0)
        || item.max_avg_wait_ms >= 100.0
}

fn is_io_wait(event: &MySqlWaitTelemetry) -> bool {
    event.event_name.starts_with("wait/io/")
}

fn is_lock_wait(event: &MySqlWaitTelemetry) -> bool {
    event.event_name.starts_with("wait/lock/")
}

fn is_sync_wait(event: &MySqlWaitTelemetry) -> bool {
    event.event_name.starts_with("wait/synch/")
}

fn is_high_wait(event: &MySqlWaitTelemetry) -> bool {
    event.total_wait_ms >= 10_000.0 || event.avg_wait_ms >= 100.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::aws::inventory::types::Pillar;
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
        wait_event_count: usize,
        total_wait_ms: f64,
        max_avg_wait_ms: f64,
        io_wait_event_count: usize,
        lock_wait_event_count: usize,
        sync_wait_event_count: usize,
        high_wait_event_count: usize,
        labels: BTreeMap<String, String>,
        collected_at: DateTime<Utc>,
    ) -> WaitEventsInventoryItem {
        WaitEventsInventoryItem {
            connection_id: "mysql-1".to_string(),
            connection_name: "orders-mysql".to_string(),
            owner: owner.map(str::to_string),
            labels,
            server_version: server_version.map(str::to_string),
            wait_event_count,
            total_wait_ms,
            max_avg_wait_ms,
            io_wait_event_count,
            lock_wait_event_count,
            sync_wait_event_count,
            high_wait_event_count,
            collected_at,
        }
    }

    fn healthy_item() -> WaitEventsInventoryItem {
        item(
            Some("database-platform"),
            Some("8.0.36"),
            4,
            1_200.0,
            15.0,
            2,
            0,
            1,
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
    fn cost_flags_missing_owner_missing_coverage_and_heavy_waits() {
        let target = item(
            Some(""),
            Some("8.0.36"),
            0,
            25_000.0,
            60.0,
            3,
            0,
            0,
            2,
            BTreeMap::new(),
            now(),
        );

        let report = evaluate_mysql_wait_events_inventory(&[target], Pillar::Cost, now());
        let codes = reason_codes(&report);

        assert!(codes.contains(&REASON_COST_OWNER_NOT_RECORDED));
        assert!(codes.contains(&REASON_COST_NO_WAIT_COVERAGE));
        assert!(codes.contains(&REASON_COST_HEAVY_WAITS));
    }

    #[test]
    fn resilience_flags_missing_coverage_and_lock_or_sync_waits() {
        let target = item(
            Some("database-platform"),
            Some("8.0.36"),
            0,
            8_000.0,
            250.0,
            0,
            2,
            1,
            3,
            labels(&[("owner", "database-platform")]),
            now(),
        );

        let report = evaluate_mysql_wait_events_inventory(&[target], Pillar::Resilience, now());
        let codes = reason_codes(&report);

        assert!(codes.contains(&REASON_RES_NO_WAIT_COVERAGE));
        assert!(codes.contains(&REASON_RES_LOCK_OR_SYNC_WAITS));
    }

    #[test]
    fn security_flags_missing_version_missing_coverage_and_lock_wait_review() {
        let target = item(
            Some("database-platform"),
            None,
            0,
            5_000.0,
            50.0,
            0,
            2,
            0,
            1,
            labels(&[("owner", "database-platform")]),
            now(),
        );

        let report = evaluate_mysql_wait_events_inventory(&[target], Pillar::Security, now());
        let codes = reason_codes(&report);

        assert!(codes.contains(&REASON_SEC_VERSION_NOT_RECORDED));
        assert!(codes.contains(&REASON_SEC_NO_WAIT_COVERAGE));
        assert!(codes.contains(&REASON_SEC_LOCK_WAIT_REVIEW));
    }

    #[test]
    fn stale_inventory_is_counted_for_any_pillar() {
        let target = WaitEventsInventoryItem {
            collected_at: now() - Duration::hours(25),
            ..healthy_item()
        };

        let report = evaluate_mysql_wait_events_inventory(&[target], Pillar::Cost, now());

        assert_eq!(report.stale_resources, 1);
        assert!(reason_codes(&report).contains(&REASON_INV_STALE_DATA));
    }

    #[test]
    fn healthy_wait_events_pass_claimed_pillars() {
        let target = healthy_item();

        for pillar in [Pillar::Cost, Pillar::Resilience, Pillar::Security] {
            let report =
                evaluate_mysql_wait_events_inventory(std::slice::from_ref(&target), pillar, now());
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

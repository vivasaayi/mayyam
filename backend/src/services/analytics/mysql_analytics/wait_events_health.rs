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

// Deterministic MySQL wait events health evaluator for roadmap rows
// 03-MYSQL-AI-TRIAGER-00198/00205/00226.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::services::analytics::mysql_analytics::mysql_telemetry::{
    MySqlFindingSeverity, MySqlTelemetrySnapshot, MySqlWaitTelemetry,
};
use crate::services::aws::inventory::types::{
    score_pillar, InventoryFinding, Pillar, PillarReport, Severity, DEFAULT_STALE_AFTER_HOURS,
};

pub const RESOURCE_TYPE: &str = "MySqlWaitEventsHealth";
pub const REASON_COST_NO_WAIT_COVERAGE: &str = "MYSQL_WAIT_EVENTS_HEALTH_COST_NO_WAIT_COVERAGE";
pub const REASON_COST_HEAVY_WAITS: &str = "MYSQL_WAIT_EVENTS_HEALTH_COST_HEAVY_WAITS";
pub const REASON_RES_NO_WAIT_COVERAGE: &str = "MYSQL_WAIT_EVENTS_HEALTH_RES_NO_WAIT_COVERAGE";
pub const REASON_RES_LOCK_OR_SYNC_WAITS: &str = "MYSQL_WAIT_EVENTS_HEALTH_RES_LOCK_OR_SYNC_WAITS";
pub const REASON_RES_HIGH_PRIORITY_FINDINGS: &str =
    "MYSQL_WAIT_EVENTS_HEALTH_RES_HIGH_PRIORITY_FINDINGS";
pub const REASON_SEC_VERSION_MISSING: &str = "MYSQL_WAIT_EVENTS_HEALTH_SEC_VERSION_MISSING";
pub const REASON_SEC_NO_WAIT_COVERAGE: &str = "MYSQL_WAIT_EVENTS_HEALTH_SEC_NO_WAIT_COVERAGE";
pub const REASON_SEC_LOCK_WAIT_REVIEW: &str = "MYSQL_WAIT_EVENTS_HEALTH_SEC_LOCK_WAIT_REVIEW";
pub const REASON_INV_STALE_DATA: &str = "MYSQL_WAIT_EVENTS_HEALTH_STALE_DATA";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WaitEventsHealthItem {
    pub connection_id: String,
    pub connection_name: String,
    pub server_version: Option<String>,
    pub wait_event_count: usize,
    pub total_wait_ms: f64,
    pub max_avg_wait_ms: f64,
    pub io_wait_event_count: usize,
    pub lock_wait_event_count: usize,
    pub sync_wait_event_count: usize,
    pub high_wait_event_count: usize,
    pub high_priority_findings: usize,
    pub collected_at: DateTime<Utc>,
}

pub fn evaluate_mysql_wait_events_health(
    items: &[WaitEventsHealthItem],
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

pub fn wait_events_health_item_from_telemetry(
    connection_id: &str,
    connection_name: &str,
    snapshot: &MySqlTelemetrySnapshot,
) -> WaitEventsHealthItem {
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
    let high_priority_findings = snapshot
        .findings
        .iter()
        .filter(|finding| {
            matches!(
                finding.severity,
                MySqlFindingSeverity::Critical | MySqlFindingSeverity::High
            )
        })
        .count();

    WaitEventsHealthItem {
        connection_id: connection_id.to_string(),
        connection_name: connection_name.to_string(),
        server_version: snapshot.server.version.clone(),
        wait_event_count: snapshot.waits.len(),
        total_wait_ms,
        max_avg_wait_ms,
        io_wait_event_count,
        lock_wait_event_count,
        sync_wait_event_count,
        high_wait_event_count,
        high_priority_findings,
        collected_at: snapshot.collected_at,
    }
}

fn evaluate_cost(
    item: &WaitEventsHealthItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if item.wait_event_count == 0 {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_NO_WAIT_COVERAGE,
            Severity::High,
            format!(
                "Wait events health for {} has no wait-event coverage",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "wait_event_count": item.wait_event_count,
                "recommendation": "Collect wait events before estimating whether spend changes should target CPU, storage, lock, or synchronization bottlenecks",
            }),
        ));
    }

    if item.total_wait_ms >= 10_000.0 || item.high_wait_event_count > 0 {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_HEAVY_WAITS,
            Severity::Medium,
            format!(
                "Wait events health for {} shows material wait pressure",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "total_wait_ms": item.total_wait_ms,
                "max_avg_wait_ms": item.max_avg_wait_ms,
                "io_wait_event_count": item.io_wait_event_count,
                "high_wait_event_count": item.high_wait_event_count,
                "recommendation": "Review top wait classes before scaling capacity so spend changes target the actual bottleneck",
            }),
        ));
    }
}

fn evaluate_resilience(
    item: &WaitEventsHealthItem,
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
                "Wait events health for {} has no evidence for resilience triage",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "wait_event_count": item.wait_event_count,
                "recommendation": "Collect wait events so incidents can be triaged by lock, IO, and synchronization bottlenecks",
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
                "Wait events health for {} shows lock or synchronization pressure",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "lock_wait_event_count": item.lock_wait_event_count,
                "sync_wait_event_count": item.sync_wait_event_count,
                "high_wait_event_count": item.high_wait_event_count,
                "max_avg_wait_ms": item.max_avg_wait_ms,
                "recommendation": "Investigate dominant lock or synchronization waits before failover or automated remediation",
            }),
        ));
    }

    if item.high_priority_findings > 0 {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_HIGH_PRIORITY_FINDINGS,
            Severity::Medium,
            format!(
                "Wait events health for {} includes high-priority telemetry findings",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "high_priority_findings": item.high_priority_findings,
            }),
        ));
    }
}

fn evaluate_security(
    item: &WaitEventsHealthItem,
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
                "Wait events health for {} has no server version evidence",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "server_version": item.server_version,
                "recommendation": "Record server version with wait-event evidence so version-specific security guidance can be mapped",
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
                "Wait events health for {} has no wait-class evidence for security review",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "wait_event_count": item.wait_event_count,
                "recommendation": "Collect wait events so metadata-lock or synchronization patterns can be reviewed without privileged ad hoc queries",
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
                "Wait events health for {} includes high lock waits that should be reviewed",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "lock_wait_event_count": item.lock_wait_event_count,
                "high_wait_event_count": item.high_wait_event_count,
                "recommendation": "Review high lock waits for administrative or metadata-lock patterns before exporting evidence",
            }),
        ));
    }
}

fn stale_finding(
    item: &WaitEventsHealthItem,
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
            "Wait events health data for {} is {} hours old (threshold {} hours)",
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
    item: &WaitEventsHealthItem,
    pillar: Pillar,
    reason_code: &str,
    severity: Severity,
    message: String,
    evidence: Value,
) -> InventoryFinding {
    InventoryFinding {
        resource_id: item.connection_id.clone(),
        arn: format!("mysql-wait-events-health:{}", item.connection_id),
        pillar,
        reason_code: reason_code.to_string(),
        severity,
        message,
        evidence,
    }
}

fn has_lock_or_sync_pressure(item: &WaitEventsHealthItem) -> bool {
    (item.high_wait_event_count > 0
        && (item.lock_wait_event_count > 0 || item.sync_wait_event_count > 0))
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
    use chrono::Duration;

    fn item() -> WaitEventsHealthItem {
        WaitEventsHealthItem {
            connection_id: "conn-1".to_string(),
            connection_name: "prod-mysql".to_string(),
            server_version: Some("8.0.36".to_string()),
            wait_event_count: 4,
            total_wait_ms: 100.0,
            max_avg_wait_ms: 5.0,
            io_wait_event_count: 1,
            lock_wait_event_count: 0,
            sync_wait_event_count: 0,
            high_wait_event_count: 0,
            high_priority_findings: 0,
            collected_at: Utc::now(),
        }
    }

    #[test]
    fn cost_flags_missing_coverage_and_heavy_waits() {
        let now = Utc::now();
        let mut item = item();
        item.wait_event_count = 0;
        item.total_wait_ms = 20_000.0;
        item.high_wait_event_count = 1;

        let report = evaluate_mysql_wait_events_health(&[item], Pillar::Cost, now);

        assert_eq!(report.resources_evaluated, 1);
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_COST_NO_WAIT_COVERAGE));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_COST_HEAVY_WAITS));
    }

    #[test]
    fn resilience_flags_missing_coverage_lock_pressure_and_high_priority_findings() {
        let now = Utc::now();
        let mut item = item();
        item.wait_event_count = 0;
        item.lock_wait_event_count = 1;
        item.high_wait_event_count = 1;
        item.max_avg_wait_ms = 150.0;
        item.high_priority_findings = 2;

        let report = evaluate_mysql_wait_events_health(&[item], Pillar::Resilience, now);

        assert_eq!(report.findings.len(), 3);
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_RES_NO_WAIT_COVERAGE));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_RES_LOCK_OR_SYNC_WAITS));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_RES_HIGH_PRIORITY_FINDINGS));
    }

    #[test]
    fn security_flags_missing_version_missing_coverage_and_lock_review() {
        let now = Utc::now();
        let mut item = item();
        item.server_version = None;
        item.wait_event_count = 0;
        item.lock_wait_event_count = 1;
        item.high_wait_event_count = 1;

        let report = evaluate_mysql_wait_events_health(&[item], Pillar::Security, now);

        assert_eq!(report.findings.len(), 3);
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_SEC_VERSION_MISSING));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_SEC_NO_WAIT_COVERAGE));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_SEC_LOCK_WAIT_REVIEW));
    }

    #[test]
    fn stale_health_data_is_counted_for_any_pillar() {
        let now = Utc::now();
        let mut item = item();
        item.collected_at = now - Duration::hours(DEFAULT_STALE_AFTER_HOURS + 2);

        let report = evaluate_mysql_wait_events_health(&[item], Pillar::Cost, now);

        assert_eq!(report.stale_resources, 1);
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_INV_STALE_DATA));
    }

    #[test]
    fn healthy_wait_events_health_passes_claimed_pillars() {
        let now = Utc::now();
        let items = vec![item()];

        for pillar in [Pillar::Cost, Pillar::Resilience, Pillar::Security] {
            let report = evaluate_mysql_wait_events_health(&items, pillar, now);
            assert_eq!(report.score, 100);
            assert!(report.findings.is_empty());
        }
    }
}

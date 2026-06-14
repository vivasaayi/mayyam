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

// Deterministic MySQL Performance Schema inventory evaluator for roadmap rows
// 03-MYSQL-AI-TRIAGER-00001/00008/00029.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::services::analytics::mysql_analytics::mysql_telemetry::{
    MySqlFindingSeverity, MySqlTelemetrySnapshot,
};
use crate::services::aws::inventory::types::{
    score_pillar, InventoryFinding, Pillar, PillarReport, Severity, COST_ALLOCATION_TAG_KEYS,
    DEFAULT_STALE_AFTER_HOURS,
};

pub const RESOURCE_TYPE: &str = "MySqlPerformanceSchema";
pub const REASON_COST_OWNER_NOT_RECORDED: &str = "MYSQL_PERF_SCHEMA_COST_OWNER_NOT_RECORDED";
pub const REASON_COST_DIGEST_COVERAGE_MISSING: &str =
    "MYSQL_PERF_SCHEMA_COST_DIGEST_COVERAGE_MISSING";
pub const REASON_RES_DISABLED: &str = "MYSQL_PERF_SCHEMA_RES_DISABLED";
pub const REASON_RES_WAIT_COVERAGE_MISSING: &str = "MYSQL_PERF_SCHEMA_RES_WAIT_COVERAGE_MISSING";
pub const REASON_SEC_VERSION_NOT_RECORDED: &str = "MYSQL_PERF_SCHEMA_SEC_VERSION_NOT_RECORDED";
pub const REASON_SEC_TELEMETRY_DISABLED: &str = "MYSQL_PERF_SCHEMA_SEC_TELEMETRY_DISABLED";
pub const REASON_INV_STALE_DATA: &str = "MYSQL_PERF_SCHEMA_INV_STALE_DATA";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceSchemaInventoryItem {
    pub connection_id: String,
    pub connection_name: String,
    pub owner: Option<String>,
    pub labels: BTreeMap<String, String>,
    pub server_version: Option<String>,
    pub performance_schema_enabled: Option<String>,
    pub sys_schema_available: bool,
    pub statement_digest_count: usize,
    pub wait_event_count: usize,
    pub table_count: usize,
    pub index_count: usize,
    pub high_priority_findings: usize,
    pub collected_at: DateTime<Utc>,
}

pub fn evaluate_mysql_performance_schema_inventory(
    items: &[PerformanceSchemaInventoryItem],
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

pub fn performance_schema_item_from_telemetry(
    connection_id: &str,
    connection_name: &str,
    owner: Option<String>,
    labels: BTreeMap<String, String>,
    snapshot: &MySqlTelemetrySnapshot,
) -> PerformanceSchemaInventoryItem {
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

    PerformanceSchemaInventoryItem {
        connection_id: connection_id.to_string(),
        connection_name: connection_name.to_string(),
        owner,
        labels,
        server_version: snapshot.server.version.clone(),
        performance_schema_enabled: snapshot.server.performance_schema_enabled.clone(),
        sys_schema_available: snapshot.server.sys_schema_available,
        statement_digest_count: snapshot.statements.len(),
        wait_event_count: snapshot.waits.len(),
        table_count: snapshot.tables.len(),
        index_count: snapshot.indexes.len(),
        high_priority_findings,
        collected_at: snapshot.collected_at,
    }
}

fn evaluate_cost(
    item: &PerformanceSchemaInventoryItem,
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
                "MySQL Performance Schema inventory for connection {} has no owner, team, project, or cost-center metadata",
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

    if item.statement_digest_count == 0 {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_DIGEST_COVERAGE_MISSING,
            Severity::High,
            format!(
                "MySQL Performance Schema inventory for connection {} has no statement digest coverage",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "connection_name": item.connection_name,
                "performance_schema_enabled": item.performance_schema_enabled,
                "statement_digest_count": item.statement_digest_count,
                "recommendation": "Enable statement digest collection so expensive SQL can be attributed before cost recommendations are made",
            }),
        ));
    }
}

fn evaluate_resilience(
    item: &PerformanceSchemaInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if !is_performance_schema_enabled(item) {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_DISABLED,
            Severity::High,
            format!(
                "MySQL Performance Schema is not enabled for connection {}",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "connection_name": item.connection_name,
                "performance_schema_enabled": item.performance_schema_enabled,
                "sys_schema_available": item.sys_schema_available,
                "recommendation": "Enable Performance Schema in a read-only telemetry rollout before relying on automated triage",
            }),
        ));
    }

    if item.wait_event_count == 0 {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_WAIT_COVERAGE_MISSING,
            Severity::Medium,
            format!(
                "MySQL Performance Schema inventory for connection {} has no wait event coverage",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "connection_name": item.connection_name,
                "wait_event_count": item.wait_event_count,
                "statement_digest_count": item.statement_digest_count,
                "recommendation": "Collect wait summaries so lock, I/O, and concurrency symptoms can be separated during incident triage",
            }),
        ));
    }
}

fn evaluate_security(
    item: &PerformanceSchemaInventoryItem,
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
                "MySQL Performance Schema inventory for connection {} has no recorded server version",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "connection_name": item.connection_name,
                "server_version": item.server_version,
                "recommendation": "Record MySQL server version with Performance Schema evidence so security advisories can be mapped deterministically",
            }),
        ));
    }

    if !is_performance_schema_enabled(item) {
        findings.push(finding(
            item,
            pillar,
            REASON_SEC_TELEMETRY_DISABLED,
            Severity::High,
            format!(
                "MySQL Performance Schema security telemetry is disabled for connection {}",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "connection_name": item.connection_name,
                "performance_schema_enabled": item.performance_schema_enabled,
                "high_priority_findings": item.high_priority_findings,
                "recommendation": "Enable Performance Schema telemetry before relying on privilege, query, or connection anomaly evidence",
            }),
        ));
    }
}

fn stale_finding(
    item: &PerformanceSchemaInventoryItem,
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
            "Inventory data for MySQL Performance Schema connection {} is {} hours old (threshold {} hours)",
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
    item: &PerformanceSchemaInventoryItem,
    pillar: Pillar,
    reason_code: &str,
    severity: Severity,
    message: String,
    evidence: Value,
) -> InventoryFinding {
    InventoryFinding {
        resource_id: item.connection_id.clone(),
        arn: format!("mysql://performance-schema/{}", item.connection_id),
        pillar,
        reason_code: reason_code.to_string(),
        severity,
        message,
        evidence,
    }
}

fn has_owner_metadata(item: &PerformanceSchemaInventoryItem) -> bool {
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

fn is_performance_schema_enabled(item: &PerformanceSchemaInventoryItem) -> bool {
    item.performance_schema_enabled
        .as_deref()
        .map(str::trim)
        .map(|value| {
            value.eq_ignore_ascii_case("on")
                || value.eq_ignore_ascii_case("1")
                || value.eq_ignore_ascii_case("true")
                || value.eq_ignore_ascii_case("yes")
        })
        .unwrap_or(false)
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
        performance_schema_enabled: Option<&str>,
        server_version: Option<&str>,
        statement_digest_count: usize,
        wait_event_count: usize,
        labels: BTreeMap<String, String>,
        collected_at: DateTime<Utc>,
    ) -> PerformanceSchemaInventoryItem {
        PerformanceSchemaInventoryItem {
            connection_id: "mysql-1".to_string(),
            connection_name: "orders-mysql".to_string(),
            owner: owner.map(str::to_string),
            labels,
            server_version: server_version.map(str::to_string),
            performance_schema_enabled: performance_schema_enabled.map(str::to_string),
            sys_schema_available: true,
            statement_digest_count,
            wait_event_count,
            table_count: 12,
            index_count: 20,
            high_priority_findings: 0,
            collected_at,
        }
    }

    fn healthy_item() -> PerformanceSchemaInventoryItem {
        item(
            Some("database-platform"),
            Some("ON"),
            Some("8.0.36"),
            25,
            9,
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
    fn cost_flags_missing_owner_and_digest_coverage() {
        let target = item(
            Some(""),
            Some("ON"),
            Some("8.0.36"),
            0,
            4,
            BTreeMap::new(),
            now(),
        );

        let report = evaluate_mysql_performance_schema_inventory(&[target], Pillar::Cost, now());
        let codes = reason_codes(&report);

        assert!(codes.contains(&REASON_COST_OWNER_NOT_RECORDED));
        assert!(codes.contains(&REASON_COST_DIGEST_COVERAGE_MISSING));
    }

    #[test]
    fn resilience_flags_disabled_schema_and_missing_waits() {
        let target = item(
            Some("database-platform"),
            Some("OFF"),
            Some("8.0.36"),
            12,
            0,
            labels(&[("owner", "database-platform")]),
            now(),
        );

        let report =
            evaluate_mysql_performance_schema_inventory(&[target], Pillar::Resilience, now());
        let codes = reason_codes(&report);

        assert!(codes.contains(&REASON_RES_DISABLED));
        assert!(codes.contains(&REASON_RES_WAIT_COVERAGE_MISSING));
    }

    #[test]
    fn security_flags_missing_version_and_disabled_telemetry() {
        let target = item(
            Some("database-platform"),
            Some("OFF"),
            None,
            12,
            4,
            labels(&[("owner", "database-platform")]),
            now(),
        );

        let report =
            evaluate_mysql_performance_schema_inventory(&[target], Pillar::Security, now());
        let codes = reason_codes(&report);

        assert!(codes.contains(&REASON_SEC_VERSION_NOT_RECORDED));
        assert!(codes.contains(&REASON_SEC_TELEMETRY_DISABLED));
    }

    #[test]
    fn stale_inventory_is_counted_for_any_pillar() {
        let target = item(
            Some("database-platform"),
            Some("ON"),
            Some("8.0.36"),
            12,
            4,
            labels(&[("owner", "database-platform")]),
            now() - Duration::hours(25),
        );

        let report = evaluate_mysql_performance_schema_inventory(&[target], Pillar::Cost, now());

        assert_eq!(report.stale_resources, 1);
        assert!(reason_codes(&report).contains(&REASON_INV_STALE_DATA));
    }

    #[test]
    fn healthy_performance_schema_passes_claimed_pillars() {
        let target = healthy_item();

        for pillar in [Pillar::Cost, Pillar::Resilience, Pillar::Security] {
            let report = evaluate_mysql_performance_schema_inventory(
                std::slice::from_ref(&target),
                pillar,
                now(),
            );
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

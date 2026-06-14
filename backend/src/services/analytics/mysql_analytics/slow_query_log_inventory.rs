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

// Deterministic MySQL slow query log inventory evaluator for roadmap rows
// 03-MYSQL-AI-TRIAGER-00099/00106/00127.

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

pub const RESOURCE_TYPE: &str = "MySqlSlowQueryLog";
pub const REASON_COST_OWNER_NOT_RECORDED: &str = "MYSQL_SLOW_QUERY_LOG_COST_OWNER_NOT_RECORDED";
pub const REASON_COST_LOG_DISABLED: &str = "MYSQL_SLOW_QUERY_LOG_COST_LOG_DISABLED";
pub const REASON_COST_THRESHOLD_HIGH: &str = "MYSQL_SLOW_QUERY_LOG_COST_THRESHOLD_HIGH";
pub const REASON_RES_LOG_DISABLED: &str = "MYSQL_SLOW_QUERY_LOG_RES_LOG_DISABLED";
pub const REASON_RES_THRESHOLD_HIGH: &str = "MYSQL_SLOW_QUERY_LOG_RES_THRESHOLD_HIGH";
pub const REASON_SEC_VERSION_NOT_RECORDED: &str = "MYSQL_SLOW_QUERY_LOG_SEC_VERSION_NOT_RECORDED";
pub const REASON_SEC_LOG_DISABLED: &str = "MYSQL_SLOW_QUERY_LOG_SEC_LOG_DISABLED";
pub const REASON_INV_STALE_DATA: &str = "MYSQL_SLOW_QUERY_LOG_INV_STALE_DATA";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlowQueryLogInventoryItem {
    pub connection_id: String,
    pub connection_name: String,
    pub owner: Option<String>,
    pub labels: BTreeMap<String, String>,
    pub server_version: Option<String>,
    pub slow_query_log_enabled: Option<String>,
    pub long_query_time_seconds: Option<f64>,
    pub slow_queries: i64,
    pub statement_digest_count: usize,
    pub high_priority_findings: usize,
    pub collected_at: DateTime<Utc>,
}

pub fn evaluate_mysql_slow_query_log_inventory(
    items: &[SlowQueryLogInventoryItem],
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

pub fn slow_query_log_item_from_telemetry(
    connection_id: &str,
    connection_name: &str,
    owner: Option<String>,
    labels: BTreeMap<String, String>,
    snapshot: &MySqlTelemetrySnapshot,
) -> SlowQueryLogInventoryItem {
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

    SlowQueryLogInventoryItem {
        connection_id: connection_id.to_string(),
        connection_name: connection_name.to_string(),
        owner,
        labels,
        server_version: snapshot.server.version.clone(),
        slow_query_log_enabled: snapshot.server.slow_query_log_enabled.clone(),
        long_query_time_seconds: snapshot.server.long_query_time_seconds,
        slow_queries: snapshot.workload.slow_queries,
        statement_digest_count: snapshot.statements.len(),
        high_priority_findings,
        collected_at: snapshot.collected_at,
    }
}

fn evaluate_cost(
    item: &SlowQueryLogInventoryItem,
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
                "MySQL slow query log inventory for connection {} has no owner, team, project, or cost-center metadata",
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

    if !is_slow_query_log_enabled(item) {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_LOG_DISABLED,
            Severity::High,
            format!(
                "MySQL slow query log is not enabled for connection {}",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "connection_name": item.connection_name,
                "slow_query_log_enabled": item.slow_query_log_enabled,
                "slow_queries": item.slow_queries,
                "statement_digest_count": item.statement_digest_count,
                "recommendation": "Enable slow_query_log in a bounded rollout so expensive SQL can be attributed before cost recommendations are made",
            }),
        ));
    }

    if long_query_time_is_high(item) {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_THRESHOLD_HIGH,
            Severity::Medium,
            format!(
                "MySQL long_query_time is too high for cost attribution on connection {}",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "connection_name": item.connection_name,
                "long_query_time_seconds": item.long_query_time_seconds,
                "recommended_max_seconds": 5.0,
                "recommendation": "Lower long_query_time for a representative workload window so expensive query families are captured",
            }),
        ));
    }
}

fn evaluate_resilience(
    item: &SlowQueryLogInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if !is_slow_query_log_enabled(item) {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_LOG_DISABLED,
            Severity::High,
            format!(
                "MySQL slow query log resilience evidence is disabled for connection {}",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "connection_name": item.connection_name,
                "slow_query_log_enabled": item.slow_query_log_enabled,
                "recommendation": "Enable slow query logging so incident triage can separate isolated slow SQL from database-wide saturation",
            }),
        ));
    }

    if long_query_time_is_high(item) {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_THRESHOLD_HIGH,
            Severity::Medium,
            format!(
                "MySQL slow query threshold is too high for resilience triage on connection {}",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "connection_name": item.connection_name,
                "long_query_time_seconds": item.long_query_time_seconds,
                "recommended_max_seconds": 5.0,
                "recommendation": "Tune long_query_time low enough to catch latency regressions before incidents depend on ad hoc query capture",
            }),
        ));
    }
}

fn evaluate_security(
    item: &SlowQueryLogInventoryItem,
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
                "MySQL slow query log inventory for connection {} has no recorded server version",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "connection_name": item.connection_name,
                "server_version": item.server_version,
                "recommendation": "Record MySQL server version with slow query evidence so version-specific security guidance can be mapped deterministically",
            }),
        ));
    }

    if !is_slow_query_log_enabled(item) {
        findings.push(finding(
            item,
            pillar,
            REASON_SEC_LOG_DISABLED,
            Severity::High,
            format!(
                "MySQL slow query log security evidence is disabled for connection {}",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "connection_name": item.connection_name,
                "slow_query_log_enabled": item.slow_query_log_enabled,
                "high_priority_findings": item.high_priority_findings,
                "recommendation": "Enable slow query logging with appropriate redaction and access controls before relying on query-shape evidence for security reviews",
            }),
        ));
    }
}

fn stale_finding(
    item: &SlowQueryLogInventoryItem,
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
            "Inventory data for MySQL slow query log connection {} is {} hours old (threshold {} hours)",
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
    item: &SlowQueryLogInventoryItem,
    pillar: Pillar,
    reason_code: &str,
    severity: Severity,
    message: String,
    evidence: Value,
) -> InventoryFinding {
    InventoryFinding {
        resource_id: item.connection_id.clone(),
        arn: format!("mysql://slow-query-log/{}", item.connection_id),
        pillar,
        reason_code: reason_code.to_string(),
        severity,
        message,
        evidence,
    }
}

fn has_owner_metadata(item: &SlowQueryLogInventoryItem) -> bool {
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

fn is_slow_query_log_enabled(item: &SlowQueryLogInventoryItem) -> bool {
    item.slow_query_log_enabled
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

fn long_query_time_is_high(item: &SlowQueryLogInventoryItem) -> bool {
    item.long_query_time_seconds
        .map(|seconds| seconds > 5.0)
        .unwrap_or(true)
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
        slow_query_log_enabled: Option<&str>,
        long_query_time_seconds: Option<f64>,
        server_version: Option<&str>,
        labels: BTreeMap<String, String>,
        collected_at: DateTime<Utc>,
    ) -> SlowQueryLogInventoryItem {
        SlowQueryLogInventoryItem {
            connection_id: "mysql-1".to_string(),
            connection_name: "orders-mysql".to_string(),
            owner: owner.map(str::to_string),
            labels,
            server_version: server_version.map(str::to_string),
            slow_query_log_enabled: slow_query_log_enabled.map(str::to_string),
            long_query_time_seconds,
            slow_queries: 12,
            statement_digest_count: 25,
            high_priority_findings: 0,
            collected_at,
        }
    }

    fn healthy_item() -> SlowQueryLogInventoryItem {
        item(
            Some("database-platform"),
            Some("ON"),
            Some(2.0),
            Some("8.0.36"),
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
    fn cost_flags_missing_owner_disabled_log_and_high_threshold() {
        let target = item(
            Some(""),
            Some("OFF"),
            Some(12.0),
            Some("8.0.36"),
            BTreeMap::new(),
            now(),
        );

        let report = evaluate_mysql_slow_query_log_inventory(&[target], Pillar::Cost, now());
        let codes = reason_codes(&report);

        assert!(codes.contains(&REASON_COST_OWNER_NOT_RECORDED));
        assert!(codes.contains(&REASON_COST_LOG_DISABLED));
        assert!(codes.contains(&REASON_COST_THRESHOLD_HIGH));
    }

    #[test]
    fn resilience_flags_disabled_log_and_high_threshold() {
        let target = item(
            Some("database-platform"),
            Some("OFF"),
            Some(10.0),
            Some("8.0.36"),
            labels(&[("owner", "database-platform")]),
            now(),
        );

        let report = evaluate_mysql_slow_query_log_inventory(&[target], Pillar::Resilience, now());
        let codes = reason_codes(&report);

        assert!(codes.contains(&REASON_RES_LOG_DISABLED));
        assert!(codes.contains(&REASON_RES_THRESHOLD_HIGH));
    }

    #[test]
    fn security_flags_missing_version_and_disabled_log() {
        let target = item(
            Some("database-platform"),
            Some("OFF"),
            Some(2.0),
            None,
            labels(&[("owner", "database-platform")]),
            now(),
        );

        let report = evaluate_mysql_slow_query_log_inventory(&[target], Pillar::Security, now());
        let codes = reason_codes(&report);

        assert!(codes.contains(&REASON_SEC_VERSION_NOT_RECORDED));
        assert!(codes.contains(&REASON_SEC_LOG_DISABLED));
    }

    #[test]
    fn stale_inventory_is_counted_for_any_pillar() {
        let target = SlowQueryLogInventoryItem {
            collected_at: now() - Duration::hours(25),
            ..healthy_item()
        };

        let report = evaluate_mysql_slow_query_log_inventory(&[target], Pillar::Cost, now());

        assert_eq!(report.stale_resources, 1);
        assert!(reason_codes(&report).contains(&REASON_INV_STALE_DATA));
    }

    #[test]
    fn healthy_slow_query_log_passes_claimed_pillars() {
        let target = healthy_item();

        for pillar in [Pillar::Cost, Pillar::Resilience, Pillar::Security] {
            let report = evaluate_mysql_slow_query_log_inventory(
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

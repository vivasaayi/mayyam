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

// Deterministic parameter-drift inventory evaluator for roadmap rows
// 03-MYSQL-AI-TRIAGER-01471/01478/01499.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::services::analytics::mysql_analytics::mysql_telemetry::MySqlTelemetrySnapshot;
use crate::services::aws::inventory::types::{
    score_pillar, InventoryFinding, Pillar, PillarReport, Severity, COST_ALLOCATION_TAG_KEYS,
    DEFAULT_STALE_AFTER_HOURS,
};

pub const RESOURCE_TYPE: &str = "MySqlParameterDrift";
pub const REASON_COST_OWNER_NOT_RECORDED: &str = "MYSQL_PARAMETER_DRIFT_COST_OWNER_NOT_RECORDED";
pub const REASON_COST_BASELINE_MISSING: &str = "MYSQL_PARAMETER_DRIFT_COST_BASELINE_MISSING";
pub const REASON_COST_DRIFT_REVIEW: &str = "MYSQL_PARAMETER_DRIFT_COST_DRIFT_REVIEW";
pub const REASON_RES_BASELINE_MISSING: &str = "MYSQL_PARAMETER_DRIFT_RES_BASELINE_MISSING";
pub const REASON_RES_RISKY_SETTINGS: &str = "MYSQL_PARAMETER_DRIFT_RES_RISKY_SETTINGS";
pub const REASON_SEC_OWNER_NOT_RECORDED: &str = "MYSQL_PARAMETER_DRIFT_SEC_OWNER_NOT_RECORDED";
pub const REASON_SEC_AUDIT_BASELINE_MISSING: &str =
    "MYSQL_PARAMETER_DRIFT_SEC_AUDIT_BASELINE_MISSING";
pub const REASON_SEC_DRIFT_REVIEW: &str = "MYSQL_PARAMETER_DRIFT_SEC_DRIFT_REVIEW";
pub const REASON_INV_STALE_DATA: &str = "MYSQL_PARAMETER_DRIFT_INV_STALE_DATA";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParameterDriftInventoryItem {
    pub connection_id: String,
    pub connection_name: String,
    pub owner: Option<String>,
    pub labels: BTreeMap<String, String>,
    pub server_version: Option<String>,
    pub observed_performance_schema: Option<String>,
    pub expected_performance_schema: Option<String>,
    pub observed_slow_query_log: Option<String>,
    pub expected_slow_query_log: Option<String>,
    pub observed_require_secure_transport: Option<String>,
    pub expected_require_secure_transport: Option<String>,
    pub observed_log_bin: Option<String>,
    pub expected_log_bin: Option<String>,
    pub observed_max_connections: i64,
    pub expected_max_connections: Option<i64>,
    pub drift_count: usize,
    pub collected_at: DateTime<Utc>,
}

pub fn evaluate_mysql_parameter_drift_inventory(
    items: &[ParameterDriftInventoryItem],
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

pub fn parameter_drift_item_from_telemetry(
    connection_id: &str,
    connection_name: &str,
    owner: Option<String>,
    labels: BTreeMap<String, String>,
    snapshot: &MySqlTelemetrySnapshot,
) -> ParameterDriftInventoryItem {
    let mut item = ParameterDriftInventoryItem {
        connection_id: connection_id.to_string(),
        connection_name: connection_name.to_string(),
        owner,
        labels,
        server_version: snapshot.server.version.clone(),
        observed_performance_schema: snapshot.server.performance_schema_enabled.clone(),
        expected_performance_schema: None,
        observed_slow_query_log: snapshot.server.slow_query_log_enabled.clone(),
        expected_slow_query_log: None,
        observed_require_secure_transport: snapshot.server.require_secure_transport.clone(),
        expected_require_secure_transport: None,
        observed_log_bin: snapshot.server.log_bin.clone(),
        expected_log_bin: None,
        observed_max_connections: snapshot.connections.max_connections,
        expected_max_connections: None,
        drift_count: 0,
        collected_at: snapshot.collected_at,
    };
    item.drift_count = count_drift(&item);
    item
}

fn evaluate_cost(
    item: &ParameterDriftInventoryItem,
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
                "Parameter-drift inventory for {} has no owner, team, project, or cost-center metadata",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "checked_keys": COST_ALLOCATION_TAG_KEYS,
            }),
        ));
    }

    if !has_baseline(item) {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_BASELINE_MISSING,
            Severity::High,
            format!(
                "Parameter-drift inventory for {} has no expected baseline",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "recommendation": "Record expected parameter baselines before estimating drift remediation, capacity, or managed-service migration cost",
            }),
        ));
    }

    if item.drift_count > 0 {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_DRIFT_REVIEW,
            Severity::Medium,
            format!(
                "Parameter-drift inventory for {} has observed drift",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "drift_count": item.drift_count,
                "recommendation": "Review drift blast radius and rollout cost before changing instance parameters",
            }),
        ));
    }
}

fn evaluate_resilience(
    item: &ParameterDriftInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if !has_baseline(item) {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_BASELINE_MISSING,
            Severity::High,
            format!(
                "Parameter-drift inventory for {} has no recovery baseline",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "recommendation": "Record expected parameters so restore, failover, and replica builds can detect drift deterministically",
            }),
        ));
    }

    if risky_resilience_settings(item) {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_RISKY_SETTINGS,
            Severity::High,
            format!(
                "Parameter-drift inventory for {} has resilience-sensitive settings disabled or drifted",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "observed_performance_schema": item.observed_performance_schema,
                "observed_log_bin": item.observed_log_bin,
                "observed_max_connections": item.observed_max_connections,
                "drift_count": item.drift_count,
                "recommendation": "Resolve resilience-sensitive parameter drift before relying on failover, PITR, or telemetry-driven triage",
            }),
        ));
    }
}

fn evaluate_security(
    item: &ParameterDriftInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if !has_owner_metadata(item) {
        findings.push(finding(
            item,
            pillar,
            REASON_SEC_OWNER_NOT_RECORDED,
            Severity::Medium,
            format!(
                "Parameter-drift inventory for {} has no owner for security review routing",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "checked_keys": COST_ALLOCATION_TAG_KEYS,
            }),
        ));
    }

    if !has_baseline(item) {
        findings.push(finding(
            item,
            pillar,
            REASON_SEC_AUDIT_BASELINE_MISSING,
            Severity::High,
            format!(
                "Parameter-drift inventory for {} has no auditable security baseline",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "recommendation": "Record security-sensitive parameter baselines before approving drift or incident exceptions",
            }),
        ));
    }

    if security_drift(item) {
        findings.push(finding(
            item,
            pillar,
            REASON_SEC_DRIFT_REVIEW,
            Severity::High,
            format!(
                "Parameter-drift inventory for {} has security-sensitive drift",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "observed_require_secure_transport": item.observed_require_secure_transport,
                "expected_require_secure_transport": item.expected_require_secure_transport,
                "observed_log_bin": item.observed_log_bin,
                "expected_log_bin": item.expected_log_bin,
                "recommendation": "Review and remediate security-sensitive parameter drift with an auditable change record",
            }),
        ));
    }
}

fn stale_finding(
    item: &ParameterDriftInventoryItem,
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
            "Inventory data for parameter-drift resource {} is {} hours old (threshold {} hours)",
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
    item: &ParameterDriftInventoryItem,
    pillar: Pillar,
    reason_code: &str,
    severity: Severity,
    message: String,
    evidence: Value,
) -> InventoryFinding {
    InventoryFinding {
        resource_id: item.connection_id.clone(),
        arn: format!("mysql://parameter-drift/{}", item.connection_id),
        pillar,
        reason_code: reason_code.to_string(),
        severity,
        message,
        evidence,
    }
}

fn has_owner_metadata(item: &ParameterDriftInventoryItem) -> bool {
    item.owner.as_deref().is_some_and(|owner| !owner.is_empty())
        || COST_ALLOCATION_TAG_KEYS
            .iter()
            .any(|key| item.labels.get(*key).is_some_and(|value| !value.is_empty()))
}

fn has_baseline(item: &ParameterDriftInventoryItem) -> bool {
    item.expected_performance_schema.is_some()
        || item.expected_slow_query_log.is_some()
        || item.expected_require_secure_transport.is_some()
        || item.expected_log_bin.is_some()
        || item.expected_max_connections.is_some()
}

fn count_drift(item: &ParameterDriftInventoryItem) -> usize {
    usize::from(string_drift(
        &item.observed_performance_schema,
        &item.expected_performance_schema,
    )) + usize::from(string_drift(
        &item.observed_slow_query_log,
        &item.expected_slow_query_log,
    )) + usize::from(string_drift(
        &item.observed_require_secure_transport,
        &item.expected_require_secure_transport,
    )) + usize::from(string_drift(&item.observed_log_bin, &item.expected_log_bin))
        + usize::from(
            item.expected_max_connections
                .is_some_and(|expected| item.observed_max_connections != expected),
        )
}

fn risky_resilience_settings(item: &ParameterDriftInventoryItem) -> bool {
    item.observed_performance_schema
        .as_deref()
        .is_some_and(|value| value.eq_ignore_ascii_case("OFF") || value == "0")
        || item
            .observed_log_bin
            .as_deref()
            .is_some_and(|value| value.eq_ignore_ascii_case("OFF") || value == "0")
        || item.observed_max_connections > 0 && item.observed_max_connections < 100
        || item.drift_count >= 2
}

fn security_drift(item: &ParameterDriftInventoryItem) -> bool {
    string_drift(
        &item.observed_require_secure_transport,
        &item.expected_require_secure_transport,
    ) || string_drift(&item.observed_log_bin, &item.expected_log_bin)
}

fn string_drift(observed: &Option<String>, expected: &Option<String>) -> bool {
    expected.as_deref().is_some_and(|expected| {
        observed
            .as_deref()
            .map(|observed| !observed.eq_ignore_ascii_case(expected))
            .unwrap_or(true)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    fn now() -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2026-06-14T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc)
    }

    fn item(owner: Option<&str>) -> ParameterDriftInventoryItem {
        let mut target = ParameterDriftInventoryItem {
            connection_id: "conn-1".to_string(),
            connection_name: "orders-db".to_string(),
            owner: owner.map(str::to_string),
            labels: BTreeMap::new(),
            server_version: Some("8.0.36".to_string()),
            observed_performance_schema: Some("ON".to_string()),
            expected_performance_schema: Some("ON".to_string()),
            observed_slow_query_log: Some("ON".to_string()),
            expected_slow_query_log: Some("ON".to_string()),
            observed_require_secure_transport: Some("ON".to_string()),
            expected_require_secure_transport: Some("ON".to_string()),
            observed_log_bin: Some("ON".to_string()),
            expected_log_bin: Some("ON".to_string()),
            observed_max_connections: 500,
            expected_max_connections: Some(500),
            drift_count: 0,
            collected_at: now() - Duration::hours(1),
        };
        target.drift_count = count_drift(&target);
        target
    }

    fn reason_codes(report: &PillarReport) -> Vec<&str> {
        report
            .findings
            .iter()
            .map(|finding| finding.reason_code.as_str())
            .collect()
    }

    #[test]
    fn cost_flags_missing_owner_missing_baseline_and_drift() {
        let mut target = item(None);
        target.expected_performance_schema = None;
        target.expected_slow_query_log = None;
        target.expected_require_secure_transport = None;
        target.expected_log_bin = None;
        target.expected_max_connections = Some(200);
        target.drift_count = count_drift(&target);

        let report = evaluate_mysql_parameter_drift_inventory(&[target], Pillar::Cost, now());
        let codes = reason_codes(&report);

        assert!(codes.contains(&REASON_COST_OWNER_NOT_RECORDED));
        assert!(codes.contains(&REASON_COST_DRIFT_REVIEW));
    }

    #[test]
    fn resilience_flags_missing_baseline_and_risky_settings() {
        let mut target = item(Some("db-team"));
        target.expected_performance_schema = None;
        target.expected_slow_query_log = None;
        target.expected_require_secure_transport = None;
        target.expected_log_bin = None;
        target.expected_max_connections = None;
        target.observed_log_bin = Some("OFF".to_string());
        target.observed_max_connections = 50;
        target.drift_count = count_drift(&target);

        let report = evaluate_mysql_parameter_drift_inventory(&[target], Pillar::Resilience, now());
        let codes = reason_codes(&report);

        assert!(codes.contains(&REASON_RES_BASELINE_MISSING));
        assert!(codes.contains(&REASON_RES_RISKY_SETTINGS));
    }

    #[test]
    fn security_flags_missing_owner_missing_baseline_and_security_drift() {
        let mut target = item(None);
        target.observed_require_secure_transport = Some("OFF".to_string());
        target.drift_count = count_drift(&target);

        let report = evaluate_mysql_parameter_drift_inventory(&[target], Pillar::Security, now());
        let codes = reason_codes(&report);

        assert!(codes.contains(&REASON_SEC_OWNER_NOT_RECORDED));
        assert!(codes.contains(&REASON_SEC_DRIFT_REVIEW));
    }

    #[test]
    fn stale_inventory_is_counted_for_any_pillar() {
        let mut target = item(Some("db-team"));
        target.collected_at = now() - Duration::hours(48);

        let report = evaluate_mysql_parameter_drift_inventory(&[target], Pillar::Cost, now());

        assert_eq!(report.stale_resources, 1);
        assert!(reason_codes(&report).contains(&REASON_INV_STALE_DATA));
    }

    #[test]
    fn healthy_parameter_drift_passes_claimed_pillars() {
        let mut target = item(Some("db-team"));
        target
            .labels
            .insert("cost-center".to_string(), "cc-42".to_string());

        for pillar in [Pillar::Cost, Pillar::Resilience, Pillar::Security] {
            let report = evaluate_mysql_parameter_drift_inventory(
                std::slice::from_ref(&target),
                pillar,
                now(),
            );
            assert!(
                report.findings.is_empty(),
                "unexpected findings for {:?}: {:?}",
                pillar,
                report.findings
            );
            assert_eq!(report.score, 100);
        }
    }
}

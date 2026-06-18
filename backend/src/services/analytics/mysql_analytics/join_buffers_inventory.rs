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

// Deterministic join-buffer inventory evaluator for roadmap rows
// 03-MYSQL-AI-TRIAGER-01128/01135/01156.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::services::analytics::mysql_analytics::mysql_telemetry::MySqlTelemetrySnapshot;
use crate::services::aws::inventory::types::{
    score_pillar, InventoryFinding, Pillar, PillarReport, Severity, COST_ALLOCATION_TAG_KEYS,
    DEFAULT_STALE_AFTER_HOURS,
};

const FULL_JOIN_SELECT_PCT_THRESHOLD: f64 = 5.0;
const FULL_JOIN_COUNT_THRESHOLD: i64 = 500;
const RANGE_CHECK_COUNT_THRESHOLD: i64 = 100;

pub const RESOURCE_TYPE: &str = "MySqlJoinBuffers";
pub const REASON_COST_OWNER_NOT_RECORDED: &str = "MYSQL_JOIN_BUFFERS_COST_OWNER_NOT_RECORDED";
pub const REASON_COST_NO_JOIN_EVIDENCE: &str = "MYSQL_JOIN_BUFFERS_COST_NO_JOIN_EVIDENCE";
pub const REASON_COST_FULL_JOIN_PRESSURE: &str = "MYSQL_JOIN_BUFFERS_COST_FULL_JOIN_PRESSURE";
pub const REASON_RES_NO_JOIN_EVIDENCE: &str = "MYSQL_JOIN_BUFFERS_RES_NO_JOIN_EVIDENCE";
pub const REASON_RES_JOIN_BUFFER_RISK: &str = "MYSQL_JOIN_BUFFERS_RES_JOIN_BUFFER_RISK";
pub const REASON_SEC_OWNER_NOT_RECORDED: &str = "MYSQL_JOIN_BUFFERS_SEC_OWNER_NOT_RECORDED";
pub const REASON_SEC_UNROUTED_JOIN_PRESSURE: &str = "MYSQL_JOIN_BUFFERS_SEC_UNROUTED_JOIN_PRESSURE";
pub const REASON_INV_STALE_DATA: &str = "MYSQL_JOIN_BUFFERS_INV_STALE_DATA";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JoinBuffersInventoryItem {
    pub connection_id: String,
    pub connection_name: String,
    pub owner: Option<String>,
    pub labels: BTreeMap<String, String>,
    pub questions: i64,
    pub queries: i64,
    pub com_select: i64,
    pub slow_queries: i64,
    pub select_full_join: i64,
    pub select_full_range_join: i64,
    pub select_range_check: i64,
    pub full_join_select_pct: Option<f64>,
    pub qps_since_start: f64,
    pub collected_at: DateTime<Utc>,
}

pub fn evaluate_mysql_join_buffers_inventory(
    items: &[JoinBuffersInventoryItem],
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

pub fn join_buffers_item_from_telemetry(
    connection_id: &str,
    connection_name: &str,
    owner: Option<String>,
    labels: BTreeMap<String, String>,
    snapshot: &MySqlTelemetrySnapshot,
) -> JoinBuffersInventoryItem {
    JoinBuffersInventoryItem {
        connection_id: connection_id.to_string(),
        connection_name: connection_name.to_string(),
        owner,
        labels,
        questions: snapshot.workload.questions,
        queries: snapshot.workload.queries,
        com_select: snapshot.workload.com_select,
        slow_queries: snapshot.workload.slow_queries,
        select_full_join: snapshot.workload.select_full_join,
        select_full_range_join: snapshot.workload.select_full_range_join,
        select_range_check: snapshot.workload.select_range_check,
        full_join_select_pct: snapshot.workload.full_join_select_pct,
        qps_since_start: snapshot.workload.qps_since_start,
        collected_at: snapshot.collected_at,
    }
}

fn evaluate_cost(
    item: &JoinBuffersInventoryItem,
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
                "Join-buffer inventory for {} has no owner, team, project, or cost-center metadata",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "connection_name": item.connection_name,
                "checked_keys": COST_ALLOCATION_TAG_KEYS,
            }),
        ));
    }

    if !has_join_evidence(item) {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_NO_JOIN_EVIDENCE,
            Severity::High,
            format!(
                "Join-buffer inventory for {} has no join status evidence",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "questions": item.questions,
                "queries": item.queries,
                "recommendation": "Collect Select_full_join, Select_full_range_join, and Select_range_check before estimating join-buffer or index savings",
            }),
        ));
    }

    if has_join_pressure(item) {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_FULL_JOIN_PRESSURE,
            Severity::Medium,
            format!(
                "Join-buffer inventory for {} shows full-join cost pressure",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "com_select": item.com_select,
                "select_full_join": item.select_full_join,
                "select_full_range_join": item.select_full_range_join,
                "select_range_check": item.select_range_check,
                "full_join_select_pct": item.full_join_select_pct,
                "recommendation": "Review missing join indexes and query predicates before increasing join_buffer_size or scaling memory",
            }),
        ));
    }
}

fn evaluate_resilience(
    item: &JoinBuffersInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if !has_join_evidence(item) {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_NO_JOIN_EVIDENCE,
            Severity::High,
            format!(
                "Join-buffer inventory for {} has no resilience evidence",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "recommendation": "Collect join counters so incidents can distinguish full joins from CPU, lock, or buffer pressure",
            }),
        ));
    }

    if has_join_pressure(item) {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_JOIN_BUFFER_RISK,
            Severity::Medium,
            format!(
                "Join-buffer evidence for {} shows full-join resilience risk",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "select_full_join": item.select_full_join,
                "select_full_range_join": item.select_full_range_join,
                "select_range_check": item.select_range_check,
                "slow_queries": item.slow_queries,
                "recommendation": "Correlate full joins with top statement digests before changing memory settings or scheduling index builds",
            }),
        ));
    }
}

fn evaluate_security(
    item: &JoinBuffersInventoryItem,
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
                "Join-buffer inventory for {} has no owner for query-change review routing",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "checked_keys": COST_ALLOCATION_TAG_KEYS,
            }),
        ));
    }

    if !has_owner_metadata(item) && has_join_pressure(item) {
        findings.push(finding(
            item,
            pillar,
            REASON_SEC_UNROUTED_JOIN_PRESSURE,
            Severity::Medium,
            format!(
                "Join pressure for {} cannot be assigned to an accountable owner",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "select_full_join": item.select_full_join,
                "full_join_select_pct": item.full_join_select_pct,
                "recommendation": "Assign ownership before approving index or query rewrites that can affect authorization-sensitive workloads",
            }),
        ));
    }
}

fn stale_finding(
    item: &JoinBuffersInventoryItem,
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
            "Inventory data for join-buffer resource {} is {} hours old (threshold {} hours)",
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
    item: &JoinBuffersInventoryItem,
    pillar: Pillar,
    reason_code: &str,
    severity: Severity,
    message: String,
    evidence: Value,
) -> InventoryFinding {
    InventoryFinding {
        resource_id: item.connection_id.clone(),
        arn: format!("mysql://join-buffers/{}", item.connection_id),
        pillar,
        reason_code: reason_code.to_string(),
        severity,
        message,
        evidence,
    }
}

fn has_owner_metadata(item: &JoinBuffersInventoryItem) -> bool {
    item.owner.as_deref().is_some_and(|owner| !owner.is_empty())
        || COST_ALLOCATION_TAG_KEYS
            .iter()
            .any(|key| item.labels.get(*key).is_some_and(|value| !value.is_empty()))
}

fn has_join_evidence(item: &JoinBuffersInventoryItem) -> bool {
    item.com_select > 0
        || item.select_full_join > 0
        || item.select_full_range_join > 0
        || item.select_range_check > 0
}

fn has_join_pressure(item: &JoinBuffersInventoryItem) -> bool {
    item.full_join_select_pct
        .is_some_and(|pct| pct >= FULL_JOIN_SELECT_PCT_THRESHOLD)
        || item.select_full_join + item.select_full_range_join >= FULL_JOIN_COUNT_THRESHOLD
        || item.select_range_check >= RANGE_CHECK_COUNT_THRESHOLD
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    fn now() -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2026-06-10T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc)
    }

    fn item(
        owner: Option<&str>,
        labels: BTreeMap<String, String>,
        com_select: i64,
        select_full_join: i64,
        select_full_range_join: i64,
        collected_hours_ago: i64,
    ) -> JoinBuffersInventoryItem {
        JoinBuffersInventoryItem {
            connection_id: "conn-1".to_string(),
            connection_name: "orders-db".to_string(),
            owner: owner.map(str::to_string),
            labels,
            questions: 10_000,
            queries: 10_000,
            com_select,
            slow_queries: 25,
            select_full_join,
            select_full_range_join,
            select_range_check: 125,
            full_join_select_pct: if com_select > 0 {
                Some((select_full_join + select_full_range_join) as f64 / com_select as f64 * 100.0)
            } else {
                None
            },
            qps_since_start: 20.0,
            collected_at: now() - Duration::hours(collected_hours_ago),
        }
    }

    fn reason_codes(report: &PillarReport) -> Vec<&str> {
        report
            .findings
            .iter()
            .map(|finding| finding.reason_code.as_str())
            .collect()
    }

    #[test]
    fn cost_flags_missing_owner_and_full_join_pressure() {
        let target = item(None, BTreeMap::new(), 10_000, 500, 100, 1);

        let report = evaluate_mysql_join_buffers_inventory(&[target], Pillar::Cost, now());
        let codes = reason_codes(&report);

        assert!(codes.contains(&REASON_COST_OWNER_NOT_RECORDED));
        assert!(codes.contains(&REASON_COST_FULL_JOIN_PRESSURE));
    }

    #[test]
    fn resilience_flags_join_buffer_risk() {
        let target = item(Some("db-team"), BTreeMap::new(), 10_000, 500, 50, 1);

        let report = evaluate_mysql_join_buffers_inventory(&[target], Pillar::Resilience, now());

        assert!(reason_codes(&report).contains(&REASON_RES_JOIN_BUFFER_RISK));
    }

    #[test]
    fn security_routes_unowned_join_pressure() {
        let target = item(None, BTreeMap::new(), 10_000, 500, 50, 1);

        let report = evaluate_mysql_join_buffers_inventory(&[target], Pillar::Security, now());
        let codes = reason_codes(&report);

        assert!(codes.contains(&REASON_SEC_OWNER_NOT_RECORDED));
        assert!(codes.contains(&REASON_SEC_UNROUTED_JOIN_PRESSURE));
    }

    #[test]
    fn stale_inventory_is_counted_for_any_pillar() {
        let mut target = item(Some("db-team"), BTreeMap::new(), 10_000, 0, 0, 48);
        target.select_range_check = 0;

        let report = evaluate_mysql_join_buffers_inventory(&[target], Pillar::Cost, now());

        assert_eq!(report.stale_resources, 1);
        let stale = report
            .findings
            .iter()
            .find(|finding| finding.reason_code == REASON_INV_STALE_DATA)
            .expect("stale finding");
        assert_eq!(stale.evidence["age_hours"], json!(48));
    }

    #[test]
    fn empty_join_evidence_is_reported() {
        let mut target = item(Some("db-team"), BTreeMap::new(), 0, 0, 0, 1);
        target.select_range_check = 0;

        let report = evaluate_mysql_join_buffers_inventory(&[target], Pillar::Cost, now());

        assert!(reason_codes(&report).contains(&REASON_COST_NO_JOIN_EVIDENCE));
    }

    #[test]
    fn healthy_join_buffers_pass_claimed_pillars() {
        let mut labels = BTreeMap::new();
        labels.insert("cost-center".to_string(), "cc-42".to_string());
        let mut target = item(None, labels, 10_000, 10, 10, 1);
        target.select_range_check = 0;

        for pillar in [Pillar::Cost, Pillar::Resilience, Pillar::Security] {
            let report =
                evaluate_mysql_join_buffers_inventory(std::slice::from_ref(&target), pillar, now());
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

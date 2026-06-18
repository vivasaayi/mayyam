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

// Deterministic sort-operations inventory evaluator for roadmap rows
// 03-MYSQL-AI-TRIAGER-01079/01086/01107.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::services::analytics::mysql_analytics::mysql_telemetry::MySqlTelemetrySnapshot;
use crate::services::aws::inventory::types::{
    score_pillar, InventoryFinding, Pillar, PillarReport, Severity, COST_ALLOCATION_TAG_KEYS,
    DEFAULT_STALE_AFTER_HOURS,
};

const SORT_MERGE_PASS_PCT_THRESHOLD: f64 = 10.0;
const SORT_MERGE_PASS_COUNT_THRESHOLD: i64 = 1_000;
const SORT_SCAN_COUNT_THRESHOLD: i64 = 10_000;

pub const RESOURCE_TYPE: &str = "MySqlSortOperations";
pub const REASON_COST_OWNER_NOT_RECORDED: &str = "MYSQL_SORT_OPS_COST_OWNER_NOT_RECORDED";
pub const REASON_COST_NO_SORT_EVIDENCE: &str = "MYSQL_SORT_OPS_COST_NO_SORT_EVIDENCE";
pub const REASON_COST_SORT_MERGE_PRESSURE: &str = "MYSQL_SORT_OPS_COST_MERGE_PRESSURE";
pub const REASON_RES_NO_SORT_EVIDENCE: &str = "MYSQL_SORT_OPS_RES_NO_SORT_EVIDENCE";
pub const REASON_RES_SORT_SPILL_RISK: &str = "MYSQL_SORT_OPS_RES_SORT_SPILL_RISK";
pub const REASON_SEC_OWNER_NOT_RECORDED: &str = "MYSQL_SORT_OPS_SEC_OWNER_NOT_RECORDED";
pub const REASON_SEC_UNROUTED_SORT_PRESSURE: &str = "MYSQL_SORT_OPS_SEC_UNROUTED_SORT_PRESSURE";
pub const REASON_INV_STALE_DATA: &str = "MYSQL_SORT_OPS_INV_STALE_DATA";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SortOperationsInventoryItem {
    pub connection_id: String,
    pub connection_name: String,
    pub owner: Option<String>,
    pub labels: BTreeMap<String, String>,
    pub questions: i64,
    pub queries: i64,
    pub slow_queries: i64,
    pub sort_merge_passes: i64,
    pub sort_range: i64,
    pub sort_rows: i64,
    pub sort_scan: i64,
    pub sort_merge_pass_pct: Option<f64>,
    pub qps_since_start: f64,
    pub collected_at: DateTime<Utc>,
}

pub fn evaluate_mysql_sort_operations_inventory(
    items: &[SortOperationsInventoryItem],
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

pub fn sort_operations_item_from_telemetry(
    connection_id: &str,
    connection_name: &str,
    owner: Option<String>,
    labels: BTreeMap<String, String>,
    snapshot: &MySqlTelemetrySnapshot,
) -> SortOperationsInventoryItem {
    SortOperationsInventoryItem {
        connection_id: connection_id.to_string(),
        connection_name: connection_name.to_string(),
        owner,
        labels,
        questions: snapshot.workload.questions,
        queries: snapshot.workload.queries,
        slow_queries: snapshot.workload.slow_queries,
        sort_merge_passes: snapshot.workload.sort_merge_passes,
        sort_range: snapshot.workload.sort_range,
        sort_rows: snapshot.workload.sort_rows,
        sort_scan: snapshot.workload.sort_scan,
        sort_merge_pass_pct: snapshot.workload.sort_merge_pass_pct,
        qps_since_start: snapshot.workload.qps_since_start,
        collected_at: snapshot.collected_at,
    }
}

fn evaluate_cost(
    item: &SortOperationsInventoryItem,
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
                "Sort-operations inventory for {} has no owner, team, project, or cost-center metadata",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "connection_name": item.connection_name,
                "checked_keys": COST_ALLOCATION_TAG_KEYS,
            }),
        ));
    }

    if !has_sort_evidence(item) {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_NO_SORT_EVIDENCE,
            Severity::High,
            format!(
                "Sort-operations inventory for {} has no global status evidence",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "questions": item.questions,
                "queries": item.queries,
                "recommendation": "Collect Sort_merge_passes, Sort_range, Sort_rows, and Sort_scan before estimating query-shape or memory savings",
            }),
        ));
    }

    if has_sort_pressure(item) {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_SORT_MERGE_PRESSURE,
            Severity::Medium,
            format!(
                "Sort-operations inventory for {} shows merge-pass cost pressure",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "sort_merge_passes": item.sort_merge_passes,
                "sort_range": item.sort_range,
                "sort_rows": item.sort_rows,
                "sort_scan": item.sort_scan,
                "sort_merge_pass_pct": item.sort_merge_pass_pct,
                "recommendation": "Review ORDER BY/GROUP BY query shapes, covering indexes, and sort_buffer_size before scaling CPU or memory for sort pressure",
            }),
        ));
    }
}

fn evaluate_resilience(
    item: &SortOperationsInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if !has_sort_evidence(item) {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_NO_SORT_EVIDENCE,
            Severity::High,
            format!(
                "Sort-operations inventory for {} has no resilience evidence",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "recommendation": "Collect sort counters so incidents can distinguish filesort or merge-pass pressure from storage, CPU, or lock saturation",
            }),
        ));
    }

    if has_sort_pressure(item) {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_SORT_SPILL_RISK,
            Severity::Medium,
            format!(
                "Sort-operations evidence for {} shows filesort spill risk",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "sort_merge_passes": item.sort_merge_passes,
                "sort_scan": item.sort_scan,
                "slow_queries": item.slow_queries,
                "sort_merge_pass_pct": item.sort_merge_pass_pct,
                "recommendation": "Correlate sort merge passes with top statement digests before changing memory limits or scheduling index rewrites",
            }),
        ));
    }
}

fn evaluate_security(
    item: &SortOperationsInventoryItem,
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
                "Sort-operations inventory for {} has no owner for query-change review routing",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "checked_keys": COST_ALLOCATION_TAG_KEYS,
            }),
        ));
    }

    if !has_owner_metadata(item) && has_sort_pressure(item) {
        findings.push(finding(
            item,
            pillar,
            REASON_SEC_UNROUTED_SORT_PRESSURE,
            Severity::Medium,
            format!(
                "Sort pressure for {} cannot be assigned to an accountable owner",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "sort_merge_passes": item.sort_merge_passes,
                "sort_merge_pass_pct": item.sort_merge_pass_pct,
                "recommendation": "Assign ownership before approving query rewrites or index changes that can affect authorization-sensitive workloads",
            }),
        ));
    }
}

fn stale_finding(
    item: &SortOperationsInventoryItem,
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
            "Inventory data for sort-operations resource {} is {} hours old (threshold {} hours)",
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
    item: &SortOperationsInventoryItem,
    pillar: Pillar,
    reason_code: &str,
    severity: Severity,
    message: String,
    evidence: Value,
) -> InventoryFinding {
    InventoryFinding {
        resource_id: item.connection_id.clone(),
        arn: format!("mysql://sort-operations/{}", item.connection_id),
        pillar,
        reason_code: reason_code.to_string(),
        severity,
        message,
        evidence,
    }
}

fn has_owner_metadata(item: &SortOperationsInventoryItem) -> bool {
    item.owner.as_deref().is_some_and(|owner| !owner.is_empty())
        || COST_ALLOCATION_TAG_KEYS
            .iter()
            .any(|key| item.labels.get(*key).is_some_and(|value| !value.is_empty()))
}

fn has_sort_evidence(item: &SortOperationsInventoryItem) -> bool {
    item.sort_merge_passes > 0 || item.sort_range > 0 || item.sort_rows > 0 || item.sort_scan > 0
}

fn has_sort_pressure(item: &SortOperationsInventoryItem) -> bool {
    item.sort_merge_pass_pct
        .is_some_and(|pct| pct >= SORT_MERGE_PASS_PCT_THRESHOLD)
        || item.sort_merge_passes >= SORT_MERGE_PASS_COUNT_THRESHOLD
        || item.sort_scan >= SORT_SCAN_COUNT_THRESHOLD
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
        sort_merge_passes: i64,
        sort_range: i64,
        sort_scan: i64,
        collected_hours_ago: i64,
    ) -> SortOperationsInventoryItem {
        SortOperationsInventoryItem {
            connection_id: "conn-1".to_string(),
            connection_name: "orders-db".to_string(),
            owner: owner.map(str::to_string),
            labels,
            questions: 10_000,
            queries: 10_000,
            slow_queries: 25,
            sort_merge_passes,
            sort_range,
            sort_rows: 500_000,
            sort_scan,
            sort_merge_pass_pct: if sort_range + sort_scan > 0 {
                Some(sort_merge_passes as f64 / (sort_range + sort_scan) as f64 * 100.0)
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
    fn cost_flags_missing_owner_and_sort_merge_pressure() {
        let target = item(None, BTreeMap::new(), 300, 1_000, 1_000, 1);

        let report = evaluate_mysql_sort_operations_inventory(&[target], Pillar::Cost, now());
        let codes = reason_codes(&report);

        assert!(codes.contains(&REASON_COST_OWNER_NOT_RECORDED));
        assert!(codes.contains(&REASON_COST_SORT_MERGE_PRESSURE));
    }

    #[test]
    fn resilience_flags_sort_spill_risk() {
        let target = item(Some("db-team"), BTreeMap::new(), 500, 2_000, 2_000, 1);

        let report = evaluate_mysql_sort_operations_inventory(&[target], Pillar::Resilience, now());

        assert!(reason_codes(&report).contains(&REASON_RES_SORT_SPILL_RISK));
    }

    #[test]
    fn security_routes_unowned_sort_pressure() {
        let target = item(None, BTreeMap::new(), 500, 2_000, 2_000, 1);

        let report = evaluate_mysql_sort_operations_inventory(&[target], Pillar::Security, now());
        let codes = reason_codes(&report);

        assert!(codes.contains(&REASON_SEC_OWNER_NOT_RECORDED));
        assert!(codes.contains(&REASON_SEC_UNROUTED_SORT_PRESSURE));
    }

    #[test]
    fn stale_inventory_is_counted_for_any_pillar() {
        let target = item(Some("db-team"), BTreeMap::new(), 0, 1_000, 0, 48);

        let report = evaluate_mysql_sort_operations_inventory(&[target], Pillar::Cost, now());

        assert_eq!(report.stale_resources, 1);
        let stale = report
            .findings
            .iter()
            .find(|finding| finding.reason_code == REASON_INV_STALE_DATA)
            .expect("stale finding");
        assert_eq!(stale.evidence["age_hours"], json!(48));
    }

    #[test]
    fn empty_sort_evidence_is_reported() {
        let mut target = item(Some("db-team"), BTreeMap::new(), 0, 0, 0, 1);
        target.sort_rows = 0;

        let report = evaluate_mysql_sort_operations_inventory(&[target], Pillar::Cost, now());

        assert!(reason_codes(&report).contains(&REASON_COST_NO_SORT_EVIDENCE));
    }

    #[test]
    fn healthy_sort_operations_pass_claimed_pillars() {
        let mut labels = BTreeMap::new();
        labels.insert("cost-center".to_string(), "cc-42".to_string());
        let target = item(None, labels, 10, 1_000, 1_000, 1);

        for pillar in [Pillar::Cost, Pillar::Resilience, Pillar::Security] {
            let report = evaluate_mysql_sort_operations_inventory(
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

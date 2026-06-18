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

// Deterministic query-plan inventory evaluator for roadmap rows
// 03-MYSQL-AI-TRIAGER-01177/01184/01205.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::services::analytics::mysql_analytics::mysql_telemetry::{
    MySqlStatementDigest, MySqlTelemetrySnapshot,
};
use crate::services::aws::inventory::types::{
    score_pillar, InventoryFinding, Pillar, PillarReport, Severity, COST_ALLOCATION_TAG_KEYS,
    DEFAULT_STALE_AFTER_HOURS,
};

const PLAN_CANDIDATE_ROWS_EXAMINED_THRESHOLD: i64 = 100_000;
const PLAN_CANDIDATE_ROWS_RATIO_THRESHOLD: f64 = 100.0;
const PLAN_CANDIDATE_MAX_TIME_MS_THRESHOLD: f64 = 5_000.0;

pub const RESOURCE_TYPE: &str = "MySqlQueryPlans";
pub const REASON_COST_OWNER_NOT_RECORDED: &str = "MYSQL_QUERY_PLANS_COST_OWNER_NOT_RECORDED";
pub const REASON_COST_NO_PLAN_COVERAGE: &str = "MYSQL_QUERY_PLANS_COST_NO_PLAN_COVERAGE";
pub const REASON_COST_PLAN_CANDIDATES: &str = "MYSQL_QUERY_PLANS_COST_PLAN_CANDIDATES";
pub const REASON_RES_NO_PLAN_COVERAGE: &str = "MYSQL_QUERY_PLANS_RES_NO_PLAN_COVERAGE";
pub const REASON_RES_PLAN_REGRESSION_RISK: &str = "MYSQL_QUERY_PLANS_RES_PLAN_REGRESSION_RISK";
pub const REASON_SEC_OWNER_NOT_RECORDED: &str = "MYSQL_QUERY_PLANS_SEC_OWNER_NOT_RECORDED";
pub const REASON_SEC_UNROUTED_PLAN_CHANGES: &str = "MYSQL_QUERY_PLANS_SEC_UNROUTED_PLAN_CHANGES";
pub const REASON_INV_STALE_DATA: &str = "MYSQL_QUERY_PLANS_INV_STALE_DATA";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryPlansInventoryItem {
    pub connection_id: String,
    pub connection_name: String,
    pub owner: Option<String>,
    pub labels: BTreeMap<String, String>,
    pub server_version: Option<String>,
    pub digest_count: usize,
    pub plan_candidate_count: usize,
    pub no_index_digest_count: usize,
    pub high_scan_digest_count: usize,
    pub latency_outlier_count: usize,
    pub sampled_plan_candidates: Vec<String>,
    pub collected_at: DateTime<Utc>,
}

pub fn evaluate_mysql_query_plans_inventory(
    items: &[QueryPlansInventoryItem],
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

pub fn query_plans_item_from_telemetry(
    connection_id: &str,
    connection_name: &str,
    owner: Option<String>,
    labels: BTreeMap<String, String>,
    snapshot: &MySqlTelemetrySnapshot,
) -> QueryPlansInventoryItem {
    let candidates = snapshot
        .statements
        .iter()
        .filter(|digest| is_plan_candidate(digest))
        .collect::<Vec<_>>();
    let no_index_digest_count = snapshot
        .statements
        .iter()
        .filter(|digest| digest.no_index_used_count > 0 || digest.no_good_index_used_count > 0)
        .count();
    let high_scan_digest_count = snapshot
        .statements
        .iter()
        .filter(|digest| is_high_scan_digest(digest))
        .count();
    let latency_outlier_count = snapshot
        .statements
        .iter()
        .filter(|digest| digest.max_time_ms >= PLAN_CANDIDATE_MAX_TIME_MS_THRESHOLD)
        .count();

    QueryPlansInventoryItem {
        connection_id: connection_id.to_string(),
        connection_name: connection_name.to_string(),
        owner,
        labels,
        server_version: snapshot.server.version.clone(),
        digest_count: snapshot.statements.len(),
        plan_candidate_count: candidates.len(),
        no_index_digest_count,
        high_scan_digest_count,
        latency_outlier_count,
        sampled_plan_candidates: candidates
            .iter()
            .take(5)
            .map(|digest| truncate(&digest.digest_text, 180))
            .collect(),
        collected_at: snapshot.collected_at,
    }
}

fn evaluate_cost(
    item: &QueryPlansInventoryItem,
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
                "Query-plan inventory for {} has no owner, team, project, or cost-center metadata",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "checked_keys": COST_ALLOCATION_TAG_KEYS,
            }),
        ));
    }

    if item.digest_count == 0 {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_NO_PLAN_COVERAGE,
            Severity::High,
            format!(
                "Query-plan inventory for {} has no statement digest coverage",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "recommendation": "Collect statement digest evidence before selecting SQL families for EXPLAIN capture or plan-cost review",
            }),
        ));
    }

    if item.plan_candidate_count > 0 {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_PLAN_CANDIDATES,
            Severity::Medium,
            format!(
                "Query-plan inventory for {} has candidate SQL families for plan review",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "plan_candidate_count": item.plan_candidate_count,
                "no_index_digest_count": item.no_index_digest_count,
                "high_scan_digest_count": item.high_scan_digest_count,
                "latency_outlier_count": item.latency_outlier_count,
                "sampled_plan_candidates": item.sampled_plan_candidates,
                "recommendation": "Capture EXPLAIN FORMAT=JSON for the sampled digests before scaling capacity or approving index work",
            }),
        ));
    }
}

fn evaluate_resilience(
    item: &QueryPlansInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if item.digest_count == 0 {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_NO_PLAN_COVERAGE,
            Severity::High,
            format!(
                "Query-plan inventory for {} has no plan-triage evidence",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "recommendation": "Collect digest evidence so incident response can pick deterministic EXPLAIN targets",
            }),
        ));
    }

    if item.high_scan_digest_count > 0 || item.latency_outlier_count > 0 {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_PLAN_REGRESSION_RISK,
            Severity::Medium,
            format!(
                "Query-plan inventory for {} shows plan regression risk",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "high_scan_digest_count": item.high_scan_digest_count,
                "latency_outlier_count": item.latency_outlier_count,
                "sampled_plan_candidates": item.sampled_plan_candidates,
                "recommendation": "Compare captured plans for high-scan or latency outlier digests before failover, memory, or index changes",
            }),
        ));
    }
}

fn evaluate_security(
    item: &QueryPlansInventoryItem,
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
                "Query-plan inventory for {} has no owner for plan-change review routing",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "checked_keys": COST_ALLOCATION_TAG_KEYS,
            }),
        ));
    }

    if !has_owner_metadata(item) && item.plan_candidate_count > 0 {
        findings.push(finding(
            item,
            pillar,
            REASON_SEC_UNROUTED_PLAN_CHANGES,
            Severity::Medium,
            format!(
                "Query-plan candidates for {} cannot be assigned to an accountable owner",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "plan_candidate_count": item.plan_candidate_count,
                "sampled_plan_candidates": item.sampled_plan_candidates,
                "recommendation": "Assign ownership before exporting query-plan evidence or approving plan-affecting index rewrites",
            }),
        ));
    }
}

fn stale_finding(
    item: &QueryPlansInventoryItem,
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
            "Inventory data for query-plan resource {} is {} hours old (threshold {} hours)",
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
    item: &QueryPlansInventoryItem,
    pillar: Pillar,
    reason_code: &str,
    severity: Severity,
    message: String,
    evidence: Value,
) -> InventoryFinding {
    InventoryFinding {
        resource_id: item.connection_id.clone(),
        arn: format!("mysql://query-plans/{}", item.connection_id),
        pillar,
        reason_code: reason_code.to_string(),
        severity,
        message,
        evidence,
    }
}

fn has_owner_metadata(item: &QueryPlansInventoryItem) -> bool {
    item.owner.as_deref().is_some_and(|owner| !owner.is_empty())
        || COST_ALLOCATION_TAG_KEYS
            .iter()
            .any(|key| item.labels.get(*key).is_some_and(|value| !value.is_empty()))
}

fn is_plan_candidate(digest: &MySqlStatementDigest) -> bool {
    digest.no_index_used_count > 0
        || digest.no_good_index_used_count > 0
        || is_high_scan_digest(digest)
        || digest.max_time_ms >= PLAN_CANDIDATE_MAX_TIME_MS_THRESHOLD
}

fn is_high_scan_digest(digest: &MySqlStatementDigest) -> bool {
    digest.rows_examined >= PLAN_CANDIDATE_ROWS_EXAMINED_THRESHOLD
        || digest
            .rows_examined_per_row_sent
            .is_some_and(|ratio| ratio >= PLAN_CANDIDATE_ROWS_RATIO_THRESHOLD)
}

fn truncate(value: &str, max_chars: usize) -> String {
    if value.chars().count() <= max_chars {
        return value.to_string();
    }
    value.chars().take(max_chars).collect::<String>()
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
        digest_count: usize,
        plan_candidate_count: usize,
        collected_hours_ago: i64,
    ) -> QueryPlansInventoryItem {
        QueryPlansInventoryItem {
            connection_id: "conn-1".to_string(),
            connection_name: "orders-db".to_string(),
            owner: owner.map(str::to_string),
            labels,
            server_version: Some("8.0.36".to_string()),
            digest_count,
            plan_candidate_count,
            no_index_digest_count: plan_candidate_count,
            high_scan_digest_count: plan_candidate_count,
            latency_outlier_count: plan_candidate_count,
            sampled_plan_candidates: vec!["SELECT * FROM orders WHERE customer_id = ?".to_string()],
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
    fn cost_flags_missing_owner_missing_coverage_and_plan_candidates() {
        let target = item(None, BTreeMap::new(), 3, 2, 1);

        let report = evaluate_mysql_query_plans_inventory(&[target], Pillar::Cost, now());
        let codes = reason_codes(&report);

        assert!(codes.contains(&REASON_COST_OWNER_NOT_RECORDED));
        assert!(codes.contains(&REASON_COST_PLAN_CANDIDATES));
    }

    #[test]
    fn cost_flags_missing_plan_coverage() {
        let target = item(Some("db-team"), BTreeMap::new(), 0, 0, 1);

        let report = evaluate_mysql_query_plans_inventory(&[target], Pillar::Cost, now());

        assert!(reason_codes(&report).contains(&REASON_COST_NO_PLAN_COVERAGE));
    }

    #[test]
    fn resilience_flags_plan_regression_risk() {
        let target = item(Some("db-team"), BTreeMap::new(), 3, 2, 1);

        let report = evaluate_mysql_query_plans_inventory(&[target], Pillar::Resilience, now());

        assert!(reason_codes(&report).contains(&REASON_RES_PLAN_REGRESSION_RISK));
    }

    #[test]
    fn security_routes_unowned_plan_candidates() {
        let target = item(None, BTreeMap::new(), 3, 2, 1);

        let report = evaluate_mysql_query_plans_inventory(&[target], Pillar::Security, now());
        let codes = reason_codes(&report);

        assert!(codes.contains(&REASON_SEC_OWNER_NOT_RECORDED));
        assert!(codes.contains(&REASON_SEC_UNROUTED_PLAN_CHANGES));
    }

    #[test]
    fn stale_inventory_is_counted_for_any_pillar() {
        let target = item(Some("db-team"), BTreeMap::new(), 3, 0, 48);

        let report = evaluate_mysql_query_plans_inventory(&[target], Pillar::Cost, now());

        assert_eq!(report.stale_resources, 1);
        let stale = report
            .findings
            .iter()
            .find(|finding| finding.reason_code == REASON_INV_STALE_DATA)
            .expect("stale finding");
        assert_eq!(stale.evidence["age_hours"], json!(48));
    }

    #[test]
    fn healthy_query_plans_pass_claimed_pillars() {
        let mut labels = BTreeMap::new();
        labels.insert("cost-center".to_string(), "cc-42".to_string());
        let target = item(None, labels, 3, 0, 1);

        for pillar in [Pillar::Cost, Pillar::Resilience, Pillar::Security] {
            let report =
                evaluate_mysql_query_plans_inventory(std::slice::from_ref(&target), pillar, now());
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

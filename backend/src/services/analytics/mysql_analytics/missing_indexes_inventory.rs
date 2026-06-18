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

// Deterministic missing-index inventory evaluator for roadmap rows
// 03-MYSQL-AI-TRIAGER-00883/00890/00911.

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

pub const RESOURCE_TYPE: &str = "MySqlMissingIndexes";
pub const REASON_COST_OWNER_NOT_RECORDED: &str = "MYSQL_MISSING_INDEXES_COST_OWNER_NOT_RECORDED";
pub const REASON_COST_NO_DIGEST_EVIDENCE: &str = "MYSQL_MISSING_INDEXES_COST_NO_DIGEST_EVIDENCE";
pub const REASON_COST_MISSING_INDEX_CANDIDATES: &str = "MYSQL_MISSING_INDEXES_COST_CANDIDATES";
pub const REASON_RES_NO_DIGEST_EVIDENCE: &str = "MYSQL_MISSING_INDEXES_RES_NO_DIGEST_EVIDENCE";
pub const REASON_RES_QUERY_PATH_RISK: &str = "MYSQL_MISSING_INDEXES_RES_QUERY_PATH_RISK";
pub const REASON_SEC_OWNER_NOT_RECORDED: &str = "MYSQL_MISSING_INDEXES_SEC_OWNER_NOT_RECORDED";
pub const REASON_SEC_UNROUTED_CANDIDATES: &str = "MYSQL_MISSING_INDEXES_SEC_UNROUTED_CANDIDATES";
pub const REASON_INV_STALE_DATA: &str = "MYSQL_MISSING_INDEXES_INV_STALE_DATA";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MissingIndexesInventoryItem {
    pub connection_id: String,
    pub connection_name: String,
    pub owner: Option<String>,
    pub labels: BTreeMap<String, String>,
    pub digest_count: usize,
    pub candidate_digest_count: usize,
    pub no_index_used_total: i64,
    pub no_good_index_used_total: i64,
    pub high_scan_digest_count: usize,
    pub max_rows_examined_per_row_sent: Option<f64>,
    pub sampled_candidate_digests: Vec<String>,
    pub collected_at: DateTime<Utc>,
}

pub fn evaluate_mysql_missing_indexes_inventory(
    items: &[MissingIndexesInventoryItem],
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

pub fn missing_indexes_item_from_telemetry(
    connection_id: &str,
    connection_name: &str,
    owner: Option<String>,
    labels: BTreeMap<String, String>,
    snapshot: &MySqlTelemetrySnapshot,
) -> MissingIndexesInventoryItem {
    let candidates = snapshot
        .statements
        .iter()
        .filter(|digest| is_missing_index_candidate(digest))
        .collect::<Vec<_>>();
    let max_rows_examined_per_row_sent = snapshot
        .statements
        .iter()
        .filter_map(|digest| digest.rows_examined_per_row_sent)
        .fold(None, |current, value| {
            Some(current.map_or(value, |existing: f64| existing.max(value)))
        });

    MissingIndexesInventoryItem {
        connection_id: connection_id.to_string(),
        connection_name: connection_name.to_string(),
        owner,
        labels,
        digest_count: snapshot.statements.len(),
        candidate_digest_count: candidates.len(),
        no_index_used_total: snapshot
            .statements
            .iter()
            .map(|digest| digest.no_index_used_count)
            .sum(),
        no_good_index_used_total: snapshot
            .statements
            .iter()
            .map(|digest| digest.no_good_index_used_count)
            .sum(),
        high_scan_digest_count: snapshot
            .statements
            .iter()
            .filter(|digest| digest.rows_examined_per_row_sent.unwrap_or(0.0) >= 100.0)
            .count(),
        max_rows_examined_per_row_sent,
        sampled_candidate_digests: candidates
            .iter()
            .take(10)
            .map(|digest| truncate(&digest.digest_text, 180))
            .collect(),
        collected_at: snapshot.collected_at,
    }
}

fn evaluate_cost(
    item: &MissingIndexesInventoryItem,
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
                "Missing-index inventory for {} has no owner, team, project, or cost-center metadata",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "connection_name": item.connection_name,
                "checked_keys": COST_ALLOCATION_TAG_KEYS,
            }),
        ));
    }

    if item.digest_count == 0 {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_NO_DIGEST_EVIDENCE,
            Severity::High,
            format!(
                "Missing-index inventory for {} has no statement digest evidence",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "digest_count": item.digest_count,
                "recommendation": "Collect performance_schema statement digests before estimating missing-index savings",
            }),
        ));
    }

    if item.candidate_digest_count > 0 {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_MISSING_INDEX_CANDIDATES,
            Severity::Medium,
            format!(
                "Missing-index inventory for {} has query families with no-index evidence",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "candidate_digest_count": item.candidate_digest_count,
                "no_index_used_total": item.no_index_used_total,
                "no_good_index_used_total": item.no_good_index_used_total,
                "max_rows_examined_per_row_sent": item.max_rows_examined_per_row_sent,
                "sampled_candidate_digests": item.sampled_candidate_digests,
                "recommendation": "Prioritize candidates by repeated no-index counters and rows examined before scaling database capacity",
            }),
        ));
    }
}

fn evaluate_resilience(
    item: &MissingIndexesInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if item.digest_count == 0 {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_NO_DIGEST_EVIDENCE,
            Severity::High,
            format!(
                "Missing-index inventory for {} has no query-family evidence for resilience triage",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "recommendation": "Collect digest evidence so incident response can separate query-plan regressions from database-wide saturation",
            }),
        ));
    }

    if item.high_scan_digest_count > 0 || item.candidate_digest_count > 0 {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_QUERY_PATH_RISK,
            Severity::Medium,
            format!(
                "Missing-index evidence for {} indicates query paths that may degrade under load",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "candidate_digest_count": item.candidate_digest_count,
                "high_scan_digest_count": item.high_scan_digest_count,
                "max_rows_examined_per_row_sent": item.max_rows_examined_per_row_sent,
                "recommendation": "Validate candidate indexes with EXPLAIN and production workload windows before relying on current query paths",
            }),
        ));
    }
}

fn evaluate_security(
    item: &MissingIndexesInventoryItem,
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
                "Missing-index inventory for {} has no owner for change review routing",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "checked_keys": COST_ALLOCATION_TAG_KEYS,
            }),
        ));
    }

    if !has_owner_metadata(item) && item.candidate_digest_count > 0 {
        findings.push(finding(
            item,
            pillar,
            REASON_SEC_UNROUTED_CANDIDATES,
            Severity::Medium,
            format!(
                "Missing-index candidates for {} cannot be assigned to an accountable owner",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "candidate_digest_count": item.candidate_digest_count,
                "sampled_candidate_digests": item.sampled_candidate_digests,
                "recommendation": "Assign ownership before approving DDL against query paths that may affect application authorization or reporting behavior",
            }),
        ));
    }
}

fn stale_finding(
    item: &MissingIndexesInventoryItem,
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
            "Inventory data for missing-index resource {} is {} hours old (threshold {} hours)",
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
    item: &MissingIndexesInventoryItem,
    pillar: Pillar,
    reason_code: &str,
    severity: Severity,
    message: String,
    evidence: Value,
) -> InventoryFinding {
    InventoryFinding {
        resource_id: item.connection_id.clone(),
        arn: format!("mysql://missing-indexes/{}", item.connection_id),
        pillar,
        reason_code: reason_code.to_string(),
        severity,
        message,
        evidence,
    }
}

fn has_owner_metadata(item: &MissingIndexesInventoryItem) -> bool {
    item.owner.as_deref().is_some_and(|owner| !owner.is_empty())
        || COST_ALLOCATION_TAG_KEYS
            .iter()
            .any(|key| item.labels.get(*key).is_some_and(|value| !value.is_empty()))
}

fn is_missing_index_candidate(digest: &MySqlStatementDigest) -> bool {
    digest.no_index_used_count > 0
        || digest.no_good_index_used_count > 0
        || digest.rows_examined_per_row_sent.unwrap_or(0.0) >= 100.0
}

fn truncate(value: &str, max_chars: usize) -> String {
    if value.chars().count() <= max_chars {
        return value.to_string();
    }
    let mut truncated = value.chars().take(max_chars).collect::<String>();
    truncated.push_str("...");
    truncated
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
        candidate_digest_count: usize,
        high_scan_digest_count: usize,
        collected_hours_ago: i64,
    ) -> MissingIndexesInventoryItem {
        MissingIndexesInventoryItem {
            connection_id: "conn-1".to_string(),
            connection_name: "orders-db".to_string(),
            owner: owner.map(str::to_string),
            labels,
            digest_count,
            candidate_digest_count,
            no_index_used_total: candidate_digest_count as i64,
            no_good_index_used_total: 0,
            high_scan_digest_count,
            max_rows_examined_per_row_sent: Some(250.0),
            sampled_candidate_digests: vec![
                "SELECT * FROM orders WHERE customer_id = ?".to_string()
            ],
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
    fn cost_flags_missing_owner_and_missing_index_candidates() {
        let target = item(None, BTreeMap::new(), 5, 2, 1, 1);

        let report = evaluate_mysql_missing_indexes_inventory(&[target], Pillar::Cost, now());
        let codes = reason_codes(&report);

        assert!(codes.contains(&REASON_COST_OWNER_NOT_RECORDED));
        assert!(codes.contains(&REASON_COST_MISSING_INDEX_CANDIDATES));
        let candidates = report
            .findings
            .iter()
            .find(|finding| finding.reason_code == REASON_COST_MISSING_INDEX_CANDIDATES)
            .expect("candidate finding");
        assert_eq!(candidates.evidence["candidate_digest_count"], json!(2));
    }

    #[test]
    fn resilience_flags_query_path_risk() {
        let target = item(Some("db-team"), BTreeMap::new(), 5, 1, 1, 1);

        let report = evaluate_mysql_missing_indexes_inventory(&[target], Pillar::Resilience, now());

        assert!(reason_codes(&report).contains(&REASON_RES_QUERY_PATH_RISK));
    }

    #[test]
    fn security_routes_unowned_candidates() {
        let target = item(None, BTreeMap::new(), 5, 1, 0, 1);

        let report = evaluate_mysql_missing_indexes_inventory(&[target], Pillar::Security, now());
        let codes = reason_codes(&report);

        assert!(codes.contains(&REASON_SEC_OWNER_NOT_RECORDED));
        assert!(codes.contains(&REASON_SEC_UNROUTED_CANDIDATES));
    }

    #[test]
    fn stale_inventory_is_counted_for_any_pillar() {
        let target = item(Some("db-team"), BTreeMap::new(), 5, 0, 0, 48);

        let report = evaluate_mysql_missing_indexes_inventory(&[target], Pillar::Cost, now());

        assert_eq!(report.stale_resources, 1);
        let stale = report
            .findings
            .iter()
            .find(|finding| finding.reason_code == REASON_INV_STALE_DATA)
            .expect("stale finding");
        assert_eq!(stale.evidence["age_hours"], json!(48));
    }

    #[test]
    fn healthy_missing_indexes_pass_claimed_pillars() {
        let mut labels = BTreeMap::new();
        labels.insert("cost-center".to_string(), "cc-42".to_string());
        let target = item(None, labels, 5, 0, 0, 1);

        for pillar in [Pillar::Cost, Pillar::Resilience, Pillar::Security] {
            let report = evaluate_mysql_missing_indexes_inventory(
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

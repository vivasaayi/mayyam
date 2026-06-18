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

// Deterministic unused-index inventory evaluator for roadmap rows
// 03-MYSQL-AI-TRIAGER-00834/00841/00862.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::services::analytics::mysql_analytics::mysql_telemetry::{
    MySqlIndexTelemetry, MySqlTelemetrySnapshot,
};
use crate::services::aws::inventory::types::{
    score_pillar, InventoryFinding, Pillar, PillarReport, Severity, COST_ALLOCATION_TAG_KEYS,
    DEFAULT_STALE_AFTER_HOURS,
};

pub const RESOURCE_TYPE: &str = "MySqlUnusedIndexes";
pub const REASON_COST_OWNER_NOT_RECORDED: &str = "MYSQL_UNUSED_INDEXES_COST_OWNER_NOT_RECORDED";
pub const REASON_COST_NO_INDEX_USAGE_METRICS: &str = "MYSQL_UNUSED_INDEXES_COST_NO_USAGE_METRICS";
pub const REASON_COST_UNUSED_WRITE_INDEXES: &str = "MYSQL_UNUSED_INDEXES_COST_WRITE_WASTE";
pub const REASON_RES_NO_INDEX_USAGE_METRICS: &str = "MYSQL_UNUSED_INDEXES_RES_NO_USAGE_METRICS";
pub const REASON_RES_UNUSED_INDEX_CHANGE_RISK: &str =
    "MYSQL_UNUSED_INDEXES_RES_CHANGE_RISK_UNVALIDATED";
pub const REASON_SEC_OWNER_NOT_RECORDED: &str = "MYSQL_UNUSED_INDEXES_SEC_OWNER_NOT_RECORDED";
pub const REASON_SEC_NO_INDEX_USAGE_METRICS: &str = "MYSQL_UNUSED_INDEXES_SEC_NO_USAGE_METRICS";
pub const REASON_SEC_UNOWNED_UNUSED_INDEXES: &str = "MYSQL_UNUSED_INDEXES_SEC_UNOWNED_FINDINGS";
pub const REASON_INV_STALE_DATA: &str = "MYSQL_UNUSED_INDEXES_INV_STALE_DATA";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnusedIndexesInventoryItem {
    pub connection_id: String,
    pub connection_name: String,
    pub owner: Option<String>,
    pub labels: BTreeMap<String, String>,
    pub index_metric_count: usize,
    pub index_count: usize,
    pub unused_secondary_index_count: usize,
    pub unused_write_heavy_index_count: usize,
    pub unused_unique_index_count: usize,
    pub sampled_unused_indexes: Vec<String>,
    pub collected_at: DateTime<Utc>,
}

pub fn evaluate_mysql_unused_indexes_inventory(
    items: &[UnusedIndexesInventoryItem],
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

pub fn unused_indexes_item_from_telemetry(
    connection_id: &str,
    connection_name: &str,
    owner: Option<String>,
    labels: BTreeMap<String, String>,
    snapshot: &MySqlTelemetrySnapshot,
) -> UnusedIndexesInventoryItem {
    let unused_indexes = snapshot
        .indexes
        .iter()
        .filter(|index| is_unused_secondary_index(index))
        .collect::<Vec<_>>();

    UnusedIndexesInventoryItem {
        connection_id: connection_id.to_string(),
        connection_name: connection_name.to_string(),
        owner,
        labels,
        index_metric_count: snapshot.indexes.len(),
        index_count: snapshot.indexes.len(),
        unused_secondary_index_count: unused_indexes.len(),
        unused_write_heavy_index_count: unused_indexes
            .iter()
            .filter(|index| index.write_count >= 100)
            .count(),
        unused_unique_index_count: unused_indexes
            .iter()
            .filter(|index| index.is_unique)
            .count(),
        sampled_unused_indexes: unused_indexes
            .iter()
            .take(10)
            .map(|index| {
                format!(
                    "{}.{}.{}",
                    index.schema_name, index.table_name, index.index_name
                )
            })
            .collect(),
        collected_at: snapshot.collected_at,
    }
}

fn evaluate_cost(
    item: &UnusedIndexesInventoryItem,
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
                "Unused-index inventory for {} has no owner, team, project, or cost-center metadata",
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

    if !has_index_usage_metrics(item) {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_NO_INDEX_USAGE_METRICS,
            Severity::High,
            format!(
                "Unused-index inventory for {} has no collected index usage evidence",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "connection_name": item.connection_name,
                "index_metric_count": item.index_metric_count,
                "recommendation": "Collect performance_schema index usage counters before estimating unused-index storage or write-amplification savings",
            }),
        ));
    }

    if item.unused_write_heavy_index_count > 0 {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_UNUSED_WRITE_INDEXES,
            Severity::Medium,
            format!(
                "Unused-index evidence for {} shows write-maintained indexes with no reads",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "connection_name": item.connection_name,
                "index_count": item.index_count,
                "unused_secondary_index_count": item.unused_secondary_index_count,
                "unused_write_heavy_index_count": item.unused_write_heavy_index_count,
                "sampled_unused_indexes": item.sampled_unused_indexes,
                "recommendation": "Review write-heavy unused secondary indexes for drop candidates after a workload-window validation and rollback plan",
            }),
        ));
    }
}

fn evaluate_resilience(
    item: &UnusedIndexesInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if !has_index_usage_metrics(item) {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_NO_INDEX_USAGE_METRICS,
            Severity::High,
            format!(
                "Unused-index inventory for {} has no resilience evidence",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "connection_name": item.connection_name,
                "index_metric_count": item.index_metric_count,
                "recommendation": "Collect index usage counters before changing indexes that may support rare resilience-critical query paths",
            }),
        ));
    }

    if item.unused_secondary_index_count > 0 {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_UNUSED_INDEX_CHANGE_RISK,
            Severity::Medium,
            format!(
                "Unused-index evidence for {} has drop candidates that need workload-window validation",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "connection_name": item.connection_name,
                "unused_secondary_index_count": item.unused_secondary_index_count,
                "unused_unique_index_count": item.unused_unique_index_count,
                "sampled_unused_indexes": item.sampled_unused_indexes,
                "recommendation": "Validate candidates against deployment windows, batch jobs, reports, and rollback requirements before executing index changes",
            }),
        ));
    }
}

fn evaluate_security(
    item: &UnusedIndexesInventoryItem,
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
                "Unused-index inventory for {} has no recorded owner for security review routing",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "connection_name": item.connection_name,
                "owner": item.owner,
                "checked_keys": COST_ALLOCATION_TAG_KEYS,
            }),
        ));
    }

    if !has_index_usage_metrics(item) {
        findings.push(finding(
            item,
            pillar,
            REASON_SEC_NO_INDEX_USAGE_METRICS,
            Severity::High,
            format!(
                "Unused-index inventory for {} has no index usage evidence for security review",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "connection_name": item.connection_name,
                "recommendation": "Collect index usage evidence before sharing drop recommendations or privileged DDL plans",
            }),
        ));
    }

    if !has_owner_metadata(item) && item.unused_secondary_index_count > 0 {
        findings.push(finding(
            item,
            pillar,
            REASON_SEC_UNOWNED_UNUSED_INDEXES,
            Severity::Medium,
            format!(
                "Unused-index findings for {} cannot be assigned to an accountable owner",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "connection_name": item.connection_name,
                "unused_secondary_index_count": item.unused_secondary_index_count,
                "sampled_unused_indexes": item.sampled_unused_indexes,
                "recommendation": "Assign ownership before approving DDL against indexes that may support application or reporting access patterns",
            }),
        ));
    }
}

fn stale_finding(
    item: &UnusedIndexesInventoryItem,
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
            "Inventory data for unused-index resource {} is {} hours old (threshold {} hours)",
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
    item: &UnusedIndexesInventoryItem,
    pillar: Pillar,
    reason_code: &str,
    severity: Severity,
    message: String,
    evidence: Value,
) -> InventoryFinding {
    InventoryFinding {
        resource_id: item.connection_id.clone(),
        arn: format!("mysql://unused-indexes/{}", item.connection_id),
        pillar,
        reason_code: reason_code.to_string(),
        severity,
        message,
        evidence,
    }
}

fn has_owner_metadata(item: &UnusedIndexesInventoryItem) -> bool {
    item.owner.as_deref().is_some_and(|owner| !owner.is_empty())
        || COST_ALLOCATION_TAG_KEYS
            .iter()
            .any(|key| item.labels.get(*key).is_some_and(|value| !value.is_empty()))
}

fn has_index_usage_metrics(item: &UnusedIndexesInventoryItem) -> bool {
    item.index_metric_count > 0
}

fn is_unused_secondary_index(index: &MySqlIndexTelemetry) -> bool {
    !index.is_primary && index.read_count == 0
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
        index_metric_count: usize,
        unused_secondary_index_count: usize,
        unused_write_heavy_index_count: usize,
        unused_unique_index_count: usize,
        collected_hours_ago: i64,
    ) -> UnusedIndexesInventoryItem {
        UnusedIndexesInventoryItem {
            connection_id: "conn-1".to_string(),
            connection_name: "orders-db".to_string(),
            owner: owner.map(str::to_string),
            labels,
            index_metric_count,
            index_count: index_metric_count,
            unused_secondary_index_count,
            unused_write_heavy_index_count,
            unused_unique_index_count,
            sampled_unused_indexes: vec!["app.orders.idx_orders_status".to_string()],
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
    fn cost_flags_missing_owner_missing_metrics_and_write_waste() {
        let target = item(None, BTreeMap::new(), 4, 2, 1, 0, 1);

        let report = evaluate_mysql_unused_indexes_inventory(&[target], Pillar::Cost, now());
        let codes = reason_codes(&report);

        assert!(codes.contains(&REASON_COST_OWNER_NOT_RECORDED));
        assert!(codes.contains(&REASON_COST_UNUSED_WRITE_INDEXES));
        assert!(!codes.contains(&REASON_COST_NO_INDEX_USAGE_METRICS));
        let waste = report
            .findings
            .iter()
            .find(|finding| finding.reason_code == REASON_COST_UNUSED_WRITE_INDEXES)
            .expect("unused write index finding");
        assert_eq!(waste.evidence["unused_write_heavy_index_count"], json!(1));
        assert!(report.score < 100);
    }

    #[test]
    fn resilience_flags_unvalidated_unused_index_change_risk() {
        let target = item(Some("db-team"), BTreeMap::new(), 3, 1, 0, 1, 1);

        let report = evaluate_mysql_unused_indexes_inventory(&[target], Pillar::Resilience, now());
        let change_risk = report
            .findings
            .iter()
            .find(|finding| finding.reason_code == REASON_RES_UNUSED_INDEX_CHANGE_RISK)
            .expect("change risk finding");

        assert_eq!(change_risk.severity, Severity::Medium);
        assert_eq!(change_risk.evidence["unused_unique_index_count"], json!(1));
    }

    #[test]
    fn security_routes_unowned_unused_index_findings() {
        let target = item(None, BTreeMap::new(), 2, 1, 1, 0, 1);

        let report = evaluate_mysql_unused_indexes_inventory(&[target], Pillar::Security, now());
        let codes = reason_codes(&report);

        assert!(codes.contains(&REASON_SEC_OWNER_NOT_RECORDED));
        assert!(codes.contains(&REASON_SEC_UNOWNED_UNUSED_INDEXES));
    }

    #[test]
    fn stale_inventory_is_counted_for_any_pillar() {
        let target = item(Some("db-team"), BTreeMap::new(), 2, 0, 0, 0, 48);

        let report = evaluate_mysql_unused_indexes_inventory(&[target], Pillar::Cost, now());

        assert_eq!(report.stale_resources, 1);
        let stale = report
            .findings
            .iter()
            .find(|finding| finding.reason_code == REASON_INV_STALE_DATA)
            .expect("stale finding");
        assert_eq!(stale.evidence["age_hours"], json!(48));
    }

    #[test]
    fn healthy_unused_indexes_pass_claimed_pillars() {
        let mut labels = BTreeMap::new();
        labels.insert("cost-center".to_string(), "cc-42".to_string());
        let target = item(None, labels, 3, 0, 0, 0, 1);

        for pillar in [Pillar::Cost, Pillar::Resilience, Pillar::Security] {
            let report = evaluate_mysql_unused_indexes_inventory(
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

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

// Deterministic index cardinality inventory evaluator for roadmap rows
// 03-MYSQL-AI-TRIAGER-00785/00792/00813.

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

pub const RESOURCE_TYPE: &str = "MySqlIndexCardinality";
pub const REASON_COST_OWNER_NOT_RECORDED: &str = "MYSQL_INDEX_CARDINALITY_COST_OWNER_NOT_RECORDED";
pub const REASON_COST_NO_INDEX_CARDINALITY_METRICS: &str =
    "MYSQL_INDEX_CARDINALITY_COST_NO_METRICS";
pub const REASON_COST_INDEX_WRITE_WASTE: &str = "MYSQL_INDEX_CARDINALITY_COST_INDEX_WRITE_WASTE";
pub const REASON_RES_NO_INDEX_CARDINALITY_METRICS: &str = "MYSQL_INDEX_CARDINALITY_RES_NO_METRICS";
pub const REASON_RES_LOW_SELECTIVITY_INDEXES: &str =
    "MYSQL_INDEX_CARDINALITY_RES_LOW_SELECTIVITY_INDEXES";
pub const REASON_RES_DUPLICATE_PREFIX_INDEXES: &str =
    "MYSQL_INDEX_CARDINALITY_RES_DUPLICATE_PREFIX_INDEXES";
pub const REASON_SEC_NO_INDEX_CARDINALITY_METRICS: &str = "MYSQL_INDEX_CARDINALITY_SEC_NO_METRICS";
pub const REASON_SEC_INDEX_SHAPE_REVIEW: &str = "MYSQL_INDEX_CARDINALITY_SEC_INDEX_SHAPE_REVIEW";
pub const REASON_INV_STALE_DATA: &str = "MYSQL_INDEX_CARDINALITY_INV_STALE_DATA";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexCardinalityInventoryItem {
    pub connection_id: String,
    pub connection_name: String,
    pub owner: Option<String>,
    pub labels: BTreeMap<String, String>,
    pub index_metric_count: usize,
    pub index_count: usize,
    pub table_count: usize,
    pub total_table_rows: i64,
    pub low_selectivity_index_count: usize,
    pub high_write_low_read_index_count: usize,
    pub duplicate_prefix_index_count: usize,
    pub collected_at: DateTime<Utc>,
}

pub fn evaluate_mysql_index_cardinality_inventory(
    items: &[IndexCardinalityInventoryItem],
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

pub fn index_cardinality_item_from_telemetry(
    connection_id: &str,
    connection_name: &str,
    owner: Option<String>,
    labels: BTreeMap<String, String>,
    snapshot: &MySqlTelemetrySnapshot,
) -> IndexCardinalityInventoryItem {
    let rows_by_table = snapshot
        .tables
        .iter()
        .map(|table| {
            (
                format!("{}.{}", table.schema_name, table.table_name),
                table.table_rows,
            )
        })
        .collect::<BTreeMap<_, _>>();

    IndexCardinalityInventoryItem {
        connection_id: connection_id.to_string(),
        connection_name: connection_name.to_string(),
        owner,
        labels,
        index_metric_count: snapshot.indexes.len() + snapshot.tables.len(),
        index_count: snapshot.indexes.len(),
        table_count: snapshot.tables.len(),
        total_table_rows: snapshot.tables.iter().map(|table| table.table_rows).sum(),
        low_selectivity_index_count: snapshot
            .indexes
            .iter()
            .filter(|index| is_low_selectivity_candidate(index, &rows_by_table))
            .count(),
        high_write_low_read_index_count: snapshot
            .indexes
            .iter()
            .filter(|index| index.write_count >= 100 && index.read_count == 0)
            .count(),
        duplicate_prefix_index_count: duplicate_prefix_index_count(&snapshot.indexes),
        collected_at: snapshot.collected_at,
    }
}

fn evaluate_cost(
    item: &IndexCardinalityInventoryItem,
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
                "Index cardinality inventory for {} has no owner, team, project, or cost-center metadata",
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

    if !has_index_cardinality_metrics(item) {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_NO_INDEX_CARDINALITY_METRICS,
            Severity::High,
            format!(
                "Index cardinality inventory for {} has no collected index evidence",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "connection_name": item.connection_name,
                "index_metric_count": item.index_metric_count,
                "recommendation": "Collect index definitions, table row counts, and index read/write counters before estimating index maintenance waste or storage spend",
            }),
        ));
    }

    if has_index_write_waste(item) {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_INDEX_WRITE_WASTE,
            Severity::Medium,
            format!(
                "Index cardinality evidence for {} shows write-heavy low-read index waste",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "connection_name": item.connection_name,
                "index_count": item.index_count,
                "table_count": item.table_count,
                "total_table_rows": item.total_table_rows,
                "high_write_low_read_index_count": item.high_write_low_read_index_count,
                "duplicate_prefix_index_count": item.duplicate_prefix_index_count,
                "recommendation": "Review low-read and prefix-overlapping indexes before adding capacity or accepting write amplification as unavoidable spend",
            }),
        ));
    }
}

fn evaluate_resilience(
    item: &IndexCardinalityInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if !has_index_cardinality_metrics(item) {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_NO_INDEX_CARDINALITY_METRICS,
            Severity::High,
            format!(
                "Index cardinality inventory for {} has no resilience evidence",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "connection_name": item.connection_name,
                "index_metric_count": item.index_metric_count,
                "recommendation": "Collect index shape and table cardinality evidence so query-plan regression risk can be evaluated deterministically",
            }),
        ));
    }

    if has_low_selectivity_indexes(item) {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_LOW_SELECTIVITY_INDEXES,
            Severity::Medium,
            format!(
                "Index cardinality evidence for {} has low-selectivity candidates",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "connection_name": item.connection_name,
                "low_selectivity_index_count": item.low_selectivity_index_count,
                "total_table_rows": item.total_table_rows,
                "recommendation": "Validate low-selectivity candidates with EXPLAIN and production read patterns before relying on them for resilience-critical query paths",
            }),
        ));
    }

    if has_duplicate_prefix_indexes(item) {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_DUPLICATE_PREFIX_INDEXES,
            Severity::Medium,
            format!(
                "Index cardinality evidence for {} has duplicate prefix indexes",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "connection_name": item.connection_name,
                "duplicate_prefix_index_count": item.duplicate_prefix_index_count,
                "recommendation": "Review prefix-overlapping indexes before schema changes because redundant index shapes can amplify write stalls and plan instability",
            }),
        ));
    }
}

fn evaluate_security(
    item: &IndexCardinalityInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if !has_index_cardinality_metrics(item) {
        findings.push(finding(
            item,
            pillar,
            REASON_SEC_NO_INDEX_CARDINALITY_METRICS,
            Severity::High,
            format!(
                "Index cardinality inventory for {} has no scoped security evidence",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "connection_name": item.connection_name,
                "index_metric_count": item.index_metric_count,
                "recommendation": "Collect index shape evidence so sensitive lookup paths and unexpected schema changes can be reviewed without privileged ad hoc diagnostics",
            }),
        ));
    }

    if needs_index_shape_review(item) {
        findings.push(finding(
            item,
            pillar,
            REASON_SEC_INDEX_SHAPE_REVIEW,
            Severity::Medium,
            format!(
                "Index cardinality shape for {} should be reviewed",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "connection_name": item.connection_name,
                "low_selectivity_index_count": item.low_selectivity_index_count,
                "duplicate_prefix_index_count": item.duplicate_prefix_index_count,
                "recommendation": "Review recent DDL, index ownership, and access-path intent before treating unusual index shape as expected application design",
            }),
        ));
    }
}

fn stale_finding(
    item: &IndexCardinalityInventoryItem,
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
            "Inventory data for index cardinality resource {} is {} hours old (threshold {} hours)",
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
    item: &IndexCardinalityInventoryItem,
    pillar: Pillar,
    reason_code: &str,
    severity: Severity,
    message: String,
    evidence: Value,
) -> InventoryFinding {
    InventoryFinding {
        resource_id: item.connection_id.clone(),
        arn: format!("mysql://index-cardinality/{}", item.connection_id),
        pillar,
        reason_code: reason_code.to_string(),
        severity,
        message,
        evidence,
    }
}

fn has_owner_metadata(item: &IndexCardinalityInventoryItem) -> bool {
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

fn has_index_cardinality_metrics(item: &IndexCardinalityInventoryItem) -> bool {
    item.index_metric_count > 0
}

fn has_index_write_waste(item: &IndexCardinalityInventoryItem) -> bool {
    has_index_cardinality_metrics(item)
        && (item.high_write_low_read_index_count > 0 || item.duplicate_prefix_index_count > 0)
}

fn has_low_selectivity_indexes(item: &IndexCardinalityInventoryItem) -> bool {
    has_index_cardinality_metrics(item) && item.low_selectivity_index_count > 0
}

fn has_duplicate_prefix_indexes(item: &IndexCardinalityInventoryItem) -> bool {
    has_index_cardinality_metrics(item) && item.duplicate_prefix_index_count > 0
}

fn needs_index_shape_review(item: &IndexCardinalityInventoryItem) -> bool {
    has_index_cardinality_metrics(item)
        && (item.low_selectivity_index_count > 0 || item.duplicate_prefix_index_count > 0)
}

fn is_low_selectivity_candidate(
    index: &MySqlIndexTelemetry,
    rows_by_table: &BTreeMap<String, i64>,
) -> bool {
    if index.is_unique || index.is_primary || index.columns.len() != 1 {
        return false;
    }

    let table_key = format!("{}.{}", index.schema_name, index.table_name);
    rows_by_table
        .get(&table_key)
        .map(|rows| *rows >= 10_000)
        .unwrap_or(false)
}

fn duplicate_prefix_index_count(indexes: &[MySqlIndexTelemetry]) -> usize {
    let mut by_table: BTreeMap<String, Vec<&MySqlIndexTelemetry>> = BTreeMap::new();
    for index in indexes
        .iter()
        .filter(|index| !index.is_primary && !index.columns.is_empty())
    {
        by_table
            .entry(format!("{}.{}", index.schema_name, index.table_name))
            .or_default()
            .push(index);
    }

    by_table
        .values()
        .map(|table_indexes| {
            table_indexes
                .iter()
                .enumerate()
                .filter(|(idx, left)| {
                    table_indexes.iter().enumerate().any(|(other_idx, right)| {
                        idx != &other_idx
                            && left.index_name != right.index_name
                            && left.columns.len() < right.columns.len()
                            && right.columns.starts_with(&left.columns)
                    })
                })
                .count()
        })
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{DateTime, Duration, Utc};
    use std::collections::BTreeMap;

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

    #[allow(clippy::too_many_arguments)]
    fn item(
        owner: Option<&str>,
        index_metric_count: usize,
        index_count: usize,
        table_count: usize,
        total_table_rows: i64,
        low_selectivity_index_count: usize,
        high_write_low_read_index_count: usize,
        duplicate_prefix_index_count: usize,
        labels: BTreeMap<String, String>,
        collected_at: DateTime<Utc>,
    ) -> IndexCardinalityInventoryItem {
        IndexCardinalityInventoryItem {
            connection_id: "mysql-1".to_string(),
            connection_name: "orders-mysql".to_string(),
            owner: owner.map(str::to_string),
            labels,
            index_metric_count,
            index_count,
            table_count,
            total_table_rows,
            low_selectivity_index_count,
            high_write_low_read_index_count,
            duplicate_prefix_index_count,
            collected_at,
        }
    }

    fn healthy_item() -> IndexCardinalityInventoryItem {
        item(
            Some("database-platform"),
            8,
            4,
            3,
            25_000,
            0,
            0,
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
    fn cost_flags_missing_owner_missing_metrics_and_write_waste() {
        let missing_metrics = item(Some(""), 0, 0, 0, 0, 0, 0, 0, BTreeMap::new(), now());
        let write_waste = item(
            Some("database-platform"),
            12,
            8,
            2,
            80_000,
            1,
            3,
            1,
            labels(&[("owner", "database-platform")]),
            now(),
        );

        let report = evaluate_mysql_index_cardinality_inventory(
            &[missing_metrics, write_waste],
            Pillar::Cost,
            now(),
        );
        let codes = reason_codes(&report);

        assert!(codes.contains(&REASON_COST_OWNER_NOT_RECORDED));
        assert!(codes.contains(&REASON_COST_NO_INDEX_CARDINALITY_METRICS));
        assert!(codes.contains(&REASON_COST_INDEX_WRITE_WASTE));
    }

    #[test]
    fn resilience_flags_missing_metrics_and_low_selectivity_risk() {
        let missing_metrics = item(
            Some("database-platform"),
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            labels(&[("owner", "database-platform")]),
            now(),
        );
        let low_selectivity = item(
            Some("database-platform"),
            10,
            7,
            2,
            120_000,
            2,
            1,
            1,
            labels(&[("owner", "database-platform")]),
            now(),
        );

        let report = evaluate_mysql_index_cardinality_inventory(
            &[missing_metrics, low_selectivity],
            Pillar::Resilience,
            now(),
        );
        let codes = reason_codes(&report);

        assert!(codes.contains(&REASON_RES_NO_INDEX_CARDINALITY_METRICS));
        assert!(codes.contains(&REASON_RES_LOW_SELECTIVITY_INDEXES));
        assert!(codes.contains(&REASON_RES_DUPLICATE_PREFIX_INDEXES));
    }

    #[test]
    fn security_flags_missing_metrics_and_unreviewed_index_shape() {
        let missing_metrics = item(
            Some("database-platform"),
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            labels(&[("owner", "database-platform")]),
            now(),
        );
        let unreviewed_shape = item(
            Some("database-platform"),
            10,
            7,
            2,
            120_000,
            2,
            0,
            1,
            labels(&[("owner", "database-platform")]),
            now(),
        );

        let report = evaluate_mysql_index_cardinality_inventory(
            &[missing_metrics, unreviewed_shape],
            Pillar::Security,
            now(),
        );
        let codes = reason_codes(&report);

        assert!(codes.contains(&REASON_SEC_NO_INDEX_CARDINALITY_METRICS));
        assert!(codes.contains(&REASON_SEC_INDEX_SHAPE_REVIEW));
    }

    #[test]
    fn stale_inventory_is_counted_for_any_pillar() {
        let stale = item(
            Some("database-platform"),
            8,
            4,
            3,
            25_000,
            0,
            0,
            0,
            labels(&[("owner", "database-platform")]),
            now() - Duration::hours(49),
        );

        let report =
            evaluate_mysql_index_cardinality_inventory(&[stale], Pillar::Resilience, now());

        assert_eq!(report.stale_resources, 1);
        assert!(reason_codes(&report).contains(&REASON_INV_STALE_DATA));
    }

    #[test]
    fn healthy_index_cardinality_passes_claimed_pillars() {
        for pillar in [Pillar::Cost, Pillar::Resilience, Pillar::Security] {
            let report =
                evaluate_mysql_index_cardinality_inventory(&[healthy_item()], pillar, now());

            assert_eq!(report.resources_evaluated, 1);
            assert_eq!(report.stale_resources, 0);
            assert!(report.findings.is_empty());
            assert!(report.score >= 99);
        }
    }
}

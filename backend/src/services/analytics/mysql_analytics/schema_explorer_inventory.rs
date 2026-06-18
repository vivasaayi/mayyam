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

// Deterministic schema-explorer inventory evaluator for roadmap rows
// 03-MYSQL-AI-TRIAGER-01226/01233/01254.

use std::collections::{BTreeMap, BTreeSet};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::services::analytics::mysql_analytics::mysql_telemetry::{
    MySqlIndexTelemetry, MySqlTableTelemetry, MySqlTelemetrySnapshot,
};
use crate::services::aws::inventory::types::{
    score_pillar, InventoryFinding, Pillar, PillarReport, Severity, COST_ALLOCATION_TAG_KEYS,
    DEFAULT_STALE_AFTER_HOURS,
};

const LARGE_TABLE_BYTES: i64 = 10 * 1024 * 1024 * 1024;
const HIGH_INDEX_COUNT_PER_TABLE: f64 = 5.0;

pub const RESOURCE_TYPE: &str = "MySqlSchemaExplorer";
pub const REASON_COST_OWNER_NOT_RECORDED: &str = "MYSQL_SCHEMA_EXPLORER_COST_OWNER_NOT_RECORDED";
pub const REASON_COST_NO_SCHEMA_EVIDENCE: &str = "MYSQL_SCHEMA_EXPLORER_COST_NO_SCHEMA_EVIDENCE";
pub const REASON_COST_LARGE_SCHEMA_SURFACE: &str =
    "MYSQL_SCHEMA_EXPLORER_COST_LARGE_SCHEMA_SURFACE";
pub const REASON_RES_NO_SCHEMA_EVIDENCE: &str = "MYSQL_SCHEMA_EXPLORER_RES_NO_SCHEMA_EVIDENCE";
pub const REASON_RES_KEY_COVERAGE_GAPS: &str = "MYSQL_SCHEMA_EXPLORER_RES_KEY_COVERAGE_GAPS";
pub const REASON_SEC_OWNER_NOT_RECORDED: &str = "MYSQL_SCHEMA_EXPLORER_SEC_OWNER_NOT_RECORDED";
pub const REASON_SEC_UNROUTED_SCHEMA_CHANGES: &str =
    "MYSQL_SCHEMA_EXPLORER_SEC_UNROUTED_SCHEMA_CHANGES";
pub const REASON_INV_STALE_DATA: &str = "MYSQL_SCHEMA_EXPLORER_INV_STALE_DATA";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchemaTableSample {
    pub schema_name: String,
    pub table_name: String,
    pub engine: Option<String>,
    pub table_rows: i64,
    pub total_bytes: i64,
    pub index_count: usize,
    pub has_primary_key: bool,
    pub read_count: i64,
    pub write_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchemaExplorerInventoryItem {
    pub connection_id: String,
    pub connection_name: String,
    pub owner: Option<String>,
    pub labels: BTreeMap<String, String>,
    pub server_version: Option<String>,
    pub schema_count: usize,
    pub table_count: usize,
    pub index_count: usize,
    pub partitioned_table_count: usize,
    pub table_without_primary_key_count: usize,
    pub large_table_count: usize,
    pub average_indexes_per_table: Option<f64>,
    pub sampled_largest_tables: Vec<SchemaTableSample>,
    pub sampled_tables_without_primary_key: Vec<SchemaTableSample>,
    pub collected_at: DateTime<Utc>,
}

pub fn evaluate_mysql_schema_explorer_inventory(
    items: &[SchemaExplorerInventoryItem],
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

pub fn schema_explorer_item_from_telemetry(
    connection_id: &str,
    connection_name: &str,
    owner: Option<String>,
    labels: BTreeMap<String, String>,
    snapshot: &MySqlTelemetrySnapshot,
) -> SchemaExplorerInventoryItem {
    let schemas = snapshot
        .tables
        .iter()
        .map(|table| table.schema_name.clone())
        .collect::<BTreeSet<_>>();
    let primary_key_tables = snapshot
        .indexes
        .iter()
        .filter(|index| index.is_primary)
        .map(table_key_for_index)
        .collect::<BTreeSet<_>>();
    let partitioned_tables = snapshot
        .partitions
        .iter()
        .map(|partition| format!("{}.{}", partition.schema_name, partition.table_name))
        .collect::<BTreeSet<_>>();
    let index_counts_by_table = index_counts_by_table(&snapshot.indexes);

    let mut table_samples = snapshot
        .tables
        .iter()
        .map(|table| sample_from_table(table, &index_counts_by_table, &primary_key_tables))
        .collect::<Vec<_>>();
    table_samples.sort_by(|left, right| {
        right
            .total_bytes
            .cmp(&left.total_bytes)
            .then_with(|| left.schema_name.cmp(&right.schema_name))
            .then_with(|| left.table_name.cmp(&right.table_name))
    });

    let table_without_primary_key_count = table_samples
        .iter()
        .filter(|sample| !sample.has_primary_key)
        .count();
    let sampled_tables_without_primary_key = table_samples
        .iter()
        .filter(|sample| !sample.has_primary_key)
        .take(10)
        .cloned()
        .collect::<Vec<_>>();
    let table_count = snapshot.tables.len();
    let index_count = snapshot.indexes.len();

    SchemaExplorerInventoryItem {
        connection_id: connection_id.to_string(),
        connection_name: connection_name.to_string(),
        owner,
        labels,
        server_version: snapshot.server.version.clone(),
        schema_count: schemas.len(),
        table_count,
        index_count,
        partitioned_table_count: partitioned_tables.len(),
        table_without_primary_key_count,
        large_table_count: table_samples
            .iter()
            .filter(|sample| sample.total_bytes >= LARGE_TABLE_BYTES)
            .count(),
        average_indexes_per_table: if table_count > 0 {
            Some(index_count as f64 / table_count as f64)
        } else {
            None
        },
        sampled_largest_tables: table_samples.into_iter().take(10).collect(),
        sampled_tables_without_primary_key,
        collected_at: snapshot.collected_at,
    }
}

fn evaluate_cost(
    item: &SchemaExplorerInventoryItem,
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
                "Schema-explorer inventory for {} has no owner, team, project, or cost-center metadata",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "checked_keys": COST_ALLOCATION_TAG_KEYS,
            }),
        ));
    }

    if item.table_count == 0 {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_NO_SCHEMA_EVIDENCE,
            Severity::High,
            format!(
                "Schema-explorer inventory for {} has no table or schema evidence",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "recommendation": "Collect information_schema table and index metadata before estimating schema-level cost posture",
            }),
        ));
    }

    if item.large_table_count > 0
        || item
            .average_indexes_per_table
            .is_some_and(|ratio| ratio >= HIGH_INDEX_COUNT_PER_TABLE)
    {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_LARGE_SCHEMA_SURFACE,
            Severity::Medium,
            format!(
                "Schema-explorer inventory for {} shows large schema cost drivers",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "schema_count": item.schema_count,
                "table_count": item.table_count,
                "index_count": item.index_count,
                "large_table_count": item.large_table_count,
                "average_indexes_per_table": item.average_indexes_per_table,
                "sampled_largest_tables": item.sampled_largest_tables,
                "recommendation": "Review largest tables and high-index schemas before approving storage, backup, or index expansion work",
            }),
        ));
    }
}

fn evaluate_resilience(
    item: &SchemaExplorerInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if item.table_count == 0 {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_NO_SCHEMA_EVIDENCE,
            Severity::High,
            format!(
                "Schema-explorer inventory for {} has no dependency evidence",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "recommendation": "Collect schema, table, index, and partition metadata so restore and migration plans have dependency evidence",
            }),
        ));
    }

    if item.table_without_primary_key_count > 0 {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_KEY_COVERAGE_GAPS,
            Severity::Medium,
            format!(
                "Schema-explorer inventory for {} has tables without primary-key evidence",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "table_without_primary_key_count": item.table_without_primary_key_count,
                "sampled_tables_without_primary_key": item.sampled_tables_without_primary_key,
                "recommendation": "Review key coverage before online schema change, replication, restore, or migration workflows",
            }),
        ));
    }
}

fn evaluate_security(
    item: &SchemaExplorerInventoryItem,
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
                "Schema-explorer inventory for {} has no owner for schema-change review routing",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "checked_keys": COST_ALLOCATION_TAG_KEYS,
            }),
        ));
    }

    if !has_owner_metadata(item) && item.table_count > 0 {
        findings.push(finding(
            item,
            pillar,
            REASON_SEC_UNROUTED_SCHEMA_CHANGES,
            Severity::Medium,
            format!(
                "Schema changes for {} cannot be assigned to an accountable owner",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "schema_count": item.schema_count,
                "table_count": item.table_count,
                "sampled_largest_tables": item.sampled_largest_tables,
                "recommendation": "Assign schema ownership before exporting dependency metadata or approving DDL changes",
            }),
        ));
    }
}

fn stale_finding(
    item: &SchemaExplorerInventoryItem,
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
            "Inventory data for schema-explorer resource {} is {} hours old (threshold {} hours)",
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
    item: &SchemaExplorerInventoryItem,
    pillar: Pillar,
    reason_code: &str,
    severity: Severity,
    message: String,
    evidence: Value,
) -> InventoryFinding {
    InventoryFinding {
        resource_id: item.connection_id.clone(),
        arn: format!("mysql://schema-explorer/{}", item.connection_id),
        pillar,
        reason_code: reason_code.to_string(),
        severity,
        message,
        evidence,
    }
}

fn has_owner_metadata(item: &SchemaExplorerInventoryItem) -> bool {
    item.owner.as_deref().is_some_and(|owner| !owner.is_empty())
        || COST_ALLOCATION_TAG_KEYS
            .iter()
            .any(|key| item.labels.get(*key).is_some_and(|value| !value.is_empty()))
}

fn index_counts_by_table(indexes: &[MySqlIndexTelemetry]) -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::new();
    for index in indexes {
        *counts.entry(table_key_for_index(index)).or_insert(0) += 1;
    }
    counts
}

fn sample_from_table(
    table: &MySqlTableTelemetry,
    index_counts: &BTreeMap<String, usize>,
    primary_key_tables: &BTreeSet<String>,
) -> SchemaTableSample {
    let table_key = table_key_for_table(table);
    SchemaTableSample {
        schema_name: table.schema_name.clone(),
        table_name: table.table_name.clone(),
        engine: table.engine.clone(),
        table_rows: table.table_rows,
        total_bytes: table_total_bytes(table),
        index_count: index_counts.get(&table_key).copied().unwrap_or(0),
        has_primary_key: primary_key_tables.contains(&table_key),
        read_count: table.read_count,
        write_count: table.write_count,
    }
}

fn table_key_for_table(table: &MySqlTableTelemetry) -> String {
    format!("{}.{}", table.schema_name, table.table_name)
}

fn table_key_for_index(index: &MySqlIndexTelemetry) -> String {
    format!("{}.{}", index.schema_name, index.table_name)
}

fn table_total_bytes(table: &MySqlTableTelemetry) -> i64 {
    table.data_length + table.index_length
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
        table_count: usize,
        large_table_count: usize,
        missing_primary_keys: usize,
        collected_hours_ago: i64,
    ) -> SchemaExplorerInventoryItem {
        SchemaExplorerInventoryItem {
            connection_id: "conn-1".to_string(),
            connection_name: "orders-db".to_string(),
            owner: owner.map(str::to_string),
            labels,
            server_version: Some("8.0.36".to_string()),
            schema_count: usize::from(table_count > 0),
            table_count,
            index_count: table_count,
            partitioned_table_count: 0,
            table_without_primary_key_count: missing_primary_keys,
            large_table_count,
            average_indexes_per_table: if table_count > 0 { Some(1.0) } else { None },
            sampled_largest_tables: vec![sample("orders", LARGE_TABLE_BYTES)],
            sampled_tables_without_primary_key: if missing_primary_keys > 0 {
                vec![sample("events", 1024)]
            } else {
                Vec::new()
            },
            collected_at: now() - Duration::hours(collected_hours_ago),
        }
    }

    fn sample(table_name: &str, total_bytes: i64) -> SchemaTableSample {
        SchemaTableSample {
            schema_name: "app".to_string(),
            table_name: table_name.to_string(),
            engine: Some("InnoDB".to_string()),
            table_rows: 10,
            total_bytes,
            index_count: 1,
            has_primary_key: table_name != "events",
            read_count: 3,
            write_count: 2,
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
    fn cost_flags_missing_owner_and_large_schema_surface() {
        let target = item(None, BTreeMap::new(), 3, 1, 0, 1);

        let report = evaluate_mysql_schema_explorer_inventory(&[target], Pillar::Cost, now());
        let codes = reason_codes(&report);

        assert!(codes.contains(&REASON_COST_OWNER_NOT_RECORDED));
        assert!(codes.contains(&REASON_COST_LARGE_SCHEMA_SURFACE));
    }

    #[test]
    fn cost_flags_missing_schema_evidence() {
        let target = item(Some("db-team"), BTreeMap::new(), 0, 0, 0, 1);

        let report = evaluate_mysql_schema_explorer_inventory(&[target], Pillar::Cost, now());

        assert!(reason_codes(&report).contains(&REASON_COST_NO_SCHEMA_EVIDENCE));
    }

    #[test]
    fn resilience_flags_primary_key_coverage_gaps() {
        let target = item(Some("db-team"), BTreeMap::new(), 3, 0, 2, 1);

        let report = evaluate_mysql_schema_explorer_inventory(&[target], Pillar::Resilience, now());

        assert!(reason_codes(&report).contains(&REASON_RES_KEY_COVERAGE_GAPS));
    }

    #[test]
    fn security_routes_unowned_schema_changes() {
        let target = item(None, BTreeMap::new(), 3, 0, 0, 1);

        let report = evaluate_mysql_schema_explorer_inventory(&[target], Pillar::Security, now());
        let codes = reason_codes(&report);

        assert!(codes.contains(&REASON_SEC_OWNER_NOT_RECORDED));
        assert!(codes.contains(&REASON_SEC_UNROUTED_SCHEMA_CHANGES));
    }

    #[test]
    fn stale_inventory_is_counted_for_any_pillar() {
        let target = item(Some("db-team"), BTreeMap::new(), 3, 0, 0, 48);

        let report = evaluate_mysql_schema_explorer_inventory(&[target], Pillar::Cost, now());

        assert_eq!(report.stale_resources, 1);
        let stale = report
            .findings
            .iter()
            .find(|finding| finding.reason_code == REASON_INV_STALE_DATA)
            .expect("stale finding");
        assert_eq!(stale.evidence["age_hours"], json!(48));
    }

    #[test]
    fn healthy_schema_explorer_passes_claimed_pillars() {
        let mut labels = BTreeMap::new();
        labels.insert("cost-center".to_string(), "cc-42".to_string());
        let target = item(None, labels, 3, 0, 0, 1);

        for pillar in [Pillar::Cost, Pillar::Resilience, Pillar::Security] {
            let report = evaluate_mysql_schema_explorer_inventory(
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

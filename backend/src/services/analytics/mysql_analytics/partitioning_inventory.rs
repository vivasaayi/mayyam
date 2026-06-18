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

// Deterministic partitioning inventory evaluator for roadmap rows
// 03-MYSQL-AI-TRIAGER-00981/00988/01009.

use std::collections::{BTreeMap, BTreeSet};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::services::analytics::mysql_analytics::mysql_telemetry::{
    MySqlPartitionTelemetry, MySqlTableTelemetry, MySqlTelemetrySnapshot,
};
use crate::services::aws::inventory::types::{
    score_pillar, InventoryFinding, Pillar, PillarReport, Severity, COST_ALLOCATION_TAG_KEYS,
    DEFAULT_STALE_AFTER_HOURS,
};

const LARGE_TABLE_BYTES: i64 = 10 * 1024 * 1024 * 1024;
const LARGE_PARTITION_BYTES: i64 = 5 * 1024 * 1024 * 1024;
const PARTITION_SKEW_RATIO: f64 = 3.0;

pub const RESOURCE_TYPE: &str = "MySqlPartitioning";
pub const REASON_COST_OWNER_NOT_RECORDED: &str = "MYSQL_PARTITIONING_COST_OWNER_NOT_RECORDED";
pub const REASON_COST_NO_TABLE_EVIDENCE: &str = "MYSQL_PARTITIONING_COST_NO_TABLE_EVIDENCE";
pub const REASON_COST_PARTITIONING_CANDIDATES: &str =
    "MYSQL_PARTITIONING_COST_PARTITIONING_CANDIDATES";
pub const REASON_RES_NO_TABLE_EVIDENCE: &str = "MYSQL_PARTITIONING_RES_NO_TABLE_EVIDENCE";
pub const REASON_RES_PARTITION_SHAPE_RISK: &str = "MYSQL_PARTITIONING_RES_SHAPE_RISK";
pub const REASON_SEC_OWNER_NOT_RECORDED: &str = "MYSQL_PARTITIONING_SEC_OWNER_NOT_RECORDED";
pub const REASON_SEC_UNROUTED_DDL: &str = "MYSQL_PARTITIONING_SEC_UNROUTED_DDL";
pub const REASON_INV_STALE_DATA: &str = "MYSQL_PARTITIONING_INV_STALE_DATA";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PartitionedTableSample {
    pub schema_name: String,
    pub table_name: String,
    pub partition_count: usize,
    pub partition_method: Option<String>,
    pub partition_expression: Option<String>,
    pub total_bytes: i64,
    pub largest_partition_bytes: i64,
    pub data_free_bytes: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LargeUnpartitionedTableSample {
    pub schema_name: String,
    pub table_name: String,
    pub engine: Option<String>,
    pub table_rows: i64,
    pub total_bytes: i64,
    pub read_count: i64,
    pub write_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PartitioningInventoryItem {
    pub connection_id: String,
    pub connection_name: String,
    pub owner: Option<String>,
    pub labels: BTreeMap<String, String>,
    pub table_count: usize,
    pub partitioned_table_count: usize,
    pub partition_count: usize,
    pub unpartitioned_large_table_count: usize,
    pub oversized_partition_count: usize,
    pub skewed_partitioned_table_count: usize,
    pub partition_data_free_bytes: i64,
    pub largest_partition_bytes: i64,
    pub sampled_partitioned_tables: Vec<PartitionedTableSample>,
    pub sampled_large_unpartitioned_tables: Vec<LargeUnpartitionedTableSample>,
    pub collected_at: DateTime<Utc>,
}

pub fn evaluate_mysql_partitioning_inventory(
    items: &[PartitioningInventoryItem],
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

pub fn partitioning_item_from_telemetry(
    connection_id: &str,
    connection_name: &str,
    owner: Option<String>,
    labels: BTreeMap<String, String>,
    snapshot: &MySqlTelemetrySnapshot,
) -> PartitioningInventoryItem {
    let partitioned_table_keys = snapshot
        .partitions
        .iter()
        .map(table_key_for_partition)
        .collect::<BTreeSet<_>>();
    let mut partitioned_samples = partitioned_table_samples(&snapshot.partitions);
    partitioned_samples.sort_by(|left, right| {
        right
            .total_bytes
            .cmp(&left.total_bytes)
            .then_with(|| left.table_name.cmp(&right.table_name))
    });

    let mut large_unpartitioned = snapshot
        .tables
        .iter()
        .filter(|table| !partitioned_table_keys.contains(&table_key_for_table(table)))
        .filter(|table| table_total_bytes(table) >= LARGE_TABLE_BYTES)
        .map(large_unpartitioned_sample)
        .collect::<Vec<_>>();
    large_unpartitioned.sort_by(|left, right| {
        right
            .total_bytes
            .cmp(&left.total_bytes)
            .then_with(|| left.table_name.cmp(&right.table_name))
    });

    let oversized_partition_count = snapshot
        .partitions
        .iter()
        .filter(|partition| partition_total_bytes(partition) >= LARGE_PARTITION_BYTES)
        .count();
    let largest_partition_bytes = snapshot
        .partitions
        .iter()
        .map(partition_total_bytes)
        .max()
        .unwrap_or(0);

    PartitioningInventoryItem {
        connection_id: connection_id.to_string(),
        connection_name: connection_name.to_string(),
        owner,
        labels,
        table_count: snapshot.tables.len(),
        partitioned_table_count: partitioned_table_keys.len(),
        partition_count: snapshot.partitions.len(),
        unpartitioned_large_table_count: large_unpartitioned.len(),
        oversized_partition_count,
        skewed_partitioned_table_count: skewed_partitioned_table_count(&snapshot.partitions),
        partition_data_free_bytes: snapshot
            .partitions
            .iter()
            .map(|partition| partition.data_free)
            .sum(),
        largest_partition_bytes,
        sampled_partitioned_tables: partitioned_samples.into_iter().take(10).collect(),
        sampled_large_unpartitioned_tables: large_unpartitioned.into_iter().take(10).collect(),
        collected_at: snapshot.collected_at,
    }
}

fn evaluate_cost(
    item: &PartitioningInventoryItem,
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
                "Partitioning inventory for {} has no owner, team, project, or cost-center metadata",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "connection_name": item.connection_name,
                "checked_keys": COST_ALLOCATION_TAG_KEYS,
            }),
        ));
    }

    if item.table_count == 0 {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_NO_TABLE_EVIDENCE,
            Severity::High,
            format!(
                "Partitioning inventory for {} has no table evidence",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "table_count": item.table_count,
                "recommendation": "Collect table and partition metadata before estimating partitioning-related storage or maintenance savings",
            }),
        ));
    }

    if item.unpartitioned_large_table_count > 0 || item.partition_data_free_bytes > 0 {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_PARTITIONING_CANDIDATES,
            Severity::Medium,
            format!(
                "Partitioning inventory for {} has storage optimization candidates",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "unpartitioned_large_table_count": item.unpartitioned_large_table_count,
                "partition_data_free_bytes": item.partition_data_free_bytes,
                "sampled_large_unpartitioned_tables": item.sampled_large_unpartitioned_tables,
                "sampled_partitioned_tables": item.sampled_partitioned_tables,
                "recommendation": "Review large unpartitioned tables and partition reclaimable bytes before adding storage or accepting long maintenance windows",
            }),
        ));
    }
}

fn evaluate_resilience(
    item: &PartitioningInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if item.table_count == 0 {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_NO_TABLE_EVIDENCE,
            Severity::High,
            format!(
                "Partitioning inventory for {} has no resilience evidence",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "recommendation": "Collect table and partition metadata so incident responders can assess large-table maintenance and pruning risk",
            }),
        ));
    }

    if item.unpartitioned_large_table_count > 0
        || item.oversized_partition_count > 0
        || item.skewed_partitioned_table_count > 0
    {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_PARTITION_SHAPE_RISK,
            Severity::Medium,
            format!(
                "Partitioning evidence for {} shows large-table or partition-shape risk",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "unpartitioned_large_table_count": item.unpartitioned_large_table_count,
                "oversized_partition_count": item.oversized_partition_count,
                "skewed_partitioned_table_count": item.skewed_partitioned_table_count,
                "largest_partition_bytes": item.largest_partition_bytes,
                "sampled_large_unpartitioned_tables": item.sampled_large_unpartitioned_tables,
                "sampled_partitioned_tables": item.sampled_partitioned_tables,
                "recommendation": "Validate partition pruning, archival windows, and rollback notes before relying on current table shape during incidents",
            }),
        ));
    }
}

fn evaluate_security(
    item: &PartitioningInventoryItem,
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
                "Partitioning inventory for {} has no owner for DDL review routing",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "checked_keys": COST_ALLOCATION_TAG_KEYS,
            }),
        ));
    }

    if !has_owner_metadata(item)
        && (item.unpartitioned_large_table_count > 0 || item.oversized_partition_count > 0)
    {
        findings.push(finding(
            item,
            pillar,
            REASON_SEC_UNROUTED_DDL,
            Severity::Medium,
            format!(
                "Partitioning DDL candidates for {} cannot be assigned to an accountable owner",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "unpartitioned_large_table_count": item.unpartitioned_large_table_count,
                "oversized_partition_count": item.oversized_partition_count,
                "recommendation": "Assign ownership before approving partitioning DDL that may rebuild large tables or alter retention behavior",
            }),
        ));
    }
}

fn stale_finding(
    item: &PartitioningInventoryItem,
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
            "Inventory data for partitioning resource {} is {} hours old (threshold {} hours)",
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
    item: &PartitioningInventoryItem,
    pillar: Pillar,
    reason_code: &str,
    severity: Severity,
    message: String,
    evidence: Value,
) -> InventoryFinding {
    InventoryFinding {
        resource_id: item.connection_id.clone(),
        arn: format!("mysql://partitioning/{}", item.connection_id),
        pillar,
        reason_code: reason_code.to_string(),
        severity,
        message,
        evidence,
    }
}

fn has_owner_metadata(item: &PartitioningInventoryItem) -> bool {
    item.owner.as_deref().is_some_and(|owner| !owner.is_empty())
        || COST_ALLOCATION_TAG_KEYS
            .iter()
            .any(|key| item.labels.get(*key).is_some_and(|value| !value.is_empty()))
}

fn partitioned_table_samples(
    partitions: &[MySqlPartitionTelemetry],
) -> Vec<PartitionedTableSample> {
    let mut grouped: BTreeMap<String, Vec<&MySqlPartitionTelemetry>> = BTreeMap::new();
    for partition in partitions {
        grouped
            .entry(table_key_for_partition(partition))
            .or_default()
            .push(partition);
    }

    grouped
        .into_values()
        .filter_map(|group| {
            let first = group.first()?;
            Some(PartitionedTableSample {
                schema_name: first.schema_name.clone(),
                table_name: first.table_name.clone(),
                partition_count: group.len(),
                partition_method: first.partition_method.clone(),
                partition_expression: first.partition_expression.clone(),
                total_bytes: group
                    .iter()
                    .map(|partition| partition_total_bytes(partition))
                    .sum(),
                largest_partition_bytes: group
                    .iter()
                    .map(|partition| partition_total_bytes(partition))
                    .max()
                    .unwrap_or(0),
                data_free_bytes: group.iter().map(|partition| partition.data_free).sum(),
            })
        })
        .collect()
}

fn large_unpartitioned_sample(table: &MySqlTableTelemetry) -> LargeUnpartitionedTableSample {
    LargeUnpartitionedTableSample {
        schema_name: table.schema_name.clone(),
        table_name: table.table_name.clone(),
        engine: table.engine.clone(),
        table_rows: table.table_rows,
        total_bytes: table_total_bytes(table),
        read_count: table.read_count,
        write_count: table.write_count,
    }
}

fn skewed_partitioned_table_count(partitions: &[MySqlPartitionTelemetry]) -> usize {
    let mut grouped: BTreeMap<String, Vec<&MySqlPartitionTelemetry>> = BTreeMap::new();
    for partition in partitions {
        grouped
            .entry(table_key_for_partition(partition))
            .or_default()
            .push(partition);
    }

    grouped
        .into_values()
        .filter(|group| group.len() >= 4)
        .filter(|group| {
            let total: i64 = group
                .iter()
                .map(|partition| partition.table_rows.max(0))
                .sum();
            if total <= 0 {
                return false;
            }
            let average = total as f64 / group.len() as f64;
            let max_rows = group
                .iter()
                .map(|partition| partition.table_rows.max(0))
                .max()
                .unwrap_or(0) as f64;
            max_rows >= average * PARTITION_SKEW_RATIO
        })
        .count()
}

fn table_key_for_table(table: &MySqlTableTelemetry) -> String {
    format!("{}.{}", table.schema_name, table.table_name)
}

fn table_key_for_partition(partition: &MySqlPartitionTelemetry) -> String {
    format!("{}.{}", partition.schema_name, partition.table_name)
}

fn table_total_bytes(table: &MySqlTableTelemetry) -> i64 {
    table.data_length.saturating_add(table.index_length)
}

fn partition_total_bytes(partition: &MySqlPartitionTelemetry) -> i64 {
    partition.data_length.saturating_add(partition.index_length)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::analytics::mysql_analytics::mysql_telemetry::{
        MySqlConnectionSnapshot, MySqlInnoDbSnapshot, MySqlLockSnapshot, MySqlServerContext,
        MySqlTelemetrySnapshot, MySqlWorkloadSnapshot,
    };
    use chrono::Duration;
    use std::collections::HashMap;

    fn now() -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2026-06-10T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc)
    }

    fn item(
        owner: Option<&str>,
        labels: BTreeMap<String, String>,
        table_count: usize,
        unpartitioned_large_table_count: usize,
        oversized_partition_count: usize,
        collected_hours_ago: i64,
    ) -> PartitioningInventoryItem {
        PartitioningInventoryItem {
            connection_id: "conn-1".to_string(),
            connection_name: "orders-db".to_string(),
            owner: owner.map(str::to_string),
            labels,
            table_count,
            partitioned_table_count: 1,
            partition_count: 4,
            unpartitioned_large_table_count,
            oversized_partition_count,
            skewed_partitioned_table_count: oversized_partition_count,
            partition_data_free_bytes: 128 * 1024 * 1024,
            largest_partition_bytes: LARGE_PARTITION_BYTES,
            sampled_partitioned_tables: vec![PartitionedTableSample {
                schema_name: "app".to_string(),
                table_name: "events".to_string(),
                partition_count: 4,
                partition_method: Some("RANGE".to_string()),
                partition_expression: Some("created_at".to_string()),
                total_bytes: 12 * 1024 * 1024 * 1024,
                largest_partition_bytes: LARGE_PARTITION_BYTES,
                data_free_bytes: 128 * 1024 * 1024,
            }],
            sampled_large_unpartitioned_tables: vec![LargeUnpartitionedTableSample {
                schema_name: "app".to_string(),
                table_name: "orders".to_string(),
                engine: Some("InnoDB".to_string()),
                table_rows: 10_000_000,
                total_bytes: LARGE_TABLE_BYTES,
                read_count: 500,
                write_count: 100,
            }],
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
    fn cost_flags_missing_owner_and_partitioning_candidates() {
        let target = item(None, BTreeMap::new(), 4, 1, 0, 1);

        let report = evaluate_mysql_partitioning_inventory(&[target], Pillar::Cost, now());
        let codes = reason_codes(&report);

        assert!(codes.contains(&REASON_COST_OWNER_NOT_RECORDED));
        assert!(codes.contains(&REASON_COST_PARTITIONING_CANDIDATES));
    }

    #[test]
    fn resilience_flags_partition_shape_risk() {
        let target = item(Some("db-team"), BTreeMap::new(), 4, 0, 1, 1);

        let report = evaluate_mysql_partitioning_inventory(&[target], Pillar::Resilience, now());

        assert!(reason_codes(&report).contains(&REASON_RES_PARTITION_SHAPE_RISK));
    }

    #[test]
    fn security_routes_unowned_partitioning_ddl() {
        let target = item(None, BTreeMap::new(), 4, 1, 0, 1);

        let report = evaluate_mysql_partitioning_inventory(&[target], Pillar::Security, now());
        let codes = reason_codes(&report);

        assert!(codes.contains(&REASON_SEC_OWNER_NOT_RECORDED));
        assert!(codes.contains(&REASON_SEC_UNROUTED_DDL));
    }

    #[test]
    fn stale_inventory_is_counted_for_any_pillar() {
        let target = item(Some("db-team"), BTreeMap::new(), 4, 0, 0, 48);

        let report = evaluate_mysql_partitioning_inventory(&[target], Pillar::Cost, now());

        assert_eq!(report.stale_resources, 1);
        let stale = report
            .findings
            .iter()
            .find(|finding| finding.reason_code == REASON_INV_STALE_DATA)
            .expect("stale finding");
        assert_eq!(stale.evidence["age_hours"], json!(48));
    }

    #[test]
    fn telemetry_mapping_counts_partition_candidates() {
        let snapshot = snapshot(
            vec![
                table("orders", LARGE_TABLE_BYTES, 0),
                table("events", 12 * 1024 * 1024 * 1024, 0),
            ],
            vec![
                partition("events", "p0", 100, 1024 * 1024 * 1024, 0),
                partition("events", "p1", 100, 1024 * 1024 * 1024, 0),
                partition("events", "p2", 100, 1024 * 1024 * 1024, 0),
                partition("events", "p3", 1_000, LARGE_PARTITION_BYTES, 0),
            ],
        );

        let item = partitioning_item_from_telemetry(
            "conn-1",
            "orders-db",
            Some("db-team".to_string()),
            BTreeMap::new(),
            &snapshot,
        );

        assert_eq!(item.table_count, 2);
        assert_eq!(item.partitioned_table_count, 1);
        assert_eq!(item.partition_count, 4);
        assert_eq!(item.unpartitioned_large_table_count, 1);
        assert_eq!(item.oversized_partition_count, 1);
        assert_eq!(item.skewed_partitioned_table_count, 1);
    }

    #[test]
    fn healthy_partitioning_passes_claimed_pillars() {
        let mut labels = BTreeMap::new();
        labels.insert("cost-center".to_string(), "cc-42".to_string());
        let target = PartitioningInventoryItem {
            unpartitioned_large_table_count: 0,
            oversized_partition_count: 0,
            skewed_partitioned_table_count: 0,
            partition_data_free_bytes: 0,
            largest_partition_bytes: 0,
            sampled_partitioned_tables: Vec::new(),
            sampled_large_unpartitioned_tables: Vec::new(),
            ..item(None, labels, 4, 0, 0, 1)
        };

        for pillar in [Pillar::Cost, Pillar::Resilience, Pillar::Security] {
            let report =
                evaluate_mysql_partitioning_inventory(std::slice::from_ref(&target), pillar, now());
            assert!(
                report.findings.is_empty(),
                "unexpected findings for {:?}: {:?}",
                pillar,
                report.findings
            );
            assert_eq!(report.score, 100);
        }
    }

    fn table(table_name: &str, data_length: i64, index_length: i64) -> MySqlTableTelemetry {
        MySqlTableTelemetry {
            schema_name: "app".to_string(),
            table_name: table_name.to_string(),
            engine: Some("InnoDB".to_string()),
            table_rows: 1_000,
            data_length,
            index_length,
            data_free: 0,
            read_count: 10,
            write_count: 5,
        }
    }

    fn partition(
        table_name: &str,
        partition_name: &str,
        table_rows: i64,
        data_length: i64,
        index_length: i64,
    ) -> MySqlPartitionTelemetry {
        MySqlPartitionTelemetry {
            schema_name: "app".to_string(),
            table_name: table_name.to_string(),
            partition_name: partition_name.to_string(),
            partition_method: Some("RANGE".to_string()),
            partition_expression: Some("created_at".to_string()),
            partition_description: Some("2026".to_string()),
            table_rows,
            data_length,
            index_length,
            data_free: 0,
        }
    }

    fn snapshot(
        tables: Vec<MySqlTableTelemetry>,
        partitions: Vec<MySqlPartitionTelemetry>,
    ) -> MySqlTelemetrySnapshot {
        MySqlTelemetrySnapshot {
            collected_at: now(),
            server: MySqlServerContext {
                version: Some("8.0".to_string()),
                uptime_seconds: 3600,
                have_ssl: Some("YES".to_string()),
                require_secure_transport: Some("ON".to_string()),
                log_bin: Some("ON".to_string()),
                binlog_expire_logs_seconds: Some(604800),
                expire_logs_days: None,
                gtid_mode: Some("ON".to_string()),
                performance_schema_enabled: Some("ON".to_string()),
                slow_query_log_enabled: Some("ON".to_string()),
                long_query_time_seconds: Some(1.0),
                sys_schema_available: true,
            },
            workload: MySqlWorkloadSnapshot {
                questions: 0,
                queries: 0,
                com_select: 0,
                com_insert: 0,
                com_update: 0,
                com_delete: 0,
                slow_queries: 0,
                created_tmp_tables: 0,
                created_tmp_disk_tables: 0,
                created_tmp_files: 0,
                tmp_disk_table_pct: None,
                sort_merge_passes: 0,
                sort_range: 0,
                sort_rows: 0,
                sort_scan: 0,
                sort_merge_pass_pct: None,
                select_full_join: 0,
                select_full_range_join: 0,
                select_range_check: 0,
                full_join_select_pct: None,
                ssl_accepts: 0,
                ssl_finished_accepts: 0,
                ssl_accept_pct: None,
                qps_since_start: 0.0,
                read_write_ratio: None,
            },
            connections: MySqlConnectionSnapshot {
                max_connections: 0,
                max_used_connections: 0,
                threads_connected: 0,
                threads_running: 0,
                threads_cached: 0,
                connection_usage_pct: None,
                peak_connection_usage_pct: None,
                aborted_clients: 0,
                aborted_connects: 0,
                connection_errors: HashMap::new(),
            },
            innodb: MySqlInnoDbSnapshot {
                buffer_pool_hit_ratio: None,
                buffer_pool_pages_total: 0,
                buffer_pool_pages_free: 0,
                buffer_pool_pages_dirty: 0,
                buffer_pool_dirty_pct: None,
                buffer_pool_free_pct: None,
                log_waits: 0,
                row_lock_waits: 0,
                row_lock_time_ms: 0,
                deadlocks: 0,
            },
            statements: Vec::new(),
            tables,
            partitions,
            indexes: Vec::new(),
            privileges: Vec::new(),
            waits: Vec::new(),
            locks: MySqlLockSnapshot {
                blocked_processes: 0,
                pending_metadata_locks: None,
                data_lock_waits: None,
            },
            findings: Vec::new(),
        }
    }
}

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

// Deterministic table-bloat inventory evaluator for roadmap rows
// 03-MYSQL-AI-TRIAGER-00932/00939/00960.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::services::analytics::mysql_analytics::mysql_telemetry::{
    MySqlTableTelemetry, MySqlTelemetrySnapshot,
};
use crate::services::aws::inventory::types::{
    score_pillar, InventoryFinding, Pillar, PillarReport, Severity, COST_ALLOCATION_TAG_KEYS,
    DEFAULT_STALE_AFTER_HOURS,
};

const MIN_BLOAT_BYTES: i64 = 64 * 1024 * 1024;
const LARGE_BLOAT_BYTES: i64 = 1024 * 1024 * 1024;
const BLOAT_RATIO_THRESHOLD: f64 = 0.20;

pub const RESOURCE_TYPE: &str = "MySqlTableBloat";
pub const REASON_COST_OWNER_NOT_RECORDED: &str = "MYSQL_TABLE_BLOAT_COST_OWNER_NOT_RECORDED";
pub const REASON_COST_NO_TABLE_EVIDENCE: &str = "MYSQL_TABLE_BLOAT_COST_NO_TABLE_EVIDENCE";
pub const REASON_COST_RECLAIMABLE_STORAGE: &str = "MYSQL_TABLE_BLOAT_COST_RECLAIMABLE_STORAGE";
pub const REASON_RES_NO_TABLE_EVIDENCE: &str = "MYSQL_TABLE_BLOAT_RES_NO_TABLE_EVIDENCE";
pub const REASON_RES_MAINTENANCE_RISK: &str = "MYSQL_TABLE_BLOAT_RES_MAINTENANCE_RISK";
pub const REASON_SEC_OWNER_NOT_RECORDED: &str = "MYSQL_TABLE_BLOAT_SEC_OWNER_NOT_RECORDED";
pub const REASON_SEC_UNROUTED_MAINTENANCE: &str = "MYSQL_TABLE_BLOAT_SEC_UNROUTED_MAINTENANCE";
pub const REASON_INV_STALE_DATA: &str = "MYSQL_TABLE_BLOAT_INV_STALE_DATA";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableBloatSample {
    pub schema_name: String,
    pub table_name: String,
    pub engine: Option<String>,
    pub table_rows: i64,
    pub allocated_bytes: i64,
    pub data_free_bytes: i64,
    pub data_free_pct: f64,
    pub read_count: i64,
    pub write_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableBloatInventoryItem {
    pub connection_id: String,
    pub connection_name: String,
    pub owner: Option<String>,
    pub labels: BTreeMap<String, String>,
    pub table_count: usize,
    pub bloated_table_count: usize,
    pub write_heavy_bloated_table_count: usize,
    pub total_allocated_bytes: i64,
    pub reclaimable_bytes_total: i64,
    pub largest_reclaimable_bytes: i64,
    pub max_data_free_pct: Option<f64>,
    pub sampled_bloated_tables: Vec<TableBloatSample>,
    pub collected_at: DateTime<Utc>,
}

pub fn evaluate_mysql_table_bloat_inventory(
    items: &[TableBloatInventoryItem],
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

pub fn table_bloat_item_from_telemetry(
    connection_id: &str,
    connection_name: &str,
    owner: Option<String>,
    labels: BTreeMap<String, String>,
    snapshot: &MySqlTelemetrySnapshot,
) -> TableBloatInventoryItem {
    let mut samples = snapshot
        .tables
        .iter()
        .filter(|table| is_bloated_table(table))
        .map(sample_from_table)
        .collect::<Vec<_>>();
    samples.sort_by(|left, right| {
        right
            .data_free_bytes
            .cmp(&left.data_free_bytes)
            .then_with(|| right.data_free_pct.total_cmp(&left.data_free_pct))
            .then_with(|| left.table_name.cmp(&right.table_name))
    });

    let total_allocated_bytes = snapshot.tables.iter().map(allocated_bytes).sum();
    let reclaimable_bytes_total = samples.iter().map(|sample| sample.data_free_bytes).sum();
    let largest_reclaimable_bytes = samples
        .iter()
        .map(|sample| sample.data_free_bytes)
        .max()
        .unwrap_or(0);
    let max_data_free_pct = samples
        .iter()
        .map(|sample| sample.data_free_pct)
        .max_by(|left, right| left.total_cmp(right));
    let write_heavy_bloated_table_count = samples
        .iter()
        .filter(|sample| sample.write_count > sample.read_count)
        .count();

    TableBloatInventoryItem {
        connection_id: connection_id.to_string(),
        connection_name: connection_name.to_string(),
        owner,
        labels,
        table_count: snapshot.tables.len(),
        bloated_table_count: samples.len(),
        write_heavy_bloated_table_count,
        total_allocated_bytes,
        reclaimable_bytes_total,
        largest_reclaimable_bytes,
        max_data_free_pct,
        sampled_bloated_tables: samples.into_iter().take(10).collect(),
        collected_at: snapshot.collected_at,
    }
}

fn evaluate_cost(
    item: &TableBloatInventoryItem,
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
                "Table-bloat inventory for {} has no owner, team, project, or cost-center metadata",
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
                "Table-bloat inventory for {} has no table-size evidence",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "table_count": item.table_count,
                "recommendation": "Collect information_schema table size and DATA_FREE evidence before estimating reclaimable storage",
            }),
        ));
    }

    if item.bloated_table_count > 0 {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_RECLAIMABLE_STORAGE,
            Severity::Medium,
            format!(
                "Table-bloat inventory for {} has reclaimable storage candidates",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "bloated_table_count": item.bloated_table_count,
                "reclaimable_bytes_total": item.reclaimable_bytes_total,
                "largest_reclaimable_bytes": item.largest_reclaimable_bytes,
                "max_data_free_pct": item.max_data_free_pct,
                "sampled_bloated_tables": item.sampled_bloated_tables,
                "recommendation": "Review table churn and maintenance windows before resizing storage or running OPTIMIZE TABLE",
            }),
        ));
    }
}

fn evaluate_resilience(
    item: &TableBloatInventoryItem,
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
                "Table-bloat inventory for {} has no resilience evidence",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "recommendation": "Collect table size, DATA_FREE, and table I/O counters so maintenance risk can be planned deterministically",
            }),
        ));
    }

    if item.write_heavy_bloated_table_count > 0
        || item.largest_reclaimable_bytes >= LARGE_BLOAT_BYTES
    {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_MAINTENANCE_RISK,
            Severity::Medium,
            format!(
                "Table-bloat evidence for {} needs maintenance-window review",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "bloated_table_count": item.bloated_table_count,
                "write_heavy_bloated_table_count": item.write_heavy_bloated_table_count,
                "largest_reclaimable_bytes": item.largest_reclaimable_bytes,
                "sampled_bloated_tables": item.sampled_bloated_tables,
                "recommendation": "Plan online DDL, backup freshness, and rollback notes before reclaiming space from write-heavy or very large tables",
            }),
        ));
    }
}

fn evaluate_security(
    item: &TableBloatInventoryItem,
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
                "Table-bloat inventory for {} has no owner for DDL review routing",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "checked_keys": COST_ALLOCATION_TAG_KEYS,
            }),
        ));
    }

    if !has_owner_metadata(item) && item.bloated_table_count > 0 {
        findings.push(finding(
            item,
            pillar,
            REASON_SEC_UNROUTED_MAINTENANCE,
            Severity::Medium,
            format!(
                "Table-bloat maintenance candidates for {} cannot be assigned to an accountable owner",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "bloated_table_count": item.bloated_table_count,
                "sampled_bloated_tables": item.sampled_bloated_tables,
                "recommendation": "Assign ownership before approving DDL that may rebuild tables or change application-visible query behavior",
            }),
        ));
    }
}

fn stale_finding(
    item: &TableBloatInventoryItem,
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
            "Inventory data for table-bloat resource {} is {} hours old (threshold {} hours)",
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
    item: &TableBloatInventoryItem,
    pillar: Pillar,
    reason_code: &str,
    severity: Severity,
    message: String,
    evidence: Value,
) -> InventoryFinding {
    InventoryFinding {
        resource_id: item.connection_id.clone(),
        arn: format!("mysql://table-bloat/{}", item.connection_id),
        pillar,
        reason_code: reason_code.to_string(),
        severity,
        message,
        evidence,
    }
}

fn has_owner_metadata(item: &TableBloatInventoryItem) -> bool {
    item.owner.as_deref().is_some_and(|owner| !owner.is_empty())
        || COST_ALLOCATION_TAG_KEYS
            .iter()
            .any(|key| item.labels.get(*key).is_some_and(|value| !value.is_empty()))
}

fn is_bloated_table(table: &MySqlTableTelemetry) -> bool {
    table.data_free >= LARGE_BLOAT_BYTES
        || (table.data_free >= MIN_BLOAT_BYTES && data_free_pct(table) >= BLOAT_RATIO_THRESHOLD)
}

fn sample_from_table(table: &MySqlTableTelemetry) -> TableBloatSample {
    TableBloatSample {
        schema_name: table.schema_name.clone(),
        table_name: table.table_name.clone(),
        engine: table.engine.clone(),
        table_rows: table.table_rows,
        allocated_bytes: allocated_bytes(table),
        data_free_bytes: table.data_free,
        data_free_pct: data_free_pct(table),
        read_count: table.read_count,
        write_count: table.write_count,
    }
}

fn allocated_bytes(table: &MySqlTableTelemetry) -> i64 {
    table.data_length.saturating_add(table.index_length)
}

fn data_free_pct(table: &MySqlTableTelemetry) -> f64 {
    let allocated = allocated_bytes(table);
    if allocated <= 0 {
        return 0.0;
    }
    table.data_free as f64 / allocated as f64
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
        bloated_table_count: usize,
        write_heavy_bloated_table_count: usize,
        collected_hours_ago: i64,
    ) -> TableBloatInventoryItem {
        TableBloatInventoryItem {
            connection_id: "conn-1".to_string(),
            connection_name: "orders-db".to_string(),
            owner: owner.map(str::to_string),
            labels,
            table_count,
            bloated_table_count,
            write_heavy_bloated_table_count,
            total_allocated_bytes: 4 * LARGE_BLOAT_BYTES,
            reclaimable_bytes_total: LARGE_BLOAT_BYTES,
            largest_reclaimable_bytes: LARGE_BLOAT_BYTES,
            max_data_free_pct: Some(0.33),
            sampled_bloated_tables: vec![TableBloatSample {
                schema_name: "app".to_string(),
                table_name: "orders".to_string(),
                engine: Some("InnoDB".to_string()),
                table_rows: 1_000_000,
                allocated_bytes: 3 * LARGE_BLOAT_BYTES,
                data_free_bytes: LARGE_BLOAT_BYTES,
                data_free_pct: 0.33,
                read_count: 100,
                write_count: 500,
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
    fn cost_flags_missing_owner_and_reclaimable_storage() {
        let target = item(None, BTreeMap::new(), 4, 1, 0, 1);

        let report = evaluate_mysql_table_bloat_inventory(&[target], Pillar::Cost, now());
        let codes = reason_codes(&report);

        assert!(codes.contains(&REASON_COST_OWNER_NOT_RECORDED));
        assert!(codes.contains(&REASON_COST_RECLAIMABLE_STORAGE));
        let reclaimable = report
            .findings
            .iter()
            .find(|finding| finding.reason_code == REASON_COST_RECLAIMABLE_STORAGE)
            .expect("reclaimable finding");
        assert_eq!(reclaimable.evidence["bloated_table_count"], json!(1));
    }

    #[test]
    fn resilience_flags_write_heavy_or_large_maintenance_risk() {
        let target = item(Some("db-team"), BTreeMap::new(), 4, 1, 1, 1);

        let report = evaluate_mysql_table_bloat_inventory(&[target], Pillar::Resilience, now());

        assert!(reason_codes(&report).contains(&REASON_RES_MAINTENANCE_RISK));
    }

    #[test]
    fn security_routes_unowned_maintenance_candidates() {
        let target = item(None, BTreeMap::new(), 4, 1, 0, 1);

        let report = evaluate_mysql_table_bloat_inventory(&[target], Pillar::Security, now());
        let codes = reason_codes(&report);

        assert!(codes.contains(&REASON_SEC_OWNER_NOT_RECORDED));
        assert!(codes.contains(&REASON_SEC_UNROUTED_MAINTENANCE));
    }

    #[test]
    fn stale_inventory_is_counted_for_any_pillar() {
        let target = item(Some("db-team"), BTreeMap::new(), 4, 0, 0, 48);

        let report = evaluate_mysql_table_bloat_inventory(&[target], Pillar::Cost, now());

        assert_eq!(report.stale_resources, 1);
        let stale = report
            .findings
            .iter()
            .find(|finding| finding.reason_code == REASON_INV_STALE_DATA)
            .expect("stale finding");
        assert_eq!(stale.evidence["age_hours"], json!(48));
    }

    #[test]
    fn telemetry_mapping_sorts_and_counts_bloated_tables() {
        let snapshot = snapshot_with_tables(vec![
            table("small", 10 * 1024 * 1024, 0, 5 * 1024 * 1024, 10, 10),
            table(
                "orders",
                3 * LARGE_BLOAT_BYTES,
                0,
                LARGE_BLOAT_BYTES,
                100,
                500,
            ),
            table("audit", LARGE_BLOAT_BYTES, 0, 256 * 1024 * 1024, 400, 40),
        ]);

        let item = table_bloat_item_from_telemetry(
            "conn-1",
            "orders-db",
            Some("db-team".to_string()),
            BTreeMap::new(),
            &snapshot,
        );

        assert_eq!(item.table_count, 3);
        assert_eq!(item.bloated_table_count, 2);
        assert_eq!(item.write_heavy_bloated_table_count, 1);
        assert_eq!(item.sampled_bloated_tables[0].table_name, "orders");
    }

    #[test]
    fn healthy_table_bloat_passes_claimed_pillars() {
        let mut labels = BTreeMap::new();
        labels.insert("cost-center".to_string(), "cc-42".to_string());
        let target = TableBloatInventoryItem {
            bloated_table_count: 0,
            reclaimable_bytes_total: 0,
            largest_reclaimable_bytes: 0,
            max_data_free_pct: None,
            sampled_bloated_tables: Vec::new(),
            ..item(None, labels, 4, 0, 0, 1)
        };

        for pillar in [Pillar::Cost, Pillar::Resilience, Pillar::Security] {
            let report =
                evaluate_mysql_table_bloat_inventory(std::slice::from_ref(&target), pillar, now());
            assert!(
                report.findings.is_empty(),
                "unexpected findings for {:?}: {:?}",
                pillar,
                report.findings
            );
            assert_eq!(report.score, 100);
        }
    }

    fn table(
        table_name: &str,
        data_length: i64,
        index_length: i64,
        data_free: i64,
        read_count: i64,
        write_count: i64,
    ) -> MySqlTableTelemetry {
        MySqlTableTelemetry {
            schema_name: "app".to_string(),
            table_name: table_name.to_string(),
            engine: Some("InnoDB".to_string()),
            table_rows: 1_000,
            data_length,
            index_length,
            data_free,
            read_count,
            write_count,
        }
    }

    fn snapshot_with_tables(tables: Vec<MySqlTableTelemetry>) -> MySqlTelemetrySnapshot {
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
            partitions: Vec::new(),
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

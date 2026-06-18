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

// Deterministic temporary-table inventory evaluator for roadmap rows
// 03-MYSQL-AI-TRIAGER-01030/01037/01058.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::services::analytics::mysql_analytics::mysql_telemetry::MySqlTelemetrySnapshot;
use crate::services::aws::inventory::types::{
    score_pillar, InventoryFinding, Pillar, PillarReport, Severity, COST_ALLOCATION_TAG_KEYS,
    DEFAULT_STALE_AFTER_HOURS,
};

const DISK_TMP_TABLE_PCT_THRESHOLD: f64 = 25.0;
const DISK_TMP_TABLE_COUNT_THRESHOLD: i64 = 1_000;
const TMP_FILE_COUNT_THRESHOLD: i64 = 100;

pub const RESOURCE_TYPE: &str = "MySqlTemporaryTables";
pub const REASON_COST_OWNER_NOT_RECORDED: &str = "MYSQL_TEMP_TABLES_COST_OWNER_NOT_RECORDED";
pub const REASON_COST_NO_WORKLOAD_EVIDENCE: &str = "MYSQL_TEMP_TABLES_COST_NO_WORKLOAD_EVIDENCE";
pub const REASON_COST_DISK_TEMP_TABLES: &str = "MYSQL_TEMP_TABLES_COST_DISK_TEMP_TABLES";
pub const REASON_RES_NO_WORKLOAD_EVIDENCE: &str = "MYSQL_TEMP_TABLES_RES_NO_WORKLOAD_EVIDENCE";
pub const REASON_RES_DISK_SPILL_RISK: &str = "MYSQL_TEMP_TABLES_RES_DISK_SPILL_RISK";
pub const REASON_SEC_OWNER_NOT_RECORDED: &str = "MYSQL_TEMP_TABLES_SEC_OWNER_NOT_RECORDED";
pub const REASON_SEC_UNROUTED_PRESSURE: &str = "MYSQL_TEMP_TABLES_SEC_UNROUTED_PRESSURE";
pub const REASON_INV_STALE_DATA: &str = "MYSQL_TEMP_TABLES_INV_STALE_DATA";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemporaryTablesInventoryItem {
    pub connection_id: String,
    pub connection_name: String,
    pub owner: Option<String>,
    pub labels: BTreeMap<String, String>,
    pub questions: i64,
    pub queries: i64,
    pub slow_queries: i64,
    pub created_tmp_tables: i64,
    pub created_tmp_disk_tables: i64,
    pub created_tmp_files: i64,
    pub tmp_disk_table_pct: Option<f64>,
    pub qps_since_start: f64,
    pub collected_at: DateTime<Utc>,
}

pub fn evaluate_mysql_temporary_tables_inventory(
    items: &[TemporaryTablesInventoryItem],
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

pub fn temporary_tables_item_from_telemetry(
    connection_id: &str,
    connection_name: &str,
    owner: Option<String>,
    labels: BTreeMap<String, String>,
    snapshot: &MySqlTelemetrySnapshot,
) -> TemporaryTablesInventoryItem {
    TemporaryTablesInventoryItem {
        connection_id: connection_id.to_string(),
        connection_name: connection_name.to_string(),
        owner,
        labels,
        questions: snapshot.workload.questions,
        queries: snapshot.workload.queries,
        slow_queries: snapshot.workload.slow_queries,
        created_tmp_tables: snapshot.workload.created_tmp_tables,
        created_tmp_disk_tables: snapshot.workload.created_tmp_disk_tables,
        created_tmp_files: snapshot.workload.created_tmp_files,
        tmp_disk_table_pct: snapshot.workload.tmp_disk_table_pct,
        qps_since_start: snapshot.workload.qps_since_start,
        collected_at: snapshot.collected_at,
    }
}

fn evaluate_cost(
    item: &TemporaryTablesInventoryItem,
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
                "Temporary-table inventory for {} has no owner, team, project, or cost-center metadata",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "connection_name": item.connection_name,
                "checked_keys": COST_ALLOCATION_TAG_KEYS,
            }),
        ));
    }

    if !has_workload_evidence(item) {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_NO_WORKLOAD_EVIDENCE,
            Severity::High,
            format!(
                "Temporary-table inventory for {} has no workload status evidence",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "questions": item.questions,
                "queries": item.queries,
                "recommendation": "Collect global status counters before estimating temporary-table memory, disk, or query-shape savings",
            }),
        ));
    }

    if has_disk_temp_table_pressure(item) {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_DISK_TEMP_TABLES,
            Severity::Medium,
            format!(
                "Temporary-table inventory for {} shows disk spill cost pressure",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "created_tmp_tables": item.created_tmp_tables,
                "created_tmp_disk_tables": item.created_tmp_disk_tables,
                "created_tmp_files": item.created_tmp_files,
                "tmp_disk_table_pct": item.tmp_disk_table_pct,
                "recommendation": "Review query shapes, tmp_table_size, max_heap_table_size, and indexes before scaling storage or memory for disk temporary tables",
            }),
        ));
    }
}

fn evaluate_resilience(
    item: &TemporaryTablesInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if !has_workload_evidence(item) {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_NO_WORKLOAD_EVIDENCE,
            Severity::High,
            format!(
                "Temporary-table inventory for {} has no resilience evidence",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "recommendation": "Collect temporary-table counters so incidents can distinguish query spills from storage or CPU saturation",
            }),
        ));
    }

    if has_disk_temp_table_pressure(item) {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_DISK_SPILL_RISK,
            Severity::Medium,
            format!(
                "Temporary-table evidence for {} shows disk spill risk",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "created_tmp_disk_tables": item.created_tmp_disk_tables,
                "created_tmp_files": item.created_tmp_files,
                "tmp_disk_table_pct": item.tmp_disk_table_pct,
                "slow_queries": item.slow_queries,
                "recommendation": "Correlate disk temporary tables with top statement digests before changing memory limits or scheduling query rewrites",
            }),
        ));
    }
}

fn evaluate_security(
    item: &TemporaryTablesInventoryItem,
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
                "Temporary-table inventory for {} has no owner for query-change review routing",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "checked_keys": COST_ALLOCATION_TAG_KEYS,
            }),
        ));
    }

    if !has_owner_metadata(item) && has_disk_temp_table_pressure(item) {
        findings.push(finding(
            item,
            pillar,
            REASON_SEC_UNROUTED_PRESSURE,
            Severity::Medium,
            format!(
                "Temporary-table pressure for {} cannot be assigned to an accountable owner",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "created_tmp_disk_tables": item.created_tmp_disk_tables,
                "tmp_disk_table_pct": item.tmp_disk_table_pct,
                "recommendation": "Assign ownership before approving query rewrites or memory setting changes that can affect authorization-sensitive workloads",
            }),
        ));
    }
}

fn stale_finding(
    item: &TemporaryTablesInventoryItem,
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
            "Inventory data for temporary-table resource {} is {} hours old (threshold {} hours)",
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
    item: &TemporaryTablesInventoryItem,
    pillar: Pillar,
    reason_code: &str,
    severity: Severity,
    message: String,
    evidence: Value,
) -> InventoryFinding {
    InventoryFinding {
        resource_id: item.connection_id.clone(),
        arn: format!("mysql://temporary-tables/{}", item.connection_id),
        pillar,
        reason_code: reason_code.to_string(),
        severity,
        message,
        evidence,
    }
}

fn has_owner_metadata(item: &TemporaryTablesInventoryItem) -> bool {
    item.owner.as_deref().is_some_and(|owner| !owner.is_empty())
        || COST_ALLOCATION_TAG_KEYS
            .iter()
            .any(|key| item.labels.get(*key).is_some_and(|value| !value.is_empty()))
}

fn has_workload_evidence(item: &TemporaryTablesInventoryItem) -> bool {
    item.questions > 0
        || item.queries > 0
        || item.created_tmp_tables > 0
        || item.created_tmp_disk_tables > 0
        || item.created_tmp_files > 0
}

fn has_disk_temp_table_pressure(item: &TemporaryTablesInventoryItem) -> bool {
    item.tmp_disk_table_pct
        .is_some_and(|pct| pct >= DISK_TMP_TABLE_PCT_THRESHOLD)
        || item.created_tmp_disk_tables >= DISK_TMP_TABLE_COUNT_THRESHOLD
        || item.created_tmp_files >= TMP_FILE_COUNT_THRESHOLD
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
        created_tmp_tables: i64,
        created_tmp_disk_tables: i64,
        collected_hours_ago: i64,
    ) -> TemporaryTablesInventoryItem {
        TemporaryTablesInventoryItem {
            connection_id: "conn-1".to_string(),
            connection_name: "orders-db".to_string(),
            owner: owner.map(str::to_string),
            labels,
            questions: 10_000,
            queries: 10_000,
            slow_queries: 25,
            created_tmp_tables,
            created_tmp_disk_tables,
            created_tmp_files: 125,
            tmp_disk_table_pct: if created_tmp_tables > 0 {
                Some(created_tmp_disk_tables as f64 / created_tmp_tables as f64 * 100.0)
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
    fn cost_flags_missing_owner_and_disk_temp_pressure() {
        let target = item(None, BTreeMap::new(), 1_000, 400, 1);

        let report = evaluate_mysql_temporary_tables_inventory(&[target], Pillar::Cost, now());
        let codes = reason_codes(&report);

        assert!(codes.contains(&REASON_COST_OWNER_NOT_RECORDED));
        assert!(codes.contains(&REASON_COST_DISK_TEMP_TABLES));
    }

    #[test]
    fn resilience_flags_disk_spill_risk() {
        let target = item(Some("db-team"), BTreeMap::new(), 1_000, 300, 1);

        let report =
            evaluate_mysql_temporary_tables_inventory(&[target], Pillar::Resilience, now());

        assert!(reason_codes(&report).contains(&REASON_RES_DISK_SPILL_RISK));
    }

    #[test]
    fn security_routes_unowned_temp_table_pressure() {
        let target = item(None, BTreeMap::new(), 1_000, 300, 1);

        let report = evaluate_mysql_temporary_tables_inventory(&[target], Pillar::Security, now());
        let codes = reason_codes(&report);

        assert!(codes.contains(&REASON_SEC_OWNER_NOT_RECORDED));
        assert!(codes.contains(&REASON_SEC_UNROUTED_PRESSURE));
    }

    #[test]
    fn stale_inventory_is_counted_for_any_pillar() {
        let target = item(Some("db-team"), BTreeMap::new(), 1_000, 0, 48);

        let report = evaluate_mysql_temporary_tables_inventory(&[target], Pillar::Cost, now());

        assert_eq!(report.stale_resources, 1);
        let stale = report
            .findings
            .iter()
            .find(|finding| finding.reason_code == REASON_INV_STALE_DATA)
            .expect("stale finding");
        assert_eq!(stale.evidence["age_hours"], json!(48));
    }

    #[test]
    fn empty_workload_evidence_is_reported() {
        let mut target = item(Some("db-team"), BTreeMap::new(), 0, 0, 1);
        target.questions = 0;
        target.queries = 0;
        target.created_tmp_files = 0;

        let report = evaluate_mysql_temporary_tables_inventory(&[target], Pillar::Cost, now());

        assert!(reason_codes(&report).contains(&REASON_COST_NO_WORKLOAD_EVIDENCE));
    }

    #[test]
    fn healthy_temporary_tables_pass_claimed_pillars() {
        let mut labels = BTreeMap::new();
        labels.insert("cost-center".to_string(), "cc-42".to_string());
        let mut target = item(None, labels, 1_000, 10, 1);
        target.created_tmp_files = 0;

        for pillar in [Pillar::Cost, Pillar::Resilience, Pillar::Security] {
            let report = evaluate_mysql_temporary_tables_inventory(
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

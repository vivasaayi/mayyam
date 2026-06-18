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

// Deterministic sys schema health evaluator for roadmap rows
// 03-MYSQL-AI-TRIAGER-00051/00058/00079.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::services::analytics::mysql_analytics::mysql_telemetry::{
    MySqlFindingSeverity, MySqlTelemetrySnapshot,
};
use crate::services::aws::inventory::types::{
    score_pillar, InventoryFinding, Pillar, PillarReport, Severity, DEFAULT_STALE_AFTER_HOURS,
};

pub const RESOURCE_TYPE: &str = "MySqlSysSchemaHealth";
pub const REASON_COST_SCHEMA_UNAVAILABLE: &str = "MYSQL_SYS_SCHEMA_HEALTH_COST_UNAVAILABLE";
pub const REASON_COST_OBJECT_COVERAGE_MISSING: &str =
    "MYSQL_SYS_SCHEMA_HEALTH_COST_OBJECT_COVERAGE_MISSING";
pub const REASON_COST_INDEX_WASTE_VISIBLE: &str = "MYSQL_SYS_SCHEMA_HEALTH_COST_INDEX_WASTE";
pub const REASON_RES_SCHEMA_UNAVAILABLE: &str = "MYSQL_SYS_SCHEMA_HEALTH_RES_UNAVAILABLE";
pub const REASON_RES_LOCK_PRESSURE: &str = "MYSQL_SYS_SCHEMA_HEALTH_RES_LOCK_PRESSURE";
pub const REASON_RES_HIGH_PRIORITY_FINDINGS: &str =
    "MYSQL_SYS_SCHEMA_HEALTH_RES_HIGH_PRIORITY_FINDINGS";
pub const REASON_SEC_SCHEMA_UNAVAILABLE: &str = "MYSQL_SYS_SCHEMA_HEALTH_SEC_UNAVAILABLE";
pub const REASON_SEC_VERSION_MISSING: &str = "MYSQL_SYS_SCHEMA_HEALTH_SEC_VERSION_MISSING";
pub const REASON_INV_STALE_DATA: &str = "MYSQL_SYS_SCHEMA_HEALTH_STALE_DATA";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SysSchemaHealthItem {
    pub connection_id: String,
    pub connection_name: String,
    pub server_version: Option<String>,
    pub performance_schema_enabled: Option<String>,
    pub sys_schema_available: bool,
    pub statement_digest_count: usize,
    pub table_count: usize,
    pub index_count: usize,
    pub unused_secondary_index_count: usize,
    pub pending_metadata_locks: Option<i64>,
    pub data_lock_waits: Option<i64>,
    pub blocked_processes: i64,
    pub high_priority_findings: usize,
    pub collected_at: DateTime<Utc>,
}

pub fn evaluate_mysql_sys_schema_health(
    items: &[SysSchemaHealthItem],
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

pub fn sys_schema_health_item_from_telemetry(
    connection_id: &str,
    connection_name: &str,
    snapshot: &MySqlTelemetrySnapshot,
) -> SysSchemaHealthItem {
    let high_priority_findings = snapshot
        .findings
        .iter()
        .filter(|finding| {
            matches!(
                finding.severity,
                MySqlFindingSeverity::Critical | MySqlFindingSeverity::High
            )
        })
        .count();
    let unused_secondary_index_count = snapshot
        .indexes
        .iter()
        .filter(|index| !index.is_primary && !index.is_unique && index.read_count == 0)
        .count();

    SysSchemaHealthItem {
        connection_id: connection_id.to_string(),
        connection_name: connection_name.to_string(),
        server_version: snapshot.server.version.clone(),
        performance_schema_enabled: snapshot.server.performance_schema_enabled.clone(),
        sys_schema_available: snapshot.server.sys_schema_available,
        statement_digest_count: snapshot.statements.len(),
        table_count: snapshot.tables.len(),
        index_count: snapshot.indexes.len(),
        unused_secondary_index_count,
        pending_metadata_locks: snapshot.locks.pending_metadata_locks,
        data_lock_waits: snapshot.locks.data_lock_waits,
        blocked_processes: snapshot.locks.blocked_processes,
        high_priority_findings,
        collected_at: snapshot.collected_at,
    }
}

fn evaluate_cost(item: &SysSchemaHealthItem, pillar: Pillar, findings: &mut Vec<InventoryFinding>) {
    if !item.sys_schema_available {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_SCHEMA_UNAVAILABLE,
            Severity::High,
            format!("sys schema health views are unavailable for {}", item.connection_name),
            json!({
                "connection_id": item.connection_id,
                "sys_schema_available": item.sys_schema_available,
                "performance_schema_enabled": item.performance_schema_enabled,
                "recommendation": "Enable sys schema before relying on normalized object and statement cost summaries",
            }),
        ));
    }

    if item.statement_digest_count == 0 || item.table_count == 0 || item.index_count == 0 {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_OBJECT_COVERAGE_MISSING,
            Severity::Medium,
            format!(
                "sys schema health for {} has incomplete statement, table, or index coverage",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "statement_digest_count": item.statement_digest_count,
                "table_count": item.table_count,
                "index_count": item.index_count,
                "recommendation": "Collect sys schema object summaries so storage and query-cost findings are explainable",
            }),
        ));
    }

    if item.unused_secondary_index_count > 0 {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_INDEX_WASTE_VISIBLE,
            Severity::Low,
            format!(
                "sys schema health for {} shows unused secondary index candidates",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "unused_secondary_index_count": item.unused_secondary_index_count,
                "recommendation": "Review unused index candidates and write amplification before dropping indexes",
            }),
        ));
    }
}

fn evaluate_resilience(
    item: &SysSchemaHealthItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if !item.sys_schema_available {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_SCHEMA_UNAVAILABLE,
            Severity::High,
            format!("sys schema resilience views are unavailable for {}", item.connection_name),
            json!({
                "connection_id": item.connection_id,
                "sys_schema_available": item.sys_schema_available,
                "recommendation": "Enable sys schema so lock, wait, and object health views can support incident triage",
            }),
        ));
    }

    if item.blocked_processes > 0
        || item.pending_metadata_locks.unwrap_or(0) > 0
        || item.data_lock_waits.unwrap_or(0) > 0
    {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_LOCK_PRESSURE,
            Severity::High,
            format!("sys schema health for {} shows lock pressure", item.connection_name),
            json!({
                "connection_id": item.connection_id,
                "blocked_processes": item.blocked_processes,
                "pending_metadata_locks": item.pending_metadata_locks,
                "data_lock_waits": item.data_lock_waits,
                "recommendation": "Identify blockers before remediation; inspect processlist, metadata locks, and transaction age",
            }),
        ));
    }

    if item.high_priority_findings > 0 {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_HIGH_PRIORITY_FINDINGS,
            Severity::Medium,
            format!(
                "sys schema health for {} includes high-priority telemetry findings",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "high_priority_findings": item.high_priority_findings,
            }),
        ));
    }
}

fn evaluate_security(
    item: &SysSchemaHealthItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if !item.sys_schema_available {
        findings.push(finding(
            item,
            pillar,
            REASON_SEC_SCHEMA_UNAVAILABLE,
            Severity::High,
            format!("sys schema security views are unavailable for {}", item.connection_name),
            json!({
                "connection_id": item.connection_id,
                "sys_schema_available": item.sys_schema_available,
                "recommendation": "Enable sys schema evidence so security-sensitive diagnostics are normalized and auditable",
            }),
        ));
    }

    if item
        .server_version
        .as_deref()
        .map(str::trim)
        .filter(|version| !version.is_empty())
        .is_none()
    {
        findings.push(finding(
            item,
            pillar,
            REASON_SEC_VERSION_MISSING,
            Severity::Medium,
            format!("sys schema health for {} has no server version evidence", item.connection_name),
            json!({
                "connection_id": item.connection_id,
                "server_version": item.server_version,
                "recommendation": "Record MySQL version with sys schema health evidence so advisory mapping is deterministic",
            }),
        ));
    }
}

fn stale_finding(
    item: &SysSchemaHealthItem,
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
            "sys schema health data for {} is {} hours old (threshold {} hours)",
            item.connection_name, age_hours, DEFAULT_STALE_AFTER_HOURS
        ),
        json!({
            "collected_at": item.collected_at,
            "age_hours": age_hours,
            "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
        }),
    ))
}

fn finding(
    item: &SysSchemaHealthItem,
    pillar: Pillar,
    reason_code: &str,
    severity: Severity,
    message: String,
    evidence: Value,
) -> InventoryFinding {
    InventoryFinding {
        resource_id: item.connection_id.clone(),
        arn: format!("mysql-sys-schema-health:{}", item.connection_id),
        pillar,
        reason_code: reason_code.to_string(),
        severity,
        message,
        evidence,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    fn item() -> SysSchemaHealthItem {
        SysSchemaHealthItem {
            connection_id: "conn-1".to_string(),
            connection_name: "prod-mysql".to_string(),
            server_version: Some("8.0.36".to_string()),
            performance_schema_enabled: Some("ON".to_string()),
            sys_schema_available: true,
            statement_digest_count: 5,
            table_count: 20,
            index_count: 12,
            unused_secondary_index_count: 0,
            pending_metadata_locks: Some(0),
            data_lock_waits: Some(0),
            blocked_processes: 0,
            high_priority_findings: 0,
            collected_at: Utc::now(),
        }
    }

    #[test]
    fn cost_flags_unavailable_schema_and_missing_coverage() {
        let now = Utc::now();
        let mut item = item();
        item.sys_schema_available = false;
        item.statement_digest_count = 0;

        let report = evaluate_mysql_sys_schema_health(&[item], Pillar::Cost, now);

        assert_eq!(report.resources_evaluated, 1);
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_COST_SCHEMA_UNAVAILABLE));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_COST_OBJECT_COVERAGE_MISSING));
    }

    #[test]
    fn resilience_flags_lock_pressure_and_high_priority_findings() {
        let now = Utc::now();
        let mut item = item();
        item.blocked_processes = 2;
        item.pending_metadata_locks = Some(1);
        item.high_priority_findings = 3;

        let report = evaluate_mysql_sys_schema_health(&[item], Pillar::Resilience, now);

        assert_eq!(report.findings.len(), 2);
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_RES_LOCK_PRESSURE));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_RES_HIGH_PRIORITY_FINDINGS));
    }

    #[test]
    fn security_flags_unavailable_schema_and_missing_version() {
        let now = Utc::now();
        let mut item = item();
        item.sys_schema_available = false;
        item.server_version = None;

        let report = evaluate_mysql_sys_schema_health(&[item], Pillar::Security, now);

        assert_eq!(report.findings.len(), 2);
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_SEC_SCHEMA_UNAVAILABLE));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_SEC_VERSION_MISSING));
    }

    #[test]
    fn stale_health_data_is_counted_for_any_pillar() {
        let now = Utc::now();
        let mut item = item();
        item.collected_at = now - Duration::hours(DEFAULT_STALE_AFTER_HOURS + 2);

        let report = evaluate_mysql_sys_schema_health(&[item], Pillar::Cost, now);

        assert_eq!(report.stale_resources, 1);
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_INV_STALE_DATA));
    }

    #[test]
    fn healthy_sys_schema_health_passes_claimed_pillars() {
        let now = Utc::now();
        let items = vec![item()];

        for pillar in [Pillar::Cost, Pillar::Resilience, Pillar::Security] {
            let report = evaluate_mysql_sys_schema_health(&items, pillar, now);
            assert_eq!(report.score, 100);
            assert!(report.findings.is_empty());
        }
    }
}

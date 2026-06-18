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

// Deterministic restore-drill inventory evaluator for roadmap rows
// 03-MYSQL-AI-TRIAGER-01422/01429/01450.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::services::analytics::mysql_analytics::mysql_telemetry::MySqlTelemetrySnapshot;
use crate::services::aws::inventory::types::{
    score_pillar, InventoryFinding, Pillar, PillarReport, Severity, COST_ALLOCATION_TAG_KEYS,
    DEFAULT_STALE_AFTER_HOURS,
};

pub const RESOURCE_TYPE: &str = "MySqlRestoreDrill";
pub const REASON_COST_OWNER_NOT_RECORDED: &str = "MYSQL_RESTORE_DRILL_COST_OWNER_NOT_RECORDED";
pub const REASON_COST_DRILL_EVIDENCE_MISSING: &str = "MYSQL_RESTORE_DRILL_COST_EVIDENCE_MISSING";
pub const REASON_COST_LONG_RESTORE_REVIEW: &str = "MYSQL_RESTORE_DRILL_COST_LONG_RESTORE_REVIEW";
pub const REASON_RES_DRILL_EVIDENCE_MISSING: &str = "MYSQL_RESTORE_DRILL_RES_EVIDENCE_MISSING";
pub const REASON_RES_PITR_OR_DRILL_GAP: &str = "MYSQL_RESTORE_DRILL_RES_PITR_OR_DRILL_GAP";
pub const REASON_SEC_OWNER_NOT_RECORDED: &str = "MYSQL_RESTORE_DRILL_SEC_OWNER_NOT_RECORDED";
pub const REASON_SEC_RESTORE_CHAIN_NOT_AUDITABLE: &str =
    "MYSQL_RESTORE_DRILL_SEC_RESTORE_CHAIN_NOT_AUDITABLE";
pub const REASON_INV_STALE_DATA: &str = "MYSQL_RESTORE_DRILL_INV_STALE_DATA";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestoreDrillInventoryItem {
    pub connection_id: String,
    pub connection_name: String,
    pub owner: Option<String>,
    pub labels: BTreeMap<String, String>,
    pub server_version: Option<String>,
    pub log_bin: Option<String>,
    pub binlog_expire_logs_seconds: Option<i64>,
    pub expire_logs_days: Option<i64>,
    pub gtid_mode: Option<String>,
    pub table_count: usize,
    pub write_operations: i64,
    pub last_restore_drill_at: Option<DateTime<Utc>>,
    pub last_restore_duration_minutes: Option<i64>,
    pub last_restore_successful: Option<bool>,
    pub collected_at: DateTime<Utc>,
}

pub fn evaluate_mysql_restore_drills_inventory(
    items: &[RestoreDrillInventoryItem],
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

pub fn restore_drill_item_from_telemetry(
    connection_id: &str,
    connection_name: &str,
    owner: Option<String>,
    labels: BTreeMap<String, String>,
    snapshot: &MySqlTelemetrySnapshot,
) -> RestoreDrillInventoryItem {
    RestoreDrillInventoryItem {
        connection_id: connection_id.to_string(),
        connection_name: connection_name.to_string(),
        owner,
        labels,
        server_version: snapshot.server.version.clone(),
        log_bin: snapshot.server.log_bin.clone(),
        binlog_expire_logs_seconds: snapshot.server.binlog_expire_logs_seconds,
        expire_logs_days: snapshot.server.expire_logs_days,
        gtid_mode: snapshot.server.gtid_mode.clone(),
        table_count: snapshot.tables.len(),
        write_operations: snapshot.workload.com_insert
            + snapshot.workload.com_update
            + snapshot.workload.com_delete,
        last_restore_drill_at: None,
        last_restore_duration_minutes: None,
        last_restore_successful: None,
        collected_at: snapshot.collected_at,
    }
}

fn evaluate_cost(
    item: &RestoreDrillInventoryItem,
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
                "Restore-drill inventory for {} has no owner, team, project, or cost-center metadata",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "checked_keys": COST_ALLOCATION_TAG_KEYS,
            }),
        ));
    }

    if !has_drill_evidence(item) {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_DRILL_EVIDENCE_MISSING,
            Severity::High,
            format!(
                "Restore-drill inventory for {} has no recorded drill evidence",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "recommendation": "Record restore drill timestamp, duration, and outcome before estimating recovery labor or migration cost",
            }),
        ));
    }

    if item
        .last_restore_duration_minutes
        .is_some_and(|duration| duration > 240)
    {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_LONG_RESTORE_REVIEW,
            Severity::Medium,
            format!(
                "Restore-drill inventory for {} has a long restore duration",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "last_restore_duration_minutes": item.last_restore_duration_minutes,
                "table_count": item.table_count,
                "write_operations": item.write_operations,
                "recommendation": "Review restore automation, data volume, and staffing assumptions before committing recovery objectives",
            }),
        ));
    }
}

fn evaluate_resilience(
    item: &RestoreDrillInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if !has_drill_evidence(item) {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_DRILL_EVIDENCE_MISSING,
            Severity::High,
            format!(
                "Restore-drill inventory for {} has no tested recovery evidence",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "recommendation": "Run and record restore drills so recovery posture is based on measured evidence",
            }),
        ));
    }

    if !pitr_ready(item) || item.last_restore_successful != Some(true) {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_PITR_OR_DRILL_GAP,
            Severity::High,
            format!(
                "Restore-drill inventory for {} has PITR or successful-drill gaps",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "log_bin": item.log_bin,
                "retention_days": retention_days(item),
                "last_restore_successful": item.last_restore_successful,
                "last_restore_drill_at": item.last_restore_drill_at,
                "recommendation": "Validate PITR prerequisites and complete a successful restore drill before relying on recovery objectives",
            }),
        ));
    }
}

fn evaluate_security(
    item: &RestoreDrillInventoryItem,
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
                "Restore-drill inventory for {} has no owner for recovery audit routing",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "checked_keys": COST_ALLOCATION_TAG_KEYS,
            }),
        ));
    }

    if item.server_version.as_deref().is_none_or(str::is_empty)
        || item.gtid_mode.as_deref().is_none_or(str::is_empty)
        || !has_drill_evidence(item)
        || item.last_restore_successful != Some(true)
    {
        findings.push(finding(
            item,
            pillar,
            REASON_SEC_RESTORE_CHAIN_NOT_AUDITABLE,
            Severity::High,
            format!(
                "Restore-drill inventory for {} is missing auditable recovery evidence",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "server_version": item.server_version,
                "gtid_mode": item.gtid_mode,
                "last_restore_drill_at": item.last_restore_drill_at,
                "last_restore_successful": item.last_restore_successful,
                "recommendation": "Keep version, GTID, PITR, and successful drill evidence available for incident and compliance review",
            }),
        ));
    }
}

fn stale_finding(
    item: &RestoreDrillInventoryItem,
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
            "Inventory data for restore-drill resource {} is {} hours old (threshold {} hours)",
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
    item: &RestoreDrillInventoryItem,
    pillar: Pillar,
    reason_code: &str,
    severity: Severity,
    message: String,
    evidence: Value,
) -> InventoryFinding {
    InventoryFinding {
        resource_id: item.connection_id.clone(),
        arn: format!("mysql://restore-drill/{}", item.connection_id),
        pillar,
        reason_code: reason_code.to_string(),
        severity,
        message,
        evidence,
    }
}

fn has_owner_metadata(item: &RestoreDrillInventoryItem) -> bool {
    item.owner.as_deref().is_some_and(|owner| !owner.is_empty())
        || COST_ALLOCATION_TAG_KEYS
            .iter()
            .any(|key| item.labels.get(*key).is_some_and(|value| !value.is_empty()))
}

fn has_drill_evidence(item: &RestoreDrillInventoryItem) -> bool {
    item.last_restore_drill_at.is_some()
        && item.last_restore_duration_minutes.is_some()
        && item.last_restore_successful.is_some()
}

fn pitr_ready(item: &RestoreDrillInventoryItem) -> bool {
    item.log_bin
        .as_deref()
        .is_some_and(|value| value.eq_ignore_ascii_case("ON") || value == "1")
        && retention_days(item).is_some_and(|days| days >= 7)
}

fn retention_days(item: &RestoreDrillInventoryItem) -> Option<i64> {
    item.binlog_expire_logs_seconds
        .map(|seconds| seconds / 86_400)
        .or(item.expire_logs_days)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    fn now() -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2026-06-14T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc)
    }

    fn item(owner: Option<&str>, successful: Option<bool>) -> RestoreDrillInventoryItem {
        RestoreDrillInventoryItem {
            connection_id: "conn-1".to_string(),
            connection_name: "orders-db".to_string(),
            owner: owner.map(str::to_string),
            labels: BTreeMap::new(),
            server_version: Some("8.0.36".to_string()),
            log_bin: Some("ON".to_string()),
            binlog_expire_logs_seconds: Some(14 * 86_400),
            expire_logs_days: None,
            gtid_mode: Some("ON".to_string()),
            table_count: 12,
            write_operations: 500,
            last_restore_drill_at: Some(now() - Duration::hours(12)),
            last_restore_duration_minutes: Some(90),
            last_restore_successful: successful,
            collected_at: now() - Duration::hours(1),
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
    fn cost_flags_missing_owner_missing_evidence_and_long_restore() {
        let mut target = item(None, None);
        target.last_restore_drill_at = None;
        target.last_restore_duration_minutes = Some(300);

        let report = evaluate_mysql_restore_drills_inventory(&[target], Pillar::Cost, now());
        let codes = reason_codes(&report);

        assert!(codes.contains(&REASON_COST_OWNER_NOT_RECORDED));
        assert!(codes.contains(&REASON_COST_DRILL_EVIDENCE_MISSING));
        assert!(codes.contains(&REASON_COST_LONG_RESTORE_REVIEW));
    }

    #[test]
    fn resilience_flags_missing_drill_and_pitr_gap() {
        let mut target = item(Some("db-team"), None);
        target.last_restore_drill_at = None;
        target.log_bin = Some("OFF".to_string());

        let report = evaluate_mysql_restore_drills_inventory(&[target], Pillar::Resilience, now());
        let codes = reason_codes(&report);

        assert!(codes.contains(&REASON_RES_DRILL_EVIDENCE_MISSING));
        assert!(codes.contains(&REASON_RES_PITR_OR_DRILL_GAP));
    }

    #[test]
    fn security_flags_missing_owner_and_audit_gap() {
        let mut target = item(None, Some(false));
        target.gtid_mode = None;

        let report = evaluate_mysql_restore_drills_inventory(&[target], Pillar::Security, now());
        let codes = reason_codes(&report);

        assert!(codes.contains(&REASON_SEC_OWNER_NOT_RECORDED));
        assert!(codes.contains(&REASON_SEC_RESTORE_CHAIN_NOT_AUDITABLE));
    }

    #[test]
    fn stale_inventory_is_counted_for_any_pillar() {
        let mut target = item(Some("db-team"), Some(true));
        target.collected_at = now() - Duration::hours(48);

        let report = evaluate_mysql_restore_drills_inventory(&[target], Pillar::Cost, now());

        assert_eq!(report.stale_resources, 1);
        assert!(reason_codes(&report).contains(&REASON_INV_STALE_DATA));
    }

    #[test]
    fn healthy_restore_drill_passes_claimed_pillars() {
        let mut target = item(Some("db-team"), Some(true));
        target
            .labels
            .insert("cost-center".to_string(), "cc-42".to_string());

        for pillar in [Pillar::Cost, Pillar::Resilience, Pillar::Security] {
            let report = evaluate_mysql_restore_drills_inventory(
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

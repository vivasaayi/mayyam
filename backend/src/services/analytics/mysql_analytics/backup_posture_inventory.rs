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

// Deterministic backup-posture inventory evaluator for roadmap rows
// 03-MYSQL-AI-TRIAGER-01373/01380/01401.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::services::analytics::mysql_analytics::mysql_telemetry::MySqlTelemetrySnapshot;
use crate::services::aws::inventory::types::{
    score_pillar, InventoryFinding, Pillar, PillarReport, Severity, COST_ALLOCATION_TAG_KEYS,
    DEFAULT_STALE_AFTER_HOURS,
};

pub const RESOURCE_TYPE: &str = "MySqlBackupPosture";
pub const REASON_COST_OWNER_NOT_RECORDED: &str = "MYSQL_BACKUP_POSTURE_COST_OWNER_NOT_RECORDED";
pub const REASON_COST_BACKUP_EVIDENCE_MISSING: &str = "MYSQL_BACKUP_POSTURE_COST_EVIDENCE_MISSING";
pub const REASON_COST_RETENTION_REVIEW: &str = "MYSQL_BACKUP_POSTURE_COST_RETENTION_REVIEW";
pub const REASON_RES_BACKUP_EVIDENCE_MISSING: &str = "MYSQL_BACKUP_POSTURE_RES_EVIDENCE_MISSING";
pub const REASON_RES_PITR_NOT_READY: &str = "MYSQL_BACKUP_POSTURE_RES_PITR_NOT_READY";
pub const REASON_SEC_OWNER_NOT_RECORDED: &str = "MYSQL_BACKUP_POSTURE_SEC_OWNER_NOT_RECORDED";
pub const REASON_SEC_BACKUP_CHAIN_NOT_AUDITABLE: &str =
    "MYSQL_BACKUP_POSTURE_SEC_BACKUP_CHAIN_NOT_AUDITABLE";
pub const REASON_INV_STALE_DATA: &str = "MYSQL_BACKUP_POSTURE_INV_STALE_DATA";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupPostureInventoryItem {
    pub connection_id: String,
    pub connection_name: String,
    pub owner: Option<String>,
    pub labels: BTreeMap<String, String>,
    pub server_version: Option<String>,
    pub log_bin: Option<String>,
    pub binlog_expire_logs_seconds: Option<i64>,
    pub expire_logs_days: Option<i64>,
    pub gtid_mode: Option<String>,
    pub write_operations: i64,
    pub qps_since_start: f64,
    pub collected_at: DateTime<Utc>,
}

pub fn evaluate_mysql_backup_posture_inventory(
    items: &[BackupPostureInventoryItem],
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

pub fn backup_posture_item_from_telemetry(
    connection_id: &str,
    connection_name: &str,
    owner: Option<String>,
    labels: BTreeMap<String, String>,
    snapshot: &MySqlTelemetrySnapshot,
) -> BackupPostureInventoryItem {
    BackupPostureInventoryItem {
        connection_id: connection_id.to_string(),
        connection_name: connection_name.to_string(),
        owner,
        labels,
        server_version: snapshot.server.version.clone(),
        log_bin: snapshot.server.log_bin.clone(),
        binlog_expire_logs_seconds: snapshot.server.binlog_expire_logs_seconds,
        expire_logs_days: snapshot.server.expire_logs_days,
        gtid_mode: snapshot.server.gtid_mode.clone(),
        write_operations: snapshot.workload.com_insert
            + snapshot.workload.com_update
            + snapshot.workload.com_delete,
        qps_since_start: snapshot.workload.qps_since_start,
        collected_at: snapshot.collected_at,
    }
}

fn evaluate_cost(
    item: &BackupPostureInventoryItem,
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
                "Backup-posture inventory for {} has no owner, team, project, or cost-center metadata",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "checked_keys": COST_ALLOCATION_TAG_KEYS,
            }),
        ));
    }

    if !has_backup_evidence(item) {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_BACKUP_EVIDENCE_MISSING,
            Severity::High,
            format!(
                "Backup-posture inventory for {} has no binary-log retention evidence",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "recommendation": "Collect log_bin, binlog retention, and GTID variables before estimating backup storage or PITR readiness cost",
            }),
        ));
    }

    if retention_days(item).is_some_and(|days| days > 35) {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_RETENTION_REVIEW,
            Severity::Medium,
            format!(
                "Backup-posture inventory for {} has extended binary-log retention",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "retention_days": retention_days(item),
                "write_operations": item.write_operations,
                "qps_since_start": item.qps_since_start,
                "recommendation": "Validate retention requirement and storage impact before increasing backup or PITR windows",
            }),
        ));
    }
}

fn evaluate_resilience(
    item: &BackupPostureInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if !has_backup_evidence(item) {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_BACKUP_EVIDENCE_MISSING,
            Severity::High,
            format!(
                "Backup-posture inventory for {} has no restore-readiness evidence",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "recommendation": "Collect backup and binary-log retention variables before scoring recovery posture",
            }),
        ));
    }

    if !pitr_ready(item) {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_PITR_NOT_READY,
            Severity::High,
            format!(
                "Backup-posture inventory for {} is not point-in-time-recovery ready",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "log_bin": item.log_bin,
                "retention_days": retention_days(item),
                "gtid_mode": item.gtid_mode,
                "recommendation": "Enable binary logging, keep at least seven days of log retention or document an alternate backup path, and validate restore drills",
            }),
        ));
    }
}

fn evaluate_security(
    item: &BackupPostureInventoryItem,
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
                "Backup-posture inventory for {} has no owner for recovery evidence review",
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
        || !pitr_ready(item)
    {
        findings.push(finding(
            item,
            pillar,
            REASON_SEC_BACKUP_CHAIN_NOT_AUDITABLE,
            Severity::High,
            format!(
                "Backup-posture inventory for {} is missing auditable recovery-chain evidence",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "server_version": item.server_version,
                "log_bin": item.log_bin,
                "retention_days": retention_days(item),
                "gtid_mode": item.gtid_mode,
                "recommendation": "Record version, binary-log retention, and GTID evidence so incident restore reviews can verify the recovery chain",
            }),
        ));
    }
}

fn stale_finding(
    item: &BackupPostureInventoryItem,
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
            "Inventory data for backup-posture resource {} is {} hours old (threshold {} hours)",
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
    item: &BackupPostureInventoryItem,
    pillar: Pillar,
    reason_code: &str,
    severity: Severity,
    message: String,
    evidence: Value,
) -> InventoryFinding {
    InventoryFinding {
        resource_id: item.connection_id.clone(),
        arn: format!("mysql://backup-posture/{}", item.connection_id),
        pillar,
        reason_code: reason_code.to_string(),
        severity,
        message,
        evidence,
    }
}

fn has_owner_metadata(item: &BackupPostureInventoryItem) -> bool {
    item.owner.as_deref().is_some_and(|owner| !owner.is_empty())
        || COST_ALLOCATION_TAG_KEYS
            .iter()
            .any(|key| item.labels.get(*key).is_some_and(|value| !value.is_empty()))
}

fn has_backup_evidence(item: &BackupPostureInventoryItem) -> bool {
    item.log_bin.is_some()
        || item.binlog_expire_logs_seconds.is_some()
        || item.expire_logs_days.is_some()
}

fn pitr_ready(item: &BackupPostureInventoryItem) -> bool {
    item.log_bin
        .as_deref()
        .is_some_and(|value| value.eq_ignore_ascii_case("ON") || value == "1")
        && retention_days(item).is_some_and(|days| days >= 7)
}

fn retention_days(item: &BackupPostureInventoryItem) -> Option<i64> {
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

    fn item(
        owner: Option<&str>,
        log_bin: Option<&str>,
        retention_seconds: Option<i64>,
        gtid_mode: Option<&str>,
        collected_hours_ago: i64,
    ) -> BackupPostureInventoryItem {
        BackupPostureInventoryItem {
            connection_id: "conn-1".to_string(),
            connection_name: "orders-db".to_string(),
            owner: owner.map(str::to_string),
            labels: BTreeMap::new(),
            server_version: Some("8.0.36".to_string()),
            log_bin: log_bin.map(str::to_string),
            binlog_expire_logs_seconds: retention_seconds,
            expire_logs_days: None,
            gtid_mode: gtid_mode.map(str::to_string),
            write_operations: 500,
            qps_since_start: 25.0,
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
    fn cost_flags_missing_owner_missing_evidence_and_extended_retention() {
        let missing = item(None, None, None, None, 1);
        let extended = item(
            Some("db-team"),
            Some("ON"),
            Some(60 * 86_400),
            Some("ON"),
            1,
        );

        let report =
            evaluate_mysql_backup_posture_inventory(&[missing, extended], Pillar::Cost, now());
        let codes = reason_codes(&report);

        assert!(codes.contains(&REASON_COST_OWNER_NOT_RECORDED));
        assert!(codes.contains(&REASON_COST_BACKUP_EVIDENCE_MISSING));
        assert!(codes.contains(&REASON_COST_RETENTION_REVIEW));
    }

    #[test]
    fn resilience_flags_missing_evidence_and_pitr_gap() {
        let target = item(Some("db-team"), Some("OFF"), Some(86_400), Some("ON"), 1);

        let report = evaluate_mysql_backup_posture_inventory(&[target], Pillar::Resilience, now());
        let codes = reason_codes(&report);

        assert!(codes.contains(&REASON_RES_PITR_NOT_READY));
    }

    #[test]
    fn resilience_flags_missing_evidence() {
        let target = item(Some("db-team"), None, None, None, 1);

        let report = evaluate_mysql_backup_posture_inventory(&[target], Pillar::Resilience, now());

        assert!(reason_codes(&report).contains(&REASON_RES_BACKUP_EVIDENCE_MISSING));
    }

    #[test]
    fn security_flags_missing_owner_and_audit_chain_gap() {
        let mut target = item(None, Some("ON"), Some(3 * 86_400), None, 1);
        target.server_version = None;

        let report = evaluate_mysql_backup_posture_inventory(&[target], Pillar::Security, now());
        let codes = reason_codes(&report);

        assert!(codes.contains(&REASON_SEC_OWNER_NOT_RECORDED));
        assert!(codes.contains(&REASON_SEC_BACKUP_CHAIN_NOT_AUDITABLE));
    }

    #[test]
    fn stale_inventory_is_counted_for_any_pillar() {
        let target = item(
            Some("db-team"),
            Some("ON"),
            Some(7 * 86_400),
            Some("ON"),
            48,
        );

        let report = evaluate_mysql_backup_posture_inventory(&[target], Pillar::Cost, now());

        assert_eq!(report.stale_resources, 1);
        assert!(reason_codes(&report).contains(&REASON_INV_STALE_DATA));
    }

    #[test]
    fn healthy_backup_posture_passes_claimed_pillars() {
        let mut target = item(
            Some("db-team"),
            Some("ON"),
            Some(14 * 86_400),
            Some("ON"),
            1,
        );
        target
            .labels
            .insert("cost-center".to_string(), "cc-42".to_string());

        for pillar in [Pillar::Cost, Pillar::Resilience, Pillar::Security] {
            let report = evaluate_mysql_backup_posture_inventory(
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

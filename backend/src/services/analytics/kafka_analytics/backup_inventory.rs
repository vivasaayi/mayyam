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

// Deterministic Kafka backup inventory evaluator for roadmap rows
// 04-KAFKA-DASHBOARD-MANAGEMENT-01324/01331/01352.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::config::KafkaClusterConfig;
use crate::services::aws::inventory::types::{
    score_pillar, InventoryFinding, Pillar, PillarReport, Severity, COST_ALLOCATION_TAG_KEYS,
    DEFAULT_STALE_AFTER_HOURS,
};

pub const RESOURCE_TYPE: &str = "KafkaBackup";
pub const REASON_COST_OWNER_NOT_RECORDED: &str = "KAFKA_BACKUP_COST_OWNER_NOT_RECORDED";
pub const REASON_COST_NO_EVIDENCE: &str = "KAFKA_BACKUP_COST_NO_EVIDENCE";
pub const REASON_COST_LONG_RETENTION: &str = "KAFKA_BACKUP_COST_LONG_RETENTION";
pub const REASON_RES_NO_EVIDENCE: &str = "KAFKA_BACKUP_RES_NO_EVIDENCE";
pub const REASON_RES_NO_RESTORE_TEST: &str = "KAFKA_BACKUP_RES_NO_RESTORE_TEST";
pub const REASON_RES_STALE_LAST_SUCCESS: &str = "KAFKA_BACKUP_RES_STALE_LAST_SUCCESS";
pub const REASON_SEC_NO_EVIDENCE: &str = "KAFKA_BACKUP_SEC_NO_EVIDENCE";
pub const REASON_SEC_NO_ENCRYPTION_EVIDENCE: &str = "KAFKA_BACKUP_SEC_NO_ENCRYPTION_EVIDENCE";
pub const REASON_SEC_NO_IMMUTABILITY_EVIDENCE: &str = "KAFKA_BACKUP_SEC_NO_IMMUTABILITY_EVIDENCE";
pub const REASON_SEC_PLAINTEXT_CONNECTION: &str = "KAFKA_BACKUP_SEC_PLAINTEXT_CONNECTION";
pub const REASON_INV_STALE_DATA: &str = "KAFKA_BACKUP_INV_STALE_DATA";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupInventoryItem {
    pub backup_id: String,
    pub cluster_name: String,
    pub backup_target: Option<String>,
    pub backup_frequency_minutes: Option<u32>,
    pub retention_days: Option<u32>,
    pub last_success_at: Option<DateTime<Utc>>,
    pub owner: Option<String>,
    pub labels: BTreeMap<String, String>,
    pub restore_test_evidence: bool,
    pub encryption_evidence: bool,
    pub immutable_backup_evidence: bool,
    pub plaintext_connection: bool,
    pub has_backup_evidence: bool,
    pub collected_at: DateTime<Utc>,
}

pub fn evaluate_backup_inventory(
    items: &[BackupInventoryItem],
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
            Pillar::Resilience => evaluate_resilience(item, pillar, now, &mut findings),
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

pub fn backup_inventory_item_from_config(
    cluster: &KafkaClusterConfig,
    collected_at: DateTime<Utc>,
) -> BackupInventoryItem {
    BackupInventoryItem {
        backup_id: format!("{}:backup", cluster.name),
        cluster_name: cluster.name.clone(),
        backup_target: None,
        backup_frequency_minutes: None,
        retention_days: None,
        last_success_at: None,
        owner: None,
        labels: BTreeMap::new(),
        restore_test_evidence: false,
        encryption_evidence: false,
        immutable_backup_evidence: false,
        plaintext_connection: cluster.security_protocol.eq_ignore_ascii_case("PLAINTEXT"),
        has_backup_evidence: false,
        collected_at,
    }
}

fn evaluate_cost(item: &BackupInventoryItem, pillar: Pillar, findings: &mut Vec<InventoryFinding>) {
    if !has_owner_metadata(item) {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_OWNER_NOT_RECORDED,
            Severity::Medium,
            format!(
                "Kafka backup inventory for cluster {} has no owner, team, project, or cost-center metadata",
                item.cluster_name
            ),
            json!({
                "backup_id": item.backup_id,
                "cluster_name": item.cluster_name,
                "checked_keys": COST_ALLOCATION_TAG_KEYS,
            }),
        ));
    }

    if !item.has_backup_evidence {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_NO_EVIDENCE,
            Severity::High,
            format!(
                "Kafka backup inventory for cluster {} has no backup evidence",
                item.cluster_name
            ),
            json!({
                "backup_id": item.backup_id,
                "cluster_name": item.cluster_name,
                "recommendation": "Collect backup target, frequency, retention, ownership, restore test, encryption, and last-success evidence before estimating backup cost posture",
            }),
        ));
    }

    if item
        .retention_days
        .is_some_and(|retention_days| retention_days > 365)
    {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_LONG_RETENTION,
            Severity::Medium,
            format!(
                "Kafka backup {} keeps backups longer than one year",
                item.backup_id
            ),
            json!({
                "backup_id": item.backup_id,
                "retention_days": item.retention_days,
                "backup_target": item.backup_target,
                "recommendation": "Review retention against recovery and compliance requirements before carrying long-term backup cost",
            }),
        ));
    }
}

fn evaluate_resilience(
    item: &BackupInventoryItem,
    pillar: Pillar,
    now: DateTime<Utc>,
    findings: &mut Vec<InventoryFinding>,
) {
    if !item.has_backup_evidence {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_NO_EVIDENCE,
            Severity::High,
            format!(
                "Kafka backup inventory for cluster {} has no resilience evidence",
                item.cluster_name
            ),
            json!({
                "backup_id": item.backup_id,
                "cluster_name": item.cluster_name,
                "recommendation": "Collect backup schedules, last successful run, restore test, retention, and target durability evidence before accepting recovery posture",
            }),
        ));
    }

    if item.has_backup_evidence && !item.restore_test_evidence {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_NO_RESTORE_TEST,
            Severity::High,
            format!("Kafka backup {} has no restore test evidence", item.backup_id),
            json!({
                "backup_id": item.backup_id,
                "backup_target": item.backup_target,
                "recommendation": "Run and record restore verification before accepting Kafka backup resilience posture",
            }),
        ));
    }

    if let Some(last_success_at) = item.last_success_at {
        let age_hours = now
            .signed_duration_since(last_success_at)
            .num_hours()
            .max(0);
        if age_hours > 24 {
            findings.push(finding(
                item,
                pillar,
                REASON_RES_STALE_LAST_SUCCESS,
                Severity::High,
                format!(
                    "Kafka backup {} last succeeded {} hour(s) ago",
                    item.backup_id, age_hours
                ),
                json!({
                    "backup_id": item.backup_id,
                    "last_success_at": last_success_at,
                    "age_hours": age_hours,
                    "recommendation": "Investigate backup freshness before accepting recovery posture",
                }),
            ));
        }
    }
}

fn evaluate_security(
    item: &BackupInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if !item.has_backup_evidence {
        findings.push(finding(
            item,
            pillar,
            REASON_SEC_NO_EVIDENCE,
            Severity::High,
            format!(
                "Kafka backup inventory for cluster {} has no security evidence",
                item.cluster_name
            ),
            json!({
                "backup_id": item.backup_id,
                "cluster_name": item.cluster_name,
                "recommendation": "Collect backup identity, target policy, encryption, immutability, access scope, and audit evidence before accepting security posture",
            }),
        ));
    }

    if item.has_backup_evidence && !item.encryption_evidence {
        findings.push(finding(
            item,
            pillar,
            REASON_SEC_NO_ENCRYPTION_EVIDENCE,
            Severity::High,
            format!("Kafka backup {} has no encryption evidence", item.backup_id),
            json!({
                "backup_id": item.backup_id,
                "backup_target": item.backup_target,
                "recommendation": "Record backup encryption and key policy evidence before accepting security posture",
            }),
        ));
    }

    if item.has_backup_evidence && !item.immutable_backup_evidence {
        findings.push(finding(
            item,
            pillar,
            REASON_SEC_NO_IMMUTABILITY_EVIDENCE,
            Severity::Medium,
            format!(
                "Kafka backup {} has no immutability evidence",
                item.backup_id
            ),
            json!({
                "backup_id": item.backup_id,
                "backup_target": item.backup_target,
                "recommendation": "Record object lock, retention lock, or equivalent immutability evidence for critical Kafka backups",
            }),
        ));
    }

    if item.plaintext_connection {
        findings.push(finding(
            item,
            pillar,
            REASON_SEC_PLAINTEXT_CONNECTION,
            Severity::High,
            format!(
                "Kafka backup {} uses PLAINTEXT or unverified transport",
                item.backup_id
            ),
            json!({
                "backup_id": item.backup_id,
                "plaintext_connection": item.plaintext_connection,
                "recommendation": "Use encrypted Kafka and backup target transport before accepting security posture",
            }),
        ));
    }
}

fn stale_finding(
    item: &BackupInventoryItem,
    pillar: Pillar,
    now: DateTime<Utc>,
) -> Option<InventoryFinding> {
    let age_hours = now
        .signed_duration_since(item.collected_at)
        .num_hours()
        .max(0);

    if age_hours <= DEFAULT_STALE_AFTER_HOURS {
        return None;
    }

    Some(finding(
        item,
        pillar,
        REASON_INV_STALE_DATA,
        Severity::High,
        format!(
            "Kafka backup inventory for cluster {} is {} hour(s) old",
            item.cluster_name, age_hours
        ),
        json!({
            "backup_id": item.backup_id,
            "cluster_name": item.cluster_name,
            "age_hours": age_hours,
            "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
            "recommendation": "Refresh Kafka backup inventory before acting on this posture report",
        }),
    ))
}

fn has_owner_metadata(item: &BackupInventoryItem) -> bool {
    item.owner
        .as_deref()
        .is_some_and(|owner| !owner.trim().is_empty())
        || COST_ALLOCATION_TAG_KEYS.iter().any(|key| {
            item.labels
                .get(*key)
                .or_else(|| item.labels.get(&key.to_ascii_lowercase()))
                .is_some_and(|value| !value.trim().is_empty())
        })
}

fn finding(
    item: &BackupInventoryItem,
    pillar: Pillar,
    reason_code: &'static str,
    severity: Severity,
    message: String,
    evidence: Value,
) -> InventoryFinding {
    InventoryFinding {
        resource_id: item.backup_id.clone(),
        arn: format!("kafka:backup/{}", item.backup_id),
        pillar,
        severity,
        reason_code: reason_code.to_string(),
        message,
        evidence,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    fn backup(now: DateTime<Utc>) -> BackupInventoryItem {
        BackupInventoryItem {
            backup_id: "prod:backup".to_string(),
            cluster_name: "prod".to_string(),
            backup_target: Some("s3://prod-kafka-backups".to_string()),
            backup_frequency_minutes: Some(60),
            retention_days: Some(30),
            last_success_at: Some(now - Duration::hours(2)),
            owner: Some("platform".to_string()),
            labels: BTreeMap::new(),
            restore_test_evidence: true,
            encryption_evidence: true,
            immutable_backup_evidence: true,
            plaintext_connection: false,
            has_backup_evidence: true,
            collected_at: now,
        }
    }

    #[test]
    fn healthy_backup_passes_claimed_pillars() {
        let now = Utc::now();
        let item = backup(now);

        for pillar in [Pillar::Cost, Pillar::Resilience, Pillar::Security] {
            let report = evaluate_backup_inventory(std::slice::from_ref(&item), pillar, now);
            assert_eq!(report.resources_evaluated, 1);
            assert!(report.findings.is_empty());
            assert_eq!(report.stale_resources, 0);
            assert_eq!(report.score, 100);
        }
    }

    #[test]
    fn cost_flags_missing_owner_missing_evidence_and_long_retention() {
        let now = Utc::now();
        let mut missing_evidence = backup(now);
        missing_evidence.owner = None;
        missing_evidence.has_backup_evidence = false;
        let mut long_retention = backup(now);
        long_retention.retention_days = Some(366);

        let report =
            evaluate_backup_inventory(&[missing_evidence, long_retention], Pillar::Cost, now);

        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_COST_OWNER_NOT_RECORDED));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_COST_NO_EVIDENCE));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_COST_LONG_RETENTION));
    }

    #[test]
    fn resilience_flags_missing_evidence_restore_test_and_stale_success() {
        let now = Utc::now();
        let mut missing_evidence = backup(now);
        missing_evidence.has_backup_evidence = false;
        let mut risky = backup(now);
        risky.restore_test_evidence = false;
        risky.last_success_at = Some(now - Duration::hours(25));

        let report = evaluate_backup_inventory(&[missing_evidence, risky], Pillar::Resilience, now);

        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_RES_NO_EVIDENCE));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_RES_NO_RESTORE_TEST));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_RES_STALE_LAST_SUCCESS));
    }

    #[test]
    fn security_flags_missing_evidence_encryption_immutability_and_plaintext() {
        let now = Utc::now();
        let mut missing_evidence = backup(now);
        missing_evidence.has_backup_evidence = false;
        let mut insecure = backup(now);
        insecure.encryption_evidence = false;
        insecure.immutable_backup_evidence = false;
        insecure.plaintext_connection = true;

        let report =
            evaluate_backup_inventory(&[missing_evidence, insecure], Pillar::Security, now);

        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_SEC_NO_EVIDENCE));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_SEC_NO_ENCRYPTION_EVIDENCE));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_SEC_NO_IMMUTABILITY_EVIDENCE));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_SEC_PLAINTEXT_CONNECTION));
    }

    #[test]
    fn stale_backup_inventory_is_counted_for_any_pillar() {
        let now = Utc::now();
        let mut item = backup(now);
        item.collected_at = now - Duration::hours(DEFAULT_STALE_AFTER_HOURS + 2);

        let report = evaluate_backup_inventory(&[item], Pillar::Security, now);

        assert_eq!(report.stale_resources, 1);
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_INV_STALE_DATA));
    }
}

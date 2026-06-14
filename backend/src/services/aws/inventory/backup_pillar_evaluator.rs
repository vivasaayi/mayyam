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

// Deterministic AWS Backup vault inventory evaluators for the cost,
// security, and resilience pillars.
//
// Evaluates fields persisted by backup_control_plane:
// number_of_recovery_points, locked, min_retention_days,
// max_retention_days, encryption_key_type, plus the tags column.
// Vault lock state (`locked`) and `encryption_key_type` are optional in
// the AWS response, so their absence is surfaced as a data-gap finding
// rather than a hard pass or fail.

use chrono::{DateTime, Utc};
use serde_json::json;

use crate::models::aws_resource::Model as AwsResourceModel;
use crate::services::aws::inventory::types::{
    check_stale, data_str, score_pillar, InventoryFinding, Pillar, PillarReport, Severity,
};

/// Only rows of this resource type are evaluated.
pub const RESOURCE_TYPE: &str = "BackupVault";

// Reason codes are the stable contract for findings; never reuse or rename.
pub const REASON_COST_NO_TAGS: &str = "BACKUP_COST_NO_TAGS";
pub const REASON_COST_EMPTY_VAULT: &str = "BACKUP_COST_EMPTY_VAULT";
pub const REASON_COST_UNBOUNDED_LOCKED_RETENTION: &str = "BACKUP_COST_UNBOUNDED_LOCKED_RETENTION";
pub const REASON_RES_NO_RECOVERY_POINTS: &str = "BACKUP_RES_NO_RECOVERY_POINTS";
pub const REASON_RES_RECOVERY_POINT_DATA_NOT_COLLECTED: &str =
    "BACKUP_RES_RECOVERY_POINT_DATA_NOT_COLLECTED";
pub const REASON_RES_LOCKED_NO_MIN_RETENTION: &str = "BACKUP_RES_LOCKED_NO_MIN_RETENTION";
pub const REASON_SEC_VAULT_NOT_LOCKED: &str = "BACKUP_SEC_VAULT_NOT_LOCKED";
pub const REASON_SEC_LOCK_DATA_NOT_COLLECTED: &str = "BACKUP_SEC_LOCK_DATA_NOT_COLLECTED";
pub const REASON_SEC_DEFAULT_ENCRYPTION_KEY: &str = "BACKUP_SEC_DEFAULT_ENCRYPTION_KEY";
pub const REASON_SEC_ENCRYPTION_DATA_NOT_COLLECTED: &str =
    "BACKUP_SEC_ENCRYPTION_DATA_NOT_COLLECTED";
pub const REASON_INV_STALE_DATA: &str = "BACKUP_INV_STALE_DATA";

/// Evaluate every Backup vault in the fleet for one pillar. Rows whose
/// `resource_type` is not `BackupVault` are skipped and not counted.
pub fn evaluate_backup_fleet(
    resources: &[AwsResourceModel],
    pillar: Pillar,
    now: DateTime<Utc>,
) -> PillarReport {
    let mut findings: Vec<InventoryFinding> = Vec::new();
    let mut stale_resources = 0usize;
    let mut evaluated = 0usize;

    for resource in resources {
        if resource.resource_type != RESOURCE_TYPE {
            continue;
        }
        evaluated += 1;

        if let Some(stale) = check_stale(resource, pillar, REASON_INV_STALE_DATA, now) {
            stale_resources += 1;
            findings.push(stale);
        }
        match pillar {
            Pillar::Cost => evaluate_cost(resource, &mut findings),
            Pillar::Security => evaluate_security(resource, &mut findings),
            Pillar::Resilience => evaluate_resilience(resource, &mut findings),
            // Pillars without checks for this service yet produce no findings.
            _ => {}
        }
    }

    let score = score_pillar(&findings);
    PillarReport {
        pillar,
        resources_evaluated: evaluated,
        stale_resources,
        score,
        findings,
    }
}

fn recovery_points(resource: &AwsResourceModel) -> Option<i64> {
    resource
        .resource_data
        .get("number_of_recovery_points")
        .and_then(|v| v.as_i64())
}

fn locked(resource: &AwsResourceModel) -> Option<bool> {
    resource
        .resource_data
        .get("locked")
        .and_then(|v| v.as_bool())
}

fn retention_days(resource: &AwsResourceModel, key: &str) -> Option<i64> {
    resource.resource_data.get(key).and_then(|v| v.as_i64())
}

fn evaluate_cost(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    let tags_empty = resource
        .tags
        .as_object()
        .map(|m| m.is_empty())
        .unwrap_or(true);
    if tags_empty {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Cost,
            reason_code: REASON_COST_NO_TAGS.to_string(),
            severity: Severity::Medium,
            message: format!(
                "Backup vault {} has no tags recorded (untagged resource or tag collection gap); backup storage spend cannot be allocated",
                resource.resource_id
            ),
            evidence: json!({ "tags": resource.tags }),
        });
    }

    // An empty vault costs nothing for storage but signals abandoned backup
    // configuration that should be cleaned up or fixed. The recovery-point
    // data gap is reported by the resilience pillar, so the cost check only
    // fires on a collected zero.
    if recovery_points(resource) == Some(0) {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Cost,
            reason_code: REASON_COST_EMPTY_VAULT.to_string(),
            severity: Severity::Low,
            message: format!(
                "Backup vault {} holds zero recovery points; remove the vault or its plan configuration if it is no longer used",
                resource.resource_id
            ),
            evidence: json!({ "number_of_recovery_points": 0 }),
        });
    }

    // A locked vault without a maximum retention ceiling keeps recovery
    // points immutable forever, so storage spend grows without bound.
    if locked(resource) == Some(true) && retention_days(resource, "max_retention_days").is_none() {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Cost,
            reason_code: REASON_COST_UNBOUNDED_LOCKED_RETENTION.to_string(),
            severity: Severity::Low,
            message: format!(
                "Locked Backup vault {} has no max_retention_days; immutable recovery points accumulate storage cost indefinitely",
                resource.resource_id
            ),
            evidence: json!({ "locked": true, "max_retention_days": null }),
        });
    }
}

fn evaluate_resilience(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    match recovery_points(resource) {
        Some(0) => {
            findings.push(InventoryFinding {
                resource_id: resource.resource_id.clone(),
                arn: resource.arn.clone(),
                pillar: Pillar::Resilience,
                reason_code: REASON_RES_NO_RECOVERY_POINTS.to_string(),
                severity: Severity::High,
                message: format!(
                    "Backup vault {} holds zero recovery points; no restorable backups exist in this vault",
                    resource.resource_id
                ),
                evidence: json!({ "number_of_recovery_points": 0 }),
            });
        }
        None => {
            findings.push(InventoryFinding {
                resource_id: resource.resource_id.clone(),
                arn: resource.arn.clone(),
                pillar: Pillar::Resilience,
                reason_code: REASON_RES_RECOVERY_POINT_DATA_NOT_COLLECTED.to_string(),
                severity: Severity::Low,
                message: format!(
                    "Recovery point count for Backup vault {} is not collected yet; resilience pillar cannot be fully assessed",
                    resource.resource_id
                ),
                evidence: json!({ "number_of_recovery_points_collected": false }),
            });
        }
        Some(_) => {}
    }

    // min_retention_days only exists on locked vaults. A lock without a
    // retention floor does not actually protect recovery points from early
    // deletion, which defeats the resilience purpose of the lock.
    if locked(resource) == Some(true) && retention_days(resource, "min_retention_days").is_none() {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Resilience,
            reason_code: REASON_RES_LOCKED_NO_MIN_RETENTION.to_string(),
            severity: Severity::Medium,
            message: format!(
                "Locked Backup vault {} has no min_retention_days; recovery points are not protected by a minimum retention floor",
                resource.resource_id
            ),
            evidence: json!({ "locked": true, "min_retention_days": null }),
        });
    }
}

fn evaluate_security(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    match locked(resource) {
        Some(false) => {
            findings.push(InventoryFinding {
                resource_id: resource.resource_id.clone(),
                arn: resource.arn.clone(),
                pillar: Pillar::Security,
                reason_code: REASON_SEC_VAULT_NOT_LOCKED.to_string(),
                severity: Severity::Medium,
                message: format!(
                    "Backup vault {} has no vault lock; recovery points are mutable and not protected against ransomware or accidental deletion",
                    resource.resource_id
                ),
                evidence: json!({ "locked": false }),
            });
        }
        None => {
            findings.push(InventoryFinding {
                resource_id: resource.resource_id.clone(),
                arn: resource.arn.clone(),
                pillar: Pillar::Security,
                reason_code: REASON_SEC_LOCK_DATA_NOT_COLLECTED.to_string(),
                severity: Severity::Low,
                message: format!(
                    "Vault lock state for Backup vault {} is not collected yet; security pillar cannot be fully assessed",
                    resource.resource_id
                ),
                evidence: json!({ "locked_collected": false }),
            });
        }
        Some(true) => {}
    }

    match data_str(&resource.resource_data, "encryption_key_type").as_deref() {
        Some("AWS_OWNED_KMS_KEY") => {
            findings.push(InventoryFinding {
                resource_id: resource.resource_id.clone(),
                arn: resource.arn.clone(),
                pillar: Pillar::Security,
                reason_code: REASON_SEC_DEFAULT_ENCRYPTION_KEY.to_string(),
                severity: Severity::Medium,
                message: format!(
                    "Backup vault {} is encrypted with an AWS-owned KMS key; use a customer-managed key to control access and auditing",
                    resource.resource_id
                ),
                evidence: json!({
                    "encryption_key_type": "AWS_OWNED_KMS_KEY",
                    "encryption_key_arn": resource.resource_data.get("encryption_key_arn"),
                }),
            });
        }
        Some(_) => {}
        None => {
            findings.push(InventoryFinding {
                resource_id: resource.resource_id.clone(),
                arn: resource.arn.clone(),
                pillar: Pillar::Security,
                reason_code: REASON_SEC_ENCRYPTION_DATA_NOT_COLLECTED.to_string(),
                severity: Severity::Low,
                message: format!(
                    "Encryption key type for Backup vault {} is not collected yet; security pillar cannot be fully assessed",
                    resource.resource_id
                ),
                evidence: json!({ "encryption_key_type_collected": false }),
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;
    use serde_json::Value;
    use uuid::Uuid;

    fn fixture(
        resource_id: &str,
        tags: Value,
        resource_data: Value,
        now: DateTime<Utc>,
    ) -> AwsResourceModel {
        let refreshed = now - Duration::hours(1);
        AwsResourceModel {
            id: Uuid::new_v4(),
            sync_id: None,
            account_id: "123456789012".to_string(),
            profile: None,
            region: "us-east-1".to_string(),
            resource_type: RESOURCE_TYPE.to_string(),
            resource_id: resource_id.to_string(),
            arn: format!(
                "arn:aws:backup:us-east-1:123456789012:backup-vault:{}",
                resource_id
            ),
            name: Some(resource_id.to_string()),
            tags,
            resource_data,
            created_at: refreshed,
            updated_at: refreshed,
            last_refreshed: refreshed,
        }
    }

    fn now() -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2026-06-10T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc)
    }

    fn healthy_vault_data() -> Value {
        json!({
            "backup_vault_name": "vault-ok",
            "backup_vault_arn": "arn:aws:backup:us-east-1:123456789012:backup-vault:vault-ok",
            "vault_type": "BACKUP_VAULT",
            "vault_state": "AVAILABLE",
            "creation_date": "2025-01-01T00:00:00Z",
            "encryption_key_arn": "arn:aws:kms:us-east-1:123456789012:key/abc",
            "encryption_key_type": "CUSTOMER_MANAGED_KMS_KEY",
            "number_of_recovery_points": 12,
            "locked": true,
            "min_retention_days": 7,
            "max_retention_days": 365,
            "lock_date": "2025-02-01T00:00:00Z",
        })
    }

    fn codes(report: &PillarReport) -> Vec<&str> {
        report
            .findings
            .iter()
            .map(|f| f.reason_code.as_str())
            .collect()
    }

    #[test]
    fn healthy_vault_passes_all_pillars() {
        let r = fixture(
            "vault-ok",
            json!({"team": "sre"}),
            healthy_vault_data(),
            now(),
        );
        for pillar in [Pillar::Cost, Pillar::Security, Pillar::Resilience] {
            let report = evaluate_backup_fleet(std::slice::from_ref(&r), pillar, now());
            assert!(
                report.findings.is_empty(),
                "unexpected for {:?}: {:?}",
                pillar,
                report.findings
            );
            assert_eq!(report.score, 100);
        }
    }

    #[test]
    fn cost_flags_untagged_vault() {
        let r = fixture("vault-untagged", json!({}), healthy_vault_data(), now());
        let report = evaluate_backup_fleet(&[r], Pillar::Cost, now());
        assert_eq!(codes(&report), vec![REASON_COST_NO_TAGS]);
    }

    #[test]
    fn cost_flags_empty_vault() {
        let mut data = healthy_vault_data();
        data["number_of_recovery_points"] = json!(0);
        let r = fixture("vault-empty", json!({"team": "sre"}), data, now());
        let report = evaluate_backup_fleet(&[r], Pillar::Cost, now());
        assert_eq!(codes(&report), vec![REASON_COST_EMPTY_VAULT]);
    }

    #[test]
    fn cost_skips_empty_vault_check_when_recovery_points_not_collected() {
        let mut data = healthy_vault_data();
        data.as_object_mut()
            .unwrap()
            .remove("number_of_recovery_points");
        let r = fixture("vault-gap", json!({"team": "sre"}), data, now());
        let report = evaluate_backup_fleet(&[r], Pillar::Cost, now());
        assert!(
            report.findings.is_empty(),
            "unexpected: {:?}",
            report.findings
        );
    }

    #[test]
    fn cost_flags_locked_vault_without_max_retention() {
        let mut data = healthy_vault_data();
        data.as_object_mut().unwrap().remove("max_retention_days");
        let r = fixture("vault-forever", json!({"team": "sre"}), data, now());
        let report = evaluate_backup_fleet(&[r], Pillar::Cost, now());
        assert_eq!(codes(&report), vec![REASON_COST_UNBOUNDED_LOCKED_RETENTION]);
    }

    #[test]
    fn cost_does_not_flag_unlocked_vault_for_unbounded_retention() {
        let mut data = healthy_vault_data();
        data["locked"] = json!(false);
        data.as_object_mut().unwrap().remove("max_retention_days");
        data.as_object_mut().unwrap().remove("min_retention_days");
        let r = fixture("vault-unlocked", json!({"team": "sre"}), data, now());
        let report = evaluate_backup_fleet(&[r], Pillar::Cost, now());
        assert!(
            report.findings.is_empty(),
            "unexpected: {:?}",
            report.findings
        );
    }

    #[test]
    fn resilience_flags_vault_with_zero_recovery_points_as_high() {
        let mut data = healthy_vault_data();
        data["number_of_recovery_points"] = json!(0);
        let r = fixture("vault-empty", json!({"team": "sre"}), data, now());
        let report = evaluate_backup_fleet(&[r], Pillar::Resilience, now());
        assert_eq!(codes(&report), vec![REASON_RES_NO_RECOVERY_POINTS]);
        assert!(matches!(report.findings[0].severity, Severity::High));
    }

    #[test]
    fn resilience_reports_gap_when_recovery_points_not_collected() {
        let mut data = healthy_vault_data();
        data.as_object_mut()
            .unwrap()
            .remove("number_of_recovery_points");
        let r = fixture("vault-gap", json!({"team": "sre"}), data, now());
        let report = evaluate_backup_fleet(&[r], Pillar::Resilience, now());
        assert_eq!(
            codes(&report),
            vec![REASON_RES_RECOVERY_POINT_DATA_NOT_COLLECTED]
        );
    }

    #[test]
    fn resilience_flags_locked_vault_without_min_retention() {
        let mut data = healthy_vault_data();
        data.as_object_mut().unwrap().remove("min_retention_days");
        let r = fixture("vault-nofloor", json!({"team": "sre"}), data, now());
        let report = evaluate_backup_fleet(&[r], Pillar::Resilience, now());
        assert_eq!(codes(&report), vec![REASON_RES_LOCKED_NO_MIN_RETENTION]);
    }

    #[test]
    fn resilience_does_not_flag_unlocked_vault_for_min_retention() {
        let mut data = healthy_vault_data();
        data["locked"] = json!(false);
        data.as_object_mut().unwrap().remove("min_retention_days");
        data.as_object_mut().unwrap().remove("max_retention_days");
        let r = fixture("vault-unlocked", json!({"team": "sre"}), data, now());
        let report = evaluate_backup_fleet(&[r], Pillar::Resilience, now());
        assert!(
            report.findings.is_empty(),
            "unexpected: {:?}",
            report.findings
        );
    }

    #[test]
    fn security_flags_unlocked_vault() {
        let mut data = healthy_vault_data();
        data["locked"] = json!(false);
        data.as_object_mut().unwrap().remove("min_retention_days");
        data.as_object_mut().unwrap().remove("max_retention_days");
        data.as_object_mut().unwrap().remove("lock_date");
        let r = fixture("vault-unlocked", json!({"team": "sre"}), data, now());
        let report = evaluate_backup_fleet(&[r], Pillar::Security, now());
        assert_eq!(codes(&report), vec![REASON_SEC_VAULT_NOT_LOCKED]);
    }

    #[test]
    fn security_reports_gap_when_lock_state_not_collected() {
        let mut data = healthy_vault_data();
        data.as_object_mut().unwrap().remove("locked");
        let r = fixture("vault-lockgap", json!({"team": "sre"}), data, now());
        let report = evaluate_backup_fleet(&[r], Pillar::Security, now());
        assert_eq!(codes(&report), vec![REASON_SEC_LOCK_DATA_NOT_COLLECTED]);
    }

    #[test]
    fn security_flags_aws_owned_encryption_key() {
        let mut data = healthy_vault_data();
        data["encryption_key_type"] = json!("AWS_OWNED_KMS_KEY");
        data.as_object_mut().unwrap().remove("encryption_key_arn");
        let r = fixture("vault-awskey", json!({"team": "sre"}), data, now());
        let report = evaluate_backup_fleet(&[r], Pillar::Security, now());
        assert_eq!(codes(&report), vec![REASON_SEC_DEFAULT_ENCRYPTION_KEY]);
    }

    #[test]
    fn security_reports_gap_when_encryption_key_type_not_collected() {
        let mut data = healthy_vault_data();
        data.as_object_mut().unwrap().remove("encryption_key_type");
        let r = fixture("vault-enckeygap", json!({"team": "sre"}), data, now());
        let report = evaluate_backup_fleet(&[r], Pillar::Security, now());
        assert_eq!(
            codes(&report),
            vec![REASON_SEC_ENCRYPTION_DATA_NOT_COLLECTED]
        );
    }

    #[test]
    fn stale_inventory_is_flagged() {
        let mut r = fixture(
            "vault-stale",
            json!({"team": "sre"}),
            healthy_vault_data(),
            now(),
        );
        r.last_refreshed = now() - Duration::hours(48);
        let report = evaluate_backup_fleet(&[r], Pillar::Resilience, now());
        assert_eq!(report.stale_resources, 1);
        assert!(codes(&report).contains(&REASON_INV_STALE_DATA));
    }

    #[test]
    fn non_backup_resources_are_skipped_and_not_counted() {
        let mut r = fixture("queue-1", json!({}), json!({}), now());
        r.resource_type = "SqsQueue".to_string();
        let report = evaluate_backup_fleet(&[r], Pillar::Cost, now());
        assert_eq!(report.resources_evaluated, 0);
        assert!(report.findings.is_empty());
    }
}

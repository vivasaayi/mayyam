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

// Deterministic Secrets Manager secret inventory evaluators for the cost,
// security, and resilience pillars (roadmap rows
// 01-AWS-CLOUD-04285/04294/04321).
//
// Evaluates fields persisted by secretsmanager_control_plane: kms_key_id,
// rotation_enabled, rotation_lambda_arn, automatically_after_days,
// last_rotated_date, last_accessed_date, owning_service, primary_region,
// plus the tags column.

use chrono::{DateTime, Duration, Utc};
use serde_json::{json, Value};

use crate::models::aws_resource::Model as AwsResourceModel;
use crate::services::aws::inventory::types::{
    check_stale, data_str, score_pillar, InventoryFinding, Pillar, PillarReport, Severity,
};

/// Only rows of this resource type are evaluated.
pub const RESOURCE_TYPE: &str = "SecretsManagerSecret";

// Reason codes are the stable contract for findings; never reuse or rename.
pub const REASON_COST_NO_TAGS: &str = "SECRETS_COST_NO_TAGS";
pub const REASON_COST_UNUSED_90D: &str = "SECRETS_COST_UNUSED_90D";
pub const REASON_SEC_ROTATION_DISABLED: &str = "SECRETS_SEC_ROTATION_DISABLED";
pub const REASON_SEC_ROTATION_OVERDUE: &str = "SECRETS_SEC_ROTATION_OVERDUE";
pub const REASON_SEC_DEFAULT_KMS_KEY: &str = "SECRETS_SEC_DEFAULT_KMS_KEY";
pub const REASON_RES_NO_REPLICATION: &str = "SECRETS_RES_NO_REPLICATION";
pub const REASON_INV_STALE_DATA: &str = "SECRETS_INV_STALE_DATA";

/// Days without a recorded access after which a secret is considered unused.
const UNUSED_AFTER_DAYS: i64 = 90;

/// Evaluate every Secrets Manager secret in the fleet for one pillar. Rows
/// whose `resource_type` is not `SecretsManagerSecret` are skipped and not
/// counted.
pub fn evaluate_secretsmanager_fleet(
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
            Pillar::Cost => evaluate_cost(resource, now, &mut findings),
            Pillar::Security => evaluate_security(resource, now, &mut findings),
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

fn data_i64(resource_data: &Value, key: &str) -> Option<i64> {
    resource_data.get(key).and_then(|v| v.as_i64())
}

fn data_bool(resource_data: &Value, key: &str) -> Option<bool> {
    resource_data.get(key).and_then(|v| v.as_bool())
}

/// Parse a stored timestamp; the collector persists aws_smithy DateTime
/// strings ("2026-01-01T00:00:00Z") which are RFC3339-compatible. Unparseable
/// values fall back to `None` rather than panicking.
fn data_datetime(resource_data: &Value, key: &str) -> Option<DateTime<Utc>> {
    data_str(resource_data, key)
        .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
        .map(|dt| dt.with_timezone(&Utc))
}

fn evaluate_cost(
    resource: &AwsResourceModel,
    now: DateTime<Utc>,
    findings: &mut Vec<InventoryFinding>,
) {
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
                "Secret {} has no tags recorded (untagged resource or tag collection gap); cost allocation cannot be assessed",
                resource.resource_id
            ),
            evidence: json!({ "tags": resource.tags }),
        });
    }

    // A secret that nothing has read for 90+ days is still billed monthly.
    // Absent last_accessed_date is not flagged: Secrets Manager tracks access
    // at day granularity and absence may be a collection gap.
    if let Some(last_accessed) = data_datetime(&resource.resource_data, "last_accessed_date") {
        let unused_days = (now - last_accessed).num_days();
        if unused_days > UNUSED_AFTER_DAYS {
            findings.push(InventoryFinding {
                resource_id: resource.resource_id.clone(),
                arn: resource.arn.clone(),
                pillar: Pillar::Cost,
                reason_code: REASON_COST_UNUSED_90D.to_string(),
                severity: Severity::Low,
                message: format!(
                    "Secret {} was last accessed {} days ago (threshold {} days); it is billed monthly but nothing reads it",
                    resource.resource_id, unused_days, UNUSED_AFTER_DAYS
                ),
                evidence: json!({
                    "last_accessed_date":
                        data_str(&resource.resource_data, "last_accessed_date"),
                    "unused_days": unused_days,
                    "threshold_days": UNUSED_AFTER_DAYS,
                }),
            });
        }
    }
}

fn evaluate_security(
    resource: &AwsResourceModel,
    now: DateTime<Utc>,
    findings: &mut Vec<InventoryFinding>,
) {
    let rotation_enabled = data_bool(&resource.resource_data, "rotation_enabled").unwrap_or(false);
    let owning_service = data_str(&resource.resource_data, "owning_service");

    // Service-owned secrets (e.g. RDS-managed) rotate through their owning
    // service and are not flagged here.
    if !rotation_enabled && owning_service.is_none() {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Security,
            reason_code: REASON_SEC_ROTATION_DISABLED.to_string(),
            severity: Severity::Medium,
            message: format!(
                "Secret {} has automatic rotation disabled and is not service-owned; long-lived credentials increase blast radius if leaked",
                resource.resource_id
            ),
            evidence: json!({
                "rotation_enabled":
                    data_bool(&resource.resource_data, "rotation_enabled"),
                "owning_service": Value::Null,
            }),
        });
    }

    // Rotation configured but not actually happening: last rotation is older
    // than twice the configured rotation window.
    if rotation_enabled {
        let after_days = data_i64(&resource.resource_data, "automatically_after_days");
        let last_rotated = data_datetime(&resource.resource_data, "last_rotated_date");
        if let (Some(after_days), Some(last_rotated)) = (after_days, last_rotated) {
            if after_days > 0 {
                let overdue_threshold = now - Duration::days(2 * after_days);
                if last_rotated < overdue_threshold {
                    let rotated_days_ago = (now - last_rotated).num_days();
                    findings.push(InventoryFinding {
                        resource_id: resource.resource_id.clone(),
                        arn: resource.arn.clone(),
                        pillar: Pillar::Security,
                        reason_code: REASON_SEC_ROTATION_OVERDUE.to_string(),
                        severity: Severity::Medium,
                        message: format!(
                            "Secret {} is configured to rotate every {} days but was last rotated {} days ago; rotation is configured but not actually happening",
                            resource.resource_id, after_days, rotated_days_ago
                        ),
                        evidence: json!({
                            "automatically_after_days": after_days,
                            "last_rotated_date":
                                data_str(&resource.resource_data, "last_rotated_date"),
                            "rotated_days_ago": rotated_days_ago,
                        }),
                    });
                }
            }
        }
    }

    if data_str(&resource.resource_data, "kms_key_id").is_none() {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Security,
            reason_code: REASON_SEC_DEFAULT_KMS_KEY.to_string(),
            severity: Severity::Low,
            message: format!(
                "Secret {} is encrypted with the account default KMS key; a customer-managed key allows key policy control and auditable rotation",
                resource.resource_id
            ),
            evidence: json!({ "kms_key_id": Value::Null }),
        });
    }
}

fn evaluate_resilience(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    if data_str(&resource.resource_data, "primary_region").is_none() {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Resilience,
            reason_code: REASON_RES_NO_REPLICATION.to_string(),
            severity: Severity::Low,
            message: format!(
                "Secret {} is a single-region secret with no multi-region replication recorded; a regional outage blocks consumers from reading it",
                resource.resource_id
            ),
            evidence: json!({ "primary_region": Value::Null }),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
                "arn:aws:secretsmanager:us-east-1:123456789012:secret:{}-AbCdEf",
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

    fn ts(days_before_now: i64) -> String {
        (now() - Duration::days(days_before_now)).to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
    }

    fn healthy_data() -> Value {
        json!({
            "name": "secret-ok",
            "kms_key_id": "arn:aws:kms:us-east-1:123456789012:key/11111111-2222-3333-4444-555555555555",
            "rotation_enabled": true,
            "rotation_lambda_arn": "arn:aws:lambda:us-east-1:123456789012:function:rotate-secret",
            "automatically_after_days": 30,
            "last_rotated_date": ts(10),
            "last_accessed_date": ts(1),
            "last_changed_date": ts(10),
            "created_date": ts(365),
            "primary_region": "us-east-1",
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
    fn cost_flags_untagged_secret() {
        let r = fixture("secret-untagged", json!({}), healthy_data(), now());
        let report = evaluate_secretsmanager_fleet(&[r], Pillar::Cost, now());
        assert_eq!(codes(&report), vec![REASON_COST_NO_TAGS]);
    }

    #[test]
    fn cost_flags_secret_unused_for_90_days() {
        let mut data = healthy_data();
        data["last_accessed_date"] = json!(ts(120));
        let r = fixture("secret-unused", json!({"team": "core"}), data, now());
        let report = evaluate_secretsmanager_fleet(&[r], Pillar::Cost, now());
        assert_eq!(codes(&report), vec![REASON_COST_UNUSED_90D]);
        assert!(matches!(report.findings[0].severity, Severity::Low));
    }

    #[test]
    fn cost_does_not_flag_absent_last_accessed_date() {
        let mut data = healthy_data();
        data.as_object_mut().unwrap().remove("last_accessed_date");
        let r = fixture("secret-noaccess", json!({"team": "core"}), data, now());
        let report = evaluate_secretsmanager_fleet(&[r], Pillar::Cost, now());
        assert!(
            report.findings.is_empty(),
            "unexpected: {:?}",
            report.findings
        );
    }

    #[test]
    fn security_flags_rotation_disabled_on_self_managed_secret() {
        let mut data = healthy_data();
        data["rotation_enabled"] = json!(false);
        let r = fixture("secret-norotate", json!({"team": "core"}), data, now());
        let report = evaluate_secretsmanager_fleet(&[r], Pillar::Security, now());
        assert_eq!(codes(&report), vec![REASON_SEC_ROTATION_DISABLED]);
        assert!(matches!(report.findings[0].severity, Severity::Medium));
    }

    #[test]
    fn security_does_not_flag_rotation_disabled_on_service_owned_secret() {
        let mut data = healthy_data();
        data["rotation_enabled"] = json!(false);
        data["owning_service"] = json!("rds");
        let r = fixture("secret-rds", json!({"team": "core"}), data, now());
        let report = evaluate_secretsmanager_fleet(&[r], Pillar::Security, now());
        assert!(
            report.findings.is_empty(),
            "unexpected: {:?}",
            report.findings
        );
    }

    #[test]
    fn security_flags_overdue_rotation() {
        let mut data = healthy_data();
        // Configured for 30 days; last rotated 61 days ago exceeds 2x window.
        data["last_rotated_date"] = json!(ts(61));
        let r = fixture("secret-overdue", json!({"team": "core"}), data, now());
        let report = evaluate_secretsmanager_fleet(&[r], Pillar::Security, now());
        assert_eq!(codes(&report), vec![REASON_SEC_ROTATION_OVERDUE]);
    }

    #[test]
    fn security_allows_rotation_within_double_window() {
        let mut data = healthy_data();
        // Configured for 30 days; last rotated 59 days ago is within 2x window.
        data["last_rotated_date"] = json!(ts(59));
        let r = fixture("secret-inwindow", json!({"team": "core"}), data, now());
        let report = evaluate_secretsmanager_fleet(&[r], Pillar::Security, now());
        assert!(
            report.findings.is_empty(),
            "unexpected: {:?}",
            report.findings
        );
    }

    #[test]
    fn security_flags_default_kms_key() {
        let mut data = healthy_data();
        data.as_object_mut().unwrap().remove("kms_key_id");
        let r = fixture("secret-defaultkey", json!({"team": "core"}), data, now());
        let report = evaluate_secretsmanager_fleet(&[r], Pillar::Security, now());
        assert_eq!(codes(&report), vec![REASON_SEC_DEFAULT_KMS_KEY]);
        assert!(matches!(report.findings[0].severity, Severity::Low));
    }

    #[test]
    fn resilience_flags_single_region_secret() {
        let mut data = healthy_data();
        data.as_object_mut().unwrap().remove("primary_region");
        let r = fixture("secret-onergn", json!({"team": "core"}), data, now());
        let report = evaluate_secretsmanager_fleet(&[r], Pillar::Resilience, now());
        assert_eq!(codes(&report), vec![REASON_RES_NO_REPLICATION]);
    }

    #[test]
    fn stale_inventory_is_flagged() {
        let mut r = fixture(
            "secret-stale",
            json!({"team": "core"}),
            healthy_data(),
            now(),
        );
        r.last_refreshed = now() - Duration::hours(48);
        let report = evaluate_secretsmanager_fleet(&[r], Pillar::Resilience, now());
        assert_eq!(report.stale_resources, 1);
        assert!(codes(&report).contains(&REASON_INV_STALE_DATA));
    }

    #[test]
    fn non_secret_resources_are_skipped_and_not_counted() {
        let mut r = fixture("queue-1", json!({}), json!({}), now());
        r.resource_type = "SqsQueue".to_string();
        let report = evaluate_secretsmanager_fleet(&[r], Pillar::Cost, now());
        assert_eq!(report.resources_evaluated, 0);
        assert!(report.findings.is_empty());
    }

    #[test]
    fn healthy_secret_passes_all_pillars() {
        let r = fixture("secret-ok", json!({"team": "core"}), healthy_data(), now());
        for pillar in [Pillar::Cost, Pillar::Security, Pillar::Resilience] {
            let report = evaluate_secretsmanager_fleet(std::slice::from_ref(&r), pillar, now());
            assert!(
                report.findings.is_empty(),
                "unexpected for {:?}: {:?}",
                pillar,
                report.findings
            );
            assert_eq!(report.score, 100);
        }
    }
}

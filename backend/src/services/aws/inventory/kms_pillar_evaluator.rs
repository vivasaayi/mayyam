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

// Deterministic KMS inventory evaluators for the cost, security, and
// resilience pillars (roadmap rows 01-AWS-CLOUD-03655/03664/03691).
//
// Evaluates fields persisted by kms_control_plane: key_state, key_spec,
// key_manager, enabled, rotation_enabled, plus the tags column. AWS-managed
// keys cannot be tagged and cannot have rotation configured by the customer,
// so tag and rotation checks are gated on key_manager == "CUSTOMER".

use chrono::{DateTime, Utc};
use serde_json::json;

use crate::models::aws_resource::Model as AwsResourceModel;
use crate::services::aws::inventory::types::{
    check_stale, data_str, score_pillar, InventoryFinding, Pillar, PillarReport, Severity,
};

/// Only rows of this resource type are evaluated.
pub const RESOURCE_TYPE: &str = "KmsKey";

// Reason codes are the stable contract for findings; never reuse or rename.
pub const REASON_COST_NO_TAGS: &str = "KMS_COST_NO_TAGS";
pub const REASON_COST_DISABLED_KEY: &str = "KMS_COST_DISABLED_KEY";
pub const REASON_SEC_ROTATION_DISABLED: &str = "KMS_SEC_ROTATION_DISABLED";
pub const REASON_SEC_ROTATION_DATA_NOT_COLLECTED: &str = "KMS_SEC_ROTATION_DATA_NOT_COLLECTED";
pub const REASON_RES_KEY_PENDING_DELETION: &str = "KMS_RES_KEY_PENDING_DELETION";
pub const REASON_RES_KEY_UNAVAILABLE: &str = "KMS_RES_KEY_UNAVAILABLE";
pub const REASON_INV_STALE_DATA: &str = "KMS_INV_STALE_DATA";

/// Evaluate every KMS key in the fleet for one pillar. Rows whose
/// `resource_type` is not `KmsKey` are skipped and not counted.
pub fn evaluate_kms_fleet(
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

fn is_customer_managed(resource: &AwsResourceModel) -> bool {
    data_str(&resource.resource_data, "key_manager").as_deref() == Some("CUSTOMER")
}

fn key_state(resource: &AwsResourceModel) -> Option<String> {
    data_str(&resource.resource_data, "key_state")
}

fn evaluate_cost(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    // AWS-managed keys cannot be tagged, so only customer-managed keys are
    // held to the tagging standard.
    if is_customer_managed(resource) {
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
                    "Customer-managed key {} has no tags recorded (untagged resource or tag collection gap); cost allocation cannot be assessed",
                    resource.resource_id
                ),
                evidence: json!({ "tags": resource.tags }),
            });
        }

        // A disabled customer-managed key still bills $1/month; a key pending
        // deletion does not bill, so it is not flagged here.
        let enabled = resource
            .resource_data
            .get("enabled")
            .and_then(|v| v.as_bool());
        let state = key_state(resource);
        if enabled == Some(false) && state.as_deref() != Some("PendingDeletion") {
            findings.push(InventoryFinding {
                resource_id: resource.resource_id.clone(),
                arn: resource.arn.clone(),
                pillar: Pillar::Cost,
                reason_code: REASON_COST_DISABLED_KEY.to_string(),
                severity: Severity::Low,
                message: format!(
                    "Customer-managed key {} is disabled but still bills USD 1/month; schedule deletion if it is no longer needed",
                    resource.resource_id
                ),
                evidence: json!({ "enabled": false, "key_state": state }),
            });
        }
    }
}

fn evaluate_security(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    // Automatic rotation only applies to customer-managed symmetric keys.
    let symmetric =
        data_str(&resource.resource_data, "key_spec").as_deref() == Some("SYMMETRIC_DEFAULT");
    if !is_customer_managed(resource) || !symmetric {
        return;
    }

    let rotation_enabled = resource
        .resource_data
        .get("rotation_enabled")
        .and_then(|v| v.as_bool());
    match rotation_enabled {
        Some(false) => {
            findings.push(InventoryFinding {
                resource_id: resource.resource_id.clone(),
                arn: resource.arn.clone(),
                pillar: Pillar::Security,
                reason_code: REASON_SEC_ROTATION_DISABLED.to_string(),
                severity: Severity::Medium,
                message: format!(
                    "Customer-managed symmetric key {} has automatic rotation disabled",
                    resource.resource_id
                ),
                evidence: json!({ "rotation_enabled": false, "key_spec": "SYMMETRIC_DEFAULT" }),
            });
        }
        None => {
            findings.push(InventoryFinding {
                resource_id: resource.resource_id.clone(),
                arn: resource.arn.clone(),
                pillar: Pillar::Security,
                reason_code: REASON_SEC_ROTATION_DATA_NOT_COLLECTED.to_string(),
                severity: Severity::Low,
                message: format!(
                    "Rotation status for customer-managed symmetric key {} is not collected yet; security pillar cannot be fully assessed",
                    resource.resource_id
                ),
                evidence: json!({ "rotation_enabled_collected": false }),
            });
        }
        Some(true) => {}
    }
}

fn evaluate_resilience(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    match key_state(resource).as_deref() {
        Some("PendingDeletion") => {
            findings.push(InventoryFinding {
                resource_id: resource.resource_id.clone(),
                arn: resource.arn.clone(),
                pillar: Pillar::Resilience,
                reason_code: REASON_RES_KEY_PENDING_DELETION.to_string(),
                severity: Severity::High,
                message: format!(
                    "Key {} is pending deletion; everything encrypted under it becomes unrecoverable once deletion completes",
                    resource.resource_id
                ),
                evidence: json!({ "key_state": "PendingDeletion" }),
            });
        }
        // "Disabled" is an intentional operator action and is covered by the
        // cost pillar; only an unavailable key is a resilience defect.
        Some("Unavailable") => {
            findings.push(InventoryFinding {
                resource_id: resource.resource_id.clone(),
                arn: resource.arn.clone(),
                pillar: Pillar::Resilience,
                reason_code: REASON_RES_KEY_UNAVAILABLE.to_string(),
                severity: Severity::Medium,
                message: format!(
                    "Key {} is in state Unavailable; cryptographic operations against it will fail",
                    resource.resource_id
                ),
                evidence: json!({ "key_state": "Unavailable" }),
            });
        }
        _ => {}
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
            arn: format!("arn:aws:kms:us-east-1:123456789012:key/{}", resource_id),
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

    fn healthy_customer_data() -> Value {
        json!({
            "key_id": "key-ok",
            "key_state": "Enabled",
            "key_usage": "ENCRYPT_DECRYPT",
            "key_spec": "SYMMETRIC_DEFAULT",
            "key_manager": "CUSTOMER",
            "origin": "AWS_KMS",
            "enabled": true,
            "multi_region": false,
            "rotation_enabled": true,
        })
    }

    fn aws_managed_data() -> Value {
        json!({
            "key_id": "key-aws",
            "key_state": "Enabled",
            "key_usage": "ENCRYPT_DECRYPT",
            "key_spec": "SYMMETRIC_DEFAULT",
            "key_manager": "AWS",
            "origin": "AWS_KMS",
            "enabled": true,
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
    fn cost_flags_untagged_customer_key() {
        let r = fixture("key-untagged", json!({}), healthy_customer_data(), now());
        let report = evaluate_kms_fleet(&[r], Pillar::Cost, now());
        assert_eq!(codes(&report), vec![REASON_COST_NO_TAGS]);
    }

    #[test]
    fn cost_flags_disabled_customer_key_still_billed() {
        let mut data = healthy_customer_data();
        data["enabled"] = json!(false);
        data["key_state"] = json!("Disabled");
        let r = fixture("key-disabled", json!({"team": "sec"}), data, now());
        let report = evaluate_kms_fleet(&[r], Pillar::Cost, now());
        assert_eq!(codes(&report), vec![REASON_COST_DISABLED_KEY]);
        assert!(report.findings[0].message.contains("1/month"));
    }

    #[test]
    fn cost_does_not_flag_pending_deletion_key_as_disabled_spend() {
        let mut data = healthy_customer_data();
        data["enabled"] = json!(false);
        data["key_state"] = json!("PendingDeletion");
        let r = fixture("key-deleting", json!({"team": "sec"}), data, now());
        let report = evaluate_kms_fleet(&[r], Pillar::Cost, now());
        assert!(
            report.findings.is_empty(),
            "unexpected: {:?}",
            report.findings
        );
    }

    #[test]
    fn aws_managed_key_not_flagged_for_tags_or_rotation() {
        let r = fixture("key-aws", json!({}), aws_managed_data(), now());
        for pillar in [Pillar::Cost, Pillar::Security] {
            let report = evaluate_kms_fleet(std::slice::from_ref(&r), pillar, now());
            assert!(
                report.findings.is_empty(),
                "unexpected for {:?}: {:?}",
                pillar,
                report.findings
            );
        }
    }

    #[test]
    fn security_flags_rotation_disabled_on_symmetric_customer_key() {
        let mut data = healthy_customer_data();
        data["rotation_enabled"] = json!(false);
        let r = fixture("key-norotate", json!({"team": "sec"}), data, now());
        let report = evaluate_kms_fleet(&[r], Pillar::Security, now());
        assert_eq!(codes(&report), vec![REASON_SEC_ROTATION_DISABLED]);
    }

    #[test]
    fn security_reports_gap_when_rotation_not_collected() {
        let mut data = healthy_customer_data();
        data.as_object_mut().unwrap().remove("rotation_enabled");
        let r = fixture("key-rotgap", json!({"team": "sec"}), data, now());
        let report = evaluate_kms_fleet(&[r], Pillar::Security, now());
        assert_eq!(codes(&report), vec![REASON_SEC_ROTATION_DATA_NOT_COLLECTED]);
    }

    #[test]
    fn security_skips_asymmetric_customer_key_for_rotation() {
        let mut data = healthy_customer_data();
        data["key_spec"] = json!("RSA_2048");
        data.as_object_mut().unwrap().remove("rotation_enabled");
        let r = fixture("key-rsa", json!({"team": "sec"}), data, now());
        let report = evaluate_kms_fleet(&[r], Pillar::Security, now());
        assert!(
            report.findings.is_empty(),
            "unexpected: {:?}",
            report.findings
        );
    }

    #[test]
    fn resilience_flags_pending_deletion_as_high() {
        let mut data = healthy_customer_data();
        data["enabled"] = json!(false);
        data["key_state"] = json!("PendingDeletion");
        let r = fixture("key-deleting", json!({"team": "sec"}), data, now());
        let report = evaluate_kms_fleet(&[r], Pillar::Resilience, now());
        assert_eq!(codes(&report), vec![REASON_RES_KEY_PENDING_DELETION]);
        assert!(matches!(report.findings[0].severity, Severity::High));
    }

    #[test]
    fn resilience_flags_unavailable_but_not_disabled() {
        let mut unavailable = healthy_customer_data();
        unavailable["key_state"] = json!("Unavailable");
        let r1 = fixture("key-unavail", json!({"team": "sec"}), unavailable, now());
        let report = evaluate_kms_fleet(&[r1], Pillar::Resilience, now());
        assert_eq!(codes(&report), vec![REASON_RES_KEY_UNAVAILABLE]);

        let mut disabled = healthy_customer_data();
        disabled["enabled"] = json!(false);
        disabled["key_state"] = json!("Disabled");
        let r2 = fixture("key-disabled", json!({"team": "sec"}), disabled, now());
        let report = evaluate_kms_fleet(&[r2], Pillar::Resilience, now());
        assert!(
            report.findings.is_empty(),
            "unexpected: {:?}",
            report.findings
        );
    }

    #[test]
    fn stale_inventory_is_flagged() {
        let mut r = fixture(
            "key-stale",
            json!({"team": "sec"}),
            healthy_customer_data(),
            now(),
        );
        r.last_refreshed = now() - Duration::hours(48);
        let report = evaluate_kms_fleet(&[r], Pillar::Resilience, now());
        assert_eq!(report.stale_resources, 1);
        assert!(codes(&report).contains(&REASON_INV_STALE_DATA));
    }

    #[test]
    fn non_kms_resources_are_skipped_and_not_counted() {
        let mut r = fixture("queue-1", json!({}), json!({}), now());
        r.resource_type = "SqsQueue".to_string();
        let report = evaluate_kms_fleet(&[r], Pillar::Cost, now());
        assert_eq!(report.resources_evaluated, 0);
        assert!(report.findings.is_empty());
    }

    #[test]
    fn healthy_customer_key_passes_all_pillars() {
        let r = fixture(
            "key-ok",
            json!({"team": "sec"}),
            healthy_customer_data(),
            now(),
        );
        for pillar in [Pillar::Cost, Pillar::Security, Pillar::Resilience] {
            let report = evaluate_kms_fleet(std::slice::from_ref(&r), pillar, now());
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

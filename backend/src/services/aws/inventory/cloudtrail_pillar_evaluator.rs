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

// Deterministic CloudTrail inventory evaluators for the cost, resilience,
// and security pillars (roadmap rows 01-AWS-CLOUD-03781/03790/03817).
//
// Evaluates fields persisted by cloudtrail_control_plane: is_logging,
// log_file_validation_enabled, kms_key_id, is_multi_region_trail,
// latest_delivery_error, plus the tags map.

use chrono::{DateTime, Utc};
use serde_json::json;

use crate::models::aws_resource::Model as AwsResourceModel;
use crate::services::aws::inventory::types::{
    check_stale, score_pillar, InventoryFinding, Pillar, PillarReport, Severity,
};

// Reason codes are the stable contract for findings; never reuse or rename.
pub const REASON_SEC_LOGGING_DISABLED: &str = "CLOUDTRAIL_SEC_LOGGING_DISABLED";
pub const REASON_SEC_LOG_VALIDATION_DISABLED: &str = "CLOUDTRAIL_SEC_LOG_VALIDATION_DISABLED";
pub const REASON_SEC_NO_KMS_ENCRYPTION: &str = "CLOUDTRAIL_SEC_NO_KMS_ENCRYPTION";
pub const REASON_SEC_LOGGING_STATUS_DATA_NOT_COLLECTED: &str =
    "CLOUDTRAIL_SEC_LOGGING_STATUS_DATA_NOT_COLLECTED";
pub const REASON_RES_NOT_MULTI_REGION: &str = "CLOUDTRAIL_RES_NOT_MULTI_REGION";
pub const REASON_RES_DELIVERY_ERROR: &str = "CLOUDTRAIL_RES_DELIVERY_ERROR";
pub const REASON_COST_NO_TAGS: &str = "CLOUDTRAIL_COST_NO_TAGS";
pub const REASON_INV_STALE_DATA: &str = "CLOUDTRAIL_INV_STALE_DATA";

const RESOURCE_TYPE: &str = "CloudTrailTrail";

/// Evaluate every CloudTrail trail in the fleet for one pillar. Resources of
/// other types are skipped and excluded from the evaluated count.
pub fn evaluate_cloudtrail_fleet(
    resources: &[AwsResourceModel],
    pillar: Pillar,
    now: DateTime<Utc>,
) -> PillarReport {
    let mut findings: Vec<InventoryFinding> = Vec::new();
    let mut stale_resources = 0usize;
    let mut resources_evaluated = 0usize;

    for resource in resources {
        if resource.resource_type != RESOURCE_TYPE {
            continue;
        }
        resources_evaluated += 1;

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
        resources_evaluated,
        stale_resources,
        score,
        findings,
    }
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
                "Trail {} has no tags recorded (untagged resource or tag collection gap); cost allocation cannot be assessed",
                resource.resource_id
            ),
            evidence: json!({ "tags": resource.tags }),
        });
    }
}

fn evaluate_security(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    let is_logging = resource
        .resource_data
        .get("is_logging")
        .and_then(|v| v.as_bool());

    match is_logging {
        None => {
            findings.push(InventoryFinding {
                resource_id: resource.resource_id.clone(),
                arn: resource.arn.clone(),
                pillar: Pillar::Security,
                reason_code: REASON_SEC_LOGGING_STATUS_DATA_NOT_COLLECTED.to_string(),
                severity: Severity::Medium,
                message: format!(
                    "Logging status for trail {} is not collected yet; security pillar cannot be fully assessed",
                    resource.resource_id
                ),
                evidence: json!({ "is_logging_collected": false }),
            });
        }
        Some(false) => {
            findings.push(InventoryFinding {
                resource_id: resource.resource_id.clone(),
                arn: resource.arn.clone(),
                pillar: Pillar::Security,
                reason_code: REASON_SEC_LOGGING_DISABLED.to_string(),
                severity: Severity::High,
                message: format!(
                    "Trail {} is not logging; API activity is not being recorded",
                    resource.resource_id
                ),
                evidence: json!({ "is_logging": false }),
            });
        }
        Some(true) => {}
    }

    let validation = resource
        .resource_data
        .get("log_file_validation_enabled")
        .and_then(|v| v.as_bool());
    if validation == Some(false) {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Security,
            reason_code: REASON_SEC_LOG_VALIDATION_DISABLED.to_string(),
            severity: Severity::Medium,
            message: format!(
                "Trail {} has log file validation disabled; log tampering cannot be detected",
                resource.resource_id
            ),
            evidence: json!({ "log_file_validation_enabled": false }),
        });
    }

    let kms_key_id = resource
        .resource_data
        .get("kms_key_id")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty());
    if kms_key_id.is_none() {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Security,
            reason_code: REASON_SEC_NO_KMS_ENCRYPTION.to_string(),
            severity: Severity::Medium,
            message: format!(
                "Trail {} does not encrypt log files with a customer managed KMS key",
                resource.resource_id
            ),
            evidence: json!({ "kms_key_id": resource.resource_data.get("kms_key_id") }),
        });
    }
}

fn evaluate_resilience(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    let multi_region = resource
        .resource_data
        .get("is_multi_region_trail")
        .and_then(|v| v.as_bool());
    if multi_region == Some(false) {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Resilience,
            reason_code: REASON_RES_NOT_MULTI_REGION.to_string(),
            severity: Severity::Medium,
            message: format!(
                "Trail {} only records its home region; activity in other regions is not captured",
                resource.resource_id
            ),
            evidence: json!({ "is_multi_region_trail": false }),
        });
    }

    let delivery_error = resource
        .resource_data
        .get("latest_delivery_error")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty());
    if let Some(error) = delivery_error {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Resilience,
            reason_code: REASON_RES_DELIVERY_ERROR.to_string(),
            severity: Severity::High,
            message: format!(
                "Trail {} failed its latest log delivery to S3; audit history has a gap",
                resource.resource_id
            ),
            evidence: json!({ "latest_delivery_error": error }),
        });
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
                "arn:aws:cloudtrail:us-east-1:123456789012:trail/{}",
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

    fn healthy_data() -> Value {
        json!({
            "trail_arn": "arn:aws:cloudtrail:us-east-1:123456789012:trail/trail-ok",
            "home_region": "us-east-1",
            "is_multi_region_trail": true,
            "is_organization_trail": false,
            "log_file_validation_enabled": true,
            "kms_key_id": "arn:aws:kms:us-east-1:123456789012:key/abc",
            "s3_bucket_name": "audit-logs",
            "include_global_service_events": true,
            "is_logging": true,
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
    fn security_flags_logging_disabled() {
        let mut data = healthy_data();
        data["is_logging"] = json!(false);
        let r = fixture("trail-off", json!({"team": "sec"}), data, now());
        let report = evaluate_cloudtrail_fleet(&[r], Pillar::Security, now());
        assert!(codes(&report).contains(&REASON_SEC_LOGGING_DISABLED));
    }

    #[test]
    fn security_flags_log_validation_disabled() {
        let mut data = healthy_data();
        data["log_file_validation_enabled"] = json!(false);
        let r = fixture("trail-noval", json!({"team": "sec"}), data, now());
        let report = evaluate_cloudtrail_fleet(&[r], Pillar::Security, now());
        assert!(codes(&report).contains(&REASON_SEC_LOG_VALIDATION_DISABLED));
    }

    #[test]
    fn security_flags_missing_kms_encryption() {
        let mut data = healthy_data();
        data.as_object_mut().unwrap().remove("kms_key_id");
        let r = fixture("trail-nokms", json!({"team": "sec"}), data, now());
        let report = evaluate_cloudtrail_fleet(&[r], Pillar::Security, now());
        assert!(codes(&report).contains(&REASON_SEC_NO_KMS_ENCRYPTION));
    }

    #[test]
    fn security_reports_gap_when_logging_status_not_collected() {
        let mut data = healthy_data();
        data.as_object_mut().unwrap().remove("is_logging");
        let r = fixture("trail-gap", json!({"team": "sec"}), data, now());
        let report = evaluate_cloudtrail_fleet(&[r], Pillar::Security, now());
        let report_codes = codes(&report);
        assert!(report_codes.contains(&REASON_SEC_LOGGING_STATUS_DATA_NOT_COLLECTED));
        assert!(!report_codes.contains(&REASON_SEC_LOGGING_DISABLED));
    }

    #[test]
    fn resilience_flags_single_region_trail() {
        let mut data = healthy_data();
        data["is_multi_region_trail"] = json!(false);
        let r = fixture("trail-single", json!({"team": "sec"}), data, now());
        let report = evaluate_cloudtrail_fleet(&[r], Pillar::Resilience, now());
        assert!(codes(&report).contains(&REASON_RES_NOT_MULTI_REGION));
    }

    #[test]
    fn resilience_flags_delivery_error() {
        let mut data = healthy_data();
        data["latest_delivery_error"] = json!("Access Denied");
        let r = fixture("trail-deliver", json!({"team": "sec"}), data, now());
        let report = evaluate_cloudtrail_fleet(&[r], Pillar::Resilience, now());
        assert!(codes(&report).contains(&REASON_RES_DELIVERY_ERROR));
    }

    #[test]
    fn cost_flags_untagged_trail() {
        let r = fixture("trail-untagged", json!({}), healthy_data(), now());
        let report = evaluate_cloudtrail_fleet(&[r], Pillar::Cost, now());
        assert_eq!(codes(&report), vec![REASON_COST_NO_TAGS]);
    }

    #[test]
    fn stale_inventory_is_reported() {
        let mut r = fixture("trail-stale", json!({"team": "sec"}), healthy_data(), now());
        r.last_refreshed = now() - Duration::hours(48);
        let report = evaluate_cloudtrail_fleet(&[r], Pillar::Security, now());
        assert_eq!(report.stale_resources, 1);
        assert!(codes(&report).contains(&REASON_INV_STALE_DATA));
    }

    #[test]
    fn non_cloudtrail_resources_are_skipped() {
        let mut r = fixture("not-a-trail", json!({}), json!({}), now());
        r.resource_type = "S3Bucket".to_string();
        let report = evaluate_cloudtrail_fleet(&[r], Pillar::Security, now());
        assert_eq!(report.resources_evaluated, 0);
        assert!(report.findings.is_empty());
    }

    #[test]
    fn healthy_trail_passes_all_pillars() {
        let r = fixture("trail-ok", json!({"team": "sec"}), healthy_data(), now());
        for pillar in [Pillar::Cost, Pillar::Security, Pillar::Resilience] {
            let report = evaluate_cloudtrail_fleet(std::slice::from_ref(&r), pillar, now());
            assert_eq!(report.resources_evaluated, 1);
            assert!(
                report.findings.is_empty(),
                "unexpected for {:?}: {:?}",
                pillar,
                report.findings
            );
        }
    }
}

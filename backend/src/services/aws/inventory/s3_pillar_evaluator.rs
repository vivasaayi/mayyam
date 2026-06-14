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

// Deterministic S3 inventory evaluators for the cost, security, and
// resilience pillars (roadmap rows 01-AWS-CLOUD-00631/00640/00667).
//
// Pure domain logic over collected `aws_resources` rows; evaluates fields
// persisted by s3_control_plane: versioning_enabled, lifecycle_rules,
// creation_date, region. Where the collector does not yet gather a posture
// signal (encryption, public access block), the evaluator emits an explicit
// data-gap finding instead of guessing.

use chrono::{DateTime, Utc};
use serde_json::json;

use crate::models::aws_resource::Model as AwsResourceModel;
use crate::services::aws::inventory::types::{
    check_stale, has_any_tag, score_pillar, InventoryFinding, Pillar, PillarReport, Severity,
    COST_ALLOCATION_TAG_KEYS, OWNER_TAG_KEYS,
};

// Reason codes are the stable contract for findings; never reuse or rename.
pub const REASON_COST_MISSING_ALLOCATION_TAGS: &str = "S3_COST_MISSING_ALLOCATION_TAGS";
pub const REASON_COST_NO_LIFECYCLE_RULES: &str = "S3_COST_NO_LIFECYCLE_RULES";
pub const REASON_SEC_MISSING_OWNER_TAG: &str = "S3_SEC_MISSING_OWNER_TAG";
pub const REASON_SEC_POSTURE_DATA_NOT_COLLECTED: &str = "S3_SEC_POSTURE_DATA_NOT_COLLECTED";
pub const REASON_RES_VERSIONING_DISABLED: &str = "S3_RES_VERSIONING_DISABLED";
pub const REASON_INV_STALE_DATA: &str = "S3_INV_STALE_DATA";

/// Evaluate every S3 bucket in the fleet for one pillar.
pub fn evaluate_s3_fleet(
    resources: &[AwsResourceModel],
    pillar: Pillar,
    now: DateTime<Utc>,
) -> PillarReport {
    let mut findings: Vec<InventoryFinding> = Vec::new();
    let mut stale_resources = 0usize;

    for resource in resources {
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
        resources_evaluated: resources.len(),
        stale_resources,
        score,
        findings,
    }
}

fn evaluate_cost(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    if !has_any_tag(&resource.tags, COST_ALLOCATION_TAG_KEYS) {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Cost,
            reason_code: REASON_COST_MISSING_ALLOCATION_TAGS.to_string(),
            severity: Severity::Medium,
            message: format!(
                "Bucket {} has no cost allocation tag (expected one of: {})",
                resource.resource_id,
                COST_ALLOCATION_TAG_KEYS.join(", ")
            ),
            evidence: json!({ "tags": resource.tags }),
        });
    }

    let has_active_lifecycle_rule = resource
        .resource_data
        .get("lifecycle_rules")
        .and_then(|v| v.as_array())
        .map(|rules| {
            rules
                .iter()
                .any(|rule| rule.get("status").and_then(|s| s.as_str()) == Some("Enabled"))
        })
        .unwrap_or(false);
    if !has_active_lifecycle_rule {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Cost,
            reason_code: REASON_COST_NO_LIFECYCLE_RULES.to_string(),
            severity: Severity::Low,
            message: format!(
                "Bucket {} has no enabled lifecycle rule; objects never transition or expire and storage cost grows unbounded",
                resource.resource_id
            ),
            evidence: json!({
                "lifecycle_rules": resource.resource_data.get("lifecycle_rules"),
            }),
        });
    }
}

fn evaluate_security(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    // The collector does not yet gather encryption or public-access-block
    // state. Surface the gap deterministically rather than scoring blind.
    let has_encryption_data = resource.resource_data.get("encryption").is_some();
    let has_public_access_data = resource.resource_data.get("public_access_block").is_some();
    if !has_encryption_data || !has_public_access_data {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Security,
            reason_code: REASON_SEC_POSTURE_DATA_NOT_COLLECTED.to_string(),
            severity: Severity::Medium,
            message: format!(
                "Bucket {} security posture (encryption, public access block) is not collected yet; security pillar cannot be fully assessed",
                resource.resource_id
            ),
            evidence: json!({
                "encryption_collected": has_encryption_data,
                "public_access_block_collected": has_public_access_data,
            }),
        });
    }

    if !has_any_tag(&resource.tags, OWNER_TAG_KEYS) {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Security,
            reason_code: REASON_SEC_MISSING_OWNER_TAG.to_string(),
            severity: Severity::Low,
            message: format!(
                "Bucket {} has no owner/team tag; security findings cannot be routed to an owner",
                resource.resource_id
            ),
            evidence: json!({ "tags": resource.tags }),
        });
    }
}

fn evaluate_resilience(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    let versioning_enabled = resource
        .resource_data
        .get("versioning_enabled")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    if !versioning_enabled {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Resilience,
            reason_code: REASON_RES_VERSIONING_DISABLED.to_string(),
            severity: Severity::Medium,
            message: format!(
                "Bucket {} has versioning disabled; deleted or overwritten objects cannot be recovered",
                resource.resource_id
            ),
            evidence: json!({
                "versioning_enabled": resource.resource_data.get("versioning_enabled"),
            }),
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
        refreshed_hours_ago: i64,
        now: DateTime<Utc>,
    ) -> AwsResourceModel {
        let refreshed = now - Duration::hours(refreshed_hours_ago);
        AwsResourceModel {
            id: Uuid::new_v4(),
            sync_id: None,
            account_id: "123456789012".to_string(),
            profile: None,
            region: "us-east-1".to_string(),
            resource_type: "S3Bucket".to_string(),
            resource_id: resource_id.to_string(),
            arn: format!("arn:aws:s3:::{}", resource_id),
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
            "region": "us-east-1",
            "versioning_enabled": true,
            "lifecycle_rules": [{"id": "expire-old", "status": "Enabled", "transition_days": 30}],
            "encryption": {"sse": "AES256"},
            "public_access_block": {"block_public_acls": true},
        })
    }

    #[test]
    fn cost_flags_missing_tags_and_missing_lifecycle_rules() {
        let r = fixture(
            "bucket-untagged",
            json!({}),
            json!({"versioning_enabled": false}),
            1,
            now(),
        );
        let report = evaluate_s3_fleet(&[r], Pillar::Cost, now());
        let codes: Vec<&str> = report
            .findings
            .iter()
            .map(|f| f.reason_code.as_str())
            .collect();
        assert!(codes.contains(&REASON_COST_MISSING_ALLOCATION_TAGS));
        assert!(codes.contains(&REASON_COST_NO_LIFECYCLE_RULES));
    }

    #[test]
    fn cost_treats_disabled_lifecycle_rule_as_missing() {
        let mut data = healthy_data();
        data["lifecycle_rules"] = json!([{"id": "off", "status": "Disabled"}]);
        let r = fixture(
            "bucket-disabled-rule",
            json!({"team": "data"}),
            data,
            1,
            now(),
        );
        let report = evaluate_s3_fleet(&[r], Pillar::Cost, now());
        assert_eq!(
            report
                .findings
                .iter()
                .map(|f| f.reason_code.as_str())
                .collect::<Vec<_>>(),
            vec![REASON_COST_NO_LIFECYCLE_RULES]
        );
    }

    #[test]
    fn cost_passes_for_tagged_bucket_with_enabled_lifecycle_rule() {
        let r = fixture(
            "bucket-good",
            json!({"team": "data"}),
            healthy_data(),
            1,
            now(),
        );
        let report = evaluate_s3_fleet(&[r], Pillar::Cost, now());
        assert!(
            report.findings.is_empty(),
            "unexpected: {:?}",
            report.findings
        );
        assert_eq!(report.score, 100);
    }

    #[test]
    fn security_reports_data_gap_when_posture_not_collected() {
        let r = fixture(
            "bucket-gap",
            json!({"owner": "sre"}),
            json!({"versioning_enabled": true}),
            1,
            now(),
        );
        let report = evaluate_s3_fleet(&[r], Pillar::Security, now());
        let gap = report
            .findings
            .iter()
            .find(|f| f.reason_code == REASON_SEC_POSTURE_DATA_NOT_COLLECTED)
            .expect("data gap finding");
        assert_eq!(gap.severity, Severity::Medium);
        assert_eq!(gap.evidence["encryption_collected"], json!(false));
    }

    #[test]
    fn security_passes_when_posture_collected_and_owned() {
        let r = fixture(
            "bucket-ok",
            json!({"owner": "sre"}),
            healthy_data(),
            1,
            now(),
        );
        let report = evaluate_s3_fleet(&[r], Pillar::Security, now());
        assert!(
            report.findings.is_empty(),
            "unexpected: {:?}",
            report.findings
        );
    }

    #[test]
    fn resilience_flags_versioning_disabled_or_missing() {
        let r = fixture(
            "bucket-novers",
            json!({"owner": "sre"}),
            json!({"region": "us-east-1"}),
            1,
            now(),
        );
        let report = evaluate_s3_fleet(&[r], Pillar::Resilience, now());
        assert_eq!(
            report
                .findings
                .iter()
                .map(|f| f.reason_code.as_str())
                .collect::<Vec<_>>(),
            vec![REASON_RES_VERSIONING_DISABLED]
        );
    }

    #[test]
    fn stale_inventory_is_reported_as_failure_path() {
        let r = fixture(
            "bucket-stale",
            json!({"owner": "sre", "project": "mayyam"}),
            healthy_data(),
            48,
            now(),
        );
        let report = evaluate_s3_fleet(&[r], Pillar::Resilience, now());
        assert_eq!(report.stale_resources, 1);
        assert!(report
            .findings
            .iter()
            .any(|f| f.reason_code == REASON_INV_STALE_DATA));
    }
}

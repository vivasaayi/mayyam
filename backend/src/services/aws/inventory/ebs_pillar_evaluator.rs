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

// Deterministic EBS inventory evaluators for the cost, security, and
// resilience pillars (roadmap rows 01-AWS-CLOUD-00694/00703/00730).
//
// Evaluates fields persisted by ebs_control_plane: volume_type, size, state,
// encrypted, availability_zone.

use chrono::{DateTime, Utc};
use serde_json::json;

use crate::models::aws_resource::Model as AwsResourceModel;
use crate::services::aws::inventory::types::{
    check_stale, data_str, has_any_tag, score_pillar, InventoryFinding, Pillar, PillarReport,
    Severity, COST_ALLOCATION_TAG_KEYS, OWNER_TAG_KEYS,
};

// Reason codes are the stable contract for findings; never reuse or rename.
pub const REASON_COST_MISSING_ALLOCATION_TAGS: &str = "EBS_COST_MISSING_ALLOCATION_TAGS";
pub const REASON_COST_UNATTACHED_VOLUME: &str = "EBS_COST_UNATTACHED_VOLUME";
pub const REASON_COST_GP2_VOLUME: &str = "EBS_COST_GP2_VOLUME";
pub const REASON_SEC_UNENCRYPTED: &str = "EBS_SEC_UNENCRYPTED";
pub const REASON_SEC_MISSING_OWNER_TAG: &str = "EBS_SEC_MISSING_OWNER_TAG";
pub const REASON_RES_SNAPSHOT_DATA_NOT_COLLECTED: &str = "EBS_RES_SNAPSHOT_DATA_NOT_COLLECTED";
pub const REASON_INV_STALE_DATA: &str = "EBS_INV_STALE_DATA";

/// Evaluate every EBS volume in the fleet for one pillar.
pub fn evaluate_ebs_fleet(
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
                "Volume {} has no cost allocation tag (expected one of: {})",
                resource.resource_id,
                COST_ALLOCATION_TAG_KEYS.join(", ")
            ),
            evidence: json!({ "tags": resource.tags }),
        });
    }

    if data_str(&resource.resource_data, "state").as_deref() == Some("available") {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Cost,
            reason_code: REASON_COST_UNATTACHED_VOLUME.to_string(),
            severity: Severity::Medium,
            message: format!(
                "Volume {} is unattached but still billed; snapshot and delete it if no longer needed",
                resource.resource_id
            ),
            evidence: json!({
                "state": "available",
                "size": resource.resource_data.get("size"),
            }),
        });
    }

    if data_str(&resource.resource_data, "volume_type").as_deref() == Some("gp2") {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Cost,
            reason_code: REASON_COST_GP2_VOLUME.to_string(),
            severity: Severity::Low,
            message: format!(
                "Volume {} is gp2; gp3 offers the same baseline performance at ~20% lower cost",
                resource.resource_id
            ),
            evidence: json!({
                "volume_type": "gp2",
                "size": resource.resource_data.get("size"),
            }),
        });
    }
}

fn evaluate_security(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    let encrypted = resource
        .resource_data
        .get("encrypted")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    if !encrypted {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Security,
            reason_code: REASON_SEC_UNENCRYPTED.to_string(),
            severity: Severity::High,
            message: format!("Volume {} is not encrypted at rest", resource.resource_id),
            evidence: json!({
                "encrypted": resource.resource_data.get("encrypted"),
                "kms_key_id": resource.resource_data.get("kms_key_id"),
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
                "Volume {} has no owner/team tag; security findings cannot be routed to an owner",
                resource.resource_id
            ),
            evidence: json!({ "tags": resource.tags }),
        });
    }
}

fn evaluate_resilience(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    // Snapshot coverage is not collected yet; surface the gap rather than
    // assuming volumes are protected.
    if resource.resource_data.get("latest_snapshot").is_none() {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Resilience,
            reason_code: REASON_RES_SNAPSHOT_DATA_NOT_COLLECTED.to_string(),
            severity: Severity::Medium,
            message: format!(
                "Snapshot coverage for volume {} is not collected yet; recovery point cannot be assessed",
                resource.resource_id
            ),
            evidence: json!({ "latest_snapshot_collected": false }),
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
            resource_type: "EbsVolume".to_string(),
            resource_id: resource_id.to_string(),
            arn: format!("arn:aws:ec2:us-east-1:123456789012:volume/{}", resource_id),
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
            "volume_type": "gp3",
            "size": 100,
            "state": "in-use",
            "encrypted": true,
            "availability_zone": "us-east-1a",
            "latest_snapshot": {"id": "snap-1", "age_hours": 12},
        })
    }

    #[test]
    fn cost_flags_untagged_unattached_gp2_volume() {
        let r = fixture(
            "vol-waste",
            json!({}),
            json!({"volume_type": "gp2", "size": 500, "state": "available", "encrypted": true}),
            1,
            now(),
        );
        let report = evaluate_ebs_fleet(&[r], Pillar::Cost, now());
        let codes: Vec<&str> = report
            .findings
            .iter()
            .map(|f| f.reason_code.as_str())
            .collect();
        assert!(codes.contains(&REASON_COST_MISSING_ALLOCATION_TAGS));
        assert!(codes.contains(&REASON_COST_UNATTACHED_VOLUME));
        assert!(codes.contains(&REASON_COST_GP2_VOLUME));
    }

    #[test]
    fn cost_passes_for_tagged_attached_gp3_volume() {
        let r = fixture(
            "vol-good",
            json!({"team": "infra"}),
            healthy_data(),
            1,
            now(),
        );
        let report = evaluate_ebs_fleet(&[r], Pillar::Cost, now());
        assert!(
            report.findings.is_empty(),
            "unexpected: {:?}",
            report.findings
        );
        assert_eq!(report.score, 100);
    }

    #[test]
    fn security_flags_unencrypted_volume_as_high() {
        let mut data = healthy_data();
        data["encrypted"] = json!(false);
        let r = fixture("vol-plain", json!({"owner": "infra"}), data, 1, now());
        let report = evaluate_ebs_fleet(&[r], Pillar::Security, now());
        let finding = report
            .findings
            .iter()
            .find(|f| f.reason_code == REASON_SEC_UNENCRYPTED)
            .expect("unencrypted finding");
        assert_eq!(finding.severity, Severity::High);
    }

    #[test]
    fn security_passes_for_encrypted_owned_volume() {
        let r = fixture(
            "vol-ok",
            json!({"owner": "infra"}),
            healthy_data(),
            1,
            now(),
        );
        let report = evaluate_ebs_fleet(&[r], Pillar::Security, now());
        assert!(
            report.findings.is_empty(),
            "unexpected: {:?}",
            report.findings
        );
    }

    #[test]
    fn resilience_reports_snapshot_data_gap() {
        let mut data = healthy_data();
        data.as_object_mut().unwrap().remove("latest_snapshot");
        let r = fixture("vol-nosnap", json!({"owner": "infra"}), data, 1, now());
        let report = evaluate_ebs_fleet(&[r], Pillar::Resilience, now());
        assert_eq!(
            report
                .findings
                .iter()
                .map(|f| f.reason_code.as_str())
                .collect::<Vec<_>>(),
            vec![REASON_RES_SNAPSHOT_DATA_NOT_COLLECTED]
        );
    }

    #[test]
    fn stale_inventory_is_reported_as_failure_path() {
        let r = fixture(
            "vol-stale",
            json!({"owner": "infra", "project": "mayyam"}),
            healthy_data(),
            48,
            now(),
        );
        let report = evaluate_ebs_fleet(&[r], Pillar::Cost, now());
        assert_eq!(report.stale_resources, 1);
        assert!(report
            .findings
            .iter()
            .any(|f| f.reason_code == REASON_INV_STALE_DATA));
    }
}

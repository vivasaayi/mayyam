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

// Deterministic EFS inventory evaluators for the cost, security, and
// resilience pillars (roadmap rows 01-AWS-CLOUD-00757/00766/00793).
//
// Evaluates fields persisted by efs_control_plane: life_cycle_state,
// encrypted, size_in_bytes (standard vs IA), number_of_mount_targets.
// The collector does not gather tags for EFS; that gap is reported
// deterministically instead of being treated as "untagged".

use chrono::{DateTime, Utc};
use serde_json::json;

use crate::models::aws_resource::Model as AwsResourceModel;
use crate::services::aws::inventory::types::{
    check_stale, score_pillar, InventoryFinding, Pillar, PillarReport, Severity,
};

// Reason codes are the stable contract for findings; never reuse or rename.
pub const REASON_COST_TAG_DATA_NOT_COLLECTED: &str = "EFS_COST_TAG_DATA_NOT_COLLECTED";
pub const REASON_COST_NO_IA_USAGE: &str = "EFS_COST_NO_IA_USAGE";
pub const REASON_SEC_UNENCRYPTED: &str = "EFS_SEC_UNENCRYPTED";
pub const REASON_RES_NO_MOUNT_TARGETS: &str = "EFS_RES_NO_MOUNT_TARGETS";
pub const REASON_RES_NOT_AVAILABLE: &str = "EFS_RES_NOT_AVAILABLE";
pub const REASON_INV_STALE_DATA: &str = "EFS_INV_STALE_DATA";

/// Evaluate every EFS file system in the fleet for one pillar.
pub fn evaluate_efs_fleet(
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
    // The EFS collector persists an empty tag map (tags need a separate API
    // call). Report the gap instead of flagging every file system untagged.
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
            reason_code: REASON_COST_TAG_DATA_NOT_COLLECTED.to_string(),
            severity: Severity::Medium,
            message: format!(
                "Tags for file system {} are not collected yet; cost allocation cannot be assessed",
                resource.resource_id
            ),
            evidence: json!({ "tags": resource.tags }),
        });
    }

    let size = resource.resource_data.get("size_in_bytes");
    let standard = size
        .and_then(|s| s.get("value_in_standard"))
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    let ia = size
        .and_then(|s| s.get("value_in_ia"))
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    if standard > 0 && ia == 0 {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Cost,
            reason_code: REASON_COST_NO_IA_USAGE.to_string(),
            severity: Severity::Low,
            message: format!(
                "File system {} stores all data in Standard; a lifecycle policy moving cold data to IA cuts storage cost up to 92%",
                resource.resource_id
            ),
            evidence: json!({
                "value_in_standard": standard,
                "value_in_ia": ia,
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
            message: format!(
                "File system {} is not encrypted at rest",
                resource.resource_id
            ),
            evidence: json!({
                "encrypted": resource.resource_data.get("encrypted"),
                "kms_key_id": resource.resource_data.get("kms_key_id"),
            }),
        });
    }
}

fn evaluate_resilience(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    let mount_targets = resource
        .resource_data
        .get("number_of_mount_targets")
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    if mount_targets == 0 {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Resilience,
            reason_code: REASON_RES_NO_MOUNT_TARGETS.to_string(),
            severity: Severity::Medium,
            message: format!(
                "File system {} has no mount targets; it is unreachable and may be abandoned",
                resource.resource_id
            ),
            evidence: json!({ "number_of_mount_targets": mount_targets }),
        });
    }

    if let Some(state) = resource
        .resource_data
        .get("life_cycle_state")
        .and_then(|v| v.as_str())
    {
        if state != "available" {
            findings.push(InventoryFinding {
                resource_id: resource.resource_id.clone(),
                arn: resource.arn.clone(),
                pillar: Pillar::Resilience,
                reason_code: REASON_RES_NOT_AVAILABLE.to_string(),
                severity: Severity::Medium,
                message: format!(
                    "File system {} is in state '{}'; it is not serving traffic normally",
                    resource.resource_id, state
                ),
                evidence: json!({ "life_cycle_state": state }),
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
            resource_type: "EfsFileSystem".to_string(),
            resource_id: resource_id.to_string(),
            arn: format!(
                "arn:aws:elasticfilesystem:us-east-1:123456789012:file-system/{}",
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
            "life_cycle_state": "available",
            "encrypted": true,
            "number_of_mount_targets": 2,
            "size_in_bytes": {"value": 1000, "value_in_standard": 600, "value_in_ia": 400},
        })
    }

    #[test]
    fn cost_reports_tag_gap_and_no_ia_usage() {
        let mut data = healthy_data();
        data["size_in_bytes"] = json!({"value": 1000, "value_in_standard": 1000, "value_in_ia": 0});
        let r = fixture("fs-cold", json!({}), data, 1, now());
        let report = evaluate_efs_fleet(&[r], Pillar::Cost, now());
        let codes: Vec<&str> = report
            .findings
            .iter()
            .map(|f| f.reason_code.as_str())
            .collect();
        assert!(codes.contains(&REASON_COST_TAG_DATA_NOT_COLLECTED));
        assert!(codes.contains(&REASON_COST_NO_IA_USAGE));
    }

    #[test]
    fn cost_passes_for_tagged_fs_with_ia_usage() {
        let r = fixture("fs-good", json!({"team": "data"}), healthy_data(), 1, now());
        let report = evaluate_efs_fleet(&[r], Pillar::Cost, now());
        assert!(
            report.findings.is_empty(),
            "unexpected: {:?}",
            report.findings
        );
    }

    #[test]
    fn security_flags_unencrypted_file_system_as_high() {
        let mut data = healthy_data();
        data["encrypted"] = json!(false);
        let r = fixture("fs-plain", json!({"owner": "data"}), data, 1, now());
        let report = evaluate_efs_fleet(&[r], Pillar::Security, now());
        let finding = report
            .findings
            .iter()
            .find(|f| f.reason_code == REASON_SEC_UNENCRYPTED)
            .expect("unencrypted finding");
        assert_eq!(finding.severity, Severity::High);
    }

    #[test]
    fn resilience_flags_no_mount_targets_and_bad_state() {
        let r = fixture(
            "fs-island",
            json!({"owner": "data"}),
            json!({"life_cycle_state": "error", "encrypted": true, "number_of_mount_targets": 0}),
            1,
            now(),
        );
        let report = evaluate_efs_fleet(&[r], Pillar::Resilience, now());
        let codes: Vec<&str> = report
            .findings
            .iter()
            .map(|f| f.reason_code.as_str())
            .collect();
        assert!(codes.contains(&REASON_RES_NO_MOUNT_TARGETS));
        assert!(codes.contains(&REASON_RES_NOT_AVAILABLE));
    }

    #[test]
    fn resilience_passes_for_available_mounted_fs() {
        let r = fixture("fs-ok", json!({"owner": "data"}), healthy_data(), 1, now());
        let report = evaluate_efs_fleet(&[r], Pillar::Resilience, now());
        assert!(
            report.findings.is_empty(),
            "unexpected: {:?}",
            report.findings
        );
    }

    #[test]
    fn stale_inventory_is_reported_as_failure_path() {
        let r = fixture(
            "fs-stale",
            json!({"owner": "data"}),
            healthy_data(),
            48,
            now(),
        );
        let report = evaluate_efs_fleet(&[r], Pillar::Security, now());
        assert_eq!(report.stale_resources, 1);
        assert!(report
            .findings
            .iter()
            .any(|f| f.reason_code == REASON_INV_STALE_DATA));
    }
}

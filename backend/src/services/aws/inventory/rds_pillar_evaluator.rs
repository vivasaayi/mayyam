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

// Deterministic RDS inventory evaluators for the cost, security, and
// resilience pillars (roadmap rows 01-AWS-CLOUD-01135/01144/01171).
//
// Evaluates fields persisted by rds_control_plane: engine, instance_class,
// allocated_storage, storage_type, multi_az, backup_retention_period.

use chrono::{DateTime, Utc};
use serde_json::json;

use crate::models::aws_resource::Model as AwsResourceModel;
use crate::services::aws::inventory::types::{
    check_stale, data_str, has_any_tag, score_pillar, InventoryFinding, Pillar, PillarReport,
    Severity, COST_ALLOCATION_TAG_KEYS, OWNER_TAG_KEYS,
};

// Reason codes are the stable contract for findings; never reuse or rename.
pub const REASON_COST_MISSING_ALLOCATION_TAGS: &str = "RDS_COST_MISSING_ALLOCATION_TAGS";
pub const REASON_COST_GP2_STORAGE: &str = "RDS_COST_GP2_STORAGE";
pub const REASON_SEC_MISSING_OWNER_TAG: &str = "RDS_SEC_MISSING_OWNER_TAG";
pub const REASON_SEC_ACCESS_DATA_NOT_COLLECTED: &str = "RDS_SEC_ACCESS_DATA_NOT_COLLECTED";
pub const REASON_RES_SINGLE_AZ: &str = "RDS_RES_SINGLE_AZ";
pub const REASON_RES_BACKUPS_DISABLED: &str = "RDS_RES_BACKUPS_DISABLED";
pub const REASON_INV_STALE_DATA: &str = "RDS_INV_STALE_DATA";

/// Evaluate every RDS instance in the fleet for one pillar.
pub fn evaluate_rds_fleet(
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
                "DB instance {} has no cost allocation tag (expected one of: {})",
                resource.resource_id,
                COST_ALLOCATION_TAG_KEYS.join(", ")
            ),
            evidence: json!({ "tags": resource.tags }),
        });
    }

    if data_str(&resource.resource_data, "storage_type").as_deref() == Some("gp2") {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Cost,
            reason_code: REASON_COST_GP2_STORAGE.to_string(),
            severity: Severity::Low,
            message: format!(
                "DB instance {} uses gp2 storage; gp3 offers the same baseline performance at lower cost",
                resource.resource_id
            ),
            evidence: json!({
                "storage_type": "gp2",
                "allocated_storage": resource.resource_data.get("allocated_storage"),
            }),
        });
    }
}

fn evaluate_security(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    // Public accessibility and encryption state are not collected yet;
    // surface the gap deterministically instead of scoring blind.
    let has_public_access_data = resource.resource_data.get("publicly_accessible").is_some();
    let has_encryption_data = resource.resource_data.get("storage_encrypted").is_some();
    if !has_public_access_data || !has_encryption_data {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Security,
            reason_code: REASON_SEC_ACCESS_DATA_NOT_COLLECTED.to_string(),
            severity: Severity::Medium,
            message: format!(
                "DB instance {} security posture (public accessibility, storage encryption) is not collected yet; security pillar cannot be fully assessed",
                resource.resource_id
            ),
            evidence: json!({
                "publicly_accessible_collected": has_public_access_data,
                "storage_encrypted_collected": has_encryption_data,
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
                "DB instance {} has no owner/team tag; security findings cannot be routed to an owner",
                resource.resource_id
            ),
            evidence: json!({ "tags": resource.tags }),
        });
    }
}

fn evaluate_resilience(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    let multi_az = resource
        .resource_data
        .get("multi_az")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    if !multi_az {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Resilience,
            reason_code: REASON_RES_SINGLE_AZ.to_string(),
            severity: Severity::Medium,
            message: format!(
                "DB instance {} is not Multi-AZ; an AZ outage causes downtime until manual recovery",
                resource.resource_id
            ),
            evidence: json!({
                "multi_az": resource.resource_data.get("multi_az"),
                "availability_zone": resource.resource_data.get("availability_zone"),
            }),
        });
    }

    let retention = resource
        .resource_data
        .get("backup_retention_period")
        .and_then(|v| v.as_i64());
    if retention == Some(0) {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Resilience,
            reason_code: REASON_RES_BACKUPS_DISABLED.to_string(),
            severity: Severity::High,
            message: format!(
                "DB instance {} has automated backups disabled (retention 0 days); point-in-time recovery is impossible",
                resource.resource_id
            ),
            evidence: json!({ "backup_retention_period": 0 }),
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
            resource_type: "RdsInstance".to_string(),
            resource_id: resource_id.to_string(),
            arn: format!("arn:aws:rds:us-east-1:123456789012:db:{}", resource_id),
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
            "engine": "postgres",
            "instance_class": "db.r6g.large",
            "storage_type": "gp3",
            "multi_az": true,
            "backup_retention_period": 7,
            "publicly_accessible": false,
            "storage_encrypted": true,
            "availability_zone": "us-east-1a",
        })
    }

    #[test]
    fn cost_flags_missing_tags_and_gp2_storage() {
        let mut data = healthy_data();
        data["storage_type"] = json!("gp2");
        let r = fixture("db-untagged", json!({}), data, 1, now());
        let report = evaluate_rds_fleet(&[r], Pillar::Cost, now());
        let codes: Vec<&str> = report
            .findings
            .iter()
            .map(|f| f.reason_code.as_str())
            .collect();
        assert!(codes.contains(&REASON_COST_MISSING_ALLOCATION_TAGS));
        assert!(codes.contains(&REASON_COST_GP2_STORAGE));
    }

    #[test]
    fn cost_passes_for_tagged_gp3_instance() {
        let r = fixture(
            "db-good",
            json!({"team": "payments"}),
            healthy_data(),
            1,
            now(),
        );
        let report = evaluate_rds_fleet(&[r], Pillar::Cost, now());
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
            "db-gap",
            json!({"owner": "dba"}),
            json!({"engine": "mysql", "multi_az": true, "backup_retention_period": 7}),
            1,
            now(),
        );
        let report = evaluate_rds_fleet(&[r], Pillar::Security, now());
        let gap = report
            .findings
            .iter()
            .find(|f| f.reason_code == REASON_SEC_ACCESS_DATA_NOT_COLLECTED)
            .expect("data gap finding");
        assert_eq!(gap.evidence["publicly_accessible_collected"], json!(false));
    }

    #[test]
    fn security_passes_when_posture_collected_and_owned() {
        let r = fixture("db-ok", json!({"owner": "dba"}), healthy_data(), 1, now());
        let report = evaluate_rds_fleet(&[r], Pillar::Security, now());
        assert!(
            report.findings.is_empty(),
            "unexpected: {:?}",
            report.findings
        );
    }

    #[test]
    fn resilience_flags_single_az_and_disabled_backups() {
        let mut data = healthy_data();
        data["multi_az"] = json!(false);
        data["backup_retention_period"] = json!(0);
        let r = fixture("db-fragile", json!({"owner": "dba"}), data, 1, now());
        let report = evaluate_rds_fleet(&[r], Pillar::Resilience, now());
        let codes: Vec<&str> = report
            .findings
            .iter()
            .map(|f| f.reason_code.as_str())
            .collect();
        assert!(codes.contains(&REASON_RES_SINGLE_AZ));
        assert!(codes.contains(&REASON_RES_BACKUPS_DISABLED));
        let backups = report
            .findings
            .iter()
            .find(|f| f.reason_code == REASON_RES_BACKUPS_DISABLED)
            .unwrap();
        assert_eq!(backups.severity, Severity::High);
    }

    #[test]
    fn stale_inventory_is_reported_as_failure_path() {
        let r = fixture(
            "db-stale",
            json!({"owner": "dba", "project": "mayyam"}),
            healthy_data(),
            48,
            now(),
        );
        let report = evaluate_rds_fleet(&[r], Pillar::Resilience, now());
        assert_eq!(report.stale_resources, 1);
        assert!(report
            .findings
            .iter()
            .any(|f| f.reason_code == REASON_INV_STALE_DATA));
    }
}

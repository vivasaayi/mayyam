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

// Deterministic FSx file system inventory evaluators for the cost, resilience,
// and security pillars (roadmap rows 01-AWS-CLOUD-00820/00829/00856).

use chrono::{DateTime, Utc};
use serde_json::{json, Value};

use crate::models::aws_resource::Model as AwsResourceModel;
use crate::services::aws::inventory::types::{
    check_stale, score_pillar, InventoryFinding, Pillar, PillarReport, Severity,
};

pub const RESOURCE_TYPE: &str = "FsxFileSystem";

pub const REASON_COST_NO_TAGS: &str = "FSX_COST_NO_TAGS";
pub const REASON_COST_BACKUP_TAGS_NOT_COPIED: &str = "FSX_COST_BACKUP_TAGS_NOT_COPIED";
pub const REASON_RES_NOT_AVAILABLE: &str = "FSX_RES_NOT_AVAILABLE";
pub const REASON_RES_SINGLE_AZ: &str = "FSX_RES_SINGLE_AZ";
pub const REASON_RES_LOW_BACKUP_RETENTION: &str = "FSX_RES_LOW_BACKUP_RETENTION";
pub const REASON_SEC_NO_CUSTOMER_KMS: &str = "FSX_SEC_NO_CUSTOMER_KMS";
pub const REASON_SEC_PUBLIC_NETWORK_TYPE: &str = "FSX_SEC_PUBLIC_NETWORK_TYPE";
pub const REASON_SEC_VOLUME_TAGS_NOT_COPIED: &str = "FSX_SEC_VOLUME_TAGS_NOT_COPIED";
pub const REASON_INV_STALE_DATA: &str = "FSX_INV_STALE_DATA";

const MIN_BACKUP_RETENTION_DAYS: i64 = 7;

pub fn evaluate_fsx_fleet(
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
            Pillar::Resilience => evaluate_resilience(resource, &mut findings),
            Pillar::Security => evaluate_security(resource, &mut findings),
            _ => {}
        }
    }

    PillarReport {
        pillar,
        resources_evaluated: evaluated,
        stale_resources,
        score: score_pillar(&findings),
        findings,
    }
}

fn data_bool(resource_data: &Value, key: &str) -> Option<bool> {
    resource_data.get(key).and_then(|v| v.as_bool())
}

fn data_i64(resource_data: &Value, key: &str) -> Option<i64> {
    resource_data.get(key).and_then(|v| v.as_i64())
}

fn data_str<'a>(resource_data: &'a Value, key: &str) -> Option<&'a str> {
    resource_data.get(key).and_then(|v| v.as_str())
}

fn tags_empty(resource: &AwsResourceModel) -> bool {
    resource
        .tags
        .as_object()
        .map(|m| m.is_empty())
        .unwrap_or(true)
}

fn evaluate_cost(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    if tags_empty(resource) {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Cost,
            reason_code: REASON_COST_NO_TAGS.to_string(),
            severity: Severity::Medium,
            message: format!(
                "FSx file system {} has no tags; storage and throughput cost allocation cannot be assessed",
                resource.resource_id
            ),
            evidence: json!({ "tags": resource.tags }),
        });
    }

    if data_bool(&resource.resource_data, "copy_tags_to_backups") == Some(false) {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Cost,
            reason_code: REASON_COST_BACKUP_TAGS_NOT_COPIED.to_string(),
            severity: Severity::Low,
            message: format!(
                "FSx file system {} does not copy tags to backups; backup spend may be unattributed",
                resource.resource_id
            ),
            evidence: json!({ "copy_tags_to_backups": false }),
        });
    }
}

fn evaluate_resilience(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    if let Some(lifecycle) = data_str(&resource.resource_data, "lifecycle") {
        if lifecycle != "AVAILABLE" {
            findings.push(InventoryFinding {
                resource_id: resource.resource_id.clone(),
                arn: resource.arn.clone(),
                pillar: Pillar::Resilience,
                reason_code: REASON_RES_NOT_AVAILABLE.to_string(),
                severity: Severity::High,
                message: format!(
                    "FSx file system {} is in lifecycle '{}', not AVAILABLE",
                    resource.resource_id, lifecycle
                ),
                evidence: json!({ "lifecycle": lifecycle }),
            });
        }
    }

    if !data_bool(&resource.resource_data, "multi_az").unwrap_or(false) {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Resilience,
            reason_code: REASON_RES_SINGLE_AZ.to_string(),
            severity: Severity::Medium,
            message: format!(
                "FSx file system {} is not recorded as Multi-AZ",
                resource.resource_id
            ),
            evidence: json!({
                "multi_az": false,
                "deployment_type": data_str(&resource.resource_data, "deployment_type"),
                "subnet_count": data_i64(&resource.resource_data, "subnet_count"),
            }),
        });
    }

    if let Some(retention) = data_i64(&resource.resource_data, "automatic_backup_retention_days") {
        if retention < MIN_BACKUP_RETENTION_DAYS {
            findings.push(InventoryFinding {
                resource_id: resource.resource_id.clone(),
                arn: resource.arn.clone(),
                pillar: Pillar::Resilience,
                reason_code: REASON_RES_LOW_BACKUP_RETENTION.to_string(),
                severity: Severity::Medium,
                message: format!(
                    "FSx file system {} backup retention is {} day(s); increase to at least {} days",
                    resource.resource_id, retention, MIN_BACKUP_RETENTION_DAYS
                ),
                evidence: json!({
                    "automatic_backup_retention_days": retention,
                    "minimum_recommended": MIN_BACKUP_RETENTION_DAYS,
                }),
            });
        }
    }
}

fn evaluate_security(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    if !data_bool(&resource.resource_data, "customer_managed_kms_key").unwrap_or(false) {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Security,
            reason_code: REASON_SEC_NO_CUSTOMER_KMS.to_string(),
            severity: Severity::Low,
            message: format!(
                "FSx file system {} does not use a customer-managed KMS key",
                resource.resource_id
            ),
            evidence: json!({ "kms_key_id": resource.resource_data.get("kms_key_id") }),
        });
    }

    if data_str(&resource.resource_data, "network_type") == Some("DUAL") {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Security,
            reason_code: REASON_SEC_PUBLIC_NETWORK_TYPE.to_string(),
            severity: Severity::Medium,
            message: format!(
                "FSx file system {} has dual-stack networking recorded; verify exposure and security group scope",
                resource.resource_id
            ),
            evidence: json!({ "network_type": "DUAL" }),
        });
    }

    if data_bool(&resource.resource_data, "copy_tags_to_volumes") == Some(false) {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Security,
            reason_code: REASON_SEC_VOLUME_TAGS_NOT_COPIED.to_string(),
            severity: Severity::Low,
            message: format!(
                "FSx file system {} does not copy tags to child volumes; ownership and policy labels can be lost",
                resource.resource_id
            ),
            evidence: json!({ "copy_tags_to_volumes": false }),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;
    use uuid::Uuid;

    fn fixture(resource_data: Value, tags: Value, now: DateTime<Utc>) -> AwsResourceModel {
        AwsResourceModel {
            id: Uuid::new_v4(),
            sync_id: None,
            account_id: "123456789012".to_string(),
            profile: None,
            region: "us-east-1".to_string(),
            resource_type: RESOURCE_TYPE.to_string(),
            resource_id: "fs-123".to_string(),
            arn: "arn:aws:fsx:us-east-1:123456789012:file-system/fs-123".to_string(),
            name: Some("fs-123".to_string()),
            tags,
            resource_data,
            created_at: now,
            updated_at: now,
            last_refreshed: now - Duration::hours(1),
        }
    }

    #[test]
    fn evaluates_fsx_inventory_findings() {
        let now = Utc::now();
        let resources = vec![fixture(
            json!({
                "lifecycle": "MISCONFIGURED",
                "multi_az": false,
                "automatic_backup_retention_days": 1,
                "copy_tags_to_backups": false,
                "copy_tags_to_volumes": false,
                "customer_managed_kms_key": false,
                "network_type": "DUAL"
            }),
            json!({}),
            now,
        )];

        let cost = evaluate_fsx_fleet(&resources, Pillar::Cost, now);
        assert!(cost
            .findings
            .iter()
            .any(|f| f.reason_code == REASON_COST_NO_TAGS));
        assert!(cost
            .findings
            .iter()
            .any(|f| f.reason_code == REASON_COST_BACKUP_TAGS_NOT_COPIED));

        let resilience = evaluate_fsx_fleet(&resources, Pillar::Resilience, now);
        assert!(resilience
            .findings
            .iter()
            .any(|f| f.reason_code == REASON_RES_NOT_AVAILABLE));
        assert!(resilience
            .findings
            .iter()
            .any(|f| f.reason_code == REASON_RES_LOW_BACKUP_RETENTION));

        let security = evaluate_fsx_fleet(&resources, Pillar::Security, now);
        assert!(security
            .findings
            .iter()
            .any(|f| f.reason_code == REASON_SEC_NO_CUSTOMER_KMS));
        assert!(security
            .findings
            .iter()
            .any(|f| f.reason_code == REASON_SEC_VOLUME_TAGS_NOT_COPIED));
    }
}

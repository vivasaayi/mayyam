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

// Deterministic Kinesis Data Firehose inventory evaluators for the cost,
// resilience, and security pillars (roadmap rows
// 01-AWS-CLOUD-02017/02026/02053).

use chrono::{DateTime, Utc};
use serde_json::{json, Value};

use crate::models::aws_resource::Model as AwsResourceModel;
use crate::services::aws::inventory::types::{
    check_stale, has_any_tag, score_pillar, InventoryFinding, Pillar, PillarReport, Severity,
    COST_ALLOCATION_TAG_KEYS,
};

pub const RESOURCE_TYPE: &str = "FirehoseDeliveryStream";

pub const REASON_COST_NO_TAGS: &str = "FIREHOSE_COST_NO_TAGS";
pub const REASON_COST_ALL_DATA_BACKUP: &str = "FIREHOSE_COST_ALL_DATA_BACKUP";
pub const REASON_RES_STREAM_NOT_ACTIVE: &str = "FIREHOSE_RES_STREAM_NOT_ACTIVE";
pub const REASON_RES_NO_DESTINATION: &str = "FIREHOSE_RES_NO_DESTINATION";
pub const REASON_RES_FAILURE_REPORTED: &str = "FIREHOSE_RES_FAILURE_REPORTED";
pub const REASON_SEC_ENCRYPTION_DISABLED: &str = "FIREHOSE_SEC_ENCRYPTION_DISABLED";
pub const REASON_SEC_LOGGING_DISABLED: &str = "FIREHOSE_SEC_LOGGING_DISABLED";
pub const REASON_INV_STALE_DATA: &str = "FIREHOSE_INV_STALE_DATA";

pub fn evaluate_firehose_fleet(
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

fn data_str<'a>(resource_data: &'a Value, key: &str) -> Option<&'a str> {
    resource_data.get(key).and_then(|v| v.as_str())
}

fn data_bool(resource_data: &Value, key: &str) -> Option<bool> {
    resource_data.get(key).and_then(|v| v.as_bool())
}

fn data_i64(resource_data: &Value, key: &str) -> Option<i64> {
    resource_data.get(key).and_then(|v| v.as_i64())
}

fn normalized_data_str(resource_data: &Value, key: &str) -> Option<String> {
    data_str(resource_data, key).map(|s| s.trim().to_ascii_uppercase().replace('-', "_"))
}

fn evaluate_cost(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    if !has_any_tag(&resource.tags, COST_ALLOCATION_TAG_KEYS) {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Cost,
            reason_code: REASON_COST_NO_TAGS.to_string(),
            severity: Severity::Medium,
            message: format!(
                "Firehose delivery stream {} has no cost allocation tags",
                resource.resource_id
            ),
            evidence: json!({ "tags": resource.tags }),
        });
    }

    if normalized_data_str(&resource.resource_data, "s3_backup_mode").as_deref() == Some("ALL_DATA")
    {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Cost,
            reason_code: REASON_COST_ALL_DATA_BACKUP.to_string(),
            severity: Severity::Low,
            message: format!(
                "Firehose delivery stream {} backs up all records to S3; verify duplicate storage is intentional",
                resource.resource_id
            ),
            evidence: json!({ "s3_backup_mode": data_str(&resource.resource_data, "s3_backup_mode") }),
        });
    }
}

fn evaluate_resilience(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    match normalized_data_str(&resource.resource_data, "delivery_stream_status").as_deref() {
        Some("ACTIVE") => {}
        Some(status) => findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Resilience,
            reason_code: REASON_RES_STREAM_NOT_ACTIVE.to_string(),
            severity: Severity::High,
            message: format!(
                "Firehose delivery stream {} is in {} state, not ACTIVE",
                resource.resource_id, status
            ),
            evidence: json!({ "delivery_stream_status": status }),
        }),
        None => findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Resilience,
            reason_code: REASON_RES_STREAM_NOT_ACTIVE.to_string(),
            severity: Severity::Medium,
            message: format!(
                "Firehose delivery stream {} status is not collected",
                resource.resource_id
            ),
            evidence: json!({ "delivery_stream_status": null }),
        }),
    }

    if data_i64(&resource.resource_data, "destinations_count").unwrap_or(0) == 0 {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Resilience,
            reason_code: REASON_RES_NO_DESTINATION.to_string(),
            severity: Severity::High,
            message: format!(
                "Firehose delivery stream {} has no destination configured in inventory",
                resource.resource_id
            ),
            evidence: json!({ "destinations_count": resource.resource_data.get("destinations_count") }),
        });
    }

    if resource.resource_data.get("failure_description").is_some() {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Resilience,
            reason_code: REASON_RES_FAILURE_REPORTED.to_string(),
            severity: Severity::High,
            message: format!(
                "Firehose delivery stream {} reports a delivery failure",
                resource.resource_id
            ),
            evidence: json!({ "failure_description": resource.resource_data.get("failure_description") }),
        });
    }
}

fn evaluate_security(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    match normalized_data_str(&resource.resource_data, "server_side_encryption_status").as_deref() {
        Some("ENABLED") | Some("ENABLING") => {}
        status => findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Security,
            reason_code: REASON_SEC_ENCRYPTION_DISABLED.to_string(),
            severity: Severity::Medium,
            message: format!(
                "Firehose delivery stream {} does not report enabled server-side encryption",
                resource.resource_id
            ),
            evidence: json!({ "server_side_encryption_status": status }),
        }),
    }

    if !data_bool(&resource.resource_data, "cloudwatch_logging_enabled").unwrap_or(false) {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Security,
            reason_code: REASON_SEC_LOGGING_DISABLED.to_string(),
            severity: Severity::Medium,
            message: format!(
                "Firehose delivery stream {} has no CloudWatch logging enabled in destination config",
                resource.resource_id
            ),
            evidence: json!({
                "cloudwatch_logging_enabled": resource.resource_data.get("cloudwatch_logging_enabled"),
            }),
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
            resource_id: "orders-stream".to_string(),
            arn: "arn:aws:firehose:us-east-1:123456789012:deliverystream/orders-stream".to_string(),
            name: Some("orders-stream".to_string()),
            tags,
            resource_data,
            created_at: now,
            updated_at: now,
            last_refreshed: now - Duration::hours(1),
        }
    }

    #[test]
    fn evaluates_firehose_inventory_findings() {
        let now = Utc::now();
        let resources = vec![fixture(
            json!({
                "delivery_stream_status": "CREATING",
                "destinations_count": 0,
                "failure_description": "destination unavailable",
                "server_side_encryption_status": "DISABLED",
                "cloudwatch_logging_enabled": false,
                "s3_backup_mode": "ALL_DATA",
            }),
            json!({}),
            now,
        )];

        let cost = evaluate_firehose_fleet(&resources, Pillar::Cost, now);
        assert_eq!(cost.resources_evaluated, 1);
        assert!(cost
            .findings
            .iter()
            .any(|f| f.reason_code == REASON_COST_NO_TAGS));
        assert!(cost
            .findings
            .iter()
            .any(|f| f.reason_code == REASON_COST_ALL_DATA_BACKUP));

        let resilience = evaluate_firehose_fleet(&resources, Pillar::Resilience, now);
        assert!(resilience
            .findings
            .iter()
            .any(|f| f.reason_code == REASON_RES_STREAM_NOT_ACTIVE));
        assert!(resilience
            .findings
            .iter()
            .any(|f| f.reason_code == REASON_RES_NO_DESTINATION));
        assert!(resilience
            .findings
            .iter()
            .any(|f| f.reason_code == REASON_RES_FAILURE_REPORTED));

        let security = evaluate_firehose_fleet(&resources, Pillar::Security, now);
        assert!(security
            .findings
            .iter()
            .any(|f| f.reason_code == REASON_SEC_ENCRYPTION_DISABLED));
        assert!(security
            .findings
            .iter()
            .any(|f| f.reason_code == REASON_SEC_LOGGING_DISABLED));
    }
}

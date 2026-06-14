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

// Deterministic DataSync task inventory evaluators for the cost, resilience,
// and security pillars (roadmap rows 01-AWS-CLOUD-01072/01081/01108).

use chrono::{DateTime, Utc};
use serde_json::{json, Value};

use crate::models::aws_resource::Model as AwsResourceModel;
use crate::services::aws::inventory::types::{
    check_stale, score_pillar, InventoryFinding, Pillar, PillarReport, Severity,
};

pub const RESOURCE_TYPE: &str = "DataSyncTask";

pub const REASON_COST_NO_TAGS: &str = "DATASYNC_COST_NO_TAGS";
pub const REASON_COST_SCHEDULED_FULL_TRANSFER: &str = "DATASYNC_COST_SCHEDULED_FULL_TRANSFER";
pub const REASON_RES_TASK_ERROR: &str = "DATASYNC_RES_TASK_ERROR";
pub const REASON_RES_VERIFY_DISABLED: &str = "DATASYNC_RES_VERIFY_DISABLED";
pub const REASON_RES_QUEUEING_DISABLED: &str = "DATASYNC_RES_QUEUEING_DISABLED";
pub const REASON_SEC_NO_LOGS: &str = "DATASYNC_SEC_NO_LOGS";
pub const REASON_SEC_LOGGING_OFF: &str = "DATASYNC_SEC_LOGGING_OFF";
pub const REASON_SEC_OBJECT_TAGS_NOT_PRESERVED: &str = "DATASYNC_SEC_OBJECT_TAGS_NOT_PRESERVED";
pub const REASON_INV_STALE_DATA: &str = "DATASYNC_INV_STALE_DATA";

pub fn evaluate_datasync_fleet(
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
                "DataSync task {} has no tags; transfer cost allocation cannot be assessed",
                resource.resource_id
            ),
            evidence: json!({ "tags": resource.tags }),
        });
    }

    if data_bool(&resource.resource_data, "has_schedule").unwrap_or(false)
        && data_str(&resource.resource_data, "transfer_mode") == Some("ALL")
    {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Cost,
            reason_code: REASON_COST_SCHEDULED_FULL_TRANSFER.to_string(),
            severity: Severity::Medium,
            message: format!(
                "DataSync task {} is scheduled and transfers all files each run; incremental mode can reduce repeated transfer charges",
                resource.resource_id
            ),
            evidence: json!({
                "has_schedule": true,
                "transfer_mode": "ALL",
            }),
        });
    }
}

fn evaluate_resilience(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    if resource.resource_data.get("error_code").is_some() {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Resilience,
            reason_code: REASON_RES_TASK_ERROR.to_string(),
            severity: Severity::High,
            message: format!(
                "DataSync task {} reports an error; investigate before relying on transfer recovery",
                resource.resource_id
            ),
            evidence: json!({
                "error_code": resource.resource_data.get("error_code"),
                "error_detail": resource.resource_data.get("error_detail"),
            }),
        });
    }

    if data_str(&resource.resource_data, "verify_mode") == Some("NONE") {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Resilience,
            reason_code: REASON_RES_VERIFY_DISABLED.to_string(),
            severity: Severity::High,
            message: format!(
                "DataSync task {} disables final verification; transfer integrity cannot be confirmed",
                resource.resource_id
            ),
            evidence: json!({ "verify_mode": "NONE" }),
        });
    }

    if data_str(&resource.resource_data, "task_queueing") == Some("DISABLED") {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Resilience,
            reason_code: REASON_RES_QUEUEING_DISABLED.to_string(),
            severity: Severity::Medium,
            message: format!(
                "DataSync task {} has queueing disabled; concurrent executions may fail instead of waiting",
                resource.resource_id
            ),
            evidence: json!({ "task_queueing": "DISABLED" }),
        });
    }
}

fn evaluate_security(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    if !data_bool(&resource.resource_data, "has_cloudwatch_logs").unwrap_or(false) {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Security,
            reason_code: REASON_SEC_NO_LOGS.to_string(),
            severity: Severity::Medium,
            message: format!(
                "DataSync task {} has no CloudWatch Logs group configured for transfer audit evidence",
                resource.resource_id
            ),
            evidence: json!({ "has_cloudwatch_logs": false }),
        });
    }

    if data_str(&resource.resource_data, "log_level") == Some("OFF") {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Security,
            reason_code: REASON_SEC_LOGGING_OFF.to_string(),
            severity: Severity::Medium,
            message: format!(
                "DataSync task {} has logging disabled",
                resource.resource_id
            ),
            evidence: json!({ "log_level": "OFF" }),
        });
    }

    if data_str(&resource.resource_data, "object_tags") == Some("NONE") {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Security,
            reason_code: REASON_SEC_OBJECT_TAGS_NOT_PRESERVED.to_string(),
            severity: Severity::Low,
            message: format!(
                "DataSync task {} does not preserve object tags; downstream ownership and security labels can be lost",
                resource.resource_id
            ),
            evidence: json!({ "object_tags": "NONE" }),
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
            resource_id: "task-123".to_string(),
            arn: "arn:aws:datasync:us-east-1:123456789012:task/task-123".to_string(),
            name: Some("copy".to_string()),
            tags,
            resource_data,
            created_at: now,
            updated_at: now,
            last_refreshed: now - Duration::hours(1),
        }
    }

    #[test]
    fn evaluates_datasync_inventory_findings() {
        let now = Utc::now();
        let resources = vec![fixture(
            json!({
                "has_schedule": true,
                "transfer_mode": "ALL",
                "verify_mode": "NONE",
                "task_queueing": "DISABLED",
                "error_code": "InternalError",
                "has_cloudwatch_logs": false,
                "log_level": "OFF",
                "object_tags": "NONE"
            }),
            json!({}),
            now,
        )];

        let cost = evaluate_datasync_fleet(&resources, Pillar::Cost, now);
        assert!(cost
            .findings
            .iter()
            .any(|f| f.reason_code == REASON_COST_SCHEDULED_FULL_TRANSFER));

        let resilience = evaluate_datasync_fleet(&resources, Pillar::Resilience, now);
        assert!(resilience
            .findings
            .iter()
            .any(|f| f.reason_code == REASON_RES_VERIFY_DISABLED));
        assert!(resilience
            .findings
            .iter()
            .any(|f| f.reason_code == REASON_RES_TASK_ERROR));

        let security = evaluate_datasync_fleet(&resources, Pillar::Security, now);
        assert!(security
            .findings
            .iter()
            .any(|f| f.reason_code == REASON_SEC_NO_LOGS));
        assert!(security
            .findings
            .iter()
            .any(|f| f.reason_code == REASON_SEC_OBJECT_TAGS_NOT_PRESERVED));
    }
}

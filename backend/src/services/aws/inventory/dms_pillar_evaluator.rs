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

// Deterministic AWS DMS inventory evaluators for the cost, resilience, and
// security pillars (roadmap rows 01-AWS-CLOUD-05104/05113/05140).

use chrono::{DateTime, Utc};
use serde_json::{json, Value};

use crate::models::aws_resource::Model as AwsResourceModel;
use crate::services::aws::inventory::types::{
    check_stale, has_any_tag, score_pillar, InventoryFinding, Pillar, PillarReport, Severity,
    COST_ALLOCATION_TAG_KEYS,
};

pub const RESOURCE_TYPE: &str = "DmsResource";

pub const REASON_COST_NO_TAGS: &str = "DMS_COST_NO_TAGS";
pub const REASON_COST_IDLE_REPLICATION_INSTANCE: &str = "DMS_COST_IDLE_REPLICATION_INSTANCE";
pub const REASON_RES_SINGLE_AZ_INSTANCE: &str = "DMS_RES_SINGLE_AZ_INSTANCE";
pub const REASON_RES_UNHEALTHY_STATUS: &str = "DMS_RES_UNHEALTHY_STATUS";
pub const REASON_RES_COLLECTION_ERRORS: &str = "DMS_RES_COLLECTION_ERRORS";
pub const REASON_SEC_PUBLIC_INSTANCE: &str = "DMS_SEC_PUBLIC_INSTANCE";
pub const REASON_SEC_ENDPOINT_SSL_DISABLED: &str = "DMS_SEC_ENDPOINT_SSL_DISABLED";
pub const REASON_SEC_ENDPOINT_WITHOUT_KMS: &str = "DMS_SEC_ENDPOINT_WITHOUT_KMS";
pub const REASON_SEC_TASK_LOGGING_DISABLED: &str = "DMS_SEC_TASK_LOGGING_DISABLED";
pub const REASON_INV_STALE_DATA: &str = "DMS_INV_STALE_DATA";

pub fn evaluate_dms_fleet(
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

fn data_usize(resource_data: &Value, key: &str) -> usize {
    resource_data
        .get(key)
        .and_then(|v| v.as_u64())
        .map(|v| v as usize)
        .unwrap_or(0)
}

fn resource_kind(resource: &AwsResourceModel) -> Option<&str> {
    data_str(&resource.resource_data, "resource_kind")
}

fn normalized_status(resource: &AwsResourceModel) -> Option<String> {
    data_str(&resource.resource_data, "status").map(|status| status.trim().to_ascii_lowercase())
}

fn finding(
    resource: &AwsResourceModel,
    pillar: Pillar,
    reason_code: &str,
    severity: Severity,
    message: String,
    evidence: Value,
) -> InventoryFinding {
    InventoryFinding {
        resource_id: resource.resource_id.clone(),
        arn: resource.arn.clone(),
        pillar,
        reason_code: reason_code.to_string(),
        severity,
        message,
        evidence,
    }
}

fn evaluate_cost(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    if resource_kind(resource) != Some("account_summary")
        && !has_any_tag(&resource.tags, COST_ALLOCATION_TAG_KEYS)
    {
        findings.push(finding(
            resource,
            Pillar::Cost,
            REASON_COST_NO_TAGS,
            Severity::Medium,
            format!(
                "DMS resource {} has no cost allocation tags",
                resource.resource_id
            ),
            json!({ "tags": resource.tags, "resource_kind": resource_kind(resource) }),
        ));
    }

    if resource_kind(resource) == Some("account_summary") {
        let resources_by_kind = resource.resource_data.get("resources_by_kind");
        let instance_count = resources_by_kind
            .and_then(|counts| counts.get("replication_instance"))
            .and_then(|count| count.as_u64())
            .unwrap_or(0);
        let task_count = resources_by_kind
            .and_then(|counts| counts.get("replication_task"))
            .and_then(|count| count.as_u64())
            .unwrap_or(0);
        if instance_count > 0 && task_count == 0 {
            findings.push(finding(
                resource,
                Pillar::Cost,
                REASON_COST_IDLE_REPLICATION_INSTANCE,
                Severity::Medium,
                format!(
                    "DMS account {} has replication instances but no replication tasks in inventory",
                    resource.resource_id
                ),
                json!({
                    "resources_by_kind": resources_by_kind,
                    "resource_count": resource.resource_data.get("resource_count"),
                }),
            ));
        }
    }
}

fn evaluate_resilience(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    if resource_kind(resource) == Some("account_summary") {
        let collection_errors = data_usize(&resource.resource_data, "collection_error_count");
        if collection_errors > 0 {
            findings.push(finding(
                resource,
                Pillar::Resilience,
                REASON_RES_COLLECTION_ERRORS,
                Severity::Medium,
                format!(
                    "DMS account {} had {} Cloud Control collection errors",
                    resource.resource_id, collection_errors
                ),
                json!({
                    "collection_error_count": collection_errors,
                    "collection_errors": resource.resource_data.get("collection_errors"),
                }),
            ));
        }
    }

    if resource_kind(resource) == Some("replication_instance")
        && data_bool(&resource.resource_data, "multi_az") == Some(false)
    {
        findings.push(finding(
            resource,
            Pillar::Resilience,
            REASON_RES_SINGLE_AZ_INSTANCE,
            Severity::High,
            format!(
                "DMS replication instance {} is not configured for Multi-AZ",
                resource.resource_id
            ),
            json!({
                "multi_az": false,
                "replication_instance_class": resource.resource_data.get("replication_instance_class"),
            }),
        ));
    }

    if let Some(status) = normalized_status(resource) {
        if matches!(
            status.as_str(),
            "failed" | "failure" | "stopped" | "deleting" | "incompatible-network"
        ) {
            findings.push(finding(
                resource,
                Pillar::Resilience,
                REASON_RES_UNHEALTHY_STATUS,
                Severity::High,
                format!(
                    "DMS resource {} is in unhealthy status {}",
                    resource.resource_id, status
                ),
                json!({
                    "status": status,
                    "resource_kind": resource_kind(resource),
                    "cloudcontrol_type": resource.resource_data.get("cloudcontrol_type"),
                }),
            ));
        }
    }
}

fn evaluate_security(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    if resource_kind(resource) == Some("replication_instance")
        && data_bool(&resource.resource_data, "publicly_accessible") == Some(true)
    {
        findings.push(finding(
            resource,
            Pillar::Security,
            REASON_SEC_PUBLIC_INSTANCE,
            Severity::High,
            format!(
                "DMS replication instance {} is publicly accessible",
                resource.resource_id
            ),
            json!({
                "publicly_accessible": true,
                "replication_instance_class": resource.resource_data.get("replication_instance_class"),
            }),
        ));
    }

    if resource_kind(resource) == Some("endpoint")
        && data_str(&resource.resource_data, "ssl_mode")
            .map(|mode| mode.eq_ignore_ascii_case("none"))
            .unwrap_or(false)
    {
        findings.push(finding(
            resource,
            Pillar::Security,
            REASON_SEC_ENDPOINT_SSL_DISABLED,
            Severity::High,
            format!("DMS endpoint {} has SSL disabled", resource.resource_id),
            json!({
                "ssl_mode": resource.resource_data.get("ssl_mode"),
                "endpoint_type": resource.resource_data.get("endpoint_type"),
                "engine_name": resource.resource_data.get("engine_name"),
            }),
        ));
    }

    if resource_kind(resource) == Some("endpoint")
        && data_str(&resource.resource_data, "kms_key_id").is_none()
    {
        findings.push(finding(
            resource,
            Pillar::Security,
            REASON_SEC_ENDPOINT_WITHOUT_KMS,
            Severity::Medium,
            format!(
                "DMS endpoint {} has no KMS key evidence in inventory",
                resource.resource_id
            ),
            json!({
                "kms_key_id": resource.resource_data.get("kms_key_id"),
                "endpoint_type": resource.resource_data.get("endpoint_type"),
                "engine_name": resource.resource_data.get("engine_name"),
            }),
        ));
    }

    if resource_kind(resource) == Some("replication_task")
        && data_bool(&resource.resource_data, "task_logging_enabled") == Some(false)
    {
        findings.push(finding(
            resource,
            Pillar::Security,
            REASON_SEC_TASK_LOGGING_DISABLED,
            Severity::Medium,
            format!(
                "DMS replication task {} has logging disabled",
                resource.resource_id
            ),
            json!({
                "task_logging_enabled": false,
                "migration_type": resource.resource_data.get("migration_type"),
            }),
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;
    use serde_json::json;
    use uuid::Uuid;

    fn fixture(resource_id: &str, resource_data: Value, tags: Value) -> AwsResourceModel {
        let now = Utc::now();
        AwsResourceModel {
            id: Uuid::new_v4(),
            sync_id: Some(Uuid::new_v4()),
            account_id: "123456789012".to_string(),
            profile: Some("test".to_string()),
            region: "us-east-1".to_string(),
            resource_type: RESOURCE_TYPE.to_string(),
            resource_id: resource_id.to_string(),
            arn: format!("arn:aws:dms:us-east-1:123456789012:{}", resource_id),
            name: Some(resource_id.to_string()),
            tags,
            resource_data,
            created_at: now,
            updated_at: now,
            last_refreshed: now - Duration::hours(26),
        }
    }

    #[test]
    fn evaluates_dms_inventory_findings() {
        let now = Utc::now();
        let resources = vec![
            fixture(
                "dms:123456789012",
                json!({
                    "resource_kind": "account_summary",
                    "resource_count": 1,
                    "collection_error_count": 1,
                    "collection_errors": [{ "cloudcontrol_type": "AWS::DMS::Certificate", "error": "denied" }],
                    "resources_by_kind": { "replication_instance": 1 },
                }),
                json!({}),
            ),
            fixture(
                "replication_instance/ri-1",
                json!({
                    "resource_kind": "replication_instance",
                    "multi_az": false,
                    "publicly_accessible": true,
                    "status": "available",
                    "replication_instance_class": "dms.t3.medium",
                }),
                json!({}),
            ),
            fixture(
                "endpoint/source",
                json!({
                    "resource_kind": "endpoint",
                    "ssl_mode": "none",
                    "engine_name": "postgres",
                    "endpoint_type": "source",
                }),
                json!({ "CostCenter": "platform" }),
            ),
            fixture(
                "replication_task/task-1",
                json!({
                    "resource_kind": "replication_task",
                    "status": "failed",
                    "migration_type": "full-load-and-cdc",
                    "task_logging_enabled": false,
                }),
                json!({ "CostCenter": "platform" }),
            ),
        ];

        let cost = evaluate_dms_fleet(&resources, Pillar::Cost, now);
        assert_eq!(cost.resources_evaluated, 4);
        assert_eq!(cost.stale_resources, 4);
        assert!(cost
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_COST_NO_TAGS));
        assert!(cost
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_COST_IDLE_REPLICATION_INSTANCE));

        let resilience = evaluate_dms_fleet(&resources, Pillar::Resilience, now);
        assert!(resilience
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_RES_COLLECTION_ERRORS));
        assert!(resilience
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_RES_SINGLE_AZ_INSTANCE));
        assert!(resilience
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_RES_UNHEALTHY_STATUS));

        let security = evaluate_dms_fleet(&resources, Pillar::Security, now);
        assert!(security
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_SEC_PUBLIC_INSTANCE));
        assert!(security
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_SEC_ENDPOINT_SSL_DISABLED));
        assert!(security
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_SEC_ENDPOINT_WITHOUT_KMS));
        assert!(security
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_SEC_TASK_LOGGING_DISABLED));
    }
}

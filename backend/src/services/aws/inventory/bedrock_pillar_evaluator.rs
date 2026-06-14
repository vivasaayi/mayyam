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

// Deterministic AWS Bedrock inventory evaluators for the cost, resilience,
// and security pillars.

use chrono::{DateTime, Utc};
use serde_json::{json, Value};

use crate::models::aws_resource::Model as AwsResourceModel;
use crate::services::aws::inventory::types::{
    check_stale, has_any_tag, score_pillar, InventoryFinding, Pillar, PillarReport, Severity,
    COST_ALLOCATION_TAG_KEYS,
};

pub const RESOURCE_TYPE: &str = "BedrockResource";

pub const REASON_COST_NO_TAGS: &str = "BEDROCK_COST_NO_TAGS";
pub const REASON_COST_PROVISIONED_THROUGHPUT: &str = "BEDROCK_COST_PROVISIONED_THROUGHPUT";
pub const REASON_COST_LEGACY_FOUNDATION_MODEL: &str = "BEDROCK_COST_LEGACY_FOUNDATION_MODEL";
pub const REASON_RES_COLLECTION_ERRORS: &str = "BEDROCK_RES_COLLECTION_ERRORS";
pub const REASON_RES_TAG_COLLECTION_ERRORS: &str = "BEDROCK_RES_TAG_COLLECTION_ERRORS";
pub const REASON_RES_FAILED_JOB: &str = "BEDROCK_RES_FAILED_JOB";
pub const REASON_RES_INCOMPLETE_JOB: &str = "BEDROCK_RES_INCOMPLETE_JOB";
pub const REASON_RES_FAILED_MODEL: &str = "BEDROCK_RES_FAILED_MODEL";
pub const REASON_RES_INACTIVE_PROVISIONED_MODEL: &str = "BEDROCK_RES_INACTIVE_PROVISIONED_MODEL";
pub const REASON_RES_INACTIVE_ROUTING_RESOURCE: &str = "BEDROCK_RES_INACTIVE_ROUTING_RESOURCE";
pub const REASON_SEC_NO_GUARDRAILS: &str = "BEDROCK_SEC_NO_GUARDRAILS";
pub const REASON_SEC_INACTIVE_GUARDRAIL: &str = "BEDROCK_SEC_INACTIVE_GUARDRAIL";
pub const REASON_SEC_BATCH_JOB_WITHOUT_VPC: &str = "BEDROCK_SEC_BATCH_JOB_WITHOUT_VPC";
pub const REASON_INV_STALE_DATA: &str = "BEDROCK_INV_STALE_DATA";

pub fn evaluate_bedrock_fleet(
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

fn data_i64(resource_data: &Value, key: &str) -> i64 {
    resource_data.get(key).and_then(|v| v.as_i64()).unwrap_or(0)
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
    if should_require_cost_tags(resource) && !has_any_tag(&resource.tags, COST_ALLOCATION_TAG_KEYS)
    {
        findings.push(finding(
            resource,
            Pillar::Cost,
            REASON_COST_NO_TAGS,
            Severity::Medium,
            format!(
                "Bedrock resource {} has no cost allocation tags",
                resource.resource_id
            ),
            json!({ "tags": resource.tags, "resource_kind": resource_kind(resource) }),
        ));
    }

    if resource_kind(resource) == Some("provisioned_model_throughput")
        && data_i64(&resource.resource_data, "model_units") > 0
    {
        findings.push(finding(
            resource,
            Pillar::Cost,
            REASON_COST_PROVISIONED_THROUGHPUT,
            Severity::High,
            format!(
                "Bedrock provisioned throughput {} allocates dedicated model units",
                resource.resource_id
            ),
            json!({
                "model_units": resource.resource_data.get("model_units"),
                "desired_model_units": resource.resource_data.get("desired_model_units"),
                "commitment_duration": resource.resource_data.get("commitment_duration"),
                "commitment_expiration_time": resource.resource_data.get("commitment_expiration_time"),
            }),
        ));
    }

    if resource_kind(resource) == Some("foundation_model")
        && data_str(&resource.resource_data, "lifecycle_status") == Some("LEGACY")
    {
        findings.push(finding(
            resource,
            Pillar::Cost,
            REASON_COST_LEGACY_FOUNDATION_MODEL,
            Severity::Medium,
            format!(
                "Bedrock foundation model {} is in legacy lifecycle status",
                resource.resource_id
            ),
            json!({
                "lifecycle_status": resource.resource_data.get("lifecycle_status"),
                "legacy_time": resource.resource_data.get("legacy_time"),
                "public_extended_access_time": resource.resource_data.get("public_extended_access_time"),
            }),
        ));
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
                    "Bedrock account {} had {} inventory collection errors",
                    resource.resource_id, collection_errors
                ),
                json!({
                    "collection_error_count": collection_errors,
                    "collection_errors": resource.resource_data.get("collection_errors"),
                }),
            ));
        }

        let tag_errors = data_usize(&resource.resource_data, "tag_collection_error_count");
        if tag_errors > 0 {
            findings.push(finding(
                resource,
                Pillar::Resilience,
                REASON_RES_TAG_COLLECTION_ERRORS,
                Severity::Low,
                format!(
                    "Bedrock account {} had {} tag collection errors",
                    resource.resource_id, tag_errors
                ),
                json!({ "tag_collection_error_count": tag_errors }),
            ));
        }

        emit_summary_count(
            resource,
            findings,
            Pillar::Resilience,
            "failed_job_count",
            REASON_RES_FAILED_JOB,
            Severity::High,
            "failed Bedrock jobs",
        );
        emit_summary_count(
            resource,
            findings,
            Pillar::Resilience,
            "incomplete_job_count",
            REASON_RES_INCOMPLETE_JOB,
            Severity::Medium,
            "incomplete Bedrock jobs",
        );
        emit_summary_count(
            resource,
            findings,
            Pillar::Resilience,
            "failed_model_count",
            REASON_RES_FAILED_MODEL,
            Severity::High,
            "failed Bedrock models",
        );
        emit_summary_count(
            resource,
            findings,
            Pillar::Resilience,
            "inactive_provisioned_model_count",
            REASON_RES_INACTIVE_PROVISIONED_MODEL,
            Severity::High,
            "inactive Bedrock provisioned throughput resources",
        );
        emit_summary_count(
            resource,
            findings,
            Pillar::Resilience,
            "inactive_routing_resource_count",
            REASON_RES_INACTIVE_ROUTING_RESOURCE,
            Severity::Medium,
            "inactive Bedrock routing resources",
        );
    }

    if is_failed_status(data_str(&resource.resource_data, "status")) {
        findings.push(finding(
            resource,
            Pillar::Resilience,
            failure_reason_for_resource(resource),
            Severity::High,
            format!(
                "Bedrock resource {} has failed status",
                resource.resource_id
            ),
            json!({
                "status": resource.resource_data.get("status"),
                "resource_kind": resource_kind(resource),
                "message": resource.resource_data.get("message"),
            }),
        ));
    } else if is_incomplete_job(resource) {
        findings.push(finding(
            resource,
            Pillar::Resilience,
            REASON_RES_INCOMPLETE_JOB,
            Severity::Medium,
            format!("Bedrock job {} has not completed", resource.resource_id),
            json!({
                "status": resource.resource_data.get("status"),
                "resource_kind": resource_kind(resource),
            }),
        ));
    } else if is_inactive_routing_or_capacity(resource) {
        findings.push(finding(
            resource,
            Pillar::Resilience,
            inactive_reason_for_resource(resource),
            Severity::Medium,
            format!("Bedrock resource {} is not active", resource.resource_id),
            json!({
                "status": resource.resource_data.get("status"),
                "resource_kind": resource_kind(resource),
            }),
        ));
    }
}

fn evaluate_security(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    if resource_kind(resource) == Some("account_summary") {
        let guardrails = data_usize(&resource.resource_data, "guardrail_count");
        let resources = data_usize(&resource.resource_data, "resource_count");
        let foundation_models = data_usize(&resource.resource_data, "foundation_model_count");
        if guardrails == 0 && resources > foundation_models {
            findings.push(finding(
                resource,
                Pillar::Security,
                REASON_SEC_NO_GUARDRAILS,
                Severity::Medium,
                format!(
                    "Bedrock account {} has no guardrail inventory for customer-managed Bedrock resources",
                    resource.resource_id
                ),
                json!({
                    "guardrail_count": guardrails,
                    "resource_count": resources,
                    "foundation_model_count": foundation_models,
                    "resources_by_kind": resource.resource_data.get("resources_by_kind"),
                }),
            ));
        }

        emit_summary_count(
            resource,
            findings,
            Pillar::Security,
            "inactive_guardrail_count",
            REASON_SEC_INACTIVE_GUARDRAIL,
            Severity::High,
            "inactive Bedrock guardrails",
        );
    }

    if resource_kind(resource) == Some("guardrail")
        && !is_active_status(data_str(&resource.resource_data, "status"))
    {
        findings.push(finding(
            resource,
            Pillar::Security,
            REASON_SEC_INACTIVE_GUARDRAIL,
            Severity::High,
            format!("Bedrock guardrail {} is not active", resource.resource_id),
            json!({
                "status": resource.resource_data.get("status"),
                "version": resource.resource_data.get("version"),
            }),
        ));
    }

    if resource_kind(resource) == Some("model_invocation_job")
        && data_bool(&resource.resource_data, "has_vpc_config") == Some(false)
    {
        findings.push(finding(
            resource,
            Pillar::Security,
            REASON_SEC_BATCH_JOB_WITHOUT_VPC,
            Severity::Medium,
            format!(
                "Bedrock batch invocation job {} has no VPC configuration evidence",
                resource.resource_id
            ),
            json!({
                "has_vpc_config": resource.resource_data.get("has_vpc_config"),
                "model_id": resource.resource_data.get("model_id"),
                "role_arn": resource.resource_data.get("role_arn"),
            }),
        ));
    }
}

fn emit_summary_count(
    resource: &AwsResourceModel,
    findings: &mut Vec<InventoryFinding>,
    pillar: Pillar,
    field: &str,
    reason_code: &str,
    severity: Severity,
    label: &str,
) {
    let count = data_usize(&resource.resource_data, field);
    if count == 0 {
        return;
    }

    findings.push(finding(
        resource,
        pillar,
        reason_code,
        severity,
        format!(
            "Bedrock account {} has {} {}",
            resource.resource_id, count, label
        ),
        json!({ field: count, "resources_by_kind": resource.resource_data.get("resources_by_kind") }),
    ));
}

fn should_require_cost_tags(resource: &AwsResourceModel) -> bool {
    !matches!(
        resource_kind(resource),
        Some("account_summary") | Some("foundation_model") | None
    )
}

fn is_incomplete_job(resource: &AwsResourceModel) -> bool {
    matches!(
        resource_kind(resource),
        Some("model_customization_job")
            | Some("model_invocation_job")
            | Some("model_import_job")
            | Some("evaluation_job")
    ) && !is_completed_status(data_str(&resource.resource_data, "status"))
        && !is_failed_status(data_str(&resource.resource_data, "status"))
}

fn is_inactive_routing_or_capacity(resource: &AwsResourceModel) -> bool {
    matches!(
        resource_kind(resource),
        Some("provisioned_model_throughput") | Some("inference_profile") | Some("prompt_router")
    ) && !is_active_status(data_str(&resource.resource_data, "status"))
}

fn failure_reason_for_resource(resource: &AwsResourceModel) -> &'static str {
    match resource_kind(resource) {
        Some("custom_model") => REASON_RES_FAILED_MODEL,
        Some("provisioned_model_throughput") => REASON_RES_INACTIVE_PROVISIONED_MODEL,
        _ => REASON_RES_FAILED_JOB,
    }
}

fn inactive_reason_for_resource(resource: &AwsResourceModel) -> &'static str {
    match resource_kind(resource) {
        Some("provisioned_model_throughput") => REASON_RES_INACTIVE_PROVISIONED_MODEL,
        _ => REASON_RES_INACTIVE_ROUTING_RESOURCE,
    }
}

fn is_active_status(status: Option<&str>) -> bool {
    matches!(
        status.map(normalized_status).as_deref(),
        Some("ACTIVE" | "AVAILABLE" | "INSERVICE" | "READY" | "COMPLETED" | "SUCCEEDED")
    )
}

fn is_completed_status(status: Option<&str>) -> bool {
    matches!(
        status.map(normalized_status).as_deref(),
        Some(
            "COMPLETED" | "COMPLETE" | "SUCCEEDED" | "ACTIVE" | "AVAILABLE" | "READY" | "INSERVICE"
        )
    )
}

fn is_failed_status(status: Option<&str>) -> bool {
    matches!(
        status.map(normalized_status).as_deref(),
        Some(
            "FAILED"
                | "EXPIRED"
                | "PARTIALLYCOMPLETED"
                | "STOPPED"
                | "STOPPING"
                | "DELETING"
                | "DELETEUNSUCCESSFUL"
        )
    )
}

fn normalized_status(status: &str) -> String {
    status
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .collect::<String>()
        .to_ascii_uppercase()
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
            arn: format!("arn:aws:bedrock:us-east-1:123456789012:{}", resource_id),
            name: Some(resource_id.to_string()),
            tags,
            resource_data,
            created_at: now,
            updated_at: now,
            last_refreshed: now - Duration::hours(26),
        }
    }

    #[test]
    fn evaluates_bedrock_inventory_findings() {
        let now = Utc::now();
        let resources = vec![
            fixture(
                "bedrock:123456789012",
                json!({
                    "resource_kind": "account_summary",
                    "resource_count": 5,
                    "foundation_model_count": 1,
                    "guardrail_count": 0,
                    "collection_error_count": 1,
                    "tag_collection_error_count": 1,
                    "failed_job_count": 1,
                    "incomplete_job_count": 1,
                    "failed_model_count": 1,
                    "inactive_provisioned_model_count": 1,
                    "inactive_routing_resource_count": 1,
                    "resources_by_kind": { "custom_model": 1, "model_invocation_job": 1 },
                }),
                json!({}),
            ),
            fixture(
                "foundation_model/legacy-model",
                json!({
                    "resource_kind": "foundation_model",
                    "lifecycle_status": "LEGACY",
                    "public_extended_access_time": "2026-01-01T00:00:00Z",
                }),
                json!({}),
            ),
            fixture(
                "provisioned_model_throughput/pt-1",
                json!({
                    "resource_kind": "provisioned_model_throughput",
                    "model_units": 2,
                    "desired_model_units": 2,
                    "status": "Creating",
                }),
                json!({}),
            ),
            fixture(
                "model_invocation_job/job-1",
                json!({
                    "resource_kind": "model_invocation_job",
                    "status": "Failed",
                    "message": "validation failed",
                    "has_vpc_config": false,
                    "model_id": "anthropic.claude",
                    "role_arn": "arn:aws:iam::123456789012:role/bedrock",
                }),
                json!({ "CostCenter": "ai" }),
            ),
            fixture(
                "prompt_router/router-1",
                json!({
                    "resource_kind": "prompt_router",
                    "status": "Updating",
                    "model_count": 2,
                }),
                json!({ "CostCenter": "ai" }),
            ),
            fixture(
                "guardrail/gr-1",
                json!({
                    "resource_kind": "guardrail",
                    "status": "Failed",
                    "version": "DRAFT",
                }),
                json!({ "CostCenter": "ai" }),
            ),
        ];

        let cost = evaluate_bedrock_fleet(&resources, Pillar::Cost, now);
        assert_eq!(cost.resources_evaluated, 6);
        assert_eq!(cost.stale_resources, 6);
        assert!(cost
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_COST_NO_TAGS));
        assert!(cost
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_COST_PROVISIONED_THROUGHPUT));
        assert!(cost
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_COST_LEGACY_FOUNDATION_MODEL));

        let resilience = evaluate_bedrock_fleet(&resources, Pillar::Resilience, now);
        assert!(resilience
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_RES_COLLECTION_ERRORS));
        assert!(resilience
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_RES_TAG_COLLECTION_ERRORS));
        assert!(resilience
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_RES_FAILED_JOB));
        assert!(resilience
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_RES_INCOMPLETE_JOB));
        assert!(resilience
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_RES_INACTIVE_PROVISIONED_MODEL));
        assert!(resilience
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_RES_INACTIVE_ROUTING_RESOURCE));

        let security = evaluate_bedrock_fleet(&resources, Pillar::Security, now);
        assert!(security
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_SEC_NO_GUARDRAILS));
        assert!(security
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_SEC_INACTIVE_GUARDRAIL));
        assert!(security
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_SEC_BATCH_JOB_WITHOUT_VPC));
    }
}

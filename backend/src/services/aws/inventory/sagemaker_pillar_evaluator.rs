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

// Deterministic AWS SageMaker AI inventory evaluators for the cost,
// resilience, and security pillars.

use chrono::{DateTime, Utc};
use serde_json::{json, Value};

use crate::models::aws_resource::Model as AwsResourceModel;
use crate::services::aws::inventory::types::{
    check_stale, has_any_tag, score_pillar, InventoryFinding, Pillar, PillarReport, Severity,
    COST_ALLOCATION_TAG_KEYS,
};

pub const RESOURCE_TYPE: &str = "SageMakerResource";

pub const REASON_COST_NO_TAGS: &str = "SAGEMAKER_COST_NO_TAGS";
pub const REASON_COST_RUNNING_NOTEBOOK: &str = "SAGEMAKER_COST_RUNNING_NOTEBOOK";
pub const REASON_COST_ENDPOINT_CAPACITY: &str = "SAGEMAKER_COST_ENDPOINT_CAPACITY";
pub const REASON_RES_COLLECTION_ERRORS: &str = "SAGEMAKER_RES_COLLECTION_ERRORS";
pub const REASON_RES_UNHEALTHY_ENDPOINT: &str = "SAGEMAKER_RES_UNHEALTHY_ENDPOINT";
pub const REASON_RES_FAILED_JOB: &str = "SAGEMAKER_RES_FAILED_JOB";
pub const REASON_RES_INCOMPLETE_JOB: &str = "SAGEMAKER_RES_INCOMPLETE_JOB";
pub const REASON_RES_FAILED_NOTEBOOK: &str = "SAGEMAKER_RES_FAILED_NOTEBOOK";
pub const REASON_RES_DOMAIN_NOT_READY: &str = "SAGEMAKER_RES_DOMAIN_NOT_READY";
pub const REASON_SEC_NOTEBOOK_DIRECT_INTERNET: &str = "SAGEMAKER_SEC_NOTEBOOK_DIRECT_INTERNET";
pub const REASON_SEC_NOTEBOOK_ROOT_ACCESS: &str = "SAGEMAKER_SEC_NOTEBOOK_ROOT_ACCESS";
pub const REASON_SEC_NOTEBOOK_VOLUME_UNENCRYPTED: &str =
    "SAGEMAKER_SEC_NOTEBOOK_VOLUME_UNENCRYPTED";
pub const REASON_SEC_ENDPOINT_CONFIG_WITHOUT_KMS: &str =
    "SAGEMAKER_SEC_ENDPOINT_CONFIG_WITHOUT_KMS";
pub const REASON_SEC_MODEL_WITHOUT_VPC: &str = "SAGEMAKER_SEC_MODEL_WITHOUT_VPC";
pub const REASON_SEC_NETWORK_ISOLATION_DISABLED: &str = "SAGEMAKER_SEC_NETWORK_ISOLATION_DISABLED";
pub const REASON_SEC_DOMAIN_WITHOUT_KMS: &str = "SAGEMAKER_SEC_DOMAIN_WITHOUT_KMS";
pub const REASON_INV_STALE_DATA: &str = "SAGEMAKER_INV_STALE_DATA";

pub fn evaluate_sagemaker_fleet(
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
    if resource_kind(resource) != Some("account_summary")
        && !has_any_tag(&resource.tags, COST_ALLOCATION_TAG_KEYS)
    {
        findings.push(finding(
            resource,
            Pillar::Cost,
            REASON_COST_NO_TAGS,
            Severity::Medium,
            format!(
                "SageMaker AI resource {} has no cost allocation tags",
                resource.resource_id
            ),
            json!({ "tags": resource.tags, "resource_kind": resource_kind(resource) }),
        ));
    }

    if resource_kind(resource) == Some("notebook_instance")
        && data_str(&resource.resource_data, "status")
            .map(is_running_notebook_status)
            .unwrap_or(false)
    {
        findings.push(finding(
            resource,
            Pillar::Cost,
            REASON_COST_RUNNING_NOTEBOOK,
            Severity::Medium,
            format!(
                "SageMaker notebook {} appears to be running compute",
                resource.resource_id
            ),
            json!({
                "status": resource.resource_data.get("status"),
                "instance_type": resource.resource_data.get("instance_type"),
                "volume_size_gb": resource.resource_data.get("volume_size_gb"),
            }),
        ));
    }

    if matches!(
        resource_kind(resource),
        Some("endpoint_config") | Some("endpoint")
    ) && data_i64(&resource.resource_data, "endpoint_capacity_instance_count") > 0
    {
        findings.push(finding(
            resource,
            Pillar::Cost,
            REASON_COST_ENDPOINT_CAPACITY,
            Severity::High,
            format!(
                "SageMaker endpoint resource {} defines dedicated instance capacity",
                resource.resource_id
            ),
            json!({
                "endpoint_capacity_instance_count": resource.resource_data.get("endpoint_capacity_instance_count"),
                "production_variant_count": resource.resource_data.get("production_variant_count"),
                "serverless_variant_count": resource.resource_data.get("serverless_variant_count"),
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
                    "SageMaker AI account {} had {} inventory collection errors",
                    resource.resource_id, collection_errors
                ),
                json!({
                    "collection_error_count": collection_errors,
                    "collection_errors": resource.resource_data.get("collection_errors"),
                }),
            ));
        }

        emit_summary_count(
            resource,
            findings,
            "unhealthy_endpoint_count",
            REASON_RES_UNHEALTHY_ENDPOINT,
            Severity::High,
            "unhealthy SageMaker endpoints",
        );
        emit_summary_count(
            resource,
            findings,
            "failed_job_count",
            REASON_RES_FAILED_JOB,
            Severity::High,
            "failed SageMaker jobs",
        );
        emit_summary_count(
            resource,
            findings,
            "incomplete_job_count",
            REASON_RES_INCOMPLETE_JOB,
            Severity::Medium,
            "incomplete SageMaker jobs",
        );
        emit_summary_count(
            resource,
            findings,
            "failed_notebook_count",
            REASON_RES_FAILED_NOTEBOOK,
            Severity::High,
            "failed SageMaker notebooks",
        );
        emit_summary_count(
            resource,
            findings,
            "domain_not_ready_count",
            REASON_RES_DOMAIN_NOT_READY,
            Severity::Medium,
            "SageMaker domains not ready",
        );
    }

    if resource_kind(resource) == Some("endpoint")
        && data_str(&resource.resource_data, "status")
            .map(is_unhealthy_endpoint_status)
            .unwrap_or(false)
    {
        findings.push(finding(
            resource,
            Pillar::Resilience,
            REASON_RES_UNHEALTHY_ENDPOINT,
            Severity::High,
            format!(
                "SageMaker endpoint {} is not in service",
                resource.resource_id
            ),
            json!({
                "status": resource.resource_data.get("status"),
                "endpoint_config_name": resource.resource_data.get("endpoint_config_name"),
            }),
        ));
    }

    if matches!(
        resource_kind(resource),
        Some("training_job") | Some("transform_job") | Some("processing_job")
    ) {
        if data_str(&resource.resource_data, "status")
            .map(is_failed_status)
            .unwrap_or(false)
        {
            findings.push(finding(
                resource,
                Pillar::Resilience,
                REASON_RES_FAILED_JOB,
                Severity::High,
                format!("SageMaker job {} failed", resource.resource_id),
                json!({ "status": resource.resource_data.get("status") }),
            ));
        } else if data_str(&resource.resource_data, "status")
            .map(|status| !is_completed_job_status(status))
            .unwrap_or(false)
        {
            findings.push(finding(
                resource,
                Pillar::Resilience,
                REASON_RES_INCOMPLETE_JOB,
                Severity::Medium,
                format!("SageMaker job {} is not complete", resource.resource_id),
                json!({ "status": resource.resource_data.get("status") }),
            ));
        }
    }
}

fn evaluate_security(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    if resource_kind(resource) == Some("notebook_instance") {
        if data_str(&resource.resource_data, "direct_internet_access")
            .map(|value| value.eq_ignore_ascii_case("enabled"))
            .unwrap_or(false)
        {
            findings.push(finding(
                resource,
                Pillar::Security,
                REASON_SEC_NOTEBOOK_DIRECT_INTERNET,
                Severity::High,
                format!(
                    "SageMaker notebook {} has direct internet access enabled",
                    resource.resource_id
                ),
                json!({
                    "direct_internet_access": resource.resource_data.get("direct_internet_access"),
                }),
            ));
        }

        if data_str(&resource.resource_data, "root_access")
            .map(|value| value.eq_ignore_ascii_case("enabled"))
            .unwrap_or(false)
        {
            findings.push(finding(
                resource,
                Pillar::Security,
                REASON_SEC_NOTEBOOK_ROOT_ACCESS,
                Severity::Medium,
                format!(
                    "SageMaker notebook {} allows root access",
                    resource.resource_id
                ),
                json!({ "root_access": resource.resource_data.get("root_access") }),
            ));
        }

        if data_str(&resource.resource_data, "kms_key_id").is_none() {
            findings.push(finding(
                resource,
                Pillar::Security,
                REASON_SEC_NOTEBOOK_VOLUME_UNENCRYPTED,
                Severity::High,
                format!(
                    "SageMaker notebook {} has no volume KMS key evidence",
                    resource.resource_id
                ),
                json!({ "kms_key_id": resource.resource_data.get("kms_key_id") }),
            ));
        }
    }

    if resource_kind(resource) == Some("endpoint_config")
        && data_str(&resource.resource_data, "kms_key_id").is_none()
    {
        findings.push(finding(
            resource,
            Pillar::Security,
            REASON_SEC_ENDPOINT_CONFIG_WITHOUT_KMS,
            Severity::High,
            format!(
                "SageMaker endpoint config {} has no KMS key evidence",
                resource.resource_id
            ),
            json!({
                "production_variant_count": resource.resource_data.get("production_variant_count"),
                "endpoint_capacity_instance_count": resource.resource_data.get("endpoint_capacity_instance_count"),
            }),
        ));
    }

    if resource_kind(resource) == Some("model") {
        if data_bool(&resource.resource_data, "vpc_configured") != Some(true) {
            findings.push(finding(
                resource,
                Pillar::Security,
                REASON_SEC_MODEL_WITHOUT_VPC,
                Severity::High,
                format!(
                    "SageMaker model {} has no VPC config evidence",
                    resource.resource_id
                ),
                json!({ "vpc_configured": resource.resource_data.get("vpc_configured") }),
            ));
        }

        if data_bool(&resource.resource_data, "network_isolation_enabled") != Some(true) {
            findings.push(finding(
                resource,
                Pillar::Security,
                REASON_SEC_NETWORK_ISOLATION_DISABLED,
                Severity::High,
                format!(
                    "SageMaker model {} does not have network isolation enabled",
                    resource.resource_id
                ),
                json!({
                    "network_isolation_enabled": resource.resource_data.get("network_isolation_enabled"),
                }),
            ));
        }
    }

    if resource_kind(resource) == Some("domain")
        && data_str(&resource.resource_data, "kms_key_id").is_none()
    {
        findings.push(finding(
            resource,
            Pillar::Security,
            REASON_SEC_DOMAIN_WITHOUT_KMS,
            Severity::Medium,
            format!(
                "SageMaker domain {} has no KMS key evidence",
                resource.resource_id
            ),
            json!({ "kms_key_id": resource.resource_data.get("kms_key_id") }),
        ));
    }
}

fn emit_summary_count(
    resource: &AwsResourceModel,
    findings: &mut Vec<InventoryFinding>,
    count_key: &str,
    reason_code: &str,
    severity: Severity,
    label: &str,
) {
    let count = data_usize(&resource.resource_data, count_key);
    if count == 0 {
        return;
    }
    findings.push(finding(
        resource,
        Pillar::Resilience,
        reason_code,
        severity,
        format!(
            "SageMaker AI account {} has {} {}",
            resource.resource_id, count, label
        ),
        json!({
            count_key: count,
            "resources_by_kind": resource.resource_data.get("resources_by_kind"),
        }),
    ));
}

fn is_running_notebook_status(status: &str) -> bool {
    matches!(
        status.to_ascii_lowercase().as_str(),
        "inservice" | "pending" | "updating"
    )
}

fn is_failed_status(status: &str) -> bool {
    let status = status.to_ascii_lowercase();
    status.contains("failed") || status.contains("error")
}

fn is_completed_job_status(status: &str) -> bool {
    matches!(
        status.to_ascii_lowercase().as_str(),
        "completed" | "stopped" | "stopping"
    )
}

fn is_unhealthy_endpoint_status(status: &str) -> bool {
    !matches!(
        status.to_ascii_lowercase().as_str(),
        "inservice" | "creating" | "updating" | "systemupdating" | "rollingback"
    )
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
            arn: format!("arn:aws:sagemaker:us-east-1:123456789012:{}", resource_id),
            name: Some(resource_id.to_string()),
            tags,
            resource_data,
            created_at: now,
            updated_at: now,
            last_refreshed: now - Duration::hours(26),
        }
    }

    #[test]
    fn evaluates_sagemaker_inventory_findings() {
        let now = Utc::now();
        let resources = vec![
            fixture(
                "sagemaker:123456789012",
                json!({
                    "resource_kind": "account_summary",
                    "resource_count": 5,
                    "collection_error_count": 1,
                    "collection_errors": [{ "cloudcontrol_type": "AWS::SageMaker::Pipeline", "error": "denied" }],
                    "resources_by_kind": { "notebook_instance": 1, "endpoint_config": 1 },
                    "failed_job_count": 1,
                    "domain_not_ready_count": 1,
                }),
                json!({}),
            ),
            fixture(
                "notebook_instance/notebook-a",
                json!({
                    "resource_kind": "notebook_instance",
                    "status": "InService",
                    "instance_type": "ml.t3.medium",
                    "volume_size_gb": 50,
                    "direct_internet_access": "Enabled",
                    "root_access": "Enabled",
                }),
                json!({}),
            ),
            fixture(
                "endpoint_config/realtime-a",
                json!({
                    "resource_kind": "endpoint_config",
                    "production_variant_count": 1,
                    "endpoint_capacity_instance_count": 2,
                }),
                json!({ "CostCenter": "ml-platform" }),
            ),
            fixture(
                "endpoint/realtime-a",
                json!({
                    "resource_kind": "endpoint",
                    "status": "Failed",
                    "endpoint_config_name": "realtime-a",
                }),
                json!({ "CostCenter": "ml-platform" }),
            ),
            fixture(
                "model/model-a",
                json!({
                    "resource_kind": "model",
                    "vpc_configured": false,
                    "network_isolation_enabled": false,
                }),
                json!({ "CostCenter": "ml-platform" }),
            ),
        ];

        let cost = evaluate_sagemaker_fleet(&resources, Pillar::Cost, now);
        assert_eq!(cost.resources_evaluated, 5);
        assert_eq!(cost.stale_resources, 5);
        assert!(cost
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_COST_NO_TAGS));
        assert!(cost
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_COST_RUNNING_NOTEBOOK));
        assert!(cost
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_COST_ENDPOINT_CAPACITY));

        let resilience = evaluate_sagemaker_fleet(&resources, Pillar::Resilience, now);
        assert!(resilience
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_RES_COLLECTION_ERRORS));
        assert!(resilience
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_RES_UNHEALTHY_ENDPOINT));
        assert!(resilience
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_RES_FAILED_JOB));

        let security = evaluate_sagemaker_fleet(&resources, Pillar::Security, now);
        assert!(security
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_SEC_NOTEBOOK_DIRECT_INTERNET));
        assert!(security
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_SEC_NOTEBOOK_ROOT_ACCESS));
        assert!(security
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_SEC_NOTEBOOK_VOLUME_UNENCRYPTED));
        assert!(security
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_SEC_ENDPOINT_CONFIG_WITHOUT_KMS));
        assert!(security
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_SEC_MODEL_WITHOUT_VPC));
        assert!(security
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_SEC_NETWORK_ISOLATION_DISABLED));
    }
}

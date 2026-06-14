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

// Deterministic AWS Comprehend inventory evaluators for cost,
// resilience, and security pillars.

use chrono::{DateTime, Utc};
use serde_json::{json, Value};

use crate::models::aws_resource::Model as AwsResourceModel;
use crate::services::aws::inventory::types::{
    check_stale, has_any_tag, score_pillar, InventoryFinding, Pillar, PillarReport, Severity,
    COST_ALLOCATION_TAG_KEYS,
};

pub const RESOURCE_TYPE: &str = "ComprehendResource";

pub const REASON_COST_NO_TAGS: &str = "COMPREHEND_COST_NO_TAGS";
pub const REASON_COST_CUSTOM_MODEL_RESOURCE: &str = "COMPREHEND_COST_CUSTOM_MODEL_RESOURCE";
pub const REASON_COST_FLYWHEEL_RESOURCE: &str = "COMPREHEND_COST_FLYWHEEL_RESOURCE";
pub const REASON_RES_COLLECTION_ERRORS: &str = "COMPREHEND_RES_COLLECTION_ERRORS";
pub const REASON_RES_FAILED_RESOURCE: &str = "COMPREHEND_RES_FAILED_RESOURCE";
pub const REASON_RES_INCOMPLETE_RESOURCE: &str = "COMPREHEND_RES_INCOMPLETE_RESOURCE";
pub const REASON_SEC_DATA_ACCESS_ROLE_MISSING: &str = "COMPREHEND_SEC_DATA_ACCESS_ROLE_MISSING";
pub const REASON_SEC_MODEL_WITHOUT_KMS: &str = "COMPREHEND_SEC_MODEL_WITHOUT_KMS";
pub const REASON_SEC_VOLUME_WITHOUT_KMS: &str = "COMPREHEND_SEC_VOLUME_WITHOUT_KMS";
pub const REASON_SEC_OUTPUT_WITHOUT_KMS: &str = "COMPREHEND_SEC_OUTPUT_WITHOUT_KMS";
pub const REASON_SEC_DATALAKE_WITHOUT_KMS: &str = "COMPREHEND_SEC_DATALAKE_WITHOUT_KMS";
pub const REASON_SEC_DATALAKE_MISSING: &str = "COMPREHEND_SEC_DATALAKE_MISSING";
pub const REASON_SEC_VPC_CONFIG_MISSING: &str = "COMPREHEND_SEC_VPC_CONFIG_MISSING";
pub const REASON_INV_STALE_DATA: &str = "COMPREHEND_INV_STALE_DATA";

pub fn evaluate_comprehend_fleet(
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

fn data_bool(resource_data: &Value, key: &str) -> bool {
    resource_data
        .get(key)
        .and_then(|value| value.as_bool())
        .unwrap_or(false)
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
    if is_custom_model_resource(resource) && !has_any_tag(&resource.tags, COST_ALLOCATION_TAG_KEYS)
    {
        findings.push(finding(
            resource,
            Pillar::Cost,
            REASON_COST_NO_TAGS,
            Severity::Medium,
            format!(
                "Comprehend resource {} has no cost allocation tags",
                resource.resource_id
            ),
            json!({
                "tags": resource.tags,
                "resource_kind": resource_kind(resource),
                "accepted_tag_keys": COST_ALLOCATION_TAG_KEYS,
            }),
        ));
    }

    match resource_kind(resource) {
        Some("document_classifier") | Some("entity_recognizer") => findings.push(finding(
            resource,
            Pillar::Cost,
            REASON_COST_CUSTOM_MODEL_RESOURCE,
            Severity::Low,
            format!(
                "Comprehend custom model resource {} should be tracked for training and hosting cost",
                resource.resource_id
            ),
            json!({
                "resource_kind": resource_kind(resource),
                "status": resource.resource_data.get("status"),
                "language_code": resource.resource_data.get("language_code"),
                "mode": resource.resource_data.get("mode"),
            }),
        )),
        Some("flywheel") => findings.push(finding(
            resource,
            Pillar::Cost,
            REASON_COST_FLYWHEEL_RESOURCE,
            Severity::Medium,
            format!(
                "Comprehend flywheel {} should be tracked as a custom ML lifecycle asset",
                resource.resource_id
            ),
            json!({
                "model_type": resource.resource_data.get("model_type"),
                "data_lake_s3_uri": resource.resource_data.get("data_lake_s3_uri"),
                "status": resource.resource_data.get("status"),
            }),
        )),
        _ => {}
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
                    "Comprehend account {} had {} inventory collection errors",
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
            Pillar::Resilience,
            "failed_resource_count",
            REASON_RES_FAILED_RESOURCE,
            Severity::High,
            "Comprehend failed resources",
        );
        emit_summary_count(
            resource,
            findings,
            Pillar::Resilience,
            "incomplete_resource_count",
            REASON_RES_INCOMPLETE_RESOURCE,
            Severity::Medium,
            "Comprehend incomplete resources",
        );
    }

    if is_custom_model_resource(resource) {
        let status = data_str(&resource.resource_data, "status");
        if status.map(is_failed_status).unwrap_or(false) {
            findings.push(finding(
                resource,
                Pillar::Resilience,
                REASON_RES_FAILED_RESOURCE,
                Severity::High,
                format!("Comprehend resource {} is failed", resource.resource_id),
                json!({
                    "resource_kind": resource_kind(resource),
                    "status": resource.resource_data.get("status"),
                    "status_message": resource.resource_data.get("status_message"),
                }),
            ));
        } else if status
            .map(|status| !is_completed_status(status))
            .unwrap_or(false)
        {
            findings.push(finding(
                resource,
                Pillar::Resilience,
                REASON_RES_INCOMPLETE_RESOURCE,
                Severity::Medium,
                format!(
                    "Comprehend resource {} is not in a completed state",
                    resource.resource_id
                ),
                json!({
                    "resource_kind": resource_kind(resource),
                    "status": resource.resource_data.get("status"),
                    "status_message": resource.resource_data.get("status_message"),
                }),
            ));
        }
    }
}

fn evaluate_security(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    if resource_kind(resource) == Some("account_summary") {
        emit_summary_count(
            resource,
            findings,
            Pillar::Security,
            "data_access_role_missing_count",
            REASON_SEC_DATA_ACCESS_ROLE_MISSING,
            Severity::High,
            "Comprehend resources without data access roles",
        );
        emit_summary_count(
            resource,
            findings,
            Pillar::Security,
            "model_without_kms_count",
            REASON_SEC_MODEL_WITHOUT_KMS,
            Severity::High,
            "Comprehend resources without model KMS keys",
        );
        emit_summary_count(
            resource,
            findings,
            Pillar::Security,
            "volume_without_kms_count",
            REASON_SEC_VOLUME_WITHOUT_KMS,
            Severity::High,
            "Comprehend resources without volume KMS keys",
        );
        emit_summary_count(
            resource,
            findings,
            Pillar::Security,
            "output_without_kms_count",
            REASON_SEC_OUTPUT_WITHOUT_KMS,
            Severity::High,
            "Comprehend output configurations without KMS keys",
        );
        emit_summary_count(
            resource,
            findings,
            Pillar::Security,
            "data_lake_without_kms_count",
            REASON_SEC_DATALAKE_WITHOUT_KMS,
            Severity::High,
            "Comprehend flywheel data lakes without KMS keys",
        );
        emit_summary_count(
            resource,
            findings,
            Pillar::Security,
            "data_lake_missing_count",
            REASON_SEC_DATALAKE_MISSING,
            Severity::Medium,
            "Comprehend flywheels without data lake evidence",
        );
        emit_summary_count(
            resource,
            findings,
            Pillar::Security,
            "vpc_config_missing_count",
            REASON_SEC_VPC_CONFIG_MISSING,
            Severity::Medium,
            "Comprehend custom model resources without VPC configuration",
        );
    }

    if is_custom_model_resource(resource)
        && data_str(&resource.resource_data, "data_access_role_arn").is_none()
    {
        findings.push(finding(
            resource,
            Pillar::Security,
            REASON_SEC_DATA_ACCESS_ROLE_MISSING,
            Severity::High,
            format!(
                "Comprehend resource {} has no data access role evidence",
                resource.resource_id
            ),
            json!({
                "resource_kind": resource_kind(resource),
                "data_access_role_arn": resource.resource_data.get("data_access_role_arn"),
            }),
        ));
    }

    if is_custom_model_resource(resource)
        && data_str(&resource.resource_data, "model_kms_key_id").is_none()
    {
        findings.push(finding(
            resource,
            Pillar::Security,
            REASON_SEC_MODEL_WITHOUT_KMS,
            Severity::High,
            format!(
                "Comprehend resource {} has no model KMS key evidence",
                resource.resource_id
            ),
            json!({
                "resource_kind": resource_kind(resource),
                "model_kms_key_id": resource.resource_data.get("model_kms_key_id"),
            }),
        ));
    }

    if matches!(
        resource_kind(resource),
        Some("document_classifier") | Some("entity_recognizer")
    ) && data_str(&resource.resource_data, "volume_kms_key_id").is_none()
    {
        findings.push(finding(
            resource,
            Pillar::Security,
            REASON_SEC_VOLUME_WITHOUT_KMS,
            Severity::High,
            format!(
                "Comprehend resource {} has no training volume KMS key evidence",
                resource.resource_id
            ),
            json!({
                "resource_kind": resource_kind(resource),
                "volume_kms_key_id": resource.resource_data.get("volume_kms_key_id"),
            }),
        ));
    }

    if is_custom_model_resource(resource)
        && data_bool(&resource.resource_data, "output_configured")
        && !data_bool(&resource.resource_data, "output_kms_configured")
    {
        findings.push(finding(
            resource,
            Pillar::Security,
            REASON_SEC_OUTPUT_WITHOUT_KMS,
            Severity::High,
            format!(
                "Comprehend resource {} output is not KMS encrypted",
                resource.resource_id
            ),
            json!({
                "resource_kind": resource_kind(resource),
                "output_configured": resource.resource_data.get("output_configured"),
                "output_kms_configured": resource.resource_data.get("output_kms_configured"),
                "output_s3_uri": resource.resource_data.get("output_s3_uri"),
            }),
        ));
    }

    if resource_kind(resource) == Some("flywheel") {
        if data_str(&resource.resource_data, "data_lake_s3_uri").is_none() {
            findings.push(finding(
                resource,
                Pillar::Security,
                REASON_SEC_DATALAKE_MISSING,
                Severity::Medium,
                format!(
                    "Comprehend flywheel {} has no data lake evidence",
                    resource.resource_id
                ),
                json!({
                    "data_lake_s3_uri": resource.resource_data.get("data_lake_s3_uri"),
                }),
            ));
        } else if !data_bool(&resource.resource_data, "data_lake_kms_configured") {
            findings.push(finding(
                resource,
                Pillar::Security,
                REASON_SEC_DATALAKE_WITHOUT_KMS,
                Severity::High,
                format!(
                    "Comprehend flywheel {} data lake is not KMS encrypted",
                    resource.resource_id
                ),
                json!({
                    "data_lake_s3_uri": resource.resource_data.get("data_lake_s3_uri"),
                    "data_lake_kms_configured": resource.resource_data.get("data_lake_kms_configured"),
                }),
            ));
        }
    }

    if matches!(
        resource_kind(resource),
        Some("document_classifier") | Some("entity_recognizer")
    ) && !data_bool(&resource.resource_data, "vpc_configured")
    {
        findings.push(finding(
            resource,
            Pillar::Security,
            REASON_SEC_VPC_CONFIG_MISSING,
            Severity::Medium,
            format!(
                "Comprehend custom model resource {} has no VPC configuration evidence",
                resource.resource_id
            ),
            json!({
                "resource_kind": resource_kind(resource),
                "vpc_configured": resource.resource_data.get("vpc_configured"),
                "subnet_count": resource.resource_data.get("subnet_count"),
                "security_group_count": resource.resource_data.get("security_group_count"),
            }),
        ));
    }
}

fn emit_summary_count(
    resource: &AwsResourceModel,
    findings: &mut Vec<InventoryFinding>,
    pillar: Pillar,
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
        pillar,
        reason_code,
        severity,
        format!(
            "Comprehend account {} has {} {}",
            resource.resource_id, count, label
        ),
        json!({
            count_key: count,
            "resources_by_kind": resource.resource_data.get("resources_by_kind"),
        }),
    ));
}

fn is_custom_model_resource(resource: &AwsResourceModel) -> bool {
    matches!(
        resource_kind(resource),
        Some("document_classifier") | Some("entity_recognizer") | Some("flywheel")
    )
}

fn is_failed_status(status: &str) -> bool {
    let status = status.to_ascii_lowercase();
    status.contains("failed") || status.contains("error")
}

fn is_completed_status(status: &str) -> bool {
    matches!(
        status.to_ascii_lowercase().as_str(),
        "active"
            | "available"
            | "ready"
            | "trained"
            | "trained_with_warning"
            | "succeeded"
            | "completed"
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
            arn: format!("arn:aws:comprehend:us-east-1:123456789012:{}", resource_id),
            name: Some(resource_id.to_string()),
            tags,
            resource_data,
            created_at: now,
            updated_at: now,
            last_refreshed: now - Duration::hours(26),
        }
    }

    #[test]
    fn evaluates_comprehend_inventory_findings() {
        let now = Utc::now();
        let resources = vec![
            fixture(
                "comprehend:123456789012",
                json!({
                    "resource_kind": "account_summary",
                    "resource_count": 3,
                    "collection_error_count": 1,
                    "collection_errors": [{ "operation": "ListResources", "error": "denied" }],
                    "resources_by_kind": {
                        "document_classifier": 1,
                        "entity_recognizer": 1,
                        "flywheel": 1
                    },
                    "failed_resource_count": 1,
                    "incomplete_resource_count": 1,
                    "data_access_role_missing_count": 1,
                    "model_without_kms_count": 2,
                    "volume_without_kms_count": 2,
                    "output_without_kms_count": 1,
                    "data_lake_without_kms_count": 1,
                    "data_lake_missing_count": 1,
                    "vpc_config_missing_count": 2,
                }),
                json!({}),
            ),
            fixture(
                "document_classifier/invoices",
                json!({
                    "resource_kind": "document_classifier",
                    "status": "IN_ERROR",
                    "status_message": "training failed",
                    "language_code": "en",
                    "mode": "MULTI_CLASS",
                    "output_configured": true,
                    "output_kms_configured": false,
                    "output_s3_uri": "s3://comprehend-output/invoices/",
                    "vpc_configured": false,
                }),
                json!({}),
            ),
            fixture(
                "entity_recognizer/entities",
                json!({
                    "resource_kind": "entity_recognizer",
                    "status": "TRAINING",
                    "language_code": "en",
                    "data_access_role_arn": "arn:aws:iam::123456789012:role/comprehend-data",
                    "model_kms_key_id": "arn:aws:kms:us-east-1:123456789012:key/model",
                    "vpc_configured": false,
                }),
                json!({ "CostCenter": "ml" }),
            ),
            fixture(
                "flywheel/reviews",
                json!({
                    "resource_kind": "flywheel",
                    "status": "ACTIVE",
                    "model_type": "DOCUMENT_CLASSIFIER",
                    "data_access_role_arn": "arn:aws:iam::123456789012:role/comprehend-flywheel",
                    "data_lake_s3_uri": "s3://comprehend-flywheel/reviews/",
                    "data_lake_kms_configured": false,
                }),
                json!({}),
            ),
        ];

        let cost = evaluate_comprehend_fleet(&resources, Pillar::Cost, now);
        assert_eq!(cost.resources_evaluated, 4);
        assert_eq!(cost.stale_resources, 4);
        assert!(cost
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_COST_NO_TAGS));
        assert!(cost
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_COST_CUSTOM_MODEL_RESOURCE));
        assert!(cost
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_COST_FLYWHEEL_RESOURCE));

        let resilience = evaluate_comprehend_fleet(&resources, Pillar::Resilience, now);
        assert!(resilience
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_RES_COLLECTION_ERRORS));
        assert!(resilience
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_RES_FAILED_RESOURCE));
        assert!(resilience
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_RES_INCOMPLETE_RESOURCE));

        let security = evaluate_comprehend_fleet(&resources, Pillar::Security, now);
        assert!(security
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_SEC_DATA_ACCESS_ROLE_MISSING));
        assert!(security
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_SEC_MODEL_WITHOUT_KMS));
        assert!(security
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_SEC_VOLUME_WITHOUT_KMS));
        assert!(security
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_SEC_OUTPUT_WITHOUT_KMS));
        assert!(security
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_SEC_DATALAKE_WITHOUT_KMS));
        assert!(security
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_SEC_DATALAKE_MISSING));
        assert!(security
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_SEC_VPC_CONFIG_MISSING));
    }
}

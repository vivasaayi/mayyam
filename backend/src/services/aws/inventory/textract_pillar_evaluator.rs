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

// Deterministic AWS Textract adapter inventory evaluators for cost,
// resilience, and security pillars.

use chrono::{DateTime, Utc};
use serde_json::{json, Value};

use crate::models::aws_resource::Model as AwsResourceModel;
use crate::services::aws::inventory::types::{
    check_stale, has_any_tag, score_pillar, InventoryFinding, Pillar, PillarReport, Severity,
    COST_ALLOCATION_TAG_KEYS,
};

pub const RESOURCE_TYPE: &str = "TextractResource";

pub const REASON_COST_NO_TAGS: &str = "TEXTRACT_COST_NO_TAGS";
pub const REASON_COST_CUSTOM_ADAPTER_VERSION: &str = "TEXTRACT_COST_CUSTOM_ADAPTER_VERSION";
pub const REASON_RES_COLLECTION_ERRORS: &str = "TEXTRACT_RES_COLLECTION_ERRORS";
pub const REASON_RES_INACTIVE_ADAPTER: &str = "TEXTRACT_RES_INACTIVE_ADAPTER";
pub const REASON_RES_FAILED_ADAPTER_VERSION: &str = "TEXTRACT_RES_FAILED_ADAPTER_VERSION";
pub const REASON_RES_INCOMPLETE_ADAPTER_VERSION: &str = "TEXTRACT_RES_INCOMPLETE_ADAPTER_VERSION";
pub const REASON_SEC_ADAPTER_VERSION_WITHOUT_KMS: &str = "TEXTRACT_SEC_ADAPTER_VERSION_WITHOUT_KMS";
pub const REASON_SEC_OUTPUT_WITHOUT_KMS: &str = "TEXTRACT_SEC_OUTPUT_WITHOUT_KMS";
pub const REASON_SEC_OUTPUT_CONFIG_MISSING: &str = "TEXTRACT_SEC_OUTPUT_CONFIG_MISSING";
pub const REASON_INV_STALE_DATA: &str = "TEXTRACT_INV_STALE_DATA";

pub fn evaluate_textract_fleet(
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

fn data_usize(resource_data: &Value, key: &str) -> usize {
    resource_data
        .get(key)
        .and_then(|v| v.as_u64())
        .map(|v| v as usize)
        .unwrap_or(0)
}

fn data_bool(resource_data: &Value, key: &str) -> bool {
    resource_data
        .get(key)
        .and_then(|value| value.as_bool())
        .unwrap_or(false)
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
    if resource_kind(resource) == Some("adapter")
        && !has_any_tag(&resource.tags, COST_ALLOCATION_TAG_KEYS)
    {
        findings.push(finding(
            resource,
            Pillar::Cost,
            REASON_COST_NO_TAGS,
            Severity::Medium,
            format!(
                "Textract adapter {} has no cost allocation tags",
                resource.resource_id
            ),
            json!({
                "tags": resource.tags,
                "accepted_tag_keys": COST_ALLOCATION_TAG_KEYS,
            }),
        ));
    }

    if resource_kind(resource) == Some("adapter_version") {
        findings.push(finding(
            resource,
            Pillar::Cost,
            REASON_COST_CUSTOM_ADAPTER_VERSION,
            Severity::Low,
            format!(
                "Textract adapter version {} should be tracked as a custom ML asset",
                resource.resource_id
            ),
            json!({
                "adapter_id": resource.resource_data.get("adapter_id"),
                "adapter_version": resource.resource_data.get("adapter_version"),
                "feature_types": resource.resource_data.get("feature_types"),
                "status": resource.resource_data.get("status"),
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
                    "Textract account {} had {} inventory collection errors",
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
            "inactive_adapter_count",
            REASON_RES_INACTIVE_ADAPTER,
            Severity::Medium,
            "Textract inactive adapters",
        );
        emit_summary_count(
            resource,
            findings,
            Pillar::Resilience,
            "failed_adapter_version_count",
            REASON_RES_FAILED_ADAPTER_VERSION,
            Severity::High,
            "Textract failed adapter versions",
        );
        emit_summary_count(
            resource,
            findings,
            Pillar::Resilience,
            "incomplete_adapter_version_count",
            REASON_RES_INCOMPLETE_ADAPTER_VERSION,
            Severity::Medium,
            "Textract incomplete adapter versions",
        );
    }

    if resource_kind(resource) == Some("adapter")
        && data_str(&resource.resource_data, "status")
            .map(|status| !is_active_status(status))
            .unwrap_or(false)
    {
        findings.push(finding(
            resource,
            Pillar::Resilience,
            REASON_RES_INACTIVE_ADAPTER,
            Severity::Medium,
            format!("Textract adapter {} is not active", resource.resource_id),
            json!({
                "status": resource.resource_data.get("status"),
                "adapter_id": resource.resource_data.get("adapter_id"),
            }),
        ));
    }

    if resource_kind(resource) == Some("adapter_version") {
        let status = data_str(&resource.resource_data, "status");
        if status.map(is_failed_status).unwrap_or(false) {
            findings.push(finding(
                resource,
                Pillar::Resilience,
                REASON_RES_FAILED_ADAPTER_VERSION,
                Severity::High,
                format!(
                    "Textract adapter version {} is failed",
                    resource.resource_id
                ),
                json!({
                    "status": resource.resource_data.get("status"),
                    "status_message": resource.resource_data.get("status_message"),
                }),
            ));
        } else if status
            .map(|status| !is_completed_version_status(status))
            .unwrap_or(false)
        {
            findings.push(finding(
                resource,
                Pillar::Resilience,
                REASON_RES_INCOMPLETE_ADAPTER_VERSION,
                Severity::Medium,
                format!(
                    "Textract adapter version {} is incomplete",
                    resource.resource_id
                ),
                json!({
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
            "adapter_version_without_kms_count",
            REASON_SEC_ADAPTER_VERSION_WITHOUT_KMS,
            Severity::High,
            "Textract adapter versions without KMS keys",
        );
        emit_summary_count(
            resource,
            findings,
            Pillar::Security,
            "output_without_kms_count",
            REASON_SEC_OUTPUT_WITHOUT_KMS,
            Severity::High,
            "Textract output configurations without KMS keys",
        );
        emit_summary_count(
            resource,
            findings,
            Pillar::Security,
            "output_config_missing_count",
            REASON_SEC_OUTPUT_CONFIG_MISSING,
            Severity::Medium,
            "Textract adapter versions without output configuration",
        );
    }

    if resource_kind(resource) == Some("adapter_version")
        && data_str(&resource.resource_data, "kms_key_id").is_none()
    {
        findings.push(finding(
            resource,
            Pillar::Security,
            REASON_SEC_ADAPTER_VERSION_WITHOUT_KMS,
            Severity::High,
            format!(
                "Textract adapter version {} has no KMS key evidence",
                resource.resource_id
            ),
            json!({
                "adapter_id": resource.resource_data.get("adapter_id"),
                "adapter_version": resource.resource_data.get("adapter_version"),
                "kms_key_id": resource.resource_data.get("kms_key_id"),
            }),
        ));
    }

    if resource_kind(resource) == Some("adapter_version")
        && data_bool(&resource.resource_data, "output_configured")
        && !data_bool(&resource.resource_data, "output_kms_configured")
    {
        findings.push(finding(
            resource,
            Pillar::Security,
            REASON_SEC_OUTPUT_WITHOUT_KMS,
            Severity::High,
            format!(
                "Textract adapter version {} has output configuration without KMS key evidence",
                resource.resource_id
            ),
            json!({
                "adapter_id": resource.resource_data.get("adapter_id"),
                "adapter_version": resource.resource_data.get("adapter_version"),
                "output_configured": resource.resource_data.get("output_configured"),
                "output_kms_configured": resource.resource_data.get("output_kms_configured"),
            }),
        ));
    }

    if resource_kind(resource) == Some("adapter_version")
        && !data_bool(&resource.resource_data, "output_configured")
    {
        findings.push(finding(
            resource,
            Pillar::Security,
            REASON_SEC_OUTPUT_CONFIG_MISSING,
            Severity::Medium,
            format!(
                "Textract adapter version {} has no output configuration evidence",
                resource.resource_id
            ),
            json!({
                "adapter_id": resource.resource_data.get("adapter_id"),
                "adapter_version": resource.resource_data.get("adapter_version"),
                "output_configured": resource.resource_data.get("output_configured"),
                "output_kms_configured": resource.resource_data.get("output_kms_configured"),
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
            "Textract account {} has {} {}",
            resource.resource_id, count, label
        ),
        json!({
            count_key: count,
            "resources_by_kind": resource.resource_data.get("resources_by_kind"),
        }),
    ));
}

fn is_failed_status(status: &str) -> bool {
    let status = status.to_ascii_lowercase();
    status.contains("failed") || status.contains("error")
}

fn is_active_status(status: &str) -> bool {
    matches!(
        status.to_ascii_lowercase().as_str(),
        "active" | "available" | "ready" | "inservice" | "succeeded" | "completed"
    )
}

fn is_completed_version_status(status: &str) -> bool {
    matches!(
        status.to_ascii_lowercase().as_str(),
        "active" | "available" | "ready" | "succeeded" | "completed"
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
            arn: format!("arn:aws:textract:us-east-1:123456789012:{}", resource_id),
            name: Some(resource_id.to_string()),
            tags,
            resource_data,
            created_at: now,
            updated_at: now,
            last_refreshed: now - Duration::hours(26),
        }
    }

    #[test]
    fn evaluates_textract_inventory_findings() {
        let now = Utc::now();
        let resources = vec![
            fixture(
                "textract:123456789012",
                json!({
                    "resource_kind": "account_summary",
                    "resource_count": 4,
                    "collection_error_count": 1,
                    "collection_errors": [{ "operation": "ListAdapterVersions", "error": "denied" }],
                    "resources_by_kind": { "adapter": 2, "adapter_version": 2 },
                    "inactive_adapter_count": 1,
                    "failed_adapter_version_count": 1,
                    "incomplete_adapter_version_count": 1,
                    "adapter_version_without_kms_count": 2,
                    "output_without_kms_count": 1,
                    "output_config_missing_count": 1,
                }),
                json!({}),
            ),
            fixture(
                "adapter/invoices",
                json!({
                    "resource_kind": "adapter",
                    "adapter_id": "adapter-a",
                    "adapter_name": "invoices",
                    "status": "INACTIVE",
                    "feature_types": ["FORMS"],
                    "feature_type_count": 1,
                }),
                json!({}),
            ),
            fixture(
                "adapter_version/adapter-a/1",
                json!({
                    "resource_kind": "adapter_version",
                    "adapter_id": "adapter-a",
                    "adapter_version": "1",
                    "status": "CREATION_ERROR",
                    "status_message": "training failed",
                    "feature_types": ["FORMS"],
                    "feature_type_count": 1,
                    "output_configured": true,
                    "output_kms_configured": false,
                    "output_s3_bucket": "textract-output",
                }),
                json!({}),
            ),
            fixture(
                "adapter_version/adapter-a/2",
                json!({
                    "resource_kind": "adapter_version",
                    "adapter_id": "adapter-a",
                    "adapter_version": "2",
                    "status": "AT_RISK",
                    "status_message": "quality degraded",
                    "feature_types": ["FORMS"],
                    "feature_type_count": 1,
                    "output_configured": false,
                    "output_kms_configured": false,
                }),
                json!({}),
            ),
        ];

        let cost = evaluate_textract_fleet(&resources, Pillar::Cost, now);
        assert_eq!(cost.resources_evaluated, 4);
        assert_eq!(cost.stale_resources, 4);
        assert!(cost
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_COST_NO_TAGS));
        assert!(cost
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_COST_CUSTOM_ADAPTER_VERSION));

        let resilience = evaluate_textract_fleet(&resources, Pillar::Resilience, now);
        assert!(resilience
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_RES_COLLECTION_ERRORS));
        assert!(resilience
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_RES_INACTIVE_ADAPTER));
        assert!(resilience
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_RES_FAILED_ADAPTER_VERSION));
        assert!(resilience
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_RES_INCOMPLETE_ADAPTER_VERSION));

        let security = evaluate_textract_fleet(&resources, Pillar::Security, now);
        assert!(security
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_SEC_ADAPTER_VERSION_WITHOUT_KMS));
        assert!(security
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_SEC_OUTPUT_WITHOUT_KMS));
        assert!(security
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_SEC_OUTPUT_CONFIG_MISSING));
    }
}

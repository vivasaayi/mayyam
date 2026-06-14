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

// Deterministic Macie account inventory evaluators for the cost,
// resilience, and security pillars (roadmap rows
// 01-AWS-CLOUD-04222/04231/04258).

use chrono::{DateTime, Utc};
use serde_json::{json, Value};

use crate::models::aws_resource::Model as AwsResourceModel;
use crate::services::aws::inventory::types::{
    check_stale, has_any_tag, score_pillar, InventoryFinding, Pillar, PillarReport, Severity,
    COST_ALLOCATION_TAG_KEYS,
};

pub const RESOURCE_TYPE: &str = "MacieAccount";

pub const REASON_COST_NO_TAGS: &str = "MACIE_COST_NO_TAGS";
pub const REASON_COST_NO_CLASSIFICATION_JOBS: &str = "MACIE_COST_NO_CLASSIFICATION_JOBS";
pub const REASON_RES_ACCOUNT_NOT_ENABLED: &str = "MACIE_RES_ACCOUNT_NOT_ENABLED";
pub const REASON_RES_UNCLASSIFIED_BUCKETS: &str = "MACIE_RES_UNCLASSIFIED_BUCKETS";
pub const REASON_RES_METADATA_GAPS: &str = "MACIE_RES_METADATA_GAPS";
pub const REASON_SEC_AUTOMATED_DISCOVERY_DISABLED: &str = "MACIE_SEC_AUTOMATED_DISCOVERY_DISABLED";
pub const REASON_SEC_PUBLIC_BUCKETS: &str = "MACIE_SEC_PUBLIC_BUCKETS";
pub const REASON_SEC_SENSITIVE_DATA_FINDINGS: &str = "MACIE_SEC_SENSITIVE_DATA_FINDINGS";
pub const REASON_SEC_POLICY_FINDINGS: &str = "MACIE_SEC_POLICY_FINDINGS";
pub const REASON_INV_STALE_DATA: &str = "MACIE_INV_STALE_DATA";

pub fn evaluate_macie_fleet(
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

fn data_usize_in(resource_data: &Value, object_key: &str, key: &str) -> usize {
    resource_data
        .get(object_key)
        .and_then(|object| object.get(key))
        .and_then(|v| v.as_u64())
        .map(|v| v as usize)
        .unwrap_or(0)
}

fn normalized_data_str(resource_data: &Value, key: &str) -> Option<String> {
    data_str(resource_data, key).map(|value| value.trim().to_ascii_uppercase())
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
    if !has_any_tag(&resource.tags, COST_ALLOCATION_TAG_KEYS) {
        findings.push(finding(
            resource,
            Pillar::Cost,
            REASON_COST_NO_TAGS,
            Severity::Medium,
            format!(
                "Macie account {} has no cost allocation tags",
                resource.resource_id
            ),
            json!({ "tags": resource.tags }),
        ));
    }

    if data_usize(&resource.resource_data, "classification_job_count") == 0
        && data_usize(&resource.resource_data, "bucket_count") > 0
    {
        findings.push(finding(
            resource,
            Pillar::Cost,
            REASON_COST_NO_CLASSIFICATION_JOBS,
            Severity::Low,
            format!(
                "Macie account {} has S3 buckets but no classification jobs in the collected sample",
                resource.resource_id
            ),
            json!({
                "classification_job_count": resource.resource_data.get("classification_job_count"),
                "bucket_count": resource.resource_data.get("bucket_count"),
                "classification_jobs": resource.resource_data.get("classification_jobs"),
            }),
        ));
    }
}

fn evaluate_resilience(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    let status = normalized_data_str(&resource.resource_data, "status");
    if status.as_deref() != Some("ENABLED") {
        findings.push(finding(
            resource,
            Pillar::Resilience,
            REASON_RES_ACCOUNT_NOT_ENABLED,
            Severity::High,
            format!("Macie account {} is not enabled", resource.resource_id),
            json!({
                "status": resource.resource_data.get("status"),
                "service_role": resource.resource_data.get("service_role"),
            }),
        ));
    }

    let unclassified_buckets = data_usize(&resource.resource_data, "not_classified_bucket_count");
    if unclassified_buckets > 0 {
        findings.push(finding(
            resource,
            Pillar::Resilience,
            REASON_RES_UNCLASSIFIED_BUCKETS,
            Severity::Medium,
            format!(
                "Macie account {} has {} buckets without sensitivity classification",
                resource.resource_id, unclassified_buckets
            ),
            json!({
                "not_classified_bucket_count": unclassified_buckets,
                "bucket_statistics": resource.resource_data.get("bucket_statistics"),
            }),
        ));
    }

    let metadata_gap_count =
        data_usize(&resource.resource_data, "classification_error_bucket_count")
            + data_usize(&resource.resource_data, "unknown_permission_bucket_count")
            + data_usize(&resource.resource_data, "unknown_encryption_bucket_count")
            + data_usize(
                &resource.resource_data,
                "unknown_shared_access_bucket_count",
            );
    if metadata_gap_count > 0 {
        findings.push(finding(
            resource,
            Pillar::Resilience,
            REASON_RES_METADATA_GAPS,
            Severity::Medium,
            format!(
                "Macie account {} has {} bucket metadata or classification gaps",
                resource.resource_id, metadata_gap_count
            ),
            json!({
                "classification_error_bucket_count": resource.resource_data.get("classification_error_bucket_count"),
                "unknown_permission_bucket_count": resource.resource_data.get("unknown_permission_bucket_count"),
                "unknown_encryption_bucket_count": resource.resource_data.get("unknown_encryption_bucket_count"),
                "unknown_shared_access_bucket_count": resource.resource_data.get("unknown_shared_access_bucket_count"),
            }),
        ));
    }
}

fn evaluate_security(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    let automated_status =
        normalized_data_str(&resource.resource_data, "automated_discovery_status");
    if automated_status.as_deref() != Some("ENABLED") {
        findings.push(finding(
            resource,
            Pillar::Security,
            REASON_SEC_AUTOMATED_DISCOVERY_DISABLED,
            Severity::High,
            format!(
                "Macie account {} does not have automated sensitive data discovery enabled",
                resource.resource_id
            ),
            json!({
                "automated_discovery_status": resource.resource_data.get("automated_discovery_status"),
                "automated_discovery": resource.resource_data.get("automated_discovery"),
            }),
        ));
    }

    let public_buckets = data_usize(&resource.resource_data, "public_bucket_count");
    if public_buckets > 0 {
        findings.push(finding(
            resource,
            Pillar::Security,
            REASON_SEC_PUBLIC_BUCKETS,
            Severity::High,
            format!(
                "Macie account {} reports {} publicly accessible buckets",
                resource.resource_id, public_buckets
            ),
            json!({
                "public_bucket_count": public_buckets,
                "bucket_statistics": resource.resource_data.get("bucket_statistics"),
            }),
        ));
    }

    let sensitive_findings = data_usize(&resource.resource_data, "sensitive_data_finding_count")
        .max(data_usize_in(
            &resource.resource_data,
            "finding_summary",
            "sensitive_data_finding_count",
        ));
    if sensitive_findings > 0 {
        findings.push(finding(
            resource,
            Pillar::Security,
            REASON_SEC_SENSITIVE_DATA_FINDINGS,
            Severity::High,
            format!(
                "Macie account {} has {} sensitive data findings in the collected sample",
                resource.resource_id, sensitive_findings
            ),
            json!({
                "sensitive_data_finding_count": sensitive_findings,
                "finding_summary": resource.resource_data.get("finding_summary"),
            }),
        ));
    }

    let policy_findings =
        data_usize(&resource.resource_data, "policy_finding_count").max(data_usize_in(
            &resource.resource_data,
            "finding_summary",
            "policy_finding_count",
        ));
    if policy_findings > 0 {
        findings.push(finding(
            resource,
            Pillar::Security,
            REASON_SEC_POLICY_FINDINGS,
            Severity::Medium,
            format!(
                "Macie account {} has {} policy findings in the collected sample",
                resource.resource_id, policy_findings
            ),
            json!({
                "policy_finding_count": policy_findings,
                "finding_summary": resource.resource_data.get("finding_summary"),
            }),
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Duration, Utc};
    use serde_json::json;
    use uuid::Uuid;

    use crate::services::aws::inventory::types::Severity;

    fn resource(resource_data: serde_json::Value, tags: serde_json::Value) -> AwsResourceModel {
        let now = Utc::now();
        AwsResourceModel {
            id: Uuid::new_v4(),
            sync_id: Some(Uuid::new_v4()),
            account_id: "123456789012".to_string(),
            profile: Some("prod".to_string()),
            region: "us-east-1".to_string(),
            resource_type: RESOURCE_TYPE.to_string(),
            resource_id: "macie:us-east-1:123456789012".to_string(),
            arn: "arn:aws:macie2:us-east-1:123456789012:configuration".to_string(),
            name: Some("Macie us-east-1".to_string()),
            tags,
            resource_data,
            created_at: now,
            updated_at: now,
            last_refreshed: now - Duration::hours(96),
        }
    }

    #[test]
    fn evaluates_macie_inventory_findings() {
        let resource = resource(
            json!({
                "status": "PAUSED",
                "finding_publishing_frequency": "SIX_HOURS",
                "automated_discovery_status": "DISABLED",
                "classification_job_count": 0,
                "active_classification_job_count": 0,
                "bucket_count": 12,
                "not_classified_bucket_count": 2,
                "sensitive_bucket_count": 1,
                "public_bucket_count": 1,
                "finding_summary": {
                    "sample_count": 3,
                    "policy_finding_count": 1,
                    "sensitive_data_finding_count": 2
                },
            }),
            json!({}),
        );
        let now = Utc::now();

        let cost = evaluate_macie_fleet(std::slice::from_ref(&resource), Pillar::Cost, now);
        assert_eq!(cost.resources_evaluated, 1);
        assert!(cost
            .findings
            .iter()
            .any(|finding| finding.reason_code == "MACIE_COST_NO_TAGS"));
        assert!(cost
            .findings
            .iter()
            .any(|finding| finding.reason_code == "MACIE_COST_NO_CLASSIFICATION_JOBS"));

        let resilience =
            evaluate_macie_fleet(std::slice::from_ref(&resource), Pillar::Resilience, now);
        assert!(resilience
            .findings
            .iter()
            .any(|finding| finding.reason_code == "MACIE_RES_ACCOUNT_NOT_ENABLED"));
        assert!(resilience.findings.iter().any(|finding| {
            finding.reason_code == "MACIE_RES_UNCLASSIFIED_BUCKETS"
                && finding.severity == Severity::Medium
        }));

        let security = evaluate_macie_fleet(std::slice::from_ref(&resource), Pillar::Security, now);
        assert!(security
            .findings
            .iter()
            .any(|finding| finding.reason_code == "MACIE_SEC_AUTOMATED_DISCOVERY_DISABLED"));
        assert!(security
            .findings
            .iter()
            .any(|finding| finding.reason_code == "MACIE_SEC_SENSITIVE_DATA_FINDINGS"));
        assert!(security
            .findings
            .iter()
            .any(|finding| finding.reason_code == "MACIE_INV_STALE_DATA"));
    }
}

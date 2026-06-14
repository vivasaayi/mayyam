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

// Deterministic AWS Trusted Advisor account inventory evaluators for the cost,
// resilience, and security pillars (roadmap rows
// 01-AWS-CLOUD-04852/04861/04888).

use chrono::{DateTime, Utc};
use serde_json::{json, Value};

use crate::models::aws_resource::Model as AwsResourceModel;
use crate::services::aws::inventory::types::{
    check_stale, has_any_tag, score_pillar, InventoryFinding, Pillar, PillarReport, Severity,
    COST_ALLOCATION_TAG_KEYS,
};

pub const RESOURCE_TYPE: &str = "TrustedAdvisorAccount";

pub const REASON_COST_NO_TAGS: &str = "TRUSTEDADVISOR_COST_NO_TAGS";
pub const REASON_COST_SAVINGS_AVAILABLE: &str = "TRUSTEDADVISOR_COST_SAVINGS_AVAILABLE";
pub const REASON_COST_WARNING_RECOMMENDATIONS: &str = "TRUSTEDADVISOR_COST_WARNING_RECOMMENDATIONS";
pub const REASON_RES_WARNING_RECOMMENDATIONS: &str = "TRUSTEDADVISOR_RES_WARNING_RECOMMENDATIONS";
pub const REASON_RES_SERVICE_LIMIT_WARNINGS: &str = "TRUSTEDADVISOR_RES_SERVICE_LIMIT_WARNINGS";
pub const REASON_RES_RESOURCE_COLLECTION_ERRORS: &str =
    "TRUSTEDADVISOR_RES_RESOURCE_COLLECTION_ERRORS";
pub const REASON_SEC_WARNING_RECOMMENDATIONS: &str = "TRUSTEDADVISOR_SEC_WARNING_RECOMMENDATIONS";
pub const REASON_SEC_ERROR_RESOURCES: &str = "TRUSTEDADVISOR_SEC_ERROR_RESOURCES";
pub const REASON_SEC_EXCLUDED_RESOURCES: &str = "TRUSTEDADVISOR_SEC_EXCLUDED_RESOURCES";
pub const REASON_INV_NO_CHECKS: &str = "TRUSTEDADVISOR_INV_NO_CHECKS";
pub const REASON_INV_STALE_DATA: &str = "TRUSTEDADVISOR_INV_STALE_DATA";

pub fn evaluate_trustedadvisor_fleet(
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

        evaluate_inventory_presence(resource, pillar, &mut findings);
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

fn data_usize(resource_data: &Value, key: &str) -> usize {
    resource_data
        .get(key)
        .and_then(|v| v.as_u64())
        .map(|v| v as usize)
        .unwrap_or(0)
}

fn data_f64(resource_data: &Value, key: &str) -> f64 {
    resource_data
        .get(key)
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0)
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

fn evaluate_inventory_presence(
    resource: &AwsResourceModel,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if data_usize(&resource.resource_data, "check_count") == 0 {
        findings.push(finding(
            resource,
            pillar,
            REASON_INV_NO_CHECKS,
            Severity::Low,
            format!(
                "Trusted Advisor account {} has no checks in collected inventory",
                resource.resource_id
            ),
            json!({
                "check_count": resource.resource_data.get("check_count"),
                "checks": resource.resource_data.get("checks"),
            }),
        ));
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
                "Trusted Advisor account {} has no cost allocation tags",
                resource.resource_id
            ),
            json!({ "tags": resource.tags }),
        ));
    }

    let savings = data_f64(&resource.resource_data, "estimated_monthly_savings");
    if savings > 0.0 {
        findings.push(finding(
            resource,
            Pillar::Cost,
            REASON_COST_SAVINGS_AVAILABLE,
            Severity::High,
            format!(
                "Trusted Advisor account {} has estimated monthly savings of {:.2}",
                resource.resource_id, savings
            ),
            json!({
                "estimated_monthly_savings": savings,
                "recommendations": resource.resource_data.get("recommendations"),
            }),
        ));
    }

    let cost_warning_or_error = data_usize(&resource.resource_data, "cost_warning_or_error_count");
    if cost_warning_or_error > 0 {
        findings.push(finding(
            resource,
            Pillar::Cost,
            REASON_COST_WARNING_RECOMMENDATIONS,
            Severity::Medium,
            format!(
                "Trusted Advisor account {} has {} cost recommendations in warning or error status",
                resource.resource_id, cost_warning_or_error
            ),
            json!({
                "cost_warning_or_error_count": cost_warning_or_error,
                "cost_recommendation_count": resource.resource_data.get("cost_recommendation_count"),
                "recommendations": resource.resource_data.get("recommendations"),
            }),
        ));
    }
}

fn evaluate_resilience(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    let resilience_warning_or_error =
        data_usize(&resource.resource_data, "resilience_warning_or_error_count");
    if resilience_warning_or_error > 0 {
        findings.push(finding(
            resource,
            Pillar::Resilience,
            REASON_RES_WARNING_RECOMMENDATIONS,
            Severity::High,
            format!(
                "Trusted Advisor account {} has {} resilience recommendations in warning or error status",
                resource.resource_id, resilience_warning_or_error
            ),
            json!({
                "resilience_warning_or_error_count": resilience_warning_or_error,
                "resilience_recommendation_count": resource.resource_data.get("resilience_recommendation_count"),
                "recommendations": resource.resource_data.get("recommendations"),
            }),
        ));
    }

    let service_limit_warnings = data_usize(
        &resource.resource_data,
        "service_limit_warning_or_error_count",
    );
    if service_limit_warnings > 0 {
        findings.push(finding(
            resource,
            Pillar::Resilience,
            REASON_RES_SERVICE_LIMIT_WARNINGS,
            Severity::Medium,
            format!(
                "Trusted Advisor account {} has {} service limit recommendations in warning or error status",
                resource.resource_id, service_limit_warnings
            ),
            json!({
                "service_limit_warning_or_error_count": service_limit_warnings,
                "recommendations": resource.resource_data.get("recommendations"),
            }),
        ));
    }

    let collection_errors = data_usize(&resource.resource_data, "resource_collection_error_count");
    if collection_errors > 0 {
        findings.push(finding(
            resource,
            Pillar::Resilience,
            REASON_RES_RESOURCE_COLLECTION_ERRORS,
            Severity::Medium,
            format!(
                "Trusted Advisor account {} had {} recommendation resource collection errors",
                resource.resource_id, collection_errors
            ),
            json!({
                "resource_collection_error_count": collection_errors,
                "sampled_resource_count": resource.resource_data.get("sampled_resource_count"),
            }),
        ));
    }
}

fn evaluate_security(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    let security_warning_or_error =
        data_usize(&resource.resource_data, "security_warning_or_error_count");
    if security_warning_or_error > 0 {
        findings.push(finding(
            resource,
            Pillar::Security,
            REASON_SEC_WARNING_RECOMMENDATIONS,
            Severity::High,
            format!(
                "Trusted Advisor account {} has {} security recommendations in warning or error status",
                resource.resource_id, security_warning_or_error
            ),
            json!({
                "security_warning_or_error_count": security_warning_or_error,
                "security_recommendation_count": resource.resource_data.get("security_recommendation_count"),
                "recommendations": resource.resource_data.get("recommendations"),
            }),
        ));
    }

    let error_resources = data_usize(&resource.resource_data, "error_resource_count");
    if error_resources > 0 {
        findings.push(finding(
            resource,
            Pillar::Security,
            REASON_SEC_ERROR_RESOURCES,
            Severity::High,
            format!(
                "Trusted Advisor account {} has {} resources in error status",
                resource.resource_id, error_resources
            ),
            json!({
                "error_resource_count": error_resources,
                "sampled_resources": resource.resource_data.get("sampled_resources"),
            }),
        ));
    }

    let excluded_resources = data_usize(&resource.resource_data, "excluded_resource_count").max(
        data_usize(&resource.resource_data, "excluded_resource_sample_count"),
    );
    if excluded_resources > 0 {
        findings.push(finding(
            resource,
            Pillar::Security,
            REASON_SEC_EXCLUDED_RESOURCES,
            Severity::Medium,
            format!(
                "Trusted Advisor account {} has {} excluded recommendation resources",
                resource.resource_id, excluded_resources
            ),
            json!({
                "excluded_resource_count": resource.resource_data.get("excluded_resource_count"),
                "excluded_resource_sample_count": resource.resource_data.get("excluded_resource_sample_count"),
                "sampled_resources": resource.resource_data.get("sampled_resources"),
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

    fn make_resource(resource_data: Value, tags: Value) -> AwsResourceModel {
        let now = Utc::now();
        AwsResourceModel {
            id: Uuid::new_v4(),
            sync_id: Some(Uuid::new_v4()),
            account_id: "123456789012".to_string(),
            profile: Some("test".to_string()),
            region: "us-east-1".to_string(),
            resource_type: RESOURCE_TYPE.to_string(),
            resource_id: "trustedadvisor:123456789012".to_string(),
            arn: "arn:aws:trustedadvisor:us-east-1:123456789012:account/123456789012".to_string(),
            name: Some("Trusted Advisor".to_string()),
            tags,
            resource_data,
            created_at: now,
            updated_at: now,
            last_refreshed: now - Duration::hours(26),
        }
    }

    #[test]
    fn evaluates_trustedadvisor_inventory_findings() {
        let now = Utc::now();
        let resource = make_resource(
            json!({
                "check_count": 12,
                "recommendation_count": 5,
                "cost_recommendation_count": 2,
                "cost_warning_or_error_count": 1,
                "estimated_monthly_savings": 315.25,
                "resilience_recommendation_count": 2,
                "resilience_warning_or_error_count": 2,
                "service_limit_warning_or_error_count": 1,
                "security_recommendation_count": 1,
                "security_warning_or_error_count": 1,
                "error_resource_count": 3,
                "excluded_resource_count": 1,
                "excluded_resource_sample_count": 1,
                "resource_collection_error_count": 1,
                "sampled_resource_count": 10,
                "recommendations": [
                    { "id": "rec-cost", "status": "warning", "pillars": ["cost_optimizing"] },
                    { "id": "rec-res", "status": "error", "pillars": ["fault_tolerance"] },
                    { "id": "rec-sec", "status": "error", "pillars": ["security"] }
                ],
                "sampled_resources": [
                    { "id": "res-1", "status": "error", "exclusion_status": "excluded" }
                ]
            }),
            json!({}),
        );

        let cost =
            evaluate_trustedadvisor_fleet(std::slice::from_ref(&resource), Pillar::Cost, now);
        assert_eq!(cost.resources_evaluated, 1);
        assert_eq!(cost.stale_resources, 1);
        assert!(cost
            .findings
            .iter()
            .any(|f| f.reason_code == REASON_COST_NO_TAGS));
        assert!(cost
            .findings
            .iter()
            .any(|f| f.reason_code == REASON_COST_SAVINGS_AVAILABLE));
        assert!(cost
            .findings
            .iter()
            .any(|f| f.reason_code == REASON_COST_WARNING_RECOMMENDATIONS));

        let resilience =
            evaluate_trustedadvisor_fleet(std::slice::from_ref(&resource), Pillar::Resilience, now);
        assert!(resilience
            .findings
            .iter()
            .any(|f| f.reason_code == REASON_RES_WARNING_RECOMMENDATIONS));
        assert!(resilience
            .findings
            .iter()
            .any(|f| f.reason_code == REASON_RES_SERVICE_LIMIT_WARNINGS));
        assert!(resilience
            .findings
            .iter()
            .any(|f| f.reason_code == REASON_RES_RESOURCE_COLLECTION_ERRORS));

        let security =
            evaluate_trustedadvisor_fleet(std::slice::from_ref(&resource), Pillar::Security, now);
        assert!(security
            .findings
            .iter()
            .any(|f| f.reason_code == REASON_SEC_WARNING_RECOMMENDATIONS));
        assert!(security
            .findings
            .iter()
            .any(|f| f.reason_code == REASON_SEC_ERROR_RESOURCES));
        assert!(security
            .findings
            .iter()
            .any(|f| f.reason_code == REASON_SEC_EXCLUDED_RESOURCES));

        let empty_inventory = make_resource(
            json!({
                "check_count": 0,
                "recommendation_count": 0
            }),
            json!({"team": "sre"}),
        );
        let no_checks = evaluate_trustedadvisor_fleet(&[empty_inventory], Pillar::Resilience, now);
        assert!(no_checks
            .findings
            .iter()
            .any(|f| f.reason_code == REASON_INV_NO_CHECKS));
    }
}

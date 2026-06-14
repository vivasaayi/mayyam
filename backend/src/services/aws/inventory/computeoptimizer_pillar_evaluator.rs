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

// Deterministic AWS Compute Optimizer account inventory evaluators for the
// cost, resilience, and security pillars (roadmap rows
// 01-AWS-CLOUD-04915/04924/04951).

use chrono::{DateTime, Utc};
use serde_json::{json, Value};

use crate::models::aws_resource::Model as AwsResourceModel;
use crate::services::aws::inventory::types::{
    check_stale, has_any_tag, score_pillar, InventoryFinding, Pillar, PillarReport, Severity,
    COST_ALLOCATION_TAG_KEYS,
};

pub const RESOURCE_TYPE: &str = "ComputeOptimizerAccount";

pub const REASON_COST_NO_TAGS: &str = "COMPUTEOPTIMIZER_COST_NO_TAGS";
pub const REASON_COST_SAVINGS_AVAILABLE: &str = "COMPUTEOPTIMIZER_COST_SAVINGS_AVAILABLE";
pub const REASON_COST_RIGHTSIZING_OPPORTUNITIES: &str =
    "COMPUTEOPTIMIZER_COST_RIGHTSIZING_OPPORTUNITIES";
pub const REASON_RES_NOT_ACTIVE: &str = "COMPUTEOPTIMIZER_RES_NOT_ACTIVE";
pub const REASON_RES_PERFORMANCE_RISK: &str = "COMPUTEOPTIMIZER_RES_PERFORMANCE_RISK";
pub const REASON_RES_COLLECTION_ERRORS: &str = "COMPUTEOPTIMIZER_RES_COLLECTION_ERRORS";
pub const REASON_SEC_NOT_ACTIVE: &str = "COMPUTEOPTIMIZER_SEC_NOT_ACTIVE";
pub const REASON_SEC_EVIDENCE_GAPS: &str = "COMPUTEOPTIMIZER_SEC_EVIDENCE_GAPS";
pub const REASON_INV_NO_RECOMMENDATIONS: &str = "COMPUTEOPTIMIZER_INV_NO_RECOMMENDATIONS";
pub const REASON_INV_STALE_DATA: &str = "COMPUTEOPTIMIZER_INV_STALE_DATA";

pub fn evaluate_computeoptimizer_fleet(
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

fn data_i64(resource_data: &Value, key: &str) -> i64 {
    resource_data.get(key).and_then(|v| v.as_i64()).unwrap_or(0)
}

fn data_f64(resource_data: &Value, key: &str) -> f64 {
    resource_data
        .get(key)
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0)
}

fn data_str<'a>(resource_data: &'a Value, key: &str) -> Option<&'a str> {
    resource_data.get(key).and_then(|v| v.as_str())
}

fn is_enrolled(resource_data: &Value) -> bool {
    data_str(resource_data, "enrollment_status") == Some("Active")
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
    if data_usize(&resource.resource_data, "recommendation_summary_count") == 0 {
        findings.push(finding(
            resource,
            pillar,
            REASON_INV_NO_RECOMMENDATIONS,
            Severity::Low,
            format!(
                "Compute Optimizer account {} has no recommendation summaries in collected inventory",
                resource.resource_id
            ),
            json!({
                "recommendation_summary_count": resource.resource_data.get("recommendation_summary_count"),
                "enrollment_status": resource.resource_data.get("enrollment_status"),
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
                "Compute Optimizer account {} has no cost allocation tags",
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
                "Compute Optimizer account {} has estimated monthly savings of {:.2}",
                resource.resource_id, savings
            ),
            json!({
                "estimated_monthly_savings": savings,
                "recommendation_summaries": resource.resource_data.get("recommendation_summaries"),
            }),
        ));
    }

    let rightsizing_opportunities =
        data_usize(&resource.resource_data, "over_provisioned_resource_count")
            + data_usize(&resource.resource_data, "not_optimized_resource_count")
            + data_usize(&resource.resource_data, "idle_resource_count")
            + data_usize(&resource.resource_data, "sampled_over_provisioned_count")
            + data_usize(&resource.resource_data, "sampled_not_optimized_count");
    if rightsizing_opportunities > 0 {
        findings.push(finding(
            resource,
            Pillar::Cost,
            REASON_COST_RIGHTSIZING_OPPORTUNITIES,
            Severity::Medium,
            format!(
                "Compute Optimizer account {} has {} rightsizing opportunities",
                resource.resource_id, rightsizing_opportunities
            ),
            json!({
                "over_provisioned_resource_count": resource.resource_data.get("over_provisioned_resource_count"),
                "not_optimized_resource_count": resource.resource_data.get("not_optimized_resource_count"),
                "idle_resource_count": resource.resource_data.get("idle_resource_count"),
                "sampled_recommendations": resource.resource_data.get("sampled_recommendations"),
            }),
        ));
    }
}

fn evaluate_resilience(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    if !is_enrolled(&resource.resource_data) {
        findings.push(finding(
            resource,
            Pillar::Resilience,
            REASON_RES_NOT_ACTIVE,
            Severity::High,
            format!(
                "Compute Optimizer account {} enrollment is not active",
                resource.resource_id
            ),
            json!({
                "enrollment_status": resource.resource_data.get("enrollment_status"),
                "enrollment_status_reason": resource.resource_data.get("enrollment_status_reason"),
            }),
        ));
    }

    let performance_risk = data_usize(&resource.resource_data, "under_provisioned_resource_count")
        + data_i64(&resource.resource_data, "high_performance_risk_count").max(0) as usize
        + data_i64(&resource.resource_data, "medium_performance_risk_count").max(0) as usize
        + data_usize(&resource.resource_data, "sampled_under_provisioned_count")
        + data_usize(
            &resource.resource_data,
            "sampled_high_performance_risk_count",
        )
        + data_usize(
            &resource.resource_data,
            "sampled_medium_performance_risk_count",
        );
    if performance_risk > 0 {
        findings.push(finding(
            resource,
            Pillar::Resilience,
            REASON_RES_PERFORMANCE_RISK,
            Severity::High,
            format!(
                "Compute Optimizer account {} has {} resources with performance risk or under-provisioning evidence",
                resource.resource_id, performance_risk
            ),
            json!({
                "under_provisioned_resource_count": resource.resource_data.get("under_provisioned_resource_count"),
                "high_performance_risk_count": resource.resource_data.get("high_performance_risk_count"),
                "medium_performance_risk_count": resource.resource_data.get("medium_performance_risk_count"),
                "sampled_recommendations": resource.resource_data.get("sampled_recommendations"),
            }),
        ));
    }

    let collection_errors = data_usize(
        &resource.resource_data,
        "recommendation_collection_error_count",
    );
    if collection_errors > 0 {
        findings.push(finding(
            resource,
            Pillar::Resilience,
            REASON_RES_COLLECTION_ERRORS,
            Severity::Medium,
            format!(
                "Compute Optimizer account {} had {} recommendation collection errors",
                resource.resource_id, collection_errors
            ),
            json!({
                "recommendation_collection_error_count": collection_errors,
                "sampled_recommendation_count": resource.resource_data.get("sampled_recommendation_count"),
            }),
        ));
    }
}

fn evaluate_security(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    if !is_enrolled(&resource.resource_data) {
        findings.push(finding(
            resource,
            Pillar::Security,
            REASON_SEC_NOT_ACTIVE,
            Severity::Medium,
            format!(
                "Compute Optimizer account {} is not active, leaving compute optimization evidence incomplete",
                resource.resource_id
            ),
            json!({
                "enrollment_status": resource.resource_data.get("enrollment_status"),
                "enrollment_status_reason": resource.resource_data.get("enrollment_status_reason"),
            }),
        ));
    }

    let evidence_gap = data_usize(
        &resource.resource_data,
        "recommendation_collection_error_count",
    ) + data_usize(&resource.resource_data, "sampled_unavailable_count");
    if evidence_gap > 0 {
        findings.push(finding(
            resource,
            Pillar::Security,
            REASON_SEC_EVIDENCE_GAPS,
            Severity::Medium,
            format!(
                "Compute Optimizer account {} has {} unavailable or uncollected recommendation evidence items",
                resource.resource_id, evidence_gap
            ),
            json!({
                "recommendation_collection_error_count": resource.resource_data.get("recommendation_collection_error_count"),
                "sampled_unavailable_count": resource.resource_data.get("sampled_unavailable_count"),
                "sampled_recommendations": resource.resource_data.get("sampled_recommendations"),
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
            resource_id: "computeoptimizer:123456789012".to_string(),
            arn: "arn:aws:compute-optimizer:us-east-1:123456789012:account/123456789012"
                .to_string(),
            name: Some("Compute Optimizer".to_string()),
            tags,
            resource_data,
            created_at: now,
            updated_at: now,
            last_refreshed: now - Duration::hours(26),
        }
    }

    #[test]
    fn evaluates_computeoptimizer_inventory_findings() {
        let now = Utc::now();
        let resource = make_resource(
            json!({
                "enrollment_status": "Inactive",
                "enrollment_status_reason": "not opted in",
                "recommendation_summary_count": 3,
                "recommendation_count": 14,
                "over_provisioned_resource_count": 4,
                "not_optimized_resource_count": 2,
                "under_provisioned_resource_count": 3,
                "idle_resource_count": 1,
                "high_performance_risk_count": 1,
                "medium_performance_risk_count": 2,
                "estimated_monthly_savings": 542.75,
                "sampled_recommendation_count": 5,
                "sampled_over_provisioned_count": 1,
                "sampled_not_optimized_count": 1,
                "sampled_under_provisioned_count": 1,
                "sampled_high_performance_risk_count": 1,
                "sampled_medium_performance_risk_count": 1,
                "sampled_unavailable_count": 1,
                "recommendation_collection_error_count": 1,
                "recommendation_summaries": [
                    { "resource_type": "Ec2Instance", "summaries": [{ "name": "Overprovisioned", "value": 4.0 }] }
                ],
                "sampled_recommendations": [
                    { "resource_family": "lambda_function", "finding": "Unavailable" }
                ]
            }),
            json!({}),
        );

        let cost =
            evaluate_computeoptimizer_fleet(std::slice::from_ref(&resource), Pillar::Cost, now);
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
            .any(|f| f.reason_code == REASON_COST_RIGHTSIZING_OPPORTUNITIES));

        let resilience = evaluate_computeoptimizer_fleet(
            std::slice::from_ref(&resource),
            Pillar::Resilience,
            now,
        );
        assert!(resilience
            .findings
            .iter()
            .any(|f| f.reason_code == REASON_RES_NOT_ACTIVE));
        assert!(resilience
            .findings
            .iter()
            .any(|f| f.reason_code == REASON_RES_PERFORMANCE_RISK));
        assert!(resilience
            .findings
            .iter()
            .any(|f| f.reason_code == REASON_RES_COLLECTION_ERRORS));

        let security =
            evaluate_computeoptimizer_fleet(std::slice::from_ref(&resource), Pillar::Security, now);
        assert!(security
            .findings
            .iter()
            .any(|f| f.reason_code == REASON_SEC_NOT_ACTIVE));
        assert!(security
            .findings
            .iter()
            .any(|f| f.reason_code == REASON_SEC_EVIDENCE_GAPS));

        let empty_inventory = make_resource(
            json!({
                "enrollment_status": "Active",
                "recommendation_summary_count": 0,
                "recommendation_count": 0
            }),
            json!({"team": "sre"}),
        );
        let no_summaries =
            evaluate_computeoptimizer_fleet(&[empty_inventory], Pillar::Resilience, now);
        assert!(no_summaries
            .findings
            .iter()
            .any(|f| f.reason_code == REASON_INV_NO_RECOMMENDATIONS));
    }
}

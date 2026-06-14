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

// Deterministic AWS Resilience Hub account inventory evaluators for the
// cost, resilience, and security pillars (roadmap rows
// 01-AWS-CLOUD-05041/05050/05077).

use chrono::{DateTime, Utc};
use serde_json::{json, Value};

use crate::models::aws_resource::Model as AwsResourceModel;
use crate::services::aws::inventory::types::{
    check_stale, has_any_tag, score_pillar, InventoryFinding, Pillar, PillarReport, Severity,
    COST_ALLOCATION_TAG_KEYS,
};

pub const RESOURCE_TYPE: &str = "ResilienceHubAccount";

pub const REASON_COST_NO_TAGS: &str = "RESILIENCEHUB_COST_NO_TAGS";
pub const REASON_COST_UNTAGGED_APPS: &str = "RESILIENCEHUB_COST_UNTAGGED_APPS";
pub const REASON_COST_LOW_POLICY_COVERAGE: &str = "RESILIENCEHUB_COST_LOW_POLICY_COVERAGE";
pub const REASON_COST_ASSESSMENT_COSTS: &str = "RESILIENCEHUB_COST_ASSESSMENT_COSTS";
pub const REASON_RES_NO_APPS: &str = "RESILIENCEHUB_RES_NO_APPS";
pub const REASON_RES_NONCOMPLIANT_APPS: &str = "RESILIENCEHUB_RES_NONCOMPLIANT_APPS";
pub const REASON_RES_LOW_SCORE: &str = "RESILIENCEHUB_RES_LOW_SCORE";
pub const REASON_RES_DRIFT: &str = "RESILIENCEHUB_RES_DRIFT";
pub const REASON_RES_ASSESSMENT_FAILURES: &str = "RESILIENCEHUB_RES_ASSESSMENT_FAILURES";
pub const REASON_SEC_NO_EVENT_SUBSCRIPTIONS: &str = "RESILIENCEHUB_SEC_NO_EVENT_SUBSCRIPTIONS";
pub const REASON_SEC_EVIDENCE_GAPS: &str = "RESILIENCEHUB_SEC_EVIDENCE_GAPS";
pub const REASON_SEC_COMPONENT_NONCOMPLIANCE: &str = "RESILIENCEHUB_SEC_COMPONENT_NONCOMPLIANCE";
pub const REASON_INV_STALE_DATA: &str = "RESILIENCEHUB_INV_STALE_DATA";

pub fn evaluate_resiliencehub_fleet(
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

fn evaluate_cost(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    if !has_any_tag(&resource.tags, COST_ALLOCATION_TAG_KEYS) {
        findings.push(finding(
            resource,
            Pillar::Cost,
            REASON_COST_NO_TAGS,
            Severity::Medium,
            format!(
                "AWS Resilience Hub account {} has no cost allocation tags",
                resource.resource_id
            ),
            json!({ "tags": resource.tags }),
        ));
    }

    let app_count = data_usize(&resource.resource_data, "app_count");
    let tagged_apps = data_usize(&resource.resource_data, "app_with_tags_count");
    if app_count > tagged_apps {
        findings.push(finding(
            resource,
            Pillar::Cost,
            REASON_COST_UNTAGGED_APPS,
            Severity::Medium,
            format!(
                "AWS Resilience Hub account {} has {} apps without tags",
                resource.resource_id,
                app_count.saturating_sub(tagged_apps)
            ),
            json!({
                "app_count": app_count,
                "app_with_tags_count": tagged_apps,
                "apps": resource.resource_data.get("apps"),
            }),
        ));
    }

    let policy_linked_apps = data_usize(&resource.resource_data, "policy_linked_app_count");
    if app_count > 0 && app_count > policy_linked_apps {
        findings.push(finding(
            resource,
            Pillar::Cost,
            REASON_COST_LOW_POLICY_COVERAGE,
            Severity::Medium,
            format!(
                "AWS Resilience Hub account {} has {} apps without linked resiliency policies",
                resource.resource_id,
                app_count.saturating_sub(policy_linked_apps)
            ),
            json!({
                "app_count": app_count,
                "policy_linked_app_count": policy_linked_apps,
                "resiliency_policies": resource.resource_data.get("resiliency_policies"),
            }),
        ));
    }

    let estimated_costs = data_f64(&resource.resource_data, "estimated_cost_amount_total");
    if estimated_costs > 0.0 {
        findings.push(finding(
            resource,
            Pillar::Cost,
            REASON_COST_ASSESSMENT_COSTS,
            Severity::Low,
            format!(
                "AWS Resilience Hub account {} has assessment remediation cost estimates",
                resource.resource_id
            ),
            json!({
                "estimated_cost_amount_total": estimated_costs,
                "assessments": resource.resource_data.get("assessments"),
            }),
        ));
    }
}

fn evaluate_resilience(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    let app_count = data_usize(&resource.resource_data, "app_count");
    if app_count == 0 {
        findings.push(finding(
            resource,
            Pillar::Resilience,
            REASON_RES_NO_APPS,
            Severity::Medium,
            format!(
                "AWS Resilience Hub account {} has no registered applications",
                resource.resource_id
            ),
            json!({ "app_count": app_count }),
        ));
        return;
    }

    let noncompliant = data_usize(&resource.resource_data, "noncompliant_app_count")
        + data_usize(&resource.resource_data, "noncompliant_assessment_count")
        + data_usize(&resource.resource_data, "noncompliant_component_count");
    if noncompliant > 0 {
        findings.push(finding(
            resource,
            Pillar::Resilience,
            REASON_RES_NONCOMPLIANT_APPS,
            Severity::High,
            format!(
                "AWS Resilience Hub account {} has {} non-compliant app, assessment, or component records",
                resource.resource_id, noncompliant
            ),
            json!({
                "noncompliant_app_count": resource.resource_data.get("noncompliant_app_count"),
                "noncompliant_assessment_count": resource.resource_data.get("noncompliant_assessment_count"),
                "noncompliant_component_count": resource.resource_data.get("noncompliant_component_count"),
                "apps": resource.resource_data.get("apps"),
                "assessments": resource.resource_data.get("assessments"),
                "component_compliances": resource.resource_data.get("component_compliances"),
            }),
        ));
    }

    let low_scores = data_usize(&resource.resource_data, "low_resiliency_score_app_count")
        + data_usize(
            &resource.resource_data,
            "low_resiliency_score_assessment_count",
        );
    if low_scores > 0 {
        findings.push(finding(
            resource,
            Pillar::Resilience,
            REASON_RES_LOW_SCORE,
            Severity::High,
            format!(
                "AWS Resilience Hub account {} has {} low resiliency scores",
                resource.resource_id, low_scores
            ),
            json!({
                "low_resiliency_score_app_count": resource.resource_data.get("low_resiliency_score_app_count"),
                "low_resiliency_score_assessment_count": resource.resource_data.get("low_resiliency_score_assessment_count"),
                "apps": resource.resource_data.get("apps"),
                "assessments": resource.resource_data.get("assessments"),
            }),
        ));
    }

    let drifted = data_usize(&resource.resource_data, "drifted_app_count")
        + data_usize(&resource.resource_data, "drifted_assessment_count");
    if drifted > 0 {
        findings.push(finding(
            resource,
            Pillar::Resilience,
            REASON_RES_DRIFT,
            Severity::Medium,
            format!(
                "AWS Resilience Hub account {} has {} drifted app or assessment records",
                resource.resource_id, drifted
            ),
            json!({
                "drifted_app_count": resource.resource_data.get("drifted_app_count"),
                "drifted_assessment_count": resource.resource_data.get("drifted_assessment_count"),
                "apps": resource.resource_data.get("apps"),
                "assessments": resource.resource_data.get("assessments"),
            }),
        ));
    }

    let failed_or_missing = data_usize(&resource.resource_data, "failed_assessment_count")
        + data_usize(&resource.resource_data, "assessment_collection_error_count");
    if failed_or_missing > 0 {
        findings.push(finding(
            resource,
            Pillar::Resilience,
            REASON_RES_ASSESSMENT_FAILURES,
            Severity::Medium,
            format!(
                "AWS Resilience Hub account {} has {} failed or missing assessment records",
                resource.resource_id, failed_or_missing
            ),
            json!({
                "failed_assessment_count": resource.resource_data.get("failed_assessment_count"),
                "assessment_collection_error_count": resource.resource_data.get("assessment_collection_error_count"),
                "assessments": resource.resource_data.get("assessments"),
            }),
        ));
    }
}

fn evaluate_security(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    let app_count = data_usize(&resource.resource_data, "app_count");
    let event_subscriptions = data_usize(&resource.resource_data, "event_subscription_count");
    if app_count > 0 && event_subscriptions == 0 {
        findings.push(finding(
            resource,
            Pillar::Security,
            REASON_SEC_NO_EVENT_SUBSCRIPTIONS,
            Severity::Medium,
            format!(
                "AWS Resilience Hub account {} has apps but no event subscriptions",
                resource.resource_id
            ),
            json!({
                "app_count": app_count,
                "event_subscription_count": event_subscriptions,
                "apps": resource.resource_data.get("apps"),
            }),
        ));
    }

    let evidence_gaps = data_usize(&resource.resource_data, "app_detail_collection_error_count")
        + data_usize(&resource.resource_data, "assessment_collection_error_count")
        + data_usize(
            &resource.resource_data,
            "component_compliance_collection_error_count",
        );
    if evidence_gaps > 0 {
        findings.push(finding(
            resource,
            Pillar::Security,
            REASON_SEC_EVIDENCE_GAPS,
            Severity::Medium,
            format!(
                "AWS Resilience Hub account {} had {} inventory evidence collection errors",
                resource.resource_id, evidence_gaps
            ),
            json!({
                "app_detail_collection_error_count": resource.resource_data.get("app_detail_collection_error_count"),
                "assessment_collection_error_count": resource.resource_data.get("assessment_collection_error_count"),
                "component_compliance_collection_error_count": resource.resource_data.get("component_compliance_collection_error_count"),
            }),
        ));
    }

    let noncompliant_components =
        data_usize(&resource.resource_data, "noncompliant_component_count");
    if noncompliant_components > 0 {
        findings.push(finding(
            resource,
            Pillar::Security,
            REASON_SEC_COMPONENT_NONCOMPLIANCE,
            Severity::High,
            format!(
                "AWS Resilience Hub account {} has {} non-compliant app components",
                resource.resource_id, noncompliant_components
            ),
            json!({
                "noncompliant_component_count": noncompliant_components,
                "component_compliances": resource.resource_data.get("component_compliances"),
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
            resource_id: "resiliencehub:123456789012".to_string(),
            arn: "arn:aws:resiliencehub:us-east-1:123456789012:account/123456789012".to_string(),
            name: Some("AWS Resilience Hub".to_string()),
            tags,
            resource_data,
            created_at: now,
            updated_at: now,
            last_refreshed: now - Duration::hours(26),
        }
    }

    #[test]
    fn evaluates_resiliencehub_inventory_findings() {
        let now = Utc::now();
        let resource = make_resource(
            json!({
                "app_count": 2,
                "policy_count": 1,
                "app_detail_count": 2,
                "app_detail_collection_error_count": 1,
                "policy_linked_app_count": 1,
                "app_with_tags_count": 1,
                "event_subscription_count": 0,
                "daily_assessment_app_count": 1,
                "disabled_assessment_app_count": 1,
                "noncompliant_app_count": 1,
                "drifted_app_count": 1,
                "low_resiliency_score_app_count": 1,
                "assessment_count": 2,
                "assessment_collection_error_count": 1,
                "noncompliant_assessment_count": 1,
                "failed_assessment_count": 1,
                "drifted_assessment_count": 1,
                "low_resiliency_score_assessment_count": 1,
                "estimated_cost_amount_total": 42.0,
                "component_compliance_count": 2,
                "noncompliant_component_count": 1,
                "component_compliance_collection_error_count": 1,
                "apps": [
                    {
                        "app_arn": "arn:aws:resiliencehub:us-east-1:123456789012:app/app-1",
                        "name": "payments",
                        "compliance_status": "PolicyBreached",
                        "resiliency_score": 55.0
                    }
                ],
                "resiliency_policies": [
                    { "policy_arn": "arn:aws:resiliencehub:us-east-1:123456789012:resiliency-policy/policy-1" }
                ],
                "assessments": [
                    {
                        "assessment_arn": "arn:aws:resiliencehub:us-east-1:123456789012:app-assessment/app-1",
                        "assessment_status": "Failed",
                        "compliance_status": "PolicyBreached",
                        "resiliency_score": 55.0
                    }
                ],
                "component_compliances": [
                    { "app_component_name": "database", "status": "PolicyBreached" }
                ]
            }),
            json!({}),
        );

        let cost = evaluate_resiliencehub_fleet(std::slice::from_ref(&resource), Pillar::Cost, now);
        assert_eq!(cost.resources_evaluated, 1);
        assert_eq!(cost.stale_resources, 1);
        assert!(cost
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_COST_NO_TAGS));
        assert!(cost
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_COST_UNTAGGED_APPS));
        assert!(cost
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_COST_LOW_POLICY_COVERAGE));
        assert!(cost
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_COST_ASSESSMENT_COSTS));

        let resilience =
            evaluate_resiliencehub_fleet(std::slice::from_ref(&resource), Pillar::Resilience, now);
        assert!(resilience
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_RES_NONCOMPLIANT_APPS));
        assert!(resilience
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_RES_LOW_SCORE));
        assert!(resilience
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_RES_DRIFT));
        assert!(resilience
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_RES_ASSESSMENT_FAILURES));

        let security =
            evaluate_resiliencehub_fleet(std::slice::from_ref(&resource), Pillar::Security, now);
        assert!(security
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_SEC_NO_EVENT_SUBSCRIPTIONS));
        assert!(security
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_SEC_EVIDENCE_GAPS));
        assert!(security
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_SEC_COMPONENT_NONCOMPLIANCE));

        let empty = make_resource(
            json!({ "app_count": 0 }),
            json!({ "CostCenter": "platform" }),
        );
        let no_apps = evaluate_resiliencehub_fleet(&[empty], Pillar::Resilience, now);
        assert!(no_apps
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_RES_NO_APPS));
    }
}

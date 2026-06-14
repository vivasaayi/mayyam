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

// Deterministic Shield inventory evaluators for the cost, resilience, and
// security pillars (roadmap rows 01-AWS-CLOUD-03970/03979/04006).

use chrono::{DateTime, Utc};
use serde_json::{json, Value};

use crate::models::aws_resource::Model as AwsResourceModel;
use crate::services::aws::inventory::types::{
    check_stale, has_any_tag, score_pillar, InventoryFinding, Pillar, PillarReport, Severity,
    COST_ALLOCATION_TAG_KEYS,
};

pub const RESOURCE_TYPE: &str = "ShieldProtection";

pub const REASON_COST_NO_TAGS: &str = "SHIELD_COST_NO_TAGS";
pub const REASON_COST_NO_PROTECTED_RESOURCE: &str = "SHIELD_COST_NO_PROTECTED_RESOURCE";
pub const REASON_RES_NO_HEALTH_CHECKS: &str = "SHIELD_RES_NO_HEALTH_CHECKS";
pub const REASON_RES_AUTO_RENEW_DISABLED: &str = "SHIELD_RES_AUTO_RENEW_DISABLED";
pub const REASON_SEC_AUTO_RESPONSE_DISABLED: &str = "SHIELD_SEC_AUTO_RESPONSE_DISABLED";
pub const REASON_SEC_PROACTIVE_ENGAGEMENT_DISABLED: &str =
    "SHIELD_SEC_PROACTIVE_ENGAGEMENT_DISABLED";
pub const REASON_INV_STALE_DATA: &str = "SHIELD_INV_STALE_DATA";

pub fn evaluate_shield_fleet(
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

fn data_usize(resource_data: &Value, key: &str) -> Option<usize> {
    resource_data
        .get(key)
        .and_then(|v| v.as_u64())
        .map(|v| v as usize)
}

fn normalized_data_str(resource_data: &Value, key: &str) -> Option<String> {
    data_str(resource_data, key).map(|s| s.trim().to_ascii_uppercase())
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
                "Shield protection {} has no cost allocation tags",
                resource.resource_id
            ),
            json!({ "tags": resource.tags }),
        ));
    }

    if data_str(&resource.resource_data, "protected_resource_arn")
        .map(|arn| arn.trim().is_empty())
        .unwrap_or(true)
    {
        findings.push(finding(
            resource,
            Pillar::Cost,
            REASON_COST_NO_PROTECTED_RESOURCE,
            Severity::High,
            format!(
                "Shield protection {} has no protected resource ARN captured",
                resource.resource_id
            ),
            json!({ "protected_resource_arn": resource.resource_data.get("protected_resource_arn") }),
        ));
    }
}

fn evaluate_resilience(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    if data_usize(&resource.resource_data, "health_check_count").unwrap_or(0) == 0 {
        findings.push(finding(
            resource,
            Pillar::Resilience,
            REASON_RES_NO_HEALTH_CHECKS,
            Severity::Medium,
            format!(
                "Shield protection {} has no Route 53 health checks associated",
                resource.resource_id
            ),
            json!({ "health_check_ids": resource.resource_data.get("health_check_ids") }),
        ));
    }

    if normalized_data_str(&resource.resource_data, "subscription_auto_renew").as_deref()
        == Some("DISABLED")
    {
        findings.push(finding(
            resource,
            Pillar::Resilience,
            REASON_RES_AUTO_RENEW_DISABLED,
            Severity::Medium,
            format!(
                "Shield Advanced subscription auto-renew is disabled for protection {}",
                resource.resource_id
            ),
            json!({
                "subscription_auto_renew": resource.resource_data.get("subscription_auto_renew"),
                "subscription": resource.resource_data.get("subscription"),
            }),
        ));
    }
}

fn evaluate_security(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    match normalized_data_str(&resource.resource_data, "automatic_response_status").as_deref() {
        Some("ENABLED") => {}
        Some(status) => findings.push(finding(
            resource,
            Pillar::Security,
            REASON_SEC_AUTO_RESPONSE_DISABLED,
            Severity::Medium,
            format!(
                "Shield automatic application layer response is {} for protection {}",
                status, resource.resource_id
            ),
            json!({
                "automatic_response_status": resource.resource_data.get("automatic_response_status"),
                "automatic_response_action": resource.resource_data.get("automatic_response_action"),
            }),
        )),
        None => findings.push(finding(
            resource,
            Pillar::Security,
            REASON_SEC_AUTO_RESPONSE_DISABLED,
            Severity::Low,
            format!(
                "Shield automatic application layer response configuration is not captured for protection {}",
                resource.resource_id
            ),
            json!({ "automatic_response_status": Value::Null }),
        )),
    }

    if normalized_data_str(&resource.resource_data, "proactive_engagement_status").as_deref()
        != Some("ENABLED")
    {
        findings.push(finding(
            resource,
            Pillar::Security,
            REASON_SEC_PROACTIVE_ENGAGEMENT_DISABLED,
            Severity::Medium,
            format!(
                "Shield proactive engagement is not enabled for protection {}",
                resource.resource_id
            ),
            json!({
                "proactive_engagement_status": resource
                    .resource_data
                    .get("proactive_engagement_status"),
            }),
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;
    use uuid::Uuid;

    fn fixture(
        resource_id: &str,
        resource_data: Value,
        tags: Value,
        now: DateTime<Utc>,
        stale: bool,
    ) -> AwsResourceModel {
        AwsResourceModel {
            id: Uuid::new_v4(),
            sync_id: None,
            account_id: "123456789012".to_string(),
            profile: None,
            region: "us-east-1".to_string(),
            resource_type: RESOURCE_TYPE.to_string(),
            resource_id: resource_id.to_string(),
            arn: format!("arn:aws:shield::123456789012:protection/{}", resource_id),
            name: Some(resource_id.to_string()),
            tags,
            resource_data,
            created_at: now,
            updated_at: now,
            last_refreshed: if stale {
                now - Duration::hours(30)
            } else {
                now - Duration::hours(1)
            },
        }
    }

    #[test]
    fn evaluates_shield_inventory_findings() {
        let now = Utc::now();
        let resources = vec![
            fixture(
                "shield-risky",
                json!({
                    "protected_resource_arn": "",
                    "health_check_ids": [],
                    "health_check_count": 0,
                    "automatic_response_status": "DISABLED",
                    "automatic_response_action": "COUNT",
                    "subscription_auto_renew": "DISABLED",
                    "proactive_engagement_status": "DISABLED",
                }),
                json!({}),
                now,
                true,
            ),
            fixture(
                "shield-healthy",
                json!({
                    "protected_resource_arn": "arn:aws:cloudfront::123456789012:distribution/ABC",
                    "health_check_ids": ["hc-1"],
                    "health_check_count": 1,
                    "automatic_response_status": "ENABLED",
                    "automatic_response_action": "BLOCK",
                    "subscription_auto_renew": "ENABLED",
                    "proactive_engagement_status": "ENABLED",
                }),
                json!({ "CostCenter": "edge-platform" }),
                now,
                false,
            ),
        ];

        let cost = evaluate_shield_fleet(&resources, Pillar::Cost, now);
        assert_eq!(cost.resources_evaluated, 2);
        assert_eq!(cost.stale_resources, 1);
        assert!(cost
            .findings
            .iter()
            .any(|f| f.reason_code == REASON_COST_NO_TAGS));
        assert!(cost
            .findings
            .iter()
            .any(|f| f.reason_code == REASON_COST_NO_PROTECTED_RESOURCE));

        let resilience = evaluate_shield_fleet(&resources, Pillar::Resilience, now);
        assert!(resilience
            .findings
            .iter()
            .any(|f| f.reason_code == REASON_RES_NO_HEALTH_CHECKS));
        assert!(resilience
            .findings
            .iter()
            .any(|f| f.reason_code == REASON_RES_AUTO_RENEW_DISABLED));

        let security = evaluate_shield_fleet(&resources, Pillar::Security, now);
        assert!(security
            .findings
            .iter()
            .any(|f| f.reason_code == REASON_SEC_AUTO_RESPONSE_DISABLED));
        assert!(security
            .findings
            .iter()
            .any(|f| f.reason_code == REASON_SEC_PROACTIVE_ENGAGEMENT_DISABLED));
        assert!(security.score < 100);
    }
}

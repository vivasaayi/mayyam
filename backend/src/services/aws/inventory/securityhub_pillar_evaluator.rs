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

// Deterministic Security Hub hub inventory evaluators for the cost,
// resilience, and security pillars (roadmap rows
// 01-AWS-CLOUD-04096/04105/04132).

use chrono::{DateTime, Utc};
use serde_json::{json, Value};

use crate::models::aws_resource::Model as AwsResourceModel;
use crate::services::aws::inventory::types::{
    check_stale, has_any_tag, score_pillar, InventoryFinding, Pillar, PillarReport, Severity,
    COST_ALLOCATION_TAG_KEYS,
};

pub const RESOURCE_TYPE: &str = "SecurityHubHub";

pub const REASON_COST_NO_TAGS: &str = "SECURITYHUB_COST_NO_TAGS";
pub const REASON_COST_NO_PRODUCT_INTEGRATIONS: &str = "SECURITYHUB_COST_NO_PRODUCT_INTEGRATIONS";
pub const REASON_RES_STANDARDS_NOT_READY: &str = "SECURITYHUB_RES_STANDARDS_NOT_READY";
pub const REASON_RES_AUTO_ENABLE_CONTROLS_DISABLED: &str =
    "SECURITYHUB_RES_AUTO_ENABLE_CONTROLS_DISABLED";
pub const REASON_SEC_NO_ENABLED_STANDARDS: &str = "SECURITYHUB_SEC_NO_ENABLED_STANDARDS";
pub const REASON_SEC_NON_CONSOLIDATED_FINDINGS: &str = "SECURITYHUB_SEC_NON_CONSOLIDATED_FINDINGS";
pub const REASON_SEC_HIGH_CRITICAL_FINDINGS: &str = "SECURITYHUB_SEC_HIGH_CRITICAL_FINDINGS";
pub const REASON_SEC_FAILED_CONTROL_FINDINGS: &str = "SECURITYHUB_SEC_FAILED_CONTROL_FINDINGS";
pub const REASON_INV_STALE_DATA: &str = "SECURITYHUB_INV_STALE_DATA";

pub fn evaluate_securityhub_fleet(
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

fn data_bool(resource_data: &Value, key: &str) -> Option<bool> {
    resource_data.get(key).and_then(|v| v.as_bool())
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
                "Security Hub hub {} has no cost allocation tags",
                resource.resource_id
            ),
            json!({ "tags": resource.tags }),
        ));
    }

    if data_usize(&resource.resource_data, "product_subscription_count") == 0 {
        findings.push(finding(
            resource,
            Pillar::Cost,
            REASON_COST_NO_PRODUCT_INTEGRATIONS,
            Severity::Low,
            format!(
                "Security Hub hub {} has no enabled product integrations captured; finding ingestion cost and coverage cannot be attributed",
                resource.resource_id
            ),
            json!({
                "product_subscription_count": resource.resource_data.get("product_subscription_count"),
                "product_subscriptions": resource.resource_data.get("product_subscriptions"),
            }),
        ));
    }
}

fn evaluate_resilience(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    let standards_not_ready = data_usize(&resource.resource_data, "standards_not_ready_count");
    if standards_not_ready > 0 {
        findings.push(finding(
            resource,
            Pillar::Resilience,
            REASON_RES_STANDARDS_NOT_READY,
            Severity::High,
            format!(
                "Security Hub hub {} has {} standards not ready for evaluation",
                resource.resource_id, standards_not_ready
            ),
            json!({
                "standards_not_ready_count": standards_not_ready,
                "standards": resource.resource_data.get("standards"),
            }),
        ));
    }

    if data_bool(&resource.resource_data, "auto_enable_controls") == Some(false) {
        findings.push(finding(
            resource,
            Pillar::Resilience,
            REASON_RES_AUTO_ENABLE_CONTROLS_DISABLED,
            Severity::Medium,
            format!(
                "Security Hub hub {} does not automatically enable new controls",
                resource.resource_id
            ),
            json!({ "auto_enable_controls": false }),
        ));
    }
}

fn evaluate_security(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    if data_usize(&resource.resource_data, "enabled_standards_count") == 0 {
        findings.push(finding(
            resource,
            Pillar::Security,
            REASON_SEC_NO_ENABLED_STANDARDS,
            Severity::High,
            format!(
                "Security Hub hub {} has no enabled standards captured",
                resource.resource_id
            ),
            json!({
                "enabled_standards_count": resource.resource_data.get("enabled_standards_count"),
                "standards": resource.resource_data.get("standards"),
            }),
        ));
    }

    if normalized_data_str(&resource.resource_data, "control_finding_generator").as_deref()
        == Some("STANDARD_CONTROL")
    {
        findings.push(finding(
            resource,
            Pillar::Security,
            REASON_SEC_NON_CONSOLIDATED_FINDINGS,
            Severity::Medium,
            format!(
                "Security Hub hub {} is not using consolidated security-control findings",
                resource.resource_id
            ),
            json!({
                "control_finding_generator": resource.resource_data.get("control_finding_generator"),
            }),
        ));
    }

    let high_or_critical = data_usize(
        &resource.resource_data,
        "high_or_critical_active_unresolved_findings",
    );
    if high_or_critical > 0 {
        findings.push(finding(
            resource,
            Pillar::Security,
            REASON_SEC_HIGH_CRITICAL_FINDINGS,
            Severity::High,
            format!(
                "Security Hub hub {} has {} active unresolved high or critical findings in the collected sample",
                resource.resource_id, high_or_critical
            ),
            json!({
                "high_or_critical_active_unresolved_findings": high_or_critical,
                "finding_summary": resource.resource_data.get("finding_summary"),
            }),
        ));
    }

    let failed_controls = data_usize(&resource.resource_data, "failed_control_finding_count");
    if failed_controls > 0 {
        findings.push(finding(
            resource,
            Pillar::Security,
            REASON_SEC_FAILED_CONTROL_FINDINGS,
            Severity::Medium,
            format!(
                "Security Hub hub {} has {} failed control findings in the collected sample",
                resource.resource_id, failed_controls
            ),
            json!({
                "failed_control_finding_count": failed_controls,
                "finding_summary": resource.resource_data.get("finding_summary"),
            }),
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;
    use uuid::Uuid;

    fn fixture(tags: Value, resource_data: Value, now: DateTime<Utc>) -> AwsResourceModel {
        AwsResourceModel {
            id: Uuid::new_v4(),
            sync_id: None,
            account_id: "123456789012".to_string(),
            profile: None,
            region: "us-east-1".to_string(),
            resource_type: RESOURCE_TYPE.to_string(),
            resource_id: "securityhub:us-east-1:123456789012".to_string(),
            arn: "arn:aws:securityhub:us-east-1:123456789012:hub/default".to_string(),
            name: Some("Security Hub us-east-1".to_string()),
            tags,
            resource_data,
            created_at: now - Duration::hours(1),
            updated_at: now - Duration::hours(1),
            last_refreshed: now - Duration::hours(1),
        }
    }

    fn now() -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2026-06-11T12:00:00Z")
            .unwrap()
            .with_timezone(&Utc)
    }

    fn reason_codes(report: &PillarReport) -> Vec<String> {
        report
            .findings
            .iter()
            .map(|finding| finding.reason_code.clone())
            .collect()
    }

    #[test]
    fn evaluates_securityhub_inventory_findings() {
        let resource = fixture(
            json!({}),
            json!({
                "auto_enable_controls": false,
                "control_finding_generator": "STANDARD_CONTROL",
                "enabled_standards_count": 0,
                "standards_not_ready_count": 1,
                "product_subscription_count": 0,
                "product_subscriptions": [],
                "standards": [
                    {
                        "standards_arn": "arn:aws:securityhub:::ruleset/cis-aws-foundations-benchmark/v/1.2.0",
                        "status": "INCOMPLETE"
                    }
                ],
                "high_or_critical_active_unresolved_findings": 2,
                "failed_control_finding_count": 4,
                "finding_summary": {
                    "sample_count": 10,
                    "high_or_critical_active_unresolved_count": 2,
                    "failed_control_finding_count": 4
                }
            }),
            now(),
        );

        let cost = evaluate_securityhub_fleet(std::slice::from_ref(&resource), Pillar::Cost, now());
        let cost_reasons = reason_codes(&cost);
        assert_eq!(cost.resources_evaluated, 1);
        assert!(cost_reasons.contains(&REASON_COST_NO_TAGS.to_string()));
        assert!(cost_reasons.contains(&REASON_COST_NO_PRODUCT_INTEGRATIONS.to_string()));

        let resilience =
            evaluate_securityhub_fleet(std::slice::from_ref(&resource), Pillar::Resilience, now());
        let resilience_reasons = reason_codes(&resilience);
        assert!(resilience_reasons.contains(&REASON_RES_STANDARDS_NOT_READY.to_string()));
        assert!(resilience_reasons.contains(&REASON_RES_AUTO_ENABLE_CONTROLS_DISABLED.to_string()));

        let security = evaluate_securityhub_fleet(&[resource], Pillar::Security, now());
        let security_reasons = reason_codes(&security);
        assert!(security_reasons.contains(&REASON_SEC_NO_ENABLED_STANDARDS.to_string()));
        assert!(security_reasons.contains(&REASON_SEC_NON_CONSOLIDATED_FINDINGS.to_string()));
        assert!(security_reasons.contains(&REASON_SEC_HIGH_CRITICAL_FINDINGS.to_string()));
        assert!(security_reasons.contains(&REASON_SEC_FAILED_CONTROL_FINDINGS.to_string()));
    }
}

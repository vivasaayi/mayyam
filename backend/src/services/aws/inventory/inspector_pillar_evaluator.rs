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

// Deterministic Inspector account inventory evaluators for the cost,
// resilience, and security pillars (roadmap rows
// 01-AWS-CLOUD-04159/04168/04195).

use chrono::{DateTime, Utc};
use serde_json::{json, Value};

use crate::models::aws_resource::Model as AwsResourceModel;
use crate::services::aws::inventory::types::{
    check_stale, has_any_tag, score_pillar, InventoryFinding, Pillar, PillarReport, Severity,
    COST_ALLOCATION_TAG_KEYS,
};

pub const RESOURCE_TYPE: &str = "InspectorAccountCoverage";

pub const REASON_COST_NO_TAGS: &str = "INSPECTOR_COST_NO_TAGS";
pub const REASON_COST_NO_COVERAGE_DATA: &str = "INSPECTOR_COST_NO_COVERAGE_DATA";
pub const REASON_RES_ACCOUNT_NOT_ENABLED: &str = "INSPECTOR_RES_ACCOUNT_NOT_ENABLED";
pub const REASON_RES_RESOURCE_SCAN_DISABLED: &str = "INSPECTOR_RES_RESOURCE_SCAN_DISABLED";
pub const REASON_RES_COVERAGE_SCAN_FAILURES: &str = "INSPECTOR_RES_COVERAGE_SCAN_FAILURES";
pub const REASON_SEC_HIGH_CRITICAL_FINDINGS: &str = "INSPECTOR_SEC_HIGH_CRITICAL_FINDINGS";
pub const REASON_SEC_EXPLOIT_AVAILABLE: &str = "INSPECTOR_SEC_EXPLOIT_AVAILABLE";
pub const REASON_SEC_FIX_AVAILABLE: &str = "INSPECTOR_SEC_FIX_AVAILABLE";
pub const REASON_INV_STALE_DATA: &str = "INSPECTOR_INV_STALE_DATA";

pub fn evaluate_inspector_fleet(
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
                "Inspector account coverage {} has no cost allocation tags",
                resource.resource_id
            ),
            json!({ "tags": resource.tags }),
        ));
    }

    if data_usize(&resource.resource_data, "coverage_total_count") == 0 {
        findings.push(finding(
            resource,
            Pillar::Cost,
            REASON_COST_NO_COVERAGE_DATA,
            Severity::Low,
            format!(
                "Inspector account coverage {} has no covered resources in the collected inventory",
                resource.resource_id
            ),
            json!({
                "coverage_total_count": resource.resource_data.get("coverage_total_count"),
                "coverage_summary": resource.resource_data.get("coverage_summary"),
            }),
        ));
    }
}

fn evaluate_resilience(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    let account_status = normalized_data_str(&resource.resource_data, "account_status");
    if account_status.as_deref() != Some("ENABLED") {
        findings.push(finding(
            resource,
            Pillar::Resilience,
            REASON_RES_ACCOUNT_NOT_ENABLED,
            Severity::High,
            format!(
                "Inspector account coverage {} is not fully enabled",
                resource.resource_id
            ),
            json!({
                "account_status": resource.resource_data.get("account_status"),
                "account_error_code": resource.resource_data.get("account_error_code"),
                "account_error_message": resource.resource_data.get("account_error_message"),
            }),
        ));
    }

    let disabled_scans = disabled_scan_states(&resource.resource_data);
    if !disabled_scans.is_empty() {
        findings.push(finding(
            resource,
            Pillar::Resilience,
            REASON_RES_RESOURCE_SCAN_DISABLED,
            Severity::Medium,
            format!(
                "Inspector account coverage {} has {} resource scan states not enabled",
                resource.resource_id,
                disabled_scans.len()
            ),
            json!({
                "disabled_scan_states": disabled_scans,
                "scan_state_summary": resource.resource_data.get("scan_state_summary"),
            }),
        ));
    }

    let inactive_coverage = data_usize(&resource.resource_data, "inactive_coverage_count");
    if inactive_coverage > 0 {
        findings.push(finding(
            resource,
            Pillar::Resilience,
            REASON_RES_COVERAGE_SCAN_FAILURES,
            Severity::Medium,
            format!(
                "Inspector account coverage {} has {} covered resources with inactive scan status in the collected sample",
                resource.resource_id, inactive_coverage
            ),
            json!({
                "inactive_coverage_count": inactive_coverage,
                "coverage_summary": resource.resource_data.get("coverage_summary"),
            }),
        ));
    }
}

fn evaluate_security(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    let high_or_critical = data_usize(&resource.resource_data, "high_or_critical_active_findings");
    if high_or_critical > 0 {
        findings.push(finding(
            resource,
            Pillar::Security,
            REASON_SEC_HIGH_CRITICAL_FINDINGS,
            Severity::High,
            format!(
                "Inspector account coverage {} has {} active high or critical findings in the collected sample",
                resource.resource_id, high_or_critical
            ),
            json!({
                "high_or_critical_active_findings": high_or_critical,
                "finding_summary": resource.resource_data.get("finding_summary"),
            }),
        ));
    }

    let exploit_available = data_usize(&resource.resource_data, "exploit_available_findings");
    if exploit_available > 0 {
        findings.push(finding(
            resource,
            Pillar::Security,
            REASON_SEC_EXPLOIT_AVAILABLE,
            Severity::High,
            format!(
                "Inspector account coverage {} has {} active findings with exploits available in the collected sample",
                resource.resource_id, exploit_available
            ),
            json!({
                "exploit_available_findings": exploit_available,
                "finding_summary": resource.resource_data.get("finding_summary"),
            }),
        ));
    }

    let fix_available = data_usize(&resource.resource_data, "fix_available_findings");
    if fix_available > 0 {
        findings.push(finding(
            resource,
            Pillar::Security,
            REASON_SEC_FIX_AVAILABLE,
            Severity::Medium,
            format!(
                "Inspector account coverage {} has {} active findings with fixes available in the collected sample",
                resource.resource_id, fix_available
            ),
            json!({
                "fix_available_findings": fix_available,
                "finding_summary": resource.resource_data.get("finding_summary"),
            }),
        ));
    }
}

fn disabled_scan_states(resource_data: &Value) -> Vec<Value> {
    resource_data
        .get("scan_state_summary")
        .and_then(|value| value.as_object())
        .map(|summary| {
            summary
                .iter()
                .filter_map(|(scan_type, state)| {
                    let status = state
                        .get("status")
                        .and_then(|value| value.as_str())
                        .unwrap_or("UNKNOWN");
                    if status.eq_ignore_ascii_case("ENABLED") {
                        return None;
                    }
                    Some(json!({
                        "scan_type": scan_type,
                        "status": status,
                        "error_code": state.get("error_code"),
                        "error_message": state.get("error_message"),
                    }))
                })
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;
    use serde_json::{json, Value};
    use uuid::Uuid;

    fn fixture(tags: Value, resource_data: Value, now: DateTime<Utc>) -> AwsResourceModel {
        AwsResourceModel {
            id: Uuid::new_v4(),
            sync_id: None,
            account_id: "123456789012".to_string(),
            profile: None,
            region: "us-east-1".to_string(),
            resource_type: RESOURCE_TYPE.to_string(),
            resource_id: "inspector:us-east-1:123456789012".to_string(),
            arn: "arn:aws:inspector2:us-east-1:123456789012:account/123456789012".to_string(),
            name: Some("Inspector us-east-1".to_string()),
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
    fn evaluates_inspector_inventory_findings() {
        let resource = fixture(
            json!({}),
            json!({
                "account_status": "DISABLED",
                "coverage_total_count": 0,
                "scan_state_summary": {
                    "ec2": { "status": "ENABLED", "error_code": "ALREADY_ENABLED", "error_message": "" },
                    "ecr": { "status": "DISABLED", "error_code": "RESOURCE_NOT_FOUND", "error_message": "not enabled" },
                    "lambda": { "status": "SUSPENDED", "error_code": "ACCESS_DENIED", "error_message": "suspended" }
                },
                "inactive_coverage_count": 2,
                "coverage_summary": {
                    "inactive_coverage_count": 2,
                    "sample_resources": [
                        {
                            "resource_type": "AWS_EC2_INSTANCE",
                            "resource_id": "i-123",
                            "scan_status_code": "INACTIVE",
                            "scan_status_reason": "STALE_INVENTORY"
                        }
                    ]
                },
                "finding_summary": {
                    "sample_count": 8,
                    "high_or_critical_active_count": 2,
                    "exploit_available_count": 1,
                    "fix_available_count": 3
                },
                "high_or_critical_active_findings": 2,
                "exploit_available_findings": 1,
                "fix_available_findings": 3
            }),
            now(),
        );

        let cost = evaluate_inspector_fleet(std::slice::from_ref(&resource), Pillar::Cost, now());
        let cost_reasons = reason_codes(&cost);
        assert_eq!(cost.resources_evaluated, 1);
        assert!(cost_reasons.contains(&REASON_COST_NO_TAGS.to_string()));
        assert!(cost_reasons.contains(&REASON_COST_NO_COVERAGE_DATA.to_string()));

        let resilience =
            evaluate_inspector_fleet(std::slice::from_ref(&resource), Pillar::Resilience, now());
        let resilience_reasons = reason_codes(&resilience);
        assert!(resilience_reasons.contains(&REASON_RES_ACCOUNT_NOT_ENABLED.to_string()));
        assert!(resilience_reasons.contains(&REASON_RES_RESOURCE_SCAN_DISABLED.to_string()));
        assert!(resilience_reasons.contains(&REASON_RES_COVERAGE_SCAN_FAILURES.to_string()));

        let security = evaluate_inspector_fleet(&[resource], Pillar::Security, now());
        let security_reasons = reason_codes(&security);
        assert!(security_reasons.contains(&REASON_SEC_HIGH_CRITICAL_FINDINGS.to_string()));
        assert!(security_reasons.contains(&REASON_SEC_EXPLOIT_AVAILABLE.to_string()));
        assert!(security_reasons.contains(&REASON_SEC_FIX_AVAILABLE.to_string()));
    }
}

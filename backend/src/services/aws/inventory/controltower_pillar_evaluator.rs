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

// Deterministic AWS Control Tower inventory evaluators for the cost,
// resilience, and security pillars (roadmap rows
// 01-AWS-CLOUD-04726/04735/04762).

use chrono::{DateTime, Utc};
use serde_json::{json, Value};

use crate::models::aws_resource::Model as AwsResourceModel;
use crate::services::aws::inventory::types::{
    check_stale, has_any_tag, score_pillar, InventoryFinding, Pillar, PillarReport, Severity,
    COST_ALLOCATION_TAG_KEYS,
};

pub const RESOURCE_TYPE: &str = "ControlTowerLandingZone";

pub const REASON_COST_NO_TAGS: &str = "CONTROLTOWER_COST_NO_TAGS";
pub const REASON_COST_NO_REGISTERED_OUS: &str = "CONTROLTOWER_COST_NO_REGISTERED_OUS";
pub const REASON_RES_NO_LANDING_ZONE: &str = "CONTROLTOWER_RES_NO_LANDING_ZONE";
pub const REASON_RES_LANDING_ZONE_NOT_ACTIVE: &str = "CONTROLTOWER_RES_LANDING_ZONE_NOT_ACTIVE";
pub const REASON_RES_DRIFT_DETECTED: &str = "CONTROLTOWER_RES_DRIFT_DETECTED";
pub const REASON_SEC_NO_ENABLED_CONTROLS: &str = "CONTROLTOWER_SEC_NO_ENABLED_CONTROLS";
pub const REASON_SEC_CONTROL_FAILURES: &str = "CONTROLTOWER_SEC_CONTROL_FAILURES";
pub const REASON_SEC_UNMANAGED_OUS: &str = "CONTROLTOWER_SEC_UNMANAGED_OUS";
pub const REASON_INV_STALE_DATA: &str = "CONTROLTOWER_INV_STALE_DATA";

pub fn evaluate_controltower_fleet(
    resources: &[AwsResourceModel],
    pillar: Pillar,
    now: DateTime<Utc>,
) -> PillarReport {
    let mut findings: Vec<InventoryFinding> = Vec::new();
    let mut stale_resources = 0usize;
    let mut resources_evaluated = 0usize;

    for resource in resources {
        if resource.resource_type != RESOURCE_TYPE {
            continue;
        }
        resources_evaluated += 1;

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
        resources_evaluated,
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

fn data_str<'a>(resource_data: &'a Value, key: &str) -> Option<&'a str> {
    resource_data.get(key).and_then(|v| v.as_str())
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
                "Control Tower inventory {} has no cost allocation tags",
                resource.resource_id
            ),
            json!({ "tags": resource.tags }),
        ));
    }

    if data_usize(&resource.resource_data, "registered_ou_count") == 0 {
        findings.push(finding(
            resource,
            Pillar::Cost,
            REASON_COST_NO_REGISTERED_OUS,
            Severity::Medium,
            format!(
                "Control Tower inventory {} has no registered organizational units",
                resource.resource_id
            ),
            json!({
                "registered_ou_count": resource.resource_data.get("registered_ou_count"),
                "organizational_units": resource.resource_data.get("organizational_units"),
            }),
        ));
    }
}

fn evaluate_resilience(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    let landing_zone_count = data_usize(&resource.resource_data, "landing_zone_count");
    if landing_zone_count == 0 {
        findings.push(finding(
            resource,
            Pillar::Resilience,
            REASON_RES_NO_LANDING_ZONE,
            Severity::High,
            format!(
                "Control Tower inventory {} has no landing zones in collected evidence",
                resource.resource_id
            ),
            json!({
                "landing_zone_count": resource.resource_data.get("landing_zone_count"),
            }),
        ));
    }

    if landing_zone_count > 0
        && normalized_data_str(&resource.resource_data, "landing_zone_status").as_deref()
            != Some("ACTIVE")
    {
        findings.push(finding(
            resource,
            Pillar::Resilience,
            REASON_RES_LANDING_ZONE_NOT_ACTIVE,
            Severity::High,
            format!(
                "Control Tower landing zone {} is not active",
                resource.resource_id
            ),
            json!({
                "landing_zone_status": resource.resource_data.get("landing_zone_status"),
                "landing_zone_arn": resource.resource_data.get("landing_zone_arn"),
            }),
        ));
    }

    if matches!(
        normalized_data_str(&resource.resource_data, "landing_zone_drift_status").as_deref(),
        Some("DRIFTED")
    ) {
        findings.push(finding(
            resource,
            Pillar::Resilience,
            REASON_RES_DRIFT_DETECTED,
            Severity::High,
            format!(
                "Control Tower landing zone {} has drift detected",
                resource.resource_id
            ),
            json!({
                "landing_zone_drift_status": resource.resource_data.get("landing_zone_drift_status"),
                "landing_zone_drift_details": resource.resource_data.get("landing_zone_drift_details"),
            }),
        ));
    }
}

fn evaluate_security(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    if data_usize(&resource.resource_data, "enabled_control_count") == 0 {
        findings.push(finding(
            resource,
            Pillar::Security,
            REASON_SEC_NO_ENABLED_CONTROLS,
            Severity::High,
            format!(
                "Control Tower inventory {} has no enabled controls in collected evidence",
                resource.resource_id
            ),
            json!({
                "enabled_control_count": resource.resource_data.get("enabled_control_count"),
                "enabled_controls": resource.resource_data.get("enabled_controls"),
            }),
        ));
    }

    let failed_control_count = data_usize(&resource.resource_data, "failed_control_count");
    if failed_control_count > 0 {
        findings.push(finding(
            resource,
            Pillar::Security,
            REASON_SEC_CONTROL_FAILURES,
            Severity::High,
            format!(
                "Control Tower inventory {} has {} enabled controls with failed evidence",
                resource.resource_id, failed_control_count
            ),
            json!({
                "failed_control_count": failed_control_count,
                "enabled_controls": resource.resource_data.get("enabled_controls"),
            }),
        ));
    }

    let unmanaged_ou_count = data_usize(&resource.resource_data, "unmanaged_ou_count");
    if unmanaged_ou_count > 0 {
        findings.push(finding(
            resource,
            Pillar::Security,
            REASON_SEC_UNMANAGED_OUS,
            Severity::Medium,
            format!(
                "Control Tower inventory {} has {} unmanaged organizational units",
                resource.resource_id, unmanaged_ou_count
            ),
            json!({
                "unmanaged_ou_count": unmanaged_ou_count,
                "organizational_units": resource.resource_data.get("organizational_units"),
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
            region: "aws-global".to_string(),
            resource_type: RESOURCE_TYPE.to_string(),
            resource_id:
                "controltower:arn:aws:controltower:us-east-1:123456789012:landingzone/lz-abc123"
                    .to_string(),
            arn: "arn:aws:controltower:us-east-1:123456789012:landingzone/lz-abc123".to_string(),
            name: Some("lz-abc123".to_string()),
            tags,
            resource_data,
            created_at: now,
            updated_at: now,
            last_refreshed: now - Duration::hours(72),
        }
    }

    #[test]
    fn evaluates_controltower_inventory_findings() {
        let landing_zone_resource = resource(
            json!({
                "landing_zone_count": 1,
                "landing_zone_arn": "arn:aws:controltower:us-east-1:123456789012:landingzone/lz-abc123",
                "landing_zone_status": "FAILED",
                "landing_zone_drift_status": "DRIFTED",
                "landing_zone_drift_details": { "message": "baseline drift detected" },
                "registered_ou_count": 0,
                "unmanaged_ou_count": 2,
                "organizational_units": [
                    { "arn": "arn:aws:organizations::123456789012:ou/o-example/ou-1", "managed_by_control_tower": false }
                ],
                "enabled_control_count": 0,
                "failed_control_count": 2,
                "enabled_controls": [
                    { "arn": "arn:aws:controltower:us-east-1:123456789012:enabledcontrol/abc", "status": "FAILED" }
                ]
            }),
            json!({}),
        );
        let now = Utc::now();

        let cost = evaluate_controltower_fleet(
            std::slice::from_ref(&landing_zone_resource),
            Pillar::Cost,
            now,
        );
        assert_eq!(cost.resources_evaluated, 1);
        assert!(cost
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_COST_NO_TAGS));
        assert!(cost.findings.iter().any(|finding| {
            finding.reason_code == REASON_COST_NO_REGISTERED_OUS
                && finding.severity == Severity::Medium
        }));

        let resilience = evaluate_controltower_fleet(
            std::slice::from_ref(&landing_zone_resource),
            Pillar::Resilience,
            now,
        );
        assert!(resilience
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_RES_LANDING_ZONE_NOT_ACTIVE));
        assert!(resilience
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_RES_DRIFT_DETECTED));

        let missing_landing_zone = resource(json!({ "landing_zone_count": 0 }), json!({}));
        let missing_resilience = evaluate_controltower_fleet(
            std::slice::from_ref(&missing_landing_zone),
            Pillar::Resilience,
            now,
        );
        assert!(missing_resilience
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_RES_NO_LANDING_ZONE));

        let security = evaluate_controltower_fleet(
            std::slice::from_ref(&landing_zone_resource),
            Pillar::Security,
            now,
        );
        assert!(security
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_SEC_NO_ENABLED_CONTROLS));
        assert!(security
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_SEC_CONTROL_FAILURES));
        assert!(security
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_SEC_UNMANAGED_OUS));
        assert!(security
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_INV_STALE_DATA));
    }
}

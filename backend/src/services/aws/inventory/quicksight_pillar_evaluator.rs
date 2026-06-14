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

// Deterministic QuickSight inventory evaluators for the cost, resilience, and
// security pillars (roadmap rows 01-AWS-CLOUD-02206/02215/02242).

use chrono::{DateTime, Utc};
use serde_json::{json, Value};

use crate::models::aws_resource::Model as AwsResourceModel;
use crate::services::aws::inventory::types::{
    check_stale, has_any_tag, score_pillar, InventoryFinding, Pillar, PillarReport, Severity,
    COST_ALLOCATION_TAG_KEYS,
};

pub const RESOURCE_TYPE: &str = "QuickSightAsset";

pub const REASON_COST_NO_TAGS: &str = "QUICKSIGHT_COST_NO_TAGS";
pub const REASON_RES_ASSET_FAILED: &str = "QUICKSIGHT_RES_ASSET_FAILED";
pub const REASON_RES_DASHBOARD_NOT_PUBLISHED: &str = "QUICKSIGHT_RES_DASHBOARD_NOT_PUBLISHED";
pub const REASON_SEC_DATA_SOURCE_SSL_DISABLED: &str = "QUICKSIGHT_SEC_DATA_SOURCE_SSL_DISABLED";
pub const REASON_INV_STALE_DATA: &str = "QUICKSIGHT_INV_STALE_DATA";

pub fn evaluate_quicksight_fleet(
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

fn data_bool(resource_data: &Value, key: &str) -> Option<bool> {
    resource_data.get(key).and_then(|v| v.as_bool())
}

fn asset_kind(resource: &AwsResourceModel) -> Option<&str> {
    data_str(&resource.resource_data, "asset_kind")
}

fn normalized_status(resource: &AwsResourceModel) -> Option<String> {
    data_str(&resource.resource_data, "status").map(|s| s.trim().to_ascii_uppercase())
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
                "QuickSight asset {} has no cost allocation tags",
                resource.resource_id
            ),
            json!({ "tags": resource.tags }),
        ));
    }
}

fn evaluate_resilience(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    if asset_kind(resource) == Some("dashboard")
        && data_str(&resource.resource_data, "last_published_time").is_none()
    {
        findings.push(finding(
            resource,
            Pillar::Resilience,
            REASON_RES_DASHBOARD_NOT_PUBLISHED,
            Severity::Medium,
            format!(
                "QuickSight dashboard {} has no published version timestamp in inventory",
                resource.resource_id
            ),
            json!({
                "last_published_time": resource.resource_data.get("last_published_time"),
                "published_version_number": resource.resource_data.get("published_version_number"),
            }),
        ));
    }

    match normalized_status(resource).as_deref() {
        Some(status) if status.contains("FAILED") => findings.push(finding(
            resource,
            Pillar::Resilience,
            REASON_RES_ASSET_FAILED,
            Severity::High,
            format!(
                "QuickSight asset {} is in failed status {}",
                resource.resource_id, status
            ),
            json!({ "status": status }),
        )),
        _ => {}
    }
}

fn evaluate_security(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    if asset_kind(resource) == Some("data_source")
        && data_bool(&resource.resource_data, "disable_ssl") == Some(true)
    {
        findings.push(finding(
            resource,
            Pillar::Security,
            REASON_SEC_DATA_SOURCE_SSL_DISABLED,
            Severity::High,
            format!(
                "QuickSight data source {} has SSL disabled",
                resource.resource_id
            ),
            json!({
                "disable_ssl": true,
                "data_source_type": resource.resource_data.get("data_source_type"),
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
    ) -> AwsResourceModel {
        AwsResourceModel {
            id: Uuid::new_v4(),
            sync_id: None,
            account_id: "123456789012".to_string(),
            profile: None,
            region: "us-east-1".to_string(),
            resource_type: RESOURCE_TYPE.to_string(),
            resource_id: resource_id.to_string(),
            arn: format!("arn:aws:quicksight:us-east-1:123456789012:{}", resource_id),
            name: Some(resource_id.to_string()),
            tags,
            resource_data,
            created_at: now,
            updated_at: now,
            last_refreshed: now - Duration::hours(1),
        }
    }

    #[test]
    fn evaluates_quicksight_inventory_findings() {
        let now = Utc::now();
        let resources = vec![
            fixture(
                "dashboard/executive",
                json!({
                    "asset_kind": "dashboard",
                    "published_version_number": 0,
                }),
                json!({}),
                now,
            ),
            fixture(
                "analysis/broken",
                json!({
                    "asset_kind": "analysis",
                    "status": "CREATION_FAILED",
                }),
                json!({ "CostCenter": "analytics" }),
                now,
            ),
            fixture(
                "data-source/warehouse",
                json!({
                    "asset_kind": "data_source",
                    "data_source_type": "POSTGRESQL",
                    "disable_ssl": true,
                    "status": "UPDATE_SUCCESSFUL",
                }),
                json!({ "CostCenter": "analytics" }),
                now,
            ),
        ];

        let cost = evaluate_quicksight_fleet(&resources, Pillar::Cost, now);
        assert_eq!(cost.resources_evaluated, 3);
        assert!(cost
            .findings
            .iter()
            .any(|f| f.reason_code == REASON_COST_NO_TAGS));

        let resilience = evaluate_quicksight_fleet(&resources, Pillar::Resilience, now);
        assert!(resilience
            .findings
            .iter()
            .any(|f| f.reason_code == REASON_RES_DASHBOARD_NOT_PUBLISHED));
        assert!(resilience
            .findings
            .iter()
            .any(|f| f.reason_code == REASON_RES_ASSET_FAILED));

        let security = evaluate_quicksight_fleet(&resources, Pillar::Security, now);
        assert!(security
            .findings
            .iter()
            .any(|f| f.reason_code == REASON_SEC_DATA_SOURCE_SSL_DISABLED));
    }
}

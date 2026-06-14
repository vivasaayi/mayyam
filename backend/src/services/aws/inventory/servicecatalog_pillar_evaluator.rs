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

// Deterministic AWS Service Catalog inventory evaluators for the cost,
// resilience, and security pillars (roadmap rows
// 01-AWS-CLOUD-04789/04798/04825).

use chrono::{DateTime, Utc};
use serde_json::{json, Value};

use crate::models::aws_resource::Model as AwsResourceModel;
use crate::services::aws::inventory::types::{
    check_stale, has_any_tag, score_pillar, InventoryFinding, Pillar, PillarReport, Severity,
    COST_ALLOCATION_TAG_KEYS,
};

pub const RESOURCE_TYPE: &str = "ServiceCatalogPortfolio";

pub const REASON_COST_NO_TAGS: &str = "SERVICECATALOG_COST_NO_TAGS";
pub const REASON_COST_NO_BUDGETS: &str = "SERVICECATALOG_COST_NO_BUDGETS";
pub const REASON_RES_NO_PRODUCTS: &str = "SERVICECATALOG_RES_NO_PRODUCTS";
pub const REASON_RES_NO_ACTIVE_ARTIFACTS: &str = "SERVICECATALOG_RES_NO_ACTIVE_ARTIFACTS";
pub const REASON_RES_NO_LAUNCH_PATHS: &str = "SERVICECATALOG_RES_NO_LAUNCH_PATHS";
pub const REASON_SEC_NO_CONSTRAINTS: &str = "SERVICECATALOG_SEC_NO_CONSTRAINTS";
pub const REASON_SEC_ACCOUNT_SHARE: &str = "SERVICECATALOG_SEC_ACCOUNT_SHARE";
pub const REASON_SEC_INACTIVE_TAG_OPTIONS: &str = "SERVICECATALOG_SEC_INACTIVE_TAG_OPTIONS";
pub const REASON_INV_STALE_DATA: &str = "SERVICECATALOG_INV_STALE_DATA";

pub fn evaluate_servicecatalog_fleet(
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

fn data_bool(resource_data: &Value, key: &str) -> Option<bool> {
    resource_data.get(key).and_then(|v| v.as_bool())
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
    if !has_any_tag(&resource.tags, COST_ALLOCATION_TAG_KEYS)
        && data_usize(&resource.resource_data, "active_cost_tag_option_count") == 0
    {
        findings.push(finding(
            resource,
            Pillar::Cost,
            REASON_COST_NO_TAGS,
            Severity::Medium,
            format!(
                "Service Catalog portfolio {} has no cost allocation tags or active tag options",
                resource.resource_id
            ),
            json!({
                "tags": resource.tags,
                "tag_options": resource.resource_data.get("tag_options"),
            }),
        ));
    }

    if data_usize(&resource.resource_data, "budget_count") == 0 {
        findings.push(finding(
            resource,
            Pillar::Cost,
            REASON_COST_NO_BUDGETS,
            Severity::Medium,
            format!(
                "Service Catalog portfolio {} has no associated budgets in collected evidence",
                resource.resource_id
            ),
            json!({
                "budget_count": resource.resource_data.get("budget_count"),
                "budgets": resource.resource_data.get("budgets"),
            }),
        ));
    }
}

fn evaluate_resilience(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    if data_usize(&resource.resource_data, "product_count") == 0 {
        findings.push(finding(
            resource,
            Pillar::Resilience,
            REASON_RES_NO_PRODUCTS,
            Severity::High,
            format!(
                "Service Catalog portfolio {} has no products in collected evidence",
                resource.resource_id
            ),
            json!({
                "product_count": resource.resource_data.get("product_count"),
                "products": resource.resource_data.get("products"),
            }),
        ));
    }

    if data_usize(&resource.resource_data, "product_count") > 0
        && data_usize(
            &resource.resource_data,
            "active_provisioning_artifact_count",
        ) == 0
    {
        findings.push(finding(
            resource,
            Pillar::Resilience,
            REASON_RES_NO_ACTIVE_ARTIFACTS,
            Severity::High,
            format!(
                "Service Catalog portfolio {} has no active provisioning artifacts",
                resource.resource_id
            ),
            json!({
                "active_provisioning_artifact_count": resource.resource_data.get("active_provisioning_artifact_count"),
                "deprecated_provisioning_artifact_count": resource.resource_data.get("deprecated_provisioning_artifact_count"),
                "products": resource.resource_data.get("products"),
            }),
        ));
    }

    if data_usize(&resource.resource_data, "product_count") > 0
        && data_usize(&resource.resource_data, "launch_path_count") == 0
        && data_bool(&resource.resource_data, "has_default_launch_path") != Some(true)
    {
        findings.push(finding(
            resource,
            Pillar::Resilience,
            REASON_RES_NO_LAUNCH_PATHS,
            Severity::Medium,
            format!(
                "Service Catalog portfolio {} has no launch paths for collected products",
                resource.resource_id
            ),
            json!({
                "launch_path_count": resource.resource_data.get("launch_path_count"),
                "has_default_launch_path": resource.resource_data.get("has_default_launch_path"),
                "products": resource.resource_data.get("products"),
            }),
        ));
    }
}

fn evaluate_security(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    if data_usize(&resource.resource_data, "constraint_count") == 0 {
        findings.push(finding(
            resource,
            Pillar::Security,
            REASON_SEC_NO_CONSTRAINTS,
            Severity::High,
            format!(
                "Service Catalog portfolio {} has no constraints in collected evidence",
                resource.resource_id
            ),
            json!({
                "constraint_count": resource.resource_data.get("constraint_count"),
                "constraints": resource.resource_data.get("constraints"),
            }),
        ));
    }

    if data_usize(&resource.resource_data, "broad_share_count") > 0 {
        findings.push(finding(
            resource,
            Pillar::Security,
            REASON_SEC_ACCOUNT_SHARE,
            Severity::High,
            format!(
                "Service Catalog portfolio {} has broad account or organization shares",
                resource.resource_id
            ),
            json!({
                "broad_share_count": resource.resource_data.get("broad_share_count"),
                "portfolio_shares": resource.resource_data.get("portfolio_shares"),
            }),
        ));
    }

    if data_usize(&resource.resource_data, "inactive_tag_option_count") > 0 {
        findings.push(finding(
            resource,
            Pillar::Security,
            REASON_SEC_INACTIVE_TAG_OPTIONS,
            Severity::Medium,
            format!(
                "Service Catalog portfolio {} has inactive tag options",
                resource.resource_id
            ),
            json!({
                "inactive_tag_option_count": resource.resource_data.get("inactive_tag_option_count"),
                "tag_options": resource.resource_data.get("tag_options"),
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
            resource_id: "servicecatalog:port-abc123".to_string(),
            arn: "arn:aws:servicecatalog:us-east-1:123456789012:portfolio/port-abc123".to_string(),
            name: Some("platform-products".to_string()),
            tags,
            resource_data,
            created_at: now,
            updated_at: now,
            last_refreshed: now - Duration::hours(26),
        }
    }

    #[test]
    fn evaluates_servicecatalog_inventory_findings() {
        let now = Utc::now();
        let resource = make_resource(
            json!({
                "portfolio_id": "port-abc123",
                "product_count": 1,
                "budget_count": 0,
                "active_cost_tag_option_count": 0,
                "active_provisioning_artifact_count": 0,
                "deprecated_provisioning_artifact_count": 1,
                "launch_path_count": 0,
                "has_default_launch_path": false,
                "constraint_count": 0,
                "broad_share_count": 1,
                "inactive_tag_option_count": 1,
                "tag_options": [
                    { "key": "CostCenter", "value": "cc-1", "active": false }
                ],
                "portfolio_shares": [
                    { "principal_id": "o-example", "type": "ORGANIZATION", "accepted": true }
                ],
                "products": [
                    {
                        "product_id": "prod-1",
                        "name": "baseline-vpc",
                        "has_default_path": false,
                        "provisioning_artifacts": [
                            { "id": "pa-1", "active": false, "guidance": "DEPRECATED" }
                        ],
                        "launch_paths": []
                    }
                ]
            }),
            json!({}),
        );

        let cost =
            evaluate_servicecatalog_fleet(std::slice::from_ref(&resource), Pillar::Cost, now);
        assert_eq!(cost.resources_evaluated, 1);
        assert_eq!(cost.stale_resources, 1);
        assert!(cost
            .findings
            .iter()
            .any(|f| f.reason_code == REASON_COST_NO_TAGS));
        assert!(cost
            .findings
            .iter()
            .any(|f| f.reason_code == REASON_COST_NO_BUDGETS));
        assert!(cost.score < 100);

        let resilience =
            evaluate_servicecatalog_fleet(std::slice::from_ref(&resource), Pillar::Resilience, now);
        assert!(resilience
            .findings
            .iter()
            .any(|f| f.reason_code == REASON_RES_NO_ACTIVE_ARTIFACTS));
        assert!(resilience
            .findings
            .iter()
            .any(|f| f.reason_code == REASON_RES_NO_LAUNCH_PATHS));

        let empty_portfolio = make_resource(
            json!({
                "product_count": 0,
                "budget_count": 0,
                "active_cost_tag_option_count": 0,
                "active_provisioning_artifact_count": 0,
                "launch_path_count": 0,
                "has_default_launch_path": false,
                "constraint_count": 0,
                "broad_share_count": 0,
                "inactive_tag_option_count": 0,
                "products": []
            }),
            json!({"owner": "platform"}),
        );
        let no_products =
            evaluate_servicecatalog_fleet(&[empty_portfolio], Pillar::Resilience, now);
        assert!(no_products
            .findings
            .iter()
            .any(|f| f.reason_code == REASON_RES_NO_PRODUCTS));

        let security =
            evaluate_servicecatalog_fleet(std::slice::from_ref(&resource), Pillar::Security, now);
        assert!(security
            .findings
            .iter()
            .any(|f| f.reason_code == REASON_SEC_NO_CONSTRAINTS));
        assert!(security
            .findings
            .iter()
            .any(|f| f.reason_code == REASON_SEC_ACCOUNT_SHARE));
        assert!(security
            .findings
            .iter()
            .any(|f| f.reason_code == REASON_SEC_INACTIVE_TAG_OPTIONS));
    }
}

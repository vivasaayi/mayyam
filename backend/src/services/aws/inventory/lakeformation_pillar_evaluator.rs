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

// Deterministic Lake Formation inventory evaluators for the cost,
// resilience, and security pillars (roadmap rows
// 01-AWS-CLOUD-02269/02278/02305).

use chrono::{DateTime, Utc};
use serde_json::{json, Value};

use crate::models::aws_resource::Model as AwsResourceModel;
use crate::services::aws::inventory::types::{
    check_stale, score_pillar, InventoryFinding, Pillar, PillarReport, Severity,
};

pub const RESOURCE_TYPE: &str = "LakeFormationDataLake";

pub const REASON_COST_NO_LF_TAGS: &str = "LAKEFORMATION_COST_NO_LF_TAGS";
pub const REASON_RES_NO_DATA_LAKE_ADMINS: &str = "LAKEFORMATION_RES_NO_DATA_LAKE_ADMINS";
pub const REASON_RES_REGISTERED_RESOURCE_WITHOUT_ROLE: &str =
    "LAKEFORMATION_RES_REGISTERED_RESOURCE_WITHOUT_ROLE";
pub const REASON_SEC_DEFAULT_PERMISSIONS_PRESENT: &str =
    "LAKEFORMATION_SEC_DEFAULT_PERMISSIONS_PRESENT";
pub const REASON_SEC_NO_DATA_LAKE_ADMINS: &str = "LAKEFORMATION_SEC_NO_DATA_LAKE_ADMINS";
pub const REASON_INV_STALE_DATA: &str = "LAKEFORMATION_INV_STALE_DATA";

pub fn evaluate_lakeformation_fleet(
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

fn data_i64(resource_data: &Value, key: &str) -> Option<i64> {
    resource_data.get(key).and_then(|v| v.as_i64())
}

fn is_registered_resource(resource: &AwsResourceModel) -> bool {
    data_str(&resource.resource_data, "resource_kind") == Some("registered_resource")
}

fn evaluate_cost(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    if data_i64(&resource.resource_data, "lf_tag_count").unwrap_or(0) == 0 {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Cost,
            reason_code: REASON_COST_NO_LF_TAGS.to_string(),
            severity: Severity::Medium,
            message: format!(
                "Lake Formation resource {} has no LF-Tags recorded for data ownership or cost allocation",
                resource.resource_id
            ),
            evidence: json!({ "lf_tag_count": resource.resource_data.get("lf_tag_count") }),
        });
    }
}

fn evaluate_resilience(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    if data_i64(&resource.resource_data, "data_lake_admin_count").unwrap_or(0) == 0 {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Resilience,
            reason_code: REASON_RES_NO_DATA_LAKE_ADMINS.to_string(),
            severity: Severity::High,
            message: format!(
                "Lake Formation resource {} has no data lake administrators recorded",
                resource.resource_id
            ),
            evidence: json!({
                "data_lake_admin_count": resource.resource_data.get("data_lake_admin_count"),
            }),
        });
    }

    if is_registered_resource(resource)
        && data_str(&resource.resource_data, "role_arn")
            .map(|s| s.trim().is_empty())
            .unwrap_or(true)
    {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Resilience,
            reason_code: REASON_RES_REGISTERED_RESOURCE_WITHOUT_ROLE.to_string(),
            severity: Severity::High,
            message: format!(
                "Lake Formation registered resource {} has no service role recorded",
                resource.resource_id
            ),
            evidence: json!({ "role_arn": resource.resource_data.get("role_arn") }),
        });
    }
}

fn evaluate_security(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    if data_i64(&resource.resource_data, "data_lake_admin_count").unwrap_or(0) == 0 {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Security,
            reason_code: REASON_SEC_NO_DATA_LAKE_ADMINS.to_string(),
            severity: Severity::High,
            message: format!(
                "Lake Formation resource {} has no explicit data lake administrators recorded",
                resource.resource_id
            ),
            evidence: json!({
                "data_lake_admin_count": resource.resource_data.get("data_lake_admin_count"),
            }),
        });
    }

    let database_default_permissions = data_i64(
        &resource.resource_data,
        "create_database_default_permissions_count",
    )
    .unwrap_or(0);
    let table_default_permissions = data_i64(
        &resource.resource_data,
        "create_table_default_permissions_count",
    )
    .unwrap_or(0);
    if database_default_permissions > 0 || table_default_permissions > 0 {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Security,
            reason_code: REASON_SEC_DEFAULT_PERMISSIONS_PRESENT.to_string(),
            severity: Severity::High,
            message: format!(
                "Lake Formation resource {} has default create permissions configured; verify broad grants are intentional",
                resource.resource_id
            ),
            evidence: json!({
                "create_database_default_permissions_count": database_default_permissions,
                "create_table_default_permissions_count": table_default_permissions,
            }),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;
    use uuid::Uuid;

    fn fixture(resource_data: Value, now: DateTime<Utc>) -> AwsResourceModel {
        AwsResourceModel {
            id: Uuid::new_v4(),
            sync_id: None,
            account_id: "123456789012".to_string(),
            profile: None,
            region: "us-east-1".to_string(),
            resource_type: RESOURCE_TYPE.to_string(),
            resource_id: "s3://analytics-lake".to_string(),
            arn: "arn:aws:s3:::analytics-lake".to_string(),
            name: Some("analytics-lake".to_string()),
            tags: json!({}),
            resource_data,
            created_at: now,
            updated_at: now,
            last_refreshed: now - Duration::hours(1),
        }
    }

    #[test]
    fn evaluates_lakeformation_inventory_findings() {
        let now = Utc::now();
        let resources = vec![fixture(
            json!({
                "resource_kind": "registered_resource",
                "lf_tag_count": 0,
                "data_lake_admin_count": 0,
                "role_arn": null,
                "create_database_default_permissions_count": 1,
                "create_table_default_permissions_count": 1,
            }),
            now,
        )];

        let cost = evaluate_lakeformation_fleet(&resources, Pillar::Cost, now);
        assert_eq!(cost.resources_evaluated, 1);
        assert!(cost
            .findings
            .iter()
            .any(|f| f.reason_code == REASON_COST_NO_LF_TAGS));

        let resilience = evaluate_lakeformation_fleet(&resources, Pillar::Resilience, now);
        assert!(resilience
            .findings
            .iter()
            .any(|f| f.reason_code == REASON_RES_NO_DATA_LAKE_ADMINS));
        assert!(resilience
            .findings
            .iter()
            .any(|f| f.reason_code == REASON_RES_REGISTERED_RESOURCE_WITHOUT_ROLE));

        let security = evaluate_lakeformation_fleet(&resources, Pillar::Security, now);
        assert!(security
            .findings
            .iter()
            .any(|f| f.reason_code == REASON_SEC_NO_DATA_LAKE_ADMINS));
        assert!(security
            .findings
            .iter()
            .any(|f| f.reason_code == REASON_SEC_DEFAULT_PERMISSIONS_PRESENT));
    }
}

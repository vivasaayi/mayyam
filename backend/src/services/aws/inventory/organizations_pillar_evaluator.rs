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

// Deterministic AWS Organizations inventory evaluators for the cost,
// resilience, and security pillars (roadmap rows
// 01-AWS-CLOUD-04663/04672/04699).

use chrono::{DateTime, Utc};
use serde_json::{json, Value};

use crate::models::aws_resource::Model as AwsResourceModel;
use crate::services::aws::inventory::types::{
    check_stale, has_any_tag, score_pillar, InventoryFinding, Pillar, PillarReport, Severity,
    COST_ALLOCATION_TAG_KEYS,
};

pub const RESOURCE_TYPE: &str = "OrganizationsOrganization";

pub const REASON_COST_NO_TAGS: &str = "ORG_COST_NO_TAGS";
pub const REASON_COST_ACCOUNT_TAG_GAPS: &str = "ORG_COST_ACCOUNT_TAG_GAPS";
pub const REASON_RES_CONSOLIDATED_BILLING_ONLY: &str = "ORG_RES_CONSOLIDATED_BILLING_ONLY";
pub const REASON_RES_NO_ACTIVE_ACCOUNTS: &str = "ORG_RES_NO_ACTIVE_ACCOUNTS";
pub const REASON_RES_SUSPENDED_ACCOUNTS: &str = "ORG_RES_SUSPENDED_ACCOUNTS";
pub const REASON_SEC_SCP_NOT_ENABLED: &str = "ORG_SEC_SCP_NOT_ENABLED";
pub const REASON_SEC_NO_SCP_POLICIES: &str = "ORG_SEC_NO_SCP_POLICIES";
pub const REASON_SEC_MANAGEMENT_ACCOUNT_GAP: &str = "ORG_SEC_MANAGEMENT_ACCOUNT_GAP";
pub const REASON_INV_STALE_DATA: &str = "ORG_INV_STALE_DATA";

pub fn evaluate_organizations_fleet(
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
                "Organizations inventory {} has no cost allocation tags",
                resource.resource_id
            ),
            json!({ "tags": resource.tags }),
        ));
    }

    let account_count = data_usize(&resource.resource_data, "account_count");
    let tagged_account_count = data_usize(
        &resource.resource_data,
        "cost_allocation_tagged_account_count",
    );
    if account_count > 0 && tagged_account_count < account_count {
        findings.push(finding(
            resource,
            Pillar::Cost,
            REASON_COST_ACCOUNT_TAG_GAPS,
            Severity::Medium,
            format!(
                "Organizations inventory {} has {} of {} accounts with cost allocation tags",
                resource.resource_id, tagged_account_count, account_count
            ),
            json!({
                "account_count": account_count,
                "cost_allocation_tagged_account_count": tagged_account_count,
                "untagged_account_count": account_count.saturating_sub(tagged_account_count),
            }),
        ));
    }
}

fn evaluate_resilience(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    if normalized_data_str(&resource.resource_data, "feature_set").as_deref()
        == Some("CONSOLIDATED_BILLING")
    {
        findings.push(finding(
            resource,
            Pillar::Resilience,
            REASON_RES_CONSOLIDATED_BILLING_ONLY,
            Severity::High,
            format!(
                "Organizations inventory {} only has consolidated billing features enabled",
                resource.resource_id
            ),
            json!({
                "feature_set": resource.resource_data.get("feature_set"),
                "organization_id": resource.resource_data.get("organization_id"),
            }),
        ));
    }

    if data_usize(&resource.resource_data, "active_account_count") == 0 {
        findings.push(finding(
            resource,
            Pillar::Resilience,
            REASON_RES_NO_ACTIVE_ACCOUNTS,
            Severity::High,
            format!(
                "Organizations inventory {} has no active member accounts in collected evidence",
                resource.resource_id
            ),
            json!({
                "active_account_count": resource.resource_data.get("active_account_count"),
                "account_count": resource.resource_data.get("account_count"),
            }),
        ));
    }

    let suspended_accounts = data_usize(&resource.resource_data, "suspended_account_count");
    if suspended_accounts > 0 {
        findings.push(finding(
            resource,
            Pillar::Resilience,
            REASON_RES_SUSPENDED_ACCOUNTS,
            Severity::Medium,
            format!(
                "Organizations inventory {} has {} suspended accounts",
                resource.resource_id, suspended_accounts
            ),
            json!({
                "suspended_account_count": suspended_accounts,
                "accounts": resource.resource_data.get("accounts"),
            }),
        ));
    }
}

fn evaluate_security(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    if data_bool(&resource.resource_data, "service_control_policy_enabled") != Some(true) {
        findings.push(finding(
            resource,
            Pillar::Security,
            REASON_SEC_SCP_NOT_ENABLED,
            Severity::High,
            format!(
                "Organizations inventory {} does not show service control policies enabled",
                resource.resource_id
            ),
            json!({
                "service_control_policy_enabled": resource.resource_data.get("service_control_policy_enabled"),
                "roots": resource.resource_data.get("roots"),
            }),
        ));
    }

    if data_usize(&resource.resource_data, "service_control_policy_count") == 0 {
        findings.push(finding(
            resource,
            Pillar::Security,
            REASON_SEC_NO_SCP_POLICIES,
            Severity::Medium,
            format!(
                "Organizations inventory {} has no service control policies in collected evidence",
                resource.resource_id
            ),
            json!({
                "service_control_policy_count": resource.resource_data.get("service_control_policy_count"),
                "service_control_policies": resource.resource_data.get("service_control_policies"),
            }),
        ));
    }

    if data_str(&resource.resource_data, "management_account_id")
        .map(|value| value.trim().is_empty())
        .unwrap_or(true)
    {
        findings.push(finding(
            resource,
            Pillar::Security,
            REASON_SEC_MANAGEMENT_ACCOUNT_GAP,
            Severity::Medium,
            format!(
                "Organizations inventory {} is missing management account identity evidence",
                resource.resource_id
            ),
            json!({
                "management_account_id": resource.resource_data.get("management_account_id"),
                "management_account_email": resource.resource_data.get("management_account_email"),
                "management_account_arn": resource.resource_data.get("management_account_arn"),
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
            resource_id: "organizations:o-exampleorgid".to_string(),
            arn: "arn:aws:organizations::123456789012:organization/o-exampleorgid".to_string(),
            name: Some("o-exampleorgid".to_string()),
            tags,
            resource_data,
            created_at: now,
            updated_at: now,
            last_refreshed: now - Duration::hours(72),
        }
    }

    #[test]
    fn evaluates_organizations_inventory_findings() {
        let resource = resource(
            json!({
                "organization_id": "o-exampleorgid",
                "feature_set": "CONSOLIDATED_BILLING",
                "management_account_id": null,
                "account_count": 4,
                "active_account_count": 0,
                "suspended_account_count": 2,
                "cost_allocation_tagged_account_count": 1,
                "service_control_policy_enabled": false,
                "service_control_policy_count": 0
            }),
            json!({}),
        );
        let now = Utc::now();

        let cost = evaluate_organizations_fleet(std::slice::from_ref(&resource), Pillar::Cost, now);
        assert_eq!(cost.resources_evaluated, 1);
        assert!(cost
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_COST_NO_TAGS));
        assert!(cost.findings.iter().any(|finding| {
            finding.reason_code == REASON_COST_ACCOUNT_TAG_GAPS
                && finding.severity == Severity::Medium
        }));

        let resilience =
            evaluate_organizations_fleet(std::slice::from_ref(&resource), Pillar::Resilience, now);
        assert!(resilience
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_RES_CONSOLIDATED_BILLING_ONLY));
        assert!(resilience
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_RES_NO_ACTIVE_ACCOUNTS));
        assert!(resilience
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_RES_SUSPENDED_ACCOUNTS));

        let security =
            evaluate_organizations_fleet(std::slice::from_ref(&resource), Pillar::Security, now);
        assert!(security
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_SEC_SCP_NOT_ENABLED));
        assert!(security
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_SEC_NO_SCP_POLICIES));
        assert!(security
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_SEC_MANAGEMENT_ACCOUNT_GAP));
        assert!(security
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_INV_STALE_DATA));
    }
}

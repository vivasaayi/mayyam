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

// Deterministic PrivateLink inventory evaluators for the cost, resilience,
// and security pillars (roadmap rows 01-AWS-CLOUD-03529/03538/03565).

use chrono::{DateTime, Utc};
use serde_json::{json, Value};

use crate::models::aws_resource::Model as AwsResourceModel;
use crate::services::aws::inventory::types::{
    check_stale, has_any_tag, score_pillar, InventoryFinding, Pillar, PillarReport, Severity,
    COST_ALLOCATION_TAG_KEYS,
};

pub const RESOURCE_TYPE: &str = "VpcEndpoint";

pub const REASON_COST_NO_TAGS: &str = "PRIVATELINK_COST_NO_TAGS";
pub const REASON_COST_NO_ASSOCIATIONS: &str = "PRIVATELINK_COST_NO_ASSOCIATIONS";
pub const REASON_RES_NOT_AVAILABLE: &str = "PRIVATELINK_RES_NOT_AVAILABLE";
pub const REASON_RES_SINGLE_AZ_INTERFACE: &str = "PRIVATELINK_RES_SINGLE_AZ_INTERFACE";
pub const REASON_SEC_NO_POLICY: &str = "PRIVATELINK_SEC_NO_POLICY";
pub const REASON_SEC_PRIVATE_DNS_DISABLED: &str = "PRIVATELINK_SEC_PRIVATE_DNS_DISABLED";
pub const REASON_INV_STALE_DATA: &str = "PRIVATELINK_INV_STALE_DATA";

pub fn evaluate_privatelink_fleet(
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

fn data_usize(resource_data: &Value, key: &str) -> Option<usize> {
    resource_data
        .get(key)
        .and_then(|v| v.as_u64())
        .map(|v| v as usize)
}

fn normalized_data_str(resource_data: &Value, key: &str) -> Option<String> {
    data_str(resource_data, key).map(|s| s.trim().to_ascii_lowercase())
}

fn is_interface_endpoint(resource_data: &Value) -> bool {
    normalized_data_str(resource_data, "endpoint_type").as_deref() == Some("interface")
}

fn is_available(resource_data: &Value) -> bool {
    normalized_data_str(resource_data, "state").as_deref() == Some("available")
}

fn is_deleted(resource_data: &Value) -> bool {
    normalized_data_str(resource_data, "state").as_deref() == Some("deleted")
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
                "PrivateLink endpoint {} has no cost allocation tags",
                resource.resource_id
            ),
            json!({ "tags": resource.tags }),
        ));
    }

    if is_available(&resource.resource_data)
        && data_usize(&resource.resource_data, "association_count") == Some(0)
    {
        findings.push(finding(
            resource,
            Pillar::Cost,
            REASON_COST_NO_ASSOCIATIONS,
            Severity::Medium,
            format!(
                "PrivateLink endpoint {} is available but has no subnet, route table, or network interface associations",
                resource.resource_id
            ),
            json!({
                "state": resource.resource_data.get("state"),
                "endpoint_type": resource.resource_data.get("endpoint_type"),
                "association_count": 0,
            }),
        ));
    }
}

fn evaluate_resilience(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    if let Some(state) = normalized_data_str(&resource.resource_data, "state") {
        if state != "available" && !is_deleted(&resource.resource_data) {
            findings.push(finding(
                resource,
                Pillar::Resilience,
                REASON_RES_NOT_AVAILABLE,
                Severity::High,
                format!(
                    "PrivateLink endpoint {} is in state {} rather than available",
                    resource.resource_id, state
                ),
                json!({ "state": resource.resource_data.get("state") }),
            ));
        }
    }

    if is_interface_endpoint(&resource.resource_data)
        && is_available(&resource.resource_data)
        && data_usize(&resource.resource_data, "subnet_count").unwrap_or(0) < 2
    {
        findings.push(finding(
            resource,
            Pillar::Resilience,
            REASON_RES_SINGLE_AZ_INTERFACE,
            Severity::Medium,
            format!(
                "PrivateLink interface endpoint {} is deployed in fewer than two subnets",
                resource.resource_id
            ),
            json!({
                "endpoint_type": resource.resource_data.get("endpoint_type"),
                "subnet_count": resource.resource_data.get("subnet_count"),
                "subnet_ids": resource.resource_data.get("subnet_ids"),
            }),
        ));
    }
}

fn evaluate_security(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    if data_bool(&resource.resource_data, "policy_document_present") == Some(false) {
        findings.push(finding(
            resource,
            Pillar::Security,
            REASON_SEC_NO_POLICY,
            Severity::Medium,
            format!(
                "PrivateLink endpoint {} has no endpoint policy captured",
                resource.resource_id
            ),
            json!({
                "endpoint_type": resource.resource_data.get("endpoint_type"),
                "service_name": resource.resource_data.get("service_name"),
                "policy_document_present": false,
            }),
        ));
    }

    if is_interface_endpoint(&resource.resource_data)
        && data_bool(&resource.resource_data, "private_dns_enabled") == Some(false)
    {
        findings.push(finding(
            resource,
            Pillar::Security,
            REASON_SEC_PRIVATE_DNS_DISABLED,
            Severity::Low,
            format!(
                "PrivateLink interface endpoint {} has private DNS disabled",
                resource.resource_id
            ),
            json!({
                "endpoint_type": resource.resource_data.get("endpoint_type"),
                "private_dns_enabled": false,
                "service_name": resource.resource_data.get("service_name"),
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
            arn: format!(
                "arn:aws:ec2:us-east-1:123456789012:vpc-endpoint/{}",
                resource_id
            ),
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
    fn evaluates_privatelink_inventory_findings() {
        let now = Utc::now();
        let resources = vec![
            fixture(
                "vpce-risky",
                json!({
                    "endpoint_type": "Interface",
                    "service_name": "com.amazonaws.us-east-1.s3",
                    "state": "Available",
                    "private_dns_enabled": false,
                    "policy_document_present": false,
                    "subnet_count": 1,
                    "subnet_ids": ["subnet-1"],
                    "route_table_count": 0,
                    "network_interface_count": 1,
                    "association_count": 1,
                }),
                json!({}),
                now,
                true,
            ),
            fixture(
                "vpce-healthy",
                json!({
                    "endpoint_type": "Interface",
                    "service_name": "com.amazonaws.us-east-1.ec2",
                    "state": "Available",
                    "private_dns_enabled": true,
                    "policy_document_present": true,
                    "subnet_count": 2,
                    "subnet_ids": ["subnet-1", "subnet-2"],
                    "route_table_count": 0,
                    "network_interface_count": 2,
                    "association_count": 2,
                }),
                json!({ "CostCenter": "network-platform" }),
                now,
                false,
            ),
            fixture(
                "vpce-failed-idle",
                json!({
                    "endpoint_type": "Gateway",
                    "service_name": "com.amazonaws.us-east-1.dynamodb",
                    "state": "Failed",
                    "private_dns_enabled": false,
                    "policy_document_present": true,
                    "subnet_count": 0,
                    "route_table_count": 0,
                    "network_interface_count": 0,
                    "association_count": 0,
                }),
                json!({ "team": "platform" }),
                now,
                false,
            ),
        ];

        let cost = evaluate_privatelink_fleet(&resources, Pillar::Cost, now);
        assert_eq!(cost.resources_evaluated, 3);
        assert_eq!(cost.stale_resources, 1);
        assert!(cost
            .findings
            .iter()
            .any(|f| f.reason_code == REASON_COST_NO_TAGS));

        let resilience = evaluate_privatelink_fleet(&resources, Pillar::Resilience, now);
        assert!(resilience
            .findings
            .iter()
            .any(|f| f.reason_code == REASON_RES_SINGLE_AZ_INTERFACE));
        assert!(resilience
            .findings
            .iter()
            .any(|f| f.reason_code == REASON_RES_NOT_AVAILABLE));

        let security = evaluate_privatelink_fleet(&resources, Pillar::Security, now);
        assert!(security
            .findings
            .iter()
            .any(|f| f.reason_code == REASON_SEC_NO_POLICY));
        assert!(security
            .findings
            .iter()
            .any(|f| f.reason_code == REASON_SEC_PRIVATE_DNS_DISABLED));
        assert!(security.score < 100);
    }

    #[test]
    fn cost_flags_available_endpoint_without_associations() {
        let now = Utc::now();
        let resource = fixture(
            "vpce-idle",
            json!({
                "endpoint_type": "Gateway",
                "state": "Available",
                "policy_document_present": true,
                "association_count": 0,
            }),
            json!({ "project": "networking" }),
            now,
            false,
        );

        let cost = evaluate_privatelink_fleet(&[resource], Pillar::Cost, now);
        assert!(cost
            .findings
            .iter()
            .any(|f| f.reason_code == REASON_COST_NO_ASSOCIATIONS));
    }
}

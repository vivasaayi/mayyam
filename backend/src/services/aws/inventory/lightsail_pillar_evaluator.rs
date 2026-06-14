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

// Deterministic Lightsail inventory evaluators for the cost, resilience, and
// security pillars (roadmap rows 01-AWS-CLOUD-00568/00577/00604).

use chrono::{DateTime, Utc};
use serde_json::{json, Value};

use crate::models::aws_resource::Model as AwsResourceModel;
use crate::services::aws::inventory::types::{
    check_stale, has_any_tag, score_pillar, InventoryFinding, Pillar, PillarReport, Severity,
    COST_ALLOCATION_TAG_KEYS,
};

pub const RESOURCE_TYPE: &str = "LightsailResource";

pub const REASON_COST_NO_TAGS: &str = "LIGHTSAIL_COST_NO_TAGS";
pub const REASON_COST_STOPPED_INSTANCE: &str = "LIGHTSAIL_COST_STOPPED_INSTANCE";
pub const REASON_COST_UNATTACHED_STATIC_IP: &str = "LIGHTSAIL_COST_UNATTACHED_STATIC_IP";
pub const REASON_RES_INSTANCE_NOT_RUNNING: &str = "LIGHTSAIL_RES_INSTANCE_NOT_RUNNING";
pub const REASON_RES_INSTANCE_WITHOUT_STATIC_IP: &str = "LIGHTSAIL_RES_INSTANCE_WITHOUT_STATIC_IP";
pub const REASON_SEC_PUBLIC_ADMIN_PORT: &str = "LIGHTSAIL_SEC_PUBLIC_ADMIN_PORT";
pub const REASON_INV_STALE_DATA: &str = "LIGHTSAIL_INV_STALE_DATA";

pub fn evaluate_lightsail_fleet(
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

fn data_array<'a>(resource_data: &'a Value, key: &str) -> Option<&'a Vec<Value>> {
    resource_data.get(key).and_then(|v| v.as_array())
}

fn resource_kind(resource: &AwsResourceModel) -> Option<&str> {
    data_str(&resource.resource_data, "resource_kind")
}

fn normalized_state(resource: &AwsResourceModel) -> Option<String> {
    data_str(&resource.resource_data, "state").map(|s| s.trim().to_ascii_lowercase())
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
                "Lightsail resource {} has no cost allocation tags",
                resource.resource_id
            ),
            json!({ "tags": resource.tags }),
        ));
    }

    if resource_kind(resource) == Some("instance")
        && normalized_state(resource).as_deref() == Some("stopped")
    {
        findings.push(finding(
            resource,
            Pillar::Cost,
            REASON_COST_STOPPED_INSTANCE,
            Severity::Medium,
            format!(
                "Lightsail instance {} is stopped; Lightsail instances continue to reserve bundled resources until deleted",
                resource.resource_id
            ),
            json!({ "state": data_str(&resource.resource_data, "state") }),
        ));
    }

    if resource_kind(resource) == Some("static_ip")
        && data_bool(&resource.resource_data, "is_attached") == Some(false)
    {
        findings.push(finding(
            resource,
            Pillar::Cost,
            REASON_COST_UNATTACHED_STATIC_IP,
            Severity::Medium,
            format!(
                "Lightsail static IP {} is not attached to an instance",
                resource.resource_id
            ),
            json!({
                "is_attached": false,
                "ip_address": resource.resource_data.get("ip_address"),
            }),
        ));
    }
}

fn evaluate_resilience(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    if resource_kind(resource) != Some("instance") {
        return;
    }

    match normalized_state(resource).as_deref() {
        Some("running") => {}
        Some(state) => findings.push(finding(
            resource,
            Pillar::Resilience,
            REASON_RES_INSTANCE_NOT_RUNNING,
            Severity::High,
            format!(
                "Lightsail instance {} is in {} state, not running",
                resource.resource_id, state
            ),
            json!({ "state": state }),
        )),
        None => findings.push(finding(
            resource,
            Pillar::Resilience,
            REASON_RES_INSTANCE_NOT_RUNNING,
            Severity::Medium,
            format!(
                "Lightsail instance {} state is not collected",
                resource.resource_id
            ),
            json!({ "state": null }),
        )),
    }

    if data_str(&resource.resource_data, "public_ip_address").is_some()
        && data_bool(&resource.resource_data, "uses_static_ip") == Some(false)
    {
        findings.push(finding(
            resource,
            Pillar::Resilience,
            REASON_RES_INSTANCE_WITHOUT_STATIC_IP,
            Severity::Medium,
            format!(
                "Lightsail instance {} uses an ephemeral public IP instead of a static IP",
                resource.resource_id
            ),
            json!({
                "public_ip_address": resource.resource_data.get("public_ip_address"),
                "uses_static_ip": false,
            }),
        ));
    }
}

fn evaluate_security(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    if resource_kind(resource) != Some("instance") {
        return;
    }

    let admin_ports: Vec<i64> = data_array(&resource.resource_data, "public_admin_ports")
        .map(|ports| ports.iter().filter_map(|p| p.as_i64()).collect())
        .unwrap_or_default();

    if !admin_ports.is_empty() {
        findings.push(finding(
            resource,
            Pillar::Security,
            REASON_SEC_PUBLIC_ADMIN_PORT,
            Severity::High,
            format!(
                "Lightsail instance {} exposes administrative port(s) {:?} publicly",
                resource.resource_id, admin_ports
            ),
            json!({ "public_admin_ports": admin_ports }),
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
            arn: format!("arn:aws:lightsail:us-east-1:123456789012:{}", resource_id),
            name: Some(resource_id.to_string()),
            tags,
            resource_data,
            created_at: now,
            updated_at: now,
            last_refreshed: now - Duration::hours(1),
        }
    }

    #[test]
    fn evaluates_lightsail_inventory_findings() {
        let now = Utc::now();
        let resources = vec![
            fixture(
                "instance/web-1",
                json!({
                    "resource_kind": "instance",
                    "state": "stopped",
                    "public_ip_address": "198.51.100.10",
                    "uses_static_ip": false,
                    "public_admin_ports": [22, 3389],
                }),
                json!({}),
                now,
            ),
            fixture(
                "static-ip/orphan",
                json!({
                    "resource_kind": "static_ip",
                    "ip_address": "198.51.100.20",
                    "is_attached": false,
                }),
                json!({ "CostCenter": "platform" }),
                now,
            ),
        ];

        let cost = evaluate_lightsail_fleet(&resources, Pillar::Cost, now);
        assert_eq!(cost.resources_evaluated, 2);
        assert!(cost
            .findings
            .iter()
            .any(|f| f.reason_code == REASON_COST_NO_TAGS));
        assert!(cost
            .findings
            .iter()
            .any(|f| f.reason_code == REASON_COST_STOPPED_INSTANCE));
        assert!(cost
            .findings
            .iter()
            .any(|f| f.reason_code == REASON_COST_UNATTACHED_STATIC_IP));

        let resilience = evaluate_lightsail_fleet(&resources, Pillar::Resilience, now);
        assert!(resilience
            .findings
            .iter()
            .any(|f| f.reason_code == REASON_RES_INSTANCE_NOT_RUNNING));
        assert!(resilience
            .findings
            .iter()
            .any(|f| f.reason_code == REASON_RES_INSTANCE_WITHOUT_STATIC_IP));

        let security = evaluate_lightsail_fleet(&resources, Pillar::Security, now);
        assert!(security
            .findings
            .iter()
            .any(|f| f.reason_code == REASON_SEC_PUBLIC_ADMIN_PORT));
    }
}

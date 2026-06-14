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

// Deterministic Amazon MQ inventory evaluators for the cost, resilience, and
// security pillars (roadmap rows 01-AWS-CLOUD-02647/02656/02683).

use chrono::{DateTime, Utc};
use serde_json::{json, Value};

use crate::models::aws_resource::Model as AwsResourceModel;
use crate::services::aws::inventory::types::{
    check_stale, has_any_tag, score_pillar, InventoryFinding, Pillar, PillarReport, Severity,
    COST_ALLOCATION_TAG_KEYS,
};

pub const RESOURCE_TYPE: &str = "AmazonMqBroker";

pub const REASON_COST_NO_TAGS: &str = "AMAZONMQ_COST_NO_TAGS";
pub const REASON_RES_BROKER_NOT_RUNNING: &str = "AMAZONMQ_RES_BROKER_NOT_RUNNING";
pub const REASON_RES_SINGLE_INSTANCE_DEPLOYMENT: &str = "AMAZONMQ_RES_SINGLE_INSTANCE_DEPLOYMENT";
pub const REASON_RES_AUTO_MINOR_UPGRADE_DISABLED: &str = "AMAZONMQ_RES_AUTO_MINOR_UPGRADE_DISABLED";
pub const REASON_SEC_PUBLICLY_ACCESSIBLE: &str = "AMAZONMQ_SEC_PUBLICLY_ACCESSIBLE";
pub const REASON_SEC_AWS_OWNED_KMS_KEY: &str = "AMAZONMQ_SEC_AWS_OWNED_KMS_KEY";
pub const REASON_INV_STALE_DATA: &str = "AMAZONMQ_INV_STALE_DATA";

pub fn evaluate_amazonmq_fleet(
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
                "Amazon MQ broker {} has no cost allocation tags",
                resource.resource_id
            ),
            json!({ "tags": resource.tags }),
        ));
    }
}

fn evaluate_resilience(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    match normalized_data_str(&resource.resource_data, "broker_state").as_deref() {
        Some("RUNNING") => {}
        Some(state) => findings.push(finding(
            resource,
            Pillar::Resilience,
            REASON_RES_BROKER_NOT_RUNNING,
            Severity::High,
            format!(
                "Amazon MQ broker {} is in state {} rather than RUNNING",
                resource.resource_id, state
            ),
            json!({ "broker_state": state }),
        )),
        None => {}
    }

    if normalized_data_str(&resource.resource_data, "deployment_mode")
        .map(|mode| mode.contains("SINGLE_INSTANCE"))
        .unwrap_or(false)
    {
        findings.push(finding(
            resource,
            Pillar::Resilience,
            REASON_RES_SINGLE_INSTANCE_DEPLOYMENT,
            Severity::Medium,
            format!(
                "Amazon MQ broker {} uses single-instance deployment",
                resource.resource_id
            ),
            json!({ "deployment_mode": resource.resource_data.get("deployment_mode") }),
        ));
    }

    if data_bool(&resource.resource_data, "auto_minor_version_upgrade") == Some(false) {
        findings.push(finding(
            resource,
            Pillar::Resilience,
            REASON_RES_AUTO_MINOR_UPGRADE_DISABLED,
            Severity::Medium,
            format!(
                "Amazon MQ broker {} has automatic minor version upgrades disabled",
                resource.resource_id
            ),
            json!({ "auto_minor_version_upgrade": false }),
        ));
    }
}

fn evaluate_security(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    if data_bool(&resource.resource_data, "publicly_accessible") == Some(true) {
        findings.push(finding(
            resource,
            Pillar::Security,
            REASON_SEC_PUBLICLY_ACCESSIBLE,
            Severity::High,
            format!(
                "Amazon MQ broker {} is publicly accessible",
                resource.resource_id
            ),
            json!({
                "publicly_accessible": true,
                "security_group_count": resource.resource_data.get("security_group_count"),
            }),
        ));
    }

    if data_bool(&resource.resource_data, "use_aws_owned_key") == Some(true) {
        findings.push(finding(
            resource,
            Pillar::Security,
            REASON_SEC_AWS_OWNED_KMS_KEY,
            Severity::Medium,
            format!(
                "Amazon MQ broker {} uses an AWS-owned encryption key instead of a customer managed key",
                resource.resource_id
            ),
            json!({
                "use_aws_owned_key": true,
                "kms_key_id": resource.resource_data.get("kms_key_id"),
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
            arn: format!("arn:aws:mq:us-east-1:123456789012:broker:{}", resource_id),
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
    fn evaluates_amazonmq_inventory_findings() {
        let now = Utc::now();
        let resources = vec![
            fixture(
                "broker/public-single",
                json!({
                    "broker_state": "REBOOT_IN_PROGRESS",
                    "deployment_mode": "SINGLE_INSTANCE",
                    "auto_minor_version_upgrade": false,
                    "publicly_accessible": true,
                    "security_group_count": 1,
                    "use_aws_owned_key": true,
                }),
                json!({}),
                now,
                true,
            ),
            fixture(
                "broker/private-active",
                json!({
                    "broker_state": "RUNNING",
                    "deployment_mode": "ACTIVE_STANDBY_MULTI_AZ",
                    "auto_minor_version_upgrade": true,
                    "publicly_accessible": false,
                    "use_aws_owned_key": false,
                    "kms_key_id": "arn:aws:kms:us-east-1:123456789012:key/customer",
                }),
                json!({ "CostCenter": "platform" }),
                now,
                false,
            ),
        ];

        let cost = evaluate_amazonmq_fleet(&resources, Pillar::Cost, now);
        assert_eq!(cost.resources_evaluated, 2);
        assert_eq!(cost.stale_resources, 1);
        assert!(cost
            .findings
            .iter()
            .any(|f| f.reason_code == REASON_COST_NO_TAGS));

        let resilience = evaluate_amazonmq_fleet(&resources, Pillar::Resilience, now);
        assert!(resilience
            .findings
            .iter()
            .any(|f| f.reason_code == REASON_RES_BROKER_NOT_RUNNING));
        assert!(resilience
            .findings
            .iter()
            .any(|f| f.reason_code == REASON_RES_SINGLE_INSTANCE_DEPLOYMENT));
        assert!(resilience
            .findings
            .iter()
            .any(|f| f.reason_code == REASON_RES_AUTO_MINOR_UPGRADE_DISABLED));

        let security = evaluate_amazonmq_fleet(&resources, Pillar::Security, now);
        assert!(security
            .findings
            .iter()
            .any(|f| f.reason_code == REASON_SEC_PUBLICLY_ACCESSIBLE));
        assert!(security
            .findings
            .iter()
            .any(|f| f.reason_code == REASON_SEC_AWS_OWNED_KMS_KEY));
        assert!(security.score < 100);
    }
}

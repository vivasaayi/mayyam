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

// Deterministic Elastic Beanstalk environment inventory evaluators for the
// cost, resilience, and security pillars (roadmap rows 01-AWS-CLOUD-00505/00514/00541).

use chrono::{DateTime, Utc};
use serde_json::{json, Value};

use crate::models::aws_resource::Model as AwsResourceModel;
use crate::services::aws::inventory::types::{
    check_stale, score_pillar, InventoryFinding, Pillar, PillarReport, Severity,
};

pub const RESOURCE_TYPE: &str = "ElasticBeanstalkEnvironment";

pub const REASON_COST_NO_TAGS: &str = "BEANSTALK_COST_NO_TAGS";
pub const REASON_COST_FIXED_CAPACITY: &str = "BEANSTALK_COST_FIXED_CAPACITY";
pub const REASON_RES_NOT_READY: &str = "BEANSTALK_RES_NOT_READY";
pub const REASON_RES_UNHEALTHY: &str = "BEANSTALK_RES_UNHEALTHY";
pub const REASON_RES_SINGLE_INSTANCE: &str = "BEANSTALK_RES_SINGLE_INSTANCE";
pub const REASON_RES_NO_ROLLING_UPDATES: &str = "BEANSTALK_RES_NO_ROLLING_UPDATES";
pub const REASON_SEC_NO_SERVICE_ROLE: &str = "BEANSTALK_SEC_NO_SERVICE_ROLE";
pub const REASON_SEC_NO_INSTANCE_PROFILE: &str = "BEANSTALK_SEC_NO_INSTANCE_PROFILE";
pub const REASON_SEC_NO_LOG_STREAMING: &str = "BEANSTALK_SEC_NO_LOG_STREAMING";
pub const REASON_SEC_BASIC_HEALTH: &str = "BEANSTALK_SEC_BASIC_HEALTH";
pub const REASON_INV_STALE_DATA: &str = "BEANSTALK_INV_STALE_DATA";

pub fn evaluate_elasticbeanstalk_fleet(
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

fn data_i64(resource_data: &Value, key: &str) -> Option<i64> {
    resource_data.get(key).and_then(|v| v.as_i64())
}

fn data_str<'a>(resource_data: &'a Value, key: &str) -> Option<&'a str> {
    resource_data.get(key).and_then(|v| v.as_str())
}

fn tags_empty(resource: &AwsResourceModel) -> bool {
    resource
        .tags
        .as_object()
        .map(|m| m.is_empty())
        .unwrap_or(true)
}

fn evaluate_cost(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    if tags_empty(resource) {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Cost,
            reason_code: REASON_COST_NO_TAGS.to_string(),
            severity: Severity::Medium,
            message: format!(
                "Elastic Beanstalk environment {} has no tags; cost allocation cannot be assessed",
                resource.resource_id
            ),
            evidence: json!({ "tags": resource.tags }),
        });
    }

    let min_size = data_i64(&resource.resource_data, "min_size");
    let max_size = data_i64(&resource.resource_data, "max_size");
    if let (Some(min_size), Some(max_size)) = (min_size, max_size) {
        if min_size >= 2 && min_size == max_size {
            findings.push(InventoryFinding {
                resource_id: resource.resource_id.clone(),
                arn: resource.arn.clone(),
                pillar: Pillar::Cost,
                reason_code: REASON_COST_FIXED_CAPACITY.to_string(),
                severity: Severity::Low,
                message: format!(
                    "Elastic Beanstalk environment {} has fixed capacity of {} instances; verify baseline demand before paying for always-on capacity",
                    resource.resource_id, min_size
                ),
                evidence: json!({ "min_size": min_size, "max_size": max_size }),
            });
        }
    }
}

fn evaluate_resilience(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    if let Some(status) = data_str(&resource.resource_data, "status") {
        if status != "Ready" {
            findings.push(InventoryFinding {
                resource_id: resource.resource_id.clone(),
                arn: resource.arn.clone(),
                pillar: Pillar::Resilience,
                reason_code: REASON_RES_NOT_READY.to_string(),
                severity: Severity::High,
                message: format!(
                    "Elastic Beanstalk environment {} is in status '{}', not Ready",
                    resource.resource_id, status
                ),
                evidence: json!({ "status": status }),
            });
        }
    }

    if let Some(health_status) = data_str(&resource.resource_data, "health_status") {
        if health_status != "Ok" && health_status != "Info" {
            findings.push(InventoryFinding {
                resource_id: resource.resource_id.clone(),
                arn: resource.arn.clone(),
                pillar: Pillar::Resilience,
                reason_code: REASON_RES_UNHEALTHY.to_string(),
                severity: Severity::High,
                message: format!(
                    "Elastic Beanstalk environment {} health status is '{}'",
                    resource.resource_id, health_status
                ),
                evidence: json!({ "health_status": health_status }),
            });
        }
    }

    if data_str(&resource.resource_data, "environment_type") == Some("SingleInstance")
        || data_i64(&resource.resource_data, "max_size").unwrap_or(2) < 2
    {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Resilience,
            reason_code: REASON_RES_SINGLE_INSTANCE.to_string(),
            severity: Severity::High,
            message: format!(
                "Elastic Beanstalk environment {} cannot tolerate an instance or AZ failure with its current capacity model",
                resource.resource_id
            ),
            evidence: json!({
                "environment_type": data_str(&resource.resource_data, "environment_type"),
                "max_size": data_i64(&resource.resource_data, "max_size"),
            }),
        });
    }

    if !data_bool(&resource.resource_data, "rolling_updates_enabled").unwrap_or(false) {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Resilience,
            reason_code: REASON_RES_NO_ROLLING_UPDATES.to_string(),
            severity: Severity::Medium,
            message: format!(
                "Elastic Beanstalk environment {} does not have rolling updates enabled",
                resource.resource_id
            ),
            evidence: json!({ "rolling_updates_enabled": false }),
        });
    }
}

fn evaluate_security(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    if resource.resource_data.get("service_role").is_none()
        && resource.resource_data.get("operations_role").is_none()
    {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Security,
            reason_code: REASON_SEC_NO_SERVICE_ROLE.to_string(),
            severity: Severity::Medium,
            message: format!(
                "Elastic Beanstalk environment {} has no service or operations role recorded",
                resource.resource_id
            ),
            evidence: json!({ "service_role": null, "operations_role": null }),
        });
    }

    if resource.resource_data.get("iam_instance_profile").is_none() {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Security,
            reason_code: REASON_SEC_NO_INSTANCE_PROFILE.to_string(),
            severity: Severity::Medium,
            message: format!(
                "Elastic Beanstalk environment {} has no instance profile recorded for workload permissions",
                resource.resource_id
            ),
            evidence: json!({ "iam_instance_profile": null }),
        });
    }

    if !data_bool(&resource.resource_data, "stream_logs_enabled").unwrap_or(false) {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Security,
            reason_code: REASON_SEC_NO_LOG_STREAMING.to_string(),
            severity: Severity::Medium,
            message: format!(
                "Elastic Beanstalk environment {} is not streaming logs to CloudWatch Logs",
                resource.resource_id
            ),
            evidence: json!({ "stream_logs_enabled": false }),
        });
    }

    if !data_bool(&resource.resource_data, "enhanced_health_reporting_enabled").unwrap_or(false) {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Security,
            reason_code: REASON_SEC_BASIC_HEALTH.to_string(),
            severity: Severity::Low,
            message: format!(
                "Elastic Beanstalk environment {} does not have enhanced health reporting enabled",
                resource.resource_id
            ),
            evidence: json!({ "enhanced_health_reporting_enabled": false }),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;
    use uuid::Uuid;

    fn fixture(resource_data: Value, tags: Value, now: DateTime<Utc>) -> AwsResourceModel {
        AwsResourceModel {
            id: Uuid::new_v4(),
            sync_id: None,
            account_id: "123456789012".to_string(),
            profile: None,
            region: "us-east-1".to_string(),
            resource_type: RESOURCE_TYPE.to_string(),
            resource_id: "e-beanstalk".to_string(),
            arn: "arn:aws:elasticbeanstalk:us-east-1:123456789012:environment/app/env".to_string(),
            name: Some("env".to_string()),
            tags,
            resource_data,
            created_at: now,
            updated_at: now,
            last_refreshed: now - Duration::hours(1),
        }
    }

    #[test]
    fn evaluates_elasticbeanstalk_inventory_findings() {
        let now = Utc::now();
        let resources = vec![fixture(
            json!({
                "status": "Updating",
                "health_status": "Warning",
                "environment_type": "SingleInstance",
                "min_size": 2,
                "max_size": 2,
                "rolling_updates_enabled": false,
                "stream_logs_enabled": false,
                "enhanced_health_reporting_enabled": false
            }),
            json!({}),
            now,
        )];

        let cost = evaluate_elasticbeanstalk_fleet(&resources, Pillar::Cost, now);
        assert!(cost
            .findings
            .iter()
            .any(|f| f.reason_code == REASON_COST_NO_TAGS));
        assert!(cost
            .findings
            .iter()
            .any(|f| f.reason_code == REASON_COST_FIXED_CAPACITY));

        let resilience = evaluate_elasticbeanstalk_fleet(&resources, Pillar::Resilience, now);
        assert!(resilience
            .findings
            .iter()
            .any(|f| f.reason_code == REASON_RES_NOT_READY));
        assert!(resilience
            .findings
            .iter()
            .any(|f| f.reason_code == REASON_RES_SINGLE_INSTANCE));

        let security = evaluate_elasticbeanstalk_fleet(&resources, Pillar::Security, now);
        assert!(security
            .findings
            .iter()
            .any(|f| f.reason_code == REASON_SEC_NO_LOG_STREAMING));
        assert!(security
            .findings
            .iter()
            .any(|f| f.reason_code == REASON_SEC_NO_SERVICE_ROLE));
    }
}

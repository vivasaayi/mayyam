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

// Deterministic Timestream table inventory evaluators for the cost,
// resilience, and security pillars (roadmap rows
// 01-AWS-CLOUD-01513/01522/01549).

use chrono::{DateTime, Utc};
use serde_json::{json, Value};

use crate::models::aws_resource::Model as AwsResourceModel;
use crate::services::aws::inventory::types::{
    check_stale, has_any_tag, score_pillar, InventoryFinding, Pillar, PillarReport, Severity,
    COST_ALLOCATION_TAG_KEYS,
};

pub const RESOURCE_TYPE: &str = "TimestreamTable";

pub const REASON_COST_NO_TAGS: &str = "TIMESTREAM_COST_NO_TAGS";
pub const REASON_COST_LONG_MEMORY_RETENTION: &str = "TIMESTREAM_COST_LONG_MEMORY_RETENTION";
pub const REASON_RES_TABLE_NOT_ACTIVE: &str = "TIMESTREAM_RES_TABLE_NOT_ACTIVE";
pub const REASON_RES_MAGNETIC_STORE_DISABLED: &str = "TIMESTREAM_RES_MAGNETIC_STORE_DISABLED";
pub const REASON_SEC_NO_CUSTOMER_KMS: &str = "TIMESTREAM_SEC_NO_CUSTOMER_KMS";
pub const REASON_INV_STALE_DATA: &str = "TIMESTREAM_INV_STALE_DATA";

pub fn evaluate_timestream_fleet(
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

fn data_i64(resource_data: &Value, key: &str) -> Option<i64> {
    resource_data.get(key).and_then(|v| v.as_i64())
}

fn evaluate_cost(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    if !has_any_tag(&resource.tags, COST_ALLOCATION_TAG_KEYS) {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Cost,
            reason_code: REASON_COST_NO_TAGS.to_string(),
            severity: Severity::Medium,
            message: format!(
                "Timestream table {} has no cost allocation tags",
                resource.resource_id
            ),
            evidence: json!({ "tags": resource.tags }),
        });
    }

    if let Some(hours) = data_i64(&resource.resource_data, "memory_retention_hours") {
        if hours > 24 {
            findings.push(InventoryFinding {
                resource_id: resource.resource_id.clone(),
                arn: resource.arn.clone(),
                pillar: Pillar::Cost,
                reason_code: REASON_COST_LONG_MEMORY_RETENTION.to_string(),
                severity: Severity::Medium,
                message: format!(
                    "Timestream table {} keeps {} hours in memory store; long memory retention can increase hot-tier cost",
                    resource.resource_id, hours
                ),
                evidence: json!({
                    "memory_retention_hours": hours,
                    "recommended_max_hours": 24,
                }),
            });
        }
    }
}

fn evaluate_resilience(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    match data_str(&resource.resource_data, "table_status") {
        Some("ACTIVE") => {}
        Some(status) => findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Resilience,
            reason_code: REASON_RES_TABLE_NOT_ACTIVE.to_string(),
            severity: Severity::High,
            message: format!(
                "Timestream table {} is in {} state, not ACTIVE",
                resource.resource_id, status
            ),
            evidence: json!({ "table_status": status }),
        }),
        None => findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Resilience,
            reason_code: REASON_RES_TABLE_NOT_ACTIVE.to_string(),
            severity: Severity::Medium,
            message: format!(
                "Timestream table {} status is not collected",
                resource.resource_id
            ),
            evidence: json!({ "table_status": null }),
        }),
    }

    if data_bool(&resource.resource_data, "magnetic_store_writes_enabled") == Some(false) {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Resilience,
            reason_code: REASON_RES_MAGNETIC_STORE_DISABLED.to_string(),
            severity: Severity::Medium,
            message: format!(
                "Timestream table {} has magnetic store writes disabled; late-arriving records can be rejected",
                resource.resource_id
            ),
            evidence: json!({ "magnetic_store_writes_enabled": false }),
        });
    }
}

fn evaluate_security(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    if data_str(&resource.resource_data, "kms_key_id")
        .map(|s| s.trim().is_empty())
        .unwrap_or(true)
    {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Security,
            reason_code: REASON_SEC_NO_CUSTOMER_KMS.to_string(),
            severity: Severity::Medium,
            message: format!(
                "Timestream table {} has no customer managed KMS key recorded for the database",
                resource.resource_id
            ),
            evidence: json!({ "kms_key_id": resource.resource_data.get("kms_key_id") }),
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
            resource_id: "metrics/events".to_string(),
            arn: "arn:aws:timestream:us-east-1:123456789012:database/metrics/table/events"
                .to_string(),
            name: Some("events".to_string()),
            tags,
            resource_data,
            created_at: now,
            updated_at: now,
            last_refreshed: now - Duration::hours(1),
        }
    }

    #[test]
    fn evaluates_timestream_inventory_findings() {
        let now = Utc::now();
        let resources = vec![fixture(
            json!({
                "table_status": "DELETING",
                "memory_retention_hours": 72,
                "magnetic_store_writes_enabled": false,
                "kms_key_id": null,
            }),
            json!({}),
            now,
        )];

        let cost = evaluate_timestream_fleet(&resources, Pillar::Cost, now);
        assert_eq!(cost.resources_evaluated, 1);
        assert!(cost
            .findings
            .iter()
            .any(|f| f.reason_code == REASON_COST_NO_TAGS));
        assert!(cost
            .findings
            .iter()
            .any(|f| f.reason_code == REASON_COST_LONG_MEMORY_RETENTION));

        let resilience = evaluate_timestream_fleet(&resources, Pillar::Resilience, now);
        assert!(resilience
            .findings
            .iter()
            .any(|f| f.reason_code == REASON_RES_TABLE_NOT_ACTIVE));
        assert!(resilience
            .findings
            .iter()
            .any(|f| f.reason_code == REASON_RES_MAGNETIC_STORE_DISABLED));

        let security = evaluate_timestream_fleet(&resources, Pillar::Security, now);
        assert!(security
            .findings
            .iter()
            .any(|f| f.reason_code == REASON_SEC_NO_CUSTOMER_KMS));
    }
}

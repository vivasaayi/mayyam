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

// Deterministic DynamoDB inventory evaluators for the cost, security, and
// resilience pillars (roadmap rows 01-AWS-CLOUD-01261/01270/01297).
//
// Evaluates fields persisted by dynamodb_control_plane: status,
// provisioned_throughput, item_count, table_size_bytes. Tags, encryption,
// and PITR/backup state are not collected yet and are reported as
// explicit data gaps.

use chrono::{DateTime, Utc};
use serde_json::json;

use crate::models::aws_resource::Model as AwsResourceModel;
use crate::services::aws::inventory::types::{
    check_stale, data_str, score_pillar, InventoryFinding, Pillar, PillarReport, Severity,
};

// Reason codes are the stable contract for findings; never reuse or rename.
pub const REASON_COST_TAG_DATA_NOT_COLLECTED: &str = "DDB_COST_TAG_DATA_NOT_COLLECTED";
pub const REASON_COST_EMPTY_PROVISIONED_TABLE: &str = "DDB_COST_EMPTY_PROVISIONED_TABLE";
pub const REASON_SEC_POSTURE_DATA_NOT_COLLECTED: &str = "DDB_SEC_POSTURE_DATA_NOT_COLLECTED";
pub const REASON_RES_BACKUP_DATA_NOT_COLLECTED: &str = "DDB_RES_BACKUP_DATA_NOT_COLLECTED";
pub const REASON_RES_TABLE_NOT_ACTIVE: &str = "DDB_RES_TABLE_NOT_ACTIVE";
pub const REASON_INV_STALE_DATA: &str = "DDB_INV_STALE_DATA";

/// Evaluate every DynamoDB table in the fleet for one pillar.
pub fn evaluate_dynamodb_fleet(
    resources: &[AwsResourceModel],
    pillar: Pillar,
    now: DateTime<Utc>,
) -> PillarReport {
    let mut findings: Vec<InventoryFinding> = Vec::new();
    let mut stale_resources = 0usize;

    for resource in resources {
        if let Some(stale) = check_stale(resource, pillar, REASON_INV_STALE_DATA, now) {
            stale_resources += 1;
            findings.push(stale);
        }
        match pillar {
            Pillar::Cost => evaluate_cost(resource, &mut findings),
            Pillar::Security => evaluate_security(resource, &mut findings),
            Pillar::Resilience => evaluate_resilience(resource, &mut findings),
            // Pillars without checks for this service yet produce no findings.
            _ => {}
        }
    }

    let score = score_pillar(&findings);
    PillarReport {
        pillar,
        resources_evaluated: resources.len(),
        stale_resources,
        score,
        findings,
    }
}

fn evaluate_cost(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    let tags_empty = resource
        .tags
        .as_object()
        .map(|m| m.is_empty())
        .unwrap_or(true);
    if tags_empty {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Cost,
            reason_code: REASON_COST_TAG_DATA_NOT_COLLECTED.to_string(),
            severity: Severity::Medium,
            message: format!(
                "Tags for table {} are not collected yet; cost allocation cannot be assessed",
                resource.resource_id
            ),
            evidence: json!({ "tags": resource.tags }),
        });
    }

    // provisioned_throughput is only persisted for provisioned-mode tables.
    let provisioned = resource.resource_data.get("provisioned_throughput");
    let item_count = resource
        .resource_data
        .get("item_count")
        .and_then(|v| v.as_i64());
    if let Some(throughput) = provisioned {
        if item_count == Some(0) {
            findings.push(InventoryFinding {
                resource_id: resource.resource_id.clone(),
                arn: resource.arn.clone(),
                pillar: Pillar::Cost,
                reason_code: REASON_COST_EMPTY_PROVISIONED_TABLE.to_string(),
                severity: Severity::Medium,
                message: format!(
                    "Table {} pays for provisioned capacity but holds zero items; switch to on-demand or delete it",
                    resource.resource_id
                ),
                evidence: json!({
                    "provisioned_throughput": throughput,
                    "item_count": 0,
                }),
            });
        }
    }
}

fn evaluate_security(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    let has_encryption_data = resource.resource_data.get("sse_description").is_some();
    if !has_encryption_data {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Security,
            reason_code: REASON_SEC_POSTURE_DATA_NOT_COLLECTED.to_string(),
            severity: Severity::Medium,
            message: format!(
                "Encryption configuration for table {} is not collected yet; security pillar cannot be fully assessed",
                resource.resource_id
            ),
            evidence: json!({ "sse_description_collected": false }),
        });
    }
}

fn evaluate_resilience(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    if let Some(status) = data_str(&resource.resource_data, "status") {
        if status != "ACTIVE" {
            findings.push(InventoryFinding {
                resource_id: resource.resource_id.clone(),
                arn: resource.arn.clone(),
                pillar: Pillar::Resilience,
                reason_code: REASON_RES_TABLE_NOT_ACTIVE.to_string(),
                severity: Severity::Medium,
                message: format!("Table {} is in status '{}'", resource.resource_id, status),
                evidence: json!({ "status": status }),
            });
        }
    }

    if resource
        .resource_data
        .get("point_in_time_recovery")
        .is_none()
    {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Resilience,
            reason_code: REASON_RES_BACKUP_DATA_NOT_COLLECTED.to_string(),
            severity: Severity::Medium,
            message: format!(
                "Point-in-time recovery state for table {} is not collected yet; recovery posture cannot be assessed",
                resource.resource_id
            ),
            evidence: json!({ "point_in_time_recovery_collected": false }),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;
    use serde_json::Value;
    use uuid::Uuid;

    fn fixture(
        resource_id: &str,
        tags: Value,
        resource_data: Value,
        now: DateTime<Utc>,
    ) -> AwsResourceModel {
        let refreshed = now - Duration::hours(1);
        AwsResourceModel {
            id: Uuid::new_v4(),
            sync_id: None,
            account_id: "123456789012".to_string(),
            profile: None,
            region: "us-east-1".to_string(),
            resource_type: "DynamoDbTable".to_string(),
            resource_id: resource_id.to_string(),
            arn: format!(
                "arn:aws:dynamodb:us-east-1:123456789012:table/{}",
                resource_id
            ),
            name: Some(resource_id.to_string()),
            tags,
            resource_data,
            created_at: refreshed,
            updated_at: refreshed,
            last_refreshed: refreshed,
        }
    }

    fn now() -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2026-06-10T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc)
    }

    fn healthy_data() -> Value {
        json!({
            "table_name": "orders",
            "status": "ACTIVE",
            "item_count": 1000,
            "table_size_bytes": 100000,
            "sse_description": {"status": "ENABLED"},
            "point_in_time_recovery": {"status": "ENABLED"},
        })
    }

    #[test]
    fn cost_flags_tag_gap_and_empty_provisioned_table() {
        let r = fixture(
            "idle-table",
            json!({}),
            json!({
                "table_name": "idle-table",
                "status": "ACTIVE",
                "item_count": 0,
                "provisioned_throughput": {"read_capacity_units": 100, "write_capacity_units": 100},
            }),
            now(),
        );
        let report = evaluate_dynamodb_fleet(&[r], Pillar::Cost, now());
        let codes: Vec<&str> = report
            .findings
            .iter()
            .map(|f| f.reason_code.as_str())
            .collect();
        assert!(codes.contains(&REASON_COST_TAG_DATA_NOT_COLLECTED));
        assert!(codes.contains(&REASON_COST_EMPTY_PROVISIONED_TABLE));
    }

    #[test]
    fn cost_passes_for_tagged_on_demand_table() {
        let r = fixture("orders", json!({"team": "commerce"}), healthy_data(), now());
        let report = evaluate_dynamodb_fleet(&[r], Pillar::Cost, now());
        assert!(
            report.findings.is_empty(),
            "unexpected: {:?}",
            report.findings
        );
    }

    #[test]
    fn security_reports_encryption_data_gap() {
        let mut data = healthy_data();
        data.as_object_mut().unwrap().remove("sse_description");
        let r = fixture("plain", json!({"team": "commerce"}), data, now());
        let report = evaluate_dynamodb_fleet(&[r], Pillar::Security, now());
        assert_eq!(
            report
                .findings
                .iter()
                .map(|f| f.reason_code.as_str())
                .collect::<Vec<_>>(),
            vec![REASON_SEC_POSTURE_DATA_NOT_COLLECTED]
        );
    }

    #[test]
    fn resilience_flags_non_active_table_and_missing_pitr_data() {
        let r = fixture(
            "deleting",
            json!({"team": "commerce"}),
            json!({"table_name": "deleting", "status": "DELETING"}),
            now(),
        );
        let report = evaluate_dynamodb_fleet(&[r], Pillar::Resilience, now());
        let codes: Vec<&str> = report
            .findings
            .iter()
            .map(|f| f.reason_code.as_str())
            .collect();
        assert!(codes.contains(&REASON_RES_TABLE_NOT_ACTIVE));
        assert!(codes.contains(&REASON_RES_BACKUP_DATA_NOT_COLLECTED));
    }

    #[test]
    fn resilience_passes_for_active_table_with_pitr_data() {
        let r = fixture("orders", json!({"team": "commerce"}), healthy_data(), now());
        let report = evaluate_dynamodb_fleet(&[r], Pillar::Resilience, now());
        assert!(
            report.findings.is_empty(),
            "unexpected: {:?}",
            report.findings
        );
    }
}

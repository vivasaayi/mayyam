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

// Deterministic SQS inventory evaluators for the cost, security, and
// resilience pillars (roadmap rows 01-AWS-CLOUD-02332/02341/02368).
//
// Evaluates fields persisted by sqs_control_plane: message_retention_period,
// visibility_timeout, fifo_queue, etc. Encryption and redrive (DLQ) policy
// are not collected yet and are reported as explicit data gaps.

use chrono::{DateTime, Utc};
use serde_json::json;

use crate::models::aws_resource::Model as AwsResourceModel;
use crate::services::aws::inventory::types::{
    check_stale, score_pillar, InventoryFinding, Pillar, PillarReport, Severity,
};

// Reason codes are the stable contract for findings; never reuse or rename.
pub const REASON_COST_TAG_DATA_NOT_COLLECTED: &str = "SQS_COST_TAG_DATA_NOT_COLLECTED";
pub const REASON_SEC_ENCRYPTION_DATA_NOT_COLLECTED: &str = "SQS_SEC_ENCRYPTION_DATA_NOT_COLLECTED";
pub const REASON_RES_DLQ_DATA_NOT_COLLECTED: &str = "SQS_RES_DLQ_DATA_NOT_COLLECTED";
pub const REASON_RES_SHORT_RETENTION: &str = "SQS_RES_SHORT_RETENTION";
pub const REASON_INV_STALE_DATA: &str = "SQS_INV_STALE_DATA";

/// Retention below one hour risks losing messages during consumer outages.
pub const MIN_SAFE_RETENTION_SECONDS: i64 = 3600;

/// Evaluate every SQS queue in the fleet for one pillar.
pub fn evaluate_sqs_fleet(
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

fn tags_missing(resource: &AwsResourceModel) -> bool {
    resource
        .tags
        .as_object()
        .map(|m| m.is_empty())
        .unwrap_or(true)
}

fn evaluate_cost(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    if tags_missing(resource) {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Cost,
            reason_code: REASON_COST_TAG_DATA_NOT_COLLECTED.to_string(),
            severity: Severity::Medium,
            message: format!(
                "Tags for queue {} are not collected yet; cost allocation cannot be assessed",
                resource.resource_id
            ),
            evidence: json!({ "tags": resource.tags }),
        });
    }
}

fn evaluate_security(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    if resource.resource_data.get("kms_master_key_id").is_none()
        && resource
            .resource_data
            .get("sqs_managed_sse_enabled")
            .is_none()
    {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Security,
            reason_code: REASON_SEC_ENCRYPTION_DATA_NOT_COLLECTED.to_string(),
            severity: Severity::Medium,
            message: format!(
                "Encryption configuration for queue {} is not collected yet; security pillar cannot be fully assessed",
                resource.resource_id
            ),
            evidence: json!({
                "kms_master_key_id_collected": false,
                "sqs_managed_sse_enabled_collected": false,
            }),
        });
    }
}

fn evaluate_resilience(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    if resource.resource_data.get("redrive_policy").is_none() {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Resilience,
            reason_code: REASON_RES_DLQ_DATA_NOT_COLLECTED.to_string(),
            severity: Severity::Medium,
            message: format!(
                "Dead-letter (redrive) policy for queue {} is not collected yet; poison-message handling cannot be assessed",
                resource.resource_id
            ),
            evidence: json!({ "redrive_policy_collected": false }),
        });
    }

    if let Some(retention) = resource
        .resource_data
        .get("message_retention_period")
        .and_then(|v| v.as_i64())
    {
        if retention < MIN_SAFE_RETENTION_SECONDS {
            findings.push(InventoryFinding {
                resource_id: resource.resource_id.clone(),
                arn: resource.arn.clone(),
                pillar: Pillar::Resilience,
                reason_code: REASON_RES_SHORT_RETENTION.to_string(),
                severity: Severity::Low,
                message: format!(
                    "Queue {} retains messages for only {} seconds; a consumer outage longer than that loses messages",
                    resource.resource_id, retention
                ),
                evidence: json!({
                    "message_retention_period": retention,
                    "min_safe_retention_seconds": MIN_SAFE_RETENTION_SECONDS,
                }),
            });
        }
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
            resource_type: "SqsQueue".to_string(),
            resource_id: resource_id.to_string(),
            arn: format!("arn:aws:sqs:us-east-1:123456789012:{}", resource_id),
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
            "queue_url": "https://sqs.us-east-1.amazonaws.com/123456789012/q",
            "message_retention_period": 345600,
            "visibility_timeout": 30,
            "fifo_queue": false,
            "kms_master_key_id": "alias/aws/sqs",
            "redrive_policy": {"maxReceiveCount": 5},
        })
    }

    #[test]
    fn cost_reports_tag_gap_for_untagged_queue() {
        let r = fixture("q-untagged", json!({}), healthy_data(), now());
        let report = evaluate_sqs_fleet(&[r], Pillar::Cost, now());
        assert_eq!(
            report
                .findings
                .iter()
                .map(|f| f.reason_code.as_str())
                .collect::<Vec<_>>(),
            vec![REASON_COST_TAG_DATA_NOT_COLLECTED]
        );
    }

    #[test]
    fn security_reports_encryption_gap_when_not_collected() {
        let r = fixture(
            "q-gap",
            json!({"team": "events"}),
            json!({"queue_url": "u", "message_retention_period": 345600, "redrive_policy": {}}),
            now(),
        );
        let report = evaluate_sqs_fleet(&[r], Pillar::Security, now());
        assert_eq!(
            report
                .findings
                .iter()
                .map(|f| f.reason_code.as_str())
                .collect::<Vec<_>>(),
            vec![REASON_SEC_ENCRYPTION_DATA_NOT_COLLECTED]
        );
    }

    #[test]
    fn resilience_flags_dlq_gap_and_short_retention() {
        let r = fixture(
            "q-fragile",
            json!({"team": "events"}),
            json!({"queue_url": "u", "message_retention_period": 600}),
            now(),
        );
        let report = evaluate_sqs_fleet(&[r], Pillar::Resilience, now());
        let codes: Vec<&str> = report
            .findings
            .iter()
            .map(|f| f.reason_code.as_str())
            .collect();
        assert!(codes.contains(&REASON_RES_DLQ_DATA_NOT_COLLECTED));
        assert!(codes.contains(&REASON_RES_SHORT_RETENTION));
    }

    #[test]
    fn resilience_passes_for_queue_with_dlq_and_default_retention() {
        let r = fixture("q-ok", json!({"team": "events"}), healthy_data(), now());
        let report = evaluate_sqs_fleet(&[r], Pillar::Resilience, now());
        assert!(
            report.findings.is_empty(),
            "unexpected: {:?}",
            report.findings
        );
    }
}

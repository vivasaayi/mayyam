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

// Deterministic SNS inventory evaluators for the cost, security, and
// resilience pillars (roadmap rows 01-AWS-CLOUD-02395/02404/02431).
//
// Evaluates fields persisted by sns_control_plane: subscriptions_confirmed,
// subscriptions_pending, display_name. KMS encryption is not collected yet
// and is reported as an explicit data gap.

use chrono::{DateTime, Utc};
use serde_json::json;

use crate::models::aws_resource::Model as AwsResourceModel;
use crate::services::aws::inventory::types::{
    check_stale, score_pillar, InventoryFinding, Pillar, PillarReport, Severity,
};

// Reason codes are the stable contract for findings; never reuse or rename.
pub const REASON_COST_TAG_DATA_NOT_COLLECTED: &str = "SNS_COST_TAG_DATA_NOT_COLLECTED";
pub const REASON_SEC_ENCRYPTION_DATA_NOT_COLLECTED: &str = "SNS_SEC_ENCRYPTION_DATA_NOT_COLLECTED";
pub const REASON_RES_NO_SUBSCRIPTIONS: &str = "SNS_RES_NO_SUBSCRIPTIONS";
pub const REASON_RES_PENDING_SUBSCRIPTIONS: &str = "SNS_RES_PENDING_SUBSCRIPTIONS";
pub const REASON_INV_STALE_DATA: &str = "SNS_INV_STALE_DATA";

/// Evaluate every SNS topic in the fleet for one pillar.
pub fn evaluate_sns_fleet(
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
                "Tags for topic {} are not collected yet; cost allocation cannot be assessed",
                resource.resource_id
            ),
            evidence: json!({ "tags": resource.tags }),
        });
    }
}

fn evaluate_security(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    if resource.resource_data.get("kms_master_key_id").is_none() {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Security,
            reason_code: REASON_SEC_ENCRYPTION_DATA_NOT_COLLECTED.to_string(),
            severity: Severity::Medium,
            message: format!(
                "Encryption configuration for topic {} is not collected yet; security pillar cannot be fully assessed",
                resource.resource_id
            ),
            evidence: json!({ "kms_master_key_id_collected": false }),
        });
    }
}

fn evaluate_resilience(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    let confirmed = resource
        .resource_data
        .get("subscriptions_confirmed")
        .and_then(|v| v.as_i64());
    if confirmed == Some(0) {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Resilience,
            reason_code: REASON_RES_NO_SUBSCRIPTIONS.to_string(),
            severity: Severity::Medium,
            message: format!(
                "Topic {} has no confirmed subscriptions; published messages are silently dropped",
                resource.resource_id
            ),
            evidence: json!({ "subscriptions_confirmed": 0 }),
        });
    }

    let pending = resource
        .resource_data
        .get("subscriptions_pending")
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    if pending > 0 {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Resilience,
            reason_code: REASON_RES_PENDING_SUBSCRIPTIONS.to_string(),
            severity: Severity::Low,
            message: format!(
                "Topic {} has {} unconfirmed subscription(s); intended consumers are not receiving messages",
                resource.resource_id, pending
            ),
            evidence: json!({ "subscriptions_pending": pending }),
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
            resource_type: "SnsTopic".to_string(),
            resource_id: resource_id.to_string(),
            arn: format!("arn:aws:sns:us-east-1:123456789012:{}", resource_id),
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
            "topic_arn": "arn:aws:sns:us-east-1:123456789012:alerts",
            "subscriptions_confirmed": 3,
            "subscriptions_pending": 0,
            "kms_master_key_id": "alias/aws/sns",
        })
    }

    #[test]
    fn resilience_flags_orphan_topic_and_pending_subscriptions() {
        let orphan = fixture(
            "orphan",
            json!({"team": "alerts"}),
            json!({"subscriptions_confirmed": 0, "subscriptions_pending": 2, "kms_master_key_id": "k"}),
            now(),
        );
        let report = evaluate_sns_fleet(&[orphan], Pillar::Resilience, now());
        let codes: Vec<&str> = report
            .findings
            .iter()
            .map(|f| f.reason_code.as_str())
            .collect();
        assert!(codes.contains(&REASON_RES_NO_SUBSCRIPTIONS));
        assert!(codes.contains(&REASON_RES_PENDING_SUBSCRIPTIONS));
    }

    #[test]
    fn resilience_passes_for_subscribed_topic() {
        let r = fixture("ok", json!({"team": "alerts"}), healthy_data(), now());
        let report = evaluate_sns_fleet(&[r], Pillar::Resilience, now());
        assert!(
            report.findings.is_empty(),
            "unexpected: {:?}",
            report.findings
        );
    }

    #[test]
    fn security_reports_encryption_gap_when_not_collected() {
        let r = fixture(
            "gap",
            json!({"team": "alerts"}),
            json!({"subscriptions_confirmed": 1}),
            now(),
        );
        let report = evaluate_sns_fleet(&[r], Pillar::Security, now());
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
    fn cost_reports_tag_gap_for_untagged_topic() {
        let r = fixture("untagged", json!({}), healthy_data(), now());
        let report = evaluate_sns_fleet(&[r], Pillar::Cost, now());
        assert_eq!(
            report
                .findings
                .iter()
                .map(|f| f.reason_code.as_str())
                .collect::<Vec<_>>(),
            vec![REASON_COST_TAG_DATA_NOT_COLLECTED]
        );
    }
}

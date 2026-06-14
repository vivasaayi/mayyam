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

// Deterministic Kinesis Data Streams inventory evaluators for the cost,
// security, and resilience pillars (roadmap rows 01-AWS-CLOUD-01891/01900/01927).
//
// The collector currently persists only stream_name and encryption_type;
// shard counts, retention, and status are reported as explicit data gaps.

use chrono::{DateTime, Utc};
use serde_json::json;

use crate::models::aws_resource::Model as AwsResourceModel;
use crate::services::aws::inventory::types::{
    check_stale, data_str, score_pillar, InventoryFinding, Pillar, PillarReport, Severity,
};

// Reason codes are the stable contract for findings; never reuse or rename.
pub const REASON_COST_TAG_DATA_NOT_COLLECTED: &str = "KINESIS_COST_TAG_DATA_NOT_COLLECTED";
pub const REASON_COST_SHARD_DATA_NOT_COLLECTED: &str = "KINESIS_COST_SHARD_DATA_NOT_COLLECTED";
pub const REASON_SEC_UNENCRYPTED: &str = "KINESIS_SEC_UNENCRYPTED";
pub const REASON_SEC_ENCRYPTION_DATA_NOT_COLLECTED: &str =
    "KINESIS_SEC_ENCRYPTION_DATA_NOT_COLLECTED";
pub const REASON_RES_STATUS_DATA_NOT_COLLECTED: &str = "KINESIS_RES_STATUS_DATA_NOT_COLLECTED";
pub const REASON_INV_STALE_DATA: &str = "KINESIS_INV_STALE_DATA";

/// Evaluate every Kinesis stream in the fleet for one pillar.
pub fn evaluate_kinesis_fleet(
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
                "Tags for stream {} are not collected yet; cost allocation cannot be assessed",
                resource.resource_id
            ),
            evidence: json!({ "tags": resource.tags }),
        });
    }

    if resource.resource_data.get("shard_count").is_none() {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Cost,
            reason_code: REASON_COST_SHARD_DATA_NOT_COLLECTED.to_string(),
            severity: Severity::Medium,
            message: format!(
                "Shard count for stream {} is not collected yet; over-provisioning cannot be assessed",
                resource.resource_id
            ),
            evidence: json!({ "shard_count_collected": false }),
        });
    }
}

fn evaluate_security(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    match data_str(&resource.resource_data, "encryption_type") {
        Some(encryption) if encryption == "NONE" => {
            findings.push(InventoryFinding {
                resource_id: resource.resource_id.clone(),
                arn: resource.arn.clone(),
                pillar: Pillar::Security,
                reason_code: REASON_SEC_UNENCRYPTED.to_string(),
                severity: Severity::High,
                message: format!(
                    "Stream {} has server-side encryption disabled",
                    resource.resource_id
                ),
                evidence: json!({ "encryption_type": encryption }),
            });
        }
        Some(_) => {}
        None => {
            findings.push(InventoryFinding {
                resource_id: resource.resource_id.clone(),
                arn: resource.arn.clone(),
                pillar: Pillar::Security,
                reason_code: REASON_SEC_ENCRYPTION_DATA_NOT_COLLECTED.to_string(),
                severity: Severity::Medium,
                message: format!(
                    "Encryption state for stream {} is not collected yet; security pillar cannot be fully assessed",
                    resource.resource_id
                ),
                evidence: json!({ "encryption_type_collected": false }),
            });
        }
    }
}

fn evaluate_resilience(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    if resource.resource_data.get("stream_status").is_none() {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Resilience,
            reason_code: REASON_RES_STATUS_DATA_NOT_COLLECTED.to_string(),
            severity: Severity::Medium,
            message: format!(
                "Status and retention for stream {} are not collected yet; resilience cannot be assessed",
                resource.resource_id
            ),
            evidence: json!({ "stream_status_collected": false }),
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
            resource_type: "KinesisStream".to_string(),
            resource_id: resource_id.to_string(),
            arn: format!(
                "arn:aws:kinesis:us-east-1:123456789012:stream/{}",
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

    #[test]
    fn security_flags_disabled_encryption_as_high() {
        let r = fixture(
            "events",
            json!({"team": "data"}),
            json!({"stream_name": "events", "encryption_type": "NONE"}),
            now(),
        );
        let report = evaluate_kinesis_fleet(&[r], Pillar::Security, now());
        let finding = report
            .findings
            .iter()
            .find(|f| f.reason_code == REASON_SEC_UNENCRYPTED)
            .expect("unencrypted finding");
        assert_eq!(finding.severity, Severity::High);
    }

    #[test]
    fn security_passes_for_kms_encrypted_stream() {
        let r = fixture(
            "events",
            json!({"team": "data"}),
            json!({"stream_name": "events", "encryption_type": "KMS"}),
            now(),
        );
        let report = evaluate_kinesis_fleet(&[r], Pillar::Security, now());
        assert!(
            report.findings.is_empty(),
            "unexpected: {:?}",
            report.findings
        );
    }

    #[test]
    fn security_reports_gap_when_encryption_not_collected() {
        let r = fixture(
            "events",
            json!({"team": "data"}),
            json!({"stream_name": "events"}),
            now(),
        );
        let report = evaluate_kinesis_fleet(&[r], Pillar::Security, now());
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
    fn cost_and_resilience_report_collector_gaps() {
        let r = fixture(
            "events",
            json!({}),
            json!({"stream_name": "events", "encryption_type": "KMS"}),
            now(),
        );
        let cost = evaluate_kinesis_fleet(std::slice::from_ref(&r), Pillar::Cost, now());
        let cost_codes: Vec<&str> = cost
            .findings
            .iter()
            .map(|f| f.reason_code.as_str())
            .collect();
        assert!(cost_codes.contains(&REASON_COST_TAG_DATA_NOT_COLLECTED));
        assert!(cost_codes.contains(&REASON_COST_SHARD_DATA_NOT_COLLECTED));
        let res = evaluate_kinesis_fleet(std::slice::from_ref(&r), Pillar::Resilience, now());
        assert_eq!(
            res.findings
                .iter()
                .map(|f| f.reason_code.as_str())
                .collect::<Vec<_>>(),
            vec![REASON_RES_STATUS_DATA_NOT_COLLECTED]
        );
    }
}

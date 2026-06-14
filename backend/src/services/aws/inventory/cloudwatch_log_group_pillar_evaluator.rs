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

// Deterministic CloudWatch log group inventory evaluators for the cost,
// resilience, and security pillars (roadmap rows 01-AWS-CLOUD-04411/04420/04447).
//
// Evaluates fields persisted by cloudwatch_control_plane::sync_log_groups:
// LogGroupName, Arn, RetentionInDays, KmsKeyId, StoredBytes,
// MetricFilterCount, LogGroupClass, plus the tags column.

use chrono::{DateTime, Utc};
use serde_json::{json, Value};

use crate::models::aws_resource::Model as AwsResourceModel;
use crate::services::aws::inventory::types::{
    check_stale, data_str, score_pillar, InventoryFinding, Pillar, PillarReport, Severity,
};

/// Only rows of this resource type are evaluated.
pub const RESOURCE_TYPE: &str = "CloudWatchLogGroup";

// Reason codes are the stable contract for findings; never reuse or rename.
pub const REASON_COST_NO_RETENTION_POLICY: &str = "CWLOG_COST_NO_RETENTION_POLICY";
pub const REASON_COST_LARGE_STANDARD_CLASS: &str = "CWLOG_COST_LARGE_STANDARD_CLASS";
pub const REASON_SEC_NO_KMS_ENCRYPTION: &str = "CWLOG_SEC_NO_KMS_ENCRYPTION";
pub const REASON_RES_STORAGE_DATA_NOT_COLLECTED: &str = "CWLOG_RES_STORAGE_DATA_NOT_COLLECTED";
pub const REASON_INV_STALE_DATA: &str = "CWLOG_INV_STALE_DATA";

/// Standard-class log groups storing at least this many bytes without the
/// cheaper Infrequent Access class are flagged (50 GiB).
pub const LARGE_STANDARD_CLASS_BYTES: i64 = 50 * 1024 * 1024 * 1024;

/// Evaluate every CloudWatch log group in the fleet for one pillar. Rows whose
/// `resource_type` is not `CloudWatchLogGroup` are skipped and not counted.
pub fn evaluate_cloudwatch_log_group_fleet(
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
            Pillar::Security => evaluate_security(resource, &mut findings),
            Pillar::Resilience => evaluate_resilience(resource, &mut findings),
            // Pillars without checks for this service yet produce no findings.
            _ => {}
        }
    }

    let score = score_pillar(&findings);
    PillarReport {
        pillar,
        resources_evaluated: evaluated,
        stale_resources,
        score,
        findings,
    }
}

fn data_i64(resource_data: &Value, key: &str) -> Option<i64> {
    resource_data.get(key).and_then(|v| v.as_i64())
}

fn evaluate_cost(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    let retention = data_i64(&resource.resource_data, "RetentionInDays");
    if retention.is_none() {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Cost,
            reason_code: REASON_COST_NO_RETENTION_POLICY.to_string(),
            severity: Severity::Medium,
            message: format!(
                "Log group {} has no retention policy; ingested events are stored forever and storage spend grows without bound",
                resource.resource_id
            ),
            evidence: json!({
                "retention_in_days": Value::Null,
                "stored_bytes": data_i64(&resource.resource_data, "StoredBytes"),
            }),
        });
    }

    let stored_bytes = data_i64(&resource.resource_data, "StoredBytes").unwrap_or(0);
    let log_group_class = data_str(&resource.resource_data, "LogGroupClass");
    let is_infrequent_access = log_group_class.as_deref() == Some("INFREQUENT_ACCESS");
    if stored_bytes >= LARGE_STANDARD_CLASS_BYTES && !is_infrequent_access {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Cost,
            reason_code: REASON_COST_LARGE_STANDARD_CLASS.to_string(),
            severity: Severity::Low,
            message: format!(
                "Log group {} stores {} bytes in the Standard class; evaluate the Infrequent Access class or export-and-expire for archival data",
                resource.resource_id, stored_bytes
            ),
            evidence: json!({
                "stored_bytes": stored_bytes,
                "log_group_class": log_group_class,
            }),
        });
    }
}

fn evaluate_security(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    // Log groups without a customer KMS key still get AWS-managed server-side
    // encryption, but key access cannot be audited or revoked per workload.
    let kms_key_id = data_str(&resource.resource_data, "KmsKeyId");
    if kms_key_id.as_deref().unwrap_or("").is_empty() {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Security,
            reason_code: REASON_SEC_NO_KMS_ENCRYPTION.to_string(),
            severity: Severity::Low,
            message: format!(
                "Log group {} is not encrypted with a customer-managed KMS key; key access to potentially sensitive log content cannot be audited or revoked per workload",
                resource.resource_id
            ),
            evidence: json!({ "kms_key_id": kms_key_id }),
        });
    }
}

fn evaluate_resilience(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    // StoredBytes is the signal both cost and triage rely on; if collection
    // did not return it, the inventory snapshot has a gap worth re-syncing.
    if data_i64(&resource.resource_data, "StoredBytes").is_none() {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Resilience,
            reason_code: REASON_RES_STORAGE_DATA_NOT_COLLECTED.to_string(),
            severity: Severity::Low,
            message: format!(
                "Stored bytes for log group {} were not collected; growth and triage signals are incomplete, so re-sync this account",
                resource.resource_id
            ),
            evidence: json!({ "stored_bytes": Value::Null }),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;
    use uuid::Uuid;

    fn fixture(resource_id: &str, resource_data: Value, now: DateTime<Utc>) -> AwsResourceModel {
        let refreshed = now - Duration::hours(1);
        AwsResourceModel {
            id: Uuid::new_v4(),
            sync_id: None,
            account_id: "123456789012".to_string(),
            profile: None,
            region: "us-east-1".to_string(),
            resource_type: RESOURCE_TYPE.to_string(),
            resource_id: resource_id.to_string(),
            arn: format!(
                "arn:aws:logs:us-east-1:123456789012:log-group:{}:*",
                resource_id
            ),
            name: Some(resource_id.to_string()),
            tags: json!({}),
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
            "LogGroupName": "/app/checkout",
            "Arn": "arn:aws:logs:us-east-1:123456789012:log-group:/app/checkout:*",
            "RetentionInDays": 30,
            "KmsKeyId": "arn:aws:kms:us-east-1:123456789012:key/abc",
            "StoredBytes": 1024,
            "MetricFilterCount": 1,
            "LogGroupClass": "STANDARD",
        })
    }

    fn codes(report: &PillarReport) -> Vec<&str> {
        report
            .findings
            .iter()
            .map(|f| f.reason_code.as_str())
            .collect()
    }

    #[test]
    fn cost_flags_missing_retention_policy() {
        let mut data = healthy_data();
        data.as_object_mut().unwrap().remove("RetentionInDays");
        let r = fixture("/app/no-retention", data, now());
        let report = evaluate_cloudwatch_log_group_fleet(&[r], Pillar::Cost, now());
        assert_eq!(codes(&report), vec![REASON_COST_NO_RETENTION_POLICY]);
        assert!(matches!(report.findings[0].severity, Severity::Medium));
    }

    #[test]
    fn cost_flags_null_retention_policy() {
        let mut data = healthy_data();
        data["RetentionInDays"] = Value::Null;
        let r = fixture("/app/null-retention", data, now());
        let report = evaluate_cloudwatch_log_group_fleet(&[r], Pillar::Cost, now());
        assert_eq!(codes(&report), vec![REASON_COST_NO_RETENTION_POLICY]);
    }

    #[test]
    fn cost_flags_large_standard_class_group() {
        let mut data = healthy_data();
        data["StoredBytes"] = json!(LARGE_STANDARD_CLASS_BYTES);
        let r = fixture("/app/large", data, now());
        let report = evaluate_cloudwatch_log_group_fleet(&[r], Pillar::Cost, now());
        assert_eq!(codes(&report), vec![REASON_COST_LARGE_STANDARD_CLASS]);
    }

    #[test]
    fn cost_does_not_flag_large_infrequent_access_group() {
        let mut data = healthy_data();
        data["StoredBytes"] = json!(LARGE_STANDARD_CLASS_BYTES);
        data["LogGroupClass"] = json!("INFREQUENT_ACCESS");
        let r = fixture("/app/large-ia", data, now());
        let report = evaluate_cloudwatch_log_group_fleet(&[r], Pillar::Cost, now());
        assert!(
            report.findings.is_empty(),
            "unexpected: {:?}",
            report.findings
        );
    }

    #[test]
    fn security_flags_missing_kms_key() {
        let mut data = healthy_data();
        data["KmsKeyId"] = Value::Null;
        let r = fixture("/app/no-kms", data, now());
        let report = evaluate_cloudwatch_log_group_fleet(&[r], Pillar::Security, now());
        assert_eq!(codes(&report), vec![REASON_SEC_NO_KMS_ENCRYPTION]);
    }

    #[test]
    fn security_passes_kms_encrypted_group() {
        let r = fixture("/app/checkout", healthy_data(), now());
        let report = evaluate_cloudwatch_log_group_fleet(&[r], Pillar::Security, now());
        assert!(
            report.findings.is_empty(),
            "unexpected: {:?}",
            report.findings
        );
    }

    #[test]
    fn resilience_flags_missing_stored_bytes() {
        let mut data = healthy_data();
        data.as_object_mut().unwrap().remove("StoredBytes");
        let r = fixture("/app/no-bytes", data, now());
        let report = evaluate_cloudwatch_log_group_fleet(&[r], Pillar::Resilience, now());
        assert_eq!(codes(&report), vec![REASON_RES_STORAGE_DATA_NOT_COLLECTED]);
    }

    #[test]
    fn stale_inventory_is_flagged() {
        let mut r = fixture("/app/stale", healthy_data(), now());
        r.last_refreshed = now() - Duration::hours(48);
        let report = evaluate_cloudwatch_log_group_fleet(&[r], Pillar::Resilience, now());
        assert_eq!(report.stale_resources, 1);
        assert!(codes(&report).contains(&REASON_INV_STALE_DATA));
    }

    #[test]
    fn non_log_group_resources_are_skipped_and_not_counted() {
        let mut r = fixture("queue-1", json!({}), now());
        r.resource_type = "SqsQueue".to_string();
        let report = evaluate_cloudwatch_log_group_fleet(&[r], Pillar::Cost, now());
        assert_eq!(report.resources_evaluated, 0);
        assert!(report.findings.is_empty());
    }

    #[test]
    fn healthy_log_group_passes_all_pillars() {
        let r = fixture("/app/checkout", healthy_data(), now());
        for pillar in [Pillar::Cost, Pillar::Security, Pillar::Resilience] {
            let report =
                evaluate_cloudwatch_log_group_fleet(std::slice::from_ref(&r), pillar, now());
            assert!(
                report.findings.is_empty(),
                "unexpected for {:?}: {:?}",
                pillar,
                report.findings
            );
            assert_eq!(report.score, 100);
        }
    }
}

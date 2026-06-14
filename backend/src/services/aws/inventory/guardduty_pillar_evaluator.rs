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

// Deterministic GuardDuty detector inventory evaluators for the cost, resilience,
// and security pillars (roadmap rows 01-AWS-CLOUD-04033/04042/04069).
//
// Evaluates fields persisted by guardduty_control_plane: status,
// finding_publishing_frequency, s3_logs_enabled, cloudtrail_enabled,
// dns_logs_enabled, flow_logs_enabled, kubernetes_audit_logs_enabled,
// features, plus tags.

use chrono::{DateTime, Utc};
use serde_json::{json, Value};

use crate::models::aws_resource::Model as AwsResourceModel;
use crate::services::aws::inventory::types::{
    check_stale, score_pillar, InventoryFinding, Pillar, PillarReport, Severity,
};

pub const RESOURCE_TYPE: &str = "GuardDutyDetector";

pub const REASON_COST_NO_TAGS: &str = "GD_COST_NO_TAGS";
pub const REASON_COST_INFREQUENT_PUBLISHING: &str = "GD_COST_INFREQUENT_PUBLISHING";
pub const REASON_RES_DETECTOR_DISABLED: &str = "GD_RES_DETECTOR_DISABLED";
pub const REASON_RES_S3_PROTECTION_DISABLED: &str = "GD_RES_S3_PROTECTION_DISABLED";
pub const REASON_SEC_CLOUDTRAIL_DISABLED: &str = "GD_SEC_CLOUDTRAIL_DISABLED";
pub const REASON_SEC_FLOW_LOGS_DISABLED: &str = "GD_SEC_FLOW_LOGS_DISABLED";
pub const REASON_SEC_DNS_LOGS_DISABLED: &str = "GD_SEC_DNS_LOGS_DISABLED";
pub const REASON_SEC_KUBERNETES_AUDIT_DISABLED: &str = "GD_SEC_KUBERNETES_AUDIT_DISABLED";
pub const REASON_INV_STALE_DATA: &str = "GD_INV_STALE_DATA";

pub fn evaluate_guardduty_fleet(
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

    let score = score_pillar(&findings);
    PillarReport {
        pillar,
        resources_evaluated: evaluated,
        stale_resources,
        score,
        findings,
    }
}

fn data_bool(resource_data: &Value, key: &str) -> Option<bool> {
    resource_data.get(key).and_then(|v| v.as_bool())
}

fn data_str<'a>(resource_data: &'a Value, key: &str) -> Option<&'a str> {
    resource_data.get(key).and_then(|v| v.as_str())
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
            reason_code: REASON_COST_NO_TAGS.to_string(),
            severity: Severity::Medium,
            message: format!(
                "GuardDuty detector {} has no tags; cost allocation cannot be assessed",
                resource.resource_id
            ),
            evidence: json!({ "tags": resource.tags }),
        });
    }

    // SIX_HOURS is the slowest publishing frequency, which means findings may
    // appear stale in downstream tooling.  FIFTEEN_MINUTES provides near-real-time
    // alerting at no additional cost, and is almost always the right choice.
    if let Some(freq) = data_str(&resource.resource_data, "finding_publishing_frequency") {
        if freq == "SIX_HOURS" {
            findings.push(InventoryFinding {
                resource_id: resource.resource_id.clone(),
                arn: resource.arn.clone(),
                pillar: Pillar::Cost,
                reason_code: REASON_COST_INFREQUENT_PUBLISHING.to_string(),
                severity: Severity::Low,
                message: format!(
                    "GuardDuty detector {} publishes findings every 6 hours; change to FIFTEEN_MINUTES for near-real-time alerting at no additional cost",
                    resource.resource_id
                ),
                evidence: json!({ "finding_publishing_frequency": freq }),
            });
        }
    }
}

fn evaluate_resilience(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    if let Some(status) = data_str(&resource.resource_data, "status") {
        if status != "ENABLED" {
            findings.push(InventoryFinding {
                resource_id: resource.resource_id.clone(),
                arn: resource.arn.clone(),
                pillar: Pillar::Resilience,
                reason_code: REASON_RES_DETECTOR_DISABLED.to_string(),
                severity: Severity::High,
                message: format!(
                    "GuardDuty detector {} is disabled; threat detection is inactive and the account has no GuardDuty coverage",
                    resource.resource_id
                ),
                evidence: json!({ "status": status }),
            });
        }
    }

    // S3 protection detects threats to S3 data (malicious API calls, data exfiltration).
    if !data_bool(&resource.resource_data, "s3_logs_enabled").unwrap_or(false) {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Resilience,
            reason_code: REASON_RES_S3_PROTECTION_DISABLED.to_string(),
            severity: Severity::Medium,
            message: format!(
                "GuardDuty detector {} does not have S3 Protection enabled; S3 API call monitoring is needed to detect data exfiltration and ransomware activity",
                resource.resource_id
            ),
            evidence: json!({ "s3_logs_enabled": false }),
        });
    }
}

fn evaluate_security(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    if !data_bool(&resource.resource_data, "cloudtrail_enabled").unwrap_or(false) {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Security,
            reason_code: REASON_SEC_CLOUDTRAIL_DISABLED.to_string(),
            severity: Severity::High,
            message: format!(
                "GuardDuty detector {} does not have CloudTrail data source enabled; CloudTrail analysis is the core data source for IAM, API-level threat detection",
                resource.resource_id
            ),
            evidence: json!({ "cloudtrail_enabled": false }),
        });
    }

    if !data_bool(&resource.resource_data, "flow_logs_enabled").unwrap_or(false) {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Security,
            reason_code: REASON_SEC_FLOW_LOGS_DISABLED.to_string(),
            severity: Severity::Medium,
            message: format!(
                "GuardDuty detector {} does not have VPC Flow Logs data source enabled; network-level threat detection requires flow log analysis",
                resource.resource_id
            ),
            evidence: json!({ "flow_logs_enabled": false }),
        });
    }

    if !data_bool(&resource.resource_data, "dns_logs_enabled").unwrap_or(false) {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Security,
            reason_code: REASON_SEC_DNS_LOGS_DISABLED.to_string(),
            severity: Severity::Medium,
            message: format!(
                "GuardDuty detector {} does not have DNS Logs data source enabled; DNS query analysis detects C2 communication and data exfiltration via DNS tunneling",
                resource.resource_id
            ),
            evidence: json!({ "dns_logs_enabled": false }),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;
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
            resource_type: RESOURCE_TYPE.to_string(),
            resource_id: resource_id.to_string(),
            arn: format!(
                "arn:aws:guardduty:us-east-1:123456789012:detector/{}",
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
        DateTime::parse_from_rfc3339("2026-06-11T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc)
    }

    fn healthy_data() -> Value {
        json!({
            "status": "ENABLED",
            "finding_publishing_frequency": "FIFTEEN_MINUTES",
            "s3_logs_enabled": true,
            "cloudtrail_enabled": true,
            "dns_logs_enabled": true,
            "flow_logs_enabled": true,
            "kubernetes_audit_logs_enabled": false,
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
    fn healthy_detector_passes_all_pillars() {
        let r = fixture(
            "abc123detector",
            json!({"team": "security"}),
            healthy_data(),
            now(),
        );
        for pillar in [Pillar::Cost, Pillar::Resilience, Pillar::Security] {
            let report = evaluate_guardduty_fleet(std::slice::from_ref(&r), pillar, now());
            assert!(
                report.findings.is_empty(),
                "unexpected for {:?}: {:?}",
                pillar,
                report.findings
            );
            assert_eq!(report.score, 100);
        }
    }

    #[test]
    fn cost_flags_untagged_detector() {
        let r = fixture("untagged", json!({}), healthy_data(), now());
        let report = evaluate_guardduty_fleet(&[r], Pillar::Cost, now());
        assert_eq!(codes(&report), vec![REASON_COST_NO_TAGS]);
    }

    #[test]
    fn cost_flags_six_hour_publishing() {
        let mut data = healthy_data();
        data["finding_publishing_frequency"] = json!("SIX_HOURS");
        let r = fixture("slow-gd", json!({"team": "security"}), data, now());
        let report = evaluate_guardduty_fleet(&[r], Pillar::Cost, now());
        assert!(codes(&report).contains(&REASON_COST_INFREQUENT_PUBLISHING));
    }

    #[test]
    fn resilience_flags_disabled_detector() {
        let mut data = healthy_data();
        data["status"] = json!("DISABLED");
        let r = fixture("disabled-gd", json!({"team": "security"}), data, now());
        let report = evaluate_guardduty_fleet(&[r], Pillar::Resilience, now());
        assert!(codes(&report).contains(&REASON_RES_DETECTOR_DISABLED));
    }

    #[test]
    fn resilience_flags_s3_protection_disabled() {
        let mut data = healthy_data();
        data["s3_logs_enabled"] = json!(false);
        let r = fixture("no-s3-gd", json!({"team": "security"}), data, now());
        let report = evaluate_guardduty_fleet(&[r], Pillar::Resilience, now());
        assert!(codes(&report).contains(&REASON_RES_S3_PROTECTION_DISABLED));
    }

    #[test]
    fn security_flags_cloudtrail_disabled() {
        let mut data = healthy_data();
        data["cloudtrail_enabled"] = json!(false);
        let r = fixture("no-ct-gd", json!({"team": "security"}), data, now());
        let report = evaluate_guardduty_fleet(&[r], Pillar::Security, now());
        assert!(codes(&report).contains(&REASON_SEC_CLOUDTRAIL_DISABLED));
    }

    #[test]
    fn security_flags_flow_logs_disabled() {
        let mut data = healthy_data();
        data["flow_logs_enabled"] = json!(false);
        let r = fixture("no-fl-gd", json!({"team": "security"}), data, now());
        let report = evaluate_guardduty_fleet(&[r], Pillar::Security, now());
        assert!(codes(&report).contains(&REASON_SEC_FLOW_LOGS_DISABLED));
    }

    #[test]
    fn security_flags_dns_logs_disabled() {
        let mut data = healthy_data();
        data["dns_logs_enabled"] = json!(false);
        let r = fixture("no-dns-gd", json!({"team": "security"}), data, now());
        let report = evaluate_guardduty_fleet(&[r], Pillar::Security, now());
        assert!(codes(&report).contains(&REASON_SEC_DNS_LOGS_DISABLED));
    }

    #[test]
    fn stale_resource_is_flagged() {
        let mut r = fixture(
            "stale-gd",
            json!({"team": "security"}),
            healthy_data(),
            now(),
        );
        r.last_refreshed = now() - Duration::hours(48);
        let report = evaluate_guardduty_fleet(&[r], Pillar::Resilience, now());
        assert_eq!(report.stale_resources, 1);
        assert!(codes(&report).contains(&REASON_INV_STALE_DATA));
    }

    #[test]
    fn non_guardduty_resources_are_skipped() {
        let mut r = fixture("queue-1", json!({}), json!({}), now());
        r.resource_type = "SqsQueue".to_string();
        let report = evaluate_guardduty_fleet(&[r], Pillar::Cost, now());
        assert_eq!(report.resources_evaluated, 0);
        assert!(report.findings.is_empty());
    }
}

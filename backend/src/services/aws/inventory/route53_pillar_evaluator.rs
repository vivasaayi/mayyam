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

// Deterministic Route 53 hosted-zone inventory evaluators for the cost,
// resilience, and security pillars (roadmap rows
// 01-AWS-CLOUD-03340/03349/03376).
//
// Evaluates fields persisted by route53_control_plane: name, private_zone,
// comment, resource_record_set_count, caller_reference,
// query_logging_enabled, plus the tags column.

use chrono::{DateTime, Utc};
use serde_json::{json, Value};

use crate::models::aws_resource::Model as AwsResourceModel;
use crate::services::aws::inventory::types::{
    check_stale, score_pillar, InventoryFinding, Pillar, PillarReport, Severity,
};

/// Only rows of this resource type are evaluated.
pub const RESOURCE_TYPE: &str = "Route53HostedZone";

// Reason codes are the stable contract for findings; never reuse or rename.
pub const REASON_COST_NO_TAGS: &str = "R53_COST_NO_TAGS";
pub const REASON_COST_EMPTY_ZONE: &str = "R53_COST_EMPTY_ZONE";
pub const REASON_RES_RECORD_QUOTA_NEAR: &str = "R53_RES_RECORD_QUOTA_NEAR";
pub const REASON_SEC_QUERY_LOGGING_DISABLED: &str = "R53_SEC_QUERY_LOGGING_DISABLED";
pub const REASON_INV_STALE_DATA: &str = "R53_INV_STALE_DATA";

/// A zone with only the default NS and SOA records serves nothing.
const EMPTY_ZONE_RECORD_THRESHOLD: i64 = 2;
/// Default Route 53 quota for records per hosted zone.
const RECORD_QUOTA: i64 = 10_000;
/// Flag when usage reaches 90% of the default record quota.
const RECORD_QUOTA_NEAR_THRESHOLD: i64 = 9_000;

/// Evaluate every Route 53 hosted zone in the fleet for one pillar. Rows whose
/// `resource_type` is not `Route53HostedZone` are skipped and not counted.
pub fn evaluate_route53_fleet(
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

fn data_bool(resource_data: &Value, key: &str) -> Option<bool> {
    resource_data.get(key).and_then(|v| v.as_bool())
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
                "Route 53 hosted zone {} has no tags recorded (untagged resource or tag collection gap); cost allocation cannot be assessed",
                resource.resource_id
            ),
            evidence: json!({ "tags": resource.tags }),
        });
    }

    // A zone holding only the default NS and SOA records answers no real DNS
    // queries but is still billed monthly.
    if let Some(count) = data_i64(&resource.resource_data, "resource_record_set_count") {
        if count <= EMPTY_ZONE_RECORD_THRESHOLD {
            findings.push(InventoryFinding {
                resource_id: resource.resource_id.clone(),
                arn: resource.arn.clone(),
                pillar: Pillar::Cost,
                reason_code: REASON_COST_EMPTY_ZONE.to_string(),
                severity: Severity::Low,
                message: format!(
                    "Route 53 hosted zone {} contains only {} record set(s) (the default NS and SOA records); it is billed monthly but serves nothing, so delete it if unused",
                    resource.resource_id, count
                ),
                evidence: json!({ "resource_record_set_count": count }),
            });
        }
    }
}

fn evaluate_security(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    let private_zone = data_bool(&resource.resource_data, "private_zone").unwrap_or(false);
    let query_logging_enabled =
        data_bool(&resource.resource_data, "query_logging_enabled").unwrap_or(false);

    // Query logging only supports public hosted zones, so private zones are
    // not flagged.
    if !private_zone && !query_logging_enabled {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Security,
            reason_code: REASON_SEC_QUERY_LOGGING_DISABLED.to_string(),
            severity: Severity::Low,
            message: format!(
                "Public Route 53 hosted zone {} has no query logging configuration; enable query logging to CloudWatch Logs for DNS query visibility",
                resource.resource_id
            ),
            evidence: json!({
                "private_zone": false,
                "query_logging_enabled": false,
            }),
        });
    }
}

fn evaluate_resilience(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    if let Some(count) = data_i64(&resource.resource_data, "resource_record_set_count") {
        if count >= RECORD_QUOTA_NEAR_THRESHOLD {
            findings.push(InventoryFinding {
                resource_id: resource.resource_id.clone(),
                arn: resource.arn.clone(),
                pillar: Pillar::Resilience,
                reason_code: REASON_RES_RECORD_QUOTA_NEAR.to_string(),
                severity: Severity::Medium,
                message: format!(
                    "Route 53 hosted zone {} holds {} record sets, near the default quota of {} records per zone; new record creation will fail at the quota, so request an increase or split the zone",
                    resource.resource_id, count, RECORD_QUOTA
                ),
                evidence: json!({
                    "resource_record_set_count": count,
                    "record_quota": RECORD_QUOTA,
                }),
            });
        }
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
            arn: format!("arn:aws:route53:::hostedzone/{}", resource_id),
            name: Some("example.com.".to_string()),
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
            "name": "example.com.",
            "private_zone": false,
            "comment": "primary public zone",
            "resource_record_set_count": 42,
            "caller_reference": "ref-1",
            "query_logging_enabled": true,
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
    fn healthy_zone_passes_all_pillars() {
        let r = fixture(
            "Z111EXAMPLE",
            json!({"team": "core"}),
            healthy_data(),
            now(),
        );
        for pillar in [Pillar::Cost, Pillar::Security, Pillar::Resilience] {
            let report = evaluate_route53_fleet(std::slice::from_ref(&r), pillar, now());
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
    fn cost_flags_untagged_zone() {
        let r = fixture("Z111UNTAGGED", json!({}), healthy_data(), now());
        let report = evaluate_route53_fleet(&[r], Pillar::Cost, now());
        assert_eq!(codes(&report), vec![REASON_COST_NO_TAGS]);
    }

    #[test]
    fn cost_flags_empty_zone() {
        let mut data = healthy_data();
        data["resource_record_set_count"] = json!(2);
        let r = fixture("Z111EMPTY", json!({"team": "core"}), data, now());
        let report = evaluate_route53_fleet(&[r], Pillar::Cost, now());
        assert_eq!(codes(&report), vec![REASON_COST_EMPTY_ZONE]);
    }

    #[test]
    fn resilience_flags_record_count_near_quota_as_medium() {
        let mut data = healthy_data();
        data["resource_record_set_count"] = json!(9500);
        let r = fixture("Z111BIG", json!({"team": "core"}), data, now());
        let report = evaluate_route53_fleet(&[r], Pillar::Resilience, now());
        assert_eq!(codes(&report), vec![REASON_RES_RECORD_QUOTA_NEAR]);
        assert!(matches!(report.findings[0].severity, Severity::Medium));
    }

    #[test]
    fn security_flags_public_zone_without_query_logging() {
        let mut data = healthy_data();
        data["query_logging_enabled"] = json!(false);
        let r = fixture("Z111NOLOG", json!({"team": "core"}), data, now());
        let report = evaluate_route53_fleet(&[r], Pillar::Security, now());
        assert_eq!(codes(&report), vec![REASON_SEC_QUERY_LOGGING_DISABLED]);
    }

    #[test]
    fn security_does_not_flag_private_zone_without_query_logging() {
        let mut data = healthy_data();
        data["private_zone"] = json!(true);
        data["query_logging_enabled"] = json!(false);
        let r = fixture("Z111PRIVATE", json!({"team": "core"}), data, now());
        let report = evaluate_route53_fleet(&[r], Pillar::Security, now());
        assert!(
            report.findings.is_empty(),
            "unexpected: {:?}",
            report.findings
        );
    }

    #[test]
    fn security_passes_public_zone_with_query_logging() {
        let r = fixture("Z111LOGGED", json!({"team": "core"}), healthy_data(), now());
        let report = evaluate_route53_fleet(&[r], Pillar::Security, now());
        assert!(
            report.findings.is_empty(),
            "unexpected: {:?}",
            report.findings
        );
    }

    #[test]
    fn stale_inventory_is_flagged() {
        let mut r = fixture("Z111STALE", json!({"team": "core"}), healthy_data(), now());
        r.last_refreshed = now() - Duration::hours(48);
        let report = evaluate_route53_fleet(&[r], Pillar::Resilience, now());
        assert_eq!(report.stale_resources, 1);
        assert!(codes(&report).contains(&REASON_INV_STALE_DATA));
    }

    #[test]
    fn non_route53_resources_are_skipped_and_not_counted() {
        let mut r = fixture("queue-1", json!({}), json!({}), now());
        r.resource_type = "SqsQueue".to_string();
        let report = evaluate_route53_fleet(&[r], Pillar::Cost, now());
        assert_eq!(report.resources_evaluated, 0);
        assert!(report.findings.is_empty());
    }
}

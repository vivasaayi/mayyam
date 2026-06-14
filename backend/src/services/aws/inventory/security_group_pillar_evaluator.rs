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

// Deterministic Security Group inventory evaluators for the cost, security,
// and resilience pillars (roadmap rows 01-AWS-CLOUD-02899/02908/02935).
//
// Evaluates fields persisted by vpc_control_plane::sync_security_groups:
// group_id, group_name, description, vpc_id. The collector persists real
// tags from the API into the `tags` map, so an empty tag map means an
// untagged group (or a tag collection gap on older rows).
//
// The collector does not persist ingress/egress rules yet, so the security
// pillar reports an honest data-gap finding instead of guessing at open
// 0.0.0.0/0 ingress. Security groups carry no per-resource resilience
// signal in the collected fields (they are regional, stateless constructs),
// so the resilience pillar is intentionally left clean rather than emitting
// noise.

use chrono::{DateTime, Utc};
use serde_json::json;

use crate::models::aws_resource::Model as AwsResourceModel;
use crate::services::aws::inventory::types::{
    check_stale, score_pillar, InventoryFinding, Pillar, PillarReport, Severity,
};

// Reason codes are the stable contract for findings; never reuse or rename.
pub const REASON_COST_NO_TAGS: &str = "SECURITYGROUP_COST_NO_TAGS";
pub const REASON_SEC_RULES_DATA_NOT_COLLECTED: &str = "SECURITYGROUP_SEC_RULES_DATA_NOT_COLLECTED";
pub const REASON_INV_STALE_DATA: &str = "SECURITYGROUP_INV_STALE_DATA";

/// Evaluate every security group in the fleet for one pillar.
pub fn evaluate_security_group_fleet(
    resources: &[AwsResourceModel],
    pillar: Pillar,
    now: DateTime<Utc>,
) -> PillarReport {
    let mut findings: Vec<InventoryFinding> = Vec::new();
    let mut stale_resources = 0usize;
    let mut evaluated = 0usize;

    for resource in resources {
        if resource.resource_type != "SecurityGroup" {
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
                "Security group {} has no tags recorded (untagged resource or tag collection gap); ownership and cleanup candidacy cannot be assessed",
                resource.resource_id
            ),
            evidence: json!({ "tags": resource.tags }),
        });
    }
}

fn evaluate_security(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    // The collector persists group_id/group_name/description/vpc_id only;
    // ingress and egress rules are not collected yet, so open-ingress
    // analysis (e.g. 0.0.0.0/0) is not possible from inventory evidence.
    findings.push(InventoryFinding {
        resource_id: resource.resource_id.clone(),
        arn: resource.arn.clone(),
        pillar: Pillar::Security,
        reason_code: REASON_SEC_RULES_DATA_NOT_COLLECTED.to_string(),
        severity: Severity::Medium,
        message: format!(
            "Ingress/egress rules for security group {} are not collected yet; open-ingress exposure cannot be assessed",
            resource.resource_id
        ),
        evidence: json!({
            "rules_collected": false,
            "collected_fields": ["group_id", "group_name", "description", "vpc_id"],
        }),
    });
}

fn evaluate_resilience(_resource: &AwsResourceModel, _findings: &mut Vec<InventoryFinding>) {
    // Security groups are regional, stateless constructs with no
    // per-resource resilience signal in the collected fields; the pillar
    // is intentionally clean (prefer no finding over noise).
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
            resource_type: "SecurityGroup".to_string(),
            resource_id: resource_id.to_string(),
            arn: format!(
                "arn:aws:ec2:us-east-1:123456789012:security-group/{}",
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
            "group_id": "sg-0123456789abcdef0",
            "group_name": "app-tier",
            "description": "App tier security group",
            "vpc_id": "vpc-0a1b2c3d4e5f6a7b8",
        })
    }

    #[test]
    fn cost_flags_untagged_security_group() {
        let r = fixture("sg-untagged", json!({}), healthy_data(), now());
        let report = evaluate_security_group_fleet(&[r], Pillar::Cost, now());
        assert_eq!(
            report
                .findings
                .iter()
                .map(|f| f.reason_code.as_str())
                .collect::<Vec<_>>(),
            vec![REASON_COST_NO_TAGS]
        );
        assert!(report.findings[0]
            .message
            .contains("untagged resource or tag collection gap"));
    }

    #[test]
    fn security_reports_gap_because_rules_are_not_collected() {
        let r = fixture("sg-norules", json!({"team": "net"}), healthy_data(), now());
        let report = evaluate_security_group_fleet(&[r], Pillar::Security, now());
        assert_eq!(
            report
                .findings
                .iter()
                .map(|f| f.reason_code.as_str())
                .collect::<Vec<_>>(),
            vec![REASON_SEC_RULES_DATA_NOT_COLLECTED]
        );
        assert_eq!(report.findings[0].severity as u8, Severity::Medium as u8);
    }

    #[test]
    fn resilience_is_intentionally_clean() {
        let r = fixture("sg-quiet", json!({"team": "net"}), healthy_data(), now());
        let report = evaluate_security_group_fleet(&[r], Pillar::Resilience, now());
        assert!(report.findings.is_empty());
        assert_eq!(report.score, 100);
    }

    #[test]
    fn stale_inventory_is_flagged() {
        let mut r = fixture("sg-old", json!({"team": "net"}), healthy_data(), now());
        r.last_refreshed = now() - Duration::hours(48);
        let report = evaluate_security_group_fleet(&[r], Pillar::Resilience, now());
        assert_eq!(report.stale_resources, 1);
        assert_eq!(
            report
                .findings
                .iter()
                .map(|f| f.reason_code.as_str())
                .collect::<Vec<_>>(),
            vec![REASON_INV_STALE_DATA]
        );
    }

    #[test]
    fn non_security_group_rows_are_skipped() {
        let mut other = fixture("vpc-123", json!({}), json!({}), now());
        other.resource_type = "Vpc".to_string();
        let sg = fixture("sg-only", json!({"team": "net"}), healthy_data(), now());
        let report = evaluate_security_group_fleet(&[other, sg], Pillar::Cost, now());
        assert_eq!(report.resources_evaluated, 1);
        assert!(report.findings.is_empty());
    }

    #[test]
    fn healthy_tagged_group_passes_cost_and_resilience() {
        let r = fixture(
            "sg-ok",
            json!({"team": "net", "owner": "sre"}),
            healthy_data(),
            now(),
        );
        for pillar in [Pillar::Cost, Pillar::Resilience] {
            let report = evaluate_security_group_fleet(std::slice::from_ref(&r), pillar, now());
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

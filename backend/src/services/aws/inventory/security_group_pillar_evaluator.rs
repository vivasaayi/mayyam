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
// group_id, group_name, description, vpc_id, ingress_rules, egress_rules.
// The collector persists real tags from the API into the `tags` map, so an
// empty tag map means an untagged group (or a tag collection gap on older
// rows).
//
// The security pillar flags unrestricted (0.0.0.0/0 or ::/0) ingress. When
// `ingress_rules` is absent (older rows synced before rule collection) it
// falls back to an honest data-gap finding instead of assuming the group is
// closed. Security groups carry no per-resource resilience signal in the
// collected fields (they are regional, stateless constructs), so the
// resilience pillar is intentionally left clean rather than emitting noise.

use chrono::{DateTime, Utc};
use serde_json::json;

use crate::models::aws_resource::Model as AwsResourceModel;
use crate::services::aws::inventory::types::{
    check_stale, score_pillar, InventoryFinding, Pillar, PillarReport, Severity,
};

// Reason codes are the stable contract for findings; never reuse or rename.
pub const REASON_COST_NO_TAGS: &str = "SECURITYGROUP_COST_NO_TAGS";
pub const REASON_SEC_RULES_DATA_NOT_COLLECTED: &str = "SECURITYGROUP_SEC_RULES_DATA_NOT_COLLECTED";
pub const REASON_SEC_UNRESTRICTED_INGRESS: &str = "SECURITYGROUP_SEC_UNRESTRICTED_INGRESS";
pub const REASON_INV_STALE_DATA: &str = "SECURITYGROUP_INV_STALE_DATA";

/// Admin/remote-access ports whose exposure to the whole internet is treated
/// as high severity rather than medium.
const ADMIN_PORTS: [i64; 2] = [22, 3389];

/// True if an ingress rule allows traffic from the entire internet
/// (0.0.0.0/0 or ::/0).
fn rule_is_world_open(rule: &serde_json::Value) -> bool {
    let ipv4_open = rule
        .get("ipv4_cidrs")
        .and_then(|v| v.as_array())
        .map(|cidrs| cidrs.iter().any(|c| c.as_str() == Some("0.0.0.0/0")))
        .unwrap_or(false);
    let ipv6_open = rule
        .get("ipv6_cidrs")
        .and_then(|v| v.as_array())
        .map(|cidrs| cidrs.iter().any(|c| c.as_str() == Some("::/0")))
        .unwrap_or(false);
    ipv4_open || ipv6_open
}

/// True if a rule exposes an admin port or every port (protocol "-1", or a
/// range covering 22/3389, or a non-"-1" protocol with no port bounds).
fn rule_exposes_admin_or_all_ports(rule: &serde_json::Value) -> bool {
    if rule.get("ip_protocol").and_then(|v| v.as_str()) == Some("-1") {
        return true;
    }
    let from = rule.get("from_port").and_then(|v| v.as_i64());
    let to = rule.get("to_port").and_then(|v| v.as_i64());
    match (from, to) {
        (Some(from), Some(to)) => ADMIN_PORTS.iter().any(|p| from <= *p && *p <= to),
        // A non-"-1" protocol with missing bounds is unusually broad; err high.
        _ => true,
    }
}

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
    let ingress_rules = resource
        .resource_data
        .get("ingress_rules")
        .and_then(|v| v.as_array());

    let Some(ingress_rules) = ingress_rules else {
        // Older rows synced before rule collection have no ingress_rules key;
        // report the gap instead of assuming the group is closed.
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Security,
            reason_code: REASON_SEC_RULES_DATA_NOT_COLLECTED.to_string(),
            severity: Severity::Medium,
            message: format!(
                "Ingress rules for security group {} are not collected yet; open-ingress exposure cannot be assessed",
                resource.resource_id
            ),
            evidence: json!({ "rules_collected": false }),
        });
        return;
    };

    let open_rules: Vec<&serde_json::Value> = ingress_rules
        .iter()
        .filter(|rule| rule_is_world_open(rule))
        .collect();
    if open_rules.is_empty() {
        return;
    }

    // Admin-port or all-port exposure to the internet is high severity;
    // any other world-open ingress (e.g. a single app port) is medium.
    let severity = if open_rules.iter().any(|r| rule_exposes_admin_or_all_ports(r)) {
        Severity::High
    } else {
        Severity::Medium
    };
    findings.push(InventoryFinding {
        resource_id: resource.resource_id.clone(),
        arn: resource.arn.clone(),
        pillar: Pillar::Security,
        reason_code: REASON_SEC_UNRESTRICTED_INGRESS.to_string(),
        severity,
        message: format!(
            "Security group {} allows unrestricted ingress (0.0.0.0/0 or ::/0) on {} rule(s); restrict the source range",
            resource.resource_id,
            open_rules.len()
        ),
        evidence: json!({ "open_ingress_rules": open_rules }),
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

    // A group synced before rule collection existed: no ingress_rules key.
    fn healthy_data() -> Value {
        json!({
            "group_id": "sg-0123456789abcdef0",
            "group_name": "app-tier",
            "description": "App tier security group",
            "vpc_id": "vpc-0a1b2c3d4e5f6a7b8",
        })
    }

    fn data_with_ingress(ingress_rules: Value) -> Value {
        let mut data = healthy_data();
        data["ingress_rules"] = ingress_rules;
        data["egress_rules"] = json!([]);
        data
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
    fn security_reports_gap_for_legacy_rows_without_rules() {
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
    fn security_flags_world_open_admin_port_as_high() {
        let data = data_with_ingress(json!([{
            "ip_protocol": "tcp", "from_port": 22, "to_port": 22,
            "ipv4_cidrs": ["0.0.0.0/0"], "ipv6_cidrs": [], "referenced_group_ids": []
        }]));
        let r = fixture("sg-ssh", json!({"team": "net"}), data, now());
        let report = evaluate_security_group_fleet(&[r], Pillar::Security, now());
        let f = report
            .findings
            .iter()
            .find(|f| f.reason_code == REASON_SEC_UNRESTRICTED_INGRESS)
            .expect("unrestricted ingress finding");
        assert_eq!(f.severity as u8, Severity::High as u8);
    }

    #[test]
    fn security_flags_world_open_app_port_as_medium() {
        let data = data_with_ingress(json!([{
            "ip_protocol": "tcp", "from_port": 443, "to_port": 443,
            "ipv4_cidrs": ["0.0.0.0/0"], "ipv6_cidrs": [], "referenced_group_ids": []
        }]));
        let r = fixture("sg-https", json!({"team": "net"}), data, now());
        let report = evaluate_security_group_fleet(&[r], Pillar::Security, now());
        let f = report
            .findings
            .iter()
            .find(|f| f.reason_code == REASON_SEC_UNRESTRICTED_INGRESS)
            .expect("unrestricted ingress finding");
        assert_eq!(f.severity as u8, Severity::Medium as u8);
    }

    #[test]
    fn security_flags_all_protocol_ipv6_open_as_high() {
        let data = data_with_ingress(json!([{
            "ip_protocol": "-1", "from_port": null, "to_port": null,
            "ipv4_cidrs": [], "ipv6_cidrs": ["::/0"], "referenced_group_ids": []
        }]));
        let r = fixture("sg-all", json!({"team": "net"}), data, now());
        let report = evaluate_security_group_fleet(&[r], Pillar::Security, now());
        let f = report
            .findings
            .iter()
            .find(|f| f.reason_code == REASON_SEC_UNRESTRICTED_INGRESS)
            .expect("unrestricted ingress finding");
        assert_eq!(f.severity as u8, Severity::High as u8);
    }

    #[test]
    fn security_passes_when_ingress_is_restricted_to_a_cidr() {
        let data = data_with_ingress(json!([{
            "ip_protocol": "tcp", "from_port": 22, "to_port": 22,
            "ipv4_cidrs": ["10.0.0.0/8"], "ipv6_cidrs": [], "referenced_group_ids": []
        }]));
        let r = fixture("sg-restricted", json!({"team": "net"}), data, now());
        let report = evaluate_security_group_fleet(&[r], Pillar::Security, now());
        assert!(report.findings.is_empty(), "unexpected: {:?}", report.findings);
        assert_eq!(report.score, 100);
    }

    #[test]
    fn security_passes_with_empty_ingress_rules() {
        let data = data_with_ingress(json!([]));
        let r = fixture("sg-empty", json!({"team": "net"}), data, now());
        let report = evaluate_security_group_fleet(&[r], Pillar::Security, now());
        assert!(report.findings.is_empty(), "unexpected: {:?}", report.findings);
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

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

// Deterministic load balancer inventory evaluators for the cost, security,
// and resilience pillars (roadmap rows 01-AWS-CLOUD-03214/03223/03250).
//
// Evaluates Alb, Nlb, and Elb (classic) rows persisted by
// load_balancer_control_plane: scheme, state, load_balancer_type,
// availability_zones (Alb only), security_groups (Alb only). The collector
// stores empty tags and does not collect listeners, so tag posture is
// reported as a gap and listener checks are deliberately absent.

use chrono::{DateTime, Utc};
use serde_json::json;

use crate::models::aws_resource::Model as AwsResourceModel;
use crate::services::aws::inventory::types::{
    check_stale, data_str, score_pillar, InventoryFinding, Pillar, PillarReport, Severity,
};

// Reason codes are the stable contract for findings; never reuse or rename.
pub const REASON_COST_NO_TAGS: &str = "ELB_COST_NO_TAGS";
pub const REASON_SEC_INTERNET_FACING: &str = "ELB_SEC_INTERNET_FACING";
pub const REASON_SEC_SCHEME_DATA_NOT_COLLECTED: &str = "ELB_SEC_SCHEME_DATA_NOT_COLLECTED";
pub const REASON_SEC_CLASSIC_GENERATION: &str = "ELB_SEC_CLASSIC_GENERATION";
pub const REASON_SEC_NO_SECURITY_GROUPS: &str = "ELB_SEC_NO_SECURITY_GROUPS";
pub const REASON_RES_SINGLE_AZ: &str = "ELB_RES_SINGLE_AZ";
pub const REASON_RES_STATE_NOT_ACTIVE: &str = "ELB_RES_STATE_NOT_ACTIVE";
pub const REASON_INV_STALE_DATA: &str = "ELB_INV_STALE_DATA";

fn is_classic(resource: &AwsResourceModel) -> bool {
    resource.resource_type == "Elb"
}

fn is_alb(resource: &AwsResourceModel) -> bool {
    resource.resource_type == "Alb"
}

/// Evaluate every ALB, NLB, and classic ELB in the fleet for one pillar.
pub fn evaluate_load_balancer_fleet(
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
            reason_code: REASON_COST_NO_TAGS.to_string(),
            severity: Severity::Medium,
            message: format!(
                "{} {} has no tags recorded (untagged resource or tag collection gap); cost allocation cannot be assessed",
                resource.resource_type, resource.resource_id
            ),
            evidence: json!({ "tags": resource.tags }),
        });
    }
}

fn evaluate_security(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    match data_str(&resource.resource_data, "scheme") {
        Some(scheme) => {
            if scheme == "internet-facing" {
                findings.push(InventoryFinding {
                    resource_id: resource.resource_id.clone(),
                    arn: resource.arn.clone(),
                    pillar: Pillar::Security,
                    reason_code: REASON_SEC_INTERNET_FACING.to_string(),
                    severity: Severity::Low,
                    message: format!(
                        "{} {} is internet-facing; confirm public exposure is intentional",
                        resource.resource_type, resource.resource_id
                    ),
                    evidence: json!({ "scheme": scheme }),
                });
            }
        }
        None => {
            findings.push(InventoryFinding {
                resource_id: resource.resource_id.clone(),
                arn: resource.arn.clone(),
                pillar: Pillar::Security,
                reason_code: REASON_SEC_SCHEME_DATA_NOT_COLLECTED.to_string(),
                severity: Severity::Low,
                message: format!(
                    "Scheme for {} {} is not collected yet; public exposure cannot be assessed",
                    resource.resource_type, resource.resource_id
                ),
                evidence: json!({ "scheme_collected": false }),
            });
        }
    }

    if is_classic(resource) {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Security,
            reason_code: REASON_SEC_CLASSIC_GENERATION.to_string(),
            severity: Severity::Medium,
            message: format!(
                "Load balancer {} is a classic (previous generation) ELB; it lacks modern TLS policies and WAF integration and should be migrated to ALB/NLB",
                resource.resource_id
            ),
            evidence: json!({ "load_balancer_type": resource.resource_data.get("load_balancer_type") }),
        });
    }

    if is_alb(resource) {
        if let Some(groups) = resource
            .resource_data
            .get("security_groups")
            .and_then(|v| v.as_array())
        {
            if groups.is_empty() {
                findings.push(InventoryFinding {
                    resource_id: resource.resource_id.clone(),
                    arn: resource.arn.clone(),
                    pillar: Pillar::Security,
                    reason_code: REASON_SEC_NO_SECURITY_GROUPS.to_string(),
                    severity: Severity::Medium,
                    message: format!(
                        "ALB {} has no security groups recorded; ingress is not restricted by any security group",
                        resource.resource_id
                    ),
                    evidence: json!({ "security_groups": [] }),
                });
            }
        }
    }
}

fn evaluate_resilience(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    // Availability zones are only collected for ALBs; skip silently when the
    // collector did not persist the field rather than inventing a value.
    if let Some(azs) = resource
        .resource_data
        .get("availability_zones")
        .and_then(|v| v.as_array())
    {
        if azs.len() == 1 {
            findings.push(InventoryFinding {
                resource_id: resource.resource_id.clone(),
                arn: resource.arn.clone(),
                pillar: Pillar::Resilience,
                reason_code: REASON_RES_SINGLE_AZ.to_string(),
                severity: Severity::Medium,
                message: format!(
                    "{} {} is attached to a single availability zone; an AZ outage takes it fully offline",
                    resource.resource_type, resource.resource_id
                ),
                evidence: json!({ "availability_zones": azs }),
            });
        }
    }

    // State is collected for ALB/NLB; classic ELBs have no state field.
    if let Some(state) = data_str(&resource.resource_data, "state") {
        if state != "active" {
            findings.push(InventoryFinding {
                resource_id: resource.resource_id.clone(),
                arn: resource.arn.clone(),
                pillar: Pillar::Resilience,
                reason_code: REASON_RES_STATE_NOT_ACTIVE.to_string(),
                severity: Severity::High,
                message: format!(
                    "{} {} is in state '{}'; it is not serving traffic normally",
                    resource.resource_type, resource.resource_id, state
                ),
                evidence: json!({ "state": state }),
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
        resource_type: &str,
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
            resource_type: resource_type.to_string(),
            resource_id: resource_id.to_string(),
            arn: format!(
                "arn:aws:elasticloadbalancing:us-east-1:123456789012:loadbalancer/{}",
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

    fn healthy_alb_data() -> Value {
        json!({
            "dns_name": "alb-ok.us-east-1.elb.amazonaws.com",
            "vpc_id": "vpc-1",
            "state": "active",
            "scheme": "internal",
            "load_balancer_type": "application",
            "availability_zones": ["us-east-1a", "us-east-1b"],
            "security_groups": ["sg-1"],
            "ip_address_type": "ipv4",
        })
    }

    fn healthy_nlb_data() -> Value {
        json!({
            "dns_name": "nlb-ok.us-east-1.elb.amazonaws.com",
            "vpc_id": "vpc-1",
            "state": "active",
            "scheme": "internal",
            "load_balancer_type": "network",
        })
    }

    fn healthy_elb_data() -> Value {
        json!({
            "dns_name": "elb-ok.us-east-1.elb.amazonaws.com",
            "vpc_id": "vpc-1",
            "scheme": "internal",
            "load_balancer_type": "classic",
        })
    }

    #[test]
    fn cost_flags_untagged_load_balancer() {
        let r = fixture("Nlb", "nlb-untagged", json!({}), healthy_nlb_data(), now());
        let report = evaluate_load_balancer_fleet(&[r], Pillar::Cost, now());
        assert_eq!(
            report
                .findings
                .iter()
                .map(|f| f.reason_code.as_str())
                .collect::<Vec<_>>(),
            vec![REASON_COST_NO_TAGS]
        );
    }

    #[test]
    fn security_flags_internet_facing_scheme() {
        let mut data = healthy_alb_data();
        data["scheme"] = json!("internet-facing");
        let r = fixture("Alb", "alb-public", json!({"team": "edge"}), data, now());
        let report = evaluate_load_balancer_fleet(&[r], Pillar::Security, now());
        assert_eq!(
            report
                .findings
                .iter()
                .map(|f| f.reason_code.as_str())
                .collect::<Vec<_>>(),
            vec![REASON_SEC_INTERNET_FACING]
        );
        assert_eq!(report.findings[0].severity, Severity::Low);
    }

    #[test]
    fn security_flags_classic_generation() {
        let r = fixture(
            "Elb",
            "legacy-elb",
            json!({"team": "edge"}),
            healthy_elb_data(),
            now(),
        );
        let report = evaluate_load_balancer_fleet(&[r], Pillar::Security, now());
        let codes: Vec<&str> = report
            .findings
            .iter()
            .map(|f| f.reason_code.as_str())
            .collect();
        assert_eq!(codes, vec![REASON_SEC_CLASSIC_GENERATION]);
    }

    #[test]
    fn security_flags_alb_without_security_groups() {
        let mut data = healthy_alb_data();
        data["security_groups"] = json!([]);
        let r = fixture("Alb", "alb-open", json!({"team": "edge"}), data, now());
        let report = evaluate_load_balancer_fleet(&[r], Pillar::Security, now());
        let codes: Vec<&str> = report
            .findings
            .iter()
            .map(|f| f.reason_code.as_str())
            .collect();
        assert!(codes.contains(&REASON_SEC_NO_SECURITY_GROUPS));
    }

    #[test]
    fn security_reports_gap_when_scheme_not_collected() {
        let r = fixture(
            "Nlb",
            "nlb-gap",
            json!({"team": "edge"}),
            json!({"dns_name": "x", "state": "active", "load_balancer_type": "network", "scheme": null}),
            now(),
        );
        let report = evaluate_load_balancer_fleet(&[r], Pillar::Security, now());
        assert_eq!(
            report
                .findings
                .iter()
                .map(|f| f.reason_code.as_str())
                .collect::<Vec<_>>(),
            vec![REASON_SEC_SCHEME_DATA_NOT_COLLECTED]
        );
    }

    #[test]
    fn resilience_flags_single_az_alb() {
        let mut data = healthy_alb_data();
        data["availability_zones"] = json!(["us-east-1a"]);
        let r = fixture("Alb", "alb-one-az", json!({"team": "edge"}), data, now());
        let report = evaluate_load_balancer_fleet(&[r], Pillar::Resilience, now());
        assert_eq!(
            report
                .findings
                .iter()
                .map(|f| f.reason_code.as_str())
                .collect::<Vec<_>>(),
            vec![REASON_RES_SINGLE_AZ]
        );
    }

    #[test]
    fn resilience_flags_non_active_state() {
        let mut data = healthy_nlb_data();
        data["state"] = json!("failed");
        let r = fixture("Nlb", "nlb-failed", json!({"team": "edge"}), data, now());
        let report = evaluate_load_balancer_fleet(&[r], Pillar::Resilience, now());
        assert_eq!(
            report
                .findings
                .iter()
                .map(|f| f.reason_code.as_str())
                .collect::<Vec<_>>(),
            vec![REASON_RES_STATE_NOT_ACTIVE]
        );
        assert_eq!(report.findings[0].severity, Severity::High);
    }

    #[test]
    fn stale_inventory_data_is_flagged() {
        let r = fixture(
            "Alb",
            "alb-stale",
            json!({"team": "edge"}),
            healthy_alb_data(),
            now(),
        );
        let later = now() + Duration::hours(48);
        let report = evaluate_load_balancer_fleet(&[r], Pillar::Resilience, later);
        assert_eq!(report.stale_resources, 1);
        assert!(report
            .findings
            .iter()
            .any(|f| f.reason_code == REASON_INV_STALE_DATA));
    }

    #[test]
    fn healthy_load_balancers_pass_all_pillars() {
        let alb = fixture(
            "Alb",
            "alb-ok",
            json!({"team": "edge"}),
            healthy_alb_data(),
            now(),
        );
        let nlb = fixture(
            "Nlb",
            "nlb-ok",
            json!({"team": "edge"}),
            healthy_nlb_data(),
            now(),
        );
        let elb = fixture(
            "Elb",
            "elb-ok",
            json!({"team": "edge"}),
            healthy_elb_data(),
            now(),
        );

        for pillar in [Pillar::Cost, Pillar::Security, Pillar::Resilience] {
            let report = evaluate_load_balancer_fleet(
                &[alb.clone(), nlb.clone(), elb.clone()],
                pillar,
                now(),
            );
            // A healthy classic ELB still deterministically reports its
            // legacy generation under the security pillar; everything else
            // must be clean.
            let expected: Vec<&str> = if pillar == Pillar::Security {
                vec![REASON_SEC_CLASSIC_GENERATION]
            } else {
                vec![]
            };
            assert_eq!(
                report
                    .findings
                    .iter()
                    .map(|f| f.reason_code.as_str())
                    .collect::<Vec<_>>(),
                expected,
                "unexpected findings for {:?}",
                pillar
            );
            assert_eq!(report.stale_resources, 0);
        }
    }
}

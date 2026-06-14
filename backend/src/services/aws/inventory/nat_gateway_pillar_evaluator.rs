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

// Deterministic NAT Gateway inventory evaluators for the cost, security,
// and resilience pillars (roadmap rows 01-AWS-CLOUD-02962/02971/02998).
//
// Evaluates fields persisted by vpc_control_plane::sync_nat_gateways:
// nat_gateway_id, state, subnet_id, vpc_id. No security posture fields are
// collected yet, so the security pillar reports an honest data gap.

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde_json::json;

use crate::models::aws_resource::Model as AwsResourceModel;
use crate::services::aws::inventory::types::{
    check_stale, data_str, score_pillar, InventoryFinding, Pillar, PillarReport, Severity,
};

const RESOURCE_TYPE: &str = "NatGateway";

// Reason codes are the stable contract for findings; never reuse or rename.
pub const REASON_COST_NO_TAGS: &str = "NATGATEWAY_COST_NO_TAGS";
pub const REASON_RES_STATE_NOT_AVAILABLE: &str = "NATGATEWAY_RES_STATE_NOT_AVAILABLE";
pub const REASON_RES_SINGLE_NAT_IN_VPC: &str = "NATGATEWAY_RES_SINGLE_NAT_IN_VPC";
pub const REASON_SEC_POSTURE_DATA_NOT_COLLECTED: &str = "NATGATEWAY_SEC_POSTURE_DATA_NOT_COLLECTED";
pub const REASON_INV_STALE_DATA: &str = "NATGATEWAY_INV_STALE_DATA";

/// Evaluate every NAT gateway in the fleet for one pillar. Rows whose
/// `resource_type` is not `NatGateway` are skipped and not counted.
pub fn evaluate_nat_gateway_fleet(
    resources: &[AwsResourceModel],
    pillar: Pillar,
    now: DateTime<Utc>,
) -> PillarReport {
    let mut findings: Vec<InventoryFinding> = Vec::new();
    let mut stale_resources = 0usize;
    let mut evaluated: Vec<&AwsResourceModel> = Vec::new();

    for resource in resources {
        if resource.resource_type != RESOURCE_TYPE {
            continue;
        }
        evaluated.push(resource);

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

    if pillar == Pillar::Resilience {
        flag_single_nat_vpcs(&evaluated, &mut findings);
    }

    let score = score_pillar(&findings);
    PillarReport {
        pillar,
        resources_evaluated: evaluated.len(),
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
                "NAT gateway {} has no tags recorded (untagged resource or tag collection gap); hourly NAT charges cannot be allocated",
                resource.resource_id
            ),
            evidence: json!({ "tags": resource.tags }),
        });
    }
}

fn evaluate_security(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    // The collector persists only nat_gateway_id, state, subnet_id and
    // vpc_id; no security posture fields exist yet, so report the gap
    // honestly instead of inventing checks.
    findings.push(InventoryFinding {
        resource_id: resource.resource_id.clone(),
        arn: resource.arn.clone(),
        pillar: Pillar::Security,
        reason_code: REASON_SEC_POSTURE_DATA_NOT_COLLECTED.to_string(),
        severity: Severity::Medium,
        message: format!(
            "Security posture fields for NAT gateway {} are not collected yet; security pillar cannot be assessed from inventory",
            resource.resource_id
        ),
        evidence: json!({ "security_fields_collected": false }),
    });
}

fn evaluate_resilience(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    if let Some(state) = data_str(&resource.resource_data, "state") {
        if state != "available" {
            findings.push(InventoryFinding {
                resource_id: resource.resource_id.clone(),
                arn: resource.arn.clone(),
                pillar: Pillar::Resilience,
                reason_code: REASON_RES_STATE_NOT_AVAILABLE.to_string(),
                severity: Severity::High,
                message: format!(
                    "NAT gateway {} is in state '{}' instead of 'available'; outbound traffic through it is degraded or down while hourly billing may continue",
                    resource.resource_id, state
                ),
                evidence: json!({ "state": state }),
            });
        }
    }
}

/// Post-loop fleet pass: a VPC served by exactly one NAT gateway has a
/// single-AZ egress dependency. Flag that lone NAT gateway row.
fn flag_single_nat_vpcs(evaluated: &[&AwsResourceModel], findings: &mut Vec<InventoryFinding>) {
    let mut per_vpc: HashMap<String, usize> = HashMap::new();
    for resource in evaluated {
        if let Some(vpc_id) = data_str(&resource.resource_data, "vpc_id") {
            *per_vpc.entry(vpc_id).or_insert(0) += 1;
        }
    }

    for resource in evaluated {
        let vpc_id = match data_str(&resource.resource_data, "vpc_id") {
            Some(v) => v,
            None => continue,
        };
        if per_vpc.get(&vpc_id).copied() != Some(1) {
            continue;
        }
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Resilience,
            reason_code: REASON_RES_SINGLE_NAT_IN_VPC.to_string(),
            severity: Severity::Medium,
            message: format!(
                "NAT gateway {} is the only NAT gateway in VPC {}; a single NAT gateway lives in one AZ, so an AZ failure removes all egress for the VPC",
                resource.resource_id, vpc_id
            ),
            evidence: json!({
                "vpc_id": vpc_id,
                "nat_gateways_in_vpc": 1,
            }),
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
            resource_type: "NatGateway".to_string(),
            resource_id: resource_id.to_string(),
            arn: format!(
                "arn:aws:ec2:us-east-1:123456789012:natgateway/{}",
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

    fn healthy_data(nat_id: &str, vpc_id: &str, subnet_id: &str) -> Value {
        json!({
            "nat_gateway_id": nat_id,
            "state": "available",
            "vpc_id": vpc_id,
            "subnet_id": subnet_id,
        })
    }

    #[test]
    fn cost_flags_missing_tags() {
        let r = fixture(
            "nat-untagged",
            json!({}),
            healthy_data("nat-untagged", "vpc-1", "subnet-a"),
            now(),
        );
        let report = evaluate_nat_gateway_fleet(&[r], Pillar::Cost, now());
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
    fn resilience_flags_non_available_state() {
        let mut data = healthy_data("nat-failed", "vpc-1", "subnet-a");
        data["state"] = json!("failed");
        let other = fixture(
            "nat-ok",
            json!({"team": "net"}),
            healthy_data("nat-ok", "vpc-1", "subnet-b"),
            now(),
        );
        let r = fixture("nat-failed", json!({"team": "net"}), data, now());
        let report = evaluate_nat_gateway_fleet(&[r, other], Pillar::Resilience, now());
        assert_eq!(
            report
                .findings
                .iter()
                .map(|f| f.reason_code.as_str())
                .collect::<Vec<_>>(),
            vec![REASON_RES_STATE_NOT_AVAILABLE]
        );
        assert_eq!(report.findings[0].resource_id, "nat-failed");
    }

    #[test]
    fn resilience_flags_single_nat_in_vpc() {
        let lone = fixture(
            "nat-lone",
            json!({"team": "net"}),
            healthy_data("nat-lone", "vpc-solo", "subnet-a"),
            now(),
        );
        let report = evaluate_nat_gateway_fleet(&[lone], Pillar::Resilience, now());
        assert_eq!(
            report
                .findings
                .iter()
                .map(|f| f.reason_code.as_str())
                .collect::<Vec<_>>(),
            vec![REASON_RES_SINGLE_NAT_IN_VPC]
        );
        assert_eq!(report.findings[0].resource_id, "nat-lone");
    }

    #[test]
    fn resilience_does_not_flag_vpc_with_two_nat_gateways() {
        let a = fixture(
            "nat-a",
            json!({"team": "net"}),
            healthy_data("nat-a", "vpc-multi", "subnet-a"),
            now(),
        );
        let b = fixture(
            "nat-b",
            json!({"team": "net"}),
            healthy_data("nat-b", "vpc-multi", "subnet-b"),
            now(),
        );
        let report = evaluate_nat_gateway_fleet(&[a, b], Pillar::Resilience, now());
        assert!(
            report.findings.is_empty(),
            "unexpected: {:?}",
            report.findings
        );
    }

    #[test]
    fn security_reports_posture_data_gap() {
        let r = fixture(
            "nat-sec",
            json!({"team": "net"}),
            healthy_data("nat-sec", "vpc-1", "subnet-a"),
            now(),
        );
        let report = evaluate_nat_gateway_fleet(&[r], Pillar::Security, now());
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
    fn stale_inventory_is_reported() {
        let mut r = fixture(
            "nat-stale",
            json!({"team": "net"}),
            healthy_data("nat-stale", "vpc-1", "subnet-a"),
            now(),
        );
        r.last_refreshed = now() - Duration::hours(48);
        let report = evaluate_nat_gateway_fleet(&[r], Pillar::Cost, now());
        assert_eq!(report.stale_resources, 1);
        assert!(report
            .findings
            .iter()
            .any(|f| f.reason_code == REASON_INV_STALE_DATA));
    }

    #[test]
    fn non_nat_gateway_rows_are_skipped() {
        let mut r = fixture(
            "igw-1",
            json!({}),
            json!({"internet_gateway_id": "igw-1"}),
            now(),
        );
        r.resource_type = "InternetGateway".to_string();
        let report = evaluate_nat_gateway_fleet(&[r], Pillar::Cost, now());
        assert_eq!(report.resources_evaluated, 0);
        assert!(report.findings.is_empty());
        assert_eq!(report.score, 100);
    }

    #[test]
    fn healthy_redundant_fleet_passes_cost_and_resilience() {
        let a = fixture(
            "nat-a",
            json!({"team": "net", "owner": "sre"}),
            healthy_data("nat-a", "vpc-multi", "subnet-a"),
            now(),
        );
        let b = fixture(
            "nat-b",
            json!({"team": "net", "owner": "sre"}),
            healthy_data("nat-b", "vpc-multi", "subnet-b"),
            now(),
        );
        let fleet = [a, b];

        for pillar in [Pillar::Cost, Pillar::Resilience] {
            let report = evaluate_nat_gateway_fleet(&fleet, pillar, now());
            assert_eq!(report.resources_evaluated, 2);
            assert!(
                report.findings.is_empty(),
                "unexpected for {:?}: {:?}",
                pillar,
                report.findings
            );
        }

        let security = evaluate_nat_gateway_fleet(&fleet, Pillar::Security, now());
        assert_eq!(security.findings.len(), 2);
        assert!(security
            .findings
            .iter()
            .all(|f| f.reason_code == REASON_SEC_POSTURE_DATA_NOT_COLLECTED));
    }
}

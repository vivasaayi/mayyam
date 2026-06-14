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

// Deterministic Subnet inventory evaluators for the cost, security, and
// resilience pillars (roadmap rows 01-AWS-CLOUD-02836/02845/02872).
//
// Evaluates fields persisted by vpc_control_plane::sync_subnets: subnet_id,
// cidr_block, vpc_id, availability_zone, availability_zone_id, state. Tags
// are collected for subnets. Per-subnet security posture (for example
// map_public_ip_on_launch) is not collected yet, so the security pillar
// reports an honest data-gap finding instead of guessing.

use chrono::{DateTime, Utc};
use serde_json::json;

use crate::models::aws_resource::Model as AwsResourceModel;
use crate::services::aws::inventory::types::{
    check_stale, score_pillar, InventoryFinding, Pillar, PillarReport, Severity,
};

// Reason codes are the stable contract for findings; never reuse or rename.
pub const REASON_COST_NO_TAGS: &str = "SUBNET_COST_NO_TAGS";
pub const REASON_RES_STATE_NOT_AVAILABLE: &str = "SUBNET_RES_STATE_NOT_AVAILABLE";
pub const REASON_RES_AZ_DATA_NOT_COLLECTED: &str = "SUBNET_RES_AZ_DATA_NOT_COLLECTED";
pub const REASON_SEC_POSTURE_DATA_NOT_COLLECTED: &str = "SUBNET_SEC_POSTURE_DATA_NOT_COLLECTED";
pub const REASON_INV_STALE_DATA: &str = "SUBNET_INV_STALE_DATA";

fn is_subnet(resource: &AwsResourceModel) -> bool {
    resource.resource_type == "Subnet"
}

/// Evaluate every subnet in the fleet for one pillar. Rows whose
/// resource_type is not "Subnet" are skipped and not counted.
pub fn evaluate_subnet_fleet(
    resources: &[AwsResourceModel],
    pillar: Pillar,
    now: DateTime<Utc>,
) -> PillarReport {
    let mut findings: Vec<InventoryFinding> = Vec::new();
    let mut stale_resources = 0usize;
    let mut resources_evaluated = 0usize;

    for resource in resources {
        if !is_subnet(resource) {
            continue;
        }
        resources_evaluated += 1;
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
        resources_evaluated,
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
                "Subnet {} has no tags recorded (untagged resource or tag collection gap); cost allocation cannot be assessed",
                resource.resource_id
            ),
            evidence: json!({ "tags": resource.tags }),
        });
    }
}

fn evaluate_security(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    // sync_subnets does not collect per-subnet security posture fields such
    // as map_public_ip_on_launch, so report the gap instead of guessing.
    findings.push(InventoryFinding {
        resource_id: resource.resource_id.clone(),
        arn: resource.arn.clone(),
        pillar: Pillar::Security,
        reason_code: REASON_SEC_POSTURE_DATA_NOT_COLLECTED.to_string(),
        severity: Severity::Medium,
        message: format!(
            "Security posture for subnet {} is not collected yet (no map_public_ip_on_launch data); security pillar cannot be fully assessed",
            resource.resource_id
        ),
        evidence: json!({ "security_posture_fields_collected": false }),
    });
}

fn evaluate_resilience(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    let state = resource.resource_data.get("state").and_then(|v| v.as_str());
    if let Some(state) = state {
        if state != "available" {
            findings.push(InventoryFinding {
                resource_id: resource.resource_id.clone(),
                arn: resource.arn.clone(),
                pillar: Pillar::Resilience,
                reason_code: REASON_RES_STATE_NOT_AVAILABLE.to_string(),
                severity: Severity::High,
                message: format!(
                    "Subnet {} is in state '{}' instead of 'available'; workloads in it may be unreachable",
                    resource.resource_id, state
                ),
                evidence: json!({ "state": state }),
            });
        }
    }

    let az = resource
        .resource_data
        .get("availability_zone")
        .and_then(|v| v.as_str());
    if az.is_none() {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Resilience,
            reason_code: REASON_RES_AZ_DATA_NOT_COLLECTED.to_string(),
            severity: Severity::Medium,
            message: format!(
                "Availability zone for subnet {} is not collected yet; AZ spread cannot be assessed",
                resource.resource_id
            ),
            evidence: json!({ "availability_zone_collected": false }),
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
            resource_type: "Subnet".to_string(),
            resource_id: resource_id.to_string(),
            arn: format!("arn:aws:ec2:us-east-1:123456789012:subnet/{}", resource_id),
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
            "subnet_id": "subnet-ok",
            "cidr_block": "10.0.1.0/24",
            "vpc_id": "vpc-1",
            "availability_zone": "us-east-1a",
            "availability_zone_id": "use1-az1",
            "state": "available",
        })
    }

    #[test]
    fn cost_flags_missing_tags() {
        let r = fixture("subnet-untagged", json!({}), healthy_data(), now());
        let report = evaluate_subnet_fleet(&[r], Pillar::Cost, now());
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
    fn resilience_flags_state_not_available() {
        let mut data = healthy_data();
        data["state"] = json!("pending");
        let r = fixture("subnet-pending", json!({"team": "net"}), data, now());
        let report = evaluate_subnet_fleet(&[r], Pillar::Resilience, now());
        assert_eq!(
            report
                .findings
                .iter()
                .map(|f| f.reason_code.as_str())
                .collect::<Vec<_>>(),
            vec![REASON_RES_STATE_NOT_AVAILABLE]
        );
    }

    #[test]
    fn resilience_reports_gap_when_az_not_collected() {
        let r = fixture(
            "subnet-no-az",
            json!({"team": "net"}),
            json!({"subnet_id": "subnet-no-az", "state": "available"}),
            now(),
        );
        let report = evaluate_subnet_fleet(&[r], Pillar::Resilience, now());
        assert_eq!(
            report
                .findings
                .iter()
                .map(|f| f.reason_code.as_str())
                .collect::<Vec<_>>(),
            vec![REASON_RES_AZ_DATA_NOT_COLLECTED]
        );
    }

    #[test]
    fn security_reports_posture_data_gap() {
        let r = fixture("subnet-sec", json!({"team": "net"}), healthy_data(), now());
        let report = evaluate_subnet_fleet(&[r], Pillar::Security, now());
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
            "subnet-stale",
            json!({"team": "net"}),
            healthy_data(),
            now(),
        );
        r.last_refreshed = now() - Duration::hours(48);
        let report = evaluate_subnet_fleet(&[r], Pillar::Cost, now());
        assert_eq!(report.stale_resources, 1);
        assert!(report
            .findings
            .iter()
            .any(|f| f.reason_code == REASON_INV_STALE_DATA));
    }

    #[test]
    fn non_subnet_rows_are_skipped() {
        let mut r = fixture("vpc-1", json!({}), json!({}), now());
        r.resource_type = "Vpc".to_string();
        let report = evaluate_subnet_fleet(&[r], Pillar::Cost, now());
        assert_eq!(report.resources_evaluated, 0);
        assert!(report.findings.is_empty());
        assert_eq!(report.score, 100);
    }

    #[test]
    fn healthy_subnet_passes_cost_and_resilience_and_only_gaps_security() {
        let r = fixture("subnet-ok", json!({"team": "net"}), healthy_data(), now());
        for pillar in [Pillar::Cost, Pillar::Resilience] {
            let report = evaluate_subnet_fleet(std::slice::from_ref(&r), pillar, now());
            assert!(
                report.findings.is_empty(),
                "unexpected for {:?}: {:?}",
                pillar,
                report.findings
            );
        }
        let report = evaluate_subnet_fleet(std::slice::from_ref(&r), Pillar::Security, now());
        assert_eq!(
            report
                .findings
                .iter()
                .map(|f| f.reason_code.as_str())
                .collect::<Vec<_>>(),
            vec![REASON_SEC_POSTURE_DATA_NOT_COLLECTED]
        );
    }
}

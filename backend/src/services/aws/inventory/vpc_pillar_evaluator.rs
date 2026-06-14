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

// Deterministic VPC inventory evaluators for the cost, security, and
// resilience pillars (roadmap rows 01-AWS-CLOUD-02773/02782/02809).
//
// Evaluates Vpc rows persisted by vpc_control_plane: vpc_id, cidr_block,
// state, is_default. Flow logs are not collected yet and are reported as
// an explicit data gap.

use chrono::{DateTime, Utc};
use serde_json::json;

use crate::models::aws_resource::Model as AwsResourceModel;
use crate::services::aws::inventory::types::{
    check_stale, data_str, score_pillar, InventoryFinding, Pillar, PillarReport, Severity,
};

// Reason codes are the stable contract for findings; never reuse or rename.
pub const REASON_COST_NO_TAGS: &str = "VPC_COST_NO_TAGS";
pub const REASON_SEC_DEFAULT_VPC_PRESENT: &str = "VPC_SEC_DEFAULT_VPC_PRESENT";
pub const REASON_SEC_FLOW_LOGS_DATA_NOT_COLLECTED: &str = "VPC_SEC_FLOW_LOGS_DATA_NOT_COLLECTED";
pub const REASON_RES_NOT_AVAILABLE: &str = "VPC_RES_NOT_AVAILABLE";
pub const REASON_INV_STALE_DATA: &str = "VPC_INV_STALE_DATA";

/// Evaluate every VPC in the fleet for one pillar.
pub fn evaluate_vpc_fleet(
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
            severity: Severity::Low,
            message: format!(
                "VPC {} has no tags recorded; attached billable resources (NAT gateways, endpoints) cannot be allocated",
                resource.resource_id
            ),
            evidence: json!({ "tags": resource.tags }),
        });
    }
}

fn evaluate_security(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    let is_default = resource
        .resource_data
        .get("is_default")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    if is_default {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Security,
            reason_code: REASON_SEC_DEFAULT_VPC_PRESENT.to_string(),
            severity: Severity::Medium,
            message: format!(
                "VPC {} is the default VPC; default VPCs allow broad implicit networking and should not host workloads",
                resource.resource_id
            ),
            evidence: json!({ "is_default": true }),
        });
    }

    if resource.resource_data.get("flow_logs").is_none() {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Security,
            reason_code: REASON_SEC_FLOW_LOGS_DATA_NOT_COLLECTED.to_string(),
            severity: Severity::Medium,
            message: format!(
                "Flow log configuration for VPC {} is not collected yet; network audit posture cannot be assessed",
                resource.resource_id
            ),
            evidence: json!({ "flow_logs_collected": false }),
        });
    }
}

fn evaluate_resilience(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    if let Some(state) = data_str(&resource.resource_data, "state") {
        if state != "available" {
            findings.push(InventoryFinding {
                resource_id: resource.resource_id.clone(),
                arn: resource.arn.clone(),
                pillar: Pillar::Resilience,
                reason_code: REASON_RES_NOT_AVAILABLE.to_string(),
                severity: Severity::Medium,
                message: format!("VPC {} is in state '{}'", resource.resource_id, state),
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
            resource_type: "Vpc".to_string(),
            resource_id: resource_id.to_string(),
            arn: format!("arn:aws:ec2:us-east-1:123456789012:vpc/{}", resource_id),
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
    fn security_flags_default_vpc_and_flow_log_gap() {
        let r = fixture(
            "vpc-default",
            json!({"team": "net"}),
            json!({"vpc_id": "vpc-default", "state": "available", "is_default": true}),
            now(),
        );
        let report = evaluate_vpc_fleet(&[r], Pillar::Security, now());
        let codes: Vec<&str> = report
            .findings
            .iter()
            .map(|f| f.reason_code.as_str())
            .collect();
        assert!(codes.contains(&REASON_SEC_DEFAULT_VPC_PRESENT));
        assert!(codes.contains(&REASON_SEC_FLOW_LOGS_DATA_NOT_COLLECTED));
    }

    #[test]
    fn security_passes_for_custom_vpc_with_flow_log_data() {
        let r = fixture(
            "vpc-app",
            json!({"team": "net"}),
            json!({"vpc_id": "vpc-app", "state": "available", "is_default": false, "flow_logs": [{"id": "fl-1"}]}),
            now(),
        );
        let report = evaluate_vpc_fleet(&[r], Pillar::Security, now());
        assert!(
            report.findings.is_empty(),
            "unexpected: {:?}",
            report.findings
        );
    }

    #[test]
    fn resilience_flags_pending_vpc() {
        let r = fixture(
            "vpc-new",
            json!({"team": "net"}),
            json!({"vpc_id": "vpc-new", "state": "pending", "is_default": false}),
            now(),
        );
        let report = evaluate_vpc_fleet(&[r], Pillar::Resilience, now());
        assert_eq!(
            report
                .findings
                .iter()
                .map(|f| f.reason_code.as_str())
                .collect::<Vec<_>>(),
            vec![REASON_RES_NOT_AVAILABLE]
        );
    }

    #[test]
    fn cost_reports_tag_gap_as_low() {
        let r = fixture(
            "vpc-untagged",
            json!({}),
            json!({"vpc_id": "vpc-untagged", "state": "available", "is_default": false}),
            now(),
        );
        let report = evaluate_vpc_fleet(&[r], Pillar::Cost, now());
        assert_eq!(report.findings.len(), 1);
        assert_eq!(report.findings[0].severity, Severity::Low);
    }
}

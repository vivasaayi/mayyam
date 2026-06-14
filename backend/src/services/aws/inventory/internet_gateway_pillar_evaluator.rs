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

// Deterministic Internet Gateway inventory evaluators for the cost,
// security, and resilience pillars (roadmap rows
// 01-AWS-CLOUD-03025/03034/03061).
//
// vpc_control_plane::sync_internet_gateways currently persists only
// `internet_gateway_id` in resource_data; attachments are not collected
// yet. Attachment-based checks therefore branch on the presence of an
// `attachments` array (entries with `state`/`vpc_id`): when the key is
// absent the resilience pillar reports a data-collection gap instead of
// guessing, and the detached-sprawl cost check stays silent.
//
// Security pillar note: an attached Internet Gateway only proves the VPC
// has a door to the internet, not that any workload is exposed — exposure
// depends on route tables, subnet associations, public IPs, and security
// groups, none of which are part of this resource's collected fields.
// Nearly every functional VPC has an attached IGW, so flagging each one
// would deterministically penalize healthy fleets with noise. The
// security pillar is intentionally left clean for this resource type;
// internet-exposure findings belong to evaluators that see routing data.

use chrono::{DateTime, Utc};
use serde_json::json;

use crate::models::aws_resource::Model as AwsResourceModel;
use crate::services::aws::inventory::types::{
    check_stale, score_pillar, InventoryFinding, Pillar, PillarReport, Severity,
};

// Reason codes are the stable contract for findings; never reuse or rename.
pub const REASON_COST_NO_TAGS: &str = "IGW_COST_NO_TAGS";
pub const REASON_COST_DETACHED: &str = "IGW_COST_DETACHED";
pub const REASON_RES_ATTACHMENT_NOT_AVAILABLE: &str = "IGW_RES_ATTACHMENT_NOT_AVAILABLE";
pub const REASON_RES_ATTACHMENT_DATA_NOT_COLLECTED: &str = "IGW_RES_ATTACHMENT_DATA_NOT_COLLECTED";
pub const REASON_INV_STALE_DATA: &str = "IGW_INV_STALE_DATA";

/// Attachment states that count as a healthy VPC attachment.
const HEALTHY_ATTACHMENT_STATES: &[&str] = &["available", "attached"];

/// Evaluate every Internet Gateway in the fleet for one pillar.
/// Rows with a different `resource_type` are skipped and not counted.
pub fn evaluate_internet_gateway_fleet(
    resources: &[AwsResourceModel],
    pillar: Pillar,
    now: DateTime<Utc>,
) -> PillarReport {
    let mut findings: Vec<InventoryFinding> = Vec::new();
    let mut stale_resources = 0usize;
    let mut evaluated = 0usize;

    for resource in resources {
        if resource.resource_type != "InternetGateway" {
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
                "Internet gateway {} has no tags recorded (untagged resource or tag collection gap); cost allocation cannot be assessed",
                resource.resource_id
            ),
            evidence: json!({ "tags": resource.tags }),
        });
    }

    // Detached-gateway sprawl is only assessable when attachments were
    // actually collected; the collector does not persist them yet, so this
    // check stays silent until it does.
    if let Some(attachments) = resource
        .resource_data
        .get("attachments")
        .and_then(|v| v.as_array())
    {
        if attachments.is_empty() {
            findings.push(InventoryFinding {
                resource_id: resource.resource_id.clone(),
                arn: resource.arn.clone(),
                pillar: Pillar::Cost,
                reason_code: REASON_COST_DETACHED.to_string(),
                severity: Severity::Low,
                message: format!(
                    "Internet gateway {} is not attached to any VPC; it is unused resource sprawl and can likely be deleted",
                    resource.resource_id
                ),
                evidence: json!({ "attachments": [] }),
            });
        }
    }
}

fn evaluate_security(_resource: &AwsResourceModel, _findings: &mut Vec<InventoryFinding>) {
    // Intentionally clean: with only `internet_gateway_id` (and at most
    // attachment state) collected, an attached IGW does not evidence
    // workload exposure, and flagging every standard VPC's gateway is
    // noise. See the module doc comment for the full rationale.
}

fn evaluate_resilience(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    let attachments = resource
        .resource_data
        .get("attachments")
        .and_then(|v| v.as_array());
    let Some(attachments) = attachments else {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Resilience,
            reason_code: REASON_RES_ATTACHMENT_DATA_NOT_COLLECTED.to_string(),
            severity: Severity::Medium,
            message: format!(
                "Attachment state for internet gateway {} is not collected yet; resilience pillar cannot be fully assessed",
                resource.resource_id
            ),
            evidence: json!({ "attachments_collected": false }),
        });
        return;
    };

    for attachment in attachments {
        let state = attachment.get("state").and_then(|v| v.as_str());
        if let Some(state) = state {
            if !HEALTHY_ATTACHMENT_STATES.contains(&state) {
                findings.push(InventoryFinding {
                    resource_id: resource.resource_id.clone(),
                    arn: resource.arn.clone(),
                    pillar: Pillar::Resilience,
                    reason_code: REASON_RES_ATTACHMENT_NOT_AVAILABLE.to_string(),
                    severity: Severity::Medium,
                    message: format!(
                        "Internet gateway {} has a VPC attachment in state '{}' (expected 'available' or 'attached'); internet routing for the VPC may be degraded",
                        resource.resource_id, state
                    ),
                    evidence: json!({
                        "state": state,
                        "vpc_id": attachment.get("vpc_id"),
                    }),
                });
            }
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
            resource_type: "InternetGateway".to_string(),
            resource_id: resource_id.to_string(),
            arn: format!(
                "arn:aws:ec2:us-east-1:123456789012:internet-gateway/{}",
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
            "internet_gateway_id": "igw-ok",
            "attachments": [{ "state": "available", "vpc_id": "vpc-1" }],
        })
    }

    #[test]
    fn cost_flags_untagged_gateway() {
        let r = fixture("igw-untagged", json!({}), healthy_data(), now());
        let report = evaluate_internet_gateway_fleet(&[r], Pillar::Cost, now());
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
    fn cost_flags_detached_gateway_when_attachments_collected() {
        let r = fixture(
            "igw-orphan",
            json!({"team": "net"}),
            json!({"internet_gateway_id": "igw-orphan", "attachments": []}),
            now(),
        );
        let report = evaluate_internet_gateway_fleet(&[r], Pillar::Cost, now());
        assert_eq!(
            report
                .findings
                .iter()
                .map(|f| f.reason_code.as_str())
                .collect::<Vec<_>>(),
            vec![REASON_COST_DETACHED]
        );
    }

    #[test]
    fn cost_stays_silent_on_detachment_when_attachments_not_collected() {
        let r = fixture(
            "igw-nodata",
            json!({"team": "net"}),
            json!({"internet_gateway_id": "igw-nodata"}),
            now(),
        );
        let report = evaluate_internet_gateway_fleet(&[r], Pillar::Cost, now());
        assert!(
            report.findings.is_empty(),
            "unexpected: {:?}",
            report.findings
        );
    }

    #[test]
    fn resilience_flags_attachment_not_available() {
        let r = fixture(
            "igw-detaching",
            json!({"team": "net"}),
            json!({
                "internet_gateway_id": "igw-detaching",
                "attachments": [{ "state": "detaching", "vpc_id": "vpc-1" }],
            }),
            now(),
        );
        let report = evaluate_internet_gateway_fleet(&[r], Pillar::Resilience, now());
        assert_eq!(
            report
                .findings
                .iter()
                .map(|f| f.reason_code.as_str())
                .collect::<Vec<_>>(),
            vec![REASON_RES_ATTACHMENT_NOT_AVAILABLE]
        );
    }

    #[test]
    fn resilience_reports_gap_when_attachments_not_collected() {
        let r = fixture(
            "igw-gap",
            json!({"team": "net"}),
            json!({"internet_gateway_id": "igw-gap"}),
            now(),
        );
        let report = evaluate_internet_gateway_fleet(&[r], Pillar::Resilience, now());
        assert_eq!(
            report
                .findings
                .iter()
                .map(|f| f.reason_code.as_str())
                .collect::<Vec<_>>(),
            vec![REASON_RES_ATTACHMENT_DATA_NOT_COLLECTED]
        );
    }

    #[test]
    fn stale_inventory_is_reported() {
        let mut r = fixture("igw-stale", json!({"team": "net"}), healthy_data(), now());
        r.last_refreshed = now() - Duration::hours(48);
        let report = evaluate_internet_gateway_fleet(&[r], Pillar::Cost, now());
        assert_eq!(report.stale_resources, 1);
        assert!(report
            .findings
            .iter()
            .any(|f| f.reason_code == REASON_INV_STALE_DATA));
    }

    #[test]
    fn non_internet_gateway_rows_are_skipped() {
        let mut other = fixture("vpc-1", json!({}), json!({}), now());
        other.resource_type = "Vpc".to_string();
        let igw = fixture("igw-only", json!({"team": "net"}), healthy_data(), now());
        let report = evaluate_internet_gateway_fleet(&[other, igw], Pillar::Cost, now());
        assert_eq!(report.resources_evaluated, 1);
        assert!(
            report.findings.is_empty(),
            "unexpected: {:?}",
            report.findings
        );
    }

    #[test]
    fn healthy_gateway_passes_all_pillars() {
        let r = fixture("igw-ok", json!({"team": "net"}), healthy_data(), now());
        for pillar in [Pillar::Cost, Pillar::Security, Pillar::Resilience] {
            let report = evaluate_internet_gateway_fleet(std::slice::from_ref(&r), pillar, now());
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

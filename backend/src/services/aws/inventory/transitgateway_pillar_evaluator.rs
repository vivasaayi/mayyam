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

// Deterministic Transit Gateway inventory evaluators for the cost,
// resilience, and security pillars (roadmap rows
// 01-AWS-CLOUD-03466/03475/03502).
//
// Evaluates fields persisted by transitgateway_control_plane: state,
// attachment_count, vpn_ecmp_support, auto_accept_shared_attachments,
// default_route_table_association, default_route_table_propagation, plus the
// tags column.

use chrono::{DateTime, Utc};
use serde_json::{json, Value};

use crate::models::aws_resource::Model as AwsResourceModel;
use crate::services::aws::inventory::types::{
    check_stale, data_str, score_pillar, InventoryFinding, Pillar, PillarReport, Severity,
};

/// Only rows of this resource type are evaluated.
pub const RESOURCE_TYPE: &str = "TransitGateway";

// Reason codes are the stable contract for findings; never reuse or rename.
pub const REASON_COST_NO_TAGS: &str = "TGW_COST_NO_TAGS";
pub const REASON_COST_NO_ATTACHMENTS: &str = "TGW_COST_NO_ATTACHMENTS";
pub const REASON_RES_NOT_AVAILABLE: &str = "TGW_RES_NOT_AVAILABLE";
pub const REASON_RES_VPN_ECMP_DISABLED: &str = "TGW_RES_VPN_ECMP_DISABLED";
pub const REASON_SEC_AUTO_ACCEPT_SHARED_ATTACHMENTS: &str =
    "TGW_SEC_AUTO_ACCEPT_SHARED_ATTACHMENTS";
pub const REASON_SEC_FLAT_ROUTING_DEFAULTS: &str = "TGW_SEC_FLAT_ROUTING_DEFAULTS";
pub const REASON_INV_STALE_DATA: &str = "TGW_INV_STALE_DATA";

/// Evaluate every Transit Gateway in the fleet for one pillar. Rows whose
/// `resource_type` is not `TransitGateway` are skipped and not counted.
pub fn evaluate_transitgateway_fleet(
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

fn data_usize(resource_data: &Value, key: &str) -> Option<usize> {
    resource_data
        .get(key)
        .and_then(|v| v.as_u64())
        .map(|v| v as usize)
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
                "Transit Gateway {} has no tags recorded (untagged resource or tag collection gap); cost allocation cannot be assessed",
                resource.resource_id
            ),
            evidence: json!({ "tags": resource.tags }),
        });
    }

    // An available transit gateway bills hourly even with nothing attached;
    // zero attachments means pure spend. Deleted or transitional gateways are
    // not flagged because the attachment count is not meaningful there.
    let state = data_str(&resource.resource_data, "state");
    let attachment_count = data_usize(&resource.resource_data, "attachment_count");
    if state.as_deref() == Some("available") && attachment_count == Some(0) {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Cost,
            reason_code: REASON_COST_NO_ATTACHMENTS.to_string(),
            severity: Severity::Medium,
            message: format!(
                "Transit Gateway {} is available but has no attachments; the hourly transit gateway charge is incurred with nothing connected",
                resource.resource_id
            ),
            evidence: json!({ "state": "available", "attachment_count": 0 }),
        });
    }
}

fn evaluate_security(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    if data_bool(&resource.resource_data, "auto_accept_shared_attachments") == Some(true) {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Security,
            reason_code: REASON_SEC_AUTO_ACCEPT_SHARED_ATTACHMENTS.to_string(),
            severity: Severity::Medium,
            message: format!(
                "Transit Gateway {} auto-accepts shared attachments; cross-account attachments attach without review",
                resource.resource_id
            ),
            evidence: json!({ "auto_accept_shared_attachments": true }),
        });
    }

    // With both default association and default propagation enabled, every
    // attachment lands on the default route table and learns every other
    // attachment's routes, producing a flat network by default.
    let default_association =
        data_bool(&resource.resource_data, "default_route_table_association").unwrap_or(false);
    let default_propagation =
        data_bool(&resource.resource_data, "default_route_table_propagation").unwrap_or(false);
    if default_association && default_propagation {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Security,
            reason_code: REASON_SEC_FLAT_ROUTING_DEFAULTS.to_string(),
            severity: Severity::Low,
            message: format!(
                "Transit Gateway {} enables both default route table association and propagation; every attachment can reach every other attachment by default (flat network)",
                resource.resource_id
            ),
            evidence: json!({
                "default_route_table_association": true,
                "default_route_table_propagation": true,
            }),
        });
    }
}

fn evaluate_resilience(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    // pending/modifying/deleting are transitional states worth surfacing;
    // "deleted" rows are inert leftovers and skip this check.
    if let Some(state) = data_str(&resource.resource_data, "state") {
        if state != "available" && state != "deleted" {
            findings.push(InventoryFinding {
                resource_id: resource.resource_id.clone(),
                arn: resource.arn.clone(),
                pillar: Pillar::Resilience,
                reason_code: REASON_RES_NOT_AVAILABLE.to_string(),
                severity: Severity::Medium,
                message: format!(
                    "Transit Gateway {} is in state '{}' instead of 'available'; attached networks may be degraded or in transition",
                    resource.resource_id, state
                ),
                evidence: json!({ "state": state }),
            });
        }
    }

    if data_bool(&resource.resource_data, "vpn_ecmp_support") == Some(false) {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Resilience,
            reason_code: REASON_RES_VPN_ECMP_DISABLED.to_string(),
            severity: Severity::Low,
            message: format!(
                "Transit Gateway {} has VPN ECMP support disabled; VPN attachments cannot load-balance across tunnels, reducing redundancy and throughput",
                resource.resource_id
            ),
            evidence: json!({ "vpn_ecmp_support": false }),
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
                "arn:aws:ec2:us-east-1:123456789012:transit-gateway/{}",
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
            "transit_gateway_id": "tgw-ok",
            "state": "available",
            "owner_id": "123456789012",
            "amazon_side_asn": 64512,
            "auto_accept_shared_attachments": false,
            "default_route_table_association": true,
            "default_route_table_propagation": false,
            "dns_support": true,
            "vpn_ecmp_support": true,
            "multicast_support": false,
            "attachment_count": 3,
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
    fn cost_flags_untagged_transit_gateway() {
        let r = fixture("tgw-untagged", json!({}), healthy_data(), now());
        let report = evaluate_transitgateway_fleet(&[r], Pillar::Cost, now());
        assert_eq!(codes(&report), vec![REASON_COST_NO_TAGS]);
    }

    #[test]
    fn cost_flags_available_gateway_without_attachments() {
        let mut data = healthy_data();
        data["attachment_count"] = json!(0);
        let r = fixture("tgw-idle", json!({"team": "network"}), data, now());
        let report = evaluate_transitgateway_fleet(&[r], Pillar::Cost, now());
        assert_eq!(codes(&report), vec![REASON_COST_NO_ATTACHMENTS]);
    }

    #[test]
    fn cost_does_not_flag_available_gateway_with_attachments() {
        let r = fixture(
            "tgw-busy",
            json!({"team": "network"}),
            healthy_data(),
            now(),
        );
        let report = evaluate_transitgateway_fleet(&[r], Pillar::Cost, now());
        assert!(
            report.findings.is_empty(),
            "unexpected: {:?}",
            report.findings
        );
    }

    #[test]
    fn resilience_flags_pending_state() {
        let mut data = healthy_data();
        data["state"] = json!("pending");
        let r = fixture("tgw-pending", json!({"team": "network"}), data, now());
        let report = evaluate_transitgateway_fleet(&[r], Pillar::Resilience, now());
        assert_eq!(codes(&report), vec![REASON_RES_NOT_AVAILABLE]);
        assert_eq!(report.findings[0].evidence["state"], json!("pending"));
    }

    #[test]
    fn deleted_gateway_is_not_flagged_for_state_or_attachments() {
        let mut data = healthy_data();
        data["state"] = json!("deleted");
        data["attachment_count"] = json!(0);
        let r = fixture("tgw-deleted", json!({"team": "network"}), data, now());
        let resilience =
            evaluate_transitgateway_fleet(std::slice::from_ref(&r), Pillar::Resilience, now());
        assert!(
            resilience.findings.is_empty(),
            "unexpected: {:?}",
            resilience.findings
        );
        let cost = evaluate_transitgateway_fleet(&[r], Pillar::Cost, now());
        assert!(cost.findings.is_empty(), "unexpected: {:?}", cost.findings);
    }

    #[test]
    fn resilience_flags_vpn_ecmp_disabled() {
        let mut data = healthy_data();
        data["vpn_ecmp_support"] = json!(false);
        let r = fixture("tgw-noecmp", json!({"team": "network"}), data, now());
        let report = evaluate_transitgateway_fleet(&[r], Pillar::Resilience, now());
        assert_eq!(codes(&report), vec![REASON_RES_VPN_ECMP_DISABLED]);
        assert!(matches!(report.findings[0].severity, Severity::Low));
    }

    #[test]
    fn security_flags_auto_accept_shared_attachments() {
        let mut data = healthy_data();
        data["auto_accept_shared_attachments"] = json!(true);
        let r = fixture("tgw-autoaccept", json!({"team": "network"}), data, now());
        let report = evaluate_transitgateway_fleet(&[r], Pillar::Security, now());
        assert_eq!(
            codes(&report),
            vec![REASON_SEC_AUTO_ACCEPT_SHARED_ATTACHMENTS]
        );
    }

    #[test]
    fn security_flags_flat_routing_defaults() {
        let mut data = healthy_data();
        data["default_route_table_association"] = json!(true);
        data["default_route_table_propagation"] = json!(true);
        let r = fixture("tgw-flat", json!({"team": "network"}), data, now());
        let report = evaluate_transitgateway_fleet(&[r], Pillar::Security, now());
        assert_eq!(codes(&report), vec![REASON_SEC_FLAT_ROUTING_DEFAULTS]);
    }

    #[test]
    fn security_requires_both_routing_defaults_for_flat_network() {
        let mut data = healthy_data();
        data["default_route_table_association"] = json!(true);
        data["default_route_table_propagation"] = json!(false);
        let r = fixture("tgw-assoc-only", json!({"team": "network"}), data, now());
        let report = evaluate_transitgateway_fleet(&[r], Pillar::Security, now());
        assert!(
            report.findings.is_empty(),
            "unexpected: {:?}",
            report.findings
        );
    }

    #[test]
    fn stale_inventory_is_flagged() {
        let mut r = fixture(
            "tgw-stale",
            json!({"team": "network"}),
            healthy_data(),
            now(),
        );
        r.last_refreshed = now() - Duration::hours(48);
        let report = evaluate_transitgateway_fleet(&[r], Pillar::Resilience, now());
        assert_eq!(report.stale_resources, 1);
        assert!(codes(&report).contains(&REASON_INV_STALE_DATA));
    }

    #[test]
    fn non_tgw_resources_are_skipped_and_not_counted() {
        let mut r = fixture("queue-1", json!({}), json!({}), now());
        r.resource_type = "SqsQueue".to_string();
        let report = evaluate_transitgateway_fleet(&[r], Pillar::Cost, now());
        assert_eq!(report.resources_evaluated, 0);
        assert!(report.findings.is_empty());
    }

    #[test]
    fn healthy_transit_gateway_passes_all_pillars() {
        let r = fixture("tgw-ok", json!({"team": "network"}), healthy_data(), now());
        for pillar in [Pillar::Cost, Pillar::Security, Pillar::Resilience] {
            let report = evaluate_transitgateway_fleet(std::slice::from_ref(&r), pillar, now());
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

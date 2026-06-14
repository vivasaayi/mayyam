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

// Deterministic Network ACL inventory evaluators for the cost, security,
// and resilience pillars (roadmap rows 01-AWS-CLOUD-03151/03160/03187).
//
// Evaluates fields persisted by vpc_control_plane::sync_network_acls:
// network_acl_id, vpc_id, is_default. The collector does not yet persist
// NACL `entries` or `associations`; the security evaluator therefore
// reports a data gap when `entries` is absent and only inspects rule
// entries when a future collector version writes them (expected shape:
// `entries: [{"rule_action": "allow", "cidr_block": "0.0.0.0/0",
// "protocol": "-1", "egress": false}]`). The unassociated-sprawl cost
// check likewise fires only when an `associations` array is collected.
//
// Resilience note: a network ACL is a stateless, regionally managed VPC
// construct with no per-resource availability configuration in the
// collected fields, so there is no honest per-NACL resilience signal.
// The resilience pillar intentionally emits no findings beyond the
// shared stale-data check rather than inventing one.

use chrono::{DateTime, Utc};
use serde_json::json;

use crate::models::aws_resource::Model as AwsResourceModel;
use crate::services::aws::inventory::types::{
    check_stale, score_pillar, InventoryFinding, Pillar, PillarReport, Severity,
};

// Reason codes are the stable contract for findings; never reuse or rename.
pub const REASON_COST_NO_TAGS: &str = "NETWORKACL_COST_NO_TAGS";
pub const REASON_COST_UNASSOCIATED: &str = "NETWORKACL_COST_UNASSOCIATED";
pub const REASON_SEC_ALLOW_ALL_INGRESS: &str = "NETWORKACL_SEC_ALLOW_ALL_INGRESS";
pub const REASON_SEC_ENTRIES_DATA_NOT_COLLECTED: &str = "NETWORKACL_SEC_ENTRIES_DATA_NOT_COLLECTED";
pub const REASON_INV_STALE_DATA: &str = "NETWORKACL_INV_STALE_DATA";

/// Evaluate every Network ACL in the fleet for one pillar. Rows whose
/// `resource_type` is not `NetworkAcl` are skipped and not counted.
pub fn evaluate_network_acl_fleet(
    resources: &[AwsResourceModel],
    pillar: Pillar,
    now: DateTime<Utc>,
) -> PillarReport {
    let mut findings: Vec<InventoryFinding> = Vec::new();
    let mut stale_resources = 0usize;
    let mut evaluated = 0usize;

    for resource in resources {
        if resource.resource_type != "NetworkAcl" {
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
                "Network ACL {} has no tags recorded (untagged resource or tag collection gap); cost allocation cannot be assessed",
                resource.resource_id
            ),
            evidence: json!({ "tags": resource.tags }),
        });
    }

    // Sprawl check: a non-default NACL associated with no subnets is unused
    // configuration debt. Fires only when the collector has actually
    // persisted both `associations` and `is_default`.
    let associations = resource
        .resource_data
        .get("associations")
        .and_then(|v| v.as_array());
    let is_default = resource
        .resource_data
        .get("is_default")
        .and_then(|v| v.as_bool());
    if let (Some(assocs), Some(false)) = (associations, is_default) {
        if assocs.is_empty() {
            findings.push(InventoryFinding {
                resource_id: resource.resource_id.clone(),
                arn: resource.arn.clone(),
                pillar: Pillar::Cost,
                reason_code: REASON_COST_UNASSOCIATED.to_string(),
                severity: Severity::Low,
                message: format!(
                    "Network ACL {} is non-default and associated with no subnets; it is unused configuration sprawl and a cleanup candidate",
                    resource.resource_id
                ),
                evidence: json!({ "associations": [], "is_default": false }),
            });
        }
    }
}

fn evaluate_security(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    let entries = resource
        .resource_data
        .get("entries")
        .and_then(|v| v.as_array());

    let Some(entries) = entries else {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Security,
            reason_code: REASON_SEC_ENTRIES_DATA_NOT_COLLECTED.to_string(),
            severity: Severity::Medium,
            message: format!(
                "Rule entries for network ACL {} are not collected yet; security pillar cannot be fully assessed",
                resource.resource_id
            ),
            evidence: json!({ "entries_collected": false }),
        });
        return;
    };

    let is_default = resource
        .resource_data
        .get("is_default")
        .and_then(|v| v.as_bool());

    let allow_all_ingress = entries.iter().find(|entry| {
        let is_ingress = entry.get("egress").and_then(|v| v.as_bool()) == Some(false);
        let is_allow = entry
            .get("rule_action")
            .and_then(|v| v.as_str())
            .map(|s| s.eq_ignore_ascii_case("allow"))
            .unwrap_or(false);
        let open_cidr = entry.get("cidr_block").and_then(|v| v.as_str()) == Some("0.0.0.0/0");
        let all_protocols = match entry.get("protocol") {
            Some(p) => p.as_str() == Some("-1") || p.as_i64() == Some(-1),
            None => false,
        };
        is_ingress && is_allow && open_cidr && all_protocols
    });

    if let Some(entry) = allow_all_ingress {
        let default_note = if is_default == Some(true) {
            "; this is a default NACL, which allows all traffic by design"
        } else {
            ""
        };
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Security,
            reason_code: REASON_SEC_ALLOW_ALL_INGRESS.to_string(),
            severity: Severity::Low,
            message: format!(
                "Network ACL {} has an ingress entry allowing all protocols and ports from 0.0.0.0/0; subnet traffic filtering relies entirely on security groups{}",
                resource.resource_id, default_note
            ),
            evidence: json!({ "entry": entry, "is_default": is_default }),
        });
    }
}

fn evaluate_resilience(_resource: &AwsResourceModel, _findings: &mut Vec<InventoryFinding>) {
    // No honest per-NACL resilience signal exists in the collected fields
    // (network_acl_id, vpc_id, is_default); see the module doc comment.
    // The shared stale-data check is the only resilience-relevant signal.
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
            resource_type: "NetworkAcl".to_string(),
            resource_id: resource_id.to_string(),
            arn: format!(
                "arn:aws:ec2:us-east-1:123456789012:network-acl/{}",
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
            "network_acl_id": "acl-healthy",
            "vpc_id": "vpc-1",
            "is_default": false,
            "associations": [{"subnet_id": "subnet-1"}],
            "entries": [
                {"rule_action": "allow", "cidr_block": "10.0.0.0/16", "protocol": "6", "egress": false},
                {"rule_action": "allow", "cidr_block": "0.0.0.0/0", "protocol": "-1", "egress": true},
            ],
        })
    }

    #[test]
    fn cost_flags_untagged_nacl() {
        let r = fixture("acl-untagged", json!({}), healthy_data(), now());
        let report = evaluate_network_acl_fleet(&[r], Pillar::Cost, now());
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
    fn cost_flags_unassociated_non_default_nacl() {
        let mut data = healthy_data();
        data["associations"] = json!([]);
        let r = fixture("acl-orphan", json!({"team": "net"}), data, now());
        let report = evaluate_network_acl_fleet(&[r], Pillar::Cost, now());
        assert_eq!(
            report
                .findings
                .iter()
                .map(|f| f.reason_code.as_str())
                .collect::<Vec<_>>(),
            vec![REASON_COST_UNASSOCIATED]
        );
    }

    #[test]
    fn cost_does_not_flag_unassociated_default_nacl() {
        let mut data = healthy_data();
        data["associations"] = json!([]);
        data["is_default"] = json!(true);
        let r = fixture("acl-default", json!({"team": "net"}), data, now());
        let report = evaluate_network_acl_fleet(&[r], Pillar::Cost, now());
        assert!(report.findings.is_empty(), "{:?}", report.findings);
    }

    #[test]
    fn security_flags_allow_all_ingress_entry() {
        let mut data = healthy_data();
        data["entries"] = json!([
            {"rule_action": "allow", "cidr_block": "0.0.0.0/0", "protocol": "-1", "egress": false},
        ]);
        data["is_default"] = json!(true);
        let r = fixture("acl-open", json!({"team": "net"}), data, now());
        let report = evaluate_network_acl_fleet(&[r], Pillar::Security, now());
        assert_eq!(report.findings.len(), 1);
        let finding = &report.findings[0];
        assert_eq!(finding.reason_code, REASON_SEC_ALLOW_ALL_INGRESS);
        assert!(matches!(finding.severity, Severity::Low));
        assert!(finding.message.contains("by design"));
    }

    #[test]
    fn security_reports_gap_when_entries_not_collected() {
        // Mirrors what sync_network_acls persists today: no `entries` key.
        let r = fixture(
            "acl-gap",
            json!({"team": "net"}),
            json!({"network_acl_id": "acl-gap", "vpc_id": "vpc-1", "is_default": false}),
            now(),
        );
        let report = evaluate_network_acl_fleet(&[r], Pillar::Security, now());
        assert_eq!(
            report
                .findings
                .iter()
                .map(|f| f.reason_code.as_str())
                .collect::<Vec<_>>(),
            vec![REASON_SEC_ENTRIES_DATA_NOT_COLLECTED]
        );
    }

    #[test]
    fn stale_inventory_is_flagged() {
        let mut r = fixture("acl-stale", json!({"team": "net"}), healthy_data(), now());
        r.last_refreshed = now() - Duration::hours(48);
        let report = evaluate_network_acl_fleet(&[r], Pillar::Resilience, now());
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
    fn skips_non_network_acl_rows() {
        let mut other = fixture("rtb-1", json!({}), json!({}), now());
        other.resource_type = "RouteTable".to_string();
        let nacl = fixture("acl-only", json!({"team": "net"}), healthy_data(), now());
        let report = evaluate_network_acl_fleet(&[other, nacl], Pillar::Cost, now());
        assert_eq!(report.resources_evaluated, 1);
        assert!(report.findings.is_empty(), "{:?}", report.findings);
    }

    #[test]
    fn healthy_nacl_passes_all_pillars() {
        let r = fixture("acl-ok", json!({"team": "net"}), healthy_data(), now());
        for pillar in [Pillar::Cost, Pillar::Security, Pillar::Resilience] {
            let report = evaluate_network_acl_fleet(std::slice::from_ref(&r), pillar, now());
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

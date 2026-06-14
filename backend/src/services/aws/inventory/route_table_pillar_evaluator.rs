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

// Deterministic route table inventory evaluators for the cost, security,
// and resilience pillars (roadmap rows 01-AWS-CLOUD-03088/03097/03124).
//
// Evaluates fields persisted by vpc_control_plane::sync_route_tables:
// route_table_id and vpc_id today. The collector does not yet persist
// `routes` or `associations`; those checks fire only when the arrays are
// present, and the security pillar reports an honest data gap until route
// entries are collected.

use chrono::{DateTime, Utc};
use serde_json::{json, Value};

use crate::models::aws_resource::Model as AwsResourceModel;
use crate::services::aws::inventory::types::{
    check_stale, score_pillar, InventoryFinding, Pillar, PillarReport, Severity,
};

// Reason codes are the stable contract for findings; never reuse or rename.
pub const REASON_COST_NO_TAGS: &str = "ROUTETABLE_COST_NO_TAGS";
pub const REASON_COST_UNASSOCIATED: &str = "ROUTETABLE_COST_UNASSOCIATED";
pub const REASON_SEC_DEFAULT_ROUTE_TO_IGW: &str = "ROUTETABLE_SEC_DEFAULT_ROUTE_TO_IGW";
pub const REASON_SEC_ROUTES_DATA_NOT_COLLECTED: &str = "ROUTETABLE_SEC_ROUTES_DATA_NOT_COLLECTED";
pub const REASON_RES_BLACKHOLE_ROUTE: &str = "ROUTETABLE_RES_BLACKHOLE_ROUTE";
pub const REASON_INV_STALE_DATA: &str = "ROUTETABLE_INV_STALE_DATA";

/// Evaluate every route table in the fleet for one pillar. Rows of other
/// resource types are skipped and not counted.
pub fn evaluate_route_table_fleet(
    resources: &[AwsResourceModel],
    pillar: Pillar,
    now: DateTime<Utc>,
) -> PillarReport {
    let mut findings: Vec<InventoryFinding> = Vec::new();
    let mut stale_resources = 0usize;
    let mut evaluated = 0usize;

    for resource in resources {
        if resource.resource_type != "RouteTable" {
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

/// The collected `routes` array, if the collector has persisted it.
fn collected_routes(resource: &AwsResourceModel) -> Option<&Vec<Value>> {
    resource
        .resource_data
        .get("routes")
        .and_then(|v| v.as_array())
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
                "Route table {} has no tags recorded (untagged resource or tag collection gap); cost allocation cannot be assessed",
                resource.resource_id
            ),
            evidence: json!({ "tags": resource.tags }),
        });
    }

    // Sprawl check fires only when associations are actually collected.
    if let Some(associations) = resource
        .resource_data
        .get("associations")
        .and_then(|v| v.as_array())
    {
        if associations.is_empty() {
            findings.push(InventoryFinding {
                resource_id: resource.resource_id.clone(),
                arn: resource.arn.clone(),
                pillar: Pillar::Cost,
                reason_code: REASON_COST_UNASSOCIATED.to_string(),
                severity: Severity::Low,
                message: format!(
                    "Route table {} has no subnet or gateway associations; it is unused sprawl and a cleanup candidate",
                    resource.resource_id
                ),
                evidence: json!({ "associations": [] }),
            });
        }
    }
}

fn evaluate_security(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    let routes = match collected_routes(resource) {
        Some(routes) => routes,
        None => {
            findings.push(InventoryFinding {
                resource_id: resource.resource_id.clone(),
                arn: resource.arn.clone(),
                pillar: Pillar::Security,
                reason_code: REASON_SEC_ROUTES_DATA_NOT_COLLECTED.to_string(),
                severity: Severity::Medium,
                message: format!(
                    "Route entries for route table {} are not collected yet; internet exposure cannot be assessed",
                    resource.resource_id
                ),
                evidence: json!({ "routes_collected": false }),
            });
            return;
        }
    };

    for route in routes {
        let destination = route
            .get("destination_cidr_block")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let gateway = route
            .get("gateway_id")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        if destination == "0.0.0.0/0" && gateway.starts_with("igw-") {
            findings.push(InventoryFinding {
                resource_id: resource.resource_id.clone(),
                arn: resource.arn.clone(),
                pillar: Pillar::Security,
                reason_code: REASON_SEC_DEFAULT_ROUTE_TO_IGW.to_string(),
                severity: Severity::Low,
                message: format!(
                    "Route table {} routes 0.0.0.0/0 to internet gateway {}; associated subnets are public (expected for public subnets, verify intent)",
                    resource.resource_id, gateway
                ),
                evidence: json!({
                    "destination_cidr_block": destination,
                    "gateway_id": gateway,
                }),
            });
        }
    }
}

fn evaluate_resilience(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    // Route state is only known when route entries are collected; until then
    // the security pillar carries the collection-gap finding.
    let routes = match collected_routes(resource) {
        Some(routes) => routes,
        None => return,
    };

    for route in routes {
        let state = route.get("state").and_then(|v| v.as_str()).unwrap_or("");
        if state.eq_ignore_ascii_case("blackhole") {
            let destination = route
                .get("destination_cidr_block")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");
            findings.push(InventoryFinding {
                resource_id: resource.resource_id.clone(),
                arn: resource.arn.clone(),
                pillar: Pillar::Resilience,
                reason_code: REASON_RES_BLACKHOLE_ROUTE.to_string(),
                severity: Severity::Medium,
                message: format!(
                    "Route table {} has a blackhole route for {}; traffic to that destination is silently dropped",
                    resource.resource_id, destination
                ),
                evidence: json!({
                    "destination_cidr_block": destination,
                    "state": state,
                }),
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
            resource_type: "RouteTable".to_string(),
            resource_id: resource_id.to_string(),
            arn: format!(
                "arn:aws:ec2:us-east-1:123456789012:route-table/{}",
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
            "route_table_id": "rtb-healthy",
            "vpc_id": "vpc-1",
            "routes": [
                { "destination_cidr_block": "10.0.0.0/16", "gateway_id": "local", "state": "active" }
            ],
            "associations": [
                { "subnet_id": "subnet-1", "main": false }
            ],
        })
    }

    #[test]
    fn cost_flags_missing_tags() {
        let r = fixture("rtb-untagged", json!({}), healthy_data(), now());
        let report = evaluate_route_table_fleet(&[r], Pillar::Cost, now());
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
    fn cost_flags_unassociated_when_associations_collected() {
        let mut data = healthy_data();
        data["associations"] = json!([]);
        let r = fixture("rtb-orphan", json!({"team": "net"}), data, now());
        let report = evaluate_route_table_fleet(&[r], Pillar::Cost, now());
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
    fn cost_skips_unassociated_check_when_associations_not_collected() {
        let r = fixture(
            "rtb-min",
            json!({"team": "net"}),
            json!({"route_table_id": "rtb-min", "vpc_id": "vpc-1"}),
            now(),
        );
        let report = evaluate_route_table_fleet(&[r], Pillar::Cost, now());
        assert!(
            report.findings.is_empty(),
            "unexpected: {:?}",
            report.findings
        );
    }

    #[test]
    fn security_flags_default_route_to_internet_gateway() {
        let mut data = healthy_data();
        data["routes"] = json!([
            { "destination_cidr_block": "10.0.0.0/16", "gateway_id": "local", "state": "active" },
            { "destination_cidr_block": "0.0.0.0/0", "gateway_id": "igw-12345", "state": "active" }
        ]);
        let r = fixture("rtb-public", json!({"team": "net"}), data, now());
        let report = evaluate_route_table_fleet(&[r], Pillar::Security, now());
        assert_eq!(
            report
                .findings
                .iter()
                .map(|f| f.reason_code.as_str())
                .collect::<Vec<_>>(),
            vec![REASON_SEC_DEFAULT_ROUTE_TO_IGW]
        );
        assert!(
            report.findings[0].severity == Severity::Low,
            "default route to IGW is informational; severity must stay Low"
        );
    }

    #[test]
    fn security_reports_gap_when_routes_not_collected() {
        let r = fixture(
            "rtb-gap",
            json!({"team": "net"}),
            json!({"route_table_id": "rtb-gap", "vpc_id": "vpc-1"}),
            now(),
        );
        let report = evaluate_route_table_fleet(&[r], Pillar::Security, now());
        assert_eq!(
            report
                .findings
                .iter()
                .map(|f| f.reason_code.as_str())
                .collect::<Vec<_>>(),
            vec![REASON_SEC_ROUTES_DATA_NOT_COLLECTED]
        );
    }

    #[test]
    fn resilience_flags_blackhole_route() {
        let mut data = healthy_data();
        data["routes"] = json!([
            { "destination_cidr_block": "10.1.0.0/16", "gateway_id": "pcx-1", "state": "blackhole" }
        ]);
        let r = fixture("rtb-blackhole", json!({"team": "net"}), data, now());
        let report = evaluate_route_table_fleet(&[r], Pillar::Resilience, now());
        assert_eq!(
            report
                .findings
                .iter()
                .map(|f| f.reason_code.as_str())
                .collect::<Vec<_>>(),
            vec![REASON_RES_BLACKHOLE_ROUTE]
        );
    }

    #[test]
    fn resilience_is_clean_when_routes_not_collected() {
        let r = fixture(
            "rtb-min",
            json!({"team": "net"}),
            json!({"route_table_id": "rtb-min", "vpc_id": "vpc-1"}),
            now(),
        );
        let report = evaluate_route_table_fleet(&[r], Pillar::Resilience, now());
        assert!(
            report.findings.is_empty(),
            "unexpected: {:?}",
            report.findings
        );
    }

    #[test]
    fn stale_inventory_is_reported() {
        let mut r = fixture("rtb-stale", json!({"team": "net"}), healthy_data(), now());
        r.last_refreshed = now() - Duration::hours(48);
        let report = evaluate_route_table_fleet(&[r], Pillar::Cost, now());
        assert_eq!(report.stale_resources, 1);
        assert!(report
            .findings
            .iter()
            .any(|f| f.reason_code == REASON_INV_STALE_DATA));
    }

    #[test]
    fn non_route_table_rows_are_skipped() {
        let mut other = fixture("subnet-1", json!({}), json!({}), now());
        other.resource_type = "Subnet".to_string();
        let rt = fixture("rtb-ok", json!({"team": "net"}), healthy_data(), now());
        let report = evaluate_route_table_fleet(&[other, rt], Pillar::Cost, now());
        assert_eq!(report.resources_evaluated, 1);
        assert!(
            report.findings.is_empty(),
            "unexpected: {:?}",
            report.findings
        );
    }

    #[test]
    fn healthy_route_table_passes_all_pillars() {
        let r = fixture("rtb-ok", json!({"team": "net"}), healthy_data(), now());
        for pillar in [Pillar::Cost, Pillar::Security, Pillar::Resilience] {
            let report = evaluate_route_table_fleet(std::slice::from_ref(&r), pillar, now());
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

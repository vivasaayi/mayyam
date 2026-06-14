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

// Deterministic Global Accelerator inventory evaluators for the cost,
// security, and resilience pillars.
//
// Evaluates fields persisted by globalaccelerator_control_plane: enabled,
// status, ip_address_type, listener_count, listeners_collected,
// flow_logs_enabled, attributes_collected, plus the tags column. A disabled
// accelerator still bills the fixed hourly fee until it is deleted, so
// enabled=false is a cost defect, not a savings.

use chrono::{DateTime, Utc};
use serde_json::json;

use crate::models::aws_resource::Model as AwsResourceModel;
use crate::services::aws::inventory::types::{
    check_stale, data_str, score_pillar, InventoryFinding, Pillar, PillarReport, Severity,
};

/// Only rows of this resource type are evaluated.
pub const RESOURCE_TYPE: &str = "GlobalAccelerator";

// Reason codes are the stable contract for findings; never reuse or rename.
pub const REASON_COST_DISABLED_ACCELERATOR: &str = "GA_COST_DISABLED_ACCELERATOR";
pub const REASON_COST_NO_LISTENERS: &str = "GA_COST_NO_LISTENERS";
pub const REASON_COST_NO_TAGS: &str = "GA_COST_NO_TAGS";
pub const REASON_RES_NOT_DEPLOYED: &str = "GA_RES_NOT_DEPLOYED";
pub const REASON_RES_STATUS_DATA_NOT_COLLECTED: &str = "GA_RES_STATUS_DATA_NOT_COLLECTED";
pub const REASON_RES_SINGLE_IP_FAMILY: &str = "GA_RES_SINGLE_IP_FAMILY";
pub const REASON_RES_LISTENER_DATA_NOT_COLLECTED: &str = "GA_RES_LISTENER_DATA_NOT_COLLECTED";
pub const REASON_SEC_FLOW_LOGS_DISABLED: &str = "GA_SEC_FLOW_LOGS_DISABLED";
pub const REASON_SEC_FLOW_LOGS_DATA_NOT_COLLECTED: &str = "GA_SEC_FLOW_LOGS_DATA_NOT_COLLECTED";
pub const REASON_INV_STALE_DATA: &str = "GA_INV_STALE_DATA";

/// Evaluate every Global Accelerator in the fleet for one pillar. Rows whose
/// `resource_type` is not `GlobalAccelerator` are skipped and not counted.
pub fn evaluate_globalaccelerator_fleet(
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

fn data_bool(resource: &AwsResourceModel, key: &str) -> Option<bool> {
    resource.resource_data.get(key).and_then(|v| v.as_bool())
}

fn listeners_collected(resource: &AwsResourceModel) -> bool {
    data_bool(resource, "listeners_collected") == Some(true)
}

fn evaluate_cost(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    // A disabled accelerator still incurs the fixed hourly fee until it is
    // deleted; disabling does not stop billing.
    if data_bool(resource, "enabled") == Some(false) {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Cost,
            reason_code: REASON_COST_DISABLED_ACCELERATOR.to_string(),
            severity: Severity::Medium,
            message: format!(
                "Global Accelerator {} is disabled but still bills the fixed hourly fee; delete it if it is no longer needed",
                resource.resource_id
            ),
            evidence: json!({ "enabled": false }),
        });
    }

    // Zero listeners means the accelerator can serve no traffic while still
    // billing. Only assert this when listener data was actually collected;
    // the collection gap itself is reported on the resilience pillar.
    if listeners_collected(resource) {
        let listener_count = resource
            .resource_data
            .get("listener_count")
            .and_then(|v| v.as_u64());
        if listener_count == Some(0) {
            findings.push(InventoryFinding {
                resource_id: resource.resource_id.clone(),
                arn: resource.arn.clone(),
                pillar: Pillar::Cost,
                reason_code: REASON_COST_NO_LISTENERS.to_string(),
                severity: Severity::Medium,
                message: format!(
                    "Global Accelerator {} has no listeners; it bills the fixed hourly fee but cannot serve any traffic",
                    resource.resource_id
                ),
                evidence: json!({ "listener_count": 0 }),
            });
        }
    }

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
                "Global Accelerator {} has no tags recorded (untagged resource or tag collection gap); cost allocation cannot be assessed",
                resource.resource_id
            ),
            evidence: json!({ "tags": resource.tags }),
        });
    }
}

fn evaluate_security(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    // Flow logs are the only audit trail for accelerator traffic. The field
    // comes from DescribeAcceleratorAttributes; when that enrichment failed
    // the collector sets attributes_collected=false and omits the field.
    match data_bool(resource, "flow_logs_enabled") {
        Some(false) => {
            findings.push(InventoryFinding {
                resource_id: resource.resource_id.clone(),
                arn: resource.arn.clone(),
                pillar: Pillar::Security,
                reason_code: REASON_SEC_FLOW_LOGS_DISABLED.to_string(),
                severity: Severity::Medium,
                message: format!(
                    "Global Accelerator {} has flow logs disabled; there is no audit evidence of accepted traffic",
                    resource.resource_id
                ),
                evidence: json!({ "flow_logs_enabled": false }),
            });
        }
        None => {
            findings.push(InventoryFinding {
                resource_id: resource.resource_id.clone(),
                arn: resource.arn.clone(),
                pillar: Pillar::Security,
                reason_code: REASON_SEC_FLOW_LOGS_DATA_NOT_COLLECTED.to_string(),
                severity: Severity::Low,
                message: format!(
                    "Flow log attributes for Global Accelerator {} are not collected yet; security pillar cannot be fully assessed",
                    resource.resource_id
                ),
                evidence: json!({
                    "attributes_collected": data_bool(resource, "attributes_collected"),
                    "flow_logs_enabled_collected": false,
                }),
            });
        }
        Some(true) => {}
    }
}

fn evaluate_resilience(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    match data_str(&resource.resource_data, "status") {
        Some(status) => {
            if status != "DEPLOYED" {
                findings.push(InventoryFinding {
                    resource_id: resource.resource_id.clone(),
                    arn: resource.arn.clone(),
                    pillar: Pillar::Resilience,
                    reason_code: REASON_RES_NOT_DEPLOYED.to_string(),
                    severity: Severity::Medium,
                    message: format!(
                        "Global Accelerator {} is in state {} (not DEPLOYED); recent configuration changes are not fully propagated",
                        resource.resource_id, status
                    ),
                    evidence: json!({ "status": status }),
                });
            }
        }
        None => {
            findings.push(InventoryFinding {
                resource_id: resource.resource_id.clone(),
                arn: resource.arn.clone(),
                pillar: Pillar::Resilience,
                reason_code: REASON_RES_STATUS_DATA_NOT_COLLECTED.to_string(),
                severity: Severity::Low,
                message: format!(
                    "Deployment status for Global Accelerator {} is not collected yet; resilience pillar cannot be fully assessed",
                    resource.resource_id
                ),
                evidence: json!({ "status_collected": false }),
            });
        }
    }

    // Informational: dual-stack is available; an IPv4-only accelerator cannot
    // serve IPv6-only clients directly.
    if let Some(ip_address_type) = data_str(&resource.resource_data, "ip_address_type") {
        if ip_address_type == "IPV4" {
            findings.push(InventoryFinding {
                resource_id: resource.resource_id.clone(),
                arn: resource.arn.clone(),
                pillar: Pillar::Resilience,
                reason_code: REASON_RES_SINGLE_IP_FAMILY.to_string(),
                severity: Severity::Low,
                message: format!(
                    "Global Accelerator {} is IPv4-only; dual-stack (DUAL_STACK) is available and serves IPv6 clients as well",
                    resource.resource_id
                ),
                evidence: json!({ "ip_address_type": ip_address_type }),
            });
        }
    }

    if !listeners_collected(resource) {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Resilience,
            reason_code: REASON_RES_LISTENER_DATA_NOT_COLLECTED.to_string(),
            severity: Severity::Low,
            message: format!(
                "Listener data for Global Accelerator {} is not collected yet; traffic-serving capability cannot be assessed",
                resource.resource_id
            ),
            evidence: json!({ "listeners_collected": data_bool(resource, "listeners_collected") }),
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
            region: "us-west-2".to_string(),
            resource_type: RESOURCE_TYPE.to_string(),
            resource_id: resource_id.to_string(),
            arn: format!(
                "arn:aws:globalaccelerator::123456789012:accelerator/{}",
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
            "accelerator_arn": "arn:aws:globalaccelerator::123456789012:accelerator/ga-ok",
            "name": "ga-ok",
            "ip_address_type": "DUAL_STACK",
            "enabled": true,
            "status": "DEPLOYED",
            "dns_name": "a1234567890abcdef.awsglobalaccelerator.com",
            "dual_stack_dns_name": "a1234567890abcdef.dualstack.awsglobalaccelerator.com",
            "created_time": "2025-01-01T00:00:00Z",
            "last_modified_time": "2025-06-01T00:00:00Z",
            "ip_sets": [
                { "ip_family": "IPv4", "ip_address_count": 2 },
                { "ip_family": "IPv6", "ip_address_count": 2 }
            ],
            "attributes_collected": true,
            "flow_logs_enabled": true,
            "flow_logs_s3_bucket": "ga-flow-logs",
            "flow_logs_s3_prefix": "prod/",
            "listeners_collected": true,
            "listener_count": 1,
            "listeners": [
                {
                    "listener_arn": "arn:aws:globalaccelerator::123456789012:accelerator/ga-ok/listener/l-1",
                    "protocol": "TCP",
                    "client_affinity": "NONE",
                    "port_ranges": [{ "from_port": 443, "to_port": 443 }]
                }
            ],
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
    fn cost_flags_disabled_accelerator_still_billed() {
        let mut data = healthy_data();
        data["enabled"] = json!(false);
        let r = fixture("ga-disabled", json!({"team": "edge"}), data, now());
        let report = evaluate_globalaccelerator_fleet(&[r], Pillar::Cost, now());
        assert_eq!(codes(&report), vec![REASON_COST_DISABLED_ACCELERATOR]);
        assert!(report.findings[0].message.contains("fixed hourly fee"));
    }

    #[test]
    fn cost_flags_accelerator_with_zero_listeners() {
        let mut data = healthy_data();
        data["listener_count"] = json!(0);
        data["listeners"] = json!([]);
        let r = fixture("ga-idle", json!({"team": "edge"}), data, now());
        let report = evaluate_globalaccelerator_fleet(&[r], Pillar::Cost, now());
        assert_eq!(codes(&report), vec![REASON_COST_NO_LISTENERS]);
    }

    #[test]
    fn cost_skips_zero_listener_check_when_listener_data_not_collected() {
        let mut data = healthy_data();
        data["listeners_collected"] = json!(false);
        data.as_object_mut().unwrap().remove("listener_count");
        data.as_object_mut().unwrap().remove("listeners");
        let r = fixture("ga-listgap", json!({"team": "edge"}), data, now());
        let report = evaluate_globalaccelerator_fleet(&[r], Pillar::Cost, now());
        assert!(
            report.findings.is_empty(),
            "unexpected: {:?}",
            report.findings
        );
    }

    #[test]
    fn cost_flags_untagged_accelerator() {
        let r = fixture("ga-untagged", json!({}), healthy_data(), now());
        let report = evaluate_globalaccelerator_fleet(&[r], Pillar::Cost, now());
        assert_eq!(codes(&report), vec![REASON_COST_NO_TAGS]);
    }

    #[test]
    fn security_flags_flow_logs_disabled() {
        let mut data = healthy_data();
        data["flow_logs_enabled"] = json!(false);
        let r = fixture("ga-nologs", json!({"team": "edge"}), data, now());
        let report = evaluate_globalaccelerator_fleet(&[r], Pillar::Security, now());
        assert_eq!(codes(&report), vec![REASON_SEC_FLOW_LOGS_DISABLED]);
        assert!(matches!(report.findings[0].severity, Severity::Medium));
    }

    #[test]
    fn security_reports_gap_when_attributes_not_collected() {
        let mut data = healthy_data();
        data["attributes_collected"] = json!(false);
        data.as_object_mut().unwrap().remove("flow_logs_enabled");
        data.as_object_mut().unwrap().remove("flow_logs_s3_bucket");
        data.as_object_mut().unwrap().remove("flow_logs_s3_prefix");
        let r = fixture("ga-attrgap", json!({"team": "edge"}), data, now());
        let report = evaluate_globalaccelerator_fleet(&[r], Pillar::Security, now());
        assert_eq!(
            codes(&report),
            vec![REASON_SEC_FLOW_LOGS_DATA_NOT_COLLECTED]
        );
        assert!(matches!(report.findings[0].severity, Severity::Low));
    }

    #[test]
    fn resilience_flags_in_progress_status() {
        let mut data = healthy_data();
        data["status"] = json!("IN_PROGRESS");
        let r = fixture("ga-deploying", json!({"team": "edge"}), data, now());
        let report = evaluate_globalaccelerator_fleet(&[r], Pillar::Resilience, now());
        assert_eq!(codes(&report), vec![REASON_RES_NOT_DEPLOYED]);
        assert!(report.findings[0].message.contains("IN_PROGRESS"));
    }

    #[test]
    fn resilience_reports_gap_when_status_not_collected() {
        let mut data = healthy_data();
        data.as_object_mut().unwrap().remove("status");
        let r = fixture("ga-statusgap", json!({"team": "edge"}), data, now());
        let report = evaluate_globalaccelerator_fleet(&[r], Pillar::Resilience, now());
        assert_eq!(codes(&report), vec![REASON_RES_STATUS_DATA_NOT_COLLECTED]);
    }

    #[test]
    fn resilience_flags_ipv4_only_as_informational_low() {
        let mut data = healthy_data();
        data["ip_address_type"] = json!("IPV4");
        let r = fixture("ga-ipv4", json!({"team": "edge"}), data, now());
        let report = evaluate_globalaccelerator_fleet(&[r], Pillar::Resilience, now());
        assert_eq!(codes(&report), vec![REASON_RES_SINGLE_IP_FAMILY]);
        assert!(matches!(report.findings[0].severity, Severity::Low));
    }

    #[test]
    fn resilience_reports_gap_when_listener_data_not_collected() {
        let mut data = healthy_data();
        data["listeners_collected"] = json!(false);
        data.as_object_mut().unwrap().remove("listener_count");
        data.as_object_mut().unwrap().remove("listeners");
        let r = fixture("ga-listgap", json!({"team": "edge"}), data, now());
        let report = evaluate_globalaccelerator_fleet(&[r], Pillar::Resilience, now());
        assert_eq!(codes(&report), vec![REASON_RES_LISTENER_DATA_NOT_COLLECTED]);
    }

    #[test]
    fn stale_inventory_is_flagged() {
        let mut r = fixture("ga-stale", json!({"team": "edge"}), healthy_data(), now());
        r.last_refreshed = now() - Duration::hours(48);
        let report = evaluate_globalaccelerator_fleet(&[r], Pillar::Resilience, now());
        assert_eq!(report.stale_resources, 1);
        assert!(codes(&report).contains(&REASON_INV_STALE_DATA));
    }

    #[test]
    fn non_globalaccelerator_resources_are_skipped_and_not_counted() {
        let mut r = fixture("queue-1", json!({}), json!({}), now());
        r.resource_type = "SqsQueue".to_string();
        let report = evaluate_globalaccelerator_fleet(&[r], Pillar::Cost, now());
        assert_eq!(report.resources_evaluated, 0);
        assert!(report.findings.is_empty());
    }

    #[test]
    fn healthy_accelerator_passes_all_pillars() {
        let r = fixture("ga-ok", json!({"team": "edge"}), healthy_data(), now());
        for pillar in [Pillar::Cost, Pillar::Security, Pillar::Resilience] {
            let report = evaluate_globalaccelerator_fleet(std::slice::from_ref(&r), pillar, now());
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

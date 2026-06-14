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

// Deterministic Storage Gateway inventory evaluators for the cost, security,
// and resilience pillars (roadmap rows 01-AWS-CLOUD-01009/01018/01045).
//
// Evaluates fields persisted by storagegateway_control_plane: GatewayId,
// GatewayName, GatewayARN, GatewayType, GatewayOperationalState. The
// collector does not persist tags or security posture fields yet, so those
// gaps are reported honestly instead of being guessed.

use chrono::{DateTime, Utc};
use serde_json::json;

use crate::models::aws_resource::Model as AwsResourceModel;
use crate::services::aws::inventory::types::{
    check_stale, score_pillar, InventoryFinding, Pillar, PillarReport, Severity,
};

const GATEWAY_RESOURCE_TYPE: &str = "StorageGateway";

// Reason codes are the stable contract for findings; never reuse or rename.
pub const REASON_COST_NO_TAGS: &str = "STORAGEGATEWAY_COST_NO_TAGS";
pub const REASON_SEC_POSTURE_DATA_NOT_COLLECTED: &str =
    "STORAGEGATEWAY_SEC_POSTURE_DATA_NOT_COLLECTED";
pub const REASON_RES_GATEWAY_NOT_ACTIVE: &str = "STORAGEGATEWAY_RES_GATEWAY_NOT_ACTIVE";
pub const REASON_RES_OPERATIONAL_STATE_DATA_NOT_COLLECTED: &str =
    "STORAGEGATEWAY_RES_OPERATIONAL_STATE_DATA_NOT_COLLECTED";
pub const REASON_INV_STALE_DATA: &str = "STORAGEGATEWAY_INV_STALE_DATA";

/// Evaluate every Storage Gateway in the fleet for one pillar.
pub fn evaluate_storagegateway_fleet(
    resources: &[AwsResourceModel],
    pillar: Pillar,
    now: DateTime<Utc>,
) -> PillarReport {
    let mut findings: Vec<InventoryFinding> = Vec::new();
    let mut stale_resources = 0usize;
    let mut evaluated = 0usize;

    for resource in resources {
        // Other resource types may share a sync batch; skip them gracefully.
        if resource.resource_type != GATEWAY_RESOURCE_TYPE {
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
                "Gateway {} has no tags recorded (untagged resource or tag collection gap); cost allocation cannot be assessed",
                resource.resource_id
            ),
            evidence: json!({ "tags": resource.tags }),
        });
    }
}

fn evaluate_security(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    // The collector persists no security posture fields (no CHAP, SMB
    // security strategy, or endpoint configuration), so report the gap
    // honestly instead of inventing a verdict.
    findings.push(InventoryFinding {
        resource_id: resource.resource_id.clone(),
        arn: resource.arn.clone(),
        pillar: Pillar::Security,
        reason_code: REASON_SEC_POSTURE_DATA_NOT_COLLECTED.to_string(),
        severity: Severity::Medium,
        message: format!(
            "Security posture for gateway {} is not collected yet (no CHAP, SMB security, or endpoint fields); security pillar cannot be assessed",
            resource.resource_id
        ),
        evidence: json!({ "security_fields_collected": false }),
    });
}

fn evaluate_resilience(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    let state = resource
        .resource_data
        .get("GatewayOperationalState")
        .and_then(|v| v.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());

    match state {
        None => {
            findings.push(InventoryFinding {
                resource_id: resource.resource_id.clone(),
                arn: resource.arn.clone(),
                pillar: Pillar::Resilience,
                reason_code: REASON_RES_OPERATIONAL_STATE_DATA_NOT_COLLECTED.to_string(),
                severity: Severity::Medium,
                message: format!(
                    "Operational state for gateway {} is not collected yet; resilience pillar cannot be fully assessed",
                    resource.resource_id
                ),
                evidence: json!({ "operational_state_collected": false }),
            });
        }
        Some(state) => {
            let normalized = state.to_ascii_lowercase();
            if normalized != "active" && normalized != "running" {
                findings.push(InventoryFinding {
                    resource_id: resource.resource_id.clone(),
                    arn: resource.arn.clone(),
                    pillar: Pillar::Resilience,
                    reason_code: REASON_RES_GATEWAY_NOT_ACTIVE.to_string(),
                    severity: Severity::High,
                    message: format!(
                        "Gateway {} is in operational state '{}' (not active/running); attached file shares and volumes may be unavailable",
                        resource.resource_id, state
                    ),
                    evidence: json!({ "GatewayOperationalState": state }),
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
            resource_type: "StorageGateway".to_string(),
            resource_id: resource_id.to_string(),
            arn: format!(
                "arn:aws:storagegateway:us-east-1:123456789012:gateway/{}",
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
            "GatewayId": "sgw-12A3456B",
            "GatewayName": "backup-gw",
            "GatewayARN": "arn:aws:storagegateway:us-east-1:123456789012:gateway/sgw-12A3456B",
            "GatewayType": "FILE_S3",
            "GatewayOperationalState": "ACTIVE",
        })
    }

    #[test]
    fn cost_flags_untagged_gateway() {
        let r = fixture("sgw-untagged", json!({}), healthy_data(), now());
        let report = evaluate_storagegateway_fleet(&[r], Pillar::Cost, now());
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
    fn security_reports_posture_data_gap() {
        let r = fixture("sgw-sec", json!({"team": "storage"}), healthy_data(), now());
        let report = evaluate_storagegateway_fleet(&[r], Pillar::Security, now());
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
    fn resilience_flags_gateway_not_active() {
        let mut data = healthy_data();
        data["GatewayOperationalState"] = json!("DISABLED");
        let r = fixture("sgw-down", json!({"team": "storage"}), data, now());
        let report = evaluate_storagegateway_fleet(&[r], Pillar::Resilience, now());
        assert_eq!(
            report
                .findings
                .iter()
                .map(|f| f.reason_code.as_str())
                .collect::<Vec<_>>(),
            vec![REASON_RES_GATEWAY_NOT_ACTIVE]
        );
    }

    #[test]
    fn resilience_reports_gap_when_state_not_collected() {
        let mut data = healthy_data();
        data["GatewayOperationalState"] = json!("");
        let r = fixture("sgw-gap", json!({"team": "storage"}), data, now());
        let report = evaluate_storagegateway_fleet(&[r], Pillar::Resilience, now());
        assert_eq!(
            report
                .findings
                .iter()
                .map(|f| f.reason_code.as_str())
                .collect::<Vec<_>>(),
            vec![REASON_RES_OPERATIONAL_STATE_DATA_NOT_COLLECTED]
        );
    }

    #[test]
    fn stale_inventory_is_flagged() {
        let mut r = fixture(
            "sgw-stale",
            json!({"team": "storage"}),
            healthy_data(),
            now(),
        );
        r.last_refreshed = now() - Duration::hours(48);
        let report = evaluate_storagegateway_fleet(&[r], Pillar::Resilience, now());
        assert_eq!(report.stale_resources, 1);
        assert!(report
            .findings
            .iter()
            .any(|f| f.reason_code == REASON_INV_STALE_DATA));
    }

    #[test]
    fn non_storagegateway_rows_are_skipped() {
        let mut r = fixture("vol-other", json!({}), json!({}), now());
        r.resource_type = "StorageGatewayVolume".to_string();
        let report = evaluate_storagegateway_fleet(&[r], Pillar::Cost, now());
        assert_eq!(report.resources_evaluated, 0);
        assert!(report.findings.is_empty());
    }

    #[test]
    fn healthy_gateway_passes_cost_and_resilience() {
        let r = fixture("sgw-ok", json!({"team": "storage"}), healthy_data(), now());
        for pillar in [Pillar::Cost, Pillar::Resilience] {
            let report = evaluate_storagegateway_fleet(std::slice::from_ref(&r), pillar, now());
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

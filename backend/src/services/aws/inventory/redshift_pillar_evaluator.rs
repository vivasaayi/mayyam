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

// Deterministic Redshift cluster inventory evaluators for the cost,
// security, and resilience pillars.
//
// Evaluates fields persisted by redshift_control_plane: node_type,
// number_of_nodes, cluster_status, cluster_availability_status,
// publicly_accessible, encrypted, kms_key_id, enhanced_vpc_routing,
// automated_snapshot_retention_period, allow_version_upgrade, plus the
// tags column.

use chrono::{DateTime, Utc};
use serde_json::json;

use crate::models::aws_resource::Model as AwsResourceModel;
use crate::services::aws::inventory::types::{
    check_stale, data_str, score_pillar, InventoryFinding, Pillar, PillarReport, Severity,
};

/// Only rows of this resource type are evaluated.
pub const RESOURCE_TYPE: &str = "RedshiftCluster";

// Reason codes are the stable contract for findings; never reuse or rename.
pub const REASON_COST_NON_RA3_NODE_TYPE: &str = "REDSHIFT_COST_NON_RA3_NODE_TYPE";
pub const REASON_COST_NODE_TYPE_DATA_NOT_COLLECTED: &str =
    "REDSHIFT_COST_NODE_TYPE_DATA_NOT_COLLECTED";
pub const REASON_COST_NO_TAGS: &str = "REDSHIFT_COST_NO_TAGS";
pub const REASON_COST_PAUSED_CLUSTER: &str = "REDSHIFT_COST_PAUSED_CLUSTER";
pub const REASON_RES_SINGLE_NODE: &str = "REDSHIFT_RES_SINGLE_NODE";
pub const REASON_RES_NODE_COUNT_DATA_NOT_COLLECTED: &str =
    "REDSHIFT_RES_NODE_COUNT_DATA_NOT_COLLECTED";
pub const REASON_RES_NO_AUTOMATED_SNAPSHOTS: &str = "REDSHIFT_RES_NO_AUTOMATED_SNAPSHOTS";
pub const REASON_RES_SNAPSHOT_DATA_NOT_COLLECTED: &str = "REDSHIFT_RES_SNAPSHOT_DATA_NOT_COLLECTED";
pub const REASON_RES_VERSION_UPGRADE_DISABLED: &str = "REDSHIFT_RES_VERSION_UPGRADE_DISABLED";
pub const REASON_RES_CLUSTER_UNAVAILABLE: &str = "REDSHIFT_RES_CLUSTER_UNAVAILABLE";
pub const REASON_SEC_PUBLICLY_ACCESSIBLE: &str = "REDSHIFT_SEC_PUBLICLY_ACCESSIBLE";
pub const REASON_SEC_NOT_ENCRYPTED: &str = "REDSHIFT_SEC_NOT_ENCRYPTED";
pub const REASON_SEC_ENCRYPTION_DATA_NOT_COLLECTED: &str =
    "REDSHIFT_SEC_ENCRYPTION_DATA_NOT_COLLECTED";
pub const REASON_SEC_DEFAULT_KMS_KEY: &str = "REDSHIFT_SEC_DEFAULT_KMS_KEY";
pub const REASON_SEC_ENHANCED_VPC_ROUTING_DISABLED: &str =
    "REDSHIFT_SEC_ENHANCED_VPC_ROUTING_DISABLED";
pub const REASON_INV_STALE_DATA: &str = "REDSHIFT_INV_STALE_DATA";

/// Evaluate every Redshift cluster in the fleet for one pillar. Rows whose
/// `resource_type` is not `RedshiftCluster` are skipped and not counted.
pub fn evaluate_redshift_fleet(
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

fn data_i64(resource: &AwsResourceModel, key: &str) -> Option<i64> {
    resource.resource_data.get(key).and_then(|v| v.as_i64())
}

fn evaluate_cost(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    // RA3 nodes separate compute from managed storage and are the current
    // generation; dc2/ds2 clusters are modernization and cost candidates.
    match data_str(&resource.resource_data, "node_type") {
        Some(node_type) => {
            if !node_type.to_ascii_lowercase().starts_with("ra3") {
                findings.push(InventoryFinding {
                    resource_id: resource.resource_id.clone(),
                    arn: resource.arn.clone(),
                    pillar: Pillar::Cost,
                    reason_code: REASON_COST_NON_RA3_NODE_TYPE.to_string(),
                    severity: Severity::Medium,
                    message: format!(
                        "Cluster {} runs previous-generation node type {}; migrating to RA3 separates compute from storage and usually lowers cost",
                        resource.resource_id, node_type
                    ),
                    evidence: json!({ "node_type": node_type }),
                });
            }
        }
        None => {
            findings.push(InventoryFinding {
                resource_id: resource.resource_id.clone(),
                arn: resource.arn.clone(),
                pillar: Pillar::Cost,
                reason_code: REASON_COST_NODE_TYPE_DATA_NOT_COLLECTED.to_string(),
                severity: Severity::Low,
                message: format!(
                    "Node type for cluster {} is not collected yet; node generation cost posture cannot be assessed",
                    resource.resource_id
                ),
                evidence: json!({ "node_type_collected": false }),
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
                "Cluster {} has no tags recorded (untagged resource or tag collection gap); cost allocation cannot be assessed",
                resource.resource_id
            ),
            evidence: json!({ "tags": resource.tags }),
        });
    }

    // A paused cluster stops compute billing but still bills for storage;
    // long-lived paused clusters are deletion or snapshot-and-restore candidates.
    let status = data_str(&resource.resource_data, "cluster_status");
    let availability = data_str(&resource.resource_data, "cluster_availability_status");
    let paused = status.as_deref().map(str::to_ascii_lowercase).as_deref() == Some("paused")
        || availability
            .as_deref()
            .map(str::to_ascii_lowercase)
            .as_deref()
            == Some("paused");
    if paused {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Cost,
            reason_code: REASON_COST_PAUSED_CLUSTER.to_string(),
            severity: Severity::Low,
            message: format!(
                "Cluster {} is paused but still provisioned and billing for storage; delete it or snapshot-and-restore if it is no longer needed",
                resource.resource_id
            ),
            evidence: json!({
                "cluster_status": status,
                "cluster_availability_status": availability,
            }),
        });
    }
}

fn evaluate_resilience(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    match data_i64(resource, "number_of_nodes") {
        Some(1) => {
            findings.push(InventoryFinding {
                resource_id: resource.resource_id.clone(),
                arn: resource.arn.clone(),
                pillar: Pillar::Resilience,
                reason_code: REASON_RES_SINGLE_NODE.to_string(),
                severity: Severity::Medium,
                message: format!(
                    "Cluster {} is single-node; a node failure has no failover path and data restore depends entirely on snapshots",
                    resource.resource_id
                ),
                evidence: json!({ "number_of_nodes": 1 }),
            });
        }
        Some(_) => {}
        None => {
            findings.push(InventoryFinding {
                resource_id: resource.resource_id.clone(),
                arn: resource.arn.clone(),
                pillar: Pillar::Resilience,
                reason_code: REASON_RES_NODE_COUNT_DATA_NOT_COLLECTED.to_string(),
                severity: Severity::Low,
                message: format!(
                    "Node count for cluster {} is not collected yet; failover posture cannot be assessed",
                    resource.resource_id
                ),
                evidence: json!({ "number_of_nodes_collected": false }),
            });
        }
    }

    match data_i64(resource, "automated_snapshot_retention_period") {
        Some(0) => {
            findings.push(InventoryFinding {
                resource_id: resource.resource_id.clone(),
                arn: resource.arn.clone(),
                pillar: Pillar::Resilience,
                reason_code: REASON_RES_NO_AUTOMATED_SNAPSHOTS.to_string(),
                severity: Severity::High,
                message: format!(
                    "Cluster {} has automated snapshots disabled (retention period 0); there is no automated point-in-time recovery",
                    resource.resource_id
                ),
                evidence: json!({ "automated_snapshot_retention_period": 0 }),
            });
        }
        Some(_) => {}
        None => {
            findings.push(InventoryFinding {
                resource_id: resource.resource_id.clone(),
                arn: resource.arn.clone(),
                pillar: Pillar::Resilience,
                reason_code: REASON_RES_SNAPSHOT_DATA_NOT_COLLECTED.to_string(),
                severity: Severity::Low,
                message: format!(
                    "Automated snapshot retention for cluster {} is not collected yet; backup posture cannot be assessed",
                    resource.resource_id
                ),
                evidence: json!({ "automated_snapshot_retention_period_collected": false }),
            });
        }
    }

    if data_bool(resource, "allow_version_upgrade") == Some(false) {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Resilience,
            reason_code: REASON_RES_VERSION_UPGRADE_DISABLED.to_string(),
            severity: Severity::Low,
            message: format!(
                "Cluster {} has automatic version upgrades disabled; it will drift behind engine fixes applied during maintenance windows",
                resource.resource_id
            ),
            evidence: json!({ "allow_version_upgrade": false }),
        });
    }

    // "Paused" is an intentional operator action covered by the cost pillar;
    // Maintenance/Modifying are transient. Only Unavailable/Failed are
    // resilience defects.
    let availability = data_str(&resource.resource_data, "cluster_availability_status");
    if let Some(state) = availability.as_deref() {
        if state.eq_ignore_ascii_case("unavailable") || state.eq_ignore_ascii_case("failed") {
            findings.push(InventoryFinding {
                resource_id: resource.resource_id.clone(),
                arn: resource.arn.clone(),
                pillar: Pillar::Resilience,
                reason_code: REASON_RES_CLUSTER_UNAVAILABLE.to_string(),
                severity: Severity::High,
                message: format!(
                    "Cluster {} availability status is {}; queries against it will fail",
                    resource.resource_id, state
                ),
                evidence: json!({ "cluster_availability_status": state }),
            });
        }
    }
}

fn evaluate_security(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    if data_bool(resource, "publicly_accessible") == Some(true) {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Security,
            reason_code: REASON_SEC_PUBLICLY_ACCESSIBLE.to_string(),
            severity: Severity::High,
            message: format!(
                "Cluster {} is publicly accessible; its endpoint is reachable from the internet",
                resource.resource_id
            ),
            evidence: json!({ "publicly_accessible": true }),
        });
    }

    match data_bool(resource, "encrypted") {
        Some(false) => {
            findings.push(InventoryFinding {
                resource_id: resource.resource_id.clone(),
                arn: resource.arn.clone(),
                pillar: Pillar::Security,
                reason_code: REASON_SEC_NOT_ENCRYPTED.to_string(),
                severity: Severity::High,
                message: format!("Cluster {} is not encrypted at rest", resource.resource_id),
                evidence: json!({ "encrypted": false }),
            });
        }
        Some(true) => {
            // Encrypted but no KMS key id recorded means the AWS-owned default
            // key is in use; customer-managed keys give auditable control.
            let kms_key_id =
                data_str(&resource.resource_data, "kms_key_id").filter(|k| !k.is_empty());
            if kms_key_id.is_none() {
                findings.push(InventoryFinding {
                    resource_id: resource.resource_id.clone(),
                    arn: resource.arn.clone(),
                    pillar: Pillar::Security,
                    reason_code: REASON_SEC_DEFAULT_KMS_KEY.to_string(),
                    severity: Severity::Low,
                    message: format!(
                        "Cluster {} is encrypted without a KMS key id recorded (default key); a customer-managed key gives auditable key control",
                        resource.resource_id
                    ),
                    evidence: json!({ "encrypted": true, "kms_key_id": null }),
                });
            }
        }
        None => {
            findings.push(InventoryFinding {
                resource_id: resource.resource_id.clone(),
                arn: resource.arn.clone(),
                pillar: Pillar::Security,
                reason_code: REASON_SEC_ENCRYPTION_DATA_NOT_COLLECTED.to_string(),
                severity: Severity::Low,
                message: format!(
                    "Encryption status for cluster {} is not collected yet; security pillar cannot be fully assessed",
                    resource.resource_id
                ),
                evidence: json!({ "encrypted_collected": false }),
            });
        }
    }

    if data_bool(resource, "enhanced_vpc_routing") == Some(false) {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Security,
            reason_code: REASON_SEC_ENHANCED_VPC_ROUTING_DISABLED.to_string(),
            severity: Severity::Low,
            message: format!(
                "Cluster {} has enhanced VPC routing disabled; COPY/UNLOAD traffic bypasses VPC network controls",
                resource.resource_id
            ),
            evidence: json!({ "enhanced_vpc_routing": false }),
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
            resource_type: RESOURCE_TYPE.to_string(),
            resource_id: resource_id.to_string(),
            arn: format!(
                "arn:aws:redshift:us-east-1:123456789012:cluster:{}",
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
            "cluster_identifier": "warehouse-1",
            "node_type": "ra3.xlplus",
            "number_of_nodes": 2,
            "cluster_status": "available",
            "cluster_availability_status": "Available",
            "availability_zone": "us-east-1a",
            "publicly_accessible": false,
            "encrypted": true,
            "kms_key_id": "arn:aws:kms:us-east-1:123456789012:key/abc",
            "enhanced_vpc_routing": true,
            "automated_snapshot_retention_period": 7,
            "manual_snapshot_retention_period": 30,
            "cluster_version": "1.0",
            "allow_version_upgrade": true,
            "maintenance_track_name": "current",
            "has_pending_modified_values": false,
            "db_name": "analytics",
            "vpc_id": "vpc-1",
            "cluster_subnet_group_name": "default",
        })
    }

    fn ok_tags() -> Value {
        json!({"team": "data"})
    }

    fn codes(report: &PillarReport) -> Vec<&str> {
        report
            .findings
            .iter()
            .map(|f| f.reason_code.as_str())
            .collect()
    }

    #[test]
    fn healthy_cluster_passes_all_pillars() {
        let r = fixture("warehouse-1", ok_tags(), healthy_data(), now());
        for pillar in [Pillar::Cost, Pillar::Security, Pillar::Resilience] {
            let report = evaluate_redshift_fleet(std::slice::from_ref(&r), pillar, now());
            assert!(
                report.findings.is_empty(),
                "unexpected for {:?}: {:?}",
                pillar,
                report.findings
            );
            assert_eq!(report.score, 100);
        }
    }

    #[test]
    fn cost_flags_non_ra3_node_type() {
        let mut data = healthy_data();
        data["node_type"] = json!("dc2.large");
        let r = fixture("warehouse-dc2", ok_tags(), data, now());
        let report = evaluate_redshift_fleet(&[r], Pillar::Cost, now());
        assert_eq!(codes(&report), vec![REASON_COST_NON_RA3_NODE_TYPE]);
        assert!(matches!(report.findings[0].severity, Severity::Medium));
    }

    #[test]
    fn cost_reports_gap_when_node_type_not_collected() {
        let mut data = healthy_data();
        data.as_object_mut().unwrap().remove("node_type");
        let r = fixture("warehouse-nogen", ok_tags(), data, now());
        let report = evaluate_redshift_fleet(&[r], Pillar::Cost, now());
        assert_eq!(
            codes(&report),
            vec![REASON_COST_NODE_TYPE_DATA_NOT_COLLECTED]
        );
        assert!(matches!(report.findings[0].severity, Severity::Low));
    }

    #[test]
    fn cost_flags_untagged_cluster() {
        let r = fixture("warehouse-untagged", json!({}), healthy_data(), now());
        let report = evaluate_redshift_fleet(&[r], Pillar::Cost, now());
        assert_eq!(codes(&report), vec![REASON_COST_NO_TAGS]);
    }

    #[test]
    fn cost_flags_paused_cluster_still_provisioned() {
        let mut data = healthy_data();
        data["cluster_status"] = json!("paused");
        data["cluster_availability_status"] = json!("Paused");
        let r = fixture("warehouse-paused", ok_tags(), data, now());
        let report = evaluate_redshift_fleet(&[r], Pillar::Cost, now());
        assert_eq!(codes(&report), vec![REASON_COST_PAUSED_CLUSTER]);
    }

    #[test]
    fn resilience_flags_single_node_cluster() {
        let mut data = healthy_data();
        data["number_of_nodes"] = json!(1);
        let r = fixture("warehouse-single", ok_tags(), data, now());
        let report = evaluate_redshift_fleet(&[r], Pillar::Resilience, now());
        assert_eq!(codes(&report), vec![REASON_RES_SINGLE_NODE]);
    }

    #[test]
    fn resilience_reports_gap_when_node_count_not_collected() {
        let mut data = healthy_data();
        data.as_object_mut().unwrap().remove("number_of_nodes");
        let r = fixture("warehouse-nocount", ok_tags(), data, now());
        let report = evaluate_redshift_fleet(&[r], Pillar::Resilience, now());
        assert_eq!(
            codes(&report),
            vec![REASON_RES_NODE_COUNT_DATA_NOT_COLLECTED]
        );
    }

    #[test]
    fn resilience_flags_disabled_automated_snapshots_as_high() {
        let mut data = healthy_data();
        data["automated_snapshot_retention_period"] = json!(0);
        let r = fixture("warehouse-nobackup", ok_tags(), data, now());
        let report = evaluate_redshift_fleet(&[r], Pillar::Resilience, now());
        assert_eq!(codes(&report), vec![REASON_RES_NO_AUTOMATED_SNAPSHOTS]);
        assert!(matches!(report.findings[0].severity, Severity::High));
    }

    #[test]
    fn resilience_reports_gap_when_snapshot_retention_not_collected() {
        let mut data = healthy_data();
        data.as_object_mut()
            .unwrap()
            .remove("automated_snapshot_retention_period");
        let r = fixture("warehouse-snapgap", ok_tags(), data, now());
        let report = evaluate_redshift_fleet(&[r], Pillar::Resilience, now());
        assert_eq!(codes(&report), vec![REASON_RES_SNAPSHOT_DATA_NOT_COLLECTED]);
    }

    #[test]
    fn resilience_flags_version_upgrade_disabled() {
        let mut data = healthy_data();
        data["allow_version_upgrade"] = json!(false);
        let r = fixture("warehouse-pinned", ok_tags(), data, now());
        let report = evaluate_redshift_fleet(&[r], Pillar::Resilience, now());
        assert_eq!(codes(&report), vec![REASON_RES_VERSION_UPGRADE_DISABLED]);
        assert!(matches!(report.findings[0].severity, Severity::Low));
    }

    #[test]
    fn resilience_flags_unavailable_cluster_but_not_paused() {
        let mut unavailable = healthy_data();
        unavailable["cluster_availability_status"] = json!("Unavailable");
        let r1 = fixture("warehouse-down", ok_tags(), unavailable, now());
        let report = evaluate_redshift_fleet(&[r1], Pillar::Resilience, now());
        assert_eq!(codes(&report), vec![REASON_RES_CLUSTER_UNAVAILABLE]);
        assert!(matches!(report.findings[0].severity, Severity::High));

        let mut paused = healthy_data();
        paused["cluster_availability_status"] = json!("Paused");
        let r2 = fixture("warehouse-paused", ok_tags(), paused, now());
        let report = evaluate_redshift_fleet(&[r2], Pillar::Resilience, now());
        assert!(
            report.findings.is_empty(),
            "unexpected: {:?}",
            report.findings
        );
    }

    #[test]
    fn security_flags_publicly_accessible_as_high() {
        let mut data = healthy_data();
        data["publicly_accessible"] = json!(true);
        let r = fixture("warehouse-public", ok_tags(), data, now());
        let report = evaluate_redshift_fleet(&[r], Pillar::Security, now());
        assert_eq!(codes(&report), vec![REASON_SEC_PUBLICLY_ACCESSIBLE]);
        assert!(matches!(report.findings[0].severity, Severity::High));
    }

    #[test]
    fn security_flags_unencrypted_cluster_as_high() {
        let mut data = healthy_data();
        data["encrypted"] = json!(false);
        data.as_object_mut().unwrap().remove("kms_key_id");
        let r = fixture("warehouse-plain", ok_tags(), data, now());
        let report = evaluate_redshift_fleet(&[r], Pillar::Security, now());
        assert_eq!(codes(&report), vec![REASON_SEC_NOT_ENCRYPTED]);
        assert!(matches!(report.findings[0].severity, Severity::High));
    }

    #[test]
    fn security_reports_gap_when_encryption_not_collected() {
        let mut data = healthy_data();
        data.as_object_mut().unwrap().remove("encrypted");
        data.as_object_mut().unwrap().remove("kms_key_id");
        let r = fixture("warehouse-encgap", ok_tags(), data, now());
        let report = evaluate_redshift_fleet(&[r], Pillar::Security, now());
        assert_eq!(
            codes(&report),
            vec![REASON_SEC_ENCRYPTION_DATA_NOT_COLLECTED]
        );
    }

    #[test]
    fn security_flags_default_kms_key_when_encrypted_without_key_id() {
        let mut data = healthy_data();
        data.as_object_mut().unwrap().remove("kms_key_id");
        let r = fixture("warehouse-defaultkey", ok_tags(), data, now());
        let report = evaluate_redshift_fleet(&[r], Pillar::Security, now());
        assert_eq!(codes(&report), vec![REASON_SEC_DEFAULT_KMS_KEY]);
        assert!(matches!(report.findings[0].severity, Severity::Low));
    }

    #[test]
    fn security_flags_enhanced_vpc_routing_disabled() {
        let mut data = healthy_data();
        data["enhanced_vpc_routing"] = json!(false);
        let r = fixture("warehouse-novpc", ok_tags(), data, now());
        let report = evaluate_redshift_fleet(&[r], Pillar::Security, now());
        assert_eq!(
            codes(&report),
            vec![REASON_SEC_ENHANCED_VPC_ROUTING_DISABLED]
        );
    }

    #[test]
    fn stale_inventory_is_flagged() {
        let mut r = fixture("warehouse-stale", ok_tags(), healthy_data(), now());
        r.last_refreshed = now() - Duration::hours(48);
        let report = evaluate_redshift_fleet(&[r], Pillar::Resilience, now());
        assert_eq!(report.stale_resources, 1);
        assert!(codes(&report).contains(&REASON_INV_STALE_DATA));
    }

    #[test]
    fn non_redshift_resources_are_skipped_and_not_counted() {
        let mut r = fixture("queue-1", json!({}), json!({}), now());
        r.resource_type = "SqsQueue".to_string();
        let report = evaluate_redshift_fleet(&[r], Pillar::Cost, now());
        assert_eq!(report.resources_evaluated, 0);
        assert!(report.findings.is_empty());
    }
}

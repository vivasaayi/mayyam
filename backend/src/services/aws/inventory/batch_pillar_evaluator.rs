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

// Deterministic AWS Batch compute environment inventory evaluators for the
// cost, security, and resilience pillars.
//
// Evaluates fields persisted by batch_control_plane: type (MANAGED/UNMANAGED),
// state (ENABLED/DISABLED), status (VALID/INVALID/...), service_role,
// compute_resource_type (EC2/SPOT/FARGATE/FARGATE_SPOT), allocation_strategy,
// minv_cpus, subnet_count, security_group_count, plus the tags column.
// compute_resources only exists for MANAGED environments, so capacity, subnet,
// and security-group checks are gated on type == "MANAGED"; an absent count
// on a managed environment is reported as a data gap, not a failure.

use chrono::{DateTime, Utc};
use serde_json::json;

use crate::models::aws_resource::Model as AwsResourceModel;
use crate::services::aws::inventory::types::{
    check_stale, data_str, score_pillar, InventoryFinding, Pillar, PillarReport, Severity,
};

/// Only rows of this resource type are evaluated.
pub const RESOURCE_TYPE: &str = "BatchComputeEnv";

// Reason codes are the stable contract for findings; never reuse or rename.
pub const REASON_COST_MIN_VCPUS_WARM: &str = "BATCH_COST_MIN_VCPUS_WARM";
pub const REASON_COST_MINVCPUS_DATA_NOT_COLLECTED: &str = "BATCH_COST_MINVCPUS_DATA_NOT_COLLECTED";
pub const REASON_COST_ON_DEMAND_ONLY: &str = "BATCH_COST_ON_DEMAND_ONLY";
pub const REASON_COST_DISABLED_ENV: &str = "BATCH_COST_DISABLED_ENV";
pub const REASON_COST_NO_TAGS: &str = "BATCH_COST_NO_TAGS";
pub const REASON_SEC_NO_SERVICE_ROLE: &str = "BATCH_SEC_NO_SERVICE_ROLE";
pub const REASON_SEC_NO_SECURITY_GROUPS: &str = "BATCH_SEC_NO_SECURITY_GROUPS";
pub const REASON_SEC_SG_DATA_NOT_COLLECTED: &str = "BATCH_SEC_SG_DATA_NOT_COLLECTED";
pub const REASON_RES_STATUS_INVALID: &str = "BATCH_RES_STATUS_INVALID";
pub const REASON_RES_SINGLE_SUBNET: &str = "BATCH_RES_SINGLE_SUBNET";
pub const REASON_RES_SUBNET_DATA_NOT_COLLECTED: &str = "BATCH_RES_SUBNET_DATA_NOT_COLLECTED";
pub const REASON_RES_SPOT_NOT_CAPACITY_OPTIMIZED: &str = "BATCH_RES_SPOT_NOT_CAPACITY_OPTIMIZED";
pub const REASON_INV_STALE_DATA: &str = "BATCH_INV_STALE_DATA";

/// Evaluate every Batch compute environment in the fleet for one pillar.
/// Rows whose `resource_type` is not `BatchComputeEnv` are skipped and not
/// counted.
pub fn evaluate_batch_fleet(
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

fn is_managed(resource: &AwsResourceModel) -> bool {
    data_str(&resource.resource_data, "type").as_deref() == Some("MANAGED")
}

fn compute_resource_type(resource: &AwsResourceModel) -> Option<String> {
    data_str(&resource.resource_data, "compute_resource_type")
}

fn data_i64(resource: &AwsResourceModel, key: &str) -> Option<i64> {
    resource.resource_data.get(key).and_then(|v| v.as_i64())
}

fn evaluate_cost(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    // minv_cpus only applies to managed EC2/SPOT environments; Fargate
    // capacity is fully on-demand and unmanaged environments have no
    // compute_resources block.
    let cr_type = compute_resource_type(resource);
    let ec2_family = matches!(cr_type.as_deref(), Some("EC2") | Some("SPOT"));
    if is_managed(resource) && ec2_family {
        match data_i64(resource, "minv_cpus") {
            Some(min_vcpus) if min_vcpus > 0 => {
                findings.push(InventoryFinding {
                    resource_id: resource.resource_id.clone(),
                    arn: resource.arn.clone(),
                    pillar: Pillar::Cost,
                    reason_code: REASON_COST_MIN_VCPUS_WARM.to_string(),
                    severity: Severity::Medium,
                    message: format!(
                        "Compute environment {} keeps minvCpus={} warm; this idle capacity is billed even when no jobs run",
                        resource.resource_id, min_vcpus
                    ),
                    evidence: json!({
                        "minv_cpus": min_vcpus,
                        "compute_resource_type": cr_type,
                    }),
                });
            }
            None => {
                findings.push(InventoryFinding {
                    resource_id: resource.resource_id.clone(),
                    arn: resource.arn.clone(),
                    pillar: Pillar::Cost,
                    reason_code: REASON_COST_MINVCPUS_DATA_NOT_COLLECTED.to_string(),
                    severity: Severity::Low,
                    message: format!(
                        "minvCpus for managed compute environment {} is not collected yet; warm-capacity spend cannot be assessed",
                        resource.resource_id
                    ),
                    evidence: json!({ "minv_cpus_collected": false }),
                });
            }
            Some(_) => {}
        }

        // Pure on-demand EC2 without any Spot consideration; Spot or
        // Fargate Spot typically cuts Batch compute cost substantially.
        if cr_type.as_deref() == Some("EC2") {
            findings.push(InventoryFinding {
                resource_id: resource.resource_id.clone(),
                arn: resource.arn.clone(),
                pillar: Pillar::Cost,
                reason_code: REASON_COST_ON_DEMAND_ONLY.to_string(),
                severity: Severity::Low,
                message: format!(
                    "Compute environment {} uses on-demand EC2 only; evaluate SPOT or FARGATE_SPOT for interruptible batch workloads",
                    resource.resource_id
                ),
                evidence: json!({
                    "compute_resource_type": "EC2",
                    "allocation_strategy": data_str(&resource.resource_data, "allocation_strategy"),
                }),
            });
        }
    }

    // A DISABLED environment no longer schedules jobs but may keep billing
    // (warm capacity, ECS cluster); lingering disabled environments should be
    // deleted.
    let state = data_str(&resource.resource_data, "state");
    let status = data_str(&resource.resource_data, "status");
    let deleting = matches!(status.as_deref(), Some("DELETING") | Some("DELETED"));
    if state.as_deref() == Some("DISABLED") && !deleting {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Cost,
            reason_code: REASON_COST_DISABLED_ENV.to_string(),
            severity: Severity::Low,
            message: format!(
                "Compute environment {} is DISABLED but not deleted; disabled environments can still incur charges",
                resource.resource_id
            ),
            evidence: json!({ "state": "DISABLED", "status": status }),
        });
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
                "Compute environment {} has no tags recorded (untagged resource or tag collection gap); cost allocation cannot be assessed",
                resource.resource_id
            ),
            evidence: json!({ "tags": resource.tags }),
        });
    }
}

fn evaluate_security(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    if data_str(&resource.resource_data, "service_role").is_none() {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Security,
            reason_code: REASON_SEC_NO_SERVICE_ROLE.to_string(),
            severity: Severity::Medium,
            message: format!(
                "Compute environment {} has no service role recorded; Batch cannot make scoped API calls on its behalf",
                resource.resource_id
            ),
            evidence: json!({ "service_role_collected": false }),
        });
    }

    // Security groups live in compute_resources, which only managed
    // environments have.
    if is_managed(resource) {
        match data_i64(resource, "security_group_count") {
            Some(0) => {
                findings.push(InventoryFinding {
                    resource_id: resource.resource_id.clone(),
                    arn: resource.arn.clone(),
                    pillar: Pillar::Security,
                    reason_code: REASON_SEC_NO_SECURITY_GROUPS.to_string(),
                    severity: Severity::Medium,
                    message: format!(
                        "Compute environment {} has no security groups on its compute resources; instances may fall back to defaults or rely on an unaudited launch template",
                        resource.resource_id
                    ),
                    evidence: json!({ "security_group_count": 0 }),
                });
            }
            None => {
                findings.push(InventoryFinding {
                    resource_id: resource.resource_id.clone(),
                    arn: resource.arn.clone(),
                    pillar: Pillar::Security,
                    reason_code: REASON_SEC_SG_DATA_NOT_COLLECTED.to_string(),
                    severity: Severity::Low,
                    message: format!(
                        "Security group data for managed compute environment {} is not collected yet; security pillar cannot be fully assessed",
                        resource.resource_id
                    ),
                    evidence: json!({ "security_group_count_collected": false }),
                });
            }
            Some(_) => {}
        }
    }
}

fn evaluate_resilience(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    let status = data_str(&resource.resource_data, "status");
    if status.as_deref() == Some("INVALID") {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Resilience,
            reason_code: REASON_RES_STATUS_INVALID.to_string(),
            severity: Severity::High,
            message: format!(
                "Compute environment {} is in INVALID status; jobs routed to it will not run until it is repaired",
                resource.resource_id
            ),
            evidence: json!({
                "status": "INVALID",
                "status_reason": data_str(&resource.resource_data, "status_reason"),
            }),
        });
    }

    if is_managed(resource) {
        match data_i64(resource, "subnet_count") {
            Some(count) if count < 2 => {
                findings.push(InventoryFinding {
                    resource_id: resource.resource_id.clone(),
                    arn: resource.arn.clone(),
                    pillar: Pillar::Resilience,
                    reason_code: REASON_RES_SINGLE_SUBNET.to_string(),
                    severity: Severity::Medium,
                    message: format!(
                        "Compute environment {} spans {} subnet(s); a single subnet provides no Availability Zone spread",
                        resource.resource_id, count
                    ),
                    evidence: json!({ "subnet_count": count }),
                });
            }
            None => {
                findings.push(InventoryFinding {
                    resource_id: resource.resource_id.clone(),
                    arn: resource.arn.clone(),
                    pillar: Pillar::Resilience,
                    reason_code: REASON_RES_SUBNET_DATA_NOT_COLLECTED.to_string(),
                    severity: Severity::Low,
                    message: format!(
                        "Subnet data for managed compute environment {} is not collected yet; AZ spread cannot be assessed",
                        resource.resource_id
                    ),
                    evidence: json!({ "subnet_count_collected": false }),
                });
            }
            Some(_) => {}
        }
    }

    // For SPOT environments, only the capacity-optimized strategies reduce
    // interruption risk. An absent allocation_strategy means the BEST_FIT
    // default is in effect.
    if compute_resource_type(resource).as_deref() == Some("SPOT") {
        let strategy = data_str(&resource.resource_data, "allocation_strategy");
        let capacity_optimized = matches!(
            strategy.as_deref(),
            Some("SPOT_CAPACITY_OPTIMIZED") | Some("SPOT_PRICE_CAPACITY_OPTIMIZED")
        );
        if !capacity_optimized {
            findings.push(InventoryFinding {
                resource_id: resource.resource_id.clone(),
                arn: resource.arn.clone(),
                pillar: Pillar::Resilience,
                reason_code: REASON_RES_SPOT_NOT_CAPACITY_OPTIMIZED.to_string(),
                severity: Severity::Medium,
                message: format!(
                    "SPOT compute environment {} uses allocation strategy {}; SPOT_PRICE_CAPACITY_OPTIMIZED or SPOT_CAPACITY_OPTIMIZED reduces interruption risk",
                    resource.resource_id,
                    strategy.as_deref().unwrap_or("BEST_FIT (default)")
                ),
                evidence: json!({
                    "compute_resource_type": "SPOT",
                    "allocation_strategy": strategy,
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
            resource_type: RESOURCE_TYPE.to_string(),
            resource_id: resource_id.to_string(),
            arn: format!(
                "arn:aws:batch:us-east-1:123456789012:compute-environment/{}",
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

    /// Managed SPOT environment with capacity-optimized allocation, zero warm
    /// vCPUs, multi-AZ subnets, security groups, and a service role.
    fn healthy_spot_data() -> Value {
        json!({
            "compute_environment_name": "ce-ok",
            "compute_environment_arn": "arn:aws:batch:us-east-1:123456789012:compute-environment/ce-ok",
            "type": "MANAGED",
            "state": "ENABLED",
            "status": "VALID",
            "service_role": "arn:aws:iam::123456789012:role/BatchServiceRole",
            "container_orchestration_type": "ECS",
            "compute_resource_type": "SPOT",
            "allocation_strategy": "SPOT_PRICE_CAPACITY_OPTIMIZED",
            "minv_cpus": 0,
            "maxv_cpus": 256,
            "desiredv_cpus": 0,
            "instance_types": ["optimal"],
            "spot_iam_fleet_role": "arn:aws:iam::123456789012:role/SpotFleetRole",
            "subnet_count": 3,
            "security_group_count": 2,
        })
    }

    fn unmanaged_data() -> Value {
        json!({
            "compute_environment_name": "ce-unmanaged",
            "compute_environment_arn": "arn:aws:batch:us-east-1:123456789012:compute-environment/ce-unmanaged",
            "type": "UNMANAGED",
            "state": "ENABLED",
            "status": "VALID",
            "service_role": "arn:aws:iam::123456789012:role/BatchServiceRole",
            "unmanagedv_cpus": 16,
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
    fn cost_flags_warm_min_vcpus() {
        let mut data = healthy_spot_data();
        data["minv_cpus"] = json!(4);
        let r = fixture("ce-warm", json!({"team": "batch"}), data, now());
        let report = evaluate_batch_fleet(&[r], Pillar::Cost, now());
        assert_eq!(codes(&report), vec![REASON_COST_MIN_VCPUS_WARM]);
        assert_eq!(report.findings[0].evidence["minv_cpus"], json!(4));
    }

    #[test]
    fn cost_data_gap_when_min_vcpus_not_collected() {
        let mut data = healthy_spot_data();
        data.as_object_mut().unwrap().remove("minv_cpus");
        let r = fixture("ce-vcpugap", json!({"team": "batch"}), data, now());
        let report = evaluate_batch_fleet(&[r], Pillar::Cost, now());
        assert_eq!(
            codes(&report),
            vec![REASON_COST_MINVCPUS_DATA_NOT_COLLECTED]
        );
    }

    #[test]
    fn cost_flags_pure_on_demand_ec2() {
        let mut data = healthy_spot_data();
        data["compute_resource_type"] = json!("EC2");
        data["allocation_strategy"] = json!("BEST_FIT_PROGRESSIVE");
        let r = fixture("ce-ondemand", json!({"team": "batch"}), data, now());
        let report = evaluate_batch_fleet(&[r], Pillar::Cost, now());
        assert_eq!(codes(&report), vec![REASON_COST_ON_DEMAND_ONLY]);
    }

    #[test]
    fn cost_flags_disabled_environment_but_not_deleting_one() {
        let mut data = healthy_spot_data();
        data["state"] = json!("DISABLED");
        let r = fixture("ce-disabled", json!({"team": "batch"}), data, now());
        let report = evaluate_batch_fleet(&[r], Pillar::Cost, now());
        assert_eq!(codes(&report), vec![REASON_COST_DISABLED_ENV]);

        let mut deleting = healthy_spot_data();
        deleting["state"] = json!("DISABLED");
        deleting["status"] = json!("DELETING");
        let r2 = fixture("ce-deleting", json!({"team": "batch"}), deleting, now());
        let report = evaluate_batch_fleet(&[r2], Pillar::Cost, now());
        assert!(
            report.findings.is_empty(),
            "unexpected: {:?}",
            report.findings
        );
    }

    #[test]
    fn cost_flags_missing_tags() {
        let r = fixture("ce-untagged", json!({}), healthy_spot_data(), now());
        let report = evaluate_batch_fleet(&[r], Pillar::Cost, now());
        assert_eq!(codes(&report), vec![REASON_COST_NO_TAGS]);
    }

    #[test]
    fn cost_skips_fargate_for_vcpu_and_on_demand_checks() {
        let mut data = healthy_spot_data();
        data["compute_resource_type"] = json!("FARGATE");
        let obj = data.as_object_mut().unwrap();
        obj.remove("allocation_strategy");
        obj.remove("minv_cpus");
        obj.remove("maxv_cpus");
        obj.remove("desiredv_cpus");
        obj.remove("spot_iam_fleet_role");
        let r = fixture("ce-fargate", json!({"team": "batch"}), data, now());
        let report = evaluate_batch_fleet(&[r], Pillar::Cost, now());
        assert!(
            report.findings.is_empty(),
            "unexpected: {:?}",
            report.findings
        );
    }

    #[test]
    fn security_flags_missing_service_role() {
        let mut data = healthy_spot_data();
        data.as_object_mut().unwrap().remove("service_role");
        let r = fixture("ce-norole", json!({"team": "batch"}), data, now());
        let report = evaluate_batch_fleet(&[r], Pillar::Security, now());
        assert_eq!(codes(&report), vec![REASON_SEC_NO_SERVICE_ROLE]);
    }

    #[test]
    fn security_flags_zero_security_groups() {
        let mut data = healthy_spot_data();
        data["security_group_count"] = json!(0);
        let r = fixture("ce-nosg", json!({"team": "batch"}), data, now());
        let report = evaluate_batch_fleet(&[r], Pillar::Security, now());
        assert_eq!(codes(&report), vec![REASON_SEC_NO_SECURITY_GROUPS]);
    }

    #[test]
    fn security_data_gap_when_sg_count_not_collected() {
        let mut data = healthy_spot_data();
        data.as_object_mut().unwrap().remove("security_group_count");
        let r = fixture("ce-sggap", json!({"team": "batch"}), data, now());
        let report = evaluate_batch_fleet(&[r], Pillar::Security, now());
        assert_eq!(codes(&report), vec![REASON_SEC_SG_DATA_NOT_COLLECTED]);
    }

    #[test]
    fn resilience_flags_invalid_status_as_high() {
        let mut data = healthy_spot_data();
        data["status"] = json!("INVALID");
        data["status_reason"] = json!("CLIENT_ERROR - service role does not exist");
        let r = fixture("ce-invalid", json!({"team": "batch"}), data, now());
        let report = evaluate_batch_fleet(&[r], Pillar::Resilience, now());
        assert_eq!(codes(&report), vec![REASON_RES_STATUS_INVALID]);
        assert!(matches!(report.findings[0].severity, Severity::High));
        assert_eq!(
            report.findings[0].evidence["status_reason"],
            json!("CLIENT_ERROR - service role does not exist")
        );
    }

    #[test]
    fn resilience_flags_single_subnet() {
        let mut data = healthy_spot_data();
        data["subnet_count"] = json!(1);
        let r = fixture("ce-oneaz", json!({"team": "batch"}), data, now());
        let report = evaluate_batch_fleet(&[r], Pillar::Resilience, now());
        assert_eq!(codes(&report), vec![REASON_RES_SINGLE_SUBNET]);
    }

    #[test]
    fn resilience_data_gap_when_subnet_count_not_collected() {
        let mut data = healthy_spot_data();
        data.as_object_mut().unwrap().remove("subnet_count");
        let r = fixture("ce-subnetgap", json!({"team": "batch"}), data, now());
        let report = evaluate_batch_fleet(&[r], Pillar::Resilience, now());
        assert_eq!(codes(&report), vec![REASON_RES_SUBNET_DATA_NOT_COLLECTED]);
    }

    #[test]
    fn resilience_flags_spot_without_capacity_optimized_strategy() {
        let mut best_fit = healthy_spot_data();
        best_fit["allocation_strategy"] = json!("BEST_FIT");
        let r1 = fixture("ce-bestfit", json!({"team": "batch"}), best_fit, now());
        let report = evaluate_batch_fleet(&[r1], Pillar::Resilience, now());
        assert_eq!(codes(&report), vec![REASON_RES_SPOT_NOT_CAPACITY_OPTIMIZED]);

        // Absent strategy means the BEST_FIT default is in effect.
        let mut defaulted = healthy_spot_data();
        defaulted
            .as_object_mut()
            .unwrap()
            .remove("allocation_strategy");
        let r2 = fixture("ce-default", json!({"team": "batch"}), defaulted, now());
        let report = evaluate_batch_fleet(&[r2], Pillar::Resilience, now());
        assert_eq!(codes(&report), vec![REASON_RES_SPOT_NOT_CAPACITY_OPTIMIZED]);
        assert!(report.findings[0].message.contains("BEST_FIT (default)"));
    }

    #[test]
    fn resilience_passes_spot_capacity_optimized() {
        let mut data = healthy_spot_data();
        data["allocation_strategy"] = json!("SPOT_CAPACITY_OPTIMIZED");
        let r = fixture("ce-capopt", json!({"team": "batch"}), data, now());
        let report = evaluate_batch_fleet(&[r], Pillar::Resilience, now());
        assert!(
            report.findings.is_empty(),
            "unexpected: {:?}",
            report.findings
        );
    }

    #[test]
    fn unmanaged_env_skips_compute_resource_checks() {
        let r = fixture(
            "ce-unmanaged",
            json!({"team": "batch"}),
            unmanaged_data(),
            now(),
        );
        for pillar in [Pillar::Cost, Pillar::Security, Pillar::Resilience] {
            let report = evaluate_batch_fleet(std::slice::from_ref(&r), pillar, now());
            assert!(
                report.findings.is_empty(),
                "unexpected for {:?}: {:?}",
                pillar,
                report.findings
            );
        }
    }

    #[test]
    fn stale_inventory_is_flagged() {
        let mut r = fixture(
            "ce-stale",
            json!({"team": "batch"}),
            healthy_spot_data(),
            now(),
        );
        r.last_refreshed = now() - Duration::hours(48);
        let report = evaluate_batch_fleet(&[r], Pillar::Resilience, now());
        assert_eq!(report.stale_resources, 1);
        assert!(codes(&report).contains(&REASON_INV_STALE_DATA));
    }

    #[test]
    fn non_batch_resources_are_skipped_and_not_counted() {
        let mut r = fixture("queue-1", json!({}), json!({}), now());
        r.resource_type = "SqsQueue".to_string();
        let report = evaluate_batch_fleet(&[r], Pillar::Cost, now());
        assert_eq!(report.resources_evaluated, 0);
        assert!(report.findings.is_empty());
    }

    #[test]
    fn healthy_spot_env_passes_all_pillars() {
        let r = fixture(
            "ce-ok",
            json!({"team": "batch"}),
            healthy_spot_data(),
            now(),
        );
        for pillar in [Pillar::Cost, Pillar::Security, Pillar::Resilience] {
            let report = evaluate_batch_fleet(std::slice::from_ref(&r), pillar, now());
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

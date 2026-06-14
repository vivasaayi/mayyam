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

// Deterministic Auto Scaling group inventory and telemetry evaluators for the
// cost, resilience, and security pillars (roadmap rows
// 01-AWS-CLOUD-00064/00073/00100 plus 01-AWS-CLOUD-00065/00074/00101).
//
// Evaluates fields persisted by autoscaling_control_plane: min_size, max_size,
// desired_capacity, availability_zones, health_check_type, load_balancer_names,
// target_group_arns, launch_configuration_name, uses_launch_template,
// uses_mixed_instances_policy, suspended_process_count, plus the tags column.

use chrono::{DateTime, Utc};
use serde_json::{json, Value};

use crate::models::aws_resource::Model as AwsResourceModel;
use crate::services::aws::inventory::types::{
    check_stale, data_str, score_pillar, InventoryFinding, Pillar, PillarReport, Severity,
};

/// Only rows of this resource type are evaluated.
pub const RESOURCE_TYPE: &str = "AutoScalingGroup";

// Reason codes are the stable contract for findings; never reuse or rename.
pub const REASON_COST_NO_TAGS: &str = "ASG_COST_NO_TAGS";
pub const REASON_COST_FIXED_SIZE: &str = "ASG_COST_FIXED_SIZE";
pub const REASON_COST_MISSING_GROUP_METRICS_TELEMETRY: &str =
    "ASG_COST_MISSING_GROUP_METRICS_TELEMETRY";
pub const REASON_COST_MISSING_CAPACITY_TELEMETRY: &str = "ASG_COST_MISSING_CAPACITY_TELEMETRY";
pub const REASON_RES_SINGLE_AZ: &str = "ASG_RES_SINGLE_AZ";
pub const REASON_RES_ELB_HEALTH_CHECK_EC2_ONLY: &str = "ASG_RES_ELB_HEALTH_CHECK_EC2_ONLY";
pub const REASON_RES_SUSPENDED_PROCESSES: &str = "ASG_RES_SUSPENDED_PROCESSES";
pub const REASON_RES_DESIRED_BELOW_MIN: &str = "ASG_RES_DESIRED_BELOW_MIN";
pub const REASON_RES_MISSING_REPLACEMENT_TELEMETRY: &str = "ASG_RES_MISSING_REPLACEMENT_TELEMETRY";
pub const REASON_RES_MISSING_INSTANCE_HEALTH_TELEMETRY: &str =
    "ASG_RES_MISSING_INSTANCE_HEALTH_TELEMETRY";
pub const REASON_RES_UNHEALTHY_INSTANCE_TELEMETRY: &str = "ASG_RES_UNHEALTHY_INSTANCE_TELEMETRY";
pub const REASON_SEC_LEGACY_LAUNCH_CONFIGURATION: &str = "ASG_SEC_LEGACY_LAUNCH_CONFIGURATION";
pub const REASON_SEC_LAUNCH_SOURCE_DATA_NOT_COLLECTED: &str =
    "ASG_SEC_LAUNCH_SOURCE_DATA_NOT_COLLECTED";
pub const REASON_SEC_MISSING_INSTANCE_TELEMETRY: &str = "ASG_SEC_MISSING_INSTANCE_TELEMETRY";
pub const REASON_SEC_TELEMETRY_COLLECTION_ERRORS: &str = "ASG_SEC_TELEMETRY_COLLECTION_ERRORS";
pub const REASON_TEL_MISSING_COLLECTION_METADATA: &str = "ASG_TEL_MISSING_COLLECTION_METADATA";
pub const REASON_TEL_COLLECTION_ERRORS: &str = "ASG_TEL_COLLECTION_ERRORS";
pub const REASON_INV_STALE_DATA: &str = "ASG_INV_STALE_DATA";

/// Evaluate every Auto Scaling group in the fleet for one pillar. Rows whose
/// `resource_type` is not `AutoScalingGroup` are skipped and not counted.
pub fn evaluate_autoscaling_fleet(
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
            Pillar::Cost => {
                evaluate_telemetry_collection(resource, pillar, &mut findings);
                evaluate_cost(resource, &mut findings);
            }
            Pillar::Security => {
                evaluate_telemetry_collection(resource, pillar, &mut findings);
                evaluate_security(resource, &mut findings);
            }
            Pillar::Resilience => {
                evaluate_telemetry_collection(resource, pillar, &mut findings);
                evaluate_resilience(resource, &mut findings);
            }
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

fn data_i64(resource_data: &Value, key: &str) -> Option<i64> {
    resource_data.get(key).and_then(|v| v.as_i64())
}

fn data_array_len(resource_data: &Value, key: &str) -> Option<usize> {
    resource_data
        .get(key)
        .and_then(|v| v.as_array())
        .map(|a| a.len())
}

fn data_u64(resource_data: &Value, key: &str) -> Option<u64> {
    resource_data.get(key).and_then(|value| {
        value
            .as_u64()
            .or_else(|| value.as_i64().map(|n| n.max(0) as u64))
    })
}

fn resource_data_keys(resource: &AwsResourceModel) -> Vec<String> {
    let mut keys = resource
        .resource_data
        .as_object()
        .map(|data| data.keys().cloned().collect::<Vec<_>>())
        .unwrap_or_default();
    keys.sort();
    keys
}

fn missing_fields<'a>(resource: &AwsResourceModel, required_fields: &'a [&str]) -> Vec<&'a str> {
    required_fields
        .iter()
        .copied()
        .filter(|field| resource.resource_data.get(*field).is_none())
        .collect()
}

fn is_elb_attached(resource: &AwsResourceModel) -> bool {
    data_array_len(&resource.resource_data, "load_balancer_names").unwrap_or(0) > 0
        || data_array_len(&resource.resource_data, "target_group_arns").unwrap_or(0) > 0
}

fn evaluate_telemetry_collection(
    resource: &AwsResourceModel,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    let required_fields = [
        "telemetry_collection_started_at",
        "telemetry_collection_completed_at",
        "telemetry_collection_duration_ms",
        "telemetry_collection_success_count",
        "telemetry_collection_failure_count",
        "telemetry_collection_error_count",
    ];
    let missing = missing_fields(resource, &required_fields);
    if !missing.is_empty() {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar,
            reason_code: REASON_TEL_MISSING_COLLECTION_METADATA.to_string(),
            severity: Severity::Medium,
            message: format!(
                "Auto Scaling group {} is missing telemetry collection metadata needed to trust the latest evidence",
                resource.resource_id
            ),
            evidence: json!({
                "required_fields": required_fields,
                "missing_fields": missing,
                "resource_data_keys": resource_data_keys(resource),
            }),
        });
    }

    let error_count =
        data_u64(&resource.resource_data, "telemetry_collection_error_count").unwrap_or(0);
    let errors = resource
        .resource_data
        .get("telemetry_collection_errors")
        .and_then(|value| value.as_array())
        .cloned()
        .unwrap_or_default();
    if error_count > 0 || !errors.is_empty() {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar,
            reason_code: REASON_TEL_COLLECTION_ERRORS.to_string(),
            severity: Severity::High,
            message: format!(
                "Auto Scaling group {} has telemetry collection errors; the pillar evidence may be incomplete",
                resource.resource_id
            ),
            evidence: json!({
                "telemetry_collection_error_count": error_count,
                "telemetry_collection_errors": errors,
            }),
        });
    }
}

fn evaluate_cost(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    let capacity_fields = ["min_size", "max_size", "desired_capacity", "instance_count"];
    let missing_capacity = missing_fields(resource, &capacity_fields);
    if !missing_capacity.is_empty() {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Cost,
            reason_code: REASON_COST_MISSING_CAPACITY_TELEMETRY.to_string(),
            severity: Severity::Medium,
            message: format!(
                "Auto Scaling group {} is missing capacity telemetry needed to explain cost posture",
                resource.resource_id
            ),
            evidence: json!({
                "required_fields": capacity_fields,
                "missing_fields": missing_capacity,
                "resource_data_keys": resource_data_keys(resource),
            }),
        });
    }

    let group_metric_fields = ["enabled_metrics", "enabled_metric_count"];
    let missing_group_metrics = missing_fields(resource, &group_metric_fields);
    if !missing_group_metrics.is_empty() {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Cost,
            reason_code: REASON_COST_MISSING_GROUP_METRICS_TELEMETRY.to_string(),
            severity: Severity::Medium,
            message: format!(
                "Auto Scaling group {} is missing enabled group metrics telemetry needed to quantify scaling cost behavior",
                resource.resource_id
            ),
            evidence: json!({
                "required_fields": group_metric_fields,
                "missing_fields": missing_group_metrics,
                "resource_data_keys": resource_data_keys(resource),
            }),
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
                "Auto Scaling group {} has no tags recorded (untagged resource or tag collection gap); cost allocation cannot be assessed",
                resource.resource_id
            ),
            evidence: json!({ "tags": resource.tags }),
        });
    }

    // A group pinned to min == max can never scale in, so capacity is paid for
    // even when idle. Empty fixed groups (max == 0) hold no instances and are
    // not flagged.
    let min_size = data_i64(&resource.resource_data, "min_size");
    let max_size = data_i64(&resource.resource_data, "max_size");
    if let (Some(min), Some(max)) = (min_size, max_size) {
        if min == max && max > 0 {
            findings.push(InventoryFinding {
                resource_id: resource.resource_id.clone(),
                arn: resource.arn.clone(),
                pillar: Pillar::Cost,
                reason_code: REASON_COST_FIXED_SIZE.to_string(),
                severity: Severity::Low,
                message: format!(
                    "Auto Scaling group {} is pinned to a fixed size (min == max == {}); it can never scale in, so idle capacity is still billed",
                    resource.resource_id, max
                ),
                evidence: json!({ "min_size": min, "max_size": max }),
            });
        }
    }
}

fn evaluate_security(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    if resource.resource_data.get("instance_health").is_none() {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Security,
            reason_code: REASON_SEC_MISSING_INSTANCE_TELEMETRY.to_string(),
            severity: Severity::Medium,
            message: format!(
                "Auto Scaling group {} is missing instance telemetry needed to validate security exposure of running members",
                resource.resource_id
            ),
            evidence: json!({
                "required_fields": ["instance_health"],
                "resource_data_keys": resource_data_keys(resource),
            }),
        });
    }

    let telemetry_error_count =
        data_u64(&resource.resource_data, "telemetry_collection_error_count").unwrap_or(0);
    if telemetry_error_count > 0 {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Security,
            reason_code: REASON_SEC_TELEMETRY_COLLECTION_ERRORS.to_string(),
            severity: Severity::High,
            message: format!(
                "Auto Scaling group {} has telemetry collection errors that can hide security evidence",
                resource.resource_id
            ),
            evidence: json!({
                "telemetry_collection_error_count": telemetry_error_count,
                "telemetry_collection_errors": resource.resource_data.get("telemetry_collection_errors"),
            }),
        });
    }

    let legacy_launch_configuration =
        data_str(&resource.resource_data, "launch_configuration_name").is_some();
    let uses_launch_template = resource
        .resource_data
        .get("uses_launch_template")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let uses_mixed_instances_policy = resource
        .resource_data
        .get("uses_mixed_instances_policy")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    if legacy_launch_configuration {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Security,
            reason_code: REASON_SEC_LEGACY_LAUNCH_CONFIGURATION.to_string(),
            severity: Severity::Medium,
            message: format!(
                "Auto Scaling group {} uses a deprecated launch configuration; migrate to a launch template to enforce IMDSv2 and current instance security features",
                resource.resource_id
            ),
            evidence: json!({
                "launch_configuration_name":
                    data_str(&resource.resource_data, "launch_configuration_name")
            }),
        });
    } else if !uses_launch_template && !uses_mixed_instances_policy {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Security,
            reason_code: REASON_SEC_LAUNCH_SOURCE_DATA_NOT_COLLECTED.to_string(),
            severity: Severity::Low,
            message: format!(
                "Launch source for Auto Scaling group {} is not collected yet (no launch configuration, launch template, or mixed instances policy recorded); security pillar cannot be fully assessed",
                resource.resource_id
            ),
            evidence: json!({ "launch_source_collected": false }),
        });
    }
}

fn evaluate_resilience(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    let replacement_fields = [
        "availability_zones",
        "health_check_type",
        "load_balancer_names",
        "target_group_arns",
        "suspended_process_count",
        "instance_count",
    ];
    let missing_replacement = missing_fields(resource, &replacement_fields);
    if !missing_replacement.is_empty() {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Resilience,
            reason_code: REASON_RES_MISSING_REPLACEMENT_TELEMETRY.to_string(),
            severity: Severity::Medium,
            message: format!(
                "Auto Scaling group {} is missing replacement telemetry needed to explain resilience posture",
                resource.resource_id
            ),
            evidence: json!({
                "required_fields": replacement_fields,
                "missing_fields": missing_replacement,
                "resource_data_keys": resource_data_keys(resource),
            }),
        });
    }

    let instance_health_fields = [
        "instance_health",
        "healthy_instance_count",
        "unhealthy_instance_count",
        "in_service_instance_count",
    ];
    let missing_instance_health = missing_fields(resource, &instance_health_fields);
    if !missing_instance_health.is_empty() {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Resilience,
            reason_code: REASON_RES_MISSING_INSTANCE_HEALTH_TELEMETRY.to_string(),
            severity: Severity::Medium,
            message: format!(
                "Auto Scaling group {} is missing instance health telemetry needed to assess replacement behavior",
                resource.resource_id
            ),
            evidence: json!({
                "required_fields": instance_health_fields,
                "missing_fields": missing_instance_health,
                "resource_data_keys": resource_data_keys(resource),
            }),
        });
    }

    let unhealthy = data_i64(&resource.resource_data, "unhealthy_instance_count").unwrap_or(0);
    if unhealthy > 0 {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Resilience,
            reason_code: REASON_RES_UNHEALTHY_INSTANCE_TELEMETRY.to_string(),
            severity: Severity::High,
            message: format!(
                "Auto Scaling group {} has {} unhealthy instance(s) in the latest telemetry",
                resource.resource_id, unhealthy
            ),
            evidence: json!({
                "unhealthy_instance_count": unhealthy,
                "instance_health": resource.resource_data.get("instance_health"),
            }),
        });
    }

    if let Some(az_count) = data_array_len(&resource.resource_data, "availability_zones") {
        if az_count <= 1 {
            findings.push(InventoryFinding {
                resource_id: resource.resource_id.clone(),
                arn: resource.arn.clone(),
                pillar: Pillar::Resilience,
                reason_code: REASON_RES_SINGLE_AZ.to_string(),
                severity: Severity::High,
                message: format!(
                    "Auto Scaling group {} spans {} availability zone(s); an AZ outage takes down all capacity",
                    resource.resource_id, az_count
                ),
                evidence: json!({
                    "availability_zones": resource.resource_data.get("availability_zones")
                }),
            });
        }
    }

    // A load-balanced group with EC2-only health checks keeps instances that
    // fail application health checks in service.
    let health_check_type = data_str(&resource.resource_data, "health_check_type");
    if is_elb_attached(resource) && health_check_type.as_deref() == Some("EC2") {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Resilience,
            reason_code: REASON_RES_ELB_HEALTH_CHECK_EC2_ONLY.to_string(),
            severity: Severity::Medium,
            message: format!(
                "Auto Scaling group {} is attached to a load balancer but uses EC2-only health checks; instances failing application health checks are not replaced",
                resource.resource_id
            ),
            evidence: json!({ "health_check_type": "EC2", "elb_attached": true }),
        });
    }

    let suspended = data_i64(&resource.resource_data, "suspended_process_count").unwrap_or(0);
    if suspended > 0 {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Resilience,
            reason_code: REASON_RES_SUSPENDED_PROCESSES.to_string(),
            severity: Severity::Medium,
            message: format!(
                "Auto Scaling group {} has {} scaling process(es) suspended; unhealthy instances may not be replaced while suspension is in effect",
                resource.resource_id, suspended
            ),
            evidence: json!({ "suspended_process_count": suspended }),
        });
    }

    let desired = data_i64(&resource.resource_data, "desired_capacity");
    let min_size = data_i64(&resource.resource_data, "min_size");
    if let (Some(desired), Some(min)) = (desired, min_size) {
        if desired < min {
            findings.push(InventoryFinding {
                resource_id: resource.resource_id.clone(),
                arn: resource.arn.clone(),
                pillar: Pillar::Resilience,
                reason_code: REASON_RES_DESIRED_BELOW_MIN.to_string(),
                severity: Severity::Low,
                message: format!(
                    "Auto Scaling group {} reports desired capacity {} below min size {}; this is an inconsistent collection snapshot worth re-syncing",
                    resource.resource_id, desired, min
                ),
                evidence: json!({ "desired_capacity": desired, "min_size": min }),
            });
        }
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
                "arn:aws:autoscaling:us-east-1:123456789012:autoScalingGroup:uuid:autoScalingGroupName/{}",
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
            "auto_scaling_group_name": "asg-ok",
            "min_size": 1,
            "max_size": 4,
            "desired_capacity": 2,
            "availability_zones": ["us-east-1a", "us-east-1b"],
            "health_check_type": "ELB",
            "health_check_grace_period": 300,
            "load_balancer_names": [],
            "target_group_arns": ["arn:aws:elasticloadbalancing:us-east-1:123456789012:targetgroup/tg/abc"],
            "uses_launch_template": true,
            "uses_mixed_instances_policy": false,
            "instance_count": 2,
            "enabled_metrics": [
                "GroupDesiredCapacity",
                "GroupInServiceInstances",
                "GroupTotalInstances",
                "GroupPendingInstances",
                "GroupStandbyInstances",
                "GroupTerminatingInstances"
            ],
            "enabled_metric_count": 6,
            "instance_health": [
                {
                    "instance_id": "i-healthy-1",
                    "health_status": "Healthy",
                    "lifecycle_state": "InService"
                },
                {
                    "instance_id": "i-healthy-2",
                    "health_status": "Healthy",
                    "lifecycle_state": "InService"
                }
            ],
            "healthy_instance_count": 2,
            "unhealthy_instance_count": 0,
            "in_service_instance_count": 2,
            "suspended_process_count": 0,
            "telemetry_collection_started_at": "2026-06-10T00:00:00Z",
            "telemetry_collection_completed_at": "2026-06-10T00:00:03Z",
            "telemetry_collection_duration_ms": 3000,
            "telemetry_collection_success_count": 1,
            "telemetry_collection_failure_count": 0,
            "telemetry_collection_error_count": 0,
            "telemetry_collection_errors": [],
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
    fn cost_flags_untagged_group() {
        let r = fixture("asg-untagged", json!({}), healthy_data(), now());
        let report = evaluate_autoscaling_fleet(&[r], Pillar::Cost, now());
        assert_eq!(codes(&report), vec![REASON_COST_NO_TAGS]);
    }

    #[test]
    fn cost_flags_fixed_size_group() {
        let mut data = healthy_data();
        data["min_size"] = json!(3);
        data["max_size"] = json!(3);
        data["desired_capacity"] = json!(3);
        let r = fixture("asg-fixed", json!({"team": "core"}), data, now());
        let report = evaluate_autoscaling_fleet(&[r], Pillar::Cost, now());
        assert_eq!(codes(&report), vec![REASON_COST_FIXED_SIZE]);
    }

    #[test]
    fn cost_does_not_flag_empty_fixed_group() {
        let mut data = healthy_data();
        data["min_size"] = json!(0);
        data["max_size"] = json!(0);
        data["desired_capacity"] = json!(0);
        let r = fixture("asg-empty", json!({"team": "core"}), data, now());
        let report = evaluate_autoscaling_fleet(&[r], Pillar::Cost, now());
        assert!(
            report.findings.is_empty(),
            "unexpected: {:?}",
            report.findings
        );
    }

    #[test]
    fn telemetry_cost_flags_missing_group_metrics() {
        let mut data = healthy_data();
        data.as_object_mut().unwrap().remove("enabled_metrics");
        data.as_object_mut().unwrap().remove("enabled_metric_count");
        let r = fixture("asg-nometrics", json!({"team": "core"}), data, now());
        let report = evaluate_autoscaling_fleet(&[r], Pillar::Cost, now());
        assert!(codes(&report).contains(&REASON_COST_MISSING_GROUP_METRICS_TELEMETRY));
    }

    #[test]
    fn resilience_flags_single_az_as_high() {
        let mut data = healthy_data();
        data["availability_zones"] = json!(["us-east-1a"]);
        let r = fixture("asg-1az", json!({"team": "core"}), data, now());
        let report = evaluate_autoscaling_fleet(&[r], Pillar::Resilience, now());
        assert_eq!(codes(&report), vec![REASON_RES_SINGLE_AZ]);
        assert!(matches!(report.findings[0].severity, Severity::High));
    }

    #[test]
    fn resilience_flags_ec2_health_check_on_load_balanced_group() {
        let mut data = healthy_data();
        data["health_check_type"] = json!("EC2");
        let r = fixture("asg-ec2hc", json!({"team": "core"}), data, now());
        let report = evaluate_autoscaling_fleet(&[r], Pillar::Resilience, now());
        assert_eq!(codes(&report), vec![REASON_RES_ELB_HEALTH_CHECK_EC2_ONLY]);
    }

    #[test]
    fn resilience_allows_ec2_health_check_without_load_balancer() {
        let mut data = healthy_data();
        data["health_check_type"] = json!("EC2");
        data["target_group_arns"] = json!([]);
        data["load_balancer_names"] = json!([]);
        let r = fixture("asg-nolb", json!({"team": "core"}), data, now());
        let report = evaluate_autoscaling_fleet(&[r], Pillar::Resilience, now());
        assert!(
            report.findings.is_empty(),
            "unexpected: {:?}",
            report.findings
        );
    }

    #[test]
    fn resilience_flags_suspended_processes() {
        let mut data = healthy_data();
        data["suspended_process_count"] = json!(2);
        let r = fixture("asg-susp", json!({"team": "core"}), data, now());
        let report = evaluate_autoscaling_fleet(&[r], Pillar::Resilience, now());
        assert_eq!(codes(&report), vec![REASON_RES_SUSPENDED_PROCESSES]);
    }

    #[test]
    fn resilience_flags_desired_below_min_snapshot() {
        let mut data = healthy_data();
        data["desired_capacity"] = json!(0);
        let r = fixture("asg-below", json!({"team": "core"}), data, now());
        let report = evaluate_autoscaling_fleet(&[r], Pillar::Resilience, now());
        assert_eq!(codes(&report), vec![REASON_RES_DESIRED_BELOW_MIN]);
    }

    #[test]
    fn telemetry_resilience_flags_missing_and_unhealthy_instance_health() {
        let mut missing_health = healthy_data();
        missing_health
            .as_object_mut()
            .unwrap()
            .remove("instance_health");
        missing_health
            .as_object_mut()
            .unwrap()
            .remove("healthy_instance_count");
        missing_health
            .as_object_mut()
            .unwrap()
            .remove("unhealthy_instance_count");
        let missing = fixture(
            "asg-no-health-telemetry",
            json!({"team": "core"}),
            missing_health,
            now(),
        );

        let mut unhealthy_data = healthy_data();
        unhealthy_data["healthy_instance_count"] = json!(1);
        unhealthy_data["unhealthy_instance_count"] = json!(1);
        unhealthy_data["instance_health"] = json!([
            {
                "instance_id": "i-healthy",
                "health_status": "Healthy",
                "lifecycle_state": "InService"
            },
            {
                "instance_id": "i-unhealthy",
                "health_status": "Unhealthy",
                "lifecycle_state": "InService"
            }
        ]);
        let unhealthy = fixture(
            "asg-unhealthy",
            json!({"team": "core"}),
            unhealthy_data,
            now(),
        );

        let report = evaluate_autoscaling_fleet(&[missing, unhealthy], Pillar::Resilience, now());
        let codes = codes(&report);
        assert!(codes.contains(&REASON_RES_MISSING_INSTANCE_HEALTH_TELEMETRY));
        assert!(codes.contains(&REASON_RES_UNHEALTHY_INSTANCE_TELEMETRY));
    }

    #[test]
    fn security_flags_legacy_launch_configuration() {
        let mut data = healthy_data();
        data["launch_configuration_name"] = json!("legacy-lc");
        data["uses_launch_template"] = json!(false);
        let r = fixture("asg-lc", json!({"team": "core"}), data, now());
        let report = evaluate_autoscaling_fleet(&[r], Pillar::Security, now());
        assert_eq!(codes(&report), vec![REASON_SEC_LEGACY_LAUNCH_CONFIGURATION]);
    }

    #[test]
    fn security_reports_gap_when_launch_source_missing() {
        let mut data = healthy_data();
        data["uses_launch_template"] = json!(false);
        data["uses_mixed_instances_policy"] = json!(false);
        let r = fixture("asg-gap", json!({"team": "core"}), data, now());
        let report = evaluate_autoscaling_fleet(&[r], Pillar::Security, now());
        assert_eq!(
            codes(&report),
            vec![REASON_SEC_LAUNCH_SOURCE_DATA_NOT_COLLECTED]
        );
    }

    #[test]
    fn security_passes_launch_template_group() {
        let r = fixture("asg-lt", json!({"team": "core"}), healthy_data(), now());
        let report = evaluate_autoscaling_fleet(&[r], Pillar::Security, now());
        assert!(
            report.findings.is_empty(),
            "unexpected: {:?}",
            report.findings
        );
    }

    #[test]
    fn telemetry_security_flags_missing_instance_telemetry_and_collection_errors() {
        let mut missing_instance_telemetry = healthy_data();
        missing_instance_telemetry
            .as_object_mut()
            .unwrap()
            .remove("instance_health");
        let missing = fixture(
            "asg-no-instance-telemetry",
            json!({"team": "core"}),
            missing_instance_telemetry,
            now(),
        );

        let mut collection_error = healthy_data();
        collection_error["telemetry_collection_success_count"] = json!(0);
        collection_error["telemetry_collection_failure_count"] = json!(1);
        collection_error["telemetry_collection_error_count"] = json!(1);
        collection_error["telemetry_collection_errors"] = json!([
            {
                "source": "autoscaling",
                "operation": "DescribeAutoScalingGroups",
                "error": "throttled"
            }
        ]);
        let errored = fixture(
            "asg-collection-error",
            json!({"team": "core"}),
            collection_error,
            now(),
        );

        let report = evaluate_autoscaling_fleet(&[missing, errored], Pillar::Security, now());
        let codes = codes(&report);
        assert!(codes.contains(&REASON_SEC_MISSING_INSTANCE_TELEMETRY));
        assert!(codes.contains(&REASON_SEC_TELEMETRY_COLLECTION_ERRORS));
    }

    #[test]
    fn stale_inventory_is_flagged() {
        let mut r = fixture("asg-stale", json!({"team": "core"}), healthy_data(), now());
        r.last_refreshed = now() - Duration::hours(48);
        let report = evaluate_autoscaling_fleet(&[r], Pillar::Resilience, now());
        assert_eq!(report.stale_resources, 1);
        assert!(codes(&report).contains(&REASON_INV_STALE_DATA));
    }

    #[test]
    fn non_asg_resources_are_skipped_and_not_counted() {
        let mut r = fixture("queue-1", json!({}), json!({}), now());
        r.resource_type = "SqsQueue".to_string();
        let report = evaluate_autoscaling_fleet(&[r], Pillar::Cost, now());
        assert_eq!(report.resources_evaluated, 0);
        assert!(report.findings.is_empty());
    }

    #[test]
    fn healthy_group_passes_all_pillars() {
        let r = fixture("asg-ok", json!({"team": "core"}), healthy_data(), now());
        for pillar in [Pillar::Cost, Pillar::Security, Pillar::Resilience] {
            let report = evaluate_autoscaling_fleet(std::slice::from_ref(&r), pillar, now());
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
    fn telemetry_metadata_gaps_are_reported_for_claimed_pillars() {
        let mut data = healthy_data();
        data.as_object_mut()
            .expect("object")
            .remove("telemetry_collection_duration_ms");
        let r = fixture(
            "asg-no-telemetry-metadata",
            json!({"team": "core"}),
            data,
            now(),
        );

        for pillar in [Pillar::Cost, Pillar::Security, Pillar::Resilience] {
            let report = evaluate_autoscaling_fleet(std::slice::from_ref(&r), pillar, now());
            assert!(
                codes(&report).contains(&"ASG_TEL_MISSING_COLLECTION_METADATA"),
                "expected missing telemetry metadata for {:?}: {:?}",
                pillar,
                report.findings
            );
        }
    }

    #[test]
    fn telemetry_collection_errors_are_reported_for_claimed_pillars() {
        let mut data = healthy_data();
        data["telemetry_collection_success_count"] = json!(0);
        data["telemetry_collection_failure_count"] = json!(1);
        data["telemetry_collection_error_count"] = json!(1);
        data["telemetry_collection_errors"] = json!([
            {
                "source": "autoscaling",
                "operation": "DescribeAutoScalingGroups",
                "error": "throttled"
            }
        ]);
        let r = fixture("asg-telemetry-error", json!({"team": "core"}), data, now());

        for pillar in [Pillar::Cost, Pillar::Security, Pillar::Resilience] {
            let report = evaluate_autoscaling_fleet(std::slice::from_ref(&r), pillar, now());
            assert!(
                codes(&report).contains(&"ASG_TEL_COLLECTION_ERRORS"),
                "expected telemetry collection error for {:?}: {:?}",
                pillar,
                report.findings
            );
        }
    }

    #[test]
    fn cost_flags_missing_capacity_telemetry() {
        let mut data = healthy_data();
        data.as_object_mut()
            .expect("object")
            .remove("desired_capacity");
        let r = fixture("asg-no-capacity", json!({"team": "core"}), data, now());

        let report = evaluate_autoscaling_fleet(&[r], Pillar::Cost, now());
        assert!(
            codes(&report).contains(&"ASG_COST_MISSING_CAPACITY_TELEMETRY"),
            "expected capacity telemetry gap: {:?}",
            report.findings
        );
    }

    #[test]
    fn resilience_flags_missing_replacement_telemetry() {
        let mut data = healthy_data();
        data.as_object_mut()
            .expect("object")
            .remove("health_check_type");
        let r = fixture(
            "asg-no-health-telemetry",
            json!({"team": "core"}),
            data,
            now(),
        );

        let report = evaluate_autoscaling_fleet(&[r], Pillar::Resilience, now());
        assert!(
            codes(&report).contains(&"ASG_RES_MISSING_REPLACEMENT_TELEMETRY"),
            "expected replacement telemetry gap: {:?}",
            report.findings
        );
    }
}

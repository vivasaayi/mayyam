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

// Deterministic EC2 inventory and telemetry evaluators for the cost, security,
// resilience, performance, scalability, and disaster-recovery pillars (roadmap
// rows 01-AWS-CLOUD-00001/00010/00037 plus
// 01-AWS-CLOUD-00002/00011/00020/00029/00038/00047/00056).
//
// Pure domain logic: takes already-collected `aws_resources` rows plus an
// explicit `now`, returns reason-coded findings with the raw evidence that
// triggered each finding. No AWS calls, no database access, no LLM.

use chrono::{DateTime, Utc};
use serde_json::{json, Value};

use crate::models::aws_resource::Model as AwsResourceModel;
use crate::services::aws::inventory::types::{
    check_stale, data_str, has_any_tag, score_pillar, InventoryFinding, Pillar, PillarReport,
    Severity, COST_ALLOCATION_TAG_KEYS, OWNER_TAG_KEYS,
};

// Reason codes are the stable contract for findings; never reuse or rename.
pub const REASON_COST_MISSING_ALLOCATION_TAGS: &str = "EC2_COST_MISSING_ALLOCATION_TAGS";
pub const REASON_COST_STOPPED_INSTANCE: &str = "EC2_COST_STOPPED_INSTANCE_ACCRUING_STORAGE";
pub const REASON_SEC_PUBLIC_IP: &str = "EC2_SEC_PUBLIC_IP_ASSIGNED";
pub const REASON_SEC_MISSING_OWNER_TAG: &str = "EC2_SEC_MISSING_OWNER_TAG";
pub const REASON_RES_MISSING_AZ: &str = "EC2_RES_MISSING_AVAILABILITY_ZONE";
pub const REASON_RES_SINGLE_AZ_CONCENTRATION: &str = "EC2_RES_SINGLE_AZ_CONCENTRATION";
pub const REASON_COST_MISSING_UTILIZATION_TELEMETRY: &str =
    "EC2_COST_MISSING_UTILIZATION_TELEMETRY";
pub const REASON_COST_LOW_UTILIZATION_TELEMETRY: &str = "EC2_COST_LOW_UTILIZATION_TELEMETRY";
pub const REASON_RES_MISSING_STATUS_TELEMETRY: &str = "EC2_RES_MISSING_STATUS_TELEMETRY";
pub const REASON_RES_STATUS_CHECK_FAILURE_TELEMETRY: &str =
    "EC2_RES_STATUS_CHECK_FAILURE_TELEMETRY";
pub const REASON_PERF_MISSING_CORE_TELEMETRY: &str = "EC2_PERF_MISSING_CORE_TELEMETRY";
pub const REASON_PERF_HIGH_CPU_TELEMETRY: &str = "EC2_PERF_HIGH_CPU_TELEMETRY";
pub const REASON_SCALE_MISSING_DEMAND_TELEMETRY: &str = "EC2_SCALE_MISSING_DEMAND_TELEMETRY";
pub const REASON_SCALE_HIGH_CPU_PRESSURE_TELEMETRY: &str = "EC2_SCALE_HIGH_CPU_PRESSURE_TELEMETRY";
pub const REASON_SEC_MISSING_PACKET_TELEMETRY: &str = "EC2_SEC_MISSING_PACKET_TELEMETRY";
pub const REASON_SEC_PUBLIC_PACKET_TRAFFIC_TELEMETRY: &str =
    "EC2_SEC_PUBLIC_PACKET_TRAFFIC_TELEMETRY";
pub const REASON_DR_MISSING_RECOVERY_POINT_TELEMETRY: &str =
    "EC2_DR_MISSING_RECOVERY_POINT_TELEMETRY";
pub const REASON_DR_STALE_RECOVERY_POINT_TELEMETRY: &str = "EC2_DR_STALE_RECOVERY_POINT_TELEMETRY";
pub const REASON_OE_MISSING_TELEMETRY_COLLECTION_METADATA: &str =
    "EC2_OE_MISSING_TELEMETRY_COLLECTION_METADATA";
pub const REASON_OE_TELEMETRY_COLLECTION_ERRORS: &str = "EC2_OE_TELEMETRY_COLLECTION_ERRORS";
pub const REASON_OE_BASIC_MONITORING: &str = "EC2_OE_BASIC_MONITORING";
pub const REASON_INV_STALE_DATA: &str = "EC2_INV_STALE_DATA";

/// Evaluate every EC2 instance in the fleet for one pillar.
pub fn evaluate_ec2_fleet(
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
            Pillar::Performance => evaluate_performance(resource, &mut findings),
            Pillar::Scalability => evaluate_scalability(resource, &mut findings),
            Pillar::DisasterRecovery => evaluate_disaster_recovery(resource, now, &mut findings),
            Pillar::OperationalExcellence => {
                evaluate_operational_excellence(resource, &mut findings)
            }
        }
    }

    if pillar == Pillar::Resilience {
        if let Some(finding) = check_az_concentration(resources) {
            findings.push(finding);
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
    if !has_any_tag(&resource.tags, COST_ALLOCATION_TAG_KEYS) {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Cost,
            reason_code: REASON_COST_MISSING_ALLOCATION_TAGS.to_string(),
            severity: Severity::Medium,
            message: format!(
                "Instance {} has no cost allocation tag (expected one of: {})",
                resource.resource_id,
                COST_ALLOCATION_TAG_KEYS.join(", ")
            ),
            evidence: json!({ "tags": resource.tags }),
        });
    }

    let state = data_str(&resource.resource_data, "state");
    if state.as_deref() == Some("stopped") {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Cost,
            reason_code: REASON_COST_STOPPED_INSTANCE.to_string(),
            severity: Severity::Low,
            message: format!(
                "Instance {} is stopped but still accrues EBS and IP charges; review for termination or snapshot",
                resource.resource_id
            ),
            evidence: json!({ "state": state }),
        });
    }

    match metric_max(resource, "CPUUtilization") {
        None => findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Cost,
            reason_code: REASON_COST_MISSING_UTILIZATION_TELEMETRY.to_string(),
            severity: Severity::Medium,
            message: format!(
                "Instance {} has no CPUUtilization telemetry; cost posture cannot distinguish idle from active capacity",
                resource.resource_id
            ),
            evidence: json!({
                "required_metric": "CPUUtilization",
                "resource_data_keys": resource_data_keys(resource),
            }),
        }),
        Some(max_cpu)
            if state.as_deref() == Some("running") && max_cpu <= LOW_CPU_UTILIZATION_MAX =>
        {
            findings.push(InventoryFinding {
                resource_id: resource.resource_id.clone(),
                arn: resource.arn.clone(),
                pillar: Pillar::Cost,
                reason_code: REASON_COST_LOW_UTILIZATION_TELEMETRY.to_string(),
                severity: Severity::Low,
                message: format!(
                    "Instance {} has low CPUUtilization telemetry; review for rightsizing or scheduling",
                    resource.resource_id
                ),
                evidence: json!({
                    "metric_name": "CPUUtilization",
                    "max": max_cpu,
                    "low_utilization_max": LOW_CPU_UTILIZATION_MAX,
                }),
            });
        }
        _ => {}
    }
}

fn evaluate_security(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    let public_ip = data_str(&resource.resource_data, "public_ip").filter(|ip| !ip.is_empty());
    if let Some(public_ip) = data_str(&resource.resource_data, "public_ip") {
        if !public_ip.is_empty() {
            findings.push(InventoryFinding {
                resource_id: resource.resource_id.clone(),
                arn: resource.arn.clone(),
                pillar: Pillar::Security,
                reason_code: REASON_SEC_PUBLIC_IP.to_string(),
                severity: Severity::High,
                message: format!(
                    "Instance {} has a public IP address assigned; verify it is intentionally internet-facing",
                    resource.resource_id
                ),
                evidence: json!({ "public_ip": public_ip }),
            });
        }
    }

    if !has_any_tag(&resource.tags, OWNER_TAG_KEYS) {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Security,
            reason_code: REASON_SEC_MISSING_OWNER_TAG.to_string(),
            severity: Severity::Low,
            message: format!(
                "Instance {} has no owner/team tag; security findings cannot be routed to an owner",
                resource.resource_id
            ),
            evidence: json!({ "tags": resource.tags }),
        });
    }

    let packet_metrics = ["NetworkPacketsIn", "NetworkPacketsOut"];
    let missing_packet_metrics = missing_metrics(resource, &packet_metrics);
    if !missing_packet_metrics.is_empty() {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Security,
            reason_code: REASON_SEC_MISSING_PACKET_TELEMETRY.to_string(),
            severity: Severity::Medium,
            message: format!(
                "Instance {} is missing EC2 packet telemetry; network exposure cannot be verified from evidence",
                resource.resource_id
            ),
            evidence: json!({
                "required_metrics": packet_metrics,
                "missing_metrics": missing_packet_metrics,
            }),
        });
    }

    if let (Some(public_ip), Some(max_packets_in)) =
        (public_ip, metric_max(resource, "NetworkPacketsIn"))
    {
        if max_packets_in > 0.0 {
            findings.push(InventoryFinding {
                resource_id: resource.resource_id.clone(),
                arn: resource.arn.clone(),
                pillar: Pillar::Security,
                reason_code: REASON_SEC_PUBLIC_PACKET_TRAFFIC_TELEMETRY.to_string(),
                severity: Severity::High,
                message: format!(
                    "Instance {} has a public IP and inbound packet telemetry",
                    resource.resource_id
                ),
                evidence: json!({
                    "public_ip": public_ip,
                    "metric_name": "NetworkPacketsIn",
                    "max": max_packets_in,
                }),
            });
        }
    }
}

fn evaluate_resilience(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    if data_str(&resource.resource_data, "availability_zone")
        .map(|az| az.is_empty())
        .unwrap_or(true)
    {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Resilience,
            reason_code: REASON_RES_MISSING_AZ.to_string(),
            severity: Severity::Medium,
            message: format!(
                "Instance {} has no availability zone recorded; placement resilience cannot be assessed",
                resource.resource_id
            ),
            evidence: json!({ "resource_data": resource.resource_data }),
        });
    }

    let status_metrics = [
        "StatusCheckFailed",
        "StatusCheckFailed_Instance",
        "StatusCheckFailed_System",
    ];
    let observed: Vec<(&str, f64)> = status_metrics
        .iter()
        .filter_map(|metric| metric_max(resource, metric).map(|value| (*metric, value)))
        .collect();

    if observed.is_empty() {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Resilience,
            reason_code: REASON_RES_MISSING_STATUS_TELEMETRY.to_string(),
            severity: Severity::Medium,
            message: format!(
                "Instance {} has no EC2 status check telemetry; reachability resilience cannot be verified",
                resource.resource_id
            ),
            evidence: json!({
                "required_metrics": status_metrics,
                "resource_data_keys": resource_data_keys(resource),
            }),
        });
    } else if let Some((metric_name, max_value)) = observed
        .iter()
        .copied()
        .find(|(_, max_value)| *max_value > 0.0)
    {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Resilience,
            reason_code: REASON_RES_STATUS_CHECK_FAILURE_TELEMETRY.to_string(),
            severity: Severity::High,
            message: format!(
                "Instance {} has non-zero EC2 status check failure telemetry",
                resource.resource_id
            ),
            evidence: json!({
                "metric_name": metric_name,
                "max": max_value,
            }),
        });
    }
}

fn evaluate_performance(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    let required_metrics = [
        "CPUUtilization",
        "NetworkIn",
        "NetworkOut",
        "DiskReadOps",
        "DiskWriteOps",
    ];
    let missing_metrics = missing_metrics(resource, &required_metrics);
    if !missing_metrics.is_empty() {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Performance,
            reason_code: REASON_PERF_MISSING_CORE_TELEMETRY.to_string(),
            severity: Severity::Medium,
            message: format!(
                "Instance {} is missing core EC2 performance telemetry",
                resource.resource_id
            ),
            evidence: json!({
                "required_metrics": required_metrics,
                "missing_metrics": missing_metrics,
            }),
        });
    }

    if let Some(max_cpu) = metric_max(resource, "CPUUtilization") {
        if max_cpu >= HIGH_CPU_UTILIZATION_MIN {
            findings.push(InventoryFinding {
                resource_id: resource.resource_id.clone(),
                arn: resource.arn.clone(),
                pillar: Pillar::Performance,
                reason_code: REASON_PERF_HIGH_CPU_TELEMETRY.to_string(),
                severity: Severity::High,
                message: format!(
                    "Instance {} has high CPUUtilization telemetry",
                    resource.resource_id
                ),
                evidence: json!({
                    "metric_name": "CPUUtilization",
                    "max": max_cpu,
                    "high_utilization_min": HIGH_CPU_UTILIZATION_MIN,
                }),
            });
        }
    }
}

fn evaluate_scalability(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    let required_metrics = ["CPUUtilization", "NetworkIn", "NetworkOut"];
    let missing_metrics = missing_metrics(resource, &required_metrics);
    if !missing_metrics.is_empty() {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Scalability,
            reason_code: REASON_SCALE_MISSING_DEMAND_TELEMETRY.to_string(),
            severity: Severity::Medium,
            message: format!(
                "Instance {} is missing EC2 demand telemetry needed to assess scaling pressure",
                resource.resource_id
            ),
            evidence: json!({
                "required_metrics": required_metrics,
                "missing_metrics": missing_metrics,
            }),
        });
    }

    if data_str(&resource.resource_data, "state").as_deref() == Some("running") {
        if let Some(max_cpu) = metric_max(resource, "CPUUtilization") {
            if max_cpu >= SCALING_CPU_PRESSURE_MIN {
                findings.push(InventoryFinding {
                    resource_id: resource.resource_id.clone(),
                    arn: resource.arn.clone(),
                    pillar: Pillar::Scalability,
                    reason_code: REASON_SCALE_HIGH_CPU_PRESSURE_TELEMETRY.to_string(),
                    severity: Severity::High,
                    message: format!(
                        "Instance {} has high CPUUtilization telemetry; scale-out or rightsizing pressure is likely",
                        resource.resource_id
                    ),
                    evidence: json!({
                        "metric_name": "CPUUtilization",
                        "max": max_cpu,
                        "scaling_cpu_pressure_min": SCALING_CPU_PRESSURE_MIN,
                    }),
                });
            }
        }
    }
}

fn evaluate_disaster_recovery(
    resource: &AwsResourceModel,
    now: DateTime<Utc>,
    findings: &mut Vec<InventoryFinding>,
) {
    match recovery_point_age_hours(resource, now) {
        None => findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::DisasterRecovery,
            reason_code: REASON_DR_MISSING_RECOVERY_POINT_TELEMETRY.to_string(),
            severity: Severity::Medium,
            message: format!(
                "Instance {} has no recovery point telemetry; EC2 restore posture cannot be verified",
                resource.resource_id
            ),
            evidence: json!({
                "expected_fields": [
                    "latest_recovery_point_age_hours",
                    "recovery_point_age_hours",
                    "latest_recovery_point_at"
                ],
                "resource_data_keys": resource_data_keys(resource),
            }),
        }),
        Some(age_hours) if age_hours > RECOVERY_POINT_STALE_AFTER_HOURS => {
            findings.push(InventoryFinding {
                resource_id: resource.resource_id.clone(),
                arn: resource.arn.clone(),
                pillar: Pillar::DisasterRecovery,
                reason_code: REASON_DR_STALE_RECOVERY_POINT_TELEMETRY.to_string(),
                severity: Severity::High,
                message: format!(
                    "Instance {} has stale recovery point telemetry",
                    resource.resource_id
                ),
                evidence: json!({
                    "latest_recovery_point_age_hours": age_hours,
                    "stale_after_hours": RECOVERY_POINT_STALE_AFTER_HOURS,
                }),
            });
        }
        _ => {}
    }
}

fn evaluate_operational_excellence(
    resource: &AwsResourceModel,
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
    let missing_fields: Vec<&str> = required_fields
        .iter()
        .copied()
        .filter(|field| resource.resource_data.get(*field).is_none())
        .collect();
    if !missing_fields.is_empty() {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::OperationalExcellence,
            reason_code: REASON_OE_MISSING_TELEMETRY_COLLECTION_METADATA.to_string(),
            severity: Severity::Medium,
            message: format!(
                "Instance {} is missing EC2 telemetry collection metadata needed for operational runbooks",
                resource.resource_id
            ),
            evidence: json!({
                "required_fields": required_fields,
                "missing_fields": missing_fields,
                "resource_data_keys": resource_data_keys(resource),
            }),
        });
    }

    let error_count = resource
        .resource_data
        .get("telemetry_collection_error_count")
        .and_then(|value| {
            value
                .as_u64()
                .or_else(|| value.as_i64().map(|n| n.max(0) as u64))
        })
        .unwrap_or(0);
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
            pillar: Pillar::OperationalExcellence,
            reason_code: REASON_OE_TELEMETRY_COLLECTION_ERRORS.to_string(),
            severity: Severity::High,
            message: format!(
                "Instance {} has EC2 telemetry collection errors; operators cannot rely on complete evidence",
                resource.resource_id
            ),
            evidence: json!({
                "telemetry_collection_error_count": error_count,
                "telemetry_collection_errors": errors,
            }),
        });
    }

    if data_str(&resource.resource_data, "monitoring_state").as_deref() == Some("disabled") {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::OperationalExcellence,
            reason_code: REASON_OE_BASIC_MONITORING.to_string(),
            severity: Severity::Low,
            message: format!(
                "Instance {} uses basic EC2 monitoring; operational diagnosis has lower-resolution telemetry",
                resource.resource_id
            ),
            evidence: json!({ "monitoring_state": "disabled" }),
        });
    }
}

/// Fleet-level check: two or more running instances all placed in one AZ.
fn check_az_concentration(resources: &[AwsResourceModel]) -> Option<InventoryFinding> {
    let placements: Vec<(&AwsResourceModel, String)> = resources
        .iter()
        .filter(|r| data_str(&r.resource_data, "state").as_deref() == Some("running"))
        .filter_map(|r| {
            data_str(&r.resource_data, "availability_zone")
                .filter(|az| !az.is_empty())
                .map(|az| (r, az))
        })
        .collect();

    if placements.len() < 2 {
        return None;
    }
    let first_az = placements[0].1.clone();
    if !placements.iter().all(|(_, az)| *az == first_az) {
        return None;
    }
    let instance_ids: Vec<&str> = placements
        .iter()
        .map(|(r, _)| r.resource_id.as_str())
        .collect();
    Some(InventoryFinding {
        resource_id: "fleet".to_string(),
        arn: String::new(),
        pillar: Pillar::Resilience,
        reason_code: REASON_RES_SINGLE_AZ_CONCENTRATION.to_string(),
        severity: Severity::Medium,
        message: format!(
            "All {} running instances are placed in availability zone {}; an AZ outage takes down the whole fleet",
            instance_ids.len(),
            first_az
        ),
        evidence: json!({
            "availability_zone": first_az,
            "instance_ids": instance_ids,
        }),
    })
}

const LOW_CPU_UTILIZATION_MAX: f64 = 5.0;
const HIGH_CPU_UTILIZATION_MIN: f64 = 90.0;
const SCALING_CPU_PRESSURE_MIN: f64 = 80.0;
const RECOVERY_POINT_STALE_AFTER_HOURS: f64 = 24.0;

fn missing_metrics(resource: &AwsResourceModel, required_metrics: &[&str]) -> Vec<String> {
    required_metrics
        .iter()
        .filter(|metric| metric_max(resource, metric).is_none())
        .map(|metric| (*metric).to_string())
        .collect()
}

fn metric_max(resource: &AwsResourceModel, metric_name: &str) -> Option<f64> {
    metric_values(&resource.resource_data, metric_name)
        .into_iter()
        .reduce(f64::max)
}

fn recovery_point_age_hours(resource: &AwsResourceModel, now: DateTime<Utc>) -> Option<f64> {
    for key in [
        "latest_recovery_point_age_hours",
        "recovery_point_age_hours",
        "backup_age_hours",
    ] {
        if let Some(value) = resource
            .resource_data
            .get(key)
            .and_then(|value| value.as_f64().or_else(|| value.as_i64().map(|n| n as f64)))
        {
            return Some(value);
        }
    }

    if let Some(value) = resource
        .resource_data
        .pointer("/disaster_recovery/latest_recovery_point_age_hours")
        .and_then(|value| value.as_f64().or_else(|| value.as_i64().map(|n| n as f64)))
    {
        return Some(value);
    }

    for key in ["latest_recovery_point_at", "recovery_point_at"] {
        if let Some(value) = resource
            .resource_data
            .get(key)
            .and_then(|value| value.as_str())
            .and_then(|value| DateTime::parse_from_rfc3339(value).ok())
        {
            return Some((now - value.with_timezone(&Utc)).num_hours() as f64);
        }
    }

    resource
        .resource_data
        .pointer("/disaster_recovery/latest_recovery_point_at")
        .and_then(|value| value.as_str())
        .and_then(|value| DateTime::parse_from_rfc3339(value).ok())
        .map(|value| (now - value.with_timezone(&Utc)).num_hours() as f64)
}

fn metric_values(resource_data: &Value, metric_name: &str) -> Vec<f64> {
    let mut values = Vec::new();
    collect_metric_values(resource_data, metric_name, &mut values);

    if let Some(cloudwatch_metrics) = resource_data.get("cloudwatch_metrics") {
        collect_metric_values(cloudwatch_metrics, metric_name, &mut values);
    }
    if let Some(cloudwatch_metrics) = resource_data.pointer("/telemetry/cloudwatch") {
        collect_metric_values(cloudwatch_metrics, metric_name, &mut values);
    }

    values
}

fn collect_metric_values(value: &Value, metric_name: &str, values: &mut Vec<f64>) {
    if metric_name_matches(value, metric_name) {
        collect_metric_payload_values(value, values);
    }

    if let Some(metrics) = value.get("metrics").and_then(|metrics| metrics.as_array()) {
        for metric in metrics {
            if metric_name_matches(metric, metric_name) {
                collect_metric_payload_values(metric, values);
            }
        }
    }

    if let Some(metrics) = value.get("metrics").and_then(|metrics| metrics.as_object()) {
        if let Some(metric) = metrics.get(metric_name) {
            collect_metric_payload_values(metric, values);
        }
    }

    if let Some(metric) = value.get(metric_name) {
        collect_metric_payload_values(metric, values);
    }
}

fn metric_name_matches(value: &Value, metric_name: &str) -> bool {
    value
        .get("metric_name")
        .or_else(|| value.get("MetricName"))
        .and_then(|name| name.as_str())
        .map(|name| name == metric_name)
        .unwrap_or(false)
}

fn collect_metric_payload_values(value: &Value, values: &mut Vec<f64>) {
    for key in ["value", "latest", "max", "average", "Value"] {
        if let Some(number) = value.get(key).and_then(|number| number.as_f64()) {
            values.push(number);
        }
    }

    if let Some(datapoints) = value
        .get("datapoints")
        .or_else(|| value.get("Datapoints"))
        .and_then(|datapoints| datapoints.as_array())
    {
        for datapoint in datapoints {
            for key in ["value", "Value", "average", "Average", "maximum", "Maximum"] {
                if let Some(number) = datapoint.get(key).and_then(|number| number.as_f64()) {
                    values.push(number);
                    break;
                }
            }
        }
    }
}

fn resource_data_keys(resource: &AwsResourceModel) -> Vec<String> {
    resource
        .resource_data
        .as_object()
        .map(|object| object.keys().cloned().collect())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::aws::inventory::types::DEFAULT_STALE_AFTER_HOURS;
    use chrono::Duration;
    use serde_json::Value;
    use uuid::Uuid;

    fn fixture(
        resource_id: &str,
        tags: Value,
        resource_data: Value,
        refreshed_hours_ago: i64,
        now: DateTime<Utc>,
    ) -> AwsResourceModel {
        let refreshed = now - Duration::hours(refreshed_hours_ago);
        AwsResourceModel {
            id: Uuid::new_v4(),
            sync_id: None,
            account_id: "123456789012".to_string(),
            profile: None,
            region: "us-east-1".to_string(),
            resource_type: "EC2Instance".to_string(),
            resource_id: resource_id.to_string(),
            arn: format!(
                "arn:aws:ec2:us-east-1:123456789012:instance/{}",
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

    fn reason_codes(report: &PillarReport) -> Vec<&str> {
        report
            .findings
            .iter()
            .map(|f| f.reason_code.as_str())
            .collect()
    }

    fn metric(metric_name: &str, values: &[f64]) -> Value {
        json!({
            "metric_name": metric_name,
            "datapoints": values.iter().map(|value| json!({ "value": value })).collect::<Vec<_>>()
        })
    }

    #[test]
    fn cost_flags_missing_allocation_tags_and_stopped_instance() {
        let r = fixture(
            "i-untagged",
            json!({}),
            json!({"state": "stopped", "availability_zone": "us-east-1a"}),
            1,
            now(),
        );
        let report = evaluate_ec2_fleet(&[r], Pillar::Cost, now());
        let codes = reason_codes(&report);
        assert!(codes.contains(&REASON_COST_MISSING_ALLOCATION_TAGS));
        assert!(codes.contains(&REASON_COST_STOPPED_INSTANCE));
        // Evidence preserved: raw tags object for the tag finding.
        let tag_finding = report
            .findings
            .iter()
            .find(|f| f.reason_code == REASON_COST_MISSING_ALLOCATION_TAGS)
            .unwrap();
        assert_eq!(tag_finding.evidence["tags"], json!({}));
        assert!(report.score < 100);
    }

    #[test]
    fn cost_passes_for_tagged_running_instance() {
        let r = fixture(
            "i-good",
            json!({"Team": "payments", "cost-center": "cc-42"}),
            json!({
                "state": "running",
                "availability_zone": "us-east-1a",
                "cloudwatch_metrics": {
                    "metrics": [
                        metric("CPUUtilization", &[35.0, 42.0])
                    ]
                }
            }),
            1,
            now(),
        );
        let report = evaluate_ec2_fleet(&[r], Pillar::Cost, now());
        assert!(
            report.findings.is_empty(),
            "unexpected: {:?}",
            report.findings
        );
        assert_eq!(report.score, 100);
        assert_eq!(report.resources_evaluated, 1);
        assert_eq!(report.stale_resources, 0);
    }

    #[test]
    fn security_flags_public_ip_as_high_and_missing_owner_as_low() {
        let r = fixture(
            "i-exposed",
            json!({}),
            json!({"state": "running", "public_ip": "54.0.0.1", "availability_zone": "us-east-1a"}),
            1,
            now(),
        );
        let report = evaluate_ec2_fleet(&[r], Pillar::Security, now());
        let public = report
            .findings
            .iter()
            .find(|f| f.reason_code == REASON_SEC_PUBLIC_IP)
            .expect("public ip finding");
        assert_eq!(public.severity, Severity::High);
        assert_eq!(public.evidence["public_ip"], json!("54.0.0.1"));
        let owner = report
            .findings
            .iter()
            .find(|f| f.reason_code == REASON_SEC_MISSING_OWNER_TAG)
            .expect("owner tag finding");
        assert_eq!(owner.severity, Severity::Low);
    }

    #[test]
    fn security_passes_for_private_owned_instance() {
        let r = fixture(
            "i-private",
            json!([{"Key": "Owner", "Value": "sre"}]),
            json!({
                "state": "running",
                "private_ip": "10.0.0.5",
                "availability_zone": "us-east-1a",
                "cloudwatch_metrics": {
                    "metrics": [
                        metric("NetworkPacketsIn", &[0.0]),
                        metric("NetworkPacketsOut", &[0.0])
                    ]
                }
            }),
            1,
            now(),
        );
        let report = evaluate_ec2_fleet(&[r], Pillar::Security, now());
        assert!(
            report.findings.is_empty(),
            "unexpected: {:?}",
            report.findings
        );
    }

    #[test]
    fn resilience_flags_missing_availability_zone() {
        let r = fixture(
            "i-noaz",
            json!({"owner": "sre"}),
            json!({
                "state": "running",
                "cloudwatch_metrics": {
                    "metrics": [
                        metric("StatusCheckFailed", &[0.0]),
                        metric("StatusCheckFailed_Instance", &[0.0]),
                        metric("StatusCheckFailed_System", &[0.0])
                    ]
                }
            }),
            1,
            now(),
        );
        let report = evaluate_ec2_fleet(&[r], Pillar::Resilience, now());
        assert_eq!(reason_codes(&report), vec![REASON_RES_MISSING_AZ]);
    }

    #[test]
    fn resilience_flags_single_az_concentration_for_running_fleet() {
        let a = fixture(
            "i-a",
            json!({"owner": "sre"}),
            json!({
                "state": "running",
                "availability_zone": "us-east-1a",
                "cloudwatch_metrics": {
                    "metrics": [
                        metric("StatusCheckFailed", &[0.0]),
                        metric("StatusCheckFailed_Instance", &[0.0]),
                        metric("StatusCheckFailed_System", &[0.0])
                    ]
                }
            }),
            1,
            now(),
        );
        let b = fixture(
            "i-b",
            json!({"owner": "sre"}),
            json!({
                "state": "running",
                "availability_zone": "us-east-1a",
                "cloudwatch_metrics": {
                    "metrics": [
                        metric("StatusCheckFailed", &[0.0]),
                        metric("StatusCheckFailed_Instance", &[0.0]),
                        metric("StatusCheckFailed_System", &[0.0])
                    ]
                }
            }),
            1,
            now(),
        );
        let report = evaluate_ec2_fleet(&[a, b], Pillar::Resilience, now());
        let fleet = report
            .findings
            .iter()
            .find(|f| f.reason_code == REASON_RES_SINGLE_AZ_CONCENTRATION)
            .expect("fleet concentration finding");
        assert_eq!(fleet.evidence["availability_zone"], json!("us-east-1a"));
        assert_eq!(fleet.evidence["instance_ids"], json!(["i-a", "i-b"]));
    }

    #[test]
    fn resilience_accepts_multi_az_fleet() {
        let a = fixture(
            "i-a",
            json!({"owner": "sre"}),
            json!({
                "state": "running",
                "availability_zone": "us-east-1a",
                "cloudwatch_metrics": {
                    "metrics": [
                        metric("StatusCheckFailed", &[0.0]),
                        metric("StatusCheckFailed_Instance", &[0.0]),
                        metric("StatusCheckFailed_System", &[0.0])
                    ]
                }
            }),
            1,
            now(),
        );
        let b = fixture(
            "i-b",
            json!({"owner": "sre"}),
            json!({
                "state": "running",
                "availability_zone": "us-east-1b",
                "cloudwatch_metrics": {
                    "metrics": [
                        metric("StatusCheckFailed", &[0.0]),
                        metric("StatusCheckFailed_Instance", &[0.0]),
                        metric("StatusCheckFailed_System", &[0.0])
                    ]
                }
            }),
            1,
            now(),
        );
        let report = evaluate_ec2_fleet(&[a, b], Pillar::Resilience, now());
        assert!(
            report.findings.is_empty(),
            "unexpected: {:?}",
            report.findings
        );
        assert_eq!(report.score, 100);
    }

    #[test]
    fn stale_inventory_is_reported_as_failure_path_for_every_pillar() {
        let r = fixture(
            "i-stale",
            json!({"owner": "sre", "project": "mayyam"}),
            json!({"state": "running", "availability_zone": "us-east-1a", "private_ip": "10.0.0.9"}),
            48,
            now(),
        );
        for pillar in [Pillar::Cost, Pillar::Security, Pillar::Resilience] {
            let report = evaluate_ec2_fleet(std::slice::from_ref(&r), pillar, now());
            assert_eq!(report.stale_resources, 1, "pillar {:?}", pillar);
            let stale = report
                .findings
                .iter()
                .find(|f| f.reason_code == REASON_INV_STALE_DATA)
                .unwrap_or_else(|| panic!("stale finding missing for {:?}", pillar));
            assert_eq!(stale.evidence["age_hours"], json!(48));
            assert_eq!(
                stale.evidence["stale_after_hours"],
                json!(DEFAULT_STALE_AFTER_HOURS)
            );
        }
    }

    #[test]
    fn ec2_telemetry_cost_flags_low_cpu_utilization() {
        let r = fixture(
            "i-idle",
            json!({"cost-center": "cc-42", "owner": "sre"}),
            json!({
                "state": "running",
                "availability_zone": "us-east-1a",
                "cloudwatch_metrics": {
                    "metrics": [
                        metric("CPUUtilization", &[1.2, 2.4, 3.0])
                    ]
                }
            }),
            1,
            now(),
        );

        let report = evaluate_ec2_fleet(&[r], Pillar::Cost, now());
        let idle = report
            .findings
            .iter()
            .find(|f| f.reason_code == REASON_COST_LOW_UTILIZATION_TELEMETRY)
            .expect("low utilization telemetry finding");
        assert_eq!(idle.severity, Severity::Low);
        assert_eq!(idle.evidence["metric_name"], json!("CPUUtilization"));
        assert_eq!(idle.evidence["max"], json!(3.0));
    }

    #[test]
    fn ec2_telemetry_resilience_flags_status_check_failures() {
        let r = fixture(
            "i-status-failed",
            json!({"owner": "sre"}),
            json!({
                "state": "running",
                "availability_zone": "us-east-1a",
                "cloudwatch_metrics": {
                    "metrics": [
                        metric("StatusCheckFailed", &[0.0, 1.0]),
                        metric("StatusCheckFailed_Instance", &[0.0]),
                        metric("StatusCheckFailed_System", &[0.0])
                    ]
                }
            }),
            1,
            now(),
        );

        let report = evaluate_ec2_fleet(&[r], Pillar::Resilience, now());
        let status = report
            .findings
            .iter()
            .find(|f| f.reason_code == REASON_RES_STATUS_CHECK_FAILURE_TELEMETRY)
            .expect("status check telemetry finding");
        assert_eq!(status.severity, Severity::High);
        assert_eq!(status.evidence["metric_name"], json!("StatusCheckFailed"));
        assert_eq!(status.evidence["max"], json!(1.0));
    }

    #[test]
    fn ec2_telemetry_performance_requires_core_metrics_and_flags_high_cpu() {
        let missing = fixture(
            "i-missing-telemetry",
            json!({"owner": "sre"}),
            json!({
                "state": "running",
                "availability_zone": "us-east-1a",
                "cloudwatch_metrics": {
                    "metrics": [
                        metric("CPUUtilization", &[35.0])
                    ]
                }
            }),
            1,
            now(),
        );
        let hot = fixture(
            "i-hot",
            json!({"owner": "sre"}),
            json!({
                "state": "running",
                "availability_zone": "us-east-1a",
                "cloudwatch_metrics": {
                    "metrics": [
                        metric("CPUUtilization", &[91.0, 94.0]),
                        metric("NetworkIn", &[1024.0]),
                        metric("NetworkOut", &[2048.0]),
                        metric("DiskReadOps", &[10.0]),
                        metric("DiskWriteOps", &[12.0])
                    ]
                }
            }),
            1,
            now(),
        );

        let report = evaluate_ec2_fleet(&[missing, hot], Pillar::Performance, now());
        let codes = reason_codes(&report);
        assert!(codes.contains(&REASON_PERF_MISSING_CORE_TELEMETRY));
        assert!(codes.contains(&REASON_PERF_HIGH_CPU_TELEMETRY));
    }

    #[test]
    fn ec2_telemetry_scalability_requires_demand_metrics_and_flags_cpu_pressure() {
        let missing = fixture(
            "i-scale-gap",
            json!({"owner": "sre"}),
            json!({
                "state": "running",
                "availability_zone": "us-east-1a",
                "cloudwatch_metrics": {
                    "metrics": [
                        metric("CPUUtilization", &[42.0])
                    ]
                }
            }),
            1,
            now(),
        );
        let pressured = fixture(
            "i-scale-hot",
            json!({"owner": "sre"}),
            json!({
                "state": "running",
                "availability_zone": "us-east-1a",
                "cloudwatch_metrics": {
                    "metrics": [
                        metric("CPUUtilization", &[83.0, 91.0]),
                        metric("NetworkIn", &[2048.0]),
                        metric("NetworkOut", &[1024.0])
                    ]
                }
            }),
            1,
            now(),
        );

        let report = evaluate_ec2_fleet(&[missing, pressured], Pillar::Scalability, now());
        let codes = reason_codes(&report);
        assert!(codes.contains(&REASON_SCALE_MISSING_DEMAND_TELEMETRY));
        assert!(codes.contains(&REASON_SCALE_HIGH_CPU_PRESSURE_TELEMETRY));
    }

    #[test]
    fn ec2_telemetry_security_requires_packet_metrics_and_flags_public_traffic() {
        let missing = fixture(
            "i-packet-gap",
            json!({"owner": "sre"}),
            json!({
                "state": "running",
                "private_ip": "10.0.0.5",
                "availability_zone": "us-east-1a",
                "cloudwatch_metrics": {
                    "metrics": [
                        metric("CPUUtilization", &[22.0])
                    ]
                }
            }),
            1,
            now(),
        );
        let exposed = fixture(
            "i-public-packets",
            json!({"owner": "sre"}),
            json!({
                "state": "running",
                "public_ip": "54.0.0.1",
                "availability_zone": "us-east-1a",
                "cloudwatch_metrics": {
                    "metrics": [
                        metric("NetworkPacketsIn", &[0.0, 1250.0]),
                        metric("NetworkPacketsOut", &[400.0])
                    ]
                }
            }),
            1,
            now(),
        );

        let report = evaluate_ec2_fleet(&[missing, exposed], Pillar::Security, now());
        let codes = reason_codes(&report);
        assert!(codes.contains(&REASON_SEC_MISSING_PACKET_TELEMETRY));
        assert!(codes.contains(&REASON_SEC_PUBLIC_PACKET_TRAFFIC_TELEMETRY));
    }

    #[test]
    fn ec2_telemetry_disaster_recovery_requires_recent_recovery_point_evidence() {
        let missing = fixture(
            "i-no-recovery-evidence",
            json!({"owner": "sre"}),
            json!({
                "state": "running",
                "availability_zone": "us-east-1a",
                "cloudwatch_metrics": {
                    "metrics": [
                        metric("StatusCheckFailed", &[0.0])
                    ]
                }
            }),
            1,
            now(),
        );
        let stale = fixture(
            "i-stale-recovery",
            json!({"owner": "sre"}),
            json!({
                "state": "running",
                "availability_zone": "us-east-1b",
                "latest_recovery_point_age_hours": 72
            }),
            1,
            now(),
        );

        let report = evaluate_ec2_fleet(&[missing, stale], Pillar::DisasterRecovery, now());
        let codes = reason_codes(&report);
        assert!(codes.contains(&REASON_DR_MISSING_RECOVERY_POINT_TELEMETRY));
        assert!(codes.contains(&REASON_DR_STALE_RECOVERY_POINT_TELEMETRY));
    }

    #[test]
    fn ec2_telemetry_operational_excellence_flags_collection_errors_and_basic_monitoring() {
        let missing_metadata = fixture(
            "i-no-collection-metadata",
            json!({"owner": "sre"}),
            json!({
                "state": "running",
                "availability_zone": "us-east-1a",
                "monitoring_state": "enabled",
                "cloudwatch_metrics": {
                    "metrics": [
                        metric("CPUUtilization", &[24.0])
                    ]
                }
            }),
            1,
            now(),
        );
        let collection_error = fixture(
            "i-collection-error",
            json!({"owner": "sre"}),
            json!({
                "state": "running",
                "availability_zone": "us-east-1a",
                "monitoring_state": "disabled",
                "telemetry_collection_started_at": "2026-06-10T00:00:00Z",
                "telemetry_collection_completed_at": "2026-06-10T00:00:03Z",
                "telemetry_collection_duration_ms": 3000,
                "telemetry_collection_error_count": 1,
                "telemetry_collection_errors": [
                    {
                        "source": "cloudwatch",
                        "operation": "GetMetricData",
                        "error": "throttled"
                    }
                ]
            }),
            1,
            now(),
        );

        let report = evaluate_ec2_fleet(
            &[missing_metadata, collection_error],
            Pillar::OperationalExcellence,
            now(),
        );
        let codes = reason_codes(&report);
        assert!(codes.contains(&REASON_OE_MISSING_TELEMETRY_COLLECTION_METADATA));
        assert!(codes.contains(&REASON_OE_TELEMETRY_COLLECTION_ERRORS));
        assert!(codes.contains(&REASON_OE_BASIC_MONITORING));
    }

    #[test]
    fn ec2_telemetry_operational_excellence_passes_for_successful_collection() {
        let resource = fixture(
            "i-collection-good",
            json!({"owner": "sre"}),
            json!({
                "state": "running",
                "availability_zone": "us-east-1a",
                "monitoring_state": "enabled",
                "telemetry_collection_started_at": "2026-06-10T00:00:00Z",
                "telemetry_collection_completed_at": "2026-06-10T00:00:03Z",
                "telemetry_collection_duration_ms": 3000,
                "telemetry_collection_success_count": 1,
                "telemetry_collection_failure_count": 0,
                "telemetry_collection_error_count": 0,
                "telemetry_collection_errors": [],
                "cloudwatch_metric_count": 3,
                "cloudwatch_metrics": {
                    "metrics": [
                        metric("CPUUtilization", &[24.0]),
                        metric("NetworkIn", &[2048.0]),
                        metric("NetworkOut", &[1024.0])
                    ]
                }
            }),
            1,
            now(),
        );

        let report = evaluate_ec2_fleet(&[resource], Pillar::OperationalExcellence, now());
        assert!(
            report.findings.is_empty(),
            "unexpected: {:?}",
            report.findings
        );
    }
}

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

// Deterministic Lambda inventory evaluators for the cost, security, and
// resilience pillars (roadmap rows 01-AWS-CLOUD-00127/00136/00163).
//
// Pure domain logic over collected `aws_resources` rows; no AWS calls,
// no database access, no LLM. Evaluates fields persisted by
// lambda_control_plane: runtime, memory_size, timeout, architectures.

use chrono::{DateTime, Utc};
use serde::Serialize;
use serde_json::{json, Value};
use std::collections::BTreeSet;

use crate::models::aws_resource::Model as AwsResourceModel;
use crate::services::aws::inventory::types::{
    check_stale, has_any_tag, score_pillar, InventoryFinding, Pillar, PillarReport, Severity,
    COST_ALLOCATION_TAG_KEYS, OWNER_TAG_KEYS,
};

// Reason codes are the stable contract for findings; never reuse or rename.
pub const REASON_COST_MISSING_ALLOCATION_TAGS: &str = "LAMBDA_COST_MISSING_ALLOCATION_TAGS";
pub const REASON_COST_X86_ONLY_ARCHITECTURE: &str = "LAMBDA_COST_X86_ONLY_ARCHITECTURE";
pub const REASON_COST_MISSING_TELEMETRY_COLLECTION_METADATA: &str =
    "LAMBDA_COST_MISSING_TELEMETRY_COLLECTION_METADATA";
pub const REASON_COST_TELEMETRY_COLLECTION_ERRORS: &str = "LAMBDA_COST_TELEMETRY_COLLECTION_ERRORS";
pub const REASON_COST_MISSING_CLOUDWATCH_TELEMETRY: &str =
    "LAMBDA_COST_MISSING_CLOUDWATCH_TELEMETRY";
pub const REASON_COST_NO_INVOCATIONS_TELEMETRY: &str = "LAMBDA_COST_NO_INVOCATIONS_TELEMETRY";
pub const REASON_COST_ERROR_OR_THROTTLE_TELEMETRY: &str = "LAMBDA_COST_ERROR_OR_THROTTLE_TELEMETRY";
pub const REASON_SEC_DEPRECATED_RUNTIME: &str = "LAMBDA_SEC_DEPRECATED_RUNTIME";
pub const REASON_SEC_MISSING_OWNER_TAG: &str = "LAMBDA_SEC_MISSING_OWNER_TAG";
pub const REASON_RES_MISSING_CONFIG_DATA: &str = "LAMBDA_RES_MISSING_CONFIG_DATA";
pub const REASON_INV_STALE_DATA: &str = "LAMBDA_INV_STALE_DATA";

/// Runtimes AWS has deprecated (no more security patches). Kept as an
/// explicit deterministic list; extend when AWS announces new deprecations.
pub const DEPRECATED_RUNTIMES: &[&str] = &[
    "python2.7",
    "python3.6",
    "python3.7",
    "nodejs10.x",
    "nodejs12.x",
    "nodejs14.x",
    "nodejs16.x",
    "dotnetcore2.1",
    "dotnetcore3.1",
    "dotnet5.0",
    "ruby2.5",
    "ruby2.7",
    "go1.x",
    "java8",
    "provided",
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum LambdaPostureStatus {
    Pass,
    Fail,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LambdaPostureRule {
    pub rule_id: &'static str,
    pub status: LambdaPostureStatus,
    pub reason_codes: Vec<&'static str>,
    pub affected_resources: Vec<String>,
    pub suppression_supported: bool,
    pub assignment_supported: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LambdaCostPostureSummary {
    pub status: LambdaPostureStatus,
    pub rules_evaluated: usize,
    pub rules_failed: usize,
    pub affected_resources: Vec<String>,
    pub rules: Vec<LambdaPostureRule>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LambdaCostTelemetrySummary {
    pub workflow_id: &'static str,
    pub read_only_mode: bool,
    pub freshness_required: bool,
    pub telemetry_collection_required: bool,
    pub cloudwatch_namespace: &'static str,
    pub cloudwatch_dimension: &'static str,
    pub required_metrics: Vec<&'static str>,
    pub export_formats: Vec<&'static str>,
    pub missing_data_reason_codes: Vec<String>,
    pub evidence_reason_codes: Vec<String>,
    pub stale_data_blocks_delivery: bool,
    pub telemetry_quality_score: u8,
}

/// Evaluate every Lambda function in the fleet for one pillar.
pub fn evaluate_lambda_fleet(
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
            // Pillars without checks for this service yet produce no findings.
            _ => {}
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

pub fn lambda_cost_posture_summary(report: &PillarReport) -> LambdaCostPostureSummary {
    let rules = vec![
        lambda_cost_posture_rule(
            report,
            "lambda-cost-inventory-freshness",
            &[REASON_INV_STALE_DATA],
        ),
        lambda_cost_posture_rule(
            report,
            "lambda-cost-telemetry-collection-metadata-present",
            &[REASON_COST_MISSING_TELEMETRY_COLLECTION_METADATA],
        ),
        lambda_cost_posture_rule(
            report,
            "lambda-cost-telemetry-collection-errors-clear",
            &[REASON_COST_TELEMETRY_COLLECTION_ERRORS],
        ),
        lambda_cost_posture_rule(
            report,
            "lambda-cost-cloudwatch-metrics-present",
            &[REASON_COST_MISSING_CLOUDWATCH_TELEMETRY],
        ),
        lambda_cost_posture_rule(
            report,
            "lambda-cost-allocation-tags-present",
            &[REASON_COST_MISSING_ALLOCATION_TAGS],
        ),
        lambda_cost_posture_rule(
            report,
            "lambda-cost-arm64-review",
            &[REASON_COST_X86_ONLY_ARCHITECTURE],
        ),
        lambda_cost_posture_rule(
            report,
            "lambda-cost-unused-or-failing-functions-reviewed",
            &[
                REASON_COST_NO_INVOCATIONS_TELEMETRY,
                REASON_COST_ERROR_OR_THROTTLE_TELEMETRY,
            ],
        ),
    ];
    let rules_failed = rules
        .iter()
        .filter(|rule| rule.status == LambdaPostureStatus::Fail)
        .count();
    let affected_resources = sorted_unique_resources(
        rules
            .iter()
            .flat_map(|rule| rule.affected_resources.iter().cloned()),
    );

    LambdaCostPostureSummary {
        status: if rules_failed == 0 {
            LambdaPostureStatus::Pass
        } else {
            LambdaPostureStatus::Fail
        },
        rules_evaluated: rules.len(),
        rules_failed,
        affected_resources,
        rules,
    }
}

pub fn lambda_cost_telemetry_summary(report: &PillarReport) -> LambdaCostTelemetrySummary {
    let missing_data_reason_codes = sorted_unique_reasons(
        report
            .findings
            .iter()
            .filter(|finding| {
                matches!(
                    finding.reason_code.as_str(),
                    REASON_COST_MISSING_TELEMETRY_COLLECTION_METADATA
                        | REASON_COST_TELEMETRY_COLLECTION_ERRORS
                        | REASON_COST_MISSING_CLOUDWATCH_TELEMETRY
                        | REASON_INV_STALE_DATA
                )
            })
            .map(|finding| finding.reason_code.clone()),
    );
    let evidence_reason_codes = sorted_unique_reasons(
        report
            .findings
            .iter()
            .map(|finding| finding.reason_code.clone()),
    );
    let stale_data_blocks_delivery = report.stale_resources > 0
        || report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_COST_TELEMETRY_COLLECTION_ERRORS);

    LambdaCostTelemetrySummary {
        workflow_id: "lambda_cost_telemetry",
        read_only_mode: true,
        freshness_required: true,
        telemetry_collection_required: true,
        cloudwatch_namespace: "AWS/Lambda",
        cloudwatch_dimension: "FunctionName",
        required_metrics: lambda_cost_metric_names().to_vec(),
        export_formats: vec!["json"],
        missing_data_reason_codes,
        evidence_reason_codes,
        stale_data_blocks_delivery,
        telemetry_quality_score: report.score,
    }
}

fn evaluate_cost(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    evaluate_cost_telemetry(resource, findings);

    if !has_any_tag(&resource.tags, COST_ALLOCATION_TAG_KEYS) {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Cost,
            reason_code: REASON_COST_MISSING_ALLOCATION_TAGS.to_string(),
            severity: Severity::Medium,
            message: format!(
                "Function {} has no cost allocation tag (expected one of: {})",
                resource.resource_id,
                COST_ALLOCATION_TAG_KEYS.join(", ")
            ),
            evidence: json!({ "tags": resource.tags }),
        });
    }

    // arm64 (Graviton) is cheaper per GB-second; x86_64-only functions are a
    // deterministic savings opportunity worth review.
    let architectures: Vec<String> = resource
        .resource_data
        .get("architectures")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|a| a.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();
    if !architectures.is_empty() && architectures.iter().all(|a| a == "x86_64") {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Cost,
            reason_code: REASON_COST_X86_ONLY_ARCHITECTURE.to_string(),
            severity: Severity::Low,
            message: format!(
                "Function {} runs only on x86_64; evaluate arm64 (Graviton) for lower per-GB-second cost",
                resource.resource_id
            ),
            evidence: json!({ "architectures": architectures }),
        });
    }
}

fn evaluate_cost_telemetry(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    let required_fields = [
        "telemetry_collection_started_at",
        "telemetry_collection_completed_at",
        "telemetry_collection_duration_ms",
        "telemetry_collection_success_count",
        "telemetry_collection_failure_count",
        "telemetry_collection_error_count",
    ];
    let missing_fields = missing_fields(resource, &required_fields);
    if !missing_fields.is_empty() {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Cost,
            reason_code: REASON_COST_MISSING_TELEMETRY_COLLECTION_METADATA.to_string(),
            severity: Severity::Medium,
            message: format!(
                "Function {} is missing Lambda telemetry collection metadata needed to trust cost evidence",
                resource.resource_id
            ),
            evidence: json!({
                "required_fields": required_fields,
                "missing_fields": missing_fields,
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
            pillar: Pillar::Cost,
            reason_code: REASON_COST_TELEMETRY_COLLECTION_ERRORS.to_string(),
            severity: Severity::High,
            message: format!(
                "Function {} has Lambda telemetry collection errors; cost posture may be incomplete",
                resource.resource_id
            ),
            evidence: json!({
                "telemetry_collection_error_count": error_count,
                "telemetry_collection_errors": errors,
            }),
        });
    }

    let missing_metrics = missing_metrics(resource, &lambda_cost_metric_names());
    if !missing_metrics.is_empty() {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Cost,
            reason_code: REASON_COST_MISSING_CLOUDWATCH_TELEMETRY.to_string(),
            severity: Severity::Medium,
            message: format!(
                "Function {} is missing Lambda CloudWatch cost telemetry for {}",
                resource.resource_id,
                missing_metrics.join(", ")
            ),
            evidence: json!({
                "required_metrics": lambda_cost_metric_names(),
                "missing_metrics": missing_metrics,
                "cloudwatch_metric_names": resource.resource_data.get("cloudwatch_metric_names"),
            }),
        });
    }

    if metric_max(resource, "Invocations") == Some(0.0) {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Cost,
            reason_code: REASON_COST_NO_INVOCATIONS_TELEMETRY.to_string(),
            severity: Severity::Low,
            message: format!(
                "Function {} has zero observed invocations in Lambda telemetry; review it as an unused-cost candidate",
                resource.resource_id
            ),
            evidence: json!({ "invocations_max": 0.0 }),
        });
    }

    let errors_max = metric_max(resource, "Errors").unwrap_or(0.0);
    let throttles_max = metric_max(resource, "Throttles").unwrap_or(0.0);
    if errors_max > 0.0 || throttles_max > 0.0 {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Cost,
            reason_code: REASON_COST_ERROR_OR_THROTTLE_TELEMETRY.to_string(),
            severity: Severity::Low,
            message: format!(
                "Function {} has Lambda error or throttle telemetry that can waste retry and duration spend",
                resource.resource_id
            ),
            evidence: json!({
                "errors_max": errors_max,
                "throttles_max": throttles_max,
            }),
        });
    }
}

fn evaluate_security(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    if let Some(runtime) = resource
        .resource_data
        .get("runtime")
        .and_then(|v| v.as_str())
    {
        if DEPRECATED_RUNTIMES.contains(&runtime) {
            findings.push(InventoryFinding {
                resource_id: resource.resource_id.clone(),
                arn: resource.arn.clone(),
                pillar: Pillar::Security,
                reason_code: REASON_SEC_DEPRECATED_RUNTIME.to_string(),
                severity: Severity::High,
                message: format!(
                    "Function {} uses deprecated runtime {}; it no longer receives security patches",
                    resource.resource_id, runtime
                ),
                evidence: json!({ "runtime": runtime }),
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
                "Function {} has no owner/team tag; security findings cannot be routed to an owner",
                resource.resource_id
            ),
            evidence: json!({ "tags": resource.tags }),
        });
    }
}

fn evaluate_resilience(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    let timeout = resource
        .resource_data
        .get("timeout")
        .and_then(|v| v.as_i64());
    let memory_size = resource
        .resource_data
        .get("memory_size")
        .and_then(|v| v.as_i64());
    if timeout.is_none() || memory_size.is_none() {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Resilience,
            reason_code: REASON_RES_MISSING_CONFIG_DATA.to_string(),
            severity: Severity::Medium,
            message: format!(
                "Function {} is missing timeout or memory configuration in inventory; resilience limits cannot be assessed",
                resource.resource_id
            ),
            evidence: json!({ "timeout": timeout, "memory_size": memory_size }),
        });
    }
}

fn lambda_cost_posture_rule(
    report: &PillarReport,
    rule_id: &'static str,
    reason_codes: &[&'static str],
) -> LambdaPostureRule {
    let affected_resources = sorted_unique_resources(
        report
            .findings
            .iter()
            .filter(|finding| reason_codes.contains(&finding.reason_code.as_str()))
            .map(|finding| finding.resource_id.clone()),
    );

    LambdaPostureRule {
        rule_id,
        status: if affected_resources.is_empty() {
            LambdaPostureStatus::Pass
        } else {
            LambdaPostureStatus::Fail
        },
        reason_codes: reason_codes.to_vec(),
        affected_resources,
        suppression_supported: true,
        assignment_supported: true,
    }
}

fn lambda_cost_metric_names() -> [&'static str; 4] {
    ["Invocations", "Duration", "Errors", "Throttles"]
}

fn missing_fields<'a>(resource: &AwsResourceModel, required_fields: &'a [&str]) -> Vec<&'a str> {
    required_fields
        .iter()
        .copied()
        .filter(|field| resource.resource_data.get(*field).is_none())
        .collect()
}

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

fn data_u64(data: &Value, key: &str) -> Option<u64> {
    data.get(key).and_then(|value| {
        value
            .as_u64()
            .or_else(|| value.as_i64().map(|n| n.max(0) as u64))
    })
}

fn resource_data_keys(resource: &AwsResourceModel) -> Vec<String> {
    resource
        .resource_data
        .as_object()
        .map(|object| object.keys().cloned().collect())
        .unwrap_or_default()
}

fn sorted_unique_resources(resources: impl Iterator<Item = String>) -> Vec<String> {
    resources
        .filter(|resource_id| !resource_id.is_empty())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn sorted_unique_reasons(reasons: impl Iterator<Item = String>) -> Vec<String> {
    reasons.collect::<BTreeSet<_>>().into_iter().collect()
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
            resource_type: "LambdaFunction".to_string(),
            resource_id: resource_id.to_string(),
            arn: format!(
                "arn:aws:lambda:us-east-1:123456789012:function:{}",
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
            "function_name": "fn",
            "runtime": "python3.12",
            "timeout": 30,
            "memory_size": 256,
            "architectures": ["arm64"],
            "telemetry_collection_started_at": "2026-06-10T00:00:00Z",
            "telemetry_collection_completed_at": "2026-06-10T00:00:03Z",
            "telemetry_collection_duration_ms": 3000,
            "telemetry_collection_success_count": 1,
            "telemetry_collection_failure_count": 0,
            "telemetry_collection_error_count": 0,
            "telemetry_collection_errors": [],
            "cloudwatch_metric_names": ["Invocations", "Duration", "Errors", "Throttles"],
            "cloudwatch_metrics": {
                "source": "CloudWatch",
                "namespace": "AWS/Lambda",
                "resource_id": "fn",
                "dimension_name": "FunctionName",
                "metrics": [
                    {
                        "metric_name": "Invocations",
                        "datapoints": [{"value": 12.0}]
                    },
                    {
                        "metric_name": "Duration",
                        "datapoints": [{"value": 120.0}]
                    },
                    {
                        "metric_name": "Errors",
                        "datapoints": [{"value": 0.0}]
                    },
                    {
                        "metric_name": "Throttles",
                        "datapoints": [{"value": 0.0}]
                    }
                ]
            },
        })
    }

    #[test]
    fn cost_flags_missing_allocation_tags_and_x86_only_architecture() {
        let mut data = healthy_data();
        data["architectures"] = json!(["x86_64"]);
        let r = fixture("fn-untagged", json!({}), data, 1, now());
        let report = evaluate_lambda_fleet(&[r], Pillar::Cost, now());
        let codes: Vec<&str> = report
            .findings
            .iter()
            .map(|f| f.reason_code.as_str())
            .collect();
        assert!(codes.contains(&REASON_COST_MISSING_ALLOCATION_TAGS));
        assert!(codes.contains(&REASON_COST_X86_ONLY_ARCHITECTURE));
        let arch = report
            .findings
            .iter()
            .find(|f| f.reason_code == REASON_COST_X86_ONLY_ARCHITECTURE)
            .unwrap();
        assert_eq!(arch.evidence["architectures"], json!(["x86_64"]));
    }

    #[test]
    fn cost_passes_for_tagged_arm64_function() {
        let r = fixture(
            "fn-good",
            json!({"team": "payments"}),
            healthy_data(),
            1,
            now(),
        );
        let report = evaluate_lambda_fleet(&[r], Pillar::Cost, now());
        assert!(
            report.findings.is_empty(),
            "unexpected: {:?}",
            report.findings
        );
        assert_eq!(report.score, 100);
    }

    #[test]
    fn lambda_cost_telemetry_flags_missing_collection_and_metric_gaps() {
        let mut data = healthy_data();
        data.as_object_mut()
            .unwrap()
            .remove("telemetry_collection_started_at");
        data.as_object_mut().unwrap().remove("cloudwatch_metrics");
        let r = fixture(
            "fn-no-telemetry",
            json!({"team": "payments"}),
            data,
            1,
            now(),
        );

        let report = evaluate_lambda_fleet(&[r], Pillar::Cost, now());
        let codes: Vec<&str> = report
            .findings
            .iter()
            .map(|f| f.reason_code.as_str())
            .collect();

        assert!(codes.contains(&REASON_COST_MISSING_TELEMETRY_COLLECTION_METADATA));
        assert!(codes.contains(&REASON_COST_MISSING_CLOUDWATCH_TELEMETRY));
        let telemetry = lambda_cost_telemetry_summary(&report);
        assert_eq!(telemetry.workflow_id, "lambda_cost_telemetry");
        assert!(telemetry
            .missing_data_reason_codes
            .contains(&REASON_COST_MISSING_CLOUDWATCH_TELEMETRY.to_string()));
    }

    #[test]
    fn lambda_cost_telemetry_reports_collection_errors_and_cost_waste() {
        let mut data = healthy_data();
        data["telemetry_collection_success_count"] = json!(0);
        data["telemetry_collection_failure_count"] = json!(1);
        data["telemetry_collection_error_count"] = json!(1);
        data["telemetry_collection_errors"] = json!([
            {
                "source": "cloudwatch",
                "operation": "GetMetricData",
                "error": "Throttling"
            }
        ]);
        data["cloudwatch_metrics"]["metrics"][0]["datapoints"] = json!([{ "value": 0.0 }]);
        data["cloudwatch_metrics"]["metrics"][2]["datapoints"] = json!([{ "value": 2.0 }]);
        data["cloudwatch_metrics"]["metrics"][3]["datapoints"] = json!([{ "value": 1.0 }]);
        let r = fixture("fn-cost-waste", json!({"team": "payments"}), data, 1, now());

        let report = evaluate_lambda_fleet(&[r], Pillar::Cost, now());
        let codes: Vec<&str> = report
            .findings
            .iter()
            .map(|f| f.reason_code.as_str())
            .collect();

        assert!(codes.contains(&REASON_COST_TELEMETRY_COLLECTION_ERRORS));
        assert!(codes.contains(&REASON_COST_NO_INVOCATIONS_TELEMETRY));
        assert!(codes.contains(&REASON_COST_ERROR_OR_THROTTLE_TELEMETRY));
        let posture = lambda_cost_posture_summary(&report);
        assert_eq!(posture.status, LambdaPostureStatus::Fail);
        assert_eq!(posture.rules_evaluated, 7);
        assert!(posture
            .affected_resources
            .contains(&"fn-cost-waste".to_string()));
    }

    #[test]
    fn security_flags_deprecated_runtime_as_high() {
        let mut data = healthy_data();
        data["runtime"] = json!("python2.7");
        let r = fixture("fn-old", json!({"owner": "sre"}), data, 1, now());
        let report = evaluate_lambda_fleet(&[r], Pillar::Security, now());
        let finding = report
            .findings
            .iter()
            .find(|f| f.reason_code == REASON_SEC_DEPRECATED_RUNTIME)
            .expect("deprecated runtime finding");
        assert_eq!(finding.severity, Severity::High);
        assert_eq!(finding.evidence["runtime"], json!("python2.7"));
    }

    #[test]
    fn security_flags_missing_owner_tag_as_low() {
        let r = fixture("fn-orphan", json!({}), healthy_data(), 1, now());
        let report = evaluate_lambda_fleet(&[r], Pillar::Security, now());
        assert_eq!(
            report
                .findings
                .iter()
                .map(|f| f.reason_code.as_str())
                .collect::<Vec<_>>(),
            vec![REASON_SEC_MISSING_OWNER_TAG]
        );
    }

    #[test]
    fn security_passes_for_owned_current_runtime() {
        let r = fixture("fn-ok", json!({"owner": "sre"}), healthy_data(), 1, now());
        let report = evaluate_lambda_fleet(&[r], Pillar::Security, now());
        assert!(
            report.findings.is_empty(),
            "unexpected: {:?}",
            report.findings
        );
    }

    #[test]
    fn resilience_flags_missing_timeout_or_memory_config() {
        let r = fixture(
            "fn-noconf",
            json!({"owner": "sre"}),
            json!({"function_name": "fn-noconf", "runtime": "python3.12"}),
            1,
            now(),
        );
        let report = evaluate_lambda_fleet(&[r], Pillar::Resilience, now());
        let finding = report
            .findings
            .iter()
            .find(|f| f.reason_code == REASON_RES_MISSING_CONFIG_DATA)
            .expect("missing config finding");
        assert_eq!(finding.evidence["timeout"], json!(null));
    }

    #[test]
    fn stale_inventory_is_reported_as_failure_path() {
        let r = fixture(
            "fn-stale",
            json!({"owner": "sre", "project": "mayyam"}),
            healthy_data(),
            48,
            now(),
        );
        let report = evaluate_lambda_fleet(&[r], Pillar::Cost, now());
        assert_eq!(report.stale_resources, 1);
        assert!(report
            .findings
            .iter()
            .any(|f| f.reason_code == REASON_INV_STALE_DATA));
    }
}

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

// Deterministic CloudWatch metric inventory evaluators for the cost,
// resilience, and security pillars (roadmap rows 01-AWS-CLOUD-04348/04357/04384).
//
// Metrics cannot carry tags, so there is no tag-based cost check. Cost
// findings focus on custom-namespace volume signals: every custom metric is
// billed per metric per month, and each distinct dimension combination is a
// separately billed metric. Evaluates fields persisted by
// cloudwatch_control_plane::sync_metrics: Namespace, MetricName, Dimensions,
// DimensionCount, IsCustomNamespace.

use chrono::{DateTime, Utc};
use serde_json::{json, Value};

use crate::models::aws_resource::Model as AwsResourceModel;
use crate::services::aws::inventory::types::{
    check_stale, data_str, score_pillar, InventoryFinding, Pillar, PillarReport, Severity,
};

/// Only rows of this resource type are evaluated.
pub const RESOURCE_TYPE: &str = "CloudWatchMetric";

// Reason codes are the stable contract for findings; never reuse or rename.
pub const REASON_COST_CUSTOM_NAMESPACE_METRIC: &str = "CWMETRIC_COST_CUSTOM_NAMESPACE_METRIC";
pub const REASON_COST_HIGH_DIMENSION_CARDINALITY: &str = "CWMETRIC_COST_HIGH_DIMENSION_CARDINALITY";
pub const REASON_SEC_IDENTITY_DATA_NOT_COLLECTED: &str = "CWMETRIC_SEC_IDENTITY_DATA_NOT_COLLECTED";
pub const REASON_RES_NO_DIMENSIONS: &str = "CWMETRIC_RES_NO_DIMENSIONS";
pub const REASON_INV_STALE_DATA: &str = "CWMETRIC_INV_STALE_DATA";

/// Custom metrics at or above this dimension count are flagged: each distinct
/// dimension combination is billed as its own metric, so wide dimension sets
/// signal a cardinality (and spend) explosion.
pub const HIGH_DIMENSION_COUNT: i64 = 8;

/// Evaluate every CloudWatch metric in the fleet for one pillar. Rows whose
/// `resource_type` is not `CloudWatchMetric` are skipped and not counted.
pub fn evaluate_cloudwatch_metric_fleet(
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

fn is_custom_namespace(resource_data: &Value) -> bool {
    resource_data
        .get("IsCustomNamespace")
        .and_then(|v| v.as_bool())
        .unwrap_or_else(|| {
            data_str(resource_data, "Namespace")
                .map(|ns| !ns.starts_with("AWS/"))
                .unwrap_or(false)
        })
}

fn dimension_count(resource_data: &Value) -> i64 {
    resource_data
        .get("DimensionCount")
        .and_then(|v| v.as_i64())
        .unwrap_or(0)
}

fn evaluate_cost(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    if !is_custom_namespace(&resource.resource_data) {
        return;
    }

    let namespace = data_str(&resource.resource_data, "Namespace");
    findings.push(InventoryFinding {
        resource_id: resource.resource_id.clone(),
        arn: resource.arn.clone(),
        pillar: Pillar::Cost,
        reason_code: REASON_COST_CUSTOM_NAMESPACE_METRIC.to_string(),
        severity: Severity::Low,
        message: format!(
            "Metric {} lives in custom namespace {}; every custom metric is billed per metric per month, so review whether it is still consumed",
            resource.resource_id,
            namespace.as_deref().unwrap_or("(unknown)")
        ),
        evidence: json!({ "namespace": namespace }),
    });

    let dims = dimension_count(&resource.resource_data);
    if dims >= HIGH_DIMENSION_COUNT {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Cost,
            reason_code: REASON_COST_HIGH_DIMENSION_CARDINALITY.to_string(),
            severity: Severity::Medium,
            message: format!(
                "Custom metric {} carries {} dimensions; each distinct dimension combination is billed as a separate metric, so wide dimension sets multiply spend",
                resource.resource_id, dims
            ),
            evidence: json!({
                "dimension_count": dims,
                "dimensions": resource.resource_data.get("Dimensions"),
            }),
        });
    }
}

fn evaluate_security(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    // Metrics carry no security configuration of their own (no policy, no
    // encryption settings). The deterministic security check is an identity
    // data gap: a metric whose namespace or name was not collected cannot be
    // attributed to an emitting workload during an investigation.
    let namespace = data_str(&resource.resource_data, "Namespace");
    let metric_name = data_str(&resource.resource_data, "MetricName");
    if namespace.as_deref().unwrap_or("").is_empty()
        || metric_name.as_deref().unwrap_or("").is_empty()
    {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Security,
            reason_code: REASON_SEC_IDENTITY_DATA_NOT_COLLECTED.to_string(),
            severity: Severity::Low,
            message: format!(
                "Metric {} is missing namespace or metric name in collected data; the emitting workload cannot be attributed, so security posture cannot be assessed",
                resource.resource_id
            ),
            evidence: json!({ "namespace": namespace, "metric_name": metric_name }),
        });
    }
}

fn evaluate_resilience(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    // A dimensionless custom metric aggregates every emitter into one series,
    // so a single failing instance is invisible behind the fleet average.
    if is_custom_namespace(&resource.resource_data) && dimension_count(&resource.resource_data) == 0
    {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Resilience,
            reason_code: REASON_RES_NO_DIMENSIONS.to_string(),
            severity: Severity::Low,
            message: format!(
                "Custom metric {} has no dimensions; all emitters collapse into one series, hiding per-resource failures behind the aggregate",
                resource.resource_id
            ),
            evidence: json!({ "dimension_count": 0 }),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;
    use uuid::Uuid;

    fn fixture(resource_id: &str, resource_data: Value, now: DateTime<Utc>) -> AwsResourceModel {
        let refreshed = now - Duration::hours(1);
        AwsResourceModel {
            id: Uuid::new_v4(),
            sync_id: None,
            account_id: "123456789012".to_string(),
            profile: None,
            region: "us-east-1".to_string(),
            resource_type: RESOURCE_TYPE.to_string(),
            resource_id: resource_id.to_string(),
            arn: format!("cloudwatch:metric:us-east-1:123456789012:{}", resource_id),
            name: Some(resource_id.to_string()),
            tags: json!({}),
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

    fn aws_metric_data() -> Value {
        json!({
            "Namespace": "AWS/EC2",
            "MetricName": "CPUUtilization",
            "Dimensions": [{ "Name": "InstanceId", "Value": "i-0abc" }],
            "DimensionCount": 1,
            "IsCustomNamespace": false,
        })
    }

    fn custom_metric_data() -> Value {
        json!({
            "Namespace": "MyApp/Orders",
            "MetricName": "OrdersPlaced",
            "Dimensions": [{ "Name": "Service", "Value": "checkout" }],
            "DimensionCount": 1,
            "IsCustomNamespace": true,
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
    fn cost_flags_custom_namespace_metric() {
        let r = fixture("MyApp/Orders:OrdersPlaced", custom_metric_data(), now());
        let report = evaluate_cloudwatch_metric_fleet(&[r], Pillar::Cost, now());
        assert_eq!(codes(&report), vec![REASON_COST_CUSTOM_NAMESPACE_METRIC]);
    }

    #[test]
    fn cost_does_not_flag_aws_namespace_metric() {
        let r = fixture("AWS/EC2:CPUUtilization", aws_metric_data(), now());
        let report = evaluate_cloudwatch_metric_fleet(&[r], Pillar::Cost, now());
        assert!(
            report.findings.is_empty(),
            "unexpected: {:?}",
            report.findings
        );
        assert_eq!(report.score, 100);
    }

    #[test]
    fn cost_flags_high_dimension_cardinality_on_custom_metric() {
        let mut data = custom_metric_data();
        data["DimensionCount"] = json!(9);
        let r = fixture("MyApp/Orders:Wide", data, now());
        let report = evaluate_cloudwatch_metric_fleet(&[r], Pillar::Cost, now());
        assert_eq!(
            codes(&report),
            vec![
                REASON_COST_CUSTOM_NAMESPACE_METRIC,
                REASON_COST_HIGH_DIMENSION_CARDINALITY
            ]
        );
    }

    #[test]
    fn cost_does_not_flag_high_dimensions_on_aws_metric() {
        let mut data = aws_metric_data();
        data["DimensionCount"] = json!(10);
        let r = fixture("AWS/EC2:Wide", data, now());
        let report = evaluate_cloudwatch_metric_fleet(&[r], Pillar::Cost, now());
        assert!(
            report.findings.is_empty(),
            "unexpected: {:?}",
            report.findings
        );
    }

    #[test]
    fn security_flags_missing_identity_data() {
        let data = json!({ "Namespace": "", "MetricName": "Orphan" });
        let r = fixture("orphan", data, now());
        let report = evaluate_cloudwatch_metric_fleet(&[r], Pillar::Security, now());
        assert_eq!(codes(&report), vec![REASON_SEC_IDENTITY_DATA_NOT_COLLECTED]);
    }

    #[test]
    fn security_passes_fully_identified_metric() {
        let r = fixture("AWS/EC2:CPUUtilization", aws_metric_data(), now());
        let report = evaluate_cloudwatch_metric_fleet(&[r], Pillar::Security, now());
        assert!(
            report.findings.is_empty(),
            "unexpected: {:?}",
            report.findings
        );
    }

    #[test]
    fn resilience_flags_dimensionless_custom_metric() {
        let mut data = custom_metric_data();
        data["Dimensions"] = json!([]);
        data["DimensionCount"] = json!(0);
        let r = fixture("MyApp/Orders:Flat", data, now());
        let report = evaluate_cloudwatch_metric_fleet(&[r], Pillar::Resilience, now());
        assert_eq!(codes(&report), vec![REASON_RES_NO_DIMENSIONS]);
    }

    #[test]
    fn resilience_allows_dimensionless_aws_metric() {
        let mut data = aws_metric_data();
        data["Dimensions"] = json!([]);
        data["DimensionCount"] = json!(0);
        let r = fixture("AWS/S3:Flat", data, now());
        let report = evaluate_cloudwatch_metric_fleet(&[r], Pillar::Resilience, now());
        assert!(
            report.findings.is_empty(),
            "unexpected: {:?}",
            report.findings
        );
    }

    #[test]
    fn stale_inventory_is_flagged() {
        let mut r = fixture("AWS/EC2:CPUUtilization", aws_metric_data(), now());
        r.last_refreshed = now() - Duration::hours(48);
        let report = evaluate_cloudwatch_metric_fleet(&[r], Pillar::Resilience, now());
        assert_eq!(report.stale_resources, 1);
        assert!(codes(&report).contains(&REASON_INV_STALE_DATA));
    }

    #[test]
    fn non_metric_resources_are_skipped_and_not_counted() {
        let mut r = fixture("queue-1", json!({}), now());
        r.resource_type = "SqsQueue".to_string();
        let report = evaluate_cloudwatch_metric_fleet(&[r], Pillar::Cost, now());
        assert_eq!(report.resources_evaluated, 0);
        assert!(report.findings.is_empty());
    }

    #[test]
    fn healthy_aws_metric_passes_all_pillars() {
        let r = fixture("AWS/EC2:CPUUtilization", aws_metric_data(), now());
        for pillar in [Pillar::Cost, Pillar::Security, Pillar::Resilience] {
            let report = evaluate_cloudwatch_metric_fleet(std::slice::from_ref(&r), pillar, now());
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

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

use chrono::{DateTime, Utc};
use serde::Serialize;
use serde_json::{json, Value};

use crate::models::aws_resource::Model as AwsResourceModel;
use crate::services::aws::inventory::eventbridge_pillar_evaluator::REASON_SCALE_TARGET_QUOTA_REACHED;
use crate::services::aws::inventory::lambda_pillar_evaluator::REASON_RES_MISSING_QUOTA_LIMIT_EVIDENCE;
use crate::services::aws::inventory::route53_pillar_evaluator::REASON_RES_RECORD_QUOTA_NEAR;
use crate::services::aws::inventory::types::DEFAULT_STALE_AFTER_HOURS;

pub const RESOURCE_TYPE: &str = "AwsQuotaExhaustionAlert";
pub const REASON_LAMBDA_QUOTA_NEAR: &str = "LAMBDA_RES_CONCURRENCY_QUOTA_NEAR";

const ROUTE53_RECORD_QUOTA: i64 = 10_000;
const ROUTE53_RECORD_ALERT_THRESHOLD: i64 = 9_000;
const EVENTBRIDGE_TARGET_QUOTA: i64 = 5;
const LAMBDA_CONCURRENCY_HEADROOM_THRESHOLD: f64 = 0.9;

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct CloudQuotaAlert {
    pub provider: &'static str,
    pub service: &'static str,
    pub resource_type: &'static str,
    pub resource_id: String,
    pub arn: String,
    pub status: &'static str,
    pub severity: &'static str,
    pub reason_code: String,
    pub summary: String,
    pub evidence: Value,
    pub last_refreshed: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct CloudQuotaWorkflow {
    pub workflow_id: &'static str,
    pub read_only_mode: bool,
    pub status: &'static str,
    pub evidence_reason_codes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct CloudQuotaHealth {
    pub workflow_id: &'static str,
    pub status: &'static str,
    pub active_alerts: usize,
    pub insufficient_data_alerts: usize,
    pub highest_severity: &'static str,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct CloudQuotaReporting {
    pub workflow_id: &'static str,
    pub report_id: &'static str,
    pub total_alerts: usize,
    pub services_with_alerts: Vec<&'static str>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct CloudQuotaAlertBundle {
    pub resource_type: &'static str,
    pub resources_evaluated: usize,
    pub active_alerts: usize,
    pub insufficient_data_alerts: usize,
    pub alerts: Vec<CloudQuotaAlert>,
    pub notification_workflow: CloudQuotaWorkflow,
    pub ai_triage_workflow: CloudQuotaWorkflow,
    pub agentic_investigation_workflow: CloudQuotaWorkflow,
    pub automation_workflow: CloudQuotaWorkflow,
    pub health_workflow: CloudQuotaHealth,
    pub reporting: CloudQuotaReporting,
}

pub fn build_cloud_quota_exhaustion_alert_bundle(
    route53_resources: &[AwsResourceModel],
    eventbridge_resources: &[AwsResourceModel],
    lambda_resources: &[AwsResourceModel],
) -> CloudQuotaAlertBundle {
    let mut alerts = Vec::new();

    for resource in route53_resources {
        if let Some(record_count) = data_i64(&resource.resource_data, "resource_record_set_count") {
            if record_count >= ROUTE53_RECORD_ALERT_THRESHOLD {
                alerts.push(CloudQuotaAlert {
                    provider: "aws",
                    service: "route53",
                    resource_type: "Route53HostedZone",
                    resource_id: resource.resource_id.clone(),
                    arn: resource.arn.clone(),
                    status: "firing",
                    severity: "medium",
                    reason_code: REASON_RES_RECORD_QUOTA_NEAR.to_string(),
                    summary: format!(
                        "Route 53 hosted zone {} is near the record-set quota",
                        resource.resource_id
                    ),
                    evidence: json!({
                        "resource_record_set_count": record_count,
                        "record_quota": ROUTE53_RECORD_QUOTA,
                    }),
                    last_refreshed: resource.last_refreshed,
                });
            }
        }
    }

    for resource in eventbridge_resources {
        if data_bool(&resource.resource_data, "state").unwrap_or(true)
            && data_i64(&resource.resource_data, "target_count")
                .is_some_and(|count| count >= EVENTBRIDGE_TARGET_QUOTA)
        {
            let count = data_i64(&resource.resource_data, "target_count").unwrap_or_default();
            alerts.push(CloudQuotaAlert {
                provider: "aws",
                service: "eventbridge",
                resource_type: "EventBridgeRule",
                resource_id: resource.resource_id.clone(),
                arn: resource.arn.clone(),
                status: "firing",
                severity: "medium",
                reason_code: REASON_SCALE_TARGET_QUOTA_REACHED.to_string(),
                summary: format!(
                    "EventBridge rule {} has reached the target quota",
                    resource.resource_id
                ),
                evidence: json!({
                    "target_count": count,
                    "target_quota_per_rule": EVENTBRIDGE_TARGET_QUOTA,
                }),
                last_refreshed: resource.last_refreshed,
            });
        }
    }

    for resource in lambda_resources {
        let reserved = data_i64(&resource.resource_data, "reserved_concurrent_executions");
        let account_limit = data_i64(&resource.resource_data, "account_concurrency_limit");

        match (reserved, account_limit) {
            (Some(reserved), Some(account_limit)) if account_limit > 0 => {
                let headroom = reserved as f64 / account_limit as f64;
                if headroom >= LAMBDA_CONCURRENCY_HEADROOM_THRESHOLD {
                    alerts.push(CloudQuotaAlert {
                        provider: "aws",
                        service: "lambda",
                        resource_type: "LambdaFunction",
                        resource_id: resource.resource_id.clone(),
                        arn: resource.arn.clone(),
                        status: "firing",
                        severity: "high",
                        reason_code: REASON_LAMBDA_QUOTA_NEAR.to_string(),
                        summary: format!(
                            "Lambda function {} is consuming most of the account concurrency quota",
                            resource.resource_id
                        ),
                        evidence: json!({
                            "reserved_concurrent_executions": reserved,
                            "account_concurrency_limit": account_limit,
                            "quota_utilization_pct": (headroom * 100.0).round(),
                        }),
                        last_refreshed: resource.last_refreshed,
                    });
                }
            }
            _ => alerts.push(CloudQuotaAlert {
                provider: "aws",
                service: "lambda",
                resource_type: "LambdaFunction",
                resource_id: resource.resource_id.clone(),
                arn: resource.arn.clone(),
                status: "insufficient_data",
                severity: "medium",
                reason_code: REASON_RES_MISSING_QUOTA_LIMIT_EVIDENCE.to_string(),
                summary: format!(
                    "Lambda function {} is missing concurrency quota evidence for alerting",
                    resource.resource_id
                ),
                evidence: json!({
                    "required_fields": ["reserved_concurrent_executions", "account_concurrency_limit"],
                    "resource_data_keys": resource_data_keys(&resource.resource_data),
                }),
                last_refreshed: resource.last_refreshed,
            }),
        }
    }

    alerts.sort_by(|left, right| {
        left.service
            .cmp(right.service)
            .then(left.resource_id.cmp(&right.resource_id))
    });

    let active_alerts = alerts
        .iter()
        .filter(|alert| alert.status == "firing")
        .count();
    let insufficient_data_alerts = alerts
        .iter()
        .filter(|alert| alert.status == "insufficient_data")
        .count();
    let evidence_reason_codes = alerts
        .iter()
        .map(|alert| alert.reason_code.clone())
        .collect::<Vec<_>>();
    let services_with_alerts = unique_services(&alerts);
    let highest_severity = if alerts.iter().any(|alert| alert.severity == "high") {
        "high"
    } else if alerts.iter().any(|alert| alert.severity == "medium") {
        "medium"
    } else {
        "info"
    };
    let workflow_status = if active_alerts > 0 {
        "action_required"
    } else if insufficient_data_alerts > 0 {
        "insufficient_data"
    } else {
        "healthy"
    };

    CloudQuotaAlertBundle {
        resource_type: RESOURCE_TYPE,
        resources_evaluated: route53_resources.len()
            + eventbridge_resources.len()
            + lambda_resources.len(),
        active_alerts,
        insufficient_data_alerts,
        alerts,
        notification_workflow: CloudQuotaWorkflow {
            workflow_id: "cloud_quota_exhaustion_notification",
            read_only_mode: true,
            status: workflow_status,
            evidence_reason_codes: evidence_reason_codes.clone(),
        },
        ai_triage_workflow: CloudQuotaWorkflow {
            workflow_id: "cloud_quota_exhaustion_ai_triage",
            read_only_mode: true,
            status: workflow_status,
            evidence_reason_codes: evidence_reason_codes.clone(),
        },
        agentic_investigation_workflow: CloudQuotaWorkflow {
            workflow_id: "cloud_quota_exhaustion_agentic_investigation",
            read_only_mode: true,
            status: workflow_status,
            evidence_reason_codes: evidence_reason_codes.clone(),
        },
        automation_workflow: CloudQuotaWorkflow {
            workflow_id: "cloud_quota_exhaustion_automation",
            read_only_mode: true,
            status: workflow_status,
            evidence_reason_codes: evidence_reason_codes.clone(),
        },
        health_workflow: CloudQuotaHealth {
            workflow_id: "cloud_quota_exhaustion_health",
            status: workflow_status,
            active_alerts,
            insufficient_data_alerts,
            highest_severity,
        },
        reporting: CloudQuotaReporting {
            workflow_id: "cloud_quota_exhaustion_reporting",
            report_id: "cloud-quota-exhaustion-summary",
            total_alerts: active_alerts + insufficient_data_alerts,
            services_with_alerts,
        },
    }
}

pub fn stale_after_hours() -> i64 {
    DEFAULT_STALE_AFTER_HOURS
}

fn data_i64(value: &Value, key: &str) -> Option<i64> {
    value.get(key).and_then(|value| match value {
        Value::Number(number) => number.as_i64(),
        Value::String(text) => text.parse::<i64>().ok(),
        _ => None,
    })
}

fn data_bool(value: &Value, key: &str) -> Option<bool> {
    value.get(key).and_then(|value| match value {
        Value::Bool(value) => Some(*value),
        Value::String(text) => match text.to_ascii_lowercase().as_str() {
            "enabled" | "true" => Some(true),
            "disabled" | "false" => Some(false),
            _ => None,
        },
        _ => None,
    })
}

fn resource_data_keys(value: &Value) -> Vec<String> {
    value
        .as_object()
        .map(|fields| fields.keys().cloned().collect::<Vec<_>>())
        .unwrap_or_default()
}

fn unique_services(alerts: &[CloudQuotaAlert]) -> Vec<&'static str> {
    let mut services = alerts.iter().map(|alert| alert.service).collect::<Vec<_>>();
    services.sort_unstable();
    services.dedup();
    services
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;
    use serde_json::json;
    use uuid::Uuid;

    fn resource(resource_type: &str, resource_id: &str, resource_data: Value) -> AwsResourceModel {
        let refreshed = Utc::now() - Duration::hours(1);
        AwsResourceModel {
            id: Uuid::new_v4(),
            sync_id: None,
            account_id: "123456789012".to_string(),
            profile: None,
            region: "us-east-1".to_string(),
            resource_type: resource_type.to_string(),
            resource_id: resource_id.to_string(),
            arn: format!("arn:aws:test:::{}", resource_id),
            name: Some(resource_id.to_string()),
            tags: json!({"team": "sre"}),
            resource_data,
            created_at: refreshed,
            updated_at: refreshed,
            last_refreshed: refreshed,
        }
    }

    #[test]
    fn quota_bundle_reports_firing_and_insufficient_data_alerts() {
        let route53 = vec![resource(
            "Route53HostedZone",
            "hosted-zone",
            json!({"resource_record_set_count": 9800}),
        )];
        let eventbridge = vec![resource(
            "EventBridgeRule",
            "rule-quota",
            json!({"state": "ENABLED", "target_count": 5}),
        )];
        let lambda = vec![
            resource(
                "LambdaFunction",
                "quota-near",
                json!({
                    "reserved_concurrent_executions": 900,
                    "account_concurrency_limit": 1000
                }),
            ),
            resource("LambdaFunction", "missing-evidence", json!({})),
        ];

        let bundle = build_cloud_quota_exhaustion_alert_bundle(&route53, &eventbridge, &lambda);

        assert_eq!(bundle.resources_evaluated, 4);
        assert_eq!(bundle.active_alerts, 3);
        assert_eq!(bundle.insufficient_data_alerts, 1);
        assert_eq!(bundle.alerts.len(), 4);
        assert_eq!(bundle.health_workflow.status, "action_required");
        assert!(bundle.reporting.services_with_alerts.contains(&"lambda"));
        assert!(bundle
            .notification_workflow
            .evidence_reason_codes
            .iter()
            .any(|code| code == REASON_RES_RECORD_QUOTA_NEAR));
    }
}

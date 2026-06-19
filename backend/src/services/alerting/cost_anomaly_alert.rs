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
use sea_orm::prelude::Decimal;
use serde::Serialize;
use serde_json::{json, Value};

use crate::models::aws_cost_anomalies::Model as AwsCostAnomalyModel;
use crate::models::aws_cost_insights::Model as AwsCostInsightModel;
use crate::services::aws::inventory::types::DEFAULT_STALE_AFTER_HOURS;

pub const RESOURCE_TYPE: &str = "AwsCostAnomalyAlert";
pub const REASON_ANOMALY_OPEN: &str = "AWS_COST_ANOMALY_ALERT_OPEN";
pub const REASON_ANOMALY_HIGH_SEVERITY: &str = "AWS_COST_ANOMALY_ALERT_HIGH_SEVERITY";
pub const REASON_ANOMALY_MISSING_INSIGHT: &str = "AWS_COST_ANOMALY_ALERT_MISSING_INSIGHT";
pub const REASON_ANOMALY_LOW_CONFIDENCE: &str = "AWS_COST_ANOMALY_ALERT_LOW_CONFIDENCE";

const LOW_CONFIDENCE_THRESHOLD: f64 = 0.7;

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct CostAnomalyAlert {
    pub account_id: String,
    pub service_name: String,
    pub anomaly_id: String,
    pub status: &'static str,
    pub severity: &'static str,
    pub reason_code: String,
    pub summary: String,
    pub evidence: Value,
    pub detected_at: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct CostAnomalyAlertWorkflow {
    pub workflow_id: &'static str,
    pub read_only_mode: bool,
    pub status: &'static str,
    pub evidence_reason_codes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct CostAnomalyAlertHealth {
    pub workflow_id: &'static str,
    pub status: &'static str,
    pub active_alerts: usize,
    pub insufficient_data_alerts: usize,
    pub highest_severity: &'static str,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct CostAnomalyAlertReporting {
    pub workflow_id: &'static str,
    pub report_id: &'static str,
    pub total_alerts: usize,
    pub services_with_alerts: Vec<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct CostAnomalyAlertBundle {
    pub resource_type: &'static str,
    pub resources_evaluated: usize,
    pub active_alerts: usize,
    pub insufficient_data_alerts: usize,
    pub alerts: Vec<CostAnomalyAlert>,
    pub notification_workflow: CostAnomalyAlertWorkflow,
    pub ai_triage_workflow: CostAnomalyAlertWorkflow,
    pub agentic_investigation_workflow: CostAnomalyAlertWorkflow,
    pub automation_workflow: CostAnomalyAlertWorkflow,
    pub health_workflow: CostAnomalyAlertHealth,
    pub reporting: CostAnomalyAlertReporting,
}

pub fn build_cost_anomaly_alert_bundle(
    anomalies: &[AwsCostAnomalyModel],
    latest_insights: &[Option<AwsCostInsightModel>],
) -> CostAnomalyAlertBundle {
    let mut alerts = Vec::new();

    for (anomaly, latest_insight) in anomalies.iter().zip(latest_insights.iter()) {
        if anomaly.status.eq_ignore_ascii_case("open") {
            alerts.push(CostAnomalyAlert {
                account_id: anomaly.account_id.clone(),
                service_name: anomaly.service_name.clone(),
                anomaly_id: anomaly.id.to_string(),
                status: "firing",
                severity: severity_label(&anomaly.severity),
                reason_code: REASON_ANOMALY_OPEN.to_string(),
                summary: format!(
                    "Cost anomaly for {} is still open and requires alert handling",
                    anomaly.service_name
                ),
                evidence: json!({
                    "anomaly_type": anomaly.anomaly_type,
                    "status": anomaly.status,
                    "actual_cost": decimal_to_f64(&anomaly.actual_cost),
                    "baseline_cost": anomaly.baseline_cost.as_ref().map(decimal_to_f64),
                    "percentage_change": anomaly.percentage_change.as_ref().map(decimal_to_f64),
                }),
                detected_at: anomaly.detected_date.to_string(),
            });
        }

        if anomaly.severity.eq_ignore_ascii_case("high") {
            alerts.push(CostAnomalyAlert {
                account_id: anomaly.account_id.clone(),
                service_name: anomaly.service_name.clone(),
                anomaly_id: anomaly.id.to_string(),
                status: "firing",
                severity: "high",
                reason_code: REASON_ANOMALY_HIGH_SEVERITY.to_string(),
                summary: format!(
                    "High-severity cost anomaly detected for {}",
                    anomaly.service_name
                ),
                evidence: json!({
                    "anomaly_type": anomaly.anomaly_type,
                    "anomaly_score": decimal_to_f64(&anomaly.anomaly_score),
                    "cost_difference": anomaly.cost_difference.as_ref().map(decimal_to_f64),
                    "description": anomaly.description,
                }),
                detected_at: anomaly.detected_date.to_string(),
            });
        }

        match latest_insight {
            Some(insight) => {
                let confidence = insight
                    .confidence_score
                    .as_ref()
                    .map(decimal_to_f64)
                    .unwrap_or_default();
                if confidence > 0.0 && confidence < LOW_CONFIDENCE_THRESHOLD {
                    alerts.push(CostAnomalyAlert {
                        account_id: anomaly.account_id.clone(),
                        service_name: anomaly.service_name.clone(),
                        anomaly_id: anomaly.id.to_string(),
                        status: "insufficient_data",
                        severity: "medium",
                        reason_code: REASON_ANOMALY_LOW_CONFIDENCE.to_string(),
                        summary: format!(
                            "Cost anomaly insight for {} has low confidence",
                            anomaly.service_name
                        ),
                        evidence: json!({
                            "insight_id": insight.id,
                            "insight_type": insight.insight_type,
                            "confidence_score": confidence,
                            "summary": insight.summary,
                        }),
                        detected_at: anomaly.detected_date.to_string(),
                    });
                }
            }
            None => alerts.push(CostAnomalyAlert {
                account_id: anomaly.account_id.clone(),
                service_name: anomaly.service_name.clone(),
                anomaly_id: anomaly.id.to_string(),
                status: "insufficient_data",
                severity: "medium",
                reason_code: REASON_ANOMALY_MISSING_INSIGHT.to_string(),
                summary: format!(
                    "Cost anomaly for {} is missing triage insight evidence",
                    anomaly.service_name
                ),
                evidence: json!({
                    "required_insight_type": "anomaly_analysis",
                    "anomaly_type": anomaly.anomaly_type,
                    "status": anomaly.status,
                }),
                detected_at: anomaly.detected_date.to_string(),
            }),
        }
    }

    alerts.sort_by(|left, right| {
        left.service_name
            .cmp(&right.service_name)
            .then(left.reason_code.cmp(&right.reason_code))
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
    let workflow_status = if active_alerts > 0 {
        "action_required"
    } else if insufficient_data_alerts > 0 {
        "insufficient_data"
    } else {
        "healthy"
    };
    let highest_severity = highest_severity(&alerts);
    let total_alerts = alerts.len();
    let services_with_alerts = unique_services(&alerts);

    CostAnomalyAlertBundle {
        resource_type: RESOURCE_TYPE,
        resources_evaluated: anomalies.len(),
        active_alerts,
        insufficient_data_alerts,
        alerts,
        notification_workflow: CostAnomalyAlertWorkflow {
            workflow_id: "cost_anomaly_notification",
            read_only_mode: true,
            status: workflow_status,
            evidence_reason_codes: evidence_reason_codes.clone(),
        },
        ai_triage_workflow: CostAnomalyAlertWorkflow {
            workflow_id: "cost_anomaly_ai_triage",
            read_only_mode: true,
            status: workflow_status,
            evidence_reason_codes: evidence_reason_codes.clone(),
        },
        agentic_investigation_workflow: CostAnomalyAlertWorkflow {
            workflow_id: "cost_anomaly_agentic_investigation",
            read_only_mode: true,
            status: workflow_status,
            evidence_reason_codes: evidence_reason_codes.clone(),
        },
        automation_workflow: CostAnomalyAlertWorkflow {
            workflow_id: "cost_anomaly_automation",
            read_only_mode: true,
            status: workflow_status,
            evidence_reason_codes: evidence_reason_codes.clone(),
        },
        health_workflow: CostAnomalyAlertHealth {
            workflow_id: "cost_anomaly_health",
            status: workflow_status,
            active_alerts,
            insufficient_data_alerts,
            highest_severity,
        },
        reporting: CostAnomalyAlertReporting {
            workflow_id: "cost_anomaly_reporting",
            report_id: "aws-cost-anomaly-alert-summary",
            total_alerts,
            services_with_alerts,
        },
    }
}

pub fn evaluated_at() -> DateTime<Utc> {
    Utc::now()
}

pub fn stale_after_hours() -> i64 {
    DEFAULT_STALE_AFTER_HOURS
}

fn severity_label(value: &str) -> &'static str {
    if value.eq_ignore_ascii_case("high") {
        "high"
    } else if value.eq_ignore_ascii_case("medium") {
        "medium"
    } else {
        "info"
    }
}

fn decimal_to_f64(value: &Decimal) -> f64 {
    value.to_string().parse::<f64>().unwrap_or_default()
}

fn unique_services(alerts: &[CostAnomalyAlert]) -> Vec<String> {
    let mut services = alerts
        .iter()
        .map(|alert| alert.service_name.clone())
        .collect::<Vec<_>>();
    services.sort();
    services.dedup();
    services
}

fn highest_severity(alerts: &[CostAnomalyAlert]) -> &'static str {
    if alerts.iter().any(|alert| alert.severity == "high") {
        "high"
    } else if alerts.iter().any(|alert| alert.severity == "medium") {
        "medium"
    } else {
        "info"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[test]
    fn cost_anomaly_alert_bundle_reports_open_high_and_missing_insight_states() {
        let anomaly = AwsCostAnomalyModel {
            id: Uuid::new_v4(),
            account_id: "123456789012".to_string(),
            service_name: "AmazonEC2".to_string(),
            anomaly_type: "spike".to_string(),
            severity: "high".to_string(),
            detected_date: chrono::NaiveDate::from_ymd_opt(2026, 6, 1).unwrap(),
            anomaly_score: Decimal::from_f64_retain(8.2).unwrap(),
            baseline_cost: Some(Decimal::from_f64_retain(120.0).unwrap()),
            actual_cost: Decimal::from_f64_retain(280.0).unwrap(),
            cost_difference: Some(Decimal::from_f64_retain(160.0).unwrap()),
            percentage_change: Some(Decimal::from_f64_retain(133.3).unwrap()),
            description: Some("Large EC2 spike".to_string()),
            status: "open".to_string(),
            created_at: Utc::now().into(),
            updated_at: Utc::now().into(),
        };

        let bundle = build_cost_anomaly_alert_bundle(&[anomaly], &[None]);

        assert_eq!(bundle.resource_type, RESOURCE_TYPE);
        assert_eq!(bundle.resources_evaluated, 1);
        assert_eq!(bundle.active_alerts, 2);
        assert_eq!(bundle.insufficient_data_alerts, 1);
        assert_eq!(bundle.alerts.len(), 3);
        assert_eq!(bundle.notification_workflow.status, "action_required");
        assert_eq!(bundle.reporting.report_id, "aws-cost-anomaly-alert-summary");
        assert!(bundle
            .alerts
            .iter()
            .any(|alert| alert.reason_code == REASON_ANOMALY_OPEN));
        assert!(bundle
            .alerts
            .iter()
            .any(|alert| alert.reason_code == REASON_ANOMALY_HIGH_SEVERITY));
        assert!(bundle
            .alerts
            .iter()
            .any(|alert| alert.reason_code == REASON_ANOMALY_MISSING_INSIGHT));
    }
}

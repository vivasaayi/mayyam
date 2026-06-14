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

// Deterministic CloudWatch inventory evaluators for the cost, security,
// and resilience pillars (roadmap rows 01-AWS-CLOUD-04474/04483/04510 for
// alarms and 01-AWS-CLOUD-04537/04546/04573 for dashboards).
//
// Evaluates both CloudWatchAlarm and CloudWatchDashboard rows persisted by
// cloudwatch_control_plane. Alarms carry PascalCase keys AlarmName,
// AlarmArn, AlarmDescription, StateValue, MetricName, Namespace;
// dashboards carry DashboardName, DashboardArn, Size, LastModified. The
// collector does not gather alarm actions, so action coverage is reported
// as an explicit data gap instead of a guessed pass or fail. CloudWatch
// alarms and dashboards expose no collected security-configurable fields
// (no encryption or resource-policy state is persisted), so the security
// pillar is intentionally left clean rather than inventing findings.

use chrono::{DateTime, Utc};
use serde_json::json;

use crate::models::aws_resource::Model as AwsResourceModel;
use crate::services::aws::inventory::types::{
    check_stale, data_str, score_pillar, InventoryFinding, Pillar, PillarReport, Severity,
};

// Reason codes are the stable contract for findings; never reuse or rename.
pub const REASON_COST_NO_TAGS: &str = "CLOUDWATCH_COST_NO_TAGS";
pub const REASON_RES_ALARM_INSUFFICIENT_DATA: &str = "CLOUDWATCH_RES_ALARM_INSUFFICIENT_DATA";
pub const REASON_RES_ALARM_STATE_DATA_NOT_COLLECTED: &str =
    "CLOUDWATCH_RES_ALARM_STATE_DATA_NOT_COLLECTED";
pub const REASON_RES_ALARM_ACTION_DATA_NOT_COLLECTED: &str =
    "CLOUDWATCH_RES_ALARM_ACTION_DATA_NOT_COLLECTED";
pub const REASON_INV_STALE_DATA: &str = "CLOUDWATCH_INV_STALE_DATA";

fn is_alarm(resource: &AwsResourceModel) -> bool {
    resource.resource_type == "CloudWatchAlarm"
}

/// Evaluate every CloudWatch alarm and dashboard in the fleet for one pillar.
pub fn evaluate_cloudwatch_fleet(
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
                "{} {} has no tags recorded (untagged resource or tag collection gap); cost allocation cannot be assessed",
                resource.resource_type, resource.resource_id
            ),
            evidence: json!({ "tags": resource.tags }),
        });
    }
}

fn evaluate_security(_resource: &AwsResourceModel, _findings: &mut Vec<InventoryFinding>) {
    // The collector persists no security-configurable fields for alarms or
    // dashboards (CloudWatch exposes no per-alarm/per-dashboard encryption
    // or resource-policy settings in the collected metadata). Emitting a
    // finding here would not be grounded in evidence, so the security
    // pillar stays clean for these resource types.
}

fn evaluate_resilience(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    if !is_alarm(resource) {
        // Dashboards carry only Size/LastModified; no honest resilience
        // signal is collected for them yet.
        return;
    }

    match data_str(&resource.resource_data, "StateValue") {
        Some(state) if state == "INSUFFICIENT_DATA" => {
            findings.push(InventoryFinding {
                resource_id: resource.resource_id.clone(),
                arn: resource.arn.clone(),
                pillar: Pillar::Resilience,
                reason_code: REASON_RES_ALARM_INSUFFICIENT_DATA.to_string(),
                severity: Severity::Medium,
                message: format!(
                    "Alarm {} is in INSUFFICIENT_DATA state; its metric signal is broken and it cannot detect incidents",
                    resource.resource_id
                ),
                evidence: json!({
                    "StateValue": state,
                    "MetricName": resource.resource_data.get("MetricName"),
                    "Namespace": resource.resource_data.get("Namespace"),
                }),
            });
        }
        Some(_) => {}
        None => {
            findings.push(InventoryFinding {
                resource_id: resource.resource_id.clone(),
                arn: resource.arn.clone(),
                pillar: Pillar::Resilience,
                reason_code: REASON_RES_ALARM_STATE_DATA_NOT_COLLECTED.to_string(),
                severity: Severity::Medium,
                message: format!(
                    "Alarm state for {} is not collected yet; alarm health cannot be assessed",
                    resource.resource_id
                ),
                evidence: json!({ "state_value_collected": false }),
            });
        }
    }

    // Alarm actions (AlarmActions, ActionsEnabled) are not persisted by the
    // collector yet, so an alarm that fires into the void cannot be told
    // apart from a fully wired one. Report the gap once per alarm.
    if resource.resource_data.get("AlarmActions").is_none()
        && resource.resource_data.get("ActionsEnabled").is_none()
    {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Resilience,
            reason_code: REASON_RES_ALARM_ACTION_DATA_NOT_COLLECTED.to_string(),
            severity: Severity::Low,
            message: format!(
                "Alarm actions for {} are not collected yet; whether the alarm notifies anyone cannot be assessed",
                resource.resource_id
            ),
            evidence: json!({ "action_fields_collected": false }),
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
        resource_type: &str,
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
            resource_type: resource_type.to_string(),
            resource_id: resource_id.to_string(),
            arn: format!("arn:aws:cloudwatch:us-east-1:123456789012:{}", resource_id),
            name: Some(resource_id.to_string()),
            tags,
            resource_data,
            created_at: refreshed,
            updated_at: refreshed,
            last_refreshed: refreshed,
        }
    }

    fn alarm_fixture(
        resource_id: &str,
        tags: Value,
        state: Value,
        now: DateTime<Utc>,
    ) -> AwsResourceModel {
        // ActionsEnabled is not persisted by the collector today; tests set
        // it explicitly to exercise the gap firing and staying quiet.
        let mut data = json!({
            "AlarmName": resource_id,
            "AlarmArn": format!("arn:aws:cloudwatch:us-east-1:123456789012:alarm:{}", resource_id),
            "AlarmDescription": "cpu high",
            "StateValue": state,
            "MetricName": "CPUUtilization",
            "Namespace": "AWS/EC2",
        });
        data["ActionsEnabled"] = json!(true);
        fixture("CloudWatchAlarm", resource_id, tags, data, now)
    }

    fn dashboard_fixture(resource_id: &str, tags: Value, now: DateTime<Utc>) -> AwsResourceModel {
        let data = json!({
            "DashboardName": resource_id,
            "DashboardArn": format!("arn:aws:cloudwatch::123456789012:dashboard/{}", resource_id),
            "Size": 2048,
            "LastModified": "2026-06-01T00:00:00Z",
        });
        fixture("CloudWatchDashboard", resource_id, tags, data, now)
    }

    fn now() -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2026-06-10T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc)
    }

    #[test]
    fn cost_flags_untagged_alarm_and_dashboard() {
        let alarm = alarm_fixture("cpu-high", json!({}), json!("OK"), now());
        let dashboard = dashboard_fixture("ops-overview", json!({}), now());
        let report = evaluate_cloudwatch_fleet(&[alarm, dashboard], Pillar::Cost, now());
        let codes: Vec<&str> = report
            .findings
            .iter()
            .map(|f| f.reason_code.as_str())
            .collect();
        assert_eq!(codes, vec![REASON_COST_NO_TAGS, REASON_COST_NO_TAGS]);
    }

    #[test]
    fn resilience_flags_insufficient_data_alarm() {
        let r = alarm_fixture(
            "broken-signal",
            json!({"team": "obs"}),
            json!("INSUFFICIENT_DATA"),
            now(),
        );
        let report = evaluate_cloudwatch_fleet(&[r], Pillar::Resilience, now());
        let codes: Vec<&str> = report
            .findings
            .iter()
            .map(|f| f.reason_code.as_str())
            .collect();
        assert!(codes.contains(&REASON_RES_ALARM_INSUFFICIENT_DATA));
    }

    #[test]
    fn resilience_reports_gap_when_alarm_state_not_collected() {
        let r = alarm_fixture("no-state", json!({"team": "obs"}), json!(null), now());
        let report = evaluate_cloudwatch_fleet(&[r], Pillar::Resilience, now());
        let codes: Vec<&str> = report
            .findings
            .iter()
            .map(|f| f.reason_code.as_str())
            .collect();
        assert!(codes.contains(&REASON_RES_ALARM_STATE_DATA_NOT_COLLECTED));
    }

    #[test]
    fn resilience_reports_gap_when_alarm_actions_not_collected() {
        // Build an alarm exactly as the collector persists it today: no
        // AlarmActions and no ActionsEnabled keys at all.
        let r = fixture(
            "CloudWatchAlarm",
            "void-alarm",
            json!({"team": "obs"}),
            json!({
                "AlarmName": "void-alarm",
                "AlarmArn": "arn:aws:cloudwatch:us-east-1:123456789012:alarm:void-alarm",
                "AlarmDescription": null,
                "StateValue": "OK",
                "MetricName": "CPUUtilization",
                "Namespace": "AWS/EC2",
            }),
            now(),
        );
        let report = evaluate_cloudwatch_fleet(&[r], Pillar::Resilience, now());
        assert_eq!(
            report
                .findings
                .iter()
                .map(|f| f.reason_code.as_str())
                .collect::<Vec<_>>(),
            vec![REASON_RES_ALARM_ACTION_DATA_NOT_COLLECTED]
        );
    }

    #[test]
    fn stale_inventory_is_flagged() {
        let stale_now = now() - Duration::hours(48);
        let r = alarm_fixture("old-alarm", json!({"team": "obs"}), json!("OK"), stale_now);
        let report = evaluate_cloudwatch_fleet(&[r], Pillar::Cost, now());
        assert_eq!(report.stale_resources, 1);
        let codes: Vec<&str> = report
            .findings
            .iter()
            .map(|f| f.reason_code.as_str())
            .collect();
        assert!(codes.contains(&REASON_INV_STALE_DATA));
    }

    #[test]
    fn healthy_resources_pass_all_pillars() {
        let alarm = alarm_fixture("cpu-ok", json!({"team": "obs"}), json!("OK"), now());
        let dashboard = dashboard_fixture("ops-ok", json!({"team": "obs"}), now());
        let fleet = vec![alarm, dashboard];
        for pillar in [Pillar::Cost, Pillar::Security, Pillar::Resilience] {
            let report = evaluate_cloudwatch_fleet(&fleet, pillar, now());
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

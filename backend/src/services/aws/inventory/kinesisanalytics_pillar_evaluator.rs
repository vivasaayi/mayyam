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

// Deterministic Kinesis Analytics (Managed Flink) inventory evaluators for
// the cost, security, and resilience pillars (roadmap rows
// 01-AWS-CLOUD-01954/01963/01990).
//
// Evaluates fields persisted by kinesisanalytics_control_plane:
// ApplicationName, ApplicationARN, ApplicationStatus, ApplicationVersionId,
// RuntimeEnvironment. The collector persists no security posture fields, so
// the security pillar reports an honest data gap instead of guessing.

use chrono::{DateTime, Utc};
use serde_json::json;

use crate::models::aws_resource::Model as AwsResourceModel;
use crate::services::aws::inventory::types::{
    check_stale, score_pillar, InventoryFinding, Pillar, PillarReport, Severity,
};

const APPLICATION_RESOURCE_TYPE: &str = "KinesisAnalyticsApp";

// Reason codes are the stable contract for findings; never reuse or rename.
pub const REASON_COST_NO_TAGS: &str = "KINESISANALYTICS_COST_NO_TAGS";
pub const REASON_COST_IDLE_APPLICATION: &str = "KINESISANALYTICS_COST_IDLE_APPLICATION";
pub const REASON_SEC_POSTURE_DATA_NOT_COLLECTED: &str =
    "KINESISANALYTICS_SEC_POSTURE_DATA_NOT_COLLECTED";
pub const REASON_RES_APP_NOT_RUNNING: &str = "KINESISANALYTICS_RES_APP_NOT_RUNNING";
pub const REASON_RES_STATUS_DATA_NOT_COLLECTED: &str =
    "KINESISANALYTICS_RES_STATUS_DATA_NOT_COLLECTED";
pub const REASON_INV_STALE_DATA: &str = "KINESISANALYTICS_INV_STALE_DATA";

/// Evaluate every Kinesis Analytics application in the fleet for one pillar.
pub fn evaluate_kinesisanalytics_fleet(
    resources: &[AwsResourceModel],
    pillar: Pillar,
    now: DateTime<Utc>,
) -> PillarReport {
    let mut findings: Vec<InventoryFinding> = Vec::new();
    let mut stale_resources = 0usize;
    let mut evaluated = 0usize;

    for resource in resources {
        // Skip rows that are not Kinesis Analytics applications gracefully.
        if resource.resource_type != APPLICATION_RESOURCE_TYPE {
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

/// Application status as persisted by the collector, normalized to uppercase
/// for deterministic comparison (the SDK persists values such as `RUNNING`,
/// `READY`, `STOPPED`).
fn application_status(resource: &AwsResourceModel) -> Option<String> {
    resource
        .resource_data
        .get("ApplicationStatus")
        .and_then(|v| v.as_str())
        .map(|s| s.trim().to_ascii_uppercase())
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
                "Application {} has no tags recorded (untagged resource or tag collection gap); cost allocation cannot be assessed",
                resource.resource_id
            ),
            evidence: json!({ "tags": resource.tags }),
        });
    }

    // A READY/STOPPED Flink application does not bill KPU compute, so this is
    // an idle-application governance signal, not a direct waste finding.
    if let Some(status) = application_status(resource) {
        if status == "READY" || status == "STOPPED" {
            findings.push(InventoryFinding {
                resource_id: resource.resource_id.clone(),
                arn: resource.arn.clone(),
                pillar: Pillar::Cost,
                reason_code: REASON_COST_IDLE_APPLICATION.to_string(),
                severity: Severity::Low,
                message: format!(
                    "Application {} is in {} state; it is not billing compute while stopped, but an idle application definition may indicate an abandoned job and any durable application storage still accrues cost",
                    resource.resource_id, status
                ),
                evidence: json!({ "ApplicationStatus": status }),
            });
        }
    }
}

fn evaluate_security(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    // The collector persists only name/ARN/status/version/runtime; no
    // encryption, VPC, or IAM posture fields are collected yet. Report the
    // gap honestly instead of inferring posture from absent data.
    findings.push(InventoryFinding {
        resource_id: resource.resource_id.clone(),
        arn: resource.arn.clone(),
        pillar: Pillar::Security,
        reason_code: REASON_SEC_POSTURE_DATA_NOT_COLLECTED.to_string(),
        severity: Severity::Medium,
        message: format!(
            "Security posture fields (encryption, VPC configuration, service role) for application {} are not collected yet; security pillar cannot be assessed from inventory",
            resource.resource_id
        ),
        evidence: json!({
            "security_fields_collected": false,
            "collected_fields": [
                "ApplicationName",
                "ApplicationARN",
                "ApplicationStatus",
                "ApplicationVersionId",
                "RuntimeEnvironment",
            ],
        }),
    });
}

fn evaluate_resilience(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    match application_status(resource) {
        Some(status) => {
            if status != "RUNNING" {
                findings.push(InventoryFinding {
                    resource_id: resource.resource_id.clone(),
                    arn: resource.arn.clone(),
                    pillar: Pillar::Resilience,
                    reason_code: REASON_RES_APP_NOT_RUNNING.to_string(),
                    severity: Severity::Medium,
                    message: format!(
                        "Application {} is in {} state, not RUNNING; its stream processing is not active and downstream consumers may be falling behind",
                        resource.resource_id, status
                    ),
                    evidence: json!({ "ApplicationStatus": status }),
                });
            }
        }
        None => {
            findings.push(InventoryFinding {
                resource_id: resource.resource_id.clone(),
                arn: resource.arn.clone(),
                pillar: Pillar::Resilience,
                reason_code: REASON_RES_STATUS_DATA_NOT_COLLECTED.to_string(),
                severity: Severity::Medium,
                message: format!(
                    "Application status for {} is not collected; resilience pillar cannot be assessed from inventory",
                    resource.resource_id
                ),
                evidence: json!({ "ApplicationStatus": null }),
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
            resource_type: "KinesisAnalyticsApp".to_string(),
            resource_id: resource_id.to_string(),
            arn: format!(
                "arn:aws:kinesisanalytics:us-east-1:123456789012:application/{}",
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

    fn app_data(status: &str) -> Value {
        json!({
            "ApplicationName": "orders-enrichment",
            "ApplicationARN": "arn:aws:kinesisanalytics:us-east-1:123456789012:application/orders-enrichment",
            "ApplicationStatus": status,
            "ApplicationVersionId": 3,
            "RuntimeEnvironment": "FLINK-1_18",
        })
    }

    #[test]
    fn cost_flags_missing_tags() {
        let r = fixture("app-untagged", json!({}), app_data("RUNNING"), now());
        let report = evaluate_kinesisanalytics_fleet(&[r], Pillar::Cost, now());
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
    fn cost_flags_idle_ready_application() {
        let r = fixture(
            "app-idle",
            json!({"team": "stream"}),
            app_data("READY"),
            now(),
        );
        let report = evaluate_kinesisanalytics_fleet(&[r], Pillar::Cost, now());
        assert_eq!(
            report
                .findings
                .iter()
                .map(|f| f.reason_code.as_str())
                .collect::<Vec<_>>(),
            vec![REASON_COST_IDLE_APPLICATION]
        );
        assert_eq!(report.findings[0].severity, Severity::Low);
    }

    #[test]
    fn security_reports_posture_data_gap() {
        let r = fixture(
            "app-sec",
            json!({"team": "stream"}),
            app_data("RUNNING"),
            now(),
        );
        let report = evaluate_kinesisanalytics_fleet(&[r], Pillar::Security, now());
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
    fn resilience_flags_application_not_running() {
        let r = fixture(
            "app-stopped",
            json!({"team": "stream"}),
            app_data("STOPPING"),
            now(),
        );
        let report = evaluate_kinesisanalytics_fleet(&[r], Pillar::Resilience, now());
        assert_eq!(
            report
                .findings
                .iter()
                .map(|f| f.reason_code.as_str())
                .collect::<Vec<_>>(),
            vec![REASON_RES_APP_NOT_RUNNING]
        );
    }

    #[test]
    fn resilience_reports_gap_when_status_not_collected() {
        let r = fixture(
            "app-nostatus",
            json!({"team": "stream"}),
            json!({"ApplicationName": "app-nostatus"}),
            now(),
        );
        let report = evaluate_kinesisanalytics_fleet(&[r], Pillar::Resilience, now());
        assert_eq!(
            report
                .findings
                .iter()
                .map(|f| f.reason_code.as_str())
                .collect::<Vec<_>>(),
            vec![REASON_RES_STATUS_DATA_NOT_COLLECTED]
        );
    }

    #[test]
    fn stale_inventory_is_flagged() {
        let mut r = fixture(
            "app-stale",
            json!({"team": "stream"}),
            app_data("RUNNING"),
            now(),
        );
        r.last_refreshed = now() - Duration::hours(48);
        let report = evaluate_kinesisanalytics_fleet(&[r], Pillar::Resilience, now());
        assert_eq!(report.stale_resources, 1);
        assert!(report
            .findings
            .iter()
            .any(|f| f.reason_code == REASON_INV_STALE_DATA));
    }

    #[test]
    fn non_kinesisanalytics_rows_are_skipped() {
        let mut r = fixture("stream-1", json!({}), json!({}), now());
        r.resource_type = "KinesisStream".to_string();
        let report = evaluate_kinesisanalytics_fleet(&[r], Pillar::Cost, now());
        assert_eq!(report.resources_evaluated, 0);
        assert!(report.findings.is_empty());
    }

    #[test]
    fn healthy_running_application_passes_cost_and_resilience() {
        let r = fixture(
            "app-ok",
            json!({"team": "stream"}),
            app_data("RUNNING"),
            now(),
        );
        for pillar in [Pillar::Cost, Pillar::Resilience] {
            let report = evaluate_kinesisanalytics_fleet(std::slice::from_ref(&r), pillar, now());
            assert!(
                report.findings.is_empty(),
                "unexpected for {:?}: {:?}",
                pillar,
                report.findings
            );
        }
        // Security always reports the honest posture data gap until the
        // collector persists security fields.
        let security = evaluate_kinesisanalytics_fleet(&[r], Pillar::Security, now());
        assert_eq!(
            security
                .findings
                .iter()
                .map(|f| f.reason_code.as_str())
                .collect::<Vec<_>>(),
            vec![REASON_SEC_POSTURE_DATA_NOT_COLLECTED]
        );
    }
}

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

// Deterministic AWS Config rule inventory evaluators for the cost, security,
// resilience, performance, scalability, disaster-recovery, and
// operational-excellence pillars.
//
// Evaluates fields persisted by config_control_plane: config_rule_state,
// maximum_execution_frequency, evaluation_status_collected,
// last_successful_evaluation_time, last_failed_evaluation_time,
// last_error_code, last_error_message, plus the tags column.
// maximum_execution_frequency is only present for periodic rules, so the
// evaluation-staleness check is gated on it; change-triggered rules evaluate
// on configuration changes and have no deterministic cadence to judge.

use chrono::{DateTime, Utc};
use serde_json::json;

use crate::models::aws_resource::Model as AwsResourceModel;
use crate::services::aws::inventory::types::{
    check_stale, data_str, has_any_tag, score_pillar, InventoryFinding, Pillar, PillarReport,
    Severity, COST_ALLOCATION_TAG_KEYS, OWNER_TAG_KEYS,
};

/// Only rows of this resource type are evaluated.
pub const RESOURCE_TYPE: &str = "ConfigRule";

/// A periodic rule is stale once its last successful evaluation is older
/// than this multiple of its configured execution frequency.
pub const STALE_EVALUATION_FREQUENCY_MULTIPLIER: i64 = 2;

// Reason codes are the stable contract for findings; never reuse or rename.
pub const REASON_COST_ACTIVE_RULE_NEVER_SUCCEEDED: &str = "CONFIG_COST_ACTIVE_RULE_NEVER_SUCCEEDED";
pub const REASON_COST_HIGH_FREQUENCY_UNTAGGED_RULE: &str =
    "CONFIG_COST_HIGH_FREQUENCY_UNTAGGED_RULE";
pub const REASON_SEC_RULE_EVALUATION_FAILING: &str = "CONFIG_SEC_RULE_EVALUATION_FAILING";
pub const REASON_SEC_RULE_NOT_ACTIVE: &str = "CONFIG_SEC_RULE_NOT_ACTIVE";
pub const REASON_RES_EVALUATION_STALE: &str = "CONFIG_RES_EVALUATION_STALE";
pub const REASON_RES_LAST_EVALUATION_FAILED: &str = "CONFIG_RES_LAST_EVALUATION_FAILED";
pub const REASON_RES_EVALUATION_TIME_UNPARSEABLE: &str = "CONFIG_RES_EVALUATION_TIME_UNPARSEABLE";
pub const REASON_PERF_INTERMITTENT_EVALUATION_FAILURES: &str =
    "CONFIG_PERF_INTERMITTENT_EVALUATION_FAILURES";
pub const REASON_SCALE_HOURLY_EVALUATION: &str = "CONFIG_SCALE_HOURLY_EVALUATION";
pub const REASON_DR_NEVER_EVALUATED: &str = "CONFIG_DR_NEVER_EVALUATED";
pub const REASON_OPEX_NO_OWNER_TAG: &str = "CONFIG_OPEX_NO_OWNER_TAG";
pub const REASON_DATA_GAP_EVALUATION_STATUS: &str = "CONFIG_DATA_GAP_EVALUATION_STATUS";
pub const REASON_INV_STALE_DATA: &str = "CONFIG_INV_STALE_DATA";

/// Evaluate every Config rule in the fleet for one pillar. Rows whose
/// `resource_type` is not `ConfigRule` are skipped and not counted.
pub fn evaluate_config_fleet(
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
            Pillar::Resilience => evaluate_resilience(resource, now, &mut findings),
            Pillar::Performance => evaluate_performance(resource, &mut findings),
            Pillar::Scalability => evaluate_scalability(resource, &mut findings),
            Pillar::DisasterRecovery => evaluate_disaster_recovery(resource, &mut findings),
            Pillar::OperationalExcellence => {
                evaluate_operational_excellence(resource, &mut findings)
            }
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

fn rule_state(resource: &AwsResourceModel) -> Option<String> {
    data_str(&resource.resource_data, "config_rule_state")
}

fn evaluation_status_collected(resource: &AwsResourceModel) -> bool {
    resource
        .resource_data
        .get("evaluation_status_collected")
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
}

fn parse_time(resource: &AwsResourceModel, key: &str) -> Option<Result<DateTime<Utc>, String>> {
    data_str(&resource.resource_data, key).map(|raw| {
        DateTime::parse_from_rfc3339(&raw)
            .map(|t| t.with_timezone(&Utc))
            .map_err(|_| raw)
    })
}

/// Hours between evaluations for a periodic rule. `None` means the rule is
/// change-triggered and has no deterministic cadence.
fn frequency_hours(resource: &AwsResourceModel) -> Option<(String, i64)> {
    let frequency = data_str(&resource.resource_data, "maximum_execution_frequency")?;
    let hours = match frequency.as_str() {
        "One_Hour" => 1,
        "Three_Hours" => 3,
        "Six_Hours" => 6,
        "Twelve_Hours" => 12,
        "TwentyFour_Hours" => 24,
        _ => 24,
    };
    Some((frequency, hours))
}

fn data_gap_finding(resource: &AwsResourceModel, pillar: Pillar) -> InventoryFinding {
    InventoryFinding {
        resource_id: resource.resource_id.clone(),
        arn: resource.arn.clone(),
        pillar,
        reason_code: REASON_DATA_GAP_EVALUATION_STATUS.to_string(),
        severity: Severity::Low,
        message: format!(
            "Evaluation status for Config rule {} is not collected yet; evaluation health cannot be assessed",
            resource.resource_id
        ),
        evidence: json!({ "evaluation_status_collected": false }),
    }
}

fn evaluate_cost(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    // An ACTIVE rule that errors and has never produced a successful
    // evaluation bills per evaluation attempt while returning no compliance
    // signal at all.
    if rule_state(resource).as_deref() == Some("ACTIVE") && evaluation_status_collected(resource) {
        let error_code = data_str(&resource.resource_data, "last_error_code");
        let never_succeeded = resource
            .resource_data
            .get("last_successful_evaluation_time")
            .is_none();
        if let Some(code) = error_code {
            if never_succeeded {
                findings.push(InventoryFinding {
                    resource_id: resource.resource_id.clone(),
                    arn: resource.arn.clone(),
                    pillar: Pillar::Cost,
                    reason_code: REASON_COST_ACTIVE_RULE_NEVER_SUCCEEDED.to_string(),
                    severity: Severity::Medium,
                    message: format!(
                        "Config rule {} is ACTIVE, has never evaluated successfully, and reports error {}; it bills for evaluations without producing compliance data",
                        resource.resource_id, code
                    ),
                    evidence: json!({
                        "config_rule_state": "ACTIVE",
                        "last_error_code": code,
                        "last_error_message": data_str(&resource.resource_data, "last_error_message"),
                        "last_successful_evaluation_time": null,
                    }),
                });
            }
        }
    }

    // Hourly evaluation is the most expensive cadence; without cost
    // allocation tags the spend cannot be attributed or justified.
    if let Some((frequency, _)) = frequency_hours(resource) {
        if frequency == "One_Hour" && !has_any_tag(&resource.tags, COST_ALLOCATION_TAG_KEYS) {
            findings.push(InventoryFinding {
                resource_id: resource.resource_id.clone(),
                arn: resource.arn.clone(),
                pillar: Pillar::Cost,
                reason_code: REASON_COST_HIGH_FREQUENCY_UNTAGGED_RULE.to_string(),
                severity: Severity::Low,
                message: format!(
                    "Config rule {} evaluates every hour (the most expensive cadence) but carries no cost allocation tag; spend cannot be attributed",
                    resource.resource_id
                ),
                evidence: json!({
                    "maximum_execution_frequency": frequency,
                    "tags": resource.tags,
                    "accepted_tag_keys": COST_ALLOCATION_TAG_KEYS,
                }),
            });
        }
    }
}

fn evaluate_security(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    // A rule that is not ACTIVE is not enforcing its compliance check.
    if let Some(state) = rule_state(resource) {
        if state != "ACTIVE" {
            findings.push(InventoryFinding {
                resource_id: resource.resource_id.clone(),
                arn: resource.arn.clone(),
                pillar: Pillar::Security,
                reason_code: REASON_SEC_RULE_NOT_ACTIVE.to_string(),
                severity: Severity::Medium,
                message: format!(
                    "Config rule {} is in state {}; it is not providing compliance coverage while in this state",
                    resource.resource_id, state
                ),
                evidence: json!({ "config_rule_state": state }),
            });
        }
    }

    if !evaluation_status_collected(resource) {
        findings.push(data_gap_finding(resource, Pillar::Security));
        return;
    }

    // A rule whose latest error has not been followed by a successful
    // evaluation is a compliance blind spot: violations go undetected.
    if let Some(code) = data_str(&resource.resource_data, "last_error_code") {
        let success = parse_time(resource, "last_successful_evaluation_time").and_then(|r| r.ok());
        let failed = parse_time(resource, "last_failed_evaluation_time").and_then(|r| r.ok());
        let unrecovered = match (failed, success) {
            (_, None) => true,
            (Some(f), Some(s)) => f >= s,
            (None, Some(_)) => false,
        };
        if unrecovered {
            findings.push(InventoryFinding {
                resource_id: resource.resource_id.clone(),
                arn: resource.arn.clone(),
                pillar: Pillar::Security,
                reason_code: REASON_SEC_RULE_EVALUATION_FAILING.to_string(),
                severity: Severity::High,
                message: format!(
                    "Config rule {} is failing to evaluate (error {}) with no successful evaluation since; non-compliant resources are going undetected",
                    resource.resource_id, code
                ),
                evidence: json!({
                    "last_error_code": code,
                    "last_error_message": data_str(&resource.resource_data, "last_error_message"),
                    "last_failed_evaluation_time": data_str(&resource.resource_data, "last_failed_evaluation_time"),
                    "last_successful_evaluation_time": data_str(&resource.resource_data, "last_successful_evaluation_time"),
                }),
            });
        }
    }
}

fn evaluate_resilience(
    resource: &AwsResourceModel,
    now: DateTime<Utc>,
    findings: &mut Vec<InventoryFinding>,
) {
    if !evaluation_status_collected(resource) {
        findings.push(data_gap_finding(resource, Pillar::Resilience));
        return;
    }

    // The most recent evaluation attempt failed: the rule's compliance view
    // is no longer being refreshed.
    let success = parse_time(resource, "last_successful_evaluation_time");
    let failed = parse_time(resource, "last_failed_evaluation_time");
    if let (Some(Ok(failed_at)), Some(Ok(succeeded_at))) = (&failed, &success) {
        if failed_at > succeeded_at {
            findings.push(InventoryFinding {
                resource_id: resource.resource_id.clone(),
                arn: resource.arn.clone(),
                pillar: Pillar::Resilience,
                reason_code: REASON_RES_LAST_EVALUATION_FAILED.to_string(),
                severity: Severity::High,
                message: format!(
                    "Config rule {} failed its most recent evaluation after its last success; its compliance view is no longer being refreshed",
                    resource.resource_id
                ),
                evidence: json!({
                    "last_failed_evaluation_time": data_str(&resource.resource_data, "last_failed_evaluation_time"),
                    "last_successful_evaluation_time": data_str(&resource.resource_data, "last_successful_evaluation_time"),
                }),
            });
        }
    }

    // Periodic rules must keep evaluating on their configured cadence; a
    // last success far older than the cadence means the schedule is broken.
    if let Some((frequency, hours)) = frequency_hours(resource) {
        match success {
            Some(Ok(succeeded_at)) => {
                let age_hours = (now - succeeded_at).num_hours();
                let threshold = hours * STALE_EVALUATION_FREQUENCY_MULTIPLIER;
                if age_hours > threshold {
                    findings.push(InventoryFinding {
                        resource_id: resource.resource_id.clone(),
                        arn: resource.arn.clone(),
                        pillar: Pillar::Resilience,
                        reason_code: REASON_RES_EVALUATION_STALE.to_string(),
                        severity: Severity::Medium,
                        message: format!(
                            "Config rule {} last evaluated successfully {} hours ago but is scheduled every {} hours (threshold {} hours); its periodic schedule appears broken",
                            resource.resource_id, age_hours, hours, threshold
                        ),
                        evidence: json!({
                            "maximum_execution_frequency": frequency,
                            "last_successful_evaluation_time": data_str(&resource.resource_data, "last_successful_evaluation_time"),
                            "age_hours": age_hours,
                            "threshold_hours": threshold,
                        }),
                    });
                }
            }
            Some(Err(raw)) => {
                findings.push(InventoryFinding {
                    resource_id: resource.resource_id.clone(),
                    arn: resource.arn.clone(),
                    pillar: Pillar::Resilience,
                    reason_code: REASON_RES_EVALUATION_TIME_UNPARSEABLE.to_string(),
                    severity: Severity::Low,
                    message: format!(
                        "Last successful evaluation time for Config rule {} could not be parsed; evaluation staleness cannot be assessed",
                        resource.resource_id
                    ),
                    evidence: json!({ "last_successful_evaluation_time": raw }),
                });
            }
            // No successful evaluation at all is covered by the failing /
            // never-succeeded checks; nothing deterministic to add here.
            None => {}
        }
    }
}

fn evaluate_performance(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    if !evaluation_status_collected(resource) {
        findings.push(data_gap_finding(resource, Pillar::Performance));
        return;
    }

    // An error that a later evaluation recovered from is not a compliance
    // blind spot (the security pillar covers unrecovered errors), but it
    // means detection latency degraded while evaluations were failing.
    if let Some(code) = data_str(&resource.resource_data, "last_error_code") {
        let success = parse_time(resource, "last_successful_evaluation_time").and_then(|r| r.ok());
        let failed = parse_time(resource, "last_failed_evaluation_time").and_then(|r| r.ok());
        let recovered = match (failed, success) {
            (_, None) => false,
            (Some(f), Some(s)) => s > f,
            (None, Some(_)) => true,
        };
        if recovered {
            findings.push(InventoryFinding {
                resource_id: resource.resource_id.clone(),
                arn: resource.arn.clone(),
                pillar: Pillar::Performance,
                reason_code: REASON_PERF_INTERMITTENT_EVALUATION_FAILURES.to_string(),
                severity: Severity::Low,
                message: format!(
                    "Config rule {} recovered after evaluation error {}; detection latency was degraded while evaluations failed",
                    resource.resource_id, code
                ),
                evidence: json!({
                    "last_error_code": code,
                    "last_error_message": data_str(&resource.resource_data, "last_error_message"),
                    "last_failed_evaluation_time": data_str(&resource.resource_data, "last_failed_evaluation_time"),
                    "last_successful_evaluation_time": data_str(&resource.resource_data, "last_successful_evaluation_time"),
                }),
            });
        }
    }
}

fn evaluate_scalability(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    // An hourly periodic rule re-evaluates its full resource scope every
    // hour; evaluation volume grows linearly with the resource inventory.
    if let Some((frequency, _)) = frequency_hours(resource) {
        if frequency == "One_Hour" {
            findings.push(InventoryFinding {
                resource_id: resource.resource_id.clone(),
                arn: resource.arn.clone(),
                pillar: Pillar::Scalability,
                reason_code: REASON_SCALE_HOURLY_EVALUATION.to_string(),
                severity: Severity::Low,
                message: format!(
                    "Config rule {} evaluates its full scope every hour; evaluation volume grows linearly with the resource inventory, so consider a change-triggered scope instead",
                    resource.resource_id
                ),
                evidence: json!({ "maximum_execution_frequency": frequency }),
            });
        }
    }
}

fn evaluate_disaster_recovery(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    if !evaluation_status_collected(resource) {
        findings.push(data_gap_finding(resource, Pillar::DisasterRecovery));
        return;
    }

    // An ACTIVE rule that has never evaluated successfully has produced no
    // compliance baseline; after an incident there is nothing recorded to
    // validate recovery against.
    if rule_state(resource).as_deref() == Some("ACTIVE")
        && resource
            .resource_data
            .get("last_successful_evaluation_time")
            .is_none()
    {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::DisasterRecovery,
            reason_code: REASON_DR_NEVER_EVALUATED.to_string(),
            severity: Severity::Medium,
            message: format!(
                "Config rule {} is ACTIVE but has never evaluated successfully; no compliance baseline exists to validate recovery against",
                resource.resource_id
            ),
            evidence: json!({
                "config_rule_state": "ACTIVE",
                "last_successful_evaluation_time": null,
                "last_error_code": data_str(&resource.resource_data, "last_error_code"),
            }),
        });
    }
}

fn evaluate_operational_excellence(
    resource: &AwsResourceModel,
    findings: &mut Vec<InventoryFinding>,
) {
    if !has_any_tag(&resource.tags, OWNER_TAG_KEYS) {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::OperationalExcellence,
            reason_code: REASON_OPEX_NO_OWNER_TAG.to_string(),
            severity: Severity::Medium,
            message: format!(
                "Config rule {} carries no owner or team tag; findings and incidents for it cannot be routed to an owner",
                resource.resource_id
            ),
            evidence: json!({
                "tags": resource.tags,
                "accepted_tag_keys": OWNER_TAG_KEYS,
            }),
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
                "arn:aws:config:us-east-1:123456789012:config-rule/{}",
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

    fn healthy_rule_data() -> Value {
        json!({
            "rule_name": "rule-ok",
            "rule_id": "config-rule-abc123",
            "config_rule_state": "ACTIVE",
            "source_owner": "AWS",
            "source_identifier": "S3_BUCKET_PUBLIC_READ_PROHIBITED",
            "maximum_execution_frequency": "TwentyFour_Hours",
            "evaluation_status_collected": true,
            "last_successful_evaluation_time": "2026-06-09T20:00:00Z",
            "first_activated_time": "2025-01-01T00:00:00Z",
            "first_evaluation_started": true,
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
    fn cost_flags_active_rule_that_never_succeeded() {
        let mut data = healthy_rule_data();
        data.as_object_mut()
            .unwrap()
            .remove("last_successful_evaluation_time");
        data["last_error_code"] = json!("INSUFFICIENT_DELIVERY_POLICY");
        data["last_error_message"] = json!("Delivery channel is not configured");
        data["last_failed_evaluation_time"] = json!("2026-06-09T23:00:00Z");
        let r = fixture("rule-neversucceeded", json!({"team": "sre"}), data, now());
        let report = evaluate_config_fleet(&[r], Pillar::Cost, now());
        assert_eq!(
            codes(&report),
            vec![REASON_COST_ACTIVE_RULE_NEVER_SUCCEEDED]
        );
    }

    #[test]
    fn cost_does_not_flag_failing_rule_that_is_not_active() {
        let mut data = healthy_rule_data();
        data.as_object_mut()
            .unwrap()
            .remove("last_successful_evaluation_time");
        data["config_rule_state"] = json!("DELETING");
        data["last_error_code"] = json!("INSUFFICIENT_DELIVERY_POLICY");
        let r = fixture("rule-deleting", json!({"team": "sre"}), data, now());
        let report = evaluate_config_fleet(&[r], Pillar::Cost, now());
        assert!(
            report.findings.is_empty(),
            "unexpected: {:?}",
            report.findings
        );
    }

    #[test]
    fn cost_flags_hourly_rule_without_cost_allocation_tags() {
        let mut data = healthy_rule_data();
        data["maximum_execution_frequency"] = json!("One_Hour");
        let r = fixture("rule-hourly-untagged", json!({}), data, now());
        let report = evaluate_config_fleet(&[r], Pillar::Cost, now());
        assert_eq!(
            codes(&report),
            vec![REASON_COST_HIGH_FREQUENCY_UNTAGGED_RULE]
        );
    }

    #[test]
    fn cost_passes_hourly_rule_with_cost_allocation_tag() {
        let mut data = healthy_rule_data();
        data["maximum_execution_frequency"] = json!("One_Hour");
        let r = fixture("rule-hourly-tagged", json!({"team": "sre"}), data, now());
        let report = evaluate_config_fleet(&[r], Pillar::Cost, now());
        assert!(
            report.findings.is_empty(),
            "unexpected: {:?}",
            report.findings
        );
    }

    #[test]
    fn security_flags_failing_evaluation_as_high() {
        let mut data = healthy_rule_data();
        data["last_error_code"] = json!("INTERNAL_ERROR");
        data["last_failed_evaluation_time"] = json!("2026-06-09T23:00:00Z");
        let r = fixture("rule-failing", json!({"team": "sre"}), data, now());
        let report = evaluate_config_fleet(&[r], Pillar::Security, now());
        assert_eq!(codes(&report), vec![REASON_SEC_RULE_EVALUATION_FAILING]);
        assert!(matches!(report.findings[0].severity, Severity::High));
    }

    #[test]
    fn security_does_not_flag_error_recovered_by_later_success() {
        let mut data = healthy_rule_data();
        data["last_error_code"] = json!("INTERNAL_ERROR");
        data["last_failed_evaluation_time"] = json!("2026-06-09T10:00:00Z");
        // last_successful_evaluation_time is 2026-06-09T20:00:00Z: recovered.
        let r = fixture("rule-recovered", json!({"team": "sre"}), data, now());
        let report = evaluate_config_fleet(&[r], Pillar::Security, now());
        assert!(
            report.findings.is_empty(),
            "unexpected: {:?}",
            report.findings
        );
    }

    #[test]
    fn security_flags_rule_not_active() {
        let mut data = healthy_rule_data();
        data["config_rule_state"] = json!("DELETING");
        let r = fixture("rule-deleting", json!({"team": "sre"}), data, now());
        let report = evaluate_config_fleet(&[r], Pillar::Security, now());
        assert_eq!(codes(&report), vec![REASON_SEC_RULE_NOT_ACTIVE]);
    }

    #[test]
    fn data_gap_reported_when_evaluation_status_not_collected() {
        let mut data = healthy_rule_data();
        let map = data.as_object_mut().unwrap();
        map.remove("evaluation_status_collected");
        map.remove("last_successful_evaluation_time");
        let r = fixture("rule-gap", json!({"team": "sre"}), data, now());
        for pillar in [Pillar::Security, Pillar::Resilience] {
            let report = evaluate_config_fleet(std::slice::from_ref(&r), pillar, now());
            assert_eq!(
                codes(&report),
                vec![REASON_DATA_GAP_EVALUATION_STATUS],
                "unexpected for {:?}",
                pillar
            );
        }
    }

    #[test]
    fn resilience_flags_failed_after_successful_evaluation() {
        let mut data = healthy_rule_data();
        data["last_failed_evaluation_time"] = json!("2026-06-09T23:00:00Z");
        // last_successful_evaluation_time is 2026-06-09T20:00:00Z: failure is newer.
        let r = fixture("rule-lastfailed", json!({"team": "sre"}), data, now());
        let report = evaluate_config_fleet(&[r], Pillar::Resilience, now());
        assert_eq!(codes(&report), vec![REASON_RES_LAST_EVALUATION_FAILED]);
        assert!(matches!(report.findings[0].severity, Severity::High));
    }

    #[test]
    fn resilience_flags_stale_periodic_evaluation() {
        let mut data = healthy_rule_data();
        data["maximum_execution_frequency"] = json!("One_Hour");
        data["last_successful_evaluation_time"] = json!("2026-06-09T14:00:00Z");
        let r = fixture("rule-stale-eval", json!({"team": "sre"}), data, now());
        let report = evaluate_config_fleet(&[r], Pillar::Resilience, now());
        assert_eq!(codes(&report), vec![REASON_RES_EVALUATION_STALE]);
    }

    #[test]
    fn resilience_does_not_judge_cadence_of_change_triggered_rule() {
        let mut data = healthy_rule_data();
        data.as_object_mut()
            .unwrap()
            .remove("maximum_execution_frequency");
        data["last_successful_evaluation_time"] = json!("2026-05-01T00:00:00Z");
        let r = fixture("rule-change-triggered", json!({"team": "sre"}), data, now());
        let report = evaluate_config_fleet(&[r], Pillar::Resilience, now());
        assert!(
            report.findings.is_empty(),
            "unexpected: {:?}",
            report.findings
        );
    }

    #[test]
    fn resilience_flags_unparseable_evaluation_time() {
        let mut data = healthy_rule_data();
        data["last_successful_evaluation_time"] = json!("not-a-timestamp");
        let r = fixture("rule-badtime", json!({"team": "sre"}), data, now());
        let report = evaluate_config_fleet(&[r], Pillar::Resilience, now());
        assert_eq!(codes(&report), vec![REASON_RES_EVALUATION_TIME_UNPARSEABLE]);
    }

    #[test]
    fn stale_inventory_is_flagged() {
        let mut r = fixture(
            "rule-stale",
            json!({"team": "sre"}),
            healthy_rule_data(),
            now(),
        );
        r.last_refreshed = now() - Duration::hours(48);
        let report = evaluate_config_fleet(&[r], Pillar::Resilience, now());
        assert_eq!(report.stale_resources, 1);
        assert!(codes(&report).contains(&REASON_INV_STALE_DATA));
    }

    #[test]
    fn non_config_resources_are_skipped_and_not_counted() {
        let mut r = fixture("queue-1", json!({}), json!({}), now());
        r.resource_type = "SqsQueue".to_string();
        let report = evaluate_config_fleet(&[r], Pillar::Cost, now());
        assert_eq!(report.resources_evaluated, 0);
        assert!(report.findings.is_empty());
    }

    #[test]
    fn empty_fleet_scores_full_marks() {
        let report = evaluate_config_fleet(&[], Pillar::Security, now());
        assert_eq!(report.resources_evaluated, 0);
        assert!(report.findings.is_empty());
        assert_eq!(report.score, 100);
    }

    #[test]
    fn performance_flags_recovered_evaluation_errors() {
        let mut data = healthy_rule_data();
        data["last_error_code"] = json!("INTERNAL_ERROR");
        data["last_failed_evaluation_time"] = json!("2026-06-09T10:00:00Z");
        // last_successful_evaluation_time is 2026-06-09T20:00:00Z: recovered.
        let r = fixture("rule-perf-recovered", json!({"team": "sre"}), data, now());
        let report = evaluate_config_fleet(&[r], Pillar::Performance, now());
        assert_eq!(
            codes(&report),
            vec![REASON_PERF_INTERMITTENT_EVALUATION_FAILURES]
        );
        assert!(matches!(report.findings[0].severity, Severity::Low));
    }

    #[test]
    fn performance_does_not_flag_unrecovered_errors_and_reports_data_gap() {
        // Unrecovered errors belong to the security pillar.
        let mut data = healthy_rule_data();
        data["last_error_code"] = json!("INTERNAL_ERROR");
        data["last_failed_evaluation_time"] = json!("2026-06-09T23:00:00Z");
        let r = fixture("rule-perf-failing", json!({"team": "sre"}), data, now());
        let report = evaluate_config_fleet(&[r], Pillar::Performance, now());
        assert!(
            report.findings.is_empty(),
            "unexpected: {:?}",
            report.findings
        );

        let mut gap = healthy_rule_data();
        gap.as_object_mut()
            .unwrap()
            .remove("evaluation_status_collected");
        let r2 = fixture("rule-perf-gap", json!({"team": "sre"}), gap, now());
        let report = evaluate_config_fleet(&[r2], Pillar::Performance, now());
        assert_eq!(codes(&report), vec![REASON_DATA_GAP_EVALUATION_STATUS]);
    }

    #[test]
    fn scalability_flags_hourly_evaluation_cadence() {
        let mut data = healthy_rule_data();
        data["maximum_execution_frequency"] = json!("One_Hour");
        let r = fixture("rule-hourly", json!({"team": "sre"}), data, now());
        let report = evaluate_config_fleet(&[r], Pillar::Scalability, now());
        assert_eq!(codes(&report), vec![REASON_SCALE_HOURLY_EVALUATION]);

        // Change-triggered rules (no frequency) have no cadence to flag.
        let mut change_triggered = healthy_rule_data();
        change_triggered
            .as_object_mut()
            .unwrap()
            .remove("maximum_execution_frequency");
        let r2 = fixture("rule-ct", json!({"team": "sre"}), change_triggered, now());
        let report = evaluate_config_fleet(&[r2], Pillar::Scalability, now());
        assert!(
            report.findings.is_empty(),
            "unexpected: {:?}",
            report.findings
        );
    }

    #[test]
    fn disaster_recovery_flags_active_rule_never_evaluated() {
        let mut data = healthy_rule_data();
        data.as_object_mut()
            .unwrap()
            .remove("last_successful_evaluation_time");
        let r = fixture("rule-dr-never", json!({"team": "sre"}), data, now());
        let report = evaluate_config_fleet(&[r], Pillar::DisasterRecovery, now());
        assert_eq!(codes(&report), vec![REASON_DR_NEVER_EVALUATED]);
        assert!(matches!(report.findings[0].severity, Severity::Medium));
    }

    #[test]
    fn disaster_recovery_reports_data_gap_when_status_not_collected() {
        let mut data = healthy_rule_data();
        let map = data.as_object_mut().unwrap();
        map.remove("evaluation_status_collected");
        map.remove("last_successful_evaluation_time");
        let r = fixture("rule-dr-gap", json!({"team": "sre"}), data, now());
        let report = evaluate_config_fleet(&[r], Pillar::DisasterRecovery, now());
        assert_eq!(codes(&report), vec![REASON_DATA_GAP_EVALUATION_STATUS]);
    }

    #[test]
    fn operational_excellence_flags_missing_owner_tag() {
        let r = fixture(
            "rule-unowned",
            json!({"environment": "prod"}),
            healthy_rule_data(),
            now(),
        );
        let report = evaluate_config_fleet(&[r], Pillar::OperationalExcellence, now());
        assert_eq!(codes(&report), vec![REASON_OPEX_NO_OWNER_TAG]);
    }

    #[test]
    fn healthy_rule_passes_all_pillars() {
        let r = fixture(
            "rule-ok",
            json!({"team": "sre"}),
            healthy_rule_data(),
            now(),
        );
        for pillar in [
            Pillar::Cost,
            Pillar::Security,
            Pillar::Resilience,
            Pillar::Performance,
            Pillar::Scalability,
            Pillar::DisasterRecovery,
            Pillar::OperationalExcellence,
        ] {
            let report = evaluate_config_fleet(std::slice::from_ref(&r), pillar, now());
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

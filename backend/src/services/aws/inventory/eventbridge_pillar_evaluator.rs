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

// Deterministic EventBridge rule inventory evaluators for the cost,
// security, resilience, performance, scalability, disaster-recovery, and
// operational-excellence pillars.
//
// Evaluates fields persisted by eventbridge_control_plane: state,
// schedule_expression, event_bus_name, managed_by, target_count, and the
// compact per-target summary (has_dead_letter_config, has_retry_policy,
// retry_max_attempts). Target enrichment can fail per rule without failing
// the sync, so every target-dependent check reports a data gap when
// target_count is absent instead of guessing.

use chrono::{DateTime, Utc};
use serde_json::json;

use crate::models::aws_resource::Model as AwsResourceModel;
use crate::services::aws::inventory::types::{
    check_stale, data_str, has_any_tag, score_pillar, InventoryFinding, Pillar, PillarReport,
    Severity, OWNER_TAG_KEYS,
};

/// Only rows of this resource type are evaluated.
pub const RESOURCE_TYPE: &str = "EventBridgeRule";

// Reason codes are the stable contract for findings; never reuse or rename.
pub const REASON_COST_DISABLED_RULE: &str = "EVENTBRIDGE_COST_DISABLED_RULE";
pub const REASON_COST_ENABLED_NO_TARGETS: &str = "EVENTBRIDGE_COST_ENABLED_NO_TARGETS";
pub const REASON_SEC_CUSTOM_BUS_NO_TARGETS: &str = "EVENTBRIDGE_SEC_CUSTOM_BUS_NO_TARGETS";
pub const REASON_SEC_MANAGED_RULE_DISABLED: &str = "EVENTBRIDGE_SEC_MANAGED_RULE_DISABLED";
pub const REASON_RES_TARGET_NO_DLQ: &str = "EVENTBRIDGE_RES_TARGET_NO_DLQ";
pub const REASON_RES_TARGET_NO_RETRY_POLICY: &str = "EVENTBRIDGE_RES_TARGET_NO_RETRY_POLICY";
pub const REASON_RES_SCHEDULED_RULE_DISABLED: &str = "EVENTBRIDGE_RES_SCHEDULED_RULE_DISABLED";
pub const REASON_PERF_BROAD_EVENT_PATTERN: &str = "EVENTBRIDGE_PERF_BROAD_EVENT_PATTERN";
pub const REASON_PERF_PATTERN_UNPARSEABLE: &str = "EVENTBRIDGE_PERF_PATTERN_UNPARSEABLE";
pub const REASON_SCALE_TARGET_QUOTA_REACHED: &str = "EVENTBRIDGE_SCALE_TARGET_QUOTA_REACHED";
pub const REASON_DR_SCHEDULED_NO_DLQ: &str = "EVENTBRIDGE_DR_SCHEDULED_NO_DLQ";
pub const REASON_OPEX_NO_OWNER_TAG: &str = "EVENTBRIDGE_OPEX_NO_OWNER_TAG";
pub const REASON_DATA_GAP_TARGETS: &str = "EVENTBRIDGE_DATA_GAP_TARGETS";
pub const REASON_INV_STALE_DATA: &str = "EVENTBRIDGE_INV_STALE_DATA";

/// Per-rule target quota in EventBridge; a rule at this count cannot gain
/// more consumers without a fan-out redesign.
pub const TARGET_QUOTA_PER_RULE: u64 = 5;

/// Evaluate every EventBridge rule in the fleet for one pillar. Rows whose
/// `resource_type` is not `EventBridgeRule` are skipped and not counted.
pub fn evaluate_eventbridge_fleet(
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
    data_str(&resource.resource_data, "state")
}

/// ENABLED and ENABLED_WITH_ALL_CLOUDTRAIL_MANAGEMENT_EVENTS both match.
fn is_enabled(resource: &AwsResourceModel) -> bool {
    rule_state(resource)
        .map(|s| s.starts_with("ENABLED"))
        .unwrap_or(false)
}

fn is_disabled(resource: &AwsResourceModel) -> bool {
    rule_state(resource).as_deref() == Some("DISABLED")
}

/// `target_count` is absent when target enrichment failed during sync.
fn target_count(resource: &AwsResourceModel) -> Option<u64> {
    resource
        .resource_data
        .get("target_count")
        .and_then(|v| v.as_u64())
}

fn event_bus_name(resource: &AwsResourceModel) -> Option<String> {
    data_str(&resource.resource_data, "event_bus_name")
}

fn data_gap_targets(resource: &AwsResourceModel, pillar: Pillar) -> InventoryFinding {
    InventoryFinding {
        resource_id: resource.resource_id.clone(),
        arn: resource.arn.clone(),
        pillar,
        reason_code: REASON_DATA_GAP_TARGETS.to_string(),
        severity: Severity::Low,
        message: format!(
            "Target data for rule {} was not collected (ListTargetsByRule failed or has not run); target-dependent checks cannot be assessed",
            resource.resource_id
        ),
        evidence: json!({ "target_count_collected": false }),
    }
}

fn evaluate_cost(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    // A disabled rule does not match events, but it lingers as inventory and
    // operator clutter; it should be deleted or re-enabled deliberately.
    if is_disabled(resource) {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Cost,
            reason_code: REASON_COST_DISABLED_RULE.to_string(),
            severity: Severity::Low,
            message: format!(
                "Rule {} is DISABLED and lingering; delete it if it is no longer needed or re-enable it deliberately",
                resource.resource_id
            ),
            evidence: json!({
                "state": "DISABLED",
                "event_bus_name": event_bus_name(resource),
            }),
        });
        return;
    }

    // An enabled rule with zero targets matches events but delivers nothing:
    // pure matching spend with no outcome.
    if is_enabled(resource) {
        match target_count(resource) {
            Some(0) => {
                findings.push(InventoryFinding {
                    resource_id: resource.resource_id.clone(),
                    arn: resource.arn.clone(),
                    pillar: Pillar::Cost,
                    reason_code: REASON_COST_ENABLED_NO_TARGETS.to_string(),
                    severity: Severity::Medium,
                    message: format!(
                        "Rule {} is ENABLED but has zero targets; matched events are billed and discarded",
                        resource.resource_id
                    ),
                    evidence: json!({
                        "state": rule_state(resource),
                        "target_count": 0,
                    }),
                });
            }
            Some(_) => {}
            None => findings.push(data_gap_targets(resource, Pillar::Cost)),
        }
    }
}

fn evaluate_security(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    // A service-managed rule (managed_by set) that has been disabled means a
    // principal tampered with an AWS-service integration.
    let managed_by = data_str(&resource.resource_data, "managed_by");
    if let Some(manager) = &managed_by {
        if is_disabled(resource) {
            findings.push(InventoryFinding {
                resource_id: resource.resource_id.clone(),
                arn: resource.arn.clone(),
                pillar: Pillar::Security,
                reason_code: REASON_SEC_MANAGED_RULE_DISABLED.to_string(),
                severity: Severity::Medium,
                message: format!(
                    "Rule {} is managed by {} but is DISABLED; a principal may have tampered with a service-managed integration",
                    resource.resource_id, manager
                ),
                evidence: json!({
                    "managed_by": manager,
                    "state": "DISABLED",
                }),
            });
        }
    }

    // An enabled rule on a custom bus with zero targets silently drops the
    // events the bus accepts, hiding activity from downstream consumers.
    let bus = event_bus_name(resource);
    let on_custom_bus = bus.as_deref().map(|b| b != "default").unwrap_or(false);
    if on_custom_bus && is_enabled(resource) {
        match target_count(resource) {
            Some(0) => {
                findings.push(InventoryFinding {
                    resource_id: resource.resource_id.clone(),
                    arn: resource.arn.clone(),
                    pillar: Pillar::Security,
                    reason_code: REASON_SEC_CUSTOM_BUS_NO_TARGETS.to_string(),
                    severity: Severity::Medium,
                    message: format!(
                        "Rule {} on custom bus {} is ENABLED with zero targets; matched events are silently dropped and never reach a consumer",
                        resource.resource_id,
                        bus.as_deref().unwrap_or("")
                    ),
                    evidence: json!({
                        "event_bus_name": bus,
                        "state": rule_state(resource),
                        "target_count": 0,
                    }),
                });
            }
            Some(_) => {}
            None => findings.push(data_gap_targets(resource, Pillar::Security)),
        }
    }
}

fn evaluate_resilience(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    // A scheduled rule that is disabled means the scheduled job is silently
    // not running at all.
    let schedule = data_str(&resource.resource_data, "schedule_expression");
    if let Some(expression) = &schedule {
        if is_disabled(resource) {
            findings.push(InventoryFinding {
                resource_id: resource.resource_id.clone(),
                arn: resource.arn.clone(),
                pillar: Pillar::Resilience,
                reason_code: REASON_RES_SCHEDULED_RULE_DISABLED.to_string(),
                severity: Severity::High,
                message: format!(
                    "Scheduled rule {} ({}) is DISABLED; the scheduled work is silently not running",
                    resource.resource_id, expression
                ),
                evidence: json!({
                    "schedule_expression": expression,
                    "state": "DISABLED",
                }),
            });
        }
    }

    // Delivery durability checks need the collected target summary.
    let count = match target_count(resource) {
        Some(c) => c,
        None => {
            findings.push(data_gap_targets(resource, Pillar::Resilience));
            return;
        }
    };
    if count == 0 {
        // Zero targets is a cost/security concern; nothing to check here.
        return;
    }

    let targets: Vec<&serde_json::Value> = resource
        .resource_data
        .get("targets")
        .and_then(|v| v.as_array())
        .map(|a| a.iter().collect())
        .unwrap_or_default();

    let target_ids_where = |key: &str| -> Vec<String> {
        targets
            .iter()
            .filter(|t| t.get(key).and_then(|v| v.as_bool()) == Some(false))
            .map(|t| {
                t.get("id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string()
            })
            .collect()
    };

    let no_dlq = target_ids_where("has_dead_letter_config");
    if !no_dlq.is_empty() {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Resilience,
            reason_code: REASON_RES_TARGET_NO_DLQ.to_string(),
            severity: Severity::Medium,
            message: format!(
                "Rule {} has {} target(s) without a dead-letter queue; events that exhaust retries are lost permanently",
                resource.resource_id,
                no_dlq.len()
            ),
            evidence: json!({
                "targets_without_dead_letter_config": no_dlq,
                "target_count": count,
            }),
        });
    }

    let no_retry = target_ids_where("has_retry_policy");
    if !no_retry.is_empty() {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Resilience,
            reason_code: REASON_RES_TARGET_NO_RETRY_POLICY.to_string(),
            severity: Severity::Low,
            message: format!(
                "Rule {} has {} target(s) without an explicit retry policy; delivery failure behavior is left to implicit defaults",
                resource.resource_id,
                no_retry.len()
            ),
            evidence: json!({
                "targets_without_retry_policy": no_retry,
                "target_count": count,
            }),
        });
    }
}

fn evaluate_performance(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    // A pattern with neither `source` nor `detail-type` is matched against
    // every event on the bus and ships an unfiltered stream to targets;
    // filtering work the bus could do is pushed onto every consumer.
    let pattern = data_str(&resource.resource_data, "event_pattern");
    let Some(raw) = pattern else {
        // Schedule-only rules have no pattern to assess.
        return;
    };
    match serde_json::from_str::<serde_json::Value>(&raw) {
        Ok(serde_json::Value::Object(map)) => {
            if !map.contains_key("source") && !map.contains_key("detail-type") {
                findings.push(InventoryFinding {
                    resource_id: resource.resource_id.clone(),
                    arn: resource.arn.clone(),
                    pillar: Pillar::Performance,
                    reason_code: REASON_PERF_BROAD_EVENT_PATTERN.to_string(),
                    severity: Severity::Low,
                    message: format!(
                        "Rule {} matches without a source or detail-type filter; every event on the bus is evaluated against it and targets receive an unfiltered stream",
                        resource.resource_id
                    ),
                    evidence: json!({
                        "event_pattern": raw,
                        "pattern_keys": map.keys().collect::<Vec<_>>(),
                    }),
                });
            }
        }
        _ => {
            findings.push(InventoryFinding {
                resource_id: resource.resource_id.clone(),
                arn: resource.arn.clone(),
                pillar: Pillar::Performance,
                reason_code: REASON_PERF_PATTERN_UNPARSEABLE.to_string(),
                severity: Severity::Low,
                message: format!(
                    "Event pattern for rule {} is not a parseable JSON object; pattern breadth cannot be assessed",
                    resource.resource_id
                ),
                evidence: json!({ "event_pattern": raw }),
            });
        }
    }
}

fn evaluate_scalability(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    if !is_enabled(resource) {
        return;
    }
    match target_count(resource) {
        Some(count) if count >= TARGET_QUOTA_PER_RULE => {
            findings.push(InventoryFinding {
                resource_id: resource.resource_id.clone(),
                arn: resource.arn.clone(),
                pillar: Pillar::Scalability,
                reason_code: REASON_SCALE_TARGET_QUOTA_REACHED.to_string(),
                severity: Severity::Medium,
                message: format!(
                    "Rule {} has {} targets, at the per-rule quota of {}; adding a consumer requires another rule or an SNS/SQS fan-out",
                    resource.resource_id, count, TARGET_QUOTA_PER_RULE
                ),
                evidence: json!({
                    "target_count": count,
                    "target_quota_per_rule": TARGET_QUOTA_PER_RULE,
                }),
            });
        }
        Some(_) => {}
        None => findings.push(data_gap_targets(resource, Pillar::Scalability)),
    }
}

fn evaluate_disaster_recovery(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    // Schedule-generated events have no upstream producer to replay them; a
    // failed scheduled invocation without a DLQ is unrecoverable work.
    let schedule = data_str(&resource.resource_data, "schedule_expression");
    let Some(expression) = schedule else {
        return;
    };
    if !is_enabled(resource) {
        // A disabled scheduled rule is already a resilience finding.
        return;
    }
    let targets = resource
        .resource_data
        .get("targets")
        .and_then(|v| v.as_array());
    match (target_count(resource), targets) {
        (Some(0), _) => {}
        (Some(count), Some(targets)) => {
            let no_dlq: Vec<String> = targets
                .iter()
                .filter(|t| {
                    t.get("has_dead_letter_config").and_then(|v| v.as_bool()) == Some(false)
                })
                .map(|t| {
                    t.get("id")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string()
                })
                .collect();
            if !no_dlq.is_empty() {
                findings.push(InventoryFinding {
                    resource_id: resource.resource_id.clone(),
                    arn: resource.arn.clone(),
                    pillar: Pillar::DisasterRecovery,
                    reason_code: REASON_DR_SCHEDULED_NO_DLQ.to_string(),
                    severity: Severity::Medium,
                    message: format!(
                        "Scheduled rule {} ({}) has {} target(s) without a dead-letter queue; schedule-generated events have no producer to replay them, so failed invocations are unrecoverable",
                        resource.resource_id,
                        expression,
                        no_dlq.len()
                    ),
                    evidence: json!({
                        "schedule_expression": expression,
                        "targets_without_dead_letter_config": no_dlq,
                        "target_count": count,
                    }),
                });
            }
        }
        _ => findings.push(data_gap_targets(resource, Pillar::DisasterRecovery)),
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
                "Rule {} carries no owner or team tag; findings and incidents for it cannot be routed to an owner",
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
            arn: format!("arn:aws:events:us-east-1:123456789012:rule/{}", resource_id),
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

    fn healthy_target() -> Value {
        json!({
            "id": "target-1",
            "arn": "arn:aws:sqs:us-east-1:123456789012:queue-1",
            "has_dead_letter_config": true,
            "has_retry_policy": true,
            "retry_max_attempts": 185,
        })
    }

    fn healthy_rule_data() -> Value {
        json!({
            "name": "rule-ok",
            "arn": "arn:aws:events:us-east-1:123456789012:rule/rule-ok",
            "state": "ENABLED",
            "event_bus_name": "default",
            "event_pattern": "{\"source\":[\"aws.ec2\"]}",
            "target_count": 1,
            "targets": [healthy_target()],
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
    fn cost_flags_disabled_rule() {
        let mut data = healthy_rule_data();
        data["state"] = json!("DISABLED");
        let r = fixture("rule-disabled", json!({"team": "sre"}), data, now());
        let report = evaluate_eventbridge_fleet(&[r], Pillar::Cost, now());
        assert_eq!(codes(&report), vec![REASON_COST_DISABLED_RULE]);
        assert!(matches!(report.findings[0].severity, Severity::Low));
    }

    #[test]
    fn cost_flags_enabled_rule_with_zero_targets() {
        let mut data = healthy_rule_data();
        data["target_count"] = json!(0);
        data["targets"] = json!([]);
        let r = fixture("rule-notargets", json!({"team": "sre"}), data, now());
        let report = evaluate_eventbridge_fleet(&[r], Pillar::Cost, now());
        assert_eq!(codes(&report), vec![REASON_COST_ENABLED_NO_TARGETS]);
        assert!(matches!(report.findings[0].severity, Severity::Medium));
    }

    #[test]
    fn cost_reports_data_gap_when_targets_not_collected() {
        let mut data = healthy_rule_data();
        data.as_object_mut().unwrap().remove("target_count");
        data.as_object_mut().unwrap().remove("targets");
        let r = fixture("rule-gap", json!({"team": "sre"}), data, now());
        let report = evaluate_eventbridge_fleet(&[r], Pillar::Cost, now());
        assert_eq!(codes(&report), vec![REASON_DATA_GAP_TARGETS]);
    }

    #[test]
    fn security_flags_custom_bus_rule_without_targets() {
        let mut data = healthy_rule_data();
        data["event_bus_name"] = json!("orders-bus");
        data["target_count"] = json!(0);
        data["targets"] = json!([]);
        let r = fixture("rule-custombus", json!({"team": "sre"}), data, now());
        let report = evaluate_eventbridge_fleet(&[r], Pillar::Security, now());
        assert_eq!(codes(&report), vec![REASON_SEC_CUSTOM_BUS_NO_TARGETS]);
    }

    #[test]
    fn security_does_not_apply_custom_bus_check_to_default_bus() {
        let mut data = healthy_rule_data();
        data["target_count"] = json!(0);
        data["targets"] = json!([]);
        let r = fixture("rule-defaultbus", json!({"team": "sre"}), data, now());
        let report = evaluate_eventbridge_fleet(&[r], Pillar::Security, now());
        assert!(
            report.findings.is_empty(),
            "unexpected: {:?}",
            report.findings
        );
    }

    #[test]
    fn security_flags_disabled_managed_rule() {
        let mut data = healthy_rule_data();
        data["state"] = json!("DISABLED");
        data["managed_by"] = json!("cloudtrail.amazonaws.com");
        let r = fixture("rule-managed", json!({"team": "sre"}), data, now());
        let report = evaluate_eventbridge_fleet(&[r], Pillar::Security, now());
        assert_eq!(codes(&report), vec![REASON_SEC_MANAGED_RULE_DISABLED]);
    }

    #[test]
    fn security_reports_data_gap_for_custom_bus_rule_without_target_data() {
        let mut data = healthy_rule_data();
        data["event_bus_name"] = json!("orders-bus");
        data.as_object_mut().unwrap().remove("target_count");
        data.as_object_mut().unwrap().remove("targets");
        let r = fixture("rule-custom-gap", json!({"team": "sre"}), data, now());
        let report = evaluate_eventbridge_fleet(&[r], Pillar::Security, now());
        assert_eq!(codes(&report), vec![REASON_DATA_GAP_TARGETS]);
    }

    #[test]
    fn resilience_flags_targets_without_dlq() {
        let mut data = healthy_rule_data();
        data["targets"][0]["has_dead_letter_config"] = json!(false);
        let r = fixture("rule-nodlq", json!({"team": "sre"}), data, now());
        let report = evaluate_eventbridge_fleet(&[r], Pillar::Resilience, now());
        assert_eq!(codes(&report), vec![REASON_RES_TARGET_NO_DLQ]);
        assert_eq!(
            report.findings[0].evidence["targets_without_dead_letter_config"],
            json!(["target-1"])
        );
    }

    #[test]
    fn resilience_flags_targets_without_retry_policy() {
        let mut data = healthy_rule_data();
        data["targets"][0]["has_retry_policy"] = json!(false);
        data["targets"][0]["retry_max_attempts"] = json!(null);
        let r = fixture("rule-noretry", json!({"team": "sre"}), data, now());
        let report = evaluate_eventbridge_fleet(&[r], Pillar::Resilience, now());
        assert_eq!(codes(&report), vec![REASON_RES_TARGET_NO_RETRY_POLICY]);
        assert!(matches!(report.findings[0].severity, Severity::Low));
    }

    #[test]
    fn resilience_flags_disabled_scheduled_rule_as_high() {
        let mut data = healthy_rule_data();
        data["state"] = json!("DISABLED");
        data["schedule_expression"] = json!("rate(5 minutes)");
        data.as_object_mut().unwrap().remove("event_pattern");
        let r = fixture("rule-cron-off", json!({"team": "sre"}), data, now());
        let report = evaluate_eventbridge_fleet(&[r], Pillar::Resilience, now());
        assert_eq!(codes(&report), vec![REASON_RES_SCHEDULED_RULE_DISABLED]);
        assert!(matches!(report.findings[0].severity, Severity::High));
    }

    #[test]
    fn resilience_reports_data_gap_when_targets_not_collected() {
        let mut data = healthy_rule_data();
        data.as_object_mut().unwrap().remove("target_count");
        data.as_object_mut().unwrap().remove("targets");
        let r = fixture("rule-gap", json!({"team": "sre"}), data, now());
        let report = evaluate_eventbridge_fleet(&[r], Pillar::Resilience, now());
        assert_eq!(codes(&report), vec![REASON_DATA_GAP_TARGETS]);
    }

    #[test]
    fn resilience_skips_target_checks_when_rule_has_zero_targets() {
        let mut data = healthy_rule_data();
        data["target_count"] = json!(0);
        data["targets"] = json!([]);
        let r = fixture("rule-zero", json!({"team": "sre"}), data, now());
        let report = evaluate_eventbridge_fleet(&[r], Pillar::Resilience, now());
        assert!(
            report.findings.is_empty(),
            "unexpected: {:?}",
            report.findings
        );
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
        let report = evaluate_eventbridge_fleet(&[r], Pillar::Resilience, now());
        assert_eq!(report.stale_resources, 1);
        assert!(codes(&report).contains(&REASON_INV_STALE_DATA));
    }

    #[test]
    fn non_eventbridge_resources_are_skipped_and_not_counted() {
        let mut r = fixture("queue-1", json!({}), json!({}), now());
        r.resource_type = "SqsQueue".to_string();
        let report = evaluate_eventbridge_fleet(&[r], Pillar::Cost, now());
        assert_eq!(report.resources_evaluated, 0);
        assert!(report.findings.is_empty());
    }

    #[test]
    fn empty_fleet_scores_perfect() {
        let report = evaluate_eventbridge_fleet(&[], Pillar::Cost, now());
        assert_eq!(report.resources_evaluated, 0);
        assert!(report.findings.is_empty());
        assert_eq!(report.score, 100);
    }

    #[test]
    fn performance_flags_pattern_without_source_or_detail_type() {
        let mut data = healthy_rule_data();
        data["event_pattern"] = json!("{\"region\":[\"us-east-1\"]}");
        let r = fixture("rule-broad", json!({"team": "sre"}), data, now());
        let report = evaluate_eventbridge_fleet(&[r], Pillar::Performance, now());
        assert_eq!(codes(&report), vec![REASON_PERF_BROAD_EVENT_PATTERN]);
        assert!(matches!(report.findings[0].severity, Severity::Low));
    }

    #[test]
    fn performance_accepts_detail_type_filter_and_skips_schedule_only_rules() {
        let mut data = healthy_rule_data();
        data["event_pattern"] =
            json!("{\"detail-type\":[\"EC2 Instance State-change Notification\"]}");
        let r = fixture("rule-dt", json!({"team": "sre"}), data, now());
        let report = evaluate_eventbridge_fleet(&[r], Pillar::Performance, now());
        assert!(
            report.findings.is_empty(),
            "unexpected: {:?}",
            report.findings
        );

        let mut scheduled = healthy_rule_data();
        scheduled.as_object_mut().unwrap().remove("event_pattern");
        scheduled["schedule_expression"] = json!("rate(5 minutes)");
        let r2 = fixture("rule-cron", json!({"team": "sre"}), scheduled, now());
        let report = evaluate_eventbridge_fleet(&[r2], Pillar::Performance, now());
        assert!(
            report.findings.is_empty(),
            "unexpected: {:?}",
            report.findings
        );
    }

    #[test]
    fn performance_flags_unparseable_pattern() {
        let mut data = healthy_rule_data();
        data["event_pattern"] = json!("not-json");
        let r = fixture("rule-badpattern", json!({"team": "sre"}), data, now());
        let report = evaluate_eventbridge_fleet(&[r], Pillar::Performance, now());
        assert_eq!(codes(&report), vec![REASON_PERF_PATTERN_UNPARSEABLE]);
    }

    #[test]
    fn scalability_flags_rule_at_target_quota() {
        let mut data = healthy_rule_data();
        data["target_count"] = json!(5);
        let r = fixture("rule-quota", json!({"team": "sre"}), data, now());
        let report = evaluate_eventbridge_fleet(&[r], Pillar::Scalability, now());
        assert_eq!(codes(&report), vec![REASON_SCALE_TARGET_QUOTA_REACHED]);
        assert!(matches!(report.findings[0].severity, Severity::Medium));
    }

    #[test]
    fn scalability_reports_data_gap_and_skips_disabled_rules() {
        let mut data = healthy_rule_data();
        data.as_object_mut().unwrap().remove("target_count");
        let r = fixture("rule-scalegap", json!({"team": "sre"}), data, now());
        let report = evaluate_eventbridge_fleet(&[r], Pillar::Scalability, now());
        assert_eq!(codes(&report), vec![REASON_DATA_GAP_TARGETS]);

        let mut disabled = healthy_rule_data();
        disabled["state"] = json!("DISABLED");
        disabled["target_count"] = json!(5);
        let r2 = fixture("rule-off", json!({"team": "sre"}), disabled, now());
        let report = evaluate_eventbridge_fleet(&[r2], Pillar::Scalability, now());
        assert!(
            report.findings.is_empty(),
            "unexpected: {:?}",
            report.findings
        );
    }

    #[test]
    fn disaster_recovery_flags_scheduled_rule_with_target_missing_dlq() {
        let mut data = healthy_rule_data();
        data.as_object_mut().unwrap().remove("event_pattern");
        data["schedule_expression"] = json!("rate(1 hour)");
        data["targets"][0]["has_dead_letter_config"] = json!(false);
        let r = fixture("rule-cron-nodlq", json!({"team": "sre"}), data, now());
        let report = evaluate_eventbridge_fleet(&[r], Pillar::DisasterRecovery, now());
        assert_eq!(codes(&report), vec![REASON_DR_SCHEDULED_NO_DLQ]);
        assert_eq!(
            report.findings[0].evidence["targets_without_dead_letter_config"],
            json!(["target-1"])
        );
    }

    #[test]
    fn disaster_recovery_ignores_pattern_rules_and_reports_schedule_data_gap() {
        let mut data = healthy_rule_data();
        data["targets"][0]["has_dead_letter_config"] = json!(false);
        let r = fixture("rule-pattern-nodlq", json!({"team": "sre"}), data, now());
        let report = evaluate_eventbridge_fleet(&[r], Pillar::DisasterRecovery, now());
        assert!(
            report.findings.is_empty(),
            "unexpected: {:?}",
            report.findings
        );

        let mut gap = healthy_rule_data();
        gap.as_object_mut().unwrap().remove("event_pattern");
        gap["schedule_expression"] = json!("rate(1 hour)");
        gap.as_object_mut().unwrap().remove("target_count");
        gap.as_object_mut().unwrap().remove("targets");
        let r2 = fixture("rule-cron-gap", json!({"team": "sre"}), gap, now());
        let report = evaluate_eventbridge_fleet(&[r2], Pillar::DisasterRecovery, now());
        assert_eq!(codes(&report), vec![REASON_DATA_GAP_TARGETS]);
    }

    #[test]
    fn operational_excellence_flags_missing_owner_tag() {
        let r = fixture(
            "rule-unowned",
            json!({"environment": "prod"}),
            healthy_rule_data(),
            now(),
        );
        let report = evaluate_eventbridge_fleet(&[r], Pillar::OperationalExcellence, now());
        assert_eq!(codes(&report), vec![REASON_OPEX_NO_OWNER_TAG]);
        assert!(matches!(report.findings[0].severity, Severity::Medium));
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
            let report = evaluate_eventbridge_fleet(std::slice::from_ref(&r), pillar, now());
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

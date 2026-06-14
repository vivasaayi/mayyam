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

// Deterministic WAF Web ACL inventory evaluators for the cost, security,
// and resilience pillars.
//
// Evaluates fields persisted by waf_control_plane: collected, rules_count,
// managed_rule_group_count, cloud_watch_metrics_enabled,
// sampled_requests_enabled, logging_enabled, scope, default_action, plus the
// tags column. Rule-based checks are gated on `rules_count` being present so
// a detail-collection failure (collected:false) surfaces as a data gap
// instead of false findings.

use chrono::{DateTime, Utc};
use serde_json::json;

use crate::models::aws_resource::Model as AwsResourceModel;
use crate::services::aws::inventory::types::{
    check_stale, data_str, score_pillar, InventoryFinding, Pillar, PillarReport, Severity,
};

/// Only rows of this resource type are evaluated.
pub const RESOURCE_TYPE: &str = "WafWebAcl";

// Reason codes are the stable contract for findings; never reuse or rename.
pub const REASON_COST_NO_RULES: &str = "WAF_COST_NO_RULES";
pub const REASON_COST_NO_TAGS: &str = "WAF_COST_NO_TAGS";
pub const REASON_RES_CLOUDWATCH_METRICS_DISABLED: &str = "WAF_RES_CLOUDWATCH_METRICS_DISABLED";
pub const REASON_RES_SAMPLED_REQUESTS_DISABLED: &str = "WAF_RES_SAMPLED_REQUESTS_DISABLED";
pub const REASON_RES_DETAIL_DATA_NOT_COLLECTED: &str = "WAF_RES_DETAIL_DATA_NOT_COLLECTED";
pub const REASON_SEC_LOGGING_DISABLED: &str = "WAF_SEC_LOGGING_DISABLED";
pub const REASON_SEC_LOGGING_DATA_NOT_COLLECTED: &str = "WAF_SEC_LOGGING_DATA_NOT_COLLECTED";
pub const REASON_SEC_NO_RULES: &str = "WAF_SEC_NO_RULES";
pub const REASON_SEC_NO_MANAGED_RULE_GROUPS: &str = "WAF_SEC_NO_MANAGED_RULE_GROUPS";
pub const REASON_INV_STALE_DATA: &str = "WAF_INV_STALE_DATA";

/// Evaluate every WAF Web ACL in the fleet for one pillar. Rows whose
/// `resource_type` is not `WafWebAcl` are skipped and not counted.
pub fn evaluate_waf_fleet(
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

fn data_u64(resource: &AwsResourceModel, key: &str) -> Option<u64> {
    resource.resource_data.get(key).and_then(|v| v.as_u64())
}

fn data_bool(resource: &AwsResourceModel, key: &str) -> Option<bool> {
    resource.resource_data.get(key).and_then(|v| v.as_bool())
}

fn evaluate_cost(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    // A web ACL bills a fixed base fee (~USD 5/month) even when it contains
    // no rules and therefore inspects nothing. Gated on rules_count being
    // collected; a detail gap is reported by the resilience pillar.
    if data_u64(resource, "rules_count") == Some(0) {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Cost,
            reason_code: REASON_COST_NO_RULES.to_string(),
            severity: Severity::Medium,
            message: format!(
                "Web ACL {} has zero rules; it bills the monthly base fee while providing no protection. Add rules or delete it",
                resource.resource_id
            ),
            evidence: json!({
                "rules_count": 0,
                "scope": data_str(&resource.resource_data, "scope"),
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
                "Web ACL {} has no tags recorded (untagged resource or tag collection gap); cost allocation cannot be assessed",
                resource.resource_id
            ),
            evidence: json!({ "tags": resource.tags }),
        });
    }
}

fn evaluate_security(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    // logging_enabled is persisted true/false when collected; the field is
    // absent only when the GetLoggingConfiguration call failed unexpectedly.
    match data_bool(resource, "logging_enabled") {
        Some(false) => {
            findings.push(InventoryFinding {
                resource_id: resource.resource_id.clone(),
                arn: resource.arn.clone(),
                pillar: Pillar::Security,
                reason_code: REASON_SEC_LOGGING_DISABLED.to_string(),
                severity: Severity::High,
                message: format!(
                    "Web ACL {} has no logging configuration; blocked and allowed requests leave no audit trail",
                    resource.resource_id
                ),
                evidence: json!({ "logging_enabled": false }),
            });
        }
        None => {
            findings.push(InventoryFinding {
                resource_id: resource.resource_id.clone(),
                arn: resource.arn.clone(),
                pillar: Pillar::Security,
                reason_code: REASON_SEC_LOGGING_DATA_NOT_COLLECTED.to_string(),
                severity: Severity::Low,
                message: format!(
                    "Logging status for Web ACL {} is not collected yet; security pillar cannot be fully assessed",
                    resource.resource_id
                ),
                evidence: json!({ "logging_enabled_collected": false }),
            });
        }
        Some(true) => {}
    }

    // Rule-based checks require collected detail; the detail gap itself is a
    // resilience finding, so absent counts are silently skipped here.
    match data_u64(resource, "rules_count") {
        Some(0) => {
            findings.push(InventoryFinding {
                resource_id: resource.resource_id.clone(),
                arn: resource.arn.clone(),
                pillar: Pillar::Security,
                reason_code: REASON_SEC_NO_RULES.to_string(),
                severity: Severity::High,
                message: format!(
                    "Web ACL {} has zero rules; every request falls through to the default action with no inspection",
                    resource.resource_id
                ),
                evidence: json!({
                    "rules_count": 0,
                    "default_action": data_str(&resource.resource_data, "default_action"),
                }),
            });
        }
        Some(rules_count) => {
            if data_u64(resource, "managed_rule_group_count") == Some(0) {
                findings.push(InventoryFinding {
                    resource_id: resource.resource_id.clone(),
                    arn: resource.arn.clone(),
                    pillar: Pillar::Security,
                    reason_code: REASON_SEC_NO_MANAGED_RULE_GROUPS.to_string(),
                    severity: Severity::Low,
                    message: format!(
                        "Web ACL {} uses no AWS managed rule groups; baseline protections (e.g. AWSManagedRulesCommonRuleSet) are missing",
                        resource.resource_id
                    ),
                    evidence: json!({
                        "rules_count": rules_count,
                        "managed_rule_group_count": 0,
                    }),
                });
            }
        }
        None => {}
    }
}

fn evaluate_resilience(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    if data_bool(resource, "collected") != Some(true) {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Resilience,
            reason_code: REASON_RES_DETAIL_DATA_NOT_COLLECTED.to_string(),
            severity: Severity::Low,
            message: format!(
                "Detail for Web ACL {} could not be collected; rule and visibility posture cannot be assessed",
                resource.resource_id
            ),
            evidence: json!({
                "collected": data_bool(resource, "collected"),
            }),
        });
        return;
    }

    if data_bool(resource, "cloud_watch_metrics_enabled") == Some(false) {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Resilience,
            reason_code: REASON_RES_CLOUDWATCH_METRICS_DISABLED.to_string(),
            severity: Severity::Medium,
            message: format!(
                "Web ACL {} has CloudWatch metrics disabled; rule matches and blocked traffic cannot be monitored or alarmed on",
                resource.resource_id
            ),
            evidence: json!({ "cloud_watch_metrics_enabled": false }),
        });
    }

    if data_bool(resource, "sampled_requests_enabled") == Some(false) {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Resilience,
            reason_code: REASON_RES_SAMPLED_REQUESTS_DISABLED.to_string(),
            severity: Severity::Low,
            message: format!(
                "Web ACL {} has sampled requests disabled; rule behavior cannot be inspected during an incident",
                resource.resource_id
            ),
            evidence: json!({ "sampled_requests_enabled": false }),
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
                "arn:aws:wafv2:us-east-1:123456789012:regional/webacl/{}/{}",
                resource_id, resource_id
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
            "name": "acl-ok",
            "web_acl_id": "acl-ok",
            "arn": "arn:aws:wafv2:us-east-1:123456789012:regional/webacl/acl-ok/acl-ok",
            "scope": "REGIONAL",
            "collected": true,
            "default_action": "block",
            "rules_count": 3,
            "managed_rule_group_count": 1,
            "capacity": 125,
            "label_namespace": "awswaf:123456789012:webacl:acl-ok:",
            "managed_by_firewall_manager": false,
            "cloud_watch_metrics_enabled": true,
            "sampled_requests_enabled": true,
            "metric_name": "acl-ok",
            "logging_enabled": true,
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
    fn healthy_web_acl_passes_all_pillars() {
        let r = fixture("acl-ok", json!({"team": "sre"}), healthy_data(), now());
        for pillar in [Pillar::Cost, Pillar::Security, Pillar::Resilience] {
            let report = evaluate_waf_fleet(std::slice::from_ref(&r), pillar, now());
            assert!(
                report.findings.is_empty(),
                "unexpected for {:?}: {:?}",
                pillar,
                report.findings
            );
            assert_eq!(report.score, 100);
            assert_eq!(report.resources_evaluated, 1);
        }
    }

    #[test]
    fn cost_flags_web_acl_with_zero_rules() {
        let mut data = healthy_data();
        data["rules_count"] = json!(0);
        data["managed_rule_group_count"] = json!(0);
        let r = fixture("acl-empty", json!({"team": "sre"}), data, now());
        let report = evaluate_waf_fleet(&[r], Pillar::Cost, now());
        assert_eq!(codes(&report), vec![REASON_COST_NO_RULES]);
    }

    #[test]
    fn cost_flags_untagged_web_acl() {
        let r = fixture("acl-untagged", json!({}), healthy_data(), now());
        let report = evaluate_waf_fleet(&[r], Pillar::Cost, now());
        assert_eq!(codes(&report), vec![REASON_COST_NO_TAGS]);
    }

    #[test]
    fn cost_skips_rule_check_when_detail_not_collected() {
        // Detail gap: rules_count absent. The gap is a resilience finding,
        // not a false cost finding.
        let data = json!({
            "name": "acl-gap",
            "scope": "REGIONAL",
            "collected": false,
            "logging_enabled": true,
        });
        let r = fixture("acl-gap", json!({"team": "sre"}), data, now());
        let report = evaluate_waf_fleet(&[r], Pillar::Cost, now());
        assert!(
            report.findings.is_empty(),
            "unexpected: {:?}",
            report.findings
        );
    }

    #[test]
    fn security_flags_logging_disabled_as_high() {
        let mut data = healthy_data();
        data["logging_enabled"] = json!(false);
        let r = fixture("acl-nolog", json!({"team": "sre"}), data, now());
        let report = evaluate_waf_fleet(&[r], Pillar::Security, now());
        assert_eq!(codes(&report), vec![REASON_SEC_LOGGING_DISABLED]);
        assert!(matches!(report.findings[0].severity, Severity::High));
    }

    #[test]
    fn security_reports_gap_when_logging_not_collected() {
        let mut data = healthy_data();
        data.as_object_mut().unwrap().remove("logging_enabled");
        let r = fixture("acl-loggap", json!({"team": "sre"}), data, now());
        let report = evaluate_waf_fleet(&[r], Pillar::Security, now());
        assert_eq!(codes(&report), vec![REASON_SEC_LOGGING_DATA_NOT_COLLECTED]);
        assert!(matches!(report.findings[0].severity, Severity::Low));
    }

    #[test]
    fn security_flags_zero_rules_as_high_without_managed_group_noise() {
        let mut data = healthy_data();
        data["rules_count"] = json!(0);
        data["managed_rule_group_count"] = json!(0);
        let r = fixture("acl-empty", json!({"team": "sre"}), data, now());
        let report = evaluate_waf_fleet(&[r], Pillar::Security, now());
        // Zero rules must not also raise the managed-rule-group finding.
        assert_eq!(codes(&report), vec![REASON_SEC_NO_RULES]);
        assert!(matches!(report.findings[0].severity, Severity::High));
    }

    #[test]
    fn security_flags_missing_managed_rule_groups_when_rules_exist() {
        let mut data = healthy_data();
        data["rules_count"] = json!(2);
        data["managed_rule_group_count"] = json!(0);
        let r = fixture("acl-custom", json!({"team": "sre"}), data, now());
        let report = evaluate_waf_fleet(&[r], Pillar::Security, now());
        assert_eq!(codes(&report), vec![REASON_SEC_NO_MANAGED_RULE_GROUPS]);
        assert!(matches!(report.findings[0].severity, Severity::Low));
    }

    #[test]
    fn security_skips_rule_checks_when_detail_not_collected() {
        let data = json!({
            "name": "acl-gap",
            "scope": "REGIONAL",
            "collected": false,
            "logging_enabled": true,
        });
        let r = fixture("acl-gap", json!({"team": "sre"}), data, now());
        let report = evaluate_waf_fleet(&[r], Pillar::Security, now());
        assert!(
            report.findings.is_empty(),
            "unexpected: {:?}",
            report.findings
        );
    }

    #[test]
    fn resilience_flags_cloudwatch_metrics_disabled() {
        let mut data = healthy_data();
        data["cloud_watch_metrics_enabled"] = json!(false);
        let r = fixture("acl-nometrics", json!({"team": "sre"}), data, now());
        let report = evaluate_waf_fleet(&[r], Pillar::Resilience, now());
        assert_eq!(codes(&report), vec![REASON_RES_CLOUDWATCH_METRICS_DISABLED]);
        assert!(matches!(report.findings[0].severity, Severity::Medium));
    }

    #[test]
    fn resilience_flags_sampled_requests_disabled() {
        let mut data = healthy_data();
        data["sampled_requests_enabled"] = json!(false);
        let r = fixture("acl-nosample", json!({"team": "sre"}), data, now());
        let report = evaluate_waf_fleet(&[r], Pillar::Resilience, now());
        assert_eq!(codes(&report), vec![REASON_RES_SAMPLED_REQUESTS_DISABLED]);
    }

    #[test]
    fn resilience_reports_detail_gap_and_skips_visibility_checks() {
        let data = json!({
            "name": "acl-gap",
            "scope": "REGIONAL",
            "collected": false,
            "logging_enabled": true,
        });
        let r = fixture("acl-gap", json!({"team": "sre"}), data, now());
        let report = evaluate_waf_fleet(&[r], Pillar::Resilience, now());
        assert_eq!(codes(&report), vec![REASON_RES_DETAIL_DATA_NOT_COLLECTED]);
    }

    #[test]
    fn stale_inventory_is_flagged() {
        let mut r = fixture("acl-stale", json!({"team": "sre"}), healthy_data(), now());
        r.last_refreshed = now() - Duration::hours(48);
        let report = evaluate_waf_fleet(&[r], Pillar::Resilience, now());
        assert_eq!(report.stale_resources, 1);
        assert!(codes(&report).contains(&REASON_INV_STALE_DATA));
    }

    #[test]
    fn non_waf_resources_are_skipped_and_not_counted() {
        let mut r = fixture("queue-1", json!({}), json!({}), now());
        r.resource_type = "SqsQueue".to_string();
        let report = evaluate_waf_fleet(&[r], Pillar::Cost, now());
        assert_eq!(report.resources_evaluated, 0);
        assert!(report.findings.is_empty());
    }
}

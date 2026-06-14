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

// Deterministic EMR cluster inventory evaluators for the cost, security, and
// resilience pillars.
//
// Evaluates fields persisted by emr_control_plane: state,
// state_change_reason_code/message, creation_date_time, auto_terminate,
// termination_protected, auto_scaling_role, log_uri, release_label,
// security_configuration, service_role, plus the tags column. The collector
// writes `collected: false` when describe_cluster fails for a cluster; in
// that case only the list summary fields are present, so detail checks are
// replaced by one data-gap finding per pillar.
//
// `visible_to_all_users` is persisted but deliberately not scored: the flag
// controls whether IAM principals beyond the creating principal can act on
// the cluster (subject to their own IAM policies). AWS recommends `true`
// with IAM-based control, so neither value is a defect by itself.

use chrono::{DateTime, Utc};
use serde_json::json;

use crate::models::aws_resource::Model as AwsResourceModel;
use crate::services::aws::inventory::types::{
    check_stale, data_str, score_pillar, InventoryFinding, Pillar, PillarReport, Severity,
};

/// Only rows of this resource type are evaluated.
pub const RESOURCE_TYPE: &str = "EmrCluster";

/// A WAITING cluster older than this with auto-terminate off is flagged idle.
pub const IDLE_AFTER_HOURS: i64 = 24;
/// EMR release major versions below this are treated as outdated.
pub const MIN_RELEASE_MAJOR: u32 = 6;

// Reason codes are the stable contract for findings; never reuse or rename.
pub const REASON_COST_NO_TAGS: &str = "EMR_COST_NO_TAGS";
pub const REASON_COST_IDLE_NO_AUTO_TERMINATE: &str = "EMR_COST_IDLE_NO_AUTO_TERMINATE";
pub const REASON_COST_IDLE_DATA_NOT_COLLECTED: &str = "EMR_COST_IDLE_DATA_NOT_COLLECTED";
pub const REASON_COST_NO_AUTO_SCALING_ROLE: &str = "EMR_COST_NO_AUTO_SCALING_ROLE";
pub const REASON_COST_DETAIL_DATA_NOT_COLLECTED: &str = "EMR_COST_DETAIL_DATA_NOT_COLLECTED";
pub const REASON_RES_TERMINATED_WITH_ERRORS: &str = "EMR_RES_TERMINATED_WITH_ERRORS";
pub const REASON_RES_TERMINATION_PROTECTION_DISABLED: &str =
    "EMR_RES_TERMINATION_PROTECTION_DISABLED";
pub const REASON_RES_NO_LOG_URI: &str = "EMR_RES_NO_LOG_URI";
pub const REASON_RES_OLD_RELEASE_LABEL: &str = "EMR_RES_OLD_RELEASE_LABEL";
pub const REASON_RES_DETAIL_DATA_NOT_COLLECTED: &str = "EMR_RES_DETAIL_DATA_NOT_COLLECTED";
pub const REASON_SEC_NO_SECURITY_CONFIGURATION: &str = "EMR_SEC_NO_SECURITY_CONFIGURATION";
pub const REASON_SEC_NO_SERVICE_ROLE: &str = "EMR_SEC_NO_SERVICE_ROLE";
pub const REASON_SEC_DETAIL_DATA_NOT_COLLECTED: &str = "EMR_SEC_DETAIL_DATA_NOT_COLLECTED";
pub const REASON_INV_STALE_DATA: &str = "EMR_INV_STALE_DATA";

/// Evaluate every EMR cluster in the fleet for one pillar. Rows whose
/// `resource_type` is not `EmrCluster` are skipped and not counted.
pub fn evaluate_emr_fleet(
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
            Pillar::Cost => evaluate_cost(resource, &mut findings, now),
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

/// The collector marks rows `collected: false` when describe_cluster failed
/// and only list-summary fields are present.
fn detail_collected(resource: &AwsResourceModel) -> bool {
    resource
        .resource_data
        .get("collected")
        .and_then(|v| v.as_bool())
        != Some(false)
}

fn data_bool(resource: &AwsResourceModel, key: &str) -> Option<bool> {
    resource.resource_data.get(key).and_then(|v| v.as_bool())
}

fn cluster_state(resource: &AwsResourceModel) -> Option<String> {
    data_str(&resource.resource_data, "state")
}

/// Hours since the persisted `creation_date_time`, when present and parseable.
fn creation_age_hours(resource: &AwsResourceModel, now: DateTime<Utc>) -> Option<i64> {
    let raw = data_str(&resource.resource_data, "creation_date_time")?;
    let created = DateTime::parse_from_rfc3339(&raw).ok()?.with_timezone(&Utc);
    Some((now - created).num_hours())
}

/// Major version from a release label like `emr-5.36.0` -> 5.
fn release_major(label: &str) -> Option<u32> {
    label
        .strip_prefix("emr-")?
        .split('.')
        .next()?
        .parse::<u32>()
        .ok()
}

fn data_gap_finding(
    resource: &AwsResourceModel,
    pillar: Pillar,
    reason_code: &str,
) -> InventoryFinding {
    InventoryFinding {
        resource_id: resource.resource_id.clone(),
        arn: resource.arn.clone(),
        pillar,
        reason_code: reason_code.to_string(),
        severity: Severity::Low,
        message: format!(
            "Cluster {} detail collection failed (describe_cluster); only list-summary fields are available, so the {} pillar cannot be fully assessed",
            resource.resource_id,
            pillar.as_str()
        ),
        evidence: json!({ "collected": false }),
    }
}

fn evaluate_cost(
    resource: &AwsResourceModel,
    findings: &mut Vec<InventoryFinding>,
    now: DateTime<Utc>,
) {
    if !detail_collected(resource) {
        findings.push(data_gap_finding(
            resource,
            Pillar::Cost,
            REASON_COST_DETAIL_DATA_NOT_COLLECTED,
        ));
        return;
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
                "Cluster {} has no tags recorded; cost allocation cannot be assessed",
                resource.resource_id
            ),
            evidence: json!({ "tags": resource.tags }),
        });
    }

    let state = cluster_state(resource);
    let auto_terminate = data_bool(resource, "auto_terminate");

    // A WAITING cluster has no running steps. With auto-terminate off it
    // keeps billing while idle; only flag once it has lived long enough to
    // rule out a cluster between back-to-back steps.
    if state.as_deref() == Some("WAITING") && auto_terminate == Some(false) {
        match creation_age_hours(resource, now) {
            Some(age_hours) if age_hours > IDLE_AFTER_HOURS => {
                findings.push(InventoryFinding {
                    resource_id: resource.resource_id.clone(),
                    arn: resource.arn.clone(),
                    pillar: Pillar::Cost,
                    reason_code: REASON_COST_IDLE_NO_AUTO_TERMINATE.to_string(),
                    severity: Severity::Medium,
                    message: format!(
                        "Cluster {} has been WAITING (no running steps) for {} hours with auto-terminate disabled; it bills while idle",
                        resource.resource_id, age_hours
                    ),
                    evidence: json!({
                        "state": "WAITING",
                        "auto_terminate": false,
                        "age_hours": age_hours,
                        "idle_after_hours": IDLE_AFTER_HOURS,
                        "normalized_instance_hours":
                            resource.resource_data.get("normalized_instance_hours"),
                    }),
                });
            }
            Some(_) => {}
            None => {
                findings.push(InventoryFinding {
                    resource_id: resource.resource_id.clone(),
                    arn: resource.arn.clone(),
                    pillar: Pillar::Cost,
                    reason_code: REASON_COST_IDLE_DATA_NOT_COLLECTED.to_string(),
                    severity: Severity::Low,
                    message: format!(
                        "Cluster {} is WAITING with auto-terminate disabled but creation_date_time is not collected; idle duration cannot be assessed",
                        resource.resource_id
                    ),
                    evidence: json!({
                        "state": "WAITING",
                        "auto_terminate": false,
                        "creation_date_time_collected": false,
                    }),
                });
            }
        }
    }

    // Long-lived clusters without an auto-scaling role have no instance-group
    // auto-scaling signal persisted, so capacity likely never scales down.
    if auto_terminate == Some(false)
        && data_str(&resource.resource_data, "auto_scaling_role").is_none()
    {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Cost,
            reason_code: REASON_COST_NO_AUTO_SCALING_ROLE.to_string(),
            severity: Severity::Low,
            message: format!(
                "Long-lived cluster {} has no auto-scaling role configured; instance-group auto-scaling is unavailable and capacity may be overprovisioned",
                resource.resource_id
            ),
            evidence: json!({ "auto_terminate": false, "auto_scaling_role": null }),
        });
    }
}

fn evaluate_resilience(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    if !detail_collected(resource) {
        findings.push(data_gap_finding(
            resource,
            Pillar::Resilience,
            REASON_RES_DETAIL_DATA_NOT_COLLECTED,
        ));
        return;
    }

    let state = cluster_state(resource);
    if state.as_deref() == Some("TERMINATED_WITH_ERRORS") {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Resilience,
            reason_code: REASON_RES_TERMINATED_WITH_ERRORS.to_string(),
            severity: Severity::High,
            message: format!(
                "Cluster {} terminated with errors; workloads on it failed",
                resource.resource_id
            ),
            evidence: json!({
                "state": "TERMINATED_WITH_ERRORS",
                "state_change_reason_code":
                    resource.resource_data.get("state_change_reason_code"),
                "state_change_reason_message":
                    resource.resource_data.get("state_change_reason_message"),
            }),
        });
    }

    // Only long-lived clusters (auto-terminate off) are held to the
    // termination-protection standard; transient clusters terminate by design.
    let auto_terminate = data_bool(resource, "auto_terminate");
    if auto_terminate == Some(false) && data_bool(resource, "termination_protected") == Some(false)
    {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Resilience,
            reason_code: REASON_RES_TERMINATION_PROTECTION_DISABLED.to_string(),
            severity: Severity::Medium,
            message: format!(
                "Long-lived cluster {} has termination protection disabled; an accidental terminate call destroys it and its HDFS data",
                resource.resource_id
            ),
            evidence: json!({ "auto_terminate": false, "termination_protected": false }),
        });
    }

    let log_uri = data_str(&resource.resource_data, "log_uri");
    if log_uri.as_deref().map(str::is_empty).unwrap_or(true) {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Resilience,
            reason_code: REASON_RES_NO_LOG_URI.to_string(),
            severity: Severity::Medium,
            message: format!(
                "Cluster {} has no log URI; step and node logs are lost when the cluster terminates, leaving no debugging evidence",
                resource.resource_id
            ),
            evidence: json!({ "log_uri": log_uri }),
        });
    }

    if let Some(label) = data_str(&resource.resource_data, "release_label") {
        if let Some(major) = release_major(&label) {
            if major < MIN_RELEASE_MAJOR {
                findings.push(InventoryFinding {
                    resource_id: resource.resource_id.clone(),
                    arn: resource.arn.clone(),
                    pillar: Pillar::Resilience,
                    reason_code: REASON_RES_OLD_RELEASE_LABEL.to_string(),
                    severity: Severity::Medium,
                    message: format!(
                        "Cluster {} runs outdated release {} (major version {} < {}); it misses current application, stability, and security fixes",
                        resource.resource_id, label, major, MIN_RELEASE_MAJOR
                    ),
                    evidence: json!({
                        "release_label": label,
                        "release_major": major,
                        "min_release_major": MIN_RELEASE_MAJOR,
                    }),
                });
            }
        }
    }
}

fn evaluate_security(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    if !detail_collected(resource) {
        findings.push(data_gap_finding(
            resource,
            Pillar::Security,
            REASON_SEC_DETAIL_DATA_NOT_COLLECTED,
        ));
        return;
    }

    // A security configuration is where EMR encryption at rest/in transit is
    // defined; its absence means no encryption configuration is attached.
    let security_configuration = data_str(&resource.resource_data, "security_configuration");
    if security_configuration
        .as_deref()
        .map(str::is_empty)
        .unwrap_or(true)
    {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Security,
            reason_code: REASON_SEC_NO_SECURITY_CONFIGURATION.to_string(),
            severity: Severity::Medium,
            message: format!(
                "Cluster {} has no security configuration attached; encryption at rest and in transit is not configured through EMR",
                resource.resource_id
            ),
            evidence: json!({ "security_configuration": security_configuration }),
        });
    }

    let service_role = data_str(&resource.resource_data, "service_role");
    if service_role.as_deref().map(str::is_empty).unwrap_or(true) {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Security,
            reason_code: REASON_SEC_NO_SERVICE_ROLE.to_string(),
            severity: Severity::Medium,
            message: format!(
                "Cluster {} has no service role recorded; the IAM boundary for EMR service actions cannot be assessed",
                resource.resource_id
            ),
            evidence: json!({ "service_role": service_role }),
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
                "arn:aws:elasticmapreduce:us-east-1:123456789012:cluster/{}",
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

    /// A long-lived cluster configured the way every check expects.
    fn healthy_data() -> Value {
        json!({
            "id": "j-OK",
            "name": "etl-prod",
            "cluster_arn": "arn:aws:elasticmapreduce:us-east-1:123456789012:cluster/j-OK",
            "collected": true,
            "state": "RUNNING",
            "creation_date_time": "2026-06-01T00:00:00Z",
            "normalized_instance_hours": 128,
            "release_label": "emr-7.1.0",
            "auto_terminate": false,
            "termination_protected": true,
            "visible_to_all_users": true,
            "applications": [{"name": "Spark", "version": "3.5.0"}],
            "security_configuration": "prod-emr-sec-config",
            "log_uri": "s3://logs/emr/",
            "service_role": "EMR_DefaultRole",
            "auto_scaling_role": "EMR_AutoScaling_DefaultRole",
            "scale_down_behavior": "TERMINATE_AT_TASK_COMPLETION",
            "ebs_root_volume_size": 32,
            "instance_collection_type": "INSTANCE_GROUP",
        })
    }

    fn degraded_data() -> Value {
        json!({
            "id": "j-GAP",
            "name": "etl-gap",
            "cluster_arn": "arn:aws:elasticmapreduce:us-east-1:123456789012:cluster/j-GAP",
            "state": "RUNNING",
            "normalized_instance_hours": 16,
            "collected": false,
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
    fn healthy_cluster_passes_all_pillars() {
        let r = fixture("j-OK", json!({"team": "data"}), healthy_data(), now());
        for pillar in [Pillar::Cost, Pillar::Security, Pillar::Resilience] {
            let report = evaluate_emr_fleet(std::slice::from_ref(&r), pillar, now());
            assert!(
                report.findings.is_empty(),
                "unexpected for {:?}: {:?}",
                pillar,
                report.findings
            );
            assert_eq!(report.score, 100);
        }
    }

    #[test]
    fn cost_flags_untagged_cluster() {
        let r = fixture("j-untagged", json!({}), healthy_data(), now());
        let report = evaluate_emr_fleet(&[r], Pillar::Cost, now());
        assert_eq!(codes(&report), vec![REASON_COST_NO_TAGS]);
    }

    #[test]
    fn cost_flags_old_waiting_cluster_without_auto_terminate() {
        let mut data = healthy_data();
        data["state"] = json!("WAITING");
        // creation_date_time in healthy_data is 9 days before now().
        let r = fixture("j-idle", json!({"team": "data"}), data, now());
        let report = evaluate_emr_fleet(&[r], Pillar::Cost, now());
        assert_eq!(codes(&report), vec![REASON_COST_IDLE_NO_AUTO_TERMINATE]);
        assert!(matches!(report.findings[0].severity, Severity::Medium));
    }

    #[test]
    fn cost_does_not_flag_young_waiting_cluster() {
        let mut data = healthy_data();
        data["state"] = json!("WAITING");
        data["creation_date_time"] = json!("2026-06-09T20:00:00Z"); // 4 hours old
        let r = fixture("j-young", json!({"team": "data"}), data, now());
        let report = evaluate_emr_fleet(&[r], Pillar::Cost, now());
        assert!(
            report.findings.is_empty(),
            "unexpected: {:?}",
            report.findings
        );
    }

    #[test]
    fn cost_does_not_flag_waiting_cluster_with_auto_terminate() {
        let mut data = healthy_data();
        data["state"] = json!("WAITING");
        data["auto_terminate"] = json!(true);
        let r = fixture("j-transient", json!({"team": "data"}), data, now());
        let report = evaluate_emr_fleet(&[r], Pillar::Cost, now());
        assert!(
            report.findings.is_empty(),
            "unexpected: {:?}",
            report.findings
        );
    }

    #[test]
    fn cost_reports_gap_when_waiting_cluster_has_no_creation_date() {
        let mut data = healthy_data();
        data["state"] = json!("WAITING");
        data.as_object_mut().unwrap().remove("creation_date_time");
        let r = fixture("j-nodate", json!({"team": "data"}), data, now());
        let report = evaluate_emr_fleet(&[r], Pillar::Cost, now());
        assert_eq!(codes(&report), vec![REASON_COST_IDLE_DATA_NOT_COLLECTED]);
    }

    #[test]
    fn cost_flags_long_lived_cluster_without_auto_scaling_role() {
        let mut data = healthy_data();
        data.as_object_mut().unwrap().remove("auto_scaling_role");
        let r = fixture("j-noscale", json!({"team": "data"}), data, now());
        let report = evaluate_emr_fleet(&[r], Pillar::Cost, now());
        assert_eq!(codes(&report), vec![REASON_COST_NO_AUTO_SCALING_ROLE]);
        assert!(matches!(report.findings[0].severity, Severity::Low));
    }

    #[test]
    fn cost_skips_auto_scaling_check_for_transient_cluster() {
        let mut data = healthy_data();
        data["auto_terminate"] = json!(true);
        data.as_object_mut().unwrap().remove("auto_scaling_role");
        let r = fixture("j-transient", json!({"team": "data"}), data, now());
        let report = evaluate_emr_fleet(&[r], Pillar::Cost, now());
        assert!(
            report.findings.is_empty(),
            "unexpected: {:?}",
            report.findings
        );
    }

    #[test]
    fn cost_reports_only_data_gap_when_describe_failed() {
        let r = fixture("j-GAP", json!({}), degraded_data(), now());
        let report = evaluate_emr_fleet(&[r], Pillar::Cost, now());
        assert_eq!(codes(&report), vec![REASON_COST_DETAIL_DATA_NOT_COLLECTED]);
    }

    #[test]
    fn resilience_flags_terminated_with_errors_as_high() {
        let mut data = healthy_data();
        data["state"] = json!("TERMINATED_WITH_ERRORS");
        data["state_change_reason_code"] = json!("BOOTSTRAP_FAILURE");
        let r = fixture("j-failed", json!({"team": "data"}), data, now());
        let report = evaluate_emr_fleet(&[r], Pillar::Resilience, now());
        assert_eq!(codes(&report), vec![REASON_RES_TERMINATED_WITH_ERRORS]);
        assert!(matches!(report.findings[0].severity, Severity::High));
    }

    #[test]
    fn resilience_flags_unprotected_long_lived_but_not_transient_cluster() {
        let mut unprotected = healthy_data();
        unprotected["termination_protected"] = json!(false);
        let r1 = fixture("j-unprot", json!({"team": "data"}), unprotected, now());
        let report = evaluate_emr_fleet(&[r1], Pillar::Resilience, now());
        assert_eq!(
            codes(&report),
            vec![REASON_RES_TERMINATION_PROTECTION_DISABLED]
        );

        let mut transient = healthy_data();
        transient["auto_terminate"] = json!(true);
        transient["termination_protected"] = json!(false);
        let r2 = fixture("j-transient", json!({"team": "data"}), transient, now());
        let report = evaluate_emr_fleet(&[r2], Pillar::Resilience, now());
        assert!(
            report.findings.is_empty(),
            "unexpected: {:?}",
            report.findings
        );
    }

    #[test]
    fn resilience_flags_missing_or_empty_log_uri() {
        let mut missing = healthy_data();
        missing.as_object_mut().unwrap().remove("log_uri");
        let r1 = fixture("j-nolog", json!({"team": "data"}), missing, now());
        let report = evaluate_emr_fleet(&[r1], Pillar::Resilience, now());
        assert_eq!(codes(&report), vec![REASON_RES_NO_LOG_URI]);

        let mut empty = healthy_data();
        empty["log_uri"] = json!("");
        let r2 = fixture("j-emptylog", json!({"team": "data"}), empty, now());
        let report = evaluate_emr_fleet(&[r2], Pillar::Resilience, now());
        assert_eq!(codes(&report), vec![REASON_RES_NO_LOG_URI]);
    }

    #[test]
    fn resilience_flags_old_release_label_but_not_current_one() {
        let mut old = healthy_data();
        old["release_label"] = json!("emr-5.36.2");
        let r1 = fixture("j-old", json!({"team": "data"}), old, now());
        let report = evaluate_emr_fleet(&[r1], Pillar::Resilience, now());
        assert_eq!(codes(&report), vec![REASON_RES_OLD_RELEASE_LABEL]);

        let mut current = healthy_data();
        current["release_label"] = json!("emr-6.15.0");
        let r2 = fixture("j-current", json!({"team": "data"}), current, now());
        let report = evaluate_emr_fleet(&[r2], Pillar::Resilience, now());
        assert!(
            report.findings.is_empty(),
            "unexpected: {:?}",
            report.findings
        );
    }

    #[test]
    fn resilience_skips_unparseable_release_label() {
        let mut data = healthy_data();
        data["release_label"] = json!("custom-build");
        let r = fixture("j-custom", json!({"team": "data"}), data, now());
        let report = evaluate_emr_fleet(&[r], Pillar::Resilience, now());
        assert!(
            report.findings.is_empty(),
            "unexpected: {:?}",
            report.findings
        );
    }

    #[test]
    fn resilience_reports_only_data_gap_when_describe_failed() {
        let r = fixture("j-GAP", json!({}), degraded_data(), now());
        let report = evaluate_emr_fleet(&[r], Pillar::Resilience, now());
        assert_eq!(codes(&report), vec![REASON_RES_DETAIL_DATA_NOT_COLLECTED]);
    }

    #[test]
    fn security_flags_missing_security_configuration() {
        let mut data = healthy_data();
        data.as_object_mut()
            .unwrap()
            .remove("security_configuration");
        let r = fixture("j-nosec", json!({"team": "data"}), data, now());
        let report = evaluate_emr_fleet(&[r], Pillar::Security, now());
        assert_eq!(codes(&report), vec![REASON_SEC_NO_SECURITY_CONFIGURATION]);
    }

    #[test]
    fn security_flags_missing_service_role() {
        let mut data = healthy_data();
        data.as_object_mut().unwrap().remove("service_role");
        let r = fixture("j-norole", json!({"team": "data"}), data, now());
        let report = evaluate_emr_fleet(&[r], Pillar::Security, now());
        assert_eq!(codes(&report), vec![REASON_SEC_NO_SERVICE_ROLE]);
    }

    #[test]
    fn security_does_not_score_visible_to_all_users() {
        let mut data = healthy_data();
        data["visible_to_all_users"] = json!(false);
        let r = fixture("j-vis", json!({"team": "data"}), data, now());
        let report = evaluate_emr_fleet(&[r], Pillar::Security, now());
        assert!(
            report.findings.is_empty(),
            "unexpected: {:?}",
            report.findings
        );
    }

    #[test]
    fn security_reports_only_data_gap_when_describe_failed() {
        let r = fixture("j-GAP", json!({}), degraded_data(), now());
        let report = evaluate_emr_fleet(&[r], Pillar::Security, now());
        assert_eq!(codes(&report), vec![REASON_SEC_DETAIL_DATA_NOT_COLLECTED]);
    }

    #[test]
    fn stale_inventory_is_flagged() {
        let mut r = fixture("j-stale", json!({"team": "data"}), healthy_data(), now());
        r.last_refreshed = now() - Duration::hours(48);
        let report = evaluate_emr_fleet(&[r], Pillar::Security, now());
        assert_eq!(report.stale_resources, 1);
        assert!(codes(&report).contains(&REASON_INV_STALE_DATA));
    }

    #[test]
    fn non_emr_resources_are_skipped_and_not_counted() {
        let mut r = fixture("vault-1", json!({}), json!({}), now());
        r.resource_type = "GlacierArchive".to_string();
        let report = evaluate_emr_fleet(&[r], Pillar::Cost, now());
        assert_eq!(report.resources_evaluated, 0);
        assert!(report.findings.is_empty());
    }

    #[test]
    fn release_major_parses_emr_labels_only() {
        assert_eq!(release_major("emr-5.36.0"), Some(5));
        assert_eq!(release_major("emr-7.1.0"), Some(7));
        assert_eq!(release_major("custom-build"), None);
        assert_eq!(release_major("emr-abc"), None);
    }
}

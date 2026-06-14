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

// Deterministic Athena workgroup inventory evaluators for the cost,
// security, and resilience pillars.
//
// Evaluates fields persisted by athena_control_plane: state,
// configuration_collected, bytes_scanned_cutoff_per_query,
// requester_pays_enabled, publish_cloud_watch_metrics_enabled,
// engine_version_selected, enforce_work_group_configuration,
// output_location, and result_encryption_option/_kms_key.
//
// `configuration_collected` is true only when GetWorkGroup returned a
// WorkGroupConfiguration; when it is false every configuration-derived
// check is replaced by a per-pillar data-gap finding. The workgroup
// `state` comes from ListWorkGroups and is evaluated even without the
// configuration detail.

use chrono::{DateTime, Utc};
use serde_json::json;

use crate::models::aws_resource::Model as AwsResourceModel;
use crate::services::aws::inventory::types::{
    check_stale, data_str, score_pillar, InventoryFinding, Pillar, PillarReport, Severity,
};

/// Only rows of this resource type are evaluated.
pub const RESOURCE_TYPE: &str = "AthenaWorkgroup";

// Reason codes are the stable contract for findings; never reuse or rename.
pub const REASON_COST_NO_SCAN_LIMIT: &str = "ATHENA_COST_NO_SCAN_LIMIT";
pub const REASON_COST_DISABLED_WORKGROUP: &str = "ATHENA_COST_DISABLED_WORKGROUP";
pub const REASON_COST_REQUESTER_PAYS_ENABLED: &str = "ATHENA_COST_REQUESTER_PAYS_ENABLED";
pub const REASON_COST_CONFIG_NOT_COLLECTED: &str = "ATHENA_COST_CONFIG_NOT_COLLECTED";
pub const REASON_RES_CLOUDWATCH_METRICS_DISABLED: &str = "ATHENA_RES_CLOUDWATCH_METRICS_DISABLED";
pub const REASON_RES_ENGINE_VERSION_PINNED: &str = "ATHENA_RES_ENGINE_VERSION_PINNED";
pub const REASON_RES_WORKGROUP_DISABLED: &str = "ATHENA_RES_WORKGROUP_DISABLED";
pub const REASON_RES_CONFIG_NOT_COLLECTED: &str = "ATHENA_RES_CONFIG_NOT_COLLECTED";
pub const REASON_SEC_RESULT_ENCRYPTION_NOT_CONFIGURED: &str =
    "ATHENA_SEC_RESULT_ENCRYPTION_NOT_CONFIGURED";
pub const REASON_SEC_RESULT_ENCRYPTION_NOT_KMS: &str = "ATHENA_SEC_RESULT_ENCRYPTION_NOT_KMS";
pub const REASON_SEC_CONFIG_NOT_ENFORCED: &str = "ATHENA_SEC_CONFIG_NOT_ENFORCED";
pub const REASON_SEC_NO_OUTPUT_LOCATION: &str = "ATHENA_SEC_NO_OUTPUT_LOCATION";
pub const REASON_SEC_CONFIG_NOT_COLLECTED: &str = "ATHENA_SEC_CONFIG_NOT_COLLECTED";
pub const REASON_INV_STALE_DATA: &str = "ATHENA_INV_STALE_DATA";

/// Evaluate every Athena workgroup in the fleet for one pillar. Rows whose
/// `resource_type` is not `AthenaWorkgroup` are skipped and not counted.
pub fn evaluate_athena_fleet(
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

fn config_collected(resource: &AwsResourceModel) -> bool {
    resource
        .resource_data
        .get("configuration_collected")
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
}

fn workgroup_state(resource: &AwsResourceModel) -> Option<String> {
    data_str(&resource.resource_data, "state")
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
            "Workgroup configuration for {} is not collected yet (GetWorkGroup detail missing); {} pillar cannot be fully assessed",
            resource.resource_id,
            pillar.as_str()
        ),
        evidence: json!({ "configuration_collected": false }),
    }
}

fn evaluate_cost(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    // A DISABLED workgroup left behind keeps result data accumulating in S3
    // and signals an abandoned analytics setup; state comes from the list
    // call so it is checked even without configuration detail.
    let state = workgroup_state(resource);
    if state.as_deref() == Some("DISABLED") {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Cost,
            reason_code: REASON_COST_DISABLED_WORKGROUP.to_string(),
            severity: Severity::Low,
            message: format!(
                "Workgroup {} is DISABLED but still present; delete it (and clean up its result location) if it is no longer needed",
                resource.resource_id
            ),
            evidence: json!({ "state": "DISABLED" }),
        });
    }

    if !config_collected(resource) {
        findings.push(data_gap_finding(
            resource,
            Pillar::Cost,
            REASON_COST_CONFIG_NOT_COLLECTED,
        ));
        return;
    }

    // BytesScannedCutoffPerQuery is absent from the API when no per-query
    // scan cap is set, so absence here means there is genuinely no cap.
    let scan_cutoff = resource
        .resource_data
        .get("bytes_scanned_cutoff_per_query")
        .and_then(|v| v.as_i64());
    if scan_cutoff.is_none() {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Cost,
            reason_code: REASON_COST_NO_SCAN_LIMIT.to_string(),
            severity: Severity::Medium,
            message: format!(
                "Workgroup {} has no per-query data scan limit; a single runaway query can scan unbounded data and Athena bills per TB scanned",
                resource.resource_id
            ),
            evidence: json!({ "bytes_scanned_cutoff_per_query": null }),
        });
    }

    let requester_pays = resource
        .resource_data
        .get("requester_pays_enabled")
        .and_then(|v| v.as_bool());
    if requester_pays == Some(true) {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Cost,
            reason_code: REASON_COST_REQUESTER_PAYS_ENABLED.to_string(),
            severity: Severity::Low,
            message: format!(
                "Workgroup {} allows queries against Requester Pays buckets; data transfer and request charges land on this account",
                resource.resource_id
            ),
            evidence: json!({ "requester_pays_enabled": true }),
        });
    }
}

fn evaluate_resilience(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    // Queries submitted to a DISABLED workgroup fail outright.
    let state = workgroup_state(resource);
    if state.as_deref() == Some("DISABLED") {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Resilience,
            reason_code: REASON_RES_WORKGROUP_DISABLED.to_string(),
            severity: Severity::Medium,
            message: format!(
                "Workgroup {} is DISABLED; any query submitted to it will fail until it is re-enabled",
                resource.resource_id
            ),
            evidence: json!({ "state": "DISABLED" }),
        });
    }

    if !config_collected(resource) {
        findings.push(data_gap_finding(
            resource,
            Pillar::Resilience,
            REASON_RES_CONFIG_NOT_COLLECTED,
        ));
        return;
    }

    // PublishCloudWatchMetricsEnabled defaults to false; absence with a
    // collected configuration means metrics are not published.
    let metrics_enabled = resource
        .resource_data
        .get("publish_cloud_watch_metrics_enabled")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    if !metrics_enabled {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Resilience,
            reason_code: REASON_RES_CLOUDWATCH_METRICS_DISABLED.to_string(),
            severity: Severity::Medium,
            message: format!(
                "Workgroup {} does not publish CloudWatch metrics; query failures, queue times, and scan volumes are invisible to alerting",
                resource.resource_id
            ),
            evidence: json!({
                "publish_cloud_watch_metrics_enabled":
                    resource.resource_data.get("publish_cloud_watch_metrics_enabled")
            }),
        });
    }

    // A pinned (non-AUTO) engine version stops receiving automatic engine
    // upgrades and will eventually be deprecated under the workgroup.
    let selected = data_str(&resource.resource_data, "engine_version_selected");
    if let Some(selected) = selected {
        if selected != "AUTO" {
            let effective = data_str(&resource.resource_data, "engine_version_effective");
            findings.push(InventoryFinding {
                resource_id: resource.resource_id.clone(),
                arn: resource.arn.clone(),
                pillar: Pillar::Resilience,
                reason_code: REASON_RES_ENGINE_VERSION_PINNED.to_string(),
                severity: Severity::Low,
                message: format!(
                    "Workgroup {} pins engine version '{}' instead of AUTO; it will not receive automatic engine upgrades and risks forced migration at deprecation",
                    resource.resource_id, selected
                ),
                evidence: json!({
                    "engine_version_selected": selected,
                    "engine_version_effective": effective,
                }),
            });
        }
    }
}

fn evaluate_security(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    if !config_collected(resource) {
        findings.push(data_gap_finding(
            resource,
            Pillar::Security,
            REASON_SEC_CONFIG_NOT_COLLECTED,
        ));
        return;
    }

    // result_encryption_option is persisted only when the workgroup result
    // configuration carries an EncryptionConfiguration; absence means query
    // results are written unencrypted by the workgroup settings.
    match data_str(&resource.resource_data, "result_encryption_option").as_deref() {
        None => {
            findings.push(InventoryFinding {
                resource_id: resource.resource_id.clone(),
                arn: resource.arn.clone(),
                pillar: Pillar::Security,
                reason_code: REASON_SEC_RESULT_ENCRYPTION_NOT_CONFIGURED.to_string(),
                severity: Severity::Medium,
                message: format!(
                    "Workgroup {} has no result encryption configuration; query results are written to S3 without workgroup-managed encryption",
                    resource.resource_id
                ),
                evidence: json!({ "result_encryption_option": null }),
            });
        }
        Some("SSE_S3") => {
            findings.push(InventoryFinding {
                resource_id: resource.resource_id.clone(),
                arn: resource.arn.clone(),
                pillar: Pillar::Security,
                reason_code: REASON_SEC_RESULT_ENCRYPTION_NOT_KMS.to_string(),
                severity: Severity::Low,
                message: format!(
                    "Workgroup {} encrypts results with SSE_S3; SSE_KMS or CSE_KMS provides key-level access control and audit for query results",
                    resource.resource_id
                ),
                evidence: json!({ "result_encryption_option": "SSE_S3" }),
            });
        }
        // SSE_KMS and CSE_KMS are KMS-backed and pass.
        Some(_) => {}
    }

    // Without enforcement, clients can override the workgroup's output
    // location and encryption with their own (possibly unencrypted) settings.
    let enforced = resource
        .resource_data
        .get("enforce_work_group_configuration")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    if !enforced {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Security,
            reason_code: REASON_SEC_CONFIG_NOT_ENFORCED.to_string(),
            severity: Severity::Medium,
            message: format!(
                "Workgroup {} does not enforce its configuration; clients can override the result location and encryption settings per query",
                resource.resource_id
            ),
            evidence: json!({
                "enforce_work_group_configuration":
                    resource.resource_data.get("enforce_work_group_configuration")
            }),
        });
    }

    if data_str(&resource.resource_data, "output_location").is_none() {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Security,
            reason_code: REASON_SEC_NO_OUTPUT_LOCATION.to_string(),
            severity: Severity::Low,
            message: format!(
                "Workgroup {} has no managed output location; result placement is left to each client and cannot be governed centrally",
                resource.resource_id
            ),
            evidence: json!({ "output_location": null }),
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
                "arn:aws:athena:us-east-1:123456789012:workgroup/{}",
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
            "name": "primary",
            "configuration_collected": true,
            "state": "ENABLED",
            "description": "primary analytics workgroup",
            "creation_time": "2025-01-01T00:00:00Z",
            "enforce_work_group_configuration": true,
            "publish_cloud_watch_metrics_enabled": true,
            "bytes_scanned_cutoff_per_query": 10737418240i64,
            "requester_pays_enabled": false,
            "engine_version_selected": "AUTO",
            "engine_version_effective": "Athena engine version 3",
            "output_location": "s3://athena-results-123456789012/primary/",
            "result_encryption_option": "SSE_KMS",
            "result_encryption_kms_key": "arn:aws:kms:us-east-1:123456789012:key/abc",
        })
    }

    fn summary_only_data() -> Value {
        json!({
            "name": "partial",
            "configuration_collected": false,
            "state": "ENABLED",
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
    fn healthy_workgroup_passes_all_pillars() {
        let r = fixture("wg-ok", json!({"team": "data"}), healthy_data(), now());
        for pillar in [Pillar::Cost, Pillar::Security, Pillar::Resilience] {
            let report = evaluate_athena_fleet(std::slice::from_ref(&r), pillar, now());
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
    fn cost_flags_missing_scan_limit() {
        let mut data = healthy_data();
        data.as_object_mut()
            .unwrap()
            .remove("bytes_scanned_cutoff_per_query");
        let r = fixture("wg-nolimit", json!({}), data, now());
        let report = evaluate_athena_fleet(&[r], Pillar::Cost, now());
        assert_eq!(codes(&report), vec![REASON_COST_NO_SCAN_LIMIT]);
        assert!(matches!(report.findings[0].severity, Severity::Medium));
    }

    #[test]
    fn cost_flags_disabled_workgroup_lingering() {
        let mut data = healthy_data();
        data["state"] = json!("DISABLED");
        let r = fixture("wg-disabled", json!({}), data, now());
        let report = evaluate_athena_fleet(&[r], Pillar::Cost, now());
        assert_eq!(codes(&report), vec![REASON_COST_DISABLED_WORKGROUP]);
    }

    #[test]
    fn cost_flags_requester_pays_enabled() {
        let mut data = healthy_data();
        data["requester_pays_enabled"] = json!(true);
        let r = fixture("wg-reqpays", json!({}), data, now());
        let report = evaluate_athena_fleet(&[r], Pillar::Cost, now());
        assert_eq!(codes(&report), vec![REASON_COST_REQUESTER_PAYS_ENABLED]);
    }

    #[test]
    fn cost_reports_data_gap_when_config_not_collected() {
        let r = fixture("wg-gap", json!({}), summary_only_data(), now());
        let report = evaluate_athena_fleet(&[r], Pillar::Cost, now());
        // Only the gap code: no scan-limit finding may be inferred from
        // a configuration that was never collected.
        assert_eq!(codes(&report), vec![REASON_COST_CONFIG_NOT_COLLECTED]);
        assert!(matches!(report.findings[0].severity, Severity::Low));
    }

    #[test]
    fn resilience_flags_metrics_disabled() {
        let mut data = healthy_data();
        data["publish_cloud_watch_metrics_enabled"] = json!(false);
        let r = fixture("wg-nometrics", json!({}), data, now());
        let report = evaluate_athena_fleet(&[r], Pillar::Resilience, now());
        assert_eq!(codes(&report), vec![REASON_RES_CLOUDWATCH_METRICS_DISABLED]);
    }

    #[test]
    fn resilience_treats_absent_metrics_flag_as_disabled_when_config_collected() {
        let mut data = healthy_data();
        data.as_object_mut()
            .unwrap()
            .remove("publish_cloud_watch_metrics_enabled");
        let r = fixture("wg-metricsgap", json!({}), data, now());
        let report = evaluate_athena_fleet(&[r], Pillar::Resilience, now());
        assert_eq!(codes(&report), vec![REASON_RES_CLOUDWATCH_METRICS_DISABLED]);
    }

    #[test]
    fn resilience_flags_pinned_engine_version() {
        let mut data = healthy_data();
        data["engine_version_selected"] = json!("Athena engine version 2");
        let r = fixture("wg-pinned", json!({}), data, now());
        let report = evaluate_athena_fleet(&[r], Pillar::Resilience, now());
        assert_eq!(codes(&report), vec![REASON_RES_ENGINE_VERSION_PINNED]);
    }

    #[test]
    fn resilience_flags_disabled_workgroup() {
        let mut data = healthy_data();
        data["state"] = json!("DISABLED");
        let r = fixture("wg-disabled", json!({}), data, now());
        let report = evaluate_athena_fleet(&[r], Pillar::Resilience, now());
        assert_eq!(codes(&report), vec![REASON_RES_WORKGROUP_DISABLED]);
        assert!(matches!(report.findings[0].severity, Severity::Medium));
    }

    #[test]
    fn resilience_reports_data_gap_when_config_not_collected() {
        let r = fixture("wg-gap", json!({}), summary_only_data(), now());
        let report = evaluate_athena_fleet(&[r], Pillar::Resilience, now());
        assert_eq!(codes(&report), vec![REASON_RES_CONFIG_NOT_COLLECTED]);
    }

    #[test]
    fn security_flags_unencrypted_results() {
        let mut data = healthy_data();
        let obj = data.as_object_mut().unwrap();
        obj.remove("result_encryption_option");
        obj.remove("result_encryption_kms_key");
        let r = fixture("wg-noenc", json!({}), data, now());
        let report = evaluate_athena_fleet(&[r], Pillar::Security, now());
        assert_eq!(
            codes(&report),
            vec![REASON_SEC_RESULT_ENCRYPTION_NOT_CONFIGURED]
        );
        assert!(matches!(report.findings[0].severity, Severity::Medium));
    }

    #[test]
    fn security_flags_sse_s3_as_not_kms_backed() {
        let mut data = healthy_data();
        data["result_encryption_option"] = json!("SSE_S3");
        data.as_object_mut()
            .unwrap()
            .remove("result_encryption_kms_key");
        let r = fixture("wg-sses3", json!({}), data, now());
        let report = evaluate_athena_fleet(&[r], Pillar::Security, now());
        assert_eq!(codes(&report), vec![REASON_SEC_RESULT_ENCRYPTION_NOT_KMS]);
        assert!(matches!(report.findings[0].severity, Severity::Low));
    }

    #[test]
    fn security_accepts_cse_kms_encryption() {
        let mut data = healthy_data();
        data["result_encryption_option"] = json!("CSE_KMS");
        let r = fixture("wg-csekms", json!({}), data, now());
        let report = evaluate_athena_fleet(&[r], Pillar::Security, now());
        assert!(
            report.findings.is_empty(),
            "unexpected: {:?}",
            report.findings
        );
    }

    #[test]
    fn security_flags_enforcement_disabled() {
        let mut data = healthy_data();
        data["enforce_work_group_configuration"] = json!(false);
        let r = fixture("wg-noenforce", json!({}), data, now());
        let report = evaluate_athena_fleet(&[r], Pillar::Security, now());
        assert_eq!(codes(&report), vec![REASON_SEC_CONFIG_NOT_ENFORCED]);
    }

    #[test]
    fn security_flags_missing_output_location() {
        let mut data = healthy_data();
        data.as_object_mut().unwrap().remove("output_location");
        let r = fixture("wg-nooutput", json!({}), data, now());
        let report = evaluate_athena_fleet(&[r], Pillar::Security, now());
        assert_eq!(codes(&report), vec![REASON_SEC_NO_OUTPUT_LOCATION]);
    }

    #[test]
    fn security_reports_data_gap_when_config_not_collected() {
        let r = fixture("wg-gap", json!({}), summary_only_data(), now());
        let report = evaluate_athena_fleet(&[r], Pillar::Security, now());
        assert_eq!(codes(&report), vec![REASON_SEC_CONFIG_NOT_COLLECTED]);
    }

    #[test]
    fn stale_inventory_is_flagged() {
        let mut r = fixture("wg-stale", json!({}), healthy_data(), now());
        r.last_refreshed = now() - Duration::hours(48);
        let report = evaluate_athena_fleet(&[r], Pillar::Resilience, now());
        assert_eq!(report.stale_resources, 1);
        assert!(codes(&report).contains(&REASON_INV_STALE_DATA));
    }

    #[test]
    fn non_athena_resources_are_skipped_and_not_counted() {
        let mut r = fixture("queue-1", json!({}), json!({}), now());
        r.resource_type = "SqsQueue".to_string();
        let report = evaluate_athena_fleet(&[r], Pillar::Cost, now());
        assert_eq!(report.resources_evaluated, 0);
        assert!(report.findings.is_empty());
    }
}

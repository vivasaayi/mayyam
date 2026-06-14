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

// Deterministic SSM document inventory evaluators for the cost, security,
// and resilience pillars.
//
// Evaluates fields persisted by ssm_control_plane: owner, document_type,
// document_format, schema_version, created_date, review_status,
// shared_account_ids, shared_publicly, plus the tags column. The collector
// only ingests account-owned (Owner=Self) documents, so the owner check is a
// consistency guard against third-party documents leaking into inventory.

use chrono::{DateTime, Utc};
use serde_json::json;

use crate::models::aws_resource::Model as AwsResourceModel;
use crate::services::aws::inventory::types::{
    check_stale, data_str, score_pillar, InventoryFinding, Pillar, PillarReport, Severity,
};

/// Only rows of this resource type are evaluated.
pub const RESOURCE_TYPE: &str = "SsmDocument";

/// A NOT_REVIEWED document older than this is treated as accumulating cruft.
pub const AGING_NEVER_REVIEWED_DAYS: i64 = 365;

// Reason codes are the stable contract for findings; never reuse or rename.
pub const REASON_COST_NO_TAGS: &str = "SSM_COST_NO_TAGS";
pub const REASON_COST_AGING_NEVER_REVIEWED: &str = "SSM_COST_AGING_NEVER_REVIEWED";
pub const REASON_COST_CREATED_DATE_DATA_NOT_COLLECTED: &str =
    "SSM_COST_CREATED_DATE_DATA_NOT_COLLECTED";
pub const REASON_RES_LEGACY_SCHEMA_VERSION: &str = "SSM_RES_LEGACY_SCHEMA_VERSION";
pub const REASON_RES_SCHEMA_DATA_NOT_COLLECTED: &str = "SSM_RES_SCHEMA_DATA_NOT_COLLECTED";
pub const REASON_RES_TEXT_FORMAT_DOCUMENT: &str = "SSM_RES_TEXT_FORMAT_DOCUMENT";
pub const REASON_SEC_DOCUMENT_SHARED_PUBLICLY: &str = "SSM_SEC_DOCUMENT_SHARED_PUBLICLY";
pub const REASON_SEC_SHARING_DATA_NOT_COLLECTED: &str = "SSM_SEC_SHARING_DATA_NOT_COLLECTED";
pub const REASON_SEC_REVIEW_NOT_APPROVED: &str = "SSM_SEC_REVIEW_NOT_APPROVED";
pub const REASON_SEC_THIRD_PARTY_OWNER: &str = "SSM_SEC_THIRD_PARTY_OWNER";
pub const REASON_SEC_OWNER_DATA_NOT_COLLECTED: &str = "SSM_SEC_OWNER_DATA_NOT_COLLECTED";
pub const REASON_INV_STALE_DATA: &str = "SSM_INV_STALE_DATA";

/// Evaluate every SSM document in the fleet for one pillar. Rows whose
/// `resource_type` is not `SsmDocument` are skipped and not counted.
pub fn evaluate_ssm_fleet(
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
            Pillar::Cost => evaluate_cost(resource, now, &mut findings),
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

fn created_date(resource: &AwsResourceModel) -> Option<DateTime<Utc>> {
    data_str(&resource.resource_data, "created_date")
        .and_then(|raw| DateTime::parse_from_rfc3339(&raw).ok())
        .map(|dt| dt.with_timezone(&Utc))
}

fn evaluate_cost(
    resource: &AwsResourceModel,
    now: DateTime<Utc>,
    findings: &mut Vec<InventoryFinding>,
) {
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
            severity: Severity::Low,
            message: format!(
                "SSM document {} has no tags recorded (untagged resource or tag collection gap); ownership and cleanup decisions cannot be routed",
                resource.resource_id
            ),
            evidence: json!({ "tags": resource.tags }),
        });
    }

    // Never-reviewed documents accumulating for years are cleanup candidates.
    // review_status is only reported by AWS for documents that support the
    // review workflow, so its absence is not a data gap.
    let review_status = data_str(&resource.resource_data, "review_status");
    if review_status.as_deref() != Some("NOT_REVIEWED") {
        return;
    }

    match created_date(resource) {
        Some(created) => {
            let age_days = (now - created).num_days();
            if age_days > AGING_NEVER_REVIEWED_DAYS {
                findings.push(InventoryFinding {
                    resource_id: resource.resource_id.clone(),
                    arn: resource.arn.clone(),
                    pillar: Pillar::Cost,
                    reason_code: REASON_COST_AGING_NEVER_REVIEWED.to_string(),
                    severity: Severity::Low,
                    message: format!(
                        "SSM document {} was created {} days ago and has never been reviewed; review or delete it to stop automation cruft from accumulating",
                        resource.resource_id, age_days
                    ),
                    evidence: json!({
                        "review_status": "NOT_REVIEWED",
                        "age_days": age_days,
                        "aging_threshold_days": AGING_NEVER_REVIEWED_DAYS,
                    }),
                });
            }
        }
        None => {
            findings.push(InventoryFinding {
                resource_id: resource.resource_id.clone(),
                arn: resource.arn.clone(),
                pillar: Pillar::Cost,
                reason_code: REASON_COST_CREATED_DATE_DATA_NOT_COLLECTED.to_string(),
                severity: Severity::Low,
                message: format!(
                    "Creation date for never-reviewed SSM document {} is not collected; aging cannot be assessed",
                    resource.resource_id
                ),
                evidence: json!({ "created_date_collected": false }),
            });
        }
    }
}

fn evaluate_resilience(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    // Schema currency only applies to Command documents: schema 1.x Command
    // documents are the legacy generation and miss newer execution semantics.
    let document_type = data_str(&resource.resource_data, "document_type");
    if document_type.as_deref() == Some("Command") {
        let schema_version = data_str(&resource.resource_data, "schema_version");
        let major = schema_version
            .as_deref()
            .and_then(|s| s.split('.').next())
            .and_then(|m| m.parse::<u32>().ok());
        match major {
            Some(m) if m < 2 => {
                findings.push(InventoryFinding {
                    resource_id: resource.resource_id.clone(),
                    arn: resource.arn.clone(),
                    pillar: Pillar::Resilience,
                    reason_code: REASON_RES_LEGACY_SCHEMA_VERSION.to_string(),
                    severity: Severity::Medium,
                    message: format!(
                        "Command document {} uses legacy schema version {}; migrate to schema 2.x for current execution semantics",
                        resource.resource_id,
                        schema_version.as_deref().unwrap_or("unknown")
                    ),
                    evidence: json!({ "schema_version": schema_version, "document_type": "Command" }),
                });
            }
            Some(_) => {}
            None => {
                findings.push(InventoryFinding {
                    resource_id: resource.resource_id.clone(),
                    arn: resource.arn.clone(),
                    pillar: Pillar::Resilience,
                    reason_code: REASON_RES_SCHEMA_DATA_NOT_COLLECTED.to_string(),
                    severity: Severity::Low,
                    message: format!(
                        "Schema version for Command document {} is not collected or unparseable; schema currency cannot be assessed",
                        resource.resource_id
                    ),
                    evidence: json!({ "schema_version": schema_version }),
                });
            }
        }
    }

    // TEXT-format documents predate structured JSON/YAML validation and are
    // harder to lint, diff, and roll back safely.
    if data_str(&resource.resource_data, "document_format").as_deref() == Some("TEXT") {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Resilience,
            reason_code: REASON_RES_TEXT_FORMAT_DOCUMENT.to_string(),
            severity: Severity::Low,
            message: format!(
                "SSM document {} is stored in TEXT format; convert to JSON or YAML for validated, reviewable content",
                resource.resource_id
            ),
            evidence: json!({ "document_format": "TEXT" }),
        });
    }
}

fn evaluate_security(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    // Sharing posture from DescribeDocumentPermission. Absent fields mean the
    // per-document permission call failed; report a gap, never fake a pass.
    match resource
        .resource_data
        .get("shared_publicly")
        .and_then(|v| v.as_bool())
    {
        Some(true) => {
            findings.push(InventoryFinding {
                resource_id: resource.resource_id.clone(),
                arn: resource.arn.clone(),
                pillar: Pillar::Security,
                reason_code: REASON_SEC_DOCUMENT_SHARED_PUBLICLY.to_string(),
                severity: Severity::High,
                message: format!(
                    "SSM document {} is shared publicly (account id 'all'); its content is readable by every AWS account",
                    resource.resource_id
                ),
                evidence: json!({
                    "shared_publicly": true,
                    "shared_account_ids": resource.resource_data.get("shared_account_ids"),
                }),
            });
        }
        Some(false) => {}
        None => {
            findings.push(InventoryFinding {
                resource_id: resource.resource_id.clone(),
                arn: resource.arn.clone(),
                pillar: Pillar::Security,
                reason_code: REASON_SEC_SHARING_DATA_NOT_COLLECTED.to_string(),
                severity: Severity::Low,
                message: format!(
                    "Sharing permissions for SSM document {} are not collected yet; public exposure cannot be assessed",
                    resource.resource_id
                ),
                evidence: json!({ "shared_publicly_collected": false }),
            });
        }
    }

    // Documents in the review workflow that are pending or rejected must not
    // be treated as approved automation content.
    let review_status = data_str(&resource.resource_data, "review_status");
    if matches!(review_status.as_deref(), Some("PENDING") | Some("REJECTED")) {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Security,
            reason_code: REASON_SEC_REVIEW_NOT_APPROVED.to_string(),
            severity: Severity::Medium,
            message: format!(
                "SSM document {} requires review but is in status {}; only APPROVED content should be executable",
                resource.resource_id,
                review_status.as_deref().unwrap_or("unknown")
            ),
            evidence: json!({ "review_status": review_status }),
        });
    }

    // The collector filters to Owner=Self, so the persisted owner should be
    // this account. Anything else is third-party content in the inventory.
    match data_str(&resource.resource_data, "owner") {
        Some(owner) => {
            if owner != resource.account_id {
                findings.push(InventoryFinding {
                    resource_id: resource.resource_id.clone(),
                    arn: resource.arn.clone(),
                    pillar: Pillar::Security,
                    reason_code: REASON_SEC_THIRD_PARTY_OWNER.to_string(),
                    severity: Severity::Medium,
                    message: format!(
                        "SSM document {} is owned by {} rather than account {}; third-party documents must be reviewed before execution",
                        resource.resource_id, owner, resource.account_id
                    ),
                    evidence: json!({ "owner": owner, "account_id": resource.account_id }),
                });
            }
        }
        None => {
            findings.push(InventoryFinding {
                resource_id: resource.resource_id.clone(),
                arn: resource.arn.clone(),
                pillar: Pillar::Security,
                reason_code: REASON_SEC_OWNER_DATA_NOT_COLLECTED.to_string(),
                severity: Severity::Low,
                message: format!(
                    "Owner for SSM document {} is not collected; document provenance cannot be assessed",
                    resource.resource_id
                ),
                evidence: json!({ "owner_collected": false }),
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

    const ACCOUNT: &str = "123456789012";

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
            account_id: ACCOUNT.to_string(),
            profile: None,
            region: "us-east-1".to_string(),
            resource_type: RESOURCE_TYPE.to_string(),
            resource_id: resource_id.to_string(),
            arn: format!("arn:aws:ssm:us-east-1:{}:document/{}", ACCOUNT, resource_id),
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
            "name": "doc-ok",
            "owner": ACCOUNT,
            "document_type": "Command",
            "document_format": "YAML",
            "document_version": "3",
            "schema_version": "2.2",
            "platform_types": ["Linux"],
            "created_date": "2026-05-01T00:00:00Z",
            "review_status": "APPROVED",
            "shared_account_ids": [],
            "shared_publicly": false,
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
    fn cost_flags_untagged_document() {
        let r = fixture("doc-untagged", json!({}), healthy_data(), now());
        let report = evaluate_ssm_fleet(&[r], Pillar::Cost, now());
        assert_eq!(codes(&report), vec![REASON_COST_NO_TAGS]);
    }

    #[test]
    fn cost_flags_aging_never_reviewed_document() {
        let mut data = healthy_data();
        data["review_status"] = json!("NOT_REVIEWED");
        data["created_date"] = json!("2023-01-01T00:00:00Z");
        let r = fixture("doc-old", json!({"team": "sre"}), data, now());
        let report = evaluate_ssm_fleet(&[r], Pillar::Cost, now());
        assert_eq!(codes(&report), vec![REASON_COST_AGING_NEVER_REVIEWED]);
    }

    #[test]
    fn cost_does_not_flag_recent_never_reviewed_document() {
        let mut data = healthy_data();
        data["review_status"] = json!("NOT_REVIEWED");
        data["created_date"] = json!("2026-05-15T00:00:00Z");
        let r = fixture("doc-new", json!({"team": "sre"}), data, now());
        let report = evaluate_ssm_fleet(&[r], Pillar::Cost, now());
        assert!(
            report.findings.is_empty(),
            "unexpected: {:?}",
            report.findings
        );
    }

    #[test]
    fn cost_does_not_flag_old_document_without_review_workflow() {
        // Most documents never report review_status; its absence means the
        // review workflow does not apply, not that the document is cruft.
        let mut data = healthy_data();
        data.as_object_mut().unwrap().remove("review_status");
        data["created_date"] = json!("2020-01-01T00:00:00Z");
        let r = fixture("doc-noreview", json!({"team": "sre"}), data, now());
        let report = evaluate_ssm_fleet(&[r], Pillar::Cost, now());
        assert!(
            report.findings.is_empty(),
            "unexpected: {:?}",
            report.findings
        );
    }

    #[test]
    fn cost_reports_gap_when_created_date_missing_for_never_reviewed_document() {
        let mut data = healthy_data();
        data["review_status"] = json!("NOT_REVIEWED");
        data.as_object_mut().unwrap().remove("created_date");
        let r = fixture("doc-nodate", json!({"team": "sre"}), data, now());
        let report = evaluate_ssm_fleet(&[r], Pillar::Cost, now());
        assert_eq!(
            codes(&report),
            vec![REASON_COST_CREATED_DATE_DATA_NOT_COLLECTED]
        );
    }

    #[test]
    fn resilience_flags_legacy_schema_command_document() {
        let mut data = healthy_data();
        data["schema_version"] = json!("1.2");
        let r = fixture("doc-legacy", json!({"team": "sre"}), data, now());
        let report = evaluate_ssm_fleet(&[r], Pillar::Resilience, now());
        assert_eq!(codes(&report), vec![REASON_RES_LEGACY_SCHEMA_VERSION]);
        assert!(matches!(report.findings[0].severity, Severity::Medium));
    }

    #[test]
    fn resilience_skips_schema_check_for_non_command_documents() {
        // Automation documents legitimately use schema 0.3.
        let mut data = healthy_data();
        data["document_type"] = json!("Automation");
        data["schema_version"] = json!("0.3");
        let r = fixture("doc-automation", json!({"team": "sre"}), data, now());
        let report = evaluate_ssm_fleet(&[r], Pillar::Resilience, now());
        assert!(
            report.findings.is_empty(),
            "unexpected: {:?}",
            report.findings
        );
    }

    #[test]
    fn resilience_reports_gap_when_schema_missing_for_command_document() {
        let mut data = healthy_data();
        data.as_object_mut().unwrap().remove("schema_version");
        let r = fixture("doc-noschema", json!({"team": "sre"}), data, now());
        let report = evaluate_ssm_fleet(&[r], Pillar::Resilience, now());
        assert_eq!(codes(&report), vec![REASON_RES_SCHEMA_DATA_NOT_COLLECTED]);
    }

    #[test]
    fn resilience_flags_text_format_document() {
        let mut data = healthy_data();
        data["document_format"] = json!("TEXT");
        let r = fixture("doc-text", json!({"team": "sre"}), data, now());
        let report = evaluate_ssm_fleet(&[r], Pillar::Resilience, now());
        assert_eq!(codes(&report), vec![REASON_RES_TEXT_FORMAT_DOCUMENT]);
    }

    #[test]
    fn security_flags_publicly_shared_document_as_high() {
        let mut data = healthy_data();
        data["shared_publicly"] = json!(true);
        data["shared_account_ids"] = json!(["all"]);
        let r = fixture("doc-public", json!({"team": "sre"}), data, now());
        let report = evaluate_ssm_fleet(&[r], Pillar::Security, now());
        assert_eq!(codes(&report), vec![REASON_SEC_DOCUMENT_SHARED_PUBLICLY]);
        assert!(matches!(report.findings[0].severity, Severity::High));
    }

    #[test]
    fn security_reports_gap_when_sharing_not_collected() {
        let mut data = healthy_data();
        data.as_object_mut().unwrap().remove("shared_publicly");
        data.as_object_mut().unwrap().remove("shared_account_ids");
        let r = fixture("doc-sharegap", json!({"team": "sre"}), data, now());
        let report = evaluate_ssm_fleet(&[r], Pillar::Security, now());
        assert_eq!(codes(&report), vec![REASON_SEC_SHARING_DATA_NOT_COLLECTED]);
    }

    #[test]
    fn security_flags_pending_and_rejected_review_statuses() {
        for status in ["PENDING", "REJECTED"] {
            let mut data = healthy_data();
            data["review_status"] = json!(status);
            let r = fixture("doc-review", json!({"team": "sre"}), data, now());
            let report = evaluate_ssm_fleet(&[r], Pillar::Security, now());
            assert_eq!(
                codes(&report),
                vec![REASON_SEC_REVIEW_NOT_APPROVED],
                "status {}",
                status
            );
        }
    }

    #[test]
    fn security_does_not_flag_not_reviewed_status_as_review_violation() {
        let mut data = healthy_data();
        data["review_status"] = json!("NOT_REVIEWED");
        let r = fixture("doc-notreviewed", json!({"team": "sre"}), data, now());
        let report = evaluate_ssm_fleet(&[r], Pillar::Security, now());
        assert!(
            report.findings.is_empty(),
            "unexpected: {:?}",
            report.findings
        );
    }

    #[test]
    fn security_flags_third_party_owner() {
        let mut data = healthy_data();
        data["owner"] = json!("999999999999");
        let r = fixture("doc-thirdparty", json!({"team": "sre"}), data, now());
        let report = evaluate_ssm_fleet(&[r], Pillar::Security, now());
        assert_eq!(codes(&report), vec![REASON_SEC_THIRD_PARTY_OWNER]);
    }

    #[test]
    fn security_reports_gap_when_owner_missing() {
        let mut data = healthy_data();
        data.as_object_mut().unwrap().remove("owner");
        let r = fixture("doc-noowner", json!({"team": "sre"}), data, now());
        let report = evaluate_ssm_fleet(&[r], Pillar::Security, now());
        assert_eq!(codes(&report), vec![REASON_SEC_OWNER_DATA_NOT_COLLECTED]);
    }

    #[test]
    fn stale_inventory_is_flagged() {
        let mut r = fixture("doc-stale", json!({"team": "sre"}), healthy_data(), now());
        r.last_refreshed = now() - Duration::hours(48);
        let report = evaluate_ssm_fleet(&[r], Pillar::Resilience, now());
        assert_eq!(report.stale_resources, 1);
        assert!(codes(&report).contains(&REASON_INV_STALE_DATA));
    }

    #[test]
    fn non_ssm_resources_are_skipped_and_not_counted() {
        let mut r = fixture("key-1", json!({}), json!({}), now());
        r.resource_type = "KmsKey".to_string();
        let report = evaluate_ssm_fleet(&[r], Pillar::Cost, now());
        assert_eq!(report.resources_evaluated, 0);
        assert!(report.findings.is_empty());
    }

    #[test]
    fn healthy_document_passes_all_pillars() {
        let r = fixture("doc-ok", json!({"team": "sre"}), healthy_data(), now());
        for pillar in [Pillar::Cost, Pillar::Security, Pillar::Resilience] {
            let report = evaluate_ssm_fleet(std::slice::from_ref(&r), pillar, now());
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

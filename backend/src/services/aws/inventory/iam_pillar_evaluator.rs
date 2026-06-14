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

// Deterministic IAM inventory evaluators for the cost, security, and
// resilience pillars (roadmap rows 01-AWS-CLOUD-03592/03601/03628).
//
// Evaluates IamUser, IamRole, IamPolicy, and IamGroup rows persisted by
// iam_control_plane: permissions_boundary, password_last_used,
// assume_role_policy_document, attachment_count,
// permissions_boundary_usage_count, plus tags for users and roles. IAM is
// global and free, so cost findings are governance/cost-hygiene signals,
// not direct spend.

use chrono::{DateTime, Utc};
use serde_json::json;

use crate::models::aws_resource::Model as AwsResourceModel;
use crate::services::aws::inventory::types::{
    check_stale, data_str, score_pillar, InventoryFinding, Pillar, PillarReport, Severity,
};

// Reason codes are the stable contract for findings; never reuse or rename.
pub const REASON_COST_NO_TAGS: &str = "IAM_COST_NO_TAGS";
pub const REASON_COST_UNUSED_POLICY: &str = "IAM_COST_UNUSED_POLICY";
pub const REASON_SEC_USER_NO_PERMISSIONS_BOUNDARY: &str = "IAM_SEC_USER_NO_PERMISSIONS_BOUNDARY";
pub const REASON_SEC_USER_STALE_PASSWORD: &str = "IAM_SEC_USER_STALE_PASSWORD";
pub const REASON_SEC_ROLE_NO_PERMISSIONS_BOUNDARY: &str = "IAM_SEC_ROLE_NO_PERMISSIONS_BOUNDARY";
pub const REASON_SEC_GROUP_POSTURE_DATA_NOT_COLLECTED: &str =
    "IAM_SEC_GROUP_POSTURE_DATA_NOT_COLLECTED";
pub const REASON_RES_ROLE_TRUST_POLICY_NOT_COLLECTED: &str =
    "IAM_RES_ROLE_TRUST_POLICY_NOT_COLLECTED";
pub const REASON_INV_STALE_DATA: &str = "IAM_INV_STALE_DATA";

/// A console password unused for longer than this is a stale credential.
pub const STALE_PASSWORD_AFTER_DAYS: i64 = 90;

const TYPE_USER: &str = "IamUser";
const TYPE_ROLE: &str = "IamRole";
const TYPE_POLICY: &str = "IamPolicy";
const TYPE_GROUP: &str = "IamGroup";

/// Evaluate every IAM user, role, policy, and group in the fleet for one pillar.
pub fn evaluate_iam_fleet(
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
            Pillar::Security => evaluate_security(resource, now, &mut findings),
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

fn tags_missing(resource: &AwsResourceModel) -> bool {
    resource
        .tags
        .as_object()
        .map(|m| m.is_empty())
        .unwrap_or(true)
}

/// The collector serializes an absent permissions boundary as JSON null.
fn has_permissions_boundary(resource: &AwsResourceModel) -> bool {
    resource
        .resource_data
        .get("permissions_boundary")
        .map(|v| !v.is_null())
        .unwrap_or(false)
}

fn data_i64(resource: &AwsResourceModel, key: &str) -> Option<i64> {
    resource.resource_data.get(key).and_then(|v| v.as_i64())
}

fn evaluate_cost(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    // Users and roles are taggable and the collector gathers their tags.
    // IAM itself is free, so a tag gap is a governance signal, not spend.
    if (resource.resource_type == TYPE_USER || resource.resource_type == TYPE_ROLE)
        && tags_missing(resource)
    {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Cost,
            reason_code: REASON_COST_NO_TAGS.to_string(),
            severity: Severity::Low,
            message: format!(
                "{} {} has no tags recorded; ownership and cost-governance attribution cannot be assessed (IAM is free, so this is a hygiene signal)",
                resource.resource_type, resource.resource_id
            ),
            evidence: json!({ "tags": resource.tags }),
        });
    }

    if resource.resource_type == TYPE_POLICY {
        let attachments = data_i64(resource, "attachment_count");
        let boundary_usage = data_i64(resource, "permissions_boundary_usage_count");
        if attachments == Some(0) && boundary_usage == Some(0) {
            findings.push(InventoryFinding {
                resource_id: resource.resource_id.clone(),
                arn: resource.arn.clone(),
                pillar: Pillar::Cost,
                reason_code: REASON_COST_UNUSED_POLICY.to_string(),
                severity: Severity::Low,
                message: format!(
                    "Policy {} is attached to no identity and used by no permissions boundary; it is unused inventory that can be removed",
                    resource.resource_id
                ),
                evidence: json!({
                    "attachment_count": 0,
                    "permissions_boundary_usage_count": 0,
                }),
            });
        }
    }
}

fn evaluate_security(
    resource: &AwsResourceModel,
    now: DateTime<Utc>,
    findings: &mut Vec<InventoryFinding>,
) {
    match resource.resource_type.as_str() {
        TYPE_USER => {
            if !has_permissions_boundary(resource) {
                findings.push(InventoryFinding {
                    resource_id: resource.resource_id.clone(),
                    arn: resource.arn.clone(),
                    pillar: Pillar::Security,
                    reason_code: REASON_SEC_USER_NO_PERMISSIONS_BOUNDARY.to_string(),
                    severity: Severity::Medium,
                    message: format!(
                        "User {} has no permissions boundary; its maximum permissions are not constrained",
                        resource.resource_id
                    ),
                    evidence: json!({ "permissions_boundary": null }),
                });
            }

            // password_last_used is null when console access is disabled or
            // never used; only a parseable timestamp can prove staleness.
            let last_used = data_str(&resource.resource_data, "password_last_used")
                .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
                .map(|d| d.with_timezone(&Utc));
            if let Some(last_used) = last_used {
                let age_days = (now - last_used).num_days();
                if age_days > STALE_PASSWORD_AFTER_DAYS {
                    findings.push(InventoryFinding {
                        resource_id: resource.resource_id.clone(),
                        arn: resource.arn.clone(),
                        pillar: Pillar::Security,
                        reason_code: REASON_SEC_USER_STALE_PASSWORD.to_string(),
                        severity: Severity::High,
                        message: format!(
                            "User {} last used its console password {} days ago (threshold {} days); the credential is stale and should be disabled or rotated",
                            resource.resource_id, age_days, STALE_PASSWORD_AFTER_DAYS
                        ),
                        evidence: json!({
                            "password_last_used": last_used,
                            "age_days": age_days,
                            "stale_after_days": STALE_PASSWORD_AFTER_DAYS,
                        }),
                    });
                }
            }
        }
        TYPE_ROLE => {
            if !has_permissions_boundary(resource) {
                findings.push(InventoryFinding {
                    resource_id: resource.resource_id.clone(),
                    arn: resource.arn.clone(),
                    pillar: Pillar::Security,
                    reason_code: REASON_SEC_ROLE_NO_PERMISSIONS_BOUNDARY.to_string(),
                    severity: Severity::Low,
                    message: format!(
                        "Role {} has no permissions boundary; its maximum permissions are not constrained",
                        resource.resource_id
                    ),
                    evidence: json!({ "permissions_boundary": null }),
                });
            }
        }
        TYPE_GROUP => {
            // The collector gathers no membership or attached-policy data
            // for groups yet; report the gap rather than scoring blind.
            findings.push(InventoryFinding {
                resource_id: resource.resource_id.clone(),
                arn: resource.arn.clone(),
                pillar: Pillar::Security,
                reason_code: REASON_SEC_GROUP_POSTURE_DATA_NOT_COLLECTED.to_string(),
                severity: Severity::Medium,
                message: format!(
                    "Security posture (members, attached policies) for group {} is not collected yet",
                    resource.resource_id
                ),
                evidence: json!({
                    "collected_fields": resource
                        .resource_data
                        .as_object()
                        .map(|m| m.keys().cloned().collect::<Vec<_>>()),
                }),
            });
        }
        _ => {}
    }
}

fn evaluate_resilience(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    // IAM has no per-resource availability posture in the collected fields;
    // the one assessable signal is role assumability via the trust policy.
    if resource.resource_type == TYPE_ROLE {
        let trust_policy_present = resource
            .resource_data
            .get("assume_role_policy_document")
            .and_then(|v| v.as_str())
            .map(|s| !s.is_empty())
            .unwrap_or(false);
        if !trust_policy_present {
            findings.push(InventoryFinding {
                resource_id: resource.resource_id.clone(),
                arn: resource.arn.clone(),
                pillar: Pillar::Resilience,
                reason_code: REASON_RES_ROLE_TRUST_POLICY_NOT_COLLECTED.to_string(),
                severity: Severity::Medium,
                message: format!(
                    "Trust policy for role {} is not collected; whether workloads can assume it after an incident cannot be assessed",
                    resource.resource_id
                ),
                evidence: json!({ "assume_role_policy_document_collected": false }),
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
        resource_type: &str,
        resource_id: &str,
        arn_suffix: &str,
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
            region: "global".to_string(),
            resource_type: resource_type.to_string(),
            resource_id: resource_id.to_string(),
            arn: format!("arn:aws:iam::123456789012:{}", arn_suffix),
            name: Some(resource_id.to_string()),
            tags,
            resource_data,
            created_at: refreshed,
            updated_at: refreshed,
            last_refreshed: refreshed,
        }
    }

    fn user_fixture(id: &str, tags: Value, data: Value, now: DateTime<Utc>) -> AwsResourceModel {
        fixture("IamUser", id, &format!("user/{}", id), tags, data, now)
    }

    fn role_fixture(id: &str, tags: Value, data: Value, now: DateTime<Utc>) -> AwsResourceModel {
        fixture("IamRole", id, &format!("role/{}", id), tags, data, now)
    }

    fn policy_fixture(id: &str, tags: Value, data: Value, now: DateTime<Utc>) -> AwsResourceModel {
        fixture("IamPolicy", id, &format!("policy/{}", id), tags, data, now)
    }

    fn group_fixture(id: &str, data: Value, now: DateTime<Utc>) -> AwsResourceModel {
        fixture(
            "IamGroup",
            id,
            &format!("group/{}", id),
            json!({}),
            data,
            now,
        )
    }

    fn now() -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2026-06-10T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc)
    }

    fn boundary() -> Value {
        json!({
            "permissions_boundary_type": "Policy",
            "permissions_boundary_arn": "arn:aws:iam::123456789012:policy/boundary",
        })
    }

    fn healthy_user_data() -> Value {
        json!({
            "user_name": "alice",
            "user_id": "AIDAEXAMPLE1",
            "path": "/",
            "create_date": "2025-01-01T00:00:00Z",
            "password_last_used": "2026-06-01T00:00:00Z",
            "permissions_boundary": boundary(),
        })
    }

    fn healthy_role_data() -> Value {
        json!({
            "role_name": "app-role",
            "role_id": "AROAEXAMPLE1",
            "path": "/",
            "create_date": "2025-01-01T00:00:00Z",
            "assume_role_policy_document": "%7B%22Version%22%3A%222012-10-17%22%7D",
            "description": "app role",
            "max_session_duration": 3600,
            "permissions_boundary": boundary(),
        })
    }

    fn healthy_policy_data() -> Value {
        json!({
            "policy_name": "app-policy",
            "policy_id": "ANPAEXAMPLE1",
            "path": "/",
            "default_version_id": "v2",
            "attachment_count": 2,
            "permissions_boundary_usage_count": 0,
            "is_attachable": true,
            "description": "app policy",
            "create_date": "2025-01-01T00:00:00Z",
            "update_date": "2025-06-01T00:00:00Z",
        })
    }

    #[test]
    fn cost_flags_untagged_user_and_role_but_not_policy() {
        let user = user_fixture("AIDAUNTAGGED", json!({}), healthy_user_data(), now());
        let role = role_fixture("AROAUNTAGGED", json!({}), healthy_role_data(), now());
        let policy = policy_fixture("ANPAUNTAGGED", json!({}), healthy_policy_data(), now());
        let report = evaluate_iam_fleet(&[user, role, policy], Pillar::Cost, now());
        let codes: Vec<&str> = report
            .findings
            .iter()
            .map(|f| f.reason_code.as_str())
            .collect();
        assert_eq!(codes, vec![REASON_COST_NO_TAGS, REASON_COST_NO_TAGS]);
    }

    #[test]
    fn cost_flags_unused_policy() {
        let mut data = healthy_policy_data();
        data["attachment_count"] = json!(0);
        data["permissions_boundary_usage_count"] = json!(0);
        let policy = policy_fixture("ANPAUNUSED", json!({"team": "sec"}), data, now());
        let report = evaluate_iam_fleet(&[policy], Pillar::Cost, now());
        assert_eq!(
            report
                .findings
                .iter()
                .map(|f| f.reason_code.as_str())
                .collect::<Vec<_>>(),
            vec![REASON_COST_UNUSED_POLICY]
        );
    }

    #[test]
    fn security_flags_user_without_boundary_and_stale_password() {
        let mut data = healthy_user_data();
        data["permissions_boundary"] = json!(null);
        data["password_last_used"] = json!("2025-06-01T00:00:00Z");
        let user = user_fixture("AIDARISKY", json!({"team": "sec"}), data, now());
        let report = evaluate_iam_fleet(&[user], Pillar::Security, now());
        let codes: Vec<&str> = report
            .findings
            .iter()
            .map(|f| f.reason_code.as_str())
            .collect();
        assert!(codes.contains(&REASON_SEC_USER_NO_PERMISSIONS_BOUNDARY));
        assert!(codes.contains(&REASON_SEC_USER_STALE_PASSWORD));
        let stale = report
            .findings
            .iter()
            .find(|f| f.reason_code == REASON_SEC_USER_STALE_PASSWORD)
            .unwrap();
        assert_eq!(stale.severity, Severity::High);
    }

    #[test]
    fn security_does_not_flag_password_when_never_used() {
        let mut data = healthy_user_data();
        data["password_last_used"] = json!(null);
        let user = user_fixture("AIDANOPASS", json!({"team": "sec"}), data, now());
        let report = evaluate_iam_fleet(&[user], Pillar::Security, now());
        assert!(
            report.findings.is_empty(),
            "unexpected: {:?}",
            report.findings
        );
    }

    #[test]
    fn security_flags_role_without_boundary() {
        let mut data = healthy_role_data();
        data["permissions_boundary"] = json!(null);
        let role = role_fixture("AROAUNBOUND", json!({"team": "sec"}), data, now());
        let report = evaluate_iam_fleet(&[role], Pillar::Security, now());
        assert_eq!(
            report
                .findings
                .iter()
                .map(|f| f.reason_code.as_str())
                .collect::<Vec<_>>(),
            vec![REASON_SEC_ROLE_NO_PERMISSIONS_BOUNDARY]
        );
    }

    #[test]
    fn security_reports_group_posture_data_gap() {
        let group = group_fixture(
            "AGPAEXAMPLE1",
            json!({"group_name": "admins", "group_id": "AGPAEXAMPLE1", "path": "/", "create_date": "2025-01-01T00:00:00Z"}),
            now(),
        );
        let report = evaluate_iam_fleet(&[group], Pillar::Security, now());
        assert_eq!(
            report
                .findings
                .iter()
                .map(|f| f.reason_code.as_str())
                .collect::<Vec<_>>(),
            vec![REASON_SEC_GROUP_POSTURE_DATA_NOT_COLLECTED]
        );
    }

    #[test]
    fn resilience_flags_role_with_missing_trust_policy() {
        let mut data = healthy_role_data();
        data["assume_role_policy_document"] = json!(null);
        let role = role_fixture("AROANOTRUST", json!({"team": "sec"}), data, now());
        let report = evaluate_iam_fleet(&[role], Pillar::Resilience, now());
        assert_eq!(
            report
                .findings
                .iter()
                .map(|f| f.reason_code.as_str())
                .collect::<Vec<_>>(),
            vec![REASON_RES_ROLE_TRUST_POLICY_NOT_COLLECTED]
        );
    }

    #[test]
    fn stale_inventory_rows_are_flagged() {
        let mut user = user_fixture(
            "AIDAOLD",
            json!({"team": "sec"}),
            healthy_user_data(),
            now(),
        );
        user.last_refreshed = now() - Duration::hours(48);
        let report = evaluate_iam_fleet(&[user], Pillar::Cost, now());
        assert_eq!(report.stale_resources, 1);
        assert!(report
            .findings
            .iter()
            .any(|f| f.reason_code == REASON_INV_STALE_DATA));
    }

    #[test]
    fn healthy_identities_pass_all_pillars() {
        let user = user_fixture(
            "AIDAHEALTHY",
            json!({"team": "sec"}),
            healthy_user_data(),
            now(),
        );
        let role = role_fixture(
            "AROAHEALTHY",
            json!({"team": "sec"}),
            healthy_role_data(),
            now(),
        );
        let policy = policy_fixture(
            "ANPAHEALTHY",
            json!({"team": "sec"}),
            healthy_policy_data(),
            now(),
        );
        let fleet = [user, role, policy];
        for pillar in [Pillar::Cost, Pillar::Security, Pillar::Resilience] {
            let report = evaluate_iam_fleet(&fleet, pillar, now());
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

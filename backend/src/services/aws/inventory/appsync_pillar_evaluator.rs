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

// Deterministic AppSync inventory evaluators for the cost, security, and
// resilience pillars (roadmap rows 01-AWS-CLOUD-02584/02593/02620).
//
// Evaluates fields persisted by appsync_control_plane: ApiId, Name, Arn,
// AuthenticationType, Uris. X-Ray tracing state is not collected yet and is
// reported as an explicit data gap.

use chrono::{DateTime, Utc};
use serde_json::json;

use crate::models::aws_resource::Model as AwsResourceModel;
use crate::services::aws::inventory::types::{
    check_stale, score_pillar, InventoryFinding, Pillar, PillarReport, Severity,
};

/// Resource type persisted by appsync_control_plane.
const APPSYNC_API_TYPE: &str = "AppSyncApi";

// Reason codes are the stable contract for findings; never reuse or rename.
pub const REASON_COST_NO_TAGS: &str = "APPSYNC_COST_NO_TAGS";
pub const REASON_SEC_API_KEY_AUTH: &str = "APPSYNC_SEC_API_KEY_AUTH";
pub const REASON_SEC_AUTH_DATA_NOT_COLLECTED: &str = "APPSYNC_SEC_AUTH_DATA_NOT_COLLECTED";
pub const REASON_RES_XRAY_DATA_NOT_COLLECTED: &str = "APPSYNC_RES_XRAY_DATA_NOT_COLLECTED";
pub const REASON_INV_STALE_DATA: &str = "APPSYNC_INV_STALE_DATA";

/// Evaluate every AppSync GraphQL API in the fleet for one pillar.
/// Rows that are not `AppSyncApi` resources are skipped.
pub fn evaluate_appsync_fleet(
    resources: &[AwsResourceModel],
    pillar: Pillar,
    now: DateTime<Utc>,
) -> PillarReport {
    let mut findings: Vec<InventoryFinding> = Vec::new();
    let mut stale_resources = 0usize;
    let mut resources_evaluated = 0usize;

    for resource in resources {
        if resource.resource_type != APPSYNC_API_TYPE {
            continue;
        }
        resources_evaluated += 1;
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
        resources_evaluated,
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
                "API {} has no tags recorded (untagged resource or tag collection gap); cost allocation cannot be assessed",
                resource.resource_id
            ),
            evidence: json!({ "tags": resource.tags }),
        });
    }
}

fn evaluate_security(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    let auth_type = resource
        .resource_data
        .get("AuthenticationType")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty());

    match auth_type {
        None => {
            findings.push(InventoryFinding {
                resource_id: resource.resource_id.clone(),
                arn: resource.arn.clone(),
                pillar: Pillar::Security,
                reason_code: REASON_SEC_AUTH_DATA_NOT_COLLECTED.to_string(),
                severity: Severity::Medium,
                message: format!(
                    "Authentication type for API {} is not collected yet; security pillar cannot be fully assessed",
                    resource.resource_id
                ),
                evidence: json!({ "authentication_type_collected": false }),
            });
        }
        Some("API_KEY") => {
            findings.push(InventoryFinding {
                resource_id: resource.resource_id.clone(),
                arn: resource.arn.clone(),
                pillar: Pillar::Security,
                reason_code: REASON_SEC_API_KEY_AUTH.to_string(),
                severity: Severity::Medium,
                message: format!(
                    "API {} uses API key authentication, the weakest AppSync auth mode; prefer IAM, Cognito, OIDC, or Lambda authorization",
                    resource.resource_id
                ),
                evidence: json!({ "AuthenticationType": "API_KEY" }),
            });
        }
        Some(_) => {}
    }
}

fn evaluate_resilience(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    // The collector does not persist X-Ray tracing state yet; report the gap
    // honestly instead of inventing a check from uncollected data.
    if resource.resource_data.get("XrayEnabled").is_none() {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Resilience,
            reason_code: REASON_RES_XRAY_DATA_NOT_COLLECTED.to_string(),
            severity: Severity::Medium,
            message: format!(
                "X-Ray tracing state for API {} is not collected yet; resilience pillar cannot be fully assessed",
                resource.resource_id
            ),
            evidence: json!({ "xray_enabled_collected": false }),
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
            resource_type: "AppSyncApi".to_string(),
            resource_id: resource_id.to_string(),
            arn: format!(
                "arn:aws:appsync:us-east-1:123456789012:apis/{}",
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
            "ApiId": "abc123",
            "Name": "orders-api",
            "Arn": "arn:aws:appsync:us-east-1:123456789012:apis/abc123",
            "AuthenticationType": "AWS_IAM",
            "Uris": { "GRAPHQL": "https://abc123.appsync-api.us-east-1.amazonaws.com/graphql" },
            "XrayEnabled": true,
        })
    }

    #[test]
    fn cost_reports_tag_gap_for_untagged_api() {
        let r = fixture("api-untagged", json!({}), healthy_data(), now());
        let report = evaluate_appsync_fleet(&[r], Pillar::Cost, now());
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
    fn security_flags_api_key_authentication() {
        let mut data = healthy_data();
        data["AuthenticationType"] = json!("API_KEY");
        let r = fixture("api-key-auth", json!({"team": "graphql"}), data, now());
        let report = evaluate_appsync_fleet(&[r], Pillar::Security, now());
        assert_eq!(
            report
                .findings
                .iter()
                .map(|f| f.reason_code.as_str())
                .collect::<Vec<_>>(),
            vec![REASON_SEC_API_KEY_AUTH]
        );
    }

    #[test]
    fn security_reports_gap_when_auth_not_collected() {
        let r = fixture(
            "api-auth-gap",
            json!({"team": "graphql"}),
            json!({"ApiId": "abc123", "AuthenticationType": "", "XrayEnabled": true}),
            now(),
        );
        let report = evaluate_appsync_fleet(&[r], Pillar::Security, now());
        assert_eq!(
            report
                .findings
                .iter()
                .map(|f| f.reason_code.as_str())
                .collect::<Vec<_>>(),
            vec![REASON_SEC_AUTH_DATA_NOT_COLLECTED]
        );
    }

    #[test]
    fn resilience_reports_gap_when_xray_not_collected() {
        let r = fixture(
            "api-xray-gap",
            json!({"team": "graphql"}),
            json!({"ApiId": "abc123", "AuthenticationType": "AWS_IAM"}),
            now(),
        );
        let report = evaluate_appsync_fleet(&[r], Pillar::Resilience, now());
        assert_eq!(
            report
                .findings
                .iter()
                .map(|f| f.reason_code.as_str())
                .collect::<Vec<_>>(),
            vec![REASON_RES_XRAY_DATA_NOT_COLLECTED]
        );
    }

    #[test]
    fn stale_inventory_is_flagged() {
        let r = fixture(
            "api-stale",
            json!({"team": "graphql"}),
            healthy_data(),
            now(),
        );
        let later = now() + Duration::hours(48);
        let report = evaluate_appsync_fleet(&[r], Pillar::Cost, later);
        assert_eq!(report.stale_resources, 1);
        assert!(report
            .findings
            .iter()
            .any(|f| f.reason_code == REASON_INV_STALE_DATA));
    }

    #[test]
    fn non_appsync_rows_are_skipped() {
        let mut other = fixture("not-appsync", json!({}), json!({}), now());
        other.resource_type = "SnsTopic".to_string();
        let report = evaluate_appsync_fleet(&[other], Pillar::Security, now());
        assert_eq!(report.resources_evaluated, 0);
        assert!(report.findings.is_empty());
    }

    #[test]
    fn healthy_api_passes_all_pillars() {
        let r = fixture("api-ok", json!({"team": "graphql"}), healthy_data(), now());
        for pillar in [Pillar::Cost, Pillar::Security, Pillar::Resilience] {
            let report = evaluate_appsync_fleet(std::slice::from_ref(&r), pillar, now());
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

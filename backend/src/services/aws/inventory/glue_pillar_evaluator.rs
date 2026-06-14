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

// Deterministic Glue Data Catalog database evaluators for the cost,
// security, and resilience pillars.
//
// Evaluates fields persisted by glue_control_plane: description,
// location_uri, create_time, table_count/table_count_truncated (bounded
// sample), create_table_default_permissions_count,
// default_permissions_grant_all_to_iam_allowed_principals (the Lake
// Formation legacy-open-catalog signal), is_federated, is_resource_link,
// plus the tags column. Resource links and federated databases have no
// storage location of their own, so the location checks are gated on both
// flags being false.

use chrono::{DateTime, Utc};
use serde_json::json;

use crate::models::aws_resource::Model as AwsResourceModel;
use crate::services::aws::inventory::types::{
    check_stale, data_str, score_pillar, InventoryFinding, Pillar, PillarReport, Severity,
};

/// Only rows of this resource type are evaluated.
pub const RESOURCE_TYPE: &str = "GlueDatabase";

// Reason codes are the stable contract for findings; never reuse or rename.
pub const REASON_COST_NO_TAGS: &str = "GLUE_COST_NO_TAGS";
pub const REASON_COST_EMPTY_DATABASE: &str = "GLUE_COST_EMPTY_DATABASE";
pub const REASON_COST_TABLE_COUNT_NOT_COLLECTED: &str = "GLUE_COST_TABLE_COUNT_NOT_COLLECTED";
pub const REASON_RES_NO_DESCRIPTION: &str = "GLUE_RES_NO_DESCRIPTION";
pub const REASON_RES_NO_LOCATION_URI: &str = "GLUE_RES_NO_LOCATION_URI";
pub const REASON_RES_CREATE_TIME_NOT_COLLECTED: &str = "GLUE_RES_CREATE_TIME_NOT_COLLECTED";
pub const REASON_SEC_DEFAULT_PERMISSIONS_OPEN: &str = "GLUE_SEC_DEFAULT_PERMISSIONS_OPEN";
pub const REASON_SEC_DEFAULT_PERMISSIONS_NOT_COLLECTED: &str =
    "GLUE_SEC_DEFAULT_PERMISSIONS_NOT_COLLECTED";
pub const REASON_SEC_NON_S3_LOCATION: &str = "GLUE_SEC_NON_S3_LOCATION";
pub const REASON_INV_STALE_DATA: &str = "GLUE_INV_STALE_DATA";

/// Evaluate every Glue database in the fleet for one pillar. Rows whose
/// `resource_type` is not `GlueDatabase` are skipped and not counted.
pub fn evaluate_glue_fleet(
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

fn data_bool(resource: &AwsResourceModel, key: &str) -> Option<bool> {
    resource.resource_data.get(key).and_then(|v| v.as_bool())
}

fn data_u64(resource: &AwsResourceModel, key: &str) -> Option<u64> {
    resource.resource_data.get(key).and_then(|v| v.as_u64())
}

/// Resource links and federated databases reference storage owned elsewhere,
/// so location-based checks do not apply to them.
fn owns_storage_location(resource: &AwsResourceModel) -> bool {
    data_bool(resource, "is_resource_link") != Some(true)
        && data_bool(resource, "is_federated") != Some(true)
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
                "Glue database {} has no tags recorded (untagged resource or tag collection gap); cost allocation cannot be assessed",
                resource.resource_id
            ),
            evidence: json!({ "tags": resource.tags }),
        });
    }

    // Empty-database signal from the bounded table sample. The count is
    // exact only when the sample was not truncated, so an empty result with
    // table_count_truncated == false means the database really has no tables.
    match data_u64(resource, "table_count") {
        Some(0) if data_bool(resource, "table_count_truncated") == Some(false) => {
            findings.push(InventoryFinding {
                resource_id: resource.resource_id.clone(),
                arn: resource.arn.clone(),
                pillar: Pillar::Cost,
                reason_code: REASON_COST_EMPTY_DATABASE.to_string(),
                severity: Severity::Low,
                message: format!(
                    "Glue database {} contains no tables; it may be unused catalog clutter that should be reviewed or removed",
                    resource.resource_id
                ),
                evidence: json!({ "table_count": 0, "table_count_truncated": false }),
            });
        }
        Some(_) => {}
        None => {
            findings.push(InventoryFinding {
                resource_id: resource.resource_id.clone(),
                arn: resource.arn.clone(),
                pillar: Pillar::Cost,
                reason_code: REASON_COST_TABLE_COUNT_NOT_COLLECTED.to_string(),
                severity: Severity::Low,
                message: format!(
                    "Table count for Glue database {} is not collected yet; unused-database posture cannot be assessed",
                    resource.resource_id
                ),
                evidence: json!({ "table_count_collected": false }),
            });
        }
    }
}

fn evaluate_security(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    // Lake Formation legacy-open-catalog signal: default table permissions
    // grant ALL to the IAM_ALLOWED_PRINCIPALS virtual group, so any IAM
    // principal with Glue API access gets full access to new tables.
    match data_bool(
        resource,
        "default_permissions_grant_all_to_iam_allowed_principals",
    ) {
        Some(true) => {
            findings.push(InventoryFinding {
                resource_id: resource.resource_id.clone(),
                arn: resource.arn.clone(),
                pillar: Pillar::Security,
                reason_code: REASON_SEC_DEFAULT_PERMISSIONS_OPEN.to_string(),
                severity: Severity::Medium,
                message: format!(
                    "Glue database {} grants ALL to IAM_ALLOWED_PRINCIPALS in its default table permissions (legacy Lake Formation open-catalog mode); new tables are open to any IAM principal with Glue access",
                    resource.resource_id
                ),
                evidence: json!({
                    "default_permissions_grant_all_to_iam_allowed_principals": true,
                    "create_table_default_permissions_count":
                        data_u64(resource, "create_table_default_permissions_count"),
                }),
            });
        }
        Some(false) => {}
        None => {
            findings.push(InventoryFinding {
                resource_id: resource.resource_id.clone(),
                arn: resource.arn.clone(),
                pillar: Pillar::Security,
                reason_code: REASON_SEC_DEFAULT_PERMISSIONS_NOT_COLLECTED.to_string(),
                severity: Severity::Low,
                message: format!(
                    "Default table permissions for Glue database {} are not collected yet; security pillar cannot be fully assessed",
                    resource.resource_id
                ),
                evidence: json!({ "default_permissions_collected": false }),
            });
        }
    }

    // Informational: a database whose location is outside S3 (HDFS, JDBC
    // paths, etc.) sits outside S3-native guardrails such as bucket policies,
    // Block Public Access, and default encryption.
    if owns_storage_location(resource) {
        if let Some(location_uri) = data_str(&resource.resource_data, "location_uri") {
            let trimmed = location_uri.trim();
            if !trimmed.is_empty() && !trimmed.to_ascii_lowercase().starts_with("s3://") {
                findings.push(InventoryFinding {
                    resource_id: resource.resource_id.clone(),
                    arn: resource.arn.clone(),
                    pillar: Pillar::Security,
                    reason_code: REASON_SEC_NON_S3_LOCATION.to_string(),
                    severity: Severity::Low,
                    message: format!(
                        "Glue database {} points at a non-S3 location {}; S3-native guardrails (bucket policy, Block Public Access, default encryption) do not apply",
                        resource.resource_id, trimmed
                    ),
                    evidence: json!({ "location_uri": trimmed }),
                });
            }
        }
    }
}

fn evaluate_resilience(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    let description_missing = data_str(&resource.resource_data, "description")
        .map(|d| d.trim().is_empty())
        .unwrap_or(true);
    if description_missing {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Resilience,
            reason_code: REASON_RES_NO_DESCRIPTION.to_string(),
            severity: Severity::Low,
            message: format!(
                "Glue database {} has no description; operators cannot tell its purpose during an incident",
                resource.resource_id
            ),
            evidence: json!({ "description": resource.resource_data.get("description") }),
        });
    }

    // A database that owns its storage but declares no location forces every
    // table to carry its own ad-hoc path, which weakens recovery and
    // relocation procedures. Resource links and federated databases are
    // exempt because their storage lives in the target catalog.
    if owns_storage_location(resource) {
        let location_missing = data_str(&resource.resource_data, "location_uri")
            .map(|l| l.trim().is_empty())
            .unwrap_or(true);
        if location_missing {
            findings.push(InventoryFinding {
                resource_id: resource.resource_id.clone(),
                arn: resource.arn.clone(),
                pillar: Pillar::Resilience,
                reason_code: REASON_RES_NO_LOCATION_URI.to_string(),
                severity: Severity::Low,
                message: format!(
                    "Glue database {} declares no location URI; table storage paths are ad-hoc and harder to back up or relocate",
                    resource.resource_id
                ),
                evidence: json!({ "location_uri": resource.resource_data.get("location_uri") }),
            });
        }
    }

    if data_str(&resource.resource_data, "create_time").is_none() {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Resilience,
            reason_code: REASON_RES_CREATE_TIME_NOT_COLLECTED.to_string(),
            severity: Severity::Low,
            message: format!(
                "Creation time for Glue database {} is not collected yet; resource age cannot be assessed",
                resource.resource_id
            ),
            evidence: json!({ "create_time_collected": false }),
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
                "arn:aws:glue:us-east-1:123456789012:database/{}",
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
            "name": "analytics",
            "arn": "arn:aws:glue:us-east-1:123456789012:database/analytics",
            "description": "Curated analytics tables",
            "location_uri": "s3://corp-analytics/warehouse/",
            "catalog_id": "123456789012",
            "create_time": "2024-03-01T00:00:00Z",
            "parameters_count": 2,
            "create_table_default_permissions_count": 0,
            "default_permissions_grant_all_to_iam_allowed_principals": false,
            "is_federated": false,
            "is_resource_link": false,
            "table_count": 5,
            "table_count_truncated": false,
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
    fn cost_flags_untagged_database() {
        let r = fixture("db-untagged", json!({}), healthy_data(), now());
        let report = evaluate_glue_fleet(&[r], Pillar::Cost, now());
        assert_eq!(codes(&report), vec![REASON_COST_NO_TAGS]);
    }

    #[test]
    fn cost_flags_empty_database_with_exact_count() {
        let mut data = healthy_data();
        data["table_count"] = json!(0);
        data["table_count_truncated"] = json!(false);
        let r = fixture("db-empty", json!({"team": "data"}), data, now());
        let report = evaluate_glue_fleet(&[r], Pillar::Cost, now());
        assert_eq!(codes(&report), vec![REASON_COST_EMPTY_DATABASE]);
        assert!(matches!(report.findings[0].severity, Severity::Low));
    }

    #[test]
    fn cost_does_not_flag_empty_when_sample_truncated() {
        let mut data = healthy_data();
        data["table_count"] = json!(0);
        data["table_count_truncated"] = json!(true);
        let r = fixture("db-trunc", json!({"team": "data"}), data, now());
        let report = evaluate_glue_fleet(&[r], Pillar::Cost, now());
        assert!(
            report.findings.is_empty(),
            "unexpected: {:?}",
            report.findings
        );
    }

    #[test]
    fn cost_reports_gap_when_table_count_not_collected() {
        let mut data = healthy_data();
        let obj = data.as_object_mut().unwrap();
        obj.remove("table_count");
        obj.remove("table_count_truncated");
        let r = fixture("db-tablegap", json!({"team": "data"}), data, now());
        let report = evaluate_glue_fleet(&[r], Pillar::Cost, now());
        assert_eq!(codes(&report), vec![REASON_COST_TABLE_COUNT_NOT_COLLECTED]);
    }

    #[test]
    fn security_flags_open_default_permissions_as_medium() {
        let mut data = healthy_data();
        data["default_permissions_grant_all_to_iam_allowed_principals"] = json!(true);
        data["create_table_default_permissions_count"] = json!(1);
        let r = fixture("db-open", json!({"team": "data"}), data, now());
        let report = evaluate_glue_fleet(&[r], Pillar::Security, now());
        assert_eq!(codes(&report), vec![REASON_SEC_DEFAULT_PERMISSIONS_OPEN]);
        assert!(matches!(report.findings[0].severity, Severity::Medium));
        assert!(report.findings[0]
            .message
            .contains("IAM_ALLOWED_PRINCIPALS"));
    }

    #[test]
    fn security_reports_gap_when_default_permissions_not_collected() {
        let mut data = healthy_data();
        data.as_object_mut()
            .unwrap()
            .remove("default_permissions_grant_all_to_iam_allowed_principals");
        let r = fixture("db-permgap", json!({"team": "data"}), data, now());
        let report = evaluate_glue_fleet(&[r], Pillar::Security, now());
        assert_eq!(
            codes(&report),
            vec![REASON_SEC_DEFAULT_PERMISSIONS_NOT_COLLECTED]
        );
    }

    #[test]
    fn security_flags_non_s3_location() {
        let mut data = healthy_data();
        data["location_uri"] = json!("hdfs://namenode:8020/warehouse");
        let r = fixture("db-hdfs", json!({"team": "data"}), data, now());
        let report = evaluate_glue_fleet(&[r], Pillar::Security, now());
        assert_eq!(codes(&report), vec![REASON_SEC_NON_S3_LOCATION]);
        assert!(matches!(report.findings[0].severity, Severity::Low));
    }

    #[test]
    fn security_does_not_flag_missing_location_or_resource_link_location() {
        // Missing location is a resilience concern, not a security one.
        let mut data = healthy_data();
        data.as_object_mut().unwrap().remove("location_uri");
        let r = fixture("db-noloc", json!({"team": "data"}), data, now());
        let report = evaluate_glue_fleet(&[r], Pillar::Security, now());
        assert!(
            report.findings.is_empty(),
            "unexpected: {:?}",
            report.findings
        );

        // A resource link's non-S3 location belongs to the target catalog.
        let mut link = healthy_data();
        link["is_resource_link"] = json!(true);
        link["location_uri"] = json!("hdfs://namenode:8020/warehouse");
        let r2 = fixture("db-link", json!({"team": "data"}), link, now());
        let report2 = evaluate_glue_fleet(&[r2], Pillar::Security, now());
        assert!(
            report2.findings.is_empty(),
            "unexpected: {:?}",
            report2.findings
        );
    }

    #[test]
    fn resilience_flags_missing_description() {
        let mut data = healthy_data();
        data.as_object_mut().unwrap().remove("description");
        let r = fixture("db-nodesc", json!({"team": "data"}), data, now());
        let report = evaluate_glue_fleet(&[r], Pillar::Resilience, now());
        assert_eq!(codes(&report), vec![REASON_RES_NO_DESCRIPTION]);

        let mut blank = healthy_data();
        blank["description"] = json!("   ");
        let r2 = fixture("db-blankdesc", json!({"team": "data"}), blank, now());
        let report2 = evaluate_glue_fleet(&[r2], Pillar::Resilience, now());
        assert_eq!(codes(&report2), vec![REASON_RES_NO_DESCRIPTION]);
    }

    #[test]
    fn resilience_flags_missing_location_only_for_storage_owning_databases() {
        let mut data = healthy_data();
        data.as_object_mut().unwrap().remove("location_uri");
        let r = fixture("db-noloc", json!({"team": "data"}), data, now());
        let report = evaluate_glue_fleet(&[r], Pillar::Resilience, now());
        assert_eq!(codes(&report), vec![REASON_RES_NO_LOCATION_URI]);

        let mut link = healthy_data();
        let obj = link.as_object_mut().unwrap();
        obj.remove("location_uri");
        obj.insert("is_resource_link".to_string(), json!(true));
        let r2 = fixture("db-link", json!({"team": "data"}), link, now());
        let report2 = evaluate_glue_fleet(&[r2], Pillar::Resilience, now());
        assert!(
            report2.findings.is_empty(),
            "unexpected: {:?}",
            report2.findings
        );

        let mut federated = healthy_data();
        let obj = federated.as_object_mut().unwrap();
        obj.remove("location_uri");
        obj.insert("is_federated".to_string(), json!(true));
        let r3 = fixture("db-fed", json!({"team": "data"}), federated, now());
        let report3 = evaluate_glue_fleet(&[r3], Pillar::Resilience, now());
        assert!(
            report3.findings.is_empty(),
            "unexpected: {:?}",
            report3.findings
        );
    }

    #[test]
    fn resilience_reports_gap_when_create_time_not_collected() {
        let mut data = healthy_data();
        data.as_object_mut().unwrap().remove("create_time");
        let r = fixture("db-notime", json!({"team": "data"}), data, now());
        let report = evaluate_glue_fleet(&[r], Pillar::Resilience, now());
        assert_eq!(codes(&report), vec![REASON_RES_CREATE_TIME_NOT_COLLECTED]);
    }

    #[test]
    fn stale_inventory_is_flagged() {
        let mut r = fixture("db-stale", json!({"team": "data"}), healthy_data(), now());
        r.last_refreshed = now() - Duration::hours(48);
        let report = evaluate_glue_fleet(&[r], Pillar::Security, now());
        assert_eq!(report.stale_resources, 1);
        assert!(codes(&report).contains(&REASON_INV_STALE_DATA));
    }

    #[test]
    fn non_glue_resources_are_skipped_and_not_counted() {
        let mut r = fixture("queue-1", json!({}), json!({}), now());
        r.resource_type = "SqsQueue".to_string();
        let report = evaluate_glue_fleet(&[r], Pillar::Cost, now());
        assert_eq!(report.resources_evaluated, 0);
        assert!(report.findings.is_empty());
    }

    #[test]
    fn healthy_database_passes_all_pillars() {
        let r = fixture("db-ok", json!({"team": "data"}), healthy_data(), now());
        for pillar in [Pillar::Cost, Pillar::Security, Pillar::Resilience] {
            let report = evaluate_glue_fleet(std::slice::from_ref(&r), pillar, now());
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

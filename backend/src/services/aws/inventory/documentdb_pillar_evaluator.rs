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

// Deterministic DocumentDB cluster inventory evaluators for the cost, resilience,
// and security pillars (roadmap rows 01-AWS-CLOUD-01387/01396/01423).
//
// Evaluates fields persisted by documentdb_control_plane: engine_version, status,
// deletion_protection, multi_az, backup_retention_period, storage_encrypted,
// kms_key_id, member_count, audit_logs_enabled, profiler_logs_enabled, plus tags.

use chrono::{DateTime, Utc};
use serde_json::{json, Value};

use crate::models::aws_resource::Model as AwsResourceModel;
use crate::services::aws::inventory::types::{
    check_stale, score_pillar, InventoryFinding, Pillar, PillarReport, Severity,
};

pub const RESOURCE_TYPE: &str = "DocumentDbCluster";

pub const REASON_COST_NO_TAGS: &str = "DOCDB_COST_NO_TAGS";
pub const REASON_COST_STOPPED: &str = "DOCDB_COST_STOPPED";
pub const REASON_RES_NO_MULTI_AZ: &str = "DOCDB_RES_NO_MULTI_AZ";
pub const REASON_RES_LOW_BACKUP_RETENTION: &str = "DOCDB_RES_LOW_BACKUP_RETENTION";
pub const REASON_RES_SINGLE_MEMBER: &str = "DOCDB_RES_SINGLE_MEMBER";
pub const REASON_RES_NO_DELETION_PROTECTION: &str = "DOCDB_RES_NO_DELETION_PROTECTION";
pub const REASON_SEC_NOT_ENCRYPTED: &str = "DOCDB_SEC_NOT_ENCRYPTED";
pub const REASON_SEC_NO_AUDIT_LOGS: &str = "DOCDB_SEC_NO_AUDIT_LOGS";
pub const REASON_INV_STALE_DATA: &str = "DOCDB_INV_STALE_DATA";

const MIN_BACKUP_RETENTION_DAYS: i64 = 7;

pub fn evaluate_documentdb_fleet(
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
            Pillar::Resilience => evaluate_resilience(resource, &mut findings),
            Pillar::Security => evaluate_security(resource, &mut findings),
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

fn data_bool(resource_data: &Value, key: &str) -> Option<bool> {
    resource_data.get(key).and_then(|v| v.as_bool())
}

fn data_i64(resource_data: &Value, key: &str) -> Option<i64> {
    resource_data.get(key).and_then(|v| v.as_i64())
}

fn data_str<'a>(resource_data: &'a Value, key: &str) -> Option<&'a str> {
    resource_data.get(key).and_then(|v| v.as_str())
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
                "DocumentDB cluster {} has no tags; cost allocation cannot be assessed",
                resource.resource_id
            ),
            evidence: json!({ "tags": resource.tags }),
        });
    }

    if let Some(status) = data_str(&resource.resource_data, "status") {
        if status == "stopped" {
            findings.push(InventoryFinding {
                resource_id: resource.resource_id.clone(),
                arn: resource.arn.clone(),
                pillar: Pillar::Cost,
                reason_code: REASON_COST_STOPPED.to_string(),
                severity: Severity::Medium,
                message: format!(
                    "DocumentDB cluster {} is stopped; storage costs accrue while stopped — delete if unused",
                    resource.resource_id
                ),
                evidence: json!({ "status": status }),
            });
        }
    }
}

fn evaluate_resilience(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    if !data_bool(&resource.resource_data, "multi_az").unwrap_or(true) {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Resilience,
            reason_code: REASON_RES_NO_MULTI_AZ.to_string(),
            severity: Severity::High,
            message: format!(
                "DocumentDB cluster {} is not Multi-AZ; a single-AZ cluster cannot fail over automatically during a zone outage",
                resource.resource_id
            ),
            evidence: json!({ "multi_az": false }),
        });
    }

    if let Some(retention) = data_i64(&resource.resource_data, "backup_retention_period") {
        if retention < MIN_BACKUP_RETENTION_DAYS {
            findings.push(InventoryFinding {
                resource_id: resource.resource_id.clone(),
                arn: resource.arn.clone(),
                pillar: Pillar::Resilience,
                reason_code: REASON_RES_LOW_BACKUP_RETENTION.to_string(),
                severity: Severity::Medium,
                message: format!(
                    "DocumentDB cluster {} backup retention is {} day(s); increase to at least {} days for meaningful recovery",
                    resource.resource_id, retention, MIN_BACKUP_RETENTION_DAYS
                ),
                evidence: json!({
                    "backup_retention_period": retention,
                    "minimum_recommended": MIN_BACKUP_RETENTION_DAYS,
                }),
            });
        }
    }

    if let Some(count) = data_i64(&resource.resource_data, "member_count") {
        if count < 2 {
            findings.push(InventoryFinding {
                resource_id: resource.resource_id.clone(),
                arn: resource.arn.clone(),
                pillar: Pillar::Resilience,
                reason_code: REASON_RES_SINGLE_MEMBER.to_string(),
                severity: Severity::Medium,
                message: format!(
                    "DocumentDB cluster {} has only {} member instance(s); add a replica for read scalability and faster failover",
                    resource.resource_id, count
                ),
                evidence: json!({ "member_count": count }),
            });
        }
    }

    if !data_bool(&resource.resource_data, "deletion_protection").unwrap_or(false) {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Resilience,
            reason_code: REASON_RES_NO_DELETION_PROTECTION.to_string(),
            severity: Severity::Medium,
            message: format!(
                "DocumentDB cluster {} does not have deletion protection enabled; accidental deletion risks data loss",
                resource.resource_id
            ),
            evidence: json!({ "deletion_protection": false }),
        });
    }
}

fn evaluate_security(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    if !data_bool(&resource.resource_data, "storage_encrypted").unwrap_or(false) {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Security,
            reason_code: REASON_SEC_NOT_ENCRYPTED.to_string(),
            severity: Severity::High,
            message: format!(
                "DocumentDB cluster {} storage is not encrypted at rest; enable encryption to protect data",
                resource.resource_id
            ),
            evidence: json!({ "storage_encrypted": false }),
        });
    }

    if !data_bool(&resource.resource_data, "audit_logs_enabled").unwrap_or(false) {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Security,
            reason_code: REASON_SEC_NO_AUDIT_LOGS.to_string(),
            severity: Severity::Medium,
            message: format!(
                "DocumentDB cluster {} does not have audit logging enabled in CloudWatch Logs; audit logs are required for access tracking and compliance",
                resource.resource_id
            ),
            evidence: json!({ "audit_logs_enabled": false }),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;
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
            arn: format!("arn:aws:rds:us-east-1:123456789012:cluster:{}", resource_id),
            name: Some(resource_id.to_string()),
            tags,
            resource_data,
            created_at: refreshed,
            updated_at: refreshed,
            last_refreshed: refreshed,
        }
    }

    fn now() -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2026-06-11T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc)
    }

    fn healthy_data() -> Value {
        json!({
            "engine": "docdb",
            "engine_version": "5.0.0",
            "status": "available",
            "deletion_protection": true,
            "multi_az": true,
            "backup_retention_period": 7,
            "storage_encrypted": true,
            "kms_key_id": "arn:aws:kms:us-east-1:123456789012:key/abc",
            "member_count": 2,
            "audit_logs_enabled": true,
            "profiler_logs_enabled": false,
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
        let r = fixture("my-docdb", json!({"team": "app"}), healthy_data(), now());
        for pillar in [Pillar::Cost, Pillar::Resilience, Pillar::Security] {
            let report = evaluate_documentdb_fleet(std::slice::from_ref(&r), pillar, now());
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
        let r = fixture("untagged", json!({}), healthy_data(), now());
        let report = evaluate_documentdb_fleet(&[r], Pillar::Cost, now());
        assert_eq!(codes(&report), vec![REASON_COST_NO_TAGS]);
    }

    #[test]
    fn cost_flags_stopped_cluster() {
        let mut data = healthy_data();
        data["status"] = json!("stopped");
        let r = fixture("stopped", json!({"team": "app"}), data, now());
        let report = evaluate_documentdb_fleet(&[r], Pillar::Cost, now());
        assert_eq!(codes(&report), vec![REASON_COST_STOPPED]);
    }

    #[test]
    fn resilience_flags_no_multi_az() {
        let mut data = healthy_data();
        data["multi_az"] = json!(false);
        let r = fixture("single-az", json!({"team": "app"}), data, now());
        let report = evaluate_documentdb_fleet(&[r], Pillar::Resilience, now());
        assert!(codes(&report).contains(&REASON_RES_NO_MULTI_AZ));
    }

    #[test]
    fn resilience_flags_low_backup_retention() {
        let mut data = healthy_data();
        data["backup_retention_period"] = json!(3);
        let r = fixture("low-backup", json!({"team": "app"}), data, now());
        let report = evaluate_documentdb_fleet(&[r], Pillar::Resilience, now());
        assert!(codes(&report).contains(&REASON_RES_LOW_BACKUP_RETENTION));
    }

    #[test]
    fn resilience_flags_single_member() {
        let mut data = healthy_data();
        data["member_count"] = json!(1);
        let r = fixture("single-member", json!({"team": "app"}), data, now());
        let report = evaluate_documentdb_fleet(&[r], Pillar::Resilience, now());
        assert!(codes(&report).contains(&REASON_RES_SINGLE_MEMBER));
    }

    #[test]
    fn resilience_flags_no_deletion_protection() {
        let mut data = healthy_data();
        data["deletion_protection"] = json!(false);
        let r = fixture("no-del-prot", json!({"team": "app"}), data, now());
        let report = evaluate_documentdb_fleet(&[r], Pillar::Resilience, now());
        assert!(codes(&report).contains(&REASON_RES_NO_DELETION_PROTECTION));
    }

    #[test]
    fn security_flags_unencrypted_storage() {
        let mut data = healthy_data();
        data["storage_encrypted"] = json!(false);
        let r = fixture("unencrypted", json!({"team": "app"}), data, now());
        let report = evaluate_documentdb_fleet(&[r], Pillar::Security, now());
        assert!(codes(&report).contains(&REASON_SEC_NOT_ENCRYPTED));
    }

    #[test]
    fn security_flags_no_audit_logs() {
        let mut data = healthy_data();
        data["audit_logs_enabled"] = json!(false);
        let r = fixture("no-audit", json!({"team": "app"}), data, now());
        let report = evaluate_documentdb_fleet(&[r], Pillar::Security, now());
        assert_eq!(codes(&report), vec![REASON_SEC_NO_AUDIT_LOGS]);
    }

    #[test]
    fn stale_resource_is_flagged() {
        let mut r = fixture("stale", json!({"team": "app"}), healthy_data(), now());
        r.last_refreshed = now() - Duration::hours(48);
        let report = evaluate_documentdb_fleet(&[r], Pillar::Resilience, now());
        assert_eq!(report.stale_resources, 1);
        assert!(codes(&report).contains(&REASON_INV_STALE_DATA));
    }

    #[test]
    fn non_docdb_resources_are_skipped() {
        let mut r = fixture("rds-1", json!({}), json!({}), now());
        r.resource_type = "RdsInstance".to_string();
        let report = evaluate_documentdb_fleet(&[r], Pillar::Cost, now());
        assert_eq!(report.resources_evaluated, 0);
    }
}

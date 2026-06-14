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

// Deterministic AWS Elastic Disaster Recovery inventory evaluators for the
// cost, resilience, and security pillars.

use chrono::{DateTime, Utc};
use serde_json::{json, Value};

use crate::models::aws_resource::Model as AwsResourceModel;
use crate::services::aws::inventory::types::{
    check_stale, has_any_tag, score_pillar, InventoryFinding, Pillar, PillarReport, Severity,
    COST_ALLOCATION_TAG_KEYS,
};

pub const RESOURCE_TYPE: &str = "DrsResource";

pub const REASON_COST_NO_TAGS: &str = "DRS_COST_NO_TAGS";
pub const REASON_COST_ACTIVE_SOURCE_WITHOUT_RECOVERY_INSTANCE: &str =
    "DRS_COST_ACTIVE_SOURCE_WITHOUT_RECOVERY_INSTANCE";
pub const REASON_RES_COLLECTION_ERRORS: &str = "DRS_RES_COLLECTION_ERRORS";
pub const REASON_RES_REPLICATION_PROBLEM: &str = "DRS_RES_REPLICATION_PROBLEM";
pub const REASON_RES_REPLICATION_BACKLOG: &str = "DRS_RES_REPLICATION_BACKLOG";
pub const REASON_RES_SOURCE_NETWORK_REPLICATION_ERROR: &str =
    "DRS_RES_SOURCE_NETWORK_REPLICATION_ERROR";
pub const REASON_RES_FAILED_LAUNCH: &str = "DRS_RES_FAILED_LAUNCH";
pub const REASON_RES_JOB_INCOMPLETE: &str = "DRS_RES_JOB_INCOMPLETE";
pub const REASON_SEC_PUBLIC_REPLICATION_PATH: &str = "DRS_SEC_PUBLIC_REPLICATION_PATH";
pub const REASON_SEC_DEFAULT_EBS_ENCRYPTION: &str = "DRS_SEC_DEFAULT_EBS_ENCRYPTION";
pub const REASON_INV_STALE_DATA: &str = "DRS_INV_STALE_DATA";

pub fn evaluate_drs_fleet(
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

    PillarReport {
        pillar,
        resources_evaluated: evaluated,
        stale_resources,
        score: score_pillar(&findings),
        findings,
    }
}

fn data_str<'a>(resource_data: &'a Value, key: &str) -> Option<&'a str> {
    resource_data.get(key).and_then(|v| v.as_str())
}

fn data_bool(resource_data: &Value, key: &str) -> Option<bool> {
    resource_data.get(key).and_then(|v| v.as_bool())
}

fn data_usize(resource_data: &Value, key: &str) -> usize {
    resource_data
        .get(key)
        .and_then(|v| v.as_u64())
        .map(|v| v as usize)
        .unwrap_or(0)
}

fn resource_kind(resource: &AwsResourceModel) -> Option<&str> {
    data_str(&resource.resource_data, "resource_kind")
}

fn finding(
    resource: &AwsResourceModel,
    pillar: Pillar,
    reason_code: &str,
    severity: Severity,
    message: String,
    evidence: Value,
) -> InventoryFinding {
    InventoryFinding {
        resource_id: resource.resource_id.clone(),
        arn: resource.arn.clone(),
        pillar,
        reason_code: reason_code.to_string(),
        severity,
        message,
        evidence,
    }
}

fn evaluate_cost(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    if resource_kind(resource) != Some("account_summary")
        && !has_any_tag(&resource.tags, COST_ALLOCATION_TAG_KEYS)
    {
        findings.push(finding(
            resource,
            Pillar::Cost,
            REASON_COST_NO_TAGS,
            Severity::Medium,
            format!(
                "Elastic Disaster Recovery resource {} has no cost allocation tags",
                resource.resource_id
            ),
            json!({ "tags": resource.tags, "resource_kind": resource_kind(resource) }),
        ));
    }

    if resource_kind(resource) == Some("source_server")
        && data_str(&resource.resource_data, "recovery_instance_id").is_none()
        && data_str(&resource.resource_data, "data_replication_state")
            .map(is_active_replication_state)
            .unwrap_or(false)
    {
        findings.push(finding(
            resource,
            Pillar::Cost,
            REASON_COST_ACTIVE_SOURCE_WITHOUT_RECOVERY_INSTANCE,
            Severity::Medium,
            format!(
                "DRS source server {} is actively replicating without recovery instance evidence",
                resource.resource_id
            ),
            json!({
                "data_replication_state": resource.resource_data.get("data_replication_state"),
                "recovery_instance_id": resource.resource_data.get("recovery_instance_id"),
                "lag_duration": resource.resource_data.get("lag_duration"),
            }),
        ));
    }
}

fn evaluate_resilience(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    if resource_kind(resource) == Some("account_summary") {
        let collection_errors = data_usize(&resource.resource_data, "collection_error_count");
        if collection_errors > 0 {
            findings.push(finding(
                resource,
                Pillar::Resilience,
                REASON_RES_COLLECTION_ERRORS,
                Severity::Medium,
                format!(
                    "Elastic Disaster Recovery account {} had {} collection errors",
                    resource.resource_id, collection_errors
                ),
                json!({
                    "collection_error_count": collection_errors,
                    "collection_errors": resource.resource_data.get("collection_errors"),
                }),
            ));
        }

        let source_network_errors =
            data_usize(&resource.resource_data, "source_network_error_count");
        if source_network_errors > 0 {
            findings.push(finding(
                resource,
                Pillar::Resilience,
                REASON_RES_SOURCE_NETWORK_REPLICATION_ERROR,
                Severity::High,
                format!(
                    "Elastic Disaster Recovery account {} has {} source network replication errors",
                    resource.resource_id, source_network_errors
                ),
                json!({
                    "source_network_error_count": source_network_errors,
                    "resources_by_kind": resource.resource_data.get("resources_by_kind"),
                }),
            ));
        }

        let failed_launches = data_usize(&resource.resource_data, "failed_launch_count");
        if failed_launches > 0 {
            findings.push(finding(
                resource,
                Pillar::Resilience,
                REASON_RES_FAILED_LAUNCH,
                Severity::High,
                format!(
                    "Elastic Disaster Recovery account {} has {} failed launch results",
                    resource.resource_id, failed_launches
                ),
                json!({ "failed_launch_count": failed_launches }),
            ));
        }

        let incomplete_jobs = data_usize(&resource.resource_data, "failed_or_incomplete_job_count");
        if incomplete_jobs > 0 {
            findings.push(finding(
                resource,
                Pillar::Resilience,
                REASON_RES_JOB_INCOMPLETE,
                Severity::Medium,
                format!(
                    "Elastic Disaster Recovery account {} has {} failed or incomplete jobs",
                    resource.resource_id, incomplete_jobs
                ),
                json!({ "failed_or_incomplete_job_count": incomplete_jobs }),
            ));
        }
    }

    if matches!(
        resource_kind(resource),
        Some("source_server") | Some("recovery_instance")
    ) {
        let replication_state = data_str(&resource.resource_data, "data_replication_state");
        let has_problem_state = replication_state
            .map(is_problem_replication_state)
            .unwrap_or(false);
        let has_error = data_str(&resource.resource_data, "data_replication_error").is_some();
        if has_problem_state || has_error {
            findings.push(finding(
                resource,
                Pillar::Resilience,
                REASON_RES_REPLICATION_PROBLEM,
                Severity::High,
                format!(
                    "DRS resource {} has replication problem evidence",
                    resource.resource_id
                ),
                json!({
                    "data_replication_state": replication_state,
                    "data_replication_error": resource.resource_data.get("data_replication_error"),
                    "data_replication_raw_error": resource.resource_data.get("data_replication_raw_error"),
                }),
            ));
        }

        if replication_state == Some("BACKLOG") {
            findings.push(finding(
                resource,
                Pillar::Resilience,
                REASON_RES_REPLICATION_BACKLOG,
                Severity::Medium,
                format!(
                    "DRS resource {} is accumulating replication backlog",
                    resource.resource_id
                ),
                json!({
                    "data_replication_state": replication_state,
                    "lag_duration": resource.resource_data.get("lag_duration"),
                }),
            ));
        }
    }

    if resource_kind(resource) == Some("source_network")
        && data_str(&resource.resource_data, "replication_status") == Some("ERROR")
    {
        findings.push(finding(
            resource,
            Pillar::Resilience,
            REASON_RES_SOURCE_NETWORK_REPLICATION_ERROR,
            Severity::High,
            format!(
                "DRS source network {} has replication error evidence",
                resource.resource_id
            ),
            json!({
                "replication_status": resource.resource_data.get("replication_status"),
                "replication_status_details": resource.resource_data.get("replication_status_details"),
            }),
        ));
    }

    if resource_kind(resource) == Some("source_server")
        && data_str(&resource.resource_data, "last_launch_result") == Some("FAILED")
    {
        findings.push(finding(
            resource,
            Pillar::Resilience,
            REASON_RES_FAILED_LAUNCH,
            Severity::High,
            format!(
                "DRS source server {} last launch failed",
                resource.resource_id
            ),
            json!({ "last_launch_result": resource.resource_data.get("last_launch_result") }),
        ));
    }

    if resource_kind(resource) == Some("job")
        && data_str(&resource.resource_data, "status")
            .map(|status| status != "COMPLETED")
            .unwrap_or(false)
    {
        findings.push(finding(
            resource,
            Pillar::Resilience,
            REASON_RES_JOB_INCOMPLETE,
            Severity::Medium,
            format!("DRS job {} did not complete cleanly", resource.resource_id),
            json!({
                "status": resource.resource_data.get("status"),
                "job_type": resource.resource_data.get("job_type"),
                "participating_server_count": resource.resource_data.get("participating_server_count"),
            }),
        ));
    }
}

fn evaluate_security(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    if resource_kind(resource) == Some("replication_configuration_template")
        && (data_bool(&resource.resource_data, "create_public_ip") == Some(true)
            || data_str(&resource.resource_data, "data_plane_routing") == Some("PUBLIC_IP"))
    {
        findings.push(finding(
            resource,
            Pillar::Security,
            REASON_SEC_PUBLIC_REPLICATION_PATH,
            Severity::High,
            format!(
                "DRS replication template {} allows public replication networking",
                resource.resource_id
            ),
            json!({
                "create_public_ip": resource.resource_data.get("create_public_ip"),
                "data_plane_routing": resource.resource_data.get("data_plane_routing"),
            }),
        ));
    }

    if resource_kind(resource) == Some("replication_configuration_template")
        && (data_str(&resource.resource_data, "ebs_encryption") == Some("DEFAULT")
            || data_str(&resource.resource_data, "ebs_encryption_key_arn").is_none())
    {
        findings.push(finding(
            resource,
            Pillar::Security,
            REASON_SEC_DEFAULT_EBS_ENCRYPTION,
            Severity::Medium,
            format!(
                "DRS replication template {} has no customer-managed EBS encryption key evidence",
                resource.resource_id
            ),
            json!({
                "ebs_encryption": resource.resource_data.get("ebs_encryption"),
                "ebs_encryption_key_arn": resource.resource_data.get("ebs_encryption_key_arn"),
            }),
        ));
    }
}

fn is_active_replication_state(state: &str) -> bool {
    matches!(
        state,
        "BACKLOG" | "CONTINUOUS" | "CREATING_SNAPSHOT" | "INITIAL_SYNC" | "INITIATING" | "RESCAN"
    )
}

fn is_problem_replication_state(state: &str) -> bool {
    matches!(state, "DISCONNECTED" | "PAUSED" | "STALLED" | "STOPPED")
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;
    use serde_json::json;
    use uuid::Uuid;

    fn fixture(resource_id: &str, resource_data: Value, tags: Value) -> AwsResourceModel {
        let now = Utc::now();
        AwsResourceModel {
            id: Uuid::new_v4(),
            sync_id: Some(Uuid::new_v4()),
            account_id: "123456789012".to_string(),
            profile: Some("test".to_string()),
            region: "us-east-1".to_string(),
            resource_type: RESOURCE_TYPE.to_string(),
            resource_id: resource_id.to_string(),
            arn: format!("arn:aws:drs:us-east-1:123456789012:{}", resource_id),
            name: Some(resource_id.to_string()),
            tags,
            resource_data,
            created_at: now,
            updated_at: now,
            last_refreshed: now - Duration::hours(26),
        }
    }

    #[test]
    fn evaluates_drs_inventory_findings() {
        let now = Utc::now();
        let resources = vec![
            fixture(
                "drs:123456789012",
                json!({
                    "resource_kind": "account_summary",
                    "resource_count": 4,
                    "collection_error_count": 1,
                    "collection_errors": [{ "operation": "DescribeJobs", "error": "denied" }],
                    "source_network_error_count": 1,
                    "failed_launch_count": 1,
                    "failed_or_incomplete_job_count": 1,
                }),
                json!({}),
            ),
            fixture(
                "source_server/s-1",
                json!({
                    "resource_kind": "source_server",
                    "data_replication_state": "BACKLOG",
                    "lag_duration": "PT2H",
                    "last_launch_result": "FAILED",
                }),
                json!({}),
            ),
            fixture(
                "recovery_instance/ri-1",
                json!({
                    "resource_kind": "recovery_instance",
                    "data_replication_state": "STALLED",
                    "data_replication_error": "FAILBACK_CLIENT_NOT_SEEN",
                }),
                json!({ "CostCenter": "dr" }),
            ),
            fixture(
                "source_network/sn-1",
                json!({
                    "resource_kind": "source_network",
                    "replication_status": "ERROR",
                    "replication_status_details": "route mismatch",
                }),
                json!({ "CostCenter": "dr" }),
            ),
            fixture(
                "job/job-1",
                json!({
                    "resource_kind": "job",
                    "status": "FAILED",
                    "job_type": "RECOVERY",
                    "participating_server_count": 2,
                }),
                json!({ "CostCenter": "dr" }),
            ),
            fixture(
                "replication_configuration_template/rct-1",
                json!({
                    "resource_kind": "replication_configuration_template",
                    "create_public_ip": true,
                    "data_plane_routing": "PUBLIC_IP",
                    "ebs_encryption": "DEFAULT",
                }),
                json!({ "CostCenter": "dr" }),
            ),
        ];

        let cost = evaluate_drs_fleet(&resources, Pillar::Cost, now);
        assert_eq!(cost.resources_evaluated, 6);
        assert_eq!(cost.stale_resources, 6);
        assert!(cost
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_COST_NO_TAGS));
        assert!(cost.findings.iter().any(|finding| {
            finding.reason_code == REASON_COST_ACTIVE_SOURCE_WITHOUT_RECOVERY_INSTANCE
        }));

        let resilience = evaluate_drs_fleet(&resources, Pillar::Resilience, now);
        assert!(resilience
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_RES_COLLECTION_ERRORS));
        assert!(resilience
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_RES_REPLICATION_PROBLEM));
        assert!(resilience
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_RES_REPLICATION_BACKLOG));
        assert!(resilience
            .findings
            .iter()
            .any(|finding| { finding.reason_code == REASON_RES_SOURCE_NETWORK_REPLICATION_ERROR }));
        assert!(resilience
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_RES_FAILED_LAUNCH));
        assert!(resilience
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_RES_JOB_INCOMPLETE));

        let security = evaluate_drs_fleet(&resources, Pillar::Security, now);
        assert!(security
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_SEC_PUBLIC_REPLICATION_PATH));
        assert!(security
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_SEC_DEFAULT_EBS_ENCRYPTION));
    }
}

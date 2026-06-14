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

// Deterministic AWS Application Migration Service inventory evaluators for
// the cost, resilience, and security pillars.

use chrono::{DateTime, Utc};
use serde_json::{json, Value};

use crate::models::aws_resource::Model as AwsResourceModel;
use crate::services::aws::inventory::types::{
    check_stale, has_any_tag, score_pillar, InventoryFinding, Pillar, PillarReport, Severity,
    COST_ALLOCATION_TAG_KEYS,
};

pub const RESOURCE_TYPE: &str = "MgnResource";

pub const REASON_COST_NO_TAGS: &str = "MGN_COST_NO_TAGS";
pub const REASON_COST_UNASSIGNED_ACTIVE_SOURCE: &str = "MGN_COST_UNASSIGNED_ACTIVE_SOURCE";
pub const REASON_RES_COLLECTION_ERRORS: &str = "MGN_RES_COLLECTION_ERRORS";
pub const REASON_RES_REPLICATION_PROBLEM: &str = "MGN_RES_REPLICATION_PROBLEM";
pub const REASON_RES_REPLICATION_BACKLOG: &str = "MGN_RES_REPLICATION_BACKLOG";
pub const REASON_RES_LIFECYCLE_NOT_READY: &str = "MGN_RES_LIFECYCLE_NOT_READY";
pub const REASON_SEC_PUBLIC_REPLICATION_PATH: &str = "MGN_SEC_PUBLIC_REPLICATION_PATH";
pub const REASON_SEC_DEFAULT_EBS_ENCRYPTION: &str = "MGN_SEC_DEFAULT_EBS_ENCRYPTION";
pub const REASON_SEC_PUBLIC_LAUNCH_IP: &str = "MGN_SEC_PUBLIC_LAUNCH_IP";
pub const REASON_SEC_PARAMETERS_ENCRYPTION_DISABLED: &str =
    "MGN_SEC_PARAMETERS_ENCRYPTION_DISABLED";
pub const REASON_INV_STALE_DATA: &str = "MGN_INV_STALE_DATA";

pub fn evaluate_mgn_fleet(
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
                "Application Migration Service resource {} has no cost allocation tags",
                resource.resource_id
            ),
            json!({ "tags": resource.tags, "resource_kind": resource_kind(resource) }),
        ));
    }

    if resource_kind(resource) == Some("source_server")
        && data_str(&resource.resource_data, "application_id").is_none()
        && data_str(&resource.resource_data, "data_replication_state")
            .map(is_active_replication_state)
            .unwrap_or(false)
    {
        findings.push(finding(
            resource,
            Pillar::Cost,
            REASON_COST_UNASSIGNED_ACTIVE_SOURCE,
            Severity::Medium,
            format!(
                "MGN source server {} is actively replicating without application ownership evidence",
                resource.resource_id
            ),
            json!({
                "data_replication_state": resource.resource_data.get("data_replication_state"),
                "application_id": resource.resource_data.get("application_id"),
                "replicator_id": resource.resource_data.get("replicator_id"),
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
                    "Application Migration Service account {} had {} collection errors",
                    resource.resource_id, collection_errors
                ),
                json!({
                    "collection_error_count": collection_errors,
                    "collection_errors": resource.resource_data.get("collection_errors"),
                }),
            ));
        }
    }

    if resource_kind(resource) == Some("source_server") {
        if data_str(&resource.resource_data, "data_replication_state")
            .map(is_problem_replication_state)
            .unwrap_or(false)
            || data_str(&resource.resource_data, "data_replication_error").is_some()
        {
            findings.push(finding(
                resource,
                Pillar::Resilience,
                REASON_RES_REPLICATION_PROBLEM,
                Severity::High,
                format!(
                    "MGN source server {} has replication problem evidence",
                    resource.resource_id
                ),
                json!({
                    "data_replication_state": resource.resource_data.get("data_replication_state"),
                    "data_replication_error": resource.resource_data.get("data_replication_error"),
                    "data_replication_raw_error": resource.resource_data.get("data_replication_raw_error"),
                }),
            ));
        }

        if data_str(&resource.resource_data, "data_replication_state") == Some("BACKLOG") {
            findings.push(finding(
                resource,
                Pillar::Resilience,
                REASON_RES_REPLICATION_BACKLOG,
                Severity::Medium,
                format!(
                    "MGN source server {} is accumulating replication backlog",
                    resource.resource_id
                ),
                json!({
                    "data_replication_state": resource.resource_data.get("data_replication_state"),
                    "lag_duration": resource.resource_data.get("lag_duration"),
                }),
            ));
        }

        if data_str(&resource.resource_data, "life_cycle_state")
            .map(is_lifecycle_not_ready)
            .unwrap_or(false)
        {
            findings.push(finding(
                resource,
                Pillar::Resilience,
                REASON_RES_LIFECYCLE_NOT_READY,
                Severity::Medium,
                format!(
                    "MGN source server {} is not ready for migration workflow",
                    resource.resource_id
                ),
                json!({
                    "life_cycle_state": resource.resource_data.get("life_cycle_state"),
                    "last_seen_by_service_date_time": resource.resource_data.get("last_seen_by_service_date_time"),
                }),
            ));
        }
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
                "MGN replication template {} allows public replication networking",
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
                "MGN replication template {} has no customer-managed EBS encryption key evidence",
                resource.resource_id
            ),
            json!({
                "ebs_encryption": resource.resource_data.get("ebs_encryption"),
                "ebs_encryption_key_arn": resource.resource_data.get("ebs_encryption_key_arn"),
            }),
        ));
    }

    if resource_kind(resource) == Some("launch_configuration_template")
        && data_bool(&resource.resource_data, "associate_public_ip_address") == Some(true)
    {
        findings.push(finding(
            resource,
            Pillar::Security,
            REASON_SEC_PUBLIC_LAUNCH_IP,
            Severity::High,
            format!(
                "MGN launch template {} associates public IP addresses",
                resource.resource_id
            ),
            json!({
                "associate_public_ip_address": true,
                "launch_disposition": resource.resource_data.get("launch_disposition"),
            }),
        ));
    }

    if resource_kind(resource) == Some("launch_configuration_template")
        && data_bool(&resource.resource_data, "enable_parameters_encryption") == Some(false)
    {
        findings.push(finding(
            resource,
            Pillar::Security,
            REASON_SEC_PARAMETERS_ENCRYPTION_DISABLED,
            Severity::Medium,
            format!(
                "MGN launch template {} has parameter encryption disabled",
                resource.resource_id
            ),
            json!({
                "enable_parameters_encryption": false,
                "parameters_encryption_key": resource.resource_data.get("parameters_encryption_key"),
            }),
        ));
    }
}

fn is_active_replication_state(state: &str) -> bool {
    matches!(
        state,
        "BACKLOG"
            | "CONTINUOUS"
            | "CREATING_SNAPSHOT"
            | "INITIAL_SYNC"
            | "PENDING_SNAPSHOT_SHIPPING"
            | "RESCAN"
            | "SHIPPING_SNAPSHOT"
    )
}

fn is_problem_replication_state(state: &str) -> bool {
    matches!(state, "DISCONNECTED" | "PAUSED" | "STALLED" | "STOPPED")
}

fn is_lifecycle_not_ready(state: &str) -> bool {
    matches!(
        state,
        "DISCONNECTED" | "NOT_READY" | "PENDING_INSTALLATION" | "STOPPED"
    )
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
            arn: format!("arn:aws:mgn:us-east-1:123456789012:{}", resource_id),
            name: Some(resource_id.to_string()),
            tags,
            resource_data,
            created_at: now,
            updated_at: now,
            last_refreshed: now - Duration::hours(26),
        }
    }

    #[test]
    fn evaluates_mgn_inventory_findings() {
        let now = Utc::now();
        let resources = vec![
            fixture(
                "mgn:123456789012",
                json!({
                    "resource_kind": "account_summary",
                    "resource_count": 4,
                    "collection_error_count": 1,
                    "collection_errors": [{ "operation": "ListConnectors", "error": "denied" }],
                    "resources_by_kind": { "source_server": 2 },
                }),
                json!({}),
            ),
            fixture(
                "source_server/s-1",
                json!({
                    "resource_kind": "source_server",
                    "data_replication_state": "BACKLOG",
                    "lag_duration": "PT2H",
                    "life_cycle_state": "NOT_READY",
                    "replicator_id": "i-123",
                }),
                json!({}),
            ),
            fixture(
                "source_server/s-2",
                json!({
                    "resource_kind": "source_server",
                    "data_replication_state": "STALLED",
                    "data_replication_error": "AGENT_NOT_SEEN",
                    "data_replication_raw_error": "agent heartbeat missing",
                    "application_id": "app-1",
                }),
                json!({ "CostCenter": "migration" }),
            ),
            fixture(
                "replication_configuration_template/rct-1",
                json!({
                    "resource_kind": "replication_configuration_template",
                    "create_public_ip": true,
                    "data_plane_routing": "PUBLIC_IP",
                    "ebs_encryption": "DEFAULT",
                }),
                json!({ "CostCenter": "migration" }),
            ),
            fixture(
                "launch_configuration_template/lct-1",
                json!({
                    "resource_kind": "launch_configuration_template",
                    "associate_public_ip_address": true,
                    "enable_parameters_encryption": false,
                    "launch_disposition": "STARTED",
                }),
                json!({ "CostCenter": "migration" }),
            ),
        ];

        let cost = evaluate_mgn_fleet(&resources, Pillar::Cost, now);
        assert_eq!(cost.resources_evaluated, 5);
        assert_eq!(cost.stale_resources, 5);
        assert!(cost
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_COST_NO_TAGS));
        assert!(cost
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_COST_UNASSIGNED_ACTIVE_SOURCE));

        let resilience = evaluate_mgn_fleet(&resources, Pillar::Resilience, now);
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
            .any(|finding| finding.reason_code == REASON_RES_LIFECYCLE_NOT_READY));

        let security = evaluate_mgn_fleet(&resources, Pillar::Security, now);
        assert!(security
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_SEC_PUBLIC_REPLICATION_PATH));
        assert!(security
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_SEC_DEFAULT_EBS_ENCRYPTION));
        assert!(security
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_SEC_PUBLIC_LAUNCH_IP));
        assert!(security
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_SEC_PARAMETERS_ENCRYPTION_DISABLED));
    }
}

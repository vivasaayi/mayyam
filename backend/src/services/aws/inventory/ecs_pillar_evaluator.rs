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

// Deterministic ECS inventory evaluators for the cost, security, and
// resilience pillars (roadmap rows 01-AWS-CLOUD-00190/00199/00226).
//
// Evaluates both EcsCluster and EcsService rows persisted by
// ecs_control_plane (PascalCase keys: Status, RunningTasksCount,
// DesiredCount, RunningCount, ...). The collector stores empty tags for
// ECS, so tag posture is reported as an explicit data gap.

use chrono::{DateTime, Utc};
use serde_json::json;

use crate::models::aws_resource::Model as AwsResourceModel;
use crate::services::aws::inventory::types::{
    check_stale, data_str, score_pillar, InventoryFinding, Pillar, PillarReport, Severity,
};

// Reason codes are the stable contract for findings; never reuse or rename.
pub const REASON_COST_TAG_DATA_NOT_COLLECTED: &str = "ECS_COST_TAG_DATA_NOT_COLLECTED";
pub const REASON_COST_IDLE_CLUSTER: &str = "ECS_COST_IDLE_CLUSTER";
pub const REASON_SEC_POSTURE_DATA_NOT_COLLECTED: &str = "ECS_SEC_POSTURE_DATA_NOT_COLLECTED";
pub const REASON_RES_SERVICE_BELOW_DESIRED: &str = "ECS_RES_SERVICE_BELOW_DESIRED";
pub const REASON_RES_SINGLE_TASK_SERVICE: &str = "ECS_RES_SINGLE_TASK_SERVICE";
pub const REASON_RES_CLUSTER_NOT_ACTIVE: &str = "ECS_RES_CLUSTER_NOT_ACTIVE";
pub const REASON_INV_STALE_DATA: &str = "ECS_INV_STALE_DATA";

fn is_cluster(resource: &AwsResourceModel) -> bool {
    resource.resource_type == "EcsCluster"
}

fn data_i64(resource: &AwsResourceModel, key: &str) -> Option<i64> {
    resource.resource_data.get(key).and_then(|v| v.as_i64())
}

/// Evaluate every ECS cluster and service in the fleet for one pillar.
pub fn evaluate_ecs_fleet(
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
            Pillar::Security => evaluate_security(resource, &mut findings),
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

fn evaluate_cost(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    if tags_missing(resource) {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Cost,
            reason_code: REASON_COST_TAG_DATA_NOT_COLLECTED.to_string(),
            severity: Severity::Medium,
            message: format!(
                "Tags for {} {} are not collected yet; cost allocation cannot be assessed",
                resource.resource_type, resource.resource_id
            ),
            evidence: json!({ "tags": resource.tags }),
        });
    }

    if is_cluster(resource) {
        let instances = data_i64(resource, "RegisteredContainerInstancesCount").unwrap_or(0);
        let running = data_i64(resource, "RunningTasksCount").unwrap_or(0);
        let services = data_i64(resource, "ActiveServicesCount").unwrap_or(0);
        if instances == 0 && running == 0 && services == 0 {
            findings.push(InventoryFinding {
                resource_id: resource.resource_id.clone(),
                arn: resource.arn.clone(),
                pillar: Pillar::Cost,
                reason_code: REASON_COST_IDLE_CLUSTER.to_string(),
                severity: Severity::Medium,
                message: format!(
                    "Cluster {} has no container instances, tasks, or services; it appears abandoned",
                    resource.resource_id
                ),
                evidence: json!({
                    "RegisteredContainerInstancesCount": instances,
                    "RunningTasksCount": running,
                    "ActiveServicesCount": services,
                }),
            });
        }
    }
}

fn evaluate_security(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    // The collector gathers no IAM/network posture for ECS yet; report the
    // gap once per resource rather than scoring blind.
    findings.push(InventoryFinding {
        resource_id: resource.resource_id.clone(),
        arn: resource.arn.clone(),
        pillar: Pillar::Security,
        reason_code: REASON_SEC_POSTURE_DATA_NOT_COLLECTED.to_string(),
        severity: Severity::Medium,
        message: format!(
            "Security posture (task role, network mode, exec config) for {} {} is not collected yet",
            resource.resource_type, resource.resource_id
        ),
        evidence: json!({
            "collected_fields": resource
                .resource_data
                .as_object()
                .map(|m| m.keys().cloned().collect::<Vec<_>>()),
        }),
    });
}

fn evaluate_resilience(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    if is_cluster(resource) {
        if let Some(status) = data_str(&resource.resource_data, "Status") {
            if status != "ACTIVE" {
                findings.push(InventoryFinding {
                    resource_id: resource.resource_id.clone(),
                    arn: resource.arn.clone(),
                    pillar: Pillar::Resilience,
                    reason_code: REASON_RES_CLUSTER_NOT_ACTIVE.to_string(),
                    severity: Severity::Medium,
                    message: format!("Cluster {} is in status '{}'", resource.resource_id, status),
                    evidence: json!({ "Status": status }),
                });
            }
        }
        return;
    }

    // Service-level checks.
    let desired = data_i64(resource, "DesiredCount");
    let running = data_i64(resource, "RunningCount");
    if let (Some(desired), Some(running)) = (desired, running) {
        if running < desired {
            findings.push(InventoryFinding {
                resource_id: resource.resource_id.clone(),
                arn: resource.arn.clone(),
                pillar: Pillar::Resilience,
                reason_code: REASON_RES_SERVICE_BELOW_DESIRED.to_string(),
                severity: Severity::High,
                message: format!(
                    "Service {} runs {} of {} desired tasks; capacity is degraded",
                    resource.resource_id, running, desired
                ),
                evidence: json!({ "DesiredCount": desired, "RunningCount": running }),
            });
        }
        if desired == 1 {
            findings.push(InventoryFinding {
                resource_id: resource.resource_id.clone(),
                arn: resource.arn.clone(),
                pillar: Pillar::Resilience,
                reason_code: REASON_RES_SINGLE_TASK_SERVICE.to_string(),
                severity: Severity::Medium,
                message: format!(
                    "Service {} has a desired count of 1; a single task failure causes downtime",
                    resource.resource_id
                ),
                evidence: json!({ "DesiredCount": desired }),
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
            resource_type: resource_type.to_string(),
            resource_id: resource_id.to_string(),
            arn: format!("arn:aws:ecs:us-east-1:123456789012:{}", resource_id),
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

    #[test]
    fn cost_flags_idle_cluster_and_tag_gap() {
        let r = fixture(
            "EcsCluster",
            "empty-cluster",
            json!({}),
            json!({"Status": "ACTIVE", "RegisteredContainerInstancesCount": 0, "RunningTasksCount": 0, "ActiveServicesCount": 0}),
            now(),
        );
        let report = evaluate_ecs_fleet(&[r], Pillar::Cost, now());
        let codes: Vec<&str> = report
            .findings
            .iter()
            .map(|f| f.reason_code.as_str())
            .collect();
        assert!(codes.contains(&REASON_COST_IDLE_CLUSTER));
        assert!(codes.contains(&REASON_COST_TAG_DATA_NOT_COLLECTED));
    }

    #[test]
    fn cost_passes_for_busy_tagged_cluster() {
        let r = fixture(
            "EcsCluster",
            "busy-cluster",
            json!({"team": "platform"}),
            json!({"Status": "ACTIVE", "RegisteredContainerInstancesCount": 3, "RunningTasksCount": 10, "ActiveServicesCount": 2}),
            now(),
        );
        let report = evaluate_ecs_fleet(&[r], Pillar::Cost, now());
        assert!(
            report.findings.is_empty(),
            "unexpected: {:?}",
            report.findings
        );
    }

    #[test]
    fn security_reports_posture_data_gap_per_resource() {
        let r = fixture(
            "EcsService",
            "svc-a",
            json!({"team": "platform"}),
            json!({"Status": "ACTIVE", "DesiredCount": 2, "RunningCount": 2}),
            now(),
        );
        let report = evaluate_ecs_fleet(&[r], Pillar::Security, now());
        assert_eq!(
            report
                .findings
                .iter()
                .map(|f| f.reason_code.as_str())
                .collect::<Vec<_>>(),
            vec![REASON_SEC_POSTURE_DATA_NOT_COLLECTED]
        );
    }

    #[test]
    fn resilience_flags_degraded_and_single_task_services() {
        let degraded = fixture(
            "EcsService",
            "svc-degraded",
            json!({"team": "platform"}),
            json!({"Status": "ACTIVE", "DesiredCount": 3, "RunningCount": 1}),
            now(),
        );
        let single = fixture(
            "EcsService",
            "svc-single",
            json!({"team": "platform"}),
            json!({"Status": "ACTIVE", "DesiredCount": 1, "RunningCount": 1}),
            now(),
        );
        let report = evaluate_ecs_fleet(&[degraded, single], Pillar::Resilience, now());
        let codes: Vec<&str> = report
            .findings
            .iter()
            .map(|f| f.reason_code.as_str())
            .collect();
        assert!(codes.contains(&REASON_RES_SERVICE_BELOW_DESIRED));
        assert!(codes.contains(&REASON_RES_SINGLE_TASK_SERVICE));
        let below = report
            .findings
            .iter()
            .find(|f| f.reason_code == REASON_RES_SERVICE_BELOW_DESIRED)
            .unwrap();
        assert_eq!(below.severity, Severity::High);
    }

    #[test]
    fn resilience_flags_inactive_cluster_and_passes_healthy_service() {
        let cluster = fixture(
            "EcsCluster",
            "draining",
            json!({"team": "platform"}),
            json!({"Status": "DEPROVISIONING", "RegisteredContainerInstancesCount": 1, "RunningTasksCount": 1, "ActiveServicesCount": 1}),
            now(),
        );
        let healthy = fixture(
            "EcsService",
            "svc-ok",
            json!({"team": "platform"}),
            json!({"Status": "ACTIVE", "DesiredCount": 2, "RunningCount": 2}),
            now(),
        );
        let report = evaluate_ecs_fleet(&[cluster, healthy], Pillar::Resilience, now());
        assert_eq!(
            report
                .findings
                .iter()
                .map(|f| f.reason_code.as_str())
                .collect::<Vec<_>>(),
            vec![REASON_RES_CLUSTER_NOT_ACTIVE]
        );
    }
}

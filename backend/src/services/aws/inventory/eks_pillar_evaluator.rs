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

// Deterministic EKS inventory evaluators for the cost, security, and
// resilience pillars (roadmap rows 01-AWS-CLOUD-00253/00262/00289).
//
// Evaluates EksCluster rows persisted by eks_control_plane (PascalCase
// keys: ClusterName, Version, Endpoint, Status). Tags are not collected
// for EKS, so tag posture is reported as an explicit data gap.

use chrono::{DateTime, Utc};
use serde_json::json;

use crate::models::aws_resource::Model as AwsResourceModel;
use crate::services::aws::inventory::types::{
    check_stale, data_str, score_pillar, InventoryFinding, Pillar, PillarReport, Severity,
};

// Reason codes are the stable contract for findings; never reuse or rename.
pub const REASON_COST_TAG_DATA_NOT_COLLECTED: &str = "EKS_COST_TAG_DATA_NOT_COLLECTED";
pub const REASON_SEC_OUTDATED_VERSION: &str = "EKS_SEC_OUTDATED_VERSION";
pub const REASON_SEC_ENDPOINT_ACCESS_DATA_NOT_COLLECTED: &str =
    "EKS_SEC_ENDPOINT_ACCESS_DATA_NOT_COLLECTED";
pub const REASON_RES_CLUSTER_NOT_ACTIVE: &str = "EKS_RES_CLUSTER_NOT_ACTIVE";
pub const REASON_INV_STALE_DATA: &str = "EKS_INV_STALE_DATA";

/// Kubernetes versions past AWS end-of-standard-support. Explicit
/// deterministic list; extend when AWS retires more versions.
pub const OUTDATED_EKS_VERSIONS: &[&str] = &[
    "1.19", "1.20", "1.21", "1.22", "1.23", "1.24", "1.25", "1.26", "1.27",
];

/// Evaluate every EKS cluster in the fleet for one pillar.
pub fn evaluate_eks_fleet(
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
            reason_code: REASON_COST_TAG_DATA_NOT_COLLECTED.to_string(),
            severity: Severity::Medium,
            message: format!(
                "Tags for cluster {} are not collected yet; cost allocation cannot be assessed",
                resource.resource_id
            ),
            evidence: json!({ "tags": resource.tags }),
        });
    }
}

fn evaluate_security(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    if let Some(version) = data_str(&resource.resource_data, "Version") {
        if OUTDATED_EKS_VERSIONS.contains(&version.as_str()) {
            findings.push(InventoryFinding {
                resource_id: resource.resource_id.clone(),
                arn: resource.arn.clone(),
                pillar: Pillar::Security,
                reason_code: REASON_SEC_OUTDATED_VERSION.to_string(),
                severity: Severity::High,
                message: format!(
                    "Cluster {} runs Kubernetes {} which is past end of standard support; it no longer receives security patches without extended support fees",
                    resource.resource_id, version
                ),
                evidence: json!({ "Version": version }),
            });
        }
    }

    // Public/private endpoint access config is not collected yet.
    if resource.resource_data.get("EndpointPublicAccess").is_none() {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Security,
            reason_code: REASON_SEC_ENDPOINT_ACCESS_DATA_NOT_COLLECTED.to_string(),
            severity: Severity::Medium,
            message: format!(
                "Endpoint access configuration for cluster {} is not collected yet; API exposure cannot be assessed",
                resource.resource_id
            ),
            evidence: json!({ "EndpointPublicAccess_collected": false }),
        });
    }
}

fn evaluate_resilience(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
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
            resource_type: "EksCluster".to_string(),
            resource_id: resource_id.to_string(),
            arn: format!("arn:aws:eks:us-east-1:123456789012:cluster/{}", resource_id),
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
            "ClusterName": "prod",
            "Version": "1.31",
            "Status": "ACTIVE",
            "EndpointPublicAccess": false,
        })
    }

    #[test]
    fn cost_reports_tag_gap_for_untagged_cluster() {
        let r = fixture("prod", json!({}), healthy_data(), now());
        let report = evaluate_eks_fleet(&[r], Pillar::Cost, now());
        assert_eq!(
            report
                .findings
                .iter()
                .map(|f| f.reason_code.as_str())
                .collect::<Vec<_>>(),
            vec![REASON_COST_TAG_DATA_NOT_COLLECTED]
        );
    }

    #[test]
    fn security_flags_outdated_version_as_high() {
        let mut data = healthy_data();
        data["Version"] = json!("1.24");
        let r = fixture("legacy", json!({"team": "platform"}), data, now());
        let report = evaluate_eks_fleet(&[r], Pillar::Security, now());
        let finding = report
            .findings
            .iter()
            .find(|f| f.reason_code == REASON_SEC_OUTDATED_VERSION)
            .expect("outdated version finding");
        assert_eq!(finding.severity, Severity::High);
        assert_eq!(finding.evidence["Version"], json!("1.24"));
    }

    #[test]
    fn security_reports_endpoint_access_gap_when_not_collected() {
        let r = fixture(
            "gap",
            json!({"team": "platform"}),
            json!({"ClusterName": "gap", "Version": "1.31", "Status": "ACTIVE"}),
            now(),
        );
        let report = evaluate_eks_fleet(&[r], Pillar::Security, now());
        assert_eq!(
            report
                .findings
                .iter()
                .map(|f| f.reason_code.as_str())
                .collect::<Vec<_>>(),
            vec![REASON_SEC_ENDPOINT_ACCESS_DATA_NOT_COLLECTED]
        );
    }

    #[test]
    fn security_passes_for_current_version_with_endpoint_data() {
        let r = fixture("prod", json!({"team": "platform"}), healthy_data(), now());
        let report = evaluate_eks_fleet(&[r], Pillar::Security, now());
        assert!(
            report.findings.is_empty(),
            "unexpected: {:?}",
            report.findings
        );
    }

    #[test]
    fn resilience_flags_non_active_cluster() {
        let mut data = healthy_data();
        data["Status"] = json!("FAILED");
        let r = fixture("broken", json!({"team": "platform"}), data, now());
        let report = evaluate_eks_fleet(&[r], Pillar::Resilience, now());
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

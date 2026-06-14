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

// Deterministic EKS Fargate profile inventory evaluators for the cost,
// security, and resilience pillars (roadmap rows
// 01-AWS-CLOUD-00316/00325/00352).
//
// Evaluates fields persisted by eks_control_plane for resource_type
// "FargateProfile": FargateProfileName, FargateProfileArn, ClusterName,
// Status, PodExecutionRoleArn, Subnets. Rows of any other resource_type are
// skipped and excluded from the evaluated count. When the Subnets array is
// absent or null we deliberately emit no subnet finding: AZ spread cannot be
// honestly assessed without data, and inventing a gap code for a field that
// is collected together with Status would double-count the same collection
// gap.

use chrono::{DateTime, Utc};
use serde_json::json;

use crate::models::aws_resource::Model as AwsResourceModel;
use crate::services::aws::inventory::types::{
    check_stale, score_pillar, InventoryFinding, Pillar, PillarReport, Severity,
};

// Reason codes are the stable contract for findings; never reuse or rename.
pub const REASON_COST_NO_TAGS: &str = "FARGATE_COST_NO_TAGS";
pub const REASON_RES_PROFILE_NOT_ACTIVE: &str = "FARGATE_RES_PROFILE_NOT_ACTIVE";
pub const REASON_RES_STATUS_DATA_NOT_COLLECTED: &str = "FARGATE_RES_STATUS_DATA_NOT_COLLECTED";
pub const REASON_RES_SINGLE_SUBNET: &str = "FARGATE_RES_SINGLE_SUBNET";
pub const REASON_SEC_POD_EXECUTION_ROLE_DATA_NOT_COLLECTED: &str =
    "FARGATE_SEC_POD_EXECUTION_ROLE_DATA_NOT_COLLECTED";
pub const REASON_INV_STALE_DATA: &str = "FARGATE_INV_STALE_DATA";

const RESOURCE_TYPE: &str = "FargateProfile";

/// Evaluate every EKS Fargate profile in the fleet for one pillar.
pub fn evaluate_fargate_fleet(
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
                "Fargate profile {} has no tags recorded (untagged resource or tag collection gap); cost allocation cannot be assessed",
                resource.resource_id
            ),
            evidence: json!({ "tags": resource.tags }),
        });
    }
}

fn evaluate_security(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    let role = resource
        .resource_data
        .get("PodExecutionRoleArn")
        .and_then(|v| v.as_str());
    if role.is_none() {
        // Every Fargate profile must have a pod execution role, so an absent
        // value is a collection gap, not a misconfigured profile.
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Security,
            reason_code: REASON_SEC_POD_EXECUTION_ROLE_DATA_NOT_COLLECTED.to_string(),
            severity: Severity::Medium,
            message: format!(
                "Pod execution role for Fargate profile {} is not collected yet (a profile cannot exist without one, so this is a data collection gap); role permissions cannot be assessed",
                resource.resource_id
            ),
            evidence: json!({ "pod_execution_role_arn_collected": false }),
        });
    }
    // When the role ARN is present there is no further honest inventory-only
    // check; assessing the role's policies needs IAM data we do not have here.
}

fn evaluate_resilience(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    let status = resource
        .resource_data
        .get("Status")
        .and_then(|v| v.as_str());
    match status {
        Some(s) if !s.eq_ignore_ascii_case("ACTIVE") => {
            findings.push(InventoryFinding {
                resource_id: resource.resource_id.clone(),
                arn: resource.arn.clone(),
                pillar: Pillar::Resilience,
                reason_code: REASON_RES_PROFILE_NOT_ACTIVE.to_string(),
                severity: Severity::High,
                message: format!(
                    "Fargate profile {} is in status {} (not ACTIVE); pods selected by this profile may fail to schedule",
                    resource.resource_id, s
                ),
                evidence: json!({ "status": s }),
            });
        }
        Some(_) => {}
        None => {
            findings.push(InventoryFinding {
                resource_id: resource.resource_id.clone(),
                arn: resource.arn.clone(),
                pillar: Pillar::Resilience,
                reason_code: REASON_RES_STATUS_DATA_NOT_COLLECTED.to_string(),
                severity: Severity::Medium,
                message: format!(
                    "Status for Fargate profile {} is not collected yet; resilience pillar cannot be fully assessed",
                    resource.resource_id
                ),
                evidence: json!({ "status_collected": false }),
            });
        }
    }

    // Single-subnet profiles place every pod in one AZ. Absent/null Subnets
    // emits no finding (see module doc comment): we do not guess AZ spread.
    if let Some(subnets) = resource
        .resource_data
        .get("Subnets")
        .and_then(|v| v.as_array())
    {
        if subnets.len() == 1 {
            findings.push(InventoryFinding {
                resource_id: resource.resource_id.clone(),
                arn: resource.arn.clone(),
                pillar: Pillar::Resilience,
                reason_code: REASON_RES_SINGLE_SUBNET.to_string(),
                severity: Severity::Medium,
                message: format!(
                    "Fargate profile {} targets a single subnet, creating a single-AZ pod placement risk; an AZ outage stops all pods on this profile",
                    resource.resource_id
                ),
                evidence: json!({ "subnets": subnets }),
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
            resource_type: "FargateProfile".to_string(),
            resource_id: resource_id.to_string(),
            arn: format!(
                "arn:aws:eks:us-east-1:123456789012:fargateprofile/demo/{}/aaaa",
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
            "FargateProfileName": "fp-ok",
            "FargateProfileArn": "arn:aws:eks:us-east-1:123456789012:fargateprofile/demo/fp-ok/aaaa",
            "ClusterName": "demo",
            "Status": "ACTIVE",
            "PodExecutionRoleArn": "arn:aws:iam::123456789012:role/fargate-pod-exec",
            "Subnets": ["subnet-a", "subnet-b"],
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
    fn cost_flags_empty_tags() {
        let r = fixture("fp-untagged", json!({}), healthy_data(), now());
        let report = evaluate_fargate_fleet(&[r], Pillar::Cost, now());
        assert_eq!(codes(&report), vec![REASON_COST_NO_TAGS]);
        assert!(report.findings[0]
            .message
            .contains("untagged resource or tag collection gap"));
    }

    #[test]
    fn resilience_flags_non_active_status_case_insensitively() {
        let mut data = healthy_data();
        data["Status"] = json!("deleting");
        let r = fixture("fp-deleting", json!({"team": "sre"}), data, now());
        let report = evaluate_fargate_fleet(&[r], Pillar::Resilience, now());
        assert_eq!(codes(&report), vec![REASON_RES_PROFILE_NOT_ACTIVE]);
    }

    #[test]
    fn resilience_reports_gap_when_status_not_collected() {
        let mut data = healthy_data();
        data["Status"] = json!(null);
        let r = fixture("fp-nostatus", json!({"team": "sre"}), data, now());
        let report = evaluate_fargate_fleet(&[r], Pillar::Resilience, now());
        assert_eq!(codes(&report), vec![REASON_RES_STATUS_DATA_NOT_COLLECTED]);
    }

    #[test]
    fn resilience_flags_single_subnet() {
        let mut data = healthy_data();
        data["Subnets"] = json!(["subnet-a"]);
        let r = fixture("fp-onesubnet", json!({"team": "sre"}), data, now());
        let report = evaluate_fargate_fleet(&[r], Pillar::Resilience, now());
        assert_eq!(codes(&report), vec![REASON_RES_SINGLE_SUBNET]);
    }

    #[test]
    fn resilience_emits_no_subnet_finding_when_subnets_not_collected() {
        let mut data = healthy_data();
        data["Subnets"] = json!(null);
        let r = fixture("fp-nosubnets", json!({"team": "sre"}), data, now());
        let report = evaluate_fargate_fleet(&[r], Pillar::Resilience, now());
        assert!(!codes(&report).contains(&REASON_RES_SINGLE_SUBNET));
    }

    #[test]
    fn security_reports_gap_when_pod_execution_role_not_collected() {
        let mut data = healthy_data();
        data["PodExecutionRoleArn"] = json!(null);
        let r = fixture("fp-norole", json!({"team": "sre"}), data, now());
        let report = evaluate_fargate_fleet(&[r], Pillar::Security, now());
        assert_eq!(
            codes(&report),
            vec![REASON_SEC_POD_EXECUTION_ROLE_DATA_NOT_COLLECTED]
        );
    }

    #[test]
    fn stale_inventory_is_flagged() {
        let mut r = fixture("fp-stale", json!({"team": "sre"}), healthy_data(), now());
        r.last_refreshed = now() - Duration::hours(48);
        let report = evaluate_fargate_fleet(&[r], Pillar::Resilience, now());
        assert_eq!(report.stale_resources, 1);
        assert!(codes(&report).contains(&REASON_INV_STALE_DATA));
    }

    #[test]
    fn non_fargate_rows_are_skipped() {
        let mut other = fixture("eks-cluster", json!({}), json!({}), now());
        other.resource_type = "EksCluster".to_string();
        let report = evaluate_fargate_fleet(&[other], Pillar::Cost, now());
        assert_eq!(report.resources_evaluated, 0);
        assert!(report.findings.is_empty());
        assert_eq!(report.score, 100);
    }

    #[test]
    fn healthy_profile_passes_all_pillars() {
        let r = fixture("fp-ok", json!({"team": "sre"}), healthy_data(), now());
        for pillar in [Pillar::Cost, Pillar::Security, Pillar::Resilience] {
            let report = evaluate_fargate_fleet(std::slice::from_ref(&r), pillar, now());
            assert_eq!(report.resources_evaluated, 1);
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

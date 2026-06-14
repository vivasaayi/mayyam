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

// Deterministic OpenSearch inventory evaluators for the cost, security,
// and resilience pillars (roadmap rows 01-AWS-CLOUD-02143/02152/02179).
//
// Evaluates fields persisted by opensearch_control_plane:
// cluster_config{instance_count, dedicated_master_enabled,
// zone_awareness_enabled}. Encryption and tags are not collected yet and
// are reported as explicit data gaps.

use chrono::{DateTime, Utc};
use serde_json::json;

use crate::models::aws_resource::Model as AwsResourceModel;
use crate::services::aws::inventory::types::{
    check_stale, score_pillar, InventoryFinding, Pillar, PillarReport, Severity,
};

// Reason codes are the stable contract for findings; never reuse or rename.
pub const REASON_COST_NO_TAGS: &str = "OPENSEARCH_COST_NO_TAGS";
pub const REASON_SEC_ENCRYPTION_DATA_NOT_COLLECTED: &str =
    "OPENSEARCH_SEC_ENCRYPTION_DATA_NOT_COLLECTED";
pub const REASON_RES_SINGLE_NODE: &str = "OPENSEARCH_RES_SINGLE_NODE";
pub const REASON_RES_NO_ZONE_AWARENESS: &str = "OPENSEARCH_RES_NO_ZONE_AWARENESS";
pub const REASON_RES_NO_DEDICATED_MASTER: &str = "OPENSEARCH_RES_NO_DEDICATED_MASTER";
pub const REASON_INV_STALE_DATA: &str = "OPENSEARCH_INV_STALE_DATA";

/// Evaluate every OpenSearch domain in the fleet for one pillar.
pub fn evaluate_opensearch_fleet(
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
            reason_code: REASON_COST_NO_TAGS.to_string(),
            severity: Severity::Medium,
            message: format!(
                "Domain {} has no tags recorded (untagged resource or tag collection gap); cost allocation cannot be assessed",
                resource.resource_id
            ),
            evidence: json!({ "tags": resource.tags }),
        });
    }
}

fn evaluate_security(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    if resource.resource_data.get("encryption_at_rest").is_none() {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Security,
            reason_code: REASON_SEC_ENCRYPTION_DATA_NOT_COLLECTED.to_string(),
            severity: Severity::Medium,
            message: format!(
                "Encryption state for domain {} is not collected yet; security pillar cannot be fully assessed",
                resource.resource_id
            ),
            evidence: json!({ "encryption_at_rest_collected": false }),
        });
    }
}

fn evaluate_resilience(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    let config = resource.resource_data.get("cluster_config");
    let instance_count = config
        .and_then(|c| c.get("instance_count"))
        .and_then(|v| v.as_i64());
    if instance_count == Some(1) {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Resilience,
            reason_code: REASON_RES_SINGLE_NODE.to_string(),
            severity: Severity::Medium,
            message: format!(
                "Domain {} runs a single data node; a node failure causes an outage and possible data loss",
                resource.resource_id
            ),
            evidence: json!({ "instance_count": 1 }),
        });
    }

    let zone_awareness = config
        .and_then(|c| c.get("zone_awareness_enabled"))
        .and_then(|v| v.as_bool());
    if zone_awareness == Some(false) {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Resilience,
            reason_code: REASON_RES_NO_ZONE_AWARENESS.to_string(),
            severity: Severity::Medium,
            message: format!(
                "Domain {} has zone awareness disabled; all nodes sit in one availability zone",
                resource.resource_id
            ),
            evidence: json!({ "zone_awareness_enabled": false }),
        });
    }

    let dedicated_master = config
        .and_then(|c| c.get("dedicated_master_enabled"))
        .and_then(|v| v.as_bool());
    if dedicated_master == Some(false) {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Resilience,
            reason_code: REASON_RES_NO_DEDICATED_MASTER.to_string(),
            severity: Severity::Low,
            message: format!(
                "Domain {} has no dedicated master nodes; heavy data-node load can destabilize the cluster",
                resource.resource_id
            ),
            evidence: json!({ "dedicated_master_enabled": false }),
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
            resource_type: "OpenSearchDomain".to_string(),
            resource_id: resource_id.to_string(),
            arn: format!("arn:aws:es:us-east-1:123456789012:domain/{}", resource_id),
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
    fn resilience_flags_single_node_no_zone_awareness_no_master() {
        let r = fixture(
            "logs",
            json!({"team": "obs"}),
            json!({
                "domain_name": "logs",
                "encryption_at_rest": {"enabled": true},
                "cluster_config": {
                    "instance_count": 1,
                    "zone_awareness_enabled": false,
                    "dedicated_master_enabled": false,
                },
            }),
            now(),
        );
        let report = evaluate_opensearch_fleet(&[r], Pillar::Resilience, now());
        let codes: Vec<&str> = report
            .findings
            .iter()
            .map(|f| f.reason_code.as_str())
            .collect();
        assert!(codes.contains(&REASON_RES_SINGLE_NODE));
        assert!(codes.contains(&REASON_RES_NO_ZONE_AWARENESS));
        assert!(codes.contains(&REASON_RES_NO_DEDICATED_MASTER));
    }

    #[test]
    fn resilience_passes_for_multi_az_domain_with_masters() {
        let r = fixture(
            "search",
            json!({"team": "obs"}),
            json!({
                "domain_name": "search",
                "encryption_at_rest": {"enabled": true},
                "cluster_config": {
                    "instance_count": 3,
                    "zone_awareness_enabled": true,
                    "dedicated_master_enabled": true,
                },
            }),
            now(),
        );
        let report = evaluate_opensearch_fleet(&[r], Pillar::Resilience, now());
        assert!(
            report.findings.is_empty(),
            "unexpected: {:?}",
            report.findings
        );
    }

    #[test]
    fn security_reports_encryption_gap_when_not_collected() {
        let r = fixture(
            "gap",
            json!({"team": "obs"}),
            json!({"domain_name": "gap", "cluster_config": {"instance_count": 3}}),
            now(),
        );
        let report = evaluate_opensearch_fleet(&[r], Pillar::Security, now());
        assert_eq!(
            report
                .findings
                .iter()
                .map(|f| f.reason_code.as_str())
                .collect::<Vec<_>>(),
            vec![REASON_SEC_ENCRYPTION_DATA_NOT_COLLECTED]
        );
    }

    #[test]
    fn cost_reports_tag_gap_for_untagged_domain() {
        let r = fixture(
            "untagged",
            json!({}),
            json!({"domain_name": "untagged", "encryption_at_rest": {"enabled": true}}),
            now(),
        );
        let report = evaluate_opensearch_fleet(&[r], Pillar::Cost, now());
        assert_eq!(
            report
                .findings
                .iter()
                .map(|f| f.reason_code.as_str())
                .collect::<Vec<_>>(),
            vec![REASON_COST_NO_TAGS]
        );
    }
}

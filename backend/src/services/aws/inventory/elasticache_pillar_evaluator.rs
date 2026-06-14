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

// Deterministic ElastiCache inventory evaluators for the cost, security,
// and resilience pillars (roadmap rows 01-AWS-CLOUD-01324/01333/01360).
//
// Evaluates fields persisted by elasticache_control_plane: engine,
// cache_node_type, num_cache_nodes, snapshot_retention_limit,
// at_rest_encryption_enabled, transit_encryption_enabled.

use chrono::{DateTime, Utc};
use serde_json::json;

use crate::models::aws_resource::Model as AwsResourceModel;
use crate::services::aws::inventory::types::{
    check_stale, score_pillar, InventoryFinding, Pillar, PillarReport, Severity,
};

// Reason codes are the stable contract for findings; never reuse or rename.
pub const REASON_COST_NO_TAGS: &str = "ELASTICACHE_COST_NO_TAGS";
pub const REASON_SEC_AT_REST_ENCRYPTION_DISABLED: &str =
    "ELASTICACHE_SEC_AT_REST_ENCRYPTION_DISABLED";
pub const REASON_SEC_TRANSIT_ENCRYPTION_DISABLED: &str =
    "ELASTICACHE_SEC_TRANSIT_ENCRYPTION_DISABLED";
pub const REASON_SEC_ENCRYPTION_DATA_NOT_COLLECTED: &str =
    "ELASTICACHE_SEC_ENCRYPTION_DATA_NOT_COLLECTED";
pub const REASON_RES_SNAPSHOTS_DISABLED: &str = "ELASTICACHE_RES_SNAPSHOTS_DISABLED";
pub const REASON_RES_SINGLE_NODE: &str = "ELASTICACHE_RES_SINGLE_NODE";
pub const REASON_INV_STALE_DATA: &str = "ELASTICACHE_INV_STALE_DATA";

/// Evaluate every ElastiCache cluster in the fleet for one pillar.
pub fn evaluate_elasticache_fleet(
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
                "Cluster {} has no tags recorded (untagged resource or tag collection gap); cost allocation cannot be assessed",
                resource.resource_id
            ),
            evidence: json!({ "tags": resource.tags }),
        });
    }
}

fn evaluate_security(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    let at_rest = resource
        .resource_data
        .get("at_rest_encryption_enabled")
        .and_then(|v| v.as_bool());
    let in_transit = resource
        .resource_data
        .get("transit_encryption_enabled")
        .and_then(|v| v.as_bool());

    match (at_rest, in_transit) {
        (None, None) => {
            findings.push(InventoryFinding {
                resource_id: resource.resource_id.clone(),
                arn: resource.arn.clone(),
                pillar: Pillar::Security,
                reason_code: REASON_SEC_ENCRYPTION_DATA_NOT_COLLECTED.to_string(),
                severity: Severity::Medium,
                message: format!(
                    "Encryption state for cluster {} is not collected yet; security pillar cannot be fully assessed",
                    resource.resource_id
                ),
                evidence: json!({ "encryption_fields_collected": false }),
            });
        }
        _ => {
            if at_rest == Some(false) {
                findings.push(InventoryFinding {
                    resource_id: resource.resource_id.clone(),
                    arn: resource.arn.clone(),
                    pillar: Pillar::Security,
                    reason_code: REASON_SEC_AT_REST_ENCRYPTION_DISABLED.to_string(),
                    severity: Severity::High,
                    message: format!(
                        "Cluster {} has at-rest encryption disabled",
                        resource.resource_id
                    ),
                    evidence: json!({ "at_rest_encryption_enabled": false }),
                });
            }
            if in_transit == Some(false) {
                findings.push(InventoryFinding {
                    resource_id: resource.resource_id.clone(),
                    arn: resource.arn.clone(),
                    pillar: Pillar::Security,
                    reason_code: REASON_SEC_TRANSIT_ENCRYPTION_DISABLED.to_string(),
                    severity: Severity::Medium,
                    message: format!(
                        "Cluster {} has in-transit encryption disabled",
                        resource.resource_id
                    ),
                    evidence: json!({ "transit_encryption_enabled": false }),
                });
            }
        }
    }
}

fn evaluate_resilience(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    let retention = resource
        .resource_data
        .get("snapshot_retention_limit")
        .and_then(|v| v.as_i64());
    if retention == Some(0) {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Resilience,
            reason_code: REASON_RES_SNAPSHOTS_DISABLED.to_string(),
            severity: Severity::High,
            message: format!(
                "Cluster {} has automatic snapshots disabled (retention 0); data cannot be recovered after node loss",
                resource.resource_id
            ),
            evidence: json!({ "snapshot_retention_limit": 0 }),
        });
    }

    let nodes = resource
        .resource_data
        .get("num_cache_nodes")
        .and_then(|v| v.as_i64());
    if nodes == Some(1) {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Resilience,
            reason_code: REASON_RES_SINGLE_NODE.to_string(),
            severity: Severity::Medium,
            message: format!(
                "Cluster {} runs a single cache node; a node failure causes a full cache outage",
                resource.resource_id
            ),
            evidence: json!({ "num_cache_nodes": 1 }),
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
            resource_type: "ElasticacheCluster".to_string(),
            resource_id: resource_id.to_string(),
            arn: format!(
                "arn:aws:elasticache:us-east-1:123456789012:cluster:{}",
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
            "engine": "redis",
            "cache_node_type": "cache.r6g.large",
            "num_cache_nodes": 2,
            "snapshot_retention_limit": 7,
            "at_rest_encryption_enabled": true,
            "transit_encryption_enabled": true,
        })
    }

    #[test]
    fn security_flags_disabled_encryption() {
        let mut data = healthy_data();
        data["at_rest_encryption_enabled"] = json!(false);
        data["transit_encryption_enabled"] = json!(false);
        let r = fixture("cache-plain", json!({"team": "perf"}), data, now());
        let report = evaluate_elasticache_fleet(&[r], Pillar::Security, now());
        let codes: Vec<&str> = report
            .findings
            .iter()
            .map(|f| f.reason_code.as_str())
            .collect();
        assert!(codes.contains(&REASON_SEC_AT_REST_ENCRYPTION_DISABLED));
        assert!(codes.contains(&REASON_SEC_TRANSIT_ENCRYPTION_DISABLED));
    }

    #[test]
    fn security_reports_gap_when_encryption_not_collected() {
        let r = fixture(
            "cache-gap",
            json!({"team": "perf"}),
            json!({"engine": "redis", "num_cache_nodes": 2}),
            now(),
        );
        let report = evaluate_elasticache_fleet(&[r], Pillar::Security, now());
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
    fn resilience_flags_disabled_snapshots_and_single_node() {
        let r = fixture(
            "cache-fragile",
            json!({"team": "perf"}),
            json!({"engine": "redis", "num_cache_nodes": 1, "snapshot_retention_limit": 0}),
            now(),
        );
        let report = evaluate_elasticache_fleet(&[r], Pillar::Resilience, now());
        let codes: Vec<&str> = report
            .findings
            .iter()
            .map(|f| f.reason_code.as_str())
            .collect();
        assert!(codes.contains(&REASON_RES_SNAPSHOTS_DISABLED));
        assert!(codes.contains(&REASON_RES_SINGLE_NODE));
    }

    #[test]
    fn healthy_cluster_passes_all_pillars() {
        let r = fixture("cache-ok", json!({"team": "perf"}), healthy_data(), now());
        for pillar in [Pillar::Cost, Pillar::Security, Pillar::Resilience] {
            let report = evaluate_elasticache_fleet(std::slice::from_ref(&r), pillar, now());
            assert!(
                report.findings.is_empty(),
                "unexpected for {:?}: {:?}",
                pillar,
                report.findings
            );
        }
    }
}

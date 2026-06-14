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

// Deterministic MemoryDB cluster inventory evaluators for the cost, resilience,
// and security pillars (roadmap rows 01-AWS-CLOUD-01576/01585/01612).
//
// Evaluates fields persisted by memorydb_control_plane: status, engine_version,
// node_type, num_shards, num_replicas_per_shard, availability_mode, tls_enabled,
// kms_key_id, acl_name, snapshot_retention_limit, plus tags.

use chrono::{DateTime, Utc};
use serde_json::{json, Value};

use crate::models::aws_resource::Model as AwsResourceModel;
use crate::services::aws::inventory::types::{
    check_stale, score_pillar, InventoryFinding, Pillar, PillarReport, Severity,
};

pub const RESOURCE_TYPE: &str = "MemoryDbCluster";

pub const REASON_COST_NO_TAGS: &str = "MEMORYDB_COST_NO_TAGS";
pub const REASON_COST_SINGLE_SHARD_OVER_REPLICATED: &str =
    "MEMORYDB_COST_SINGLE_SHARD_OVER_REPLICATED";
pub const REASON_RES_SINGLE_AVAILABILITY_ZONE: &str = "MEMORYDB_RES_SINGLE_AVAILABILITY_ZONE";
pub const REASON_RES_NO_REPLICAS: &str = "MEMORYDB_RES_NO_REPLICAS";
pub const REASON_RES_LOW_SNAPSHOT_RETENTION: &str = "MEMORYDB_RES_LOW_SNAPSHOT_RETENTION";
pub const REASON_RES_CLUSTER_NOT_AVAILABLE: &str = "MEMORYDB_RES_CLUSTER_NOT_AVAILABLE";
pub const REASON_SEC_TLS_DISABLED: &str = "MEMORYDB_SEC_TLS_DISABLED";
pub const REASON_SEC_NO_KMS_KEY: &str = "MEMORYDB_SEC_NO_KMS_KEY";
pub const REASON_SEC_OPEN_ACCESS_ACL: &str = "MEMORYDB_SEC_OPEN_ACCESS_ACL";
pub const REASON_INV_STALE_DATA: &str = "MEMORYDB_INV_STALE_DATA";

const MIN_SNAPSHOT_RETENTION_DAYS: i64 = 7;

pub fn evaluate_memorydb_fleet(
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
                "MemoryDB cluster {} has no tags; cost allocation cannot be assessed",
                resource.resource_id
            ),
            evidence: json!({ "tags": resource.tags }),
        });
    }

    // A single-shard cluster with 2+ replicas has expensive over-replication
    // relative to sharding the data across multiple shards with 1 replica each.
    if let (Some(shards), Some(replicas)) = (
        data_i64(&resource.resource_data, "num_shards"),
        data_i64(&resource.resource_data, "num_replicas_per_shard"),
    ) {
        if shards == 1 && replicas >= 2 {
            findings.push(InventoryFinding {
                resource_id: resource.resource_id.clone(),
                arn: resource.arn.clone(),
                pillar: Pillar::Cost,
                reason_code: REASON_COST_SINGLE_SHARD_OVER_REPLICATED.to_string(),
                severity: Severity::Low,
                message: format!(
                    "MemoryDB cluster {} has 1 shard with {} replicas; consider adding shards and reducing replicas to improve cost efficiency and throughput",
                    resource.resource_id, replicas
                ),
                evidence: json!({ "num_shards": shards, "num_replicas_per_shard": replicas }),
            });
        }
    }
}

fn evaluate_resilience(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    if let Some(status) = data_str(&resource.resource_data, "status") {
        if status != "available" {
            findings.push(InventoryFinding {
                resource_id: resource.resource_id.clone(),
                arn: resource.arn.clone(),
                pillar: Pillar::Resilience,
                reason_code: REASON_RES_CLUSTER_NOT_AVAILABLE.to_string(),
                severity: Severity::High,
                message: format!(
                    "MemoryDB cluster {} is in status '{}' rather than 'available'; investigate to prevent data-stream disruption",
                    resource.resource_id, status
                ),
                evidence: json!({ "status": status }),
            });
        }
    }

    // MultiAZ means replicas are spread across AZs.
    if let Some(availability_mode) = data_str(&resource.resource_data, "availability_mode") {
        if availability_mode == "SingleAZ" {
            findings.push(InventoryFinding {
                resource_id: resource.resource_id.clone(),
                arn: resource.arn.clone(),
                pillar: Pillar::Resilience,
                reason_code: REASON_RES_SINGLE_AVAILABILITY_ZONE.to_string(),
                severity: Severity::High,
                message: format!(
                    "MemoryDB cluster {} uses SingleAZ availability mode; use MultiAZ to tolerate zone-level failures without data loss",
                    resource.resource_id
                ),
                evidence: json!({ "availability_mode": availability_mode }),
            });
        }
    }

    if let Some(replicas) = data_i64(&resource.resource_data, "num_replicas_per_shard") {
        if replicas == 0 {
            findings.push(InventoryFinding {
                resource_id: resource.resource_id.clone(),
                arn: resource.arn.clone(),
                pillar: Pillar::Resilience,
                reason_code: REASON_RES_NO_REPLICAS.to_string(),
                severity: Severity::High,
                message: format!(
                    "MemoryDB cluster {} has 0 replicas per shard; without replicas, any shard failure causes data loss",
                    resource.resource_id
                ),
                evidence: json!({ "num_replicas_per_shard": replicas }),
            });
        }
    }

    if let Some(retention) = data_i64(&resource.resource_data, "snapshot_retention_limit") {
        if retention < MIN_SNAPSHOT_RETENTION_DAYS {
            findings.push(InventoryFinding {
                resource_id: resource.resource_id.clone(),
                arn: resource.arn.clone(),
                pillar: Pillar::Resilience,
                reason_code: REASON_RES_LOW_SNAPSHOT_RETENTION.to_string(),
                severity: Severity::Medium,
                message: format!(
                    "MemoryDB cluster {} snapshot retention is {} day(s); increase to at least {} days for point-in-time recovery",
                    resource.resource_id, retention, MIN_SNAPSHOT_RETENTION_DAYS
                ),
                evidence: json!({
                    "snapshot_retention_limit": retention,
                    "minimum_recommended": MIN_SNAPSHOT_RETENTION_DAYS,
                }),
            });
        }
    }
}

fn evaluate_security(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    if !data_bool(&resource.resource_data, "tls_enabled").unwrap_or(false) {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Security,
            reason_code: REASON_SEC_TLS_DISABLED.to_string(),
            severity: Severity::High,
            message: format!(
                "MemoryDB cluster {} does not have TLS in-transit encryption enabled; enable TLS to prevent eavesdropping on cluster traffic",
                resource.resource_id
            ),
            evidence: json!({ "tls_enabled": false }),
        });
    }

    if resource.resource_data.get("kms_key_id").is_none() {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Security,
            reason_code: REASON_SEC_NO_KMS_KEY.to_string(),
            severity: Severity::Low,
            message: format!(
                "MemoryDB cluster {} does not use a customer-managed KMS key for encryption at rest; use a CMK for key rotation control and audit visibility",
                resource.resource_id
            ),
            evidence: json!({ "kms_key_id": null }),
        });
    }

    // The default ACL "open-access" grants all users full access to all commands.
    if let Some(acl) = data_str(&resource.resource_data, "acl_name") {
        if acl == "open-access" {
            findings.push(InventoryFinding {
                resource_id: resource.resource_id.clone(),
                arn: resource.arn.clone(),
                pillar: Pillar::Security,
                reason_code: REASON_SEC_OPEN_ACCESS_ACL.to_string(),
                severity: Severity::High,
                message: format!(
                    "MemoryDB cluster {} uses the 'open-access' ACL which grants all users full access; assign a custom ACL with least-privilege user permissions",
                    resource.resource_id
                ),
                evidence: json!({ "acl_name": acl }),
            });
        }
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
            arn: format!(
                "arn:aws:memory-db:us-east-1:123456789012:cluster/{}",
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
        DateTime::parse_from_rfc3339("2026-06-11T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc)
    }

    fn healthy_data() -> Value {
        json!({
            "status": "available",
            "engine_version": "7.1",
            "node_type": "db.r6g.large",
            "num_shards": 2,
            "num_replicas_per_shard": 1,
            "availability_mode": "MultiAZ",
            "tls_enabled": true,
            "kms_key_id": "arn:aws:kms:us-east-1:123456789012:key/abc",
            "acl_name": "my-acl",
            "snapshot_retention_limit": 7,
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
        let r = fixture(
            "my-memorydb",
            json!({"team": "cache"}),
            healthy_data(),
            now(),
        );
        for pillar in [Pillar::Cost, Pillar::Resilience, Pillar::Security] {
            let report = evaluate_memorydb_fleet(std::slice::from_ref(&r), pillar, now());
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
        let report = evaluate_memorydb_fleet(&[r], Pillar::Cost, now());
        assert_eq!(codes(&report), vec![REASON_COST_NO_TAGS]);
    }

    #[test]
    fn cost_flags_single_shard_over_replicated() {
        let mut data = healthy_data();
        data["num_shards"] = json!(1);
        data["num_replicas_per_shard"] = json!(2);
        let r = fixture("over-replicated", json!({"team": "cache"}), data, now());
        let report = evaluate_memorydb_fleet(&[r], Pillar::Cost, now());
        assert!(codes(&report).contains(&REASON_COST_SINGLE_SHARD_OVER_REPLICATED));
    }

    #[test]
    fn resilience_flags_unavailable_cluster() {
        let mut data = healthy_data();
        data["status"] = json!("failed");
        let r = fixture("failed-mdb", json!({"team": "cache"}), data, now());
        let report = evaluate_memorydb_fleet(&[r], Pillar::Resilience, now());
        assert!(codes(&report).contains(&REASON_RES_CLUSTER_NOT_AVAILABLE));
    }

    #[test]
    fn resilience_flags_single_az() {
        let mut data = healthy_data();
        data["availability_mode"] = json!("SingleAZ");
        let r = fixture("single-az-mdb", json!({"team": "cache"}), data, now());
        let report = evaluate_memorydb_fleet(&[r], Pillar::Resilience, now());
        assert!(codes(&report).contains(&REASON_RES_SINGLE_AVAILABILITY_ZONE));
    }

    #[test]
    fn resilience_flags_no_replicas() {
        let mut data = healthy_data();
        data["num_replicas_per_shard"] = json!(0);
        let r = fixture("no-replicas", json!({"team": "cache"}), data, now());
        let report = evaluate_memorydb_fleet(&[r], Pillar::Resilience, now());
        assert!(codes(&report).contains(&REASON_RES_NO_REPLICAS));
    }

    #[test]
    fn resilience_flags_low_snapshot_retention() {
        let mut data = healthy_data();
        data["snapshot_retention_limit"] = json!(3);
        let r = fixture("low-snap", json!({"team": "cache"}), data, now());
        let report = evaluate_memorydb_fleet(&[r], Pillar::Resilience, now());
        assert!(codes(&report).contains(&REASON_RES_LOW_SNAPSHOT_RETENTION));
    }

    #[test]
    fn security_flags_tls_disabled() {
        let mut data = healthy_data();
        data["tls_enabled"] = json!(false);
        let r = fixture("no-tls", json!({"team": "cache"}), data, now());
        let report = evaluate_memorydb_fleet(&[r], Pillar::Security, now());
        assert!(codes(&report).contains(&REASON_SEC_TLS_DISABLED));
    }

    #[test]
    fn security_flags_no_kms_key() {
        let mut data = healthy_data();
        data.as_object_mut().unwrap().remove("kms_key_id");
        let r = fixture("no-kms", json!({"team": "cache"}), data, now());
        let report = evaluate_memorydb_fleet(&[r], Pillar::Security, now());
        assert!(codes(&report).contains(&REASON_SEC_NO_KMS_KEY));
    }

    #[test]
    fn security_flags_open_access_acl() {
        let mut data = healthy_data();
        data["acl_name"] = json!("open-access");
        let r = fixture("open-mdb", json!({"team": "cache"}), data, now());
        let report = evaluate_memorydb_fleet(&[r], Pillar::Security, now());
        assert!(codes(&report).contains(&REASON_SEC_OPEN_ACCESS_ACL));
    }

    #[test]
    fn stale_resource_is_flagged() {
        let mut r = fixture("stale-mdb", json!({"team": "cache"}), healthy_data(), now());
        r.last_refreshed = now() - Duration::hours(48);
        let report = evaluate_memorydb_fleet(&[r], Pillar::Resilience, now());
        assert_eq!(report.stale_resources, 1);
        assert!(codes(&report).contains(&REASON_INV_STALE_DATA));
    }

    #[test]
    fn non_memorydb_resources_are_skipped() {
        let mut r = fixture("cache-1", json!({}), json!({}), now());
        r.resource_type = "ElasticacheCluster".to_string();
        let report = evaluate_memorydb_fleet(&[r], Pillar::Cost, now());
        assert_eq!(report.resources_evaluated, 0);
    }
}

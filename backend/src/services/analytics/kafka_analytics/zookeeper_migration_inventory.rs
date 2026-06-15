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

// Deterministic ZooKeeper migration inventory evaluator for roadmap rows
// 04-KAFKA-DASHBOARD-MANAGEMENT-01275/01282/01303.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::config::KafkaClusterConfig;
use crate::services::aws::inventory::types::{
    score_pillar, InventoryFinding, Pillar, PillarReport, Severity, COST_ALLOCATION_TAG_KEYS,
    DEFAULT_STALE_AFTER_HOURS,
};

pub const RESOURCE_TYPE: &str = "KafkaZooKeeperMigration";
pub const REASON_COST_OWNER_NOT_RECORDED: &str =
    "KAFKA_ZOOKEEPER_MIGRATION_COST_OWNER_NOT_RECORDED";
pub const REASON_COST_NO_EVIDENCE: &str = "KAFKA_ZOOKEEPER_MIGRATION_COST_NO_EVIDENCE";
pub const REASON_COST_LEGACY_DEPENDENCIES: &str =
    "KAFKA_ZOOKEEPER_MIGRATION_COST_LEGACY_DEPENDENCIES";
pub const REASON_RES_NO_EVIDENCE: &str = "KAFKA_ZOOKEEPER_MIGRATION_RES_NO_EVIDENCE";
pub const REASON_RES_NO_METADATA_MIGRATION_EVIDENCE: &str =
    "KAFKA_ZOOKEEPER_MIGRATION_RES_NO_METADATA_MIGRATION_EVIDENCE";
pub const REASON_RES_NO_ROLLBACK_PLAN: &str = "KAFKA_ZOOKEEPER_MIGRATION_RES_NO_ROLLBACK_PLAN";
pub const REASON_SEC_NO_EVIDENCE: &str = "KAFKA_ZOOKEEPER_MIGRATION_SEC_NO_EVIDENCE";
pub const REASON_SEC_NO_ACL_MIGRATION_EVIDENCE: &str =
    "KAFKA_ZOOKEEPER_MIGRATION_SEC_NO_ACL_MIGRATION_EVIDENCE";
pub const REASON_SEC_PLAINTEXT_CONNECTION: &str =
    "KAFKA_ZOOKEEPER_MIGRATION_SEC_PLAINTEXT_CONNECTION";
pub const REASON_INV_STALE_DATA: &str = "KAFKA_ZOOKEEPER_MIGRATION_INV_STALE_DATA";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZooKeeperMigrationInventoryItem {
    pub migration_id: String,
    pub cluster_name: String,
    pub zookeeper_connect: Option<String>,
    pub zookeeper_version: Option<String>,
    pub kafka_version: Option<String>,
    pub migration_phase: Option<String>,
    pub legacy_zookeeper_dependency_count: Option<u32>,
    pub owner: Option<String>,
    pub labels: BTreeMap<String, String>,
    pub metadata_migration_evidence: bool,
    pub rollback_plan_evidence: bool,
    pub acl_migration_evidence: bool,
    pub plaintext_connection: bool,
    pub has_migration_evidence: bool,
    pub collected_at: DateTime<Utc>,
}

pub fn evaluate_zookeeper_migration_inventory(
    items: &[ZooKeeperMigrationInventoryItem],
    pillar: Pillar,
    now: DateTime<Utc>,
) -> PillarReport {
    let mut stale_resources = 0;
    let mut findings = Vec::new();

    for item in items {
        if let Some(finding) = stale_finding(item, pillar, now) {
            stale_resources += 1;
            findings.push(finding);
        }

        match pillar {
            Pillar::Cost => evaluate_cost(item, pillar, &mut findings),
            Pillar::Resilience => evaluate_resilience(item, pillar, &mut findings),
            Pillar::Security => evaluate_security(item, pillar, &mut findings),
            _ => {}
        }
    }

    PillarReport {
        pillar,
        resources_evaluated: items.len(),
        stale_resources,
        score: score_pillar(&findings),
        findings,
    }
}

pub fn zookeeper_migration_inventory_item_from_config(
    cluster: &KafkaClusterConfig,
    collected_at: DateTime<Utc>,
) -> ZooKeeperMigrationInventoryItem {
    ZooKeeperMigrationInventoryItem {
        migration_id: format!("{}:zookeeper-migration", cluster.name),
        cluster_name: cluster.name.clone(),
        zookeeper_connect: None,
        zookeeper_version: None,
        kafka_version: None,
        migration_phase: None,
        legacy_zookeeper_dependency_count: None,
        owner: None,
        labels: BTreeMap::new(),
        metadata_migration_evidence: false,
        rollback_plan_evidence: false,
        acl_migration_evidence: false,
        plaintext_connection: cluster.security_protocol.eq_ignore_ascii_case("PLAINTEXT"),
        has_migration_evidence: false,
        collected_at,
    }
}

fn evaluate_cost(
    item: &ZooKeeperMigrationInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if !has_owner_metadata(item) {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_OWNER_NOT_RECORDED,
            Severity::Medium,
            format!(
                "Kafka ZooKeeper migration inventory for cluster {} has no owner, team, project, or cost-center metadata",
                item.cluster_name
            ),
            json!({
                "migration_id": item.migration_id,
                "cluster_name": item.cluster_name,
                "checked_keys": COST_ALLOCATION_TAG_KEYS,
            }),
        ));
    }

    if !item.has_migration_evidence {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_NO_EVIDENCE,
            Severity::High,
            format!(
                "Kafka ZooKeeper migration inventory for cluster {} has no migration evidence",
                item.cluster_name
            ),
            json!({
                "migration_id": item.migration_id,
                "cluster_name": item.cluster_name,
                "recommendation": "Collect ZooKeeper connect strings, Kafka and ZooKeeper versions, migration phase, dependency count, ownership, and rollback evidence before estimating migration cost posture",
            }),
        ));
    }

    if item
        .legacy_zookeeper_dependency_count
        .is_some_and(|dependency_count| dependency_count > 0)
    {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_LEGACY_DEPENDENCIES,
            Severity::Medium,
            format!(
                "Kafka ZooKeeper migration {} still has legacy ZooKeeper dependencies",
                item.migration_id
            ),
            json!({
                "migration_id": item.migration_id,
                "legacy_zookeeper_dependency_count": item.legacy_zookeeper_dependency_count,
                "recommendation": "Retire or migrate remaining ZooKeeper dependencies before accepting migration savings",
            }),
        ));
    }
}

fn evaluate_resilience(
    item: &ZooKeeperMigrationInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if !item.has_migration_evidence {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_NO_EVIDENCE,
            Severity::High,
            format!(
                "Kafka ZooKeeper migration inventory for cluster {} has no resilience evidence",
                item.cluster_name
            ),
            json!({
                "migration_id": item.migration_id,
                "cluster_name": item.cluster_name,
                "recommendation": "Collect migration phase, metadata migration status, rollback plan, dependency graph, and quorum compatibility evidence before accepting resilience posture",
            }),
        ));
    }

    if item.has_migration_evidence && !item.metadata_migration_evidence {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_NO_METADATA_MIGRATION_EVIDENCE,
            Severity::High,
            format!(
                "Kafka ZooKeeper migration {} has no metadata migration evidence",
                item.migration_id
            ),
            json!({
                "migration_id": item.migration_id,
                "migration_phase": item.migration_phase,
                "recommendation": "Record metadata migration readiness and validation evidence before accepting migration resilience posture",
            }),
        ));
    }

    if item.has_migration_evidence && !item.rollback_plan_evidence {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_NO_ROLLBACK_PLAN,
            Severity::High,
            format!(
                "Kafka ZooKeeper migration {} has no rollback plan evidence",
                item.migration_id
            ),
            json!({
                "migration_id": item.migration_id,
                "migration_phase": item.migration_phase,
                "recommendation": "Record rollback or recovery notes before approving ZooKeeper migration changes",
            }),
        ));
    }
}

fn evaluate_security(
    item: &ZooKeeperMigrationInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if !item.has_migration_evidence {
        findings.push(finding(
            item,
            pillar,
            REASON_SEC_NO_EVIDENCE,
            Severity::High,
            format!(
                "Kafka ZooKeeper migration inventory for cluster {} has no security evidence",
                item.cluster_name
            ),
            json!({
                "migration_id": item.migration_id,
                "cluster_name": item.cluster_name,
                "recommendation": "Collect ZooKeeper ACLs, Kafka ACL migration evidence, listener security, credentials scope, and audit evidence before accepting security posture",
            }),
        ));
    }

    if item.has_migration_evidence && !item.acl_migration_evidence {
        findings.push(finding(
            item,
            pillar,
            REASON_SEC_NO_ACL_MIGRATION_EVIDENCE,
            Severity::High,
            format!(
                "Kafka ZooKeeper migration {} has no ACL migration evidence",
                item.migration_id
            ),
            json!({
                "migration_id": item.migration_id,
                "zookeeper_connect": item.zookeeper_connect,
                "recommendation": "Record ZooKeeper ACL and Kafka ACL migration evidence before accepting security posture",
            }),
        ));
    }

    if item.plaintext_connection {
        findings.push(finding(
            item,
            pillar,
            REASON_SEC_PLAINTEXT_CONNECTION,
            Severity::High,
            format!(
                "Kafka ZooKeeper migration {} uses PLAINTEXT or unverified transport",
                item.migration_id
            ),
            json!({
                "migration_id": item.migration_id,
                "plaintext_connection": item.plaintext_connection,
                "recommendation": "Use encrypted Kafka and ZooKeeper transport before accepting migration security posture",
            }),
        ));
    }
}

fn stale_finding(
    item: &ZooKeeperMigrationInventoryItem,
    pillar: Pillar,
    now: DateTime<Utc>,
) -> Option<InventoryFinding> {
    let age_hours = now
        .signed_duration_since(item.collected_at)
        .num_hours()
        .max(0);

    if age_hours <= DEFAULT_STALE_AFTER_HOURS {
        return None;
    }

    Some(finding(
        item,
        pillar,
        REASON_INV_STALE_DATA,
        Severity::High,
        format!(
            "Kafka ZooKeeper migration inventory for cluster {} is {} hour(s) old",
            item.cluster_name, age_hours
        ),
        json!({
            "migration_id": item.migration_id,
            "cluster_name": item.cluster_name,
            "age_hours": age_hours,
            "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
            "recommendation": "Refresh ZooKeeper migration inventory before acting on this posture report",
        }),
    ))
}

fn has_owner_metadata(item: &ZooKeeperMigrationInventoryItem) -> bool {
    item.owner
        .as_deref()
        .is_some_and(|owner| !owner.trim().is_empty())
        || COST_ALLOCATION_TAG_KEYS.iter().any(|key| {
            item.labels
                .get(*key)
                .or_else(|| item.labels.get(&key.to_ascii_lowercase()))
                .is_some_and(|value| !value.trim().is_empty())
        })
}

fn finding(
    item: &ZooKeeperMigrationInventoryItem,
    pillar: Pillar,
    reason_code: &'static str,
    severity: Severity,
    message: String,
    evidence: Value,
) -> InventoryFinding {
    InventoryFinding {
        resource_id: item.migration_id.clone(),
        arn: format!("kafka:zookeeper-migration/{}", item.migration_id),
        pillar,
        severity,
        reason_code: reason_code.to_string(),
        message,
        evidence,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    fn migration(now: DateTime<Utc>) -> ZooKeeperMigrationInventoryItem {
        ZooKeeperMigrationInventoryItem {
            migration_id: "prod:zookeeper-migration".to_string(),
            cluster_name: "prod".to_string(),
            zookeeper_connect: Some("zk-1:2181,zk-2:2181,zk-3:2181".to_string()),
            zookeeper_version: Some("3.8.4".to_string()),
            kafka_version: Some("3.7.0".to_string()),
            migration_phase: Some("dual-write".to_string()),
            legacy_zookeeper_dependency_count: Some(0),
            owner: Some("platform".to_string()),
            labels: BTreeMap::new(),
            metadata_migration_evidence: true,
            rollback_plan_evidence: true,
            acl_migration_evidence: true,
            plaintext_connection: false,
            has_migration_evidence: true,
            collected_at: now,
        }
    }

    #[test]
    fn healthy_zookeeper_migration_passes_claimed_pillars() {
        let now = Utc::now();
        let item = migration(now);

        for pillar in [Pillar::Cost, Pillar::Resilience, Pillar::Security] {
            let report =
                evaluate_zookeeper_migration_inventory(std::slice::from_ref(&item), pillar, now);
            assert_eq!(report.resources_evaluated, 1);
            assert!(report.findings.is_empty());
            assert_eq!(report.stale_resources, 0);
            assert_eq!(report.score, 100);
        }
    }

    #[test]
    fn cost_flags_missing_owner_missing_evidence_and_legacy_dependencies() {
        let now = Utc::now();
        let mut missing_evidence = migration(now);
        missing_evidence.owner = None;
        missing_evidence.has_migration_evidence = false;
        let mut legacy = migration(now);
        legacy.legacy_zookeeper_dependency_count = Some(2);

        let report =
            evaluate_zookeeper_migration_inventory(&[missing_evidence, legacy], Pillar::Cost, now);

        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_COST_OWNER_NOT_RECORDED));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_COST_NO_EVIDENCE));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_COST_LEGACY_DEPENDENCIES));
    }

    #[test]
    fn resilience_flags_missing_evidence_metadata_and_rollback_plan() {
        let now = Utc::now();
        let mut missing_evidence = migration(now);
        missing_evidence.has_migration_evidence = false;
        let mut risky = migration(now);
        risky.metadata_migration_evidence = false;
        risky.rollback_plan_evidence = false;

        let report = evaluate_zookeeper_migration_inventory(
            &[missing_evidence, risky],
            Pillar::Resilience,
            now,
        );

        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_RES_NO_EVIDENCE));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_RES_NO_METADATA_MIGRATION_EVIDENCE));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_RES_NO_ROLLBACK_PLAN));
    }

    #[test]
    fn security_flags_missing_evidence_acl_migration_and_plaintext() {
        let now = Utc::now();
        let mut missing_evidence = migration(now);
        missing_evidence.has_migration_evidence = false;
        let mut insecure = migration(now);
        insecure.acl_migration_evidence = false;
        insecure.plaintext_connection = true;

        let report = evaluate_zookeeper_migration_inventory(
            &[missing_evidence, insecure],
            Pillar::Security,
            now,
        );

        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_SEC_NO_EVIDENCE));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_SEC_NO_ACL_MIGRATION_EVIDENCE));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_SEC_PLAINTEXT_CONNECTION));
    }

    #[test]
    fn stale_zookeeper_migration_inventory_is_counted_for_any_pillar() {
        let now = Utc::now();
        let mut item = migration(now);
        item.collected_at = now - Duration::hours(DEFAULT_STALE_AFTER_HOURS + 2);

        let report = evaluate_zookeeper_migration_inventory(&[item], Pillar::Security, now);

        assert_eq!(report.stale_resources, 1);
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_INV_STALE_DATA));
    }
}

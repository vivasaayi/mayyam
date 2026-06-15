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

// Deterministic topic migration inventory evaluator for roadmap rows
// 04-KAFKA-DASHBOARD-MANAGEMENT-01422/01429/01450.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::config::KafkaClusterConfig;
use crate::services::aws::inventory::types::{
    score_pillar, InventoryFinding, Pillar, PillarReport, Severity, COST_ALLOCATION_TAG_KEYS,
    DEFAULT_STALE_AFTER_HOURS,
};

pub const RESOURCE_TYPE: &str = "KafkaTopicMigration";
pub const REASON_COST_OWNER_NOT_RECORDED: &str = "KAFKA_TOPIC_MIGRATION_COST_OWNER_NOT_RECORDED";
pub const REASON_COST_NO_EVIDENCE: &str = "KAFKA_TOPIC_MIGRATION_COST_NO_EVIDENCE";
pub const REASON_COST_HIGH_LAG: &str = "KAFKA_TOPIC_MIGRATION_COST_HIGH_LAG";
pub const REASON_RES_NO_EVIDENCE: &str = "KAFKA_TOPIC_MIGRATION_RES_NO_EVIDENCE";
pub const REASON_RES_NO_CUTOVER_PLAN: &str = "KAFKA_TOPIC_MIGRATION_RES_NO_CUTOVER_PLAN";
pub const REASON_RES_NO_VALIDATION: &str = "KAFKA_TOPIC_MIGRATION_RES_NO_VALIDATION";
pub const REASON_SEC_NO_EVIDENCE: &str = "KAFKA_TOPIC_MIGRATION_SEC_NO_EVIDENCE";
pub const REASON_SEC_NO_ACL_MIGRATION_EVIDENCE: &str =
    "KAFKA_TOPIC_MIGRATION_SEC_NO_ACL_MIGRATION_EVIDENCE";
pub const REASON_SEC_PLAINTEXT_CONNECTION: &str = "KAFKA_TOPIC_MIGRATION_SEC_PLAINTEXT_CONNECTION";
pub const REASON_INV_STALE_DATA: &str = "KAFKA_TOPIC_MIGRATION_INV_STALE_DATA";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TopicMigrationInventoryItem {
    pub migration_id: String,
    pub cluster_name: String,
    pub source_topic: Option<String>,
    pub target_topic: Option<String>,
    pub source_cluster: Option<String>,
    pub target_cluster: Option<String>,
    pub replication_lag_messages: Option<i64>,
    pub owner: Option<String>,
    pub labels: BTreeMap<String, String>,
    pub cutover_plan_evidence: bool,
    pub rollback_plan_evidence: bool,
    pub data_validation_evidence: bool,
    pub acl_migration_evidence: bool,
    pub plaintext_connection: bool,
    pub has_migration_evidence: bool,
    pub collected_at: DateTime<Utc>,
}

pub fn evaluate_topic_migration_inventory(
    items: &[TopicMigrationInventoryItem],
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

pub fn topic_migration_inventory_item_from_config(
    cluster: &KafkaClusterConfig,
    collected_at: DateTime<Utc>,
) -> TopicMigrationInventoryItem {
    TopicMigrationInventoryItem {
        migration_id: format!("{}:topic-migration", cluster.name),
        cluster_name: cluster.name.clone(),
        source_topic: None,
        target_topic: None,
        source_cluster: Some(cluster.name.clone()),
        target_cluster: None,
        replication_lag_messages: None,
        owner: None,
        labels: BTreeMap::new(),
        cutover_plan_evidence: false,
        rollback_plan_evidence: false,
        data_validation_evidence: false,
        acl_migration_evidence: false,
        plaintext_connection: cluster.security_protocol.eq_ignore_ascii_case("PLAINTEXT"),
        has_migration_evidence: false,
        collected_at,
    }
}

fn evaluate_cost(
    item: &TopicMigrationInventoryItem,
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
                "Kafka topic migration inventory for cluster {} has no owner, team, project, or cost-center metadata",
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
                "Kafka topic migration inventory for cluster {} has no migration evidence",
                item.cluster_name
            ),
            json!({
                "migration_id": item.migration_id,
                "cluster_name": item.cluster_name,
                "recommendation": "Collect source and target topics, clusters, replication lag, ownership, cutover plan, rollback plan, and validation evidence before estimating migration cost posture",
            }),
        ));
    }

    if item
        .replication_lag_messages
        .is_some_and(|lag_messages| lag_messages > 1_000_000)
    {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_HIGH_LAG,
            Severity::Medium,
            format!(
                "Kafka topic migration {} has high replication lag",
                item.migration_id
            ),
            json!({
                "migration_id": item.migration_id,
                "replication_lag_messages": item.replication_lag_messages,
                "recommendation": "Reduce migration lag before scaling migration windows or extending dual-run cost",
            }),
        ));
    }
}

fn evaluate_resilience(
    item: &TopicMigrationInventoryItem,
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
                "Kafka topic migration inventory for cluster {} has no resilience evidence",
                item.cluster_name
            ),
            json!({
                "migration_id": item.migration_id,
                "cluster_name": item.cluster_name,
                "recommendation": "Collect cutover, rollback, lag, ordering, validation, source, and target evidence before accepting topic migration resilience posture",
            }),
        ));
    }

    if item.has_migration_evidence && (!item.cutover_plan_evidence || !item.rollback_plan_evidence)
    {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_NO_CUTOVER_PLAN,
            Severity::High,
            format!(
                "Kafka topic migration {} has incomplete cutover or rollback evidence",
                item.migration_id
            ),
            json!({
                "migration_id": item.migration_id,
                "cutover_plan_evidence": item.cutover_plan_evidence,
                "rollback_plan_evidence": item.rollback_plan_evidence,
                "recommendation": "Record cutover and rollback evidence before approving topic migration changes",
            }),
        ));
    }

    if item.has_migration_evidence && !item.data_validation_evidence {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_NO_VALIDATION,
            Severity::High,
            format!(
                "Kafka topic migration {} has no data validation evidence",
                item.migration_id
            ),
            json!({
                "migration_id": item.migration_id,
                "source_topic": item.source_topic,
                "target_topic": item.target_topic,
                "recommendation": "Record message-count, offset, schema, ordering, and consumer validation before accepting migration readiness",
            }),
        ));
    }
}

fn evaluate_security(
    item: &TopicMigrationInventoryItem,
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
                "Kafka topic migration inventory for cluster {} has no security evidence",
                item.cluster_name
            ),
            json!({
                "migration_id": item.migration_id,
                "cluster_name": item.cluster_name,
                "recommendation": "Collect source and target ACLs, credentials scope, transport, audit, and approval evidence before accepting migration security posture",
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
                "Kafka topic migration {} has no ACL migration evidence",
                item.migration_id
            ),
            json!({
                "migration_id": item.migration_id,
                "source_topic": item.source_topic,
                "target_topic": item.target_topic,
                "recommendation": "Record source and target ACL migration evidence before accepting security posture",
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
                "Kafka topic migration {} uses PLAINTEXT or unverified transport",
                item.migration_id
            ),
            json!({
                "migration_id": item.migration_id,
                "plaintext_connection": item.plaintext_connection,
                "recommendation": "Use encrypted source and target Kafka transport before accepting migration security posture",
            }),
        ));
    }
}

fn stale_finding(
    item: &TopicMigrationInventoryItem,
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
            "Kafka topic migration inventory for cluster {} is {} hour(s) old",
            item.cluster_name, age_hours
        ),
        json!({
            "migration_id": item.migration_id,
            "cluster_name": item.cluster_name,
            "age_hours": age_hours,
            "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
            "recommendation": "Refresh Kafka topic migration inventory before acting on this posture report",
        }),
    ))
}

fn has_owner_metadata(item: &TopicMigrationInventoryItem) -> bool {
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
    item: &TopicMigrationInventoryItem,
    pillar: Pillar,
    reason_code: &'static str,
    severity: Severity,
    message: String,
    evidence: Value,
) -> InventoryFinding {
    InventoryFinding {
        resource_id: item.migration_id.clone(),
        arn: format!("kafka:topic-migration/{}", item.migration_id),
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

    fn migration(now: DateTime<Utc>) -> TopicMigrationInventoryItem {
        TopicMigrationInventoryItem {
            migration_id: "prod:topic-migration:orders".to_string(),
            cluster_name: "prod".to_string(),
            source_topic: Some("orders-v1".to_string()),
            target_topic: Some("orders-v2".to_string()),
            source_cluster: Some("prod".to_string()),
            target_cluster: Some("prod-next".to_string()),
            replication_lag_messages: Some(100),
            owner: Some("platform".to_string()),
            labels: BTreeMap::new(),
            cutover_plan_evidence: true,
            rollback_plan_evidence: true,
            data_validation_evidence: true,
            acl_migration_evidence: true,
            plaintext_connection: false,
            has_migration_evidence: true,
            collected_at: now,
        }
    }

    #[test]
    fn healthy_topic_migration_passes_claimed_pillars() {
        let now = Utc::now();
        let item = migration(now);

        for pillar in [Pillar::Cost, Pillar::Resilience, Pillar::Security] {
            let report =
                evaluate_topic_migration_inventory(std::slice::from_ref(&item), pillar, now);
            assert_eq!(report.resources_evaluated, 1);
            assert!(report.findings.is_empty());
            assert_eq!(report.stale_resources, 0);
            assert_eq!(report.score, 100);
        }
    }

    #[test]
    fn cost_flags_missing_owner_missing_evidence_and_high_lag() {
        let now = Utc::now();
        let mut missing_evidence = migration(now);
        missing_evidence.owner = None;
        missing_evidence.has_migration_evidence = false;
        let mut high_lag = migration(now);
        high_lag.replication_lag_messages = Some(1_000_001);

        let report =
            evaluate_topic_migration_inventory(&[missing_evidence, high_lag], Pillar::Cost, now);

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
            .any(|finding| finding.reason_code == REASON_COST_HIGH_LAG));
    }

    #[test]
    fn resilience_flags_missing_evidence_cutover_and_validation() {
        let now = Utc::now();
        let mut missing_evidence = migration(now);
        missing_evidence.has_migration_evidence = false;
        let mut risky = migration(now);
        risky.cutover_plan_evidence = false;
        risky.rollback_plan_evidence = false;
        risky.data_validation_evidence = false;

        let report =
            evaluate_topic_migration_inventory(&[missing_evidence, risky], Pillar::Resilience, now);

        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_RES_NO_EVIDENCE));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_RES_NO_CUTOVER_PLAN));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_RES_NO_VALIDATION));
    }

    #[test]
    fn security_flags_missing_evidence_acl_migration_and_plaintext() {
        let now = Utc::now();
        let mut missing_evidence = migration(now);
        missing_evidence.has_migration_evidence = false;
        let mut insecure = migration(now);
        insecure.acl_migration_evidence = false;
        insecure.plaintext_connection = true;

        let report = evaluate_topic_migration_inventory(
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
    fn stale_topic_migration_inventory_is_counted_for_any_pillar() {
        let now = Utc::now();
        let mut item = migration(now);
        item.collected_at = now - Duration::hours(DEFAULT_STALE_AFTER_HOURS + 2);

        let report = evaluate_topic_migration_inventory(&[item], Pillar::Security, now);

        assert_eq!(report.stale_resources, 1);
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_INV_STALE_DATA));
    }
}

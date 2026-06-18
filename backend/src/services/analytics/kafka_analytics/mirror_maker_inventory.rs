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

// Deterministic MirrorMaker 2 inventory evaluator for roadmap rows
// 04-KAFKA-DASHBOARD-MANAGEMENT-01128/01135/01156.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::config::KafkaClusterConfig;
use crate::services::aws::inventory::types::{
    score_pillar, InventoryFinding, Pillar, PillarReport, Severity, COST_ALLOCATION_TAG_KEYS,
    DEFAULT_STALE_AFTER_HOURS,
};

pub const RESOURCE_TYPE: &str = "KafkaMirrorMaker2";
pub const REASON_COST_OWNER_NOT_RECORDED: &str = "KAFKA_MM2_COST_OWNER_NOT_RECORDED";
pub const REASON_COST_NO_EVIDENCE: &str = "KAFKA_MM2_COST_NO_EVIDENCE";
pub const REASON_COST_HIGH_TASK_COUNT: &str = "KAFKA_MM2_COST_HIGH_TASK_COUNT";
pub const REASON_RES_NO_EVIDENCE: &str = "KAFKA_MM2_RES_NO_EVIDENCE";
pub const REASON_RES_NO_FLOW_EVIDENCE: &str = "KAFKA_MM2_RES_NO_FLOW_EVIDENCE";
pub const REASON_RES_HIGH_REPLICATION_LAG: &str = "KAFKA_MM2_RES_HIGH_REPLICATION_LAG";
pub const REASON_SEC_NO_EVIDENCE: &str = "KAFKA_MM2_SEC_NO_EVIDENCE";
pub const REASON_SEC_NO_PRINCIPAL: &str = "KAFKA_MM2_SEC_NO_PRINCIPAL";
pub const REASON_SEC_PLAINTEXT_CONNECTION: &str = "KAFKA_MM2_SEC_PLAINTEXT_CONNECTION";
pub const REASON_INV_STALE_DATA: &str = "KAFKA_MM2_INV_STALE_DATA";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MirrorMakerInventoryItem {
    pub replication_id: String,
    pub source_cluster_name: String,
    pub target_cluster_name: Option<String>,
    pub connector_name: String,
    pub source_alias: Option<String>,
    pub target_alias: Option<String>,
    pub replicated_topics: Vec<String>,
    pub task_count: Option<u32>,
    pub max_replication_lag_messages: Option<u64>,
    pub owner: Option<String>,
    pub labels: BTreeMap<String, String>,
    pub replication_flow_evidence: bool,
    pub principal: Option<String>,
    pub plaintext_connection: bool,
    pub has_mirror_maker_evidence: bool,
    pub collected_at: DateTime<Utc>,
}

pub fn evaluate_mirror_maker_inventory(
    items: &[MirrorMakerInventoryItem],
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

pub fn mirror_maker_inventory_item_from_config(
    cluster: &KafkaClusterConfig,
    collected_at: DateTime<Utc>,
) -> MirrorMakerInventoryItem {
    MirrorMakerInventoryItem {
        replication_id: format!("{}:mirror-maker-2", cluster.name),
        source_cluster_name: cluster.name.clone(),
        target_cluster_name: None,
        connector_name: "<uncollected>".to_string(),
        source_alias: None,
        target_alias: None,
        replicated_topics: Vec::new(),
        task_count: None,
        max_replication_lag_messages: None,
        owner: None,
        labels: BTreeMap::new(),
        replication_flow_evidence: false,
        principal: None,
        plaintext_connection: cluster.security_protocol.eq_ignore_ascii_case("PLAINTEXT"),
        has_mirror_maker_evidence: false,
        collected_at,
    }
}

fn evaluate_cost(
    item: &MirrorMakerInventoryItem,
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
                "MirrorMaker 2 inventory for source cluster {} has no owner, team, project, or cost-center metadata",
                item.source_cluster_name
            ),
            json!({
                "replication_id": item.replication_id,
                "source_cluster_name": item.source_cluster_name,
                "checked_keys": COST_ALLOCATION_TAG_KEYS,
            }),
        ));
    }

    if !item.has_mirror_maker_evidence {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_NO_EVIDENCE,
            Severity::High,
            format!(
                "MirrorMaker 2 inventory for source cluster {} has no replication evidence",
                item.source_cluster_name
            ),
            json!({
                "replication_id": item.replication_id,
                "source_cluster_name": item.source_cluster_name,
                "recommendation": "Collect MirrorMaker connector names, source/target aliases, replicated topics, tasks, lag, ownership, labels, principal, and transport evidence before estimating replication cost posture",
            }),
        ));
    }

    if item.task_count.is_some_and(|task_count| task_count >= 64) {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_HIGH_TASK_COUNT,
            Severity::Medium,
            format!("MirrorMaker 2 replication {} has a high task count", item.replication_id),
            json!({
                "replication_id": item.replication_id,
                "task_count": item.task_count,
                "recommendation": "Review MirrorMaker task parallelism, replicated topic set, and cluster throughput before scaling workers",
            }),
        ));
    }
}

fn evaluate_resilience(
    item: &MirrorMakerInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if !item.has_mirror_maker_evidence {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_NO_EVIDENCE,
            Severity::High,
            format!(
                "MirrorMaker 2 inventory for source cluster {} has no resilience evidence",
                item.source_cluster_name
            ),
            json!({
                "replication_id": item.replication_id,
                "source_cluster_name": item.source_cluster_name,
                "recommendation": "Collect replication flows, task state, lag, checkpoints, and offset sync evidence before accepting recovery posture",
            }),
        ));
    }

    if item.has_mirror_maker_evidence && !item.replication_flow_evidence {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_NO_FLOW_EVIDENCE,
            Severity::High,
            format!(
                "MirrorMaker 2 replication {} has no replication flow evidence",
                item.replication_id
            ),
            json!({
                "replication_id": item.replication_id,
                "replication_flow_evidence": item.replication_flow_evidence,
                "recommendation": "Collect source alias, target alias, topic flow, checkpoint, heartbeat, and offset sync evidence",
            }),
        ));
    }

    if item
        .max_replication_lag_messages
        .is_some_and(|lag| lag >= 100_000)
    {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_HIGH_REPLICATION_LAG,
            Severity::High,
            format!(
                "MirrorMaker 2 replication {} has high replication lag",
                item.replication_id
            ),
            json!({
                "replication_id": item.replication_id,
                "max_replication_lag_messages": item.max_replication_lag_messages,
                "recommendation": "Reduce replication lag before accepting cross-cluster recovery posture",
            }),
        ));
    }
}

fn evaluate_security(
    item: &MirrorMakerInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if !item.has_mirror_maker_evidence {
        findings.push(finding(
            item,
            pillar,
            REASON_SEC_NO_EVIDENCE,
            Severity::High,
            format!(
                "MirrorMaker 2 inventory for source cluster {} has no security evidence",
                item.source_cluster_name
            ),
            json!({
                "replication_id": item.replication_id,
                "source_cluster_name": item.source_cluster_name,
                "recommendation": "Collect principal, ACL scope, source and target transport, ownership, and audit evidence before accepting security posture",
            }),
        ));
    }

    if item.has_mirror_maker_evidence
        && item
            .principal
            .as_deref()
            .is_none_or(|principal| principal.trim().is_empty())
    {
        findings.push(finding(
            item,
            pillar,
            REASON_SEC_NO_PRINCIPAL,
            Severity::High,
            format!(
                "MirrorMaker 2 replication {} has no principal evidence",
                item.replication_id
            ),
            json!({
                "replication_id": item.replication_id,
                "principal": item.principal,
                "recommendation": "Record the service principal and least-privilege ACL scope used by MirrorMaker",
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
                "MirrorMaker 2 replication {} uses PLAINTEXT or unverified transport",
                item.replication_id
            ),
            json!({
                "replication_id": item.replication_id,
                "plaintext_connection": item.plaintext_connection,
                "recommendation": "Use encrypted transport for source, target, and worker communication before accepting security posture",
            }),
        ));
    }
}

fn stale_finding(
    item: &MirrorMakerInventoryItem,
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
            "MirrorMaker 2 inventory for source cluster {} is {} hour(s) old",
            item.source_cluster_name, age_hours
        ),
        json!({
            "replication_id": item.replication_id,
            "source_cluster_name": item.source_cluster_name,
            "age_hours": age_hours,
            "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
            "recommendation": "Refresh MirrorMaker 2 inventory before acting on this posture report",
        }),
    ))
}

fn has_owner_metadata(item: &MirrorMakerInventoryItem) -> bool {
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
    item: &MirrorMakerInventoryItem,
    pillar: Pillar,
    reason_code: &'static str,
    severity: Severity,
    message: String,
    evidence: Value,
) -> InventoryFinding {
    InventoryFinding {
        resource_id: item.replication_id.clone(),
        arn: format!("kafka:mirror-maker-2/{}", item.replication_id),
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

    fn replication(now: DateTime<Utc>) -> MirrorMakerInventoryItem {
        MirrorMakerInventoryItem {
            replication_id: "prod:mirror-maker-2:dr".to_string(),
            source_cluster_name: "prod".to_string(),
            target_cluster_name: Some("dr".to_string()),
            connector_name: "prod-to-dr".to_string(),
            source_alias: Some("prod".to_string()),
            target_alias: Some("dr".to_string()),
            replicated_topics: vec!["orders".to_string()],
            task_count: Some(8),
            max_replication_lag_messages: Some(100),
            owner: Some("platform".to_string()),
            labels: BTreeMap::new(),
            replication_flow_evidence: true,
            principal: Some("User:mm2".to_string()),
            plaintext_connection: false,
            has_mirror_maker_evidence: true,
            collected_at: now,
        }
    }

    #[test]
    fn healthy_replication_passes_claimed_pillars() {
        let now = Utc::now();
        let item = replication(now);

        for pillar in [Pillar::Cost, Pillar::Resilience, Pillar::Security] {
            let report = evaluate_mirror_maker_inventory(std::slice::from_ref(&item), pillar, now);
            assert_eq!(report.resources_evaluated, 1);
            assert!(report.findings.is_empty());
            assert_eq!(report.stale_resources, 0);
            assert_eq!(report.score, 100);
        }
    }

    #[test]
    fn cost_flags_missing_owner_missing_evidence_and_high_task_count() {
        let now = Utc::now();
        let mut missing_evidence = replication(now);
        missing_evidence.owner = None;
        missing_evidence.has_mirror_maker_evidence = false;
        let mut large = replication(now);
        large.task_count = Some(64);

        let report = evaluate_mirror_maker_inventory(&[missing_evidence, large], Pillar::Cost, now);

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
            .any(|finding| finding.reason_code == REASON_COST_HIGH_TASK_COUNT));
    }

    #[test]
    fn resilience_flags_missing_evidence_flow_evidence_and_lag() {
        let now = Utc::now();
        let mut missing_evidence = replication(now);
        missing_evidence.has_mirror_maker_evidence = false;
        let mut risky = replication(now);
        risky.replication_flow_evidence = false;
        risky.max_replication_lag_messages = Some(100_000);

        let report =
            evaluate_mirror_maker_inventory(&[missing_evidence, risky], Pillar::Resilience, now);

        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_RES_NO_EVIDENCE));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_RES_NO_FLOW_EVIDENCE));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_RES_HIGH_REPLICATION_LAG));
    }

    #[test]
    fn security_flags_missing_evidence_principal_and_plaintext() {
        let now = Utc::now();
        let mut missing_evidence = replication(now);
        missing_evidence.has_mirror_maker_evidence = false;
        let mut insecure = replication(now);
        insecure.principal = Some(" ".to_string());
        insecure.plaintext_connection = true;

        let report =
            evaluate_mirror_maker_inventory(&[missing_evidence, insecure], Pillar::Security, now);

        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_SEC_NO_EVIDENCE));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_SEC_NO_PRINCIPAL));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_SEC_PLAINTEXT_CONNECTION));
    }

    #[test]
    fn stale_mirror_maker_inventory_is_counted_for_any_pillar() {
        let now = Utc::now();
        let mut item = replication(now);
        item.collected_at = now - Duration::hours(DEFAULT_STALE_AFTER_HOURS + 2);

        let report = evaluate_mirror_maker_inventory(&[item], Pillar::Security, now);

        assert_eq!(report.stale_resources, 1);
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_INV_STALE_DATA));
    }
}

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

// Deterministic Kafka Streams inventory evaluator for roadmap rows
// 04-KAFKA-DASHBOARD-MANAGEMENT-01079/01086/01107.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::config::KafkaClusterConfig;
use crate::services::aws::inventory::types::{
    score_pillar, InventoryFinding, Pillar, PillarReport, Severity, COST_ALLOCATION_TAG_KEYS,
    DEFAULT_STALE_AFTER_HOURS,
};

pub const RESOURCE_TYPE: &str = "KafkaStreamsApplication";
pub const REASON_COST_OWNER_NOT_RECORDED: &str = "KAFKA_STREAMS_COST_OWNER_NOT_RECORDED";
pub const REASON_COST_NO_EVIDENCE: &str = "KAFKA_STREAMS_COST_NO_EVIDENCE";
pub const REASON_COST_HIGH_THREAD_COUNT: &str = "KAFKA_STREAMS_COST_HIGH_THREAD_COUNT";
pub const REASON_RES_NO_EVIDENCE: &str = "KAFKA_STREAMS_RES_NO_EVIDENCE";
pub const REASON_RES_NO_STATE_STORE_EVIDENCE: &str = "KAFKA_STREAMS_RES_NO_STATE_STORE_EVIDENCE";
pub const REASON_RES_STANDBY_REPLICAS_MISSING: &str = "KAFKA_STREAMS_RES_STANDBY_REPLICAS_MISSING";
pub const REASON_SEC_NO_EVIDENCE: &str = "KAFKA_STREAMS_SEC_NO_EVIDENCE";
pub const REASON_SEC_NO_APP_ID: &str = "KAFKA_STREAMS_SEC_NO_APP_ID";
pub const REASON_SEC_PLAINTEXT_CONNECTION: &str = "KAFKA_STREAMS_SEC_PLAINTEXT_CONNECTION";
pub const REASON_INV_STALE_DATA: &str = "KAFKA_STREAMS_INV_STALE_DATA";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KafkaStreamsInventoryItem {
    pub stream_id: String,
    pub cluster_name: String,
    pub application_id: Option<String>,
    pub application_name: String,
    pub topology_name: Option<String>,
    pub input_topics: Vec<String>,
    pub output_topics: Vec<String>,
    pub thread_count: Option<u32>,
    pub standby_replica_count: Option<u32>,
    pub state_store_count: Option<u32>,
    pub owner: Option<String>,
    pub labels: BTreeMap<String, String>,
    pub state_store_evidence: bool,
    pub plaintext_connection: bool,
    pub has_streams_evidence: bool,
    pub collected_at: DateTime<Utc>,
}

pub fn evaluate_kafka_streams_inventory(
    items: &[KafkaStreamsInventoryItem],
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

pub fn streams_inventory_item_from_config(
    cluster: &KafkaClusterConfig,
    collected_at: DateTime<Utc>,
) -> KafkaStreamsInventoryItem {
    KafkaStreamsInventoryItem {
        stream_id: format!("{}:streams", cluster.name),
        cluster_name: cluster.name.clone(),
        application_id: None,
        application_name: "<uncollected>".to_string(),
        topology_name: None,
        input_topics: Vec::new(),
        output_topics: Vec::new(),
        thread_count: None,
        standby_replica_count: None,
        state_store_count: None,
        owner: None,
        labels: BTreeMap::new(),
        state_store_evidence: false,
        plaintext_connection: cluster.security_protocol.eq_ignore_ascii_case("PLAINTEXT"),
        has_streams_evidence: false,
        collected_at,
    }
}

fn evaluate_cost(
    item: &KafkaStreamsInventoryItem,
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
                "Kafka Streams inventory for cluster {} has no owner, team, project, or cost-center metadata",
                item.cluster_name
            ),
            json!({
                "stream_id": item.stream_id,
                "cluster_name": item.cluster_name,
                "checked_keys": COST_ALLOCATION_TAG_KEYS,
            }),
        ));
    }

    if !item.has_streams_evidence {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_NO_EVIDENCE,
            Severity::High,
            format!(
                "Kafka Streams inventory for cluster {} has no application evidence",
                item.cluster_name
            ),
            json!({
                "stream_id": item.stream_id,
                "cluster_name": item.cluster_name,
                "recommendation": "Collect Streams application ID, topology, topic dependencies, thread counts, state stores, ownership, labels, and runtime evidence before estimating cost posture",
            }),
        ));
    }

    if item
        .thread_count
        .is_some_and(|thread_count| thread_count >= 64)
    {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_HIGH_THREAD_COUNT,
            Severity::Medium,
            format!("Kafka Streams application {} has a high thread count", item.stream_id),
            json!({
                "stream_id": item.stream_id,
                "thread_count": item.thread_count,
                "application_id": item.application_id,
                "recommendation": "Review Streams thread parallelism, partitions, and instance count before scaling compute",
            }),
        ));
    }
}

fn evaluate_resilience(
    item: &KafkaStreamsInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if !item.has_streams_evidence {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_NO_EVIDENCE,
            Severity::High,
            format!(
                "Kafka Streams inventory for cluster {} has no resilience evidence",
                item.cluster_name
            ),
            json!({
                "stream_id": item.stream_id,
                "cluster_name": item.cluster_name,
                "recommendation": "Collect Streams topology, task assignment, state store, standby replica, and changelog evidence before accepting recovery posture",
            }),
        ));
    }

    if item.has_streams_evidence && !item.state_store_evidence {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_NO_STATE_STORE_EVIDENCE,
            Severity::High,
            format!(
                "Kafka Streams application {} has no state store evidence",
                item.stream_id
            ),
            json!({
                "stream_id": item.stream_id,
                "state_store_evidence": item.state_store_evidence,
                "recommendation": "Collect state store, changelog topic, and restore evidence before accepting recovery posture",
            }),
        ));
    }

    if item.has_streams_evidence
        && item
            .standby_replica_count
            .is_some_and(|standby_replica_count| standby_replica_count == 0)
        && item
            .state_store_count
            .is_some_and(|state_store_count| state_store_count > 0)
    {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_STANDBY_REPLICAS_MISSING,
            Severity::High,
            format!(
                "Kafka Streams application {} has state stores without standby replicas",
                item.stream_id
            ),
            json!({
                "stream_id": item.stream_id,
                "standby_replica_count": item.standby_replica_count,
                "state_store_count": item.state_store_count,
                "recommendation": "Configure standby replicas or document restore objectives for stateful Streams applications",
            }),
        ));
    }
}

fn evaluate_security(
    item: &KafkaStreamsInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if !item.has_streams_evidence {
        findings.push(finding(
            item,
            pillar,
            REASON_SEC_NO_EVIDENCE,
            Severity::High,
            format!(
                "Kafka Streams inventory for cluster {} has no security evidence",
                item.cluster_name
            ),
            json!({
                "stream_id": item.stream_id,
                "cluster_name": item.cluster_name,
                "recommendation": "Collect application ID, principal, topic dependency, transport, ownership, and audit evidence before accepting security posture",
            }),
        ));
    }

    if item.has_streams_evidence
        && item
            .application_id
            .as_deref()
            .is_none_or(|application_id| application_id.trim().is_empty())
    {
        findings.push(finding(
            item,
            pillar,
            REASON_SEC_NO_APP_ID,
            Severity::High,
            format!(
                "Kafka Streams application {} has no stable application ID evidence",
                item.stream_id
            ),
            json!({
                "stream_id": item.stream_id,
                "application_id": item.application_id,
                "recommendation": "Record stable Streams application IDs so consumer groups, state stores, changelog topics, and ACLs can be governed",
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
                "Kafka Streams application {} uses PLAINTEXT or unverified transport",
                item.stream_id
            ),
            json!({
                "stream_id": item.stream_id,
                "plaintext_connection": item.plaintext_connection,
                "recommendation": "Use encrypted Streams client and Kafka transport before accepting security posture",
            }),
        ));
    }
}

fn stale_finding(
    item: &KafkaStreamsInventoryItem,
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
            "Kafka Streams inventory for cluster {} is {} hour(s) old",
            item.cluster_name, age_hours
        ),
        json!({
            "stream_id": item.stream_id,
            "cluster_name": item.cluster_name,
            "age_hours": age_hours,
            "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
            "recommendation": "Refresh Kafka Streams inventory before acting on this posture report",
        }),
    ))
}

fn has_owner_metadata(item: &KafkaStreamsInventoryItem) -> bool {
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
    item: &KafkaStreamsInventoryItem,
    pillar: Pillar,
    reason_code: &'static str,
    severity: Severity,
    message: String,
    evidence: Value,
) -> InventoryFinding {
    InventoryFinding {
        resource_id: item.stream_id.clone(),
        arn: format!("kafka:streams/{}", item.stream_id),
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

    fn streams_app(now: DateTime<Utc>) -> KafkaStreamsInventoryItem {
        KafkaStreamsInventoryItem {
            stream_id: "prod:streams:orders".to_string(),
            cluster_name: "prod".to_string(),
            application_id: Some("orders-streams".to_string()),
            application_name: "orders".to_string(),
            topology_name: Some("orders-topology".to_string()),
            input_topics: vec!["orders".to_string()],
            output_topics: vec!["orders-enriched".to_string()],
            thread_count: Some(8),
            standby_replica_count: Some(1),
            state_store_count: Some(2),
            owner: Some("orders".to_string()),
            labels: BTreeMap::new(),
            state_store_evidence: true,
            plaintext_connection: false,
            has_streams_evidence: true,
            collected_at: now,
        }
    }

    #[test]
    fn healthy_streams_app_passes_claimed_pillars() {
        let now = Utc::now();
        let item = streams_app(now);

        for pillar in [Pillar::Cost, Pillar::Resilience, Pillar::Security] {
            let report = evaluate_kafka_streams_inventory(std::slice::from_ref(&item), pillar, now);
            assert_eq!(report.resources_evaluated, 1);
            assert!(report.findings.is_empty());
            assert_eq!(report.stale_resources, 0);
            assert_eq!(report.score, 100);
        }
    }

    #[test]
    fn cost_flags_missing_owner_missing_evidence_and_high_thread_count() {
        let now = Utc::now();
        let mut missing_evidence = streams_app(now);
        missing_evidence.owner = None;
        missing_evidence.has_streams_evidence = false;
        let mut large = streams_app(now);
        large.thread_count = Some(64);

        let report =
            evaluate_kafka_streams_inventory(&[missing_evidence, large], Pillar::Cost, now);

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
            .any(|finding| finding.reason_code == REASON_COST_HIGH_THREAD_COUNT));
    }

    #[test]
    fn resilience_flags_missing_evidence_state_store_evidence_and_standby_replicas() {
        let now = Utc::now();
        let mut missing_evidence = streams_app(now);
        missing_evidence.has_streams_evidence = false;
        let mut risky = streams_app(now);
        risky.state_store_evidence = false;
        risky.standby_replica_count = Some(0);

        let report =
            evaluate_kafka_streams_inventory(&[missing_evidence, risky], Pillar::Resilience, now);

        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_RES_NO_EVIDENCE));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_RES_NO_STATE_STORE_EVIDENCE));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_RES_STANDBY_REPLICAS_MISSING));
    }

    #[test]
    fn security_flags_missing_evidence_missing_app_id_and_plaintext() {
        let now = Utc::now();
        let mut missing_evidence = streams_app(now);
        missing_evidence.has_streams_evidence = false;
        let mut insecure = streams_app(now);
        insecure.application_id = Some(" ".to_string());
        insecure.plaintext_connection = true;

        let report =
            evaluate_kafka_streams_inventory(&[missing_evidence, insecure], Pillar::Security, now);

        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_SEC_NO_EVIDENCE));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_SEC_NO_APP_ID));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_SEC_PLAINTEXT_CONNECTION));
    }

    #[test]
    fn stale_streams_inventory_is_counted_for_any_pillar() {
        let now = Utc::now();
        let mut item = streams_app(now);
        item.collected_at = now - Duration::hours(DEFAULT_STALE_AFTER_HOURS + 2);

        let report = evaluate_kafka_streams_inventory(&[item], Pillar::Security, now);

        assert_eq!(report.stale_resources, 1);
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_INV_STALE_DATA));
    }
}

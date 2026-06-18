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

// Deterministic tiered storage inventory evaluator for roadmap rows
// 04-KAFKA-DASHBOARD-MANAGEMENT-01177/01184/01205.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::config::KafkaClusterConfig;
use crate::services::aws::inventory::types::{
    score_pillar, InventoryFinding, Pillar, PillarReport, Severity, COST_ALLOCATION_TAG_KEYS,
    DEFAULT_STALE_AFTER_HOURS,
};

pub const RESOURCE_TYPE: &str = "KafkaTieredStorage";
pub const REASON_COST_OWNER_NOT_RECORDED: &str = "KAFKA_TIERED_STORAGE_COST_OWNER_NOT_RECORDED";
pub const REASON_COST_NO_EVIDENCE: &str = "KAFKA_TIERED_STORAGE_COST_NO_EVIDENCE";
pub const REASON_COST_TIERING_DISABLED: &str = "KAFKA_TIERED_STORAGE_COST_TIERING_DISABLED";
pub const REASON_RES_NO_EVIDENCE: &str = "KAFKA_TIERED_STORAGE_RES_NO_EVIDENCE";
pub const REASON_RES_NO_REMOTE_STORE_EVIDENCE: &str =
    "KAFKA_TIERED_STORAGE_RES_NO_REMOTE_STORE_EVIDENCE";
pub const REASON_RES_SHORT_LOCAL_RETENTION: &str = "KAFKA_TIERED_STORAGE_RES_SHORT_LOCAL_RETENTION";
pub const REASON_SEC_NO_EVIDENCE: &str = "KAFKA_TIERED_STORAGE_SEC_NO_EVIDENCE";
pub const REASON_SEC_NO_ENCRYPTION_EVIDENCE: &str =
    "KAFKA_TIERED_STORAGE_SEC_NO_ENCRYPTION_EVIDENCE";
pub const REASON_SEC_PLAINTEXT_CONNECTION: &str = "KAFKA_TIERED_STORAGE_SEC_PLAINTEXT_CONNECTION";
pub const REASON_INV_STALE_DATA: &str = "KAFKA_TIERED_STORAGE_INV_STALE_DATA";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TieredStorageInventoryItem {
    pub storage_id: String,
    pub cluster_name: String,
    pub topic_name: Option<String>,
    pub remote_storage_provider: Option<String>,
    pub remote_bucket: Option<String>,
    pub tiering_enabled: Option<bool>,
    pub local_retention_ms: Option<i64>,
    pub remote_retention_ms: Option<i64>,
    pub owner: Option<String>,
    pub labels: BTreeMap<String, String>,
    pub remote_store_evidence: bool,
    pub encryption_evidence: bool,
    pub plaintext_connection: bool,
    pub has_tiered_storage_evidence: bool,
    pub collected_at: DateTime<Utc>,
}

pub fn evaluate_tiered_storage_inventory(
    items: &[TieredStorageInventoryItem],
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

pub fn tiered_storage_inventory_item_from_config(
    cluster: &KafkaClusterConfig,
    collected_at: DateTime<Utc>,
) -> TieredStorageInventoryItem {
    TieredStorageInventoryItem {
        storage_id: format!("{}:tiered-storage", cluster.name),
        cluster_name: cluster.name.clone(),
        topic_name: None,
        remote_storage_provider: None,
        remote_bucket: None,
        tiering_enabled: None,
        local_retention_ms: None,
        remote_retention_ms: None,
        owner: None,
        labels: BTreeMap::new(),
        remote_store_evidence: false,
        encryption_evidence: false,
        plaintext_connection: cluster.security_protocol.eq_ignore_ascii_case("PLAINTEXT"),
        has_tiered_storage_evidence: false,
        collected_at,
    }
}

fn evaluate_cost(
    item: &TieredStorageInventoryItem,
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
                "Kafka tiered storage inventory for cluster {} has no owner, team, project, or cost-center metadata",
                item.cluster_name
            ),
            json!({
                "storage_id": item.storage_id,
                "cluster_name": item.cluster_name,
                "checked_keys": COST_ALLOCATION_TAG_KEYS,
            }),
        ));
    }

    if !item.has_tiered_storage_evidence {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_NO_EVIDENCE,
            Severity::High,
            format!(
                "Kafka tiered storage inventory for cluster {} has no tiering evidence",
                item.cluster_name
            ),
            json!({
                "storage_id": item.storage_id,
                "cluster_name": item.cluster_name,
                "recommendation": "Collect topic-level tiering status, remote storage provider, bucket, retention settings, ownership, and encryption evidence before estimating storage cost posture",
            }),
        ));
    }

    if item.has_tiered_storage_evidence && item.tiering_enabled == Some(false) {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_TIERING_DISABLED,
            Severity::Medium,
            format!("Kafka tiered storage {} is disabled", item.storage_id),
            json!({
                "storage_id": item.storage_id,
                "tiering_enabled": item.tiering_enabled,
                "topic_name": item.topic_name,
                "recommendation": "Review local disk usage and retention before leaving eligible topics without remote tiering",
            }),
        ));
    }
}

fn evaluate_resilience(
    item: &TieredStorageInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if !item.has_tiered_storage_evidence {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_NO_EVIDENCE,
            Severity::High,
            format!(
                "Kafka tiered storage inventory for cluster {} has no resilience evidence",
                item.cluster_name
            ),
            json!({
                "storage_id": item.storage_id,
                "cluster_name": item.cluster_name,
                "recommendation": "Collect remote store, object lifecycle, topic retention, restore, and fetch evidence before accepting recovery posture",
            }),
        ));
    }

    if item.has_tiered_storage_evidence && !item.remote_store_evidence {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_NO_REMOTE_STORE_EVIDENCE,
            Severity::High,
            format!(
                "Kafka tiered storage {} has no remote store evidence",
                item.storage_id
            ),
            json!({
                "storage_id": item.storage_id,
                "remote_storage_provider": item.remote_storage_provider,
                "remote_bucket": item.remote_bucket,
                "recommendation": "Collect remote object store, lifecycle, availability, and restore evidence before accepting resilience posture",
            }),
        ));
    }

    if item
        .local_retention_ms
        .is_some_and(|local_retention_ms| local_retention_ms < 3_600_000)
    {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_SHORT_LOCAL_RETENTION,
            Severity::Medium,
            format!(
                "Kafka tiered storage {} has very short local retention",
                item.storage_id
            ),
            json!({
                "storage_id": item.storage_id,
                "local_retention_ms": item.local_retention_ms,
                "remote_retention_ms": item.remote_retention_ms,
                "recommendation": "Verify remote fetch and restore objectives before reducing local retention below one hour",
            }),
        ));
    }
}

fn evaluate_security(
    item: &TieredStorageInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if !item.has_tiered_storage_evidence {
        findings.push(finding(
            item,
            pillar,
            REASON_SEC_NO_EVIDENCE,
            Severity::High,
            format!(
                "Kafka tiered storage inventory for cluster {} has no security evidence",
                item.cluster_name
            ),
            json!({
                "storage_id": item.storage_id,
                "cluster_name": item.cluster_name,
                "recommendation": "Collect remote storage identity, bucket policy, encryption, transport, ownership, and audit evidence before accepting security posture",
            }),
        ));
    }

    if item.has_tiered_storage_evidence && !item.encryption_evidence {
        findings.push(finding(
            item,
            pillar,
            REASON_SEC_NO_ENCRYPTION_EVIDENCE,
            Severity::High,
            format!(
                "Kafka tiered storage {} has no encryption evidence",
                item.storage_id
            ),
            json!({
                "storage_id": item.storage_id,
                "remote_storage_provider": item.remote_storage_provider,
                "remote_bucket": item.remote_bucket,
                "recommendation": "Record remote storage encryption and key policy evidence before accepting security posture",
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
                "Kafka tiered storage {} uses PLAINTEXT or unverified transport",
                item.storage_id
            ),
            json!({
                "storage_id": item.storage_id,
                "plaintext_connection": item.plaintext_connection,
                "recommendation": "Use encrypted broker and remote storage transport before accepting security posture",
            }),
        ));
    }
}

fn stale_finding(
    item: &TieredStorageInventoryItem,
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
            "Kafka tiered storage inventory for cluster {} is {} hour(s) old",
            item.cluster_name, age_hours
        ),
        json!({
            "storage_id": item.storage_id,
            "cluster_name": item.cluster_name,
            "age_hours": age_hours,
            "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
            "recommendation": "Refresh Kafka tiered storage inventory before acting on this posture report",
        }),
    ))
}

fn has_owner_metadata(item: &TieredStorageInventoryItem) -> bool {
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
    item: &TieredStorageInventoryItem,
    pillar: Pillar,
    reason_code: &'static str,
    severity: Severity,
    message: String,
    evidence: Value,
) -> InventoryFinding {
    InventoryFinding {
        resource_id: item.storage_id.clone(),
        arn: format!("kafka:tiered-storage/{}", item.storage_id),
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

    fn tiered_storage(now: DateTime<Utc>) -> TieredStorageInventoryItem {
        TieredStorageInventoryItem {
            storage_id: "prod:tiered-storage:orders".to_string(),
            cluster_name: "prod".to_string(),
            topic_name: Some("orders".to_string()),
            remote_storage_provider: Some("s3".to_string()),
            remote_bucket: Some("prod-kafka-tiered-storage".to_string()),
            tiering_enabled: Some(true),
            local_retention_ms: Some(86_400_000),
            remote_retention_ms: Some(2_592_000_000),
            owner: Some("platform".to_string()),
            labels: BTreeMap::new(),
            remote_store_evidence: true,
            encryption_evidence: true,
            plaintext_connection: false,
            has_tiered_storage_evidence: true,
            collected_at: now,
        }
    }

    #[test]
    fn healthy_tiered_storage_passes_claimed_pillars() {
        let now = Utc::now();
        let item = tiered_storage(now);

        for pillar in [Pillar::Cost, Pillar::Resilience, Pillar::Security] {
            let report =
                evaluate_tiered_storage_inventory(std::slice::from_ref(&item), pillar, now);
            assert_eq!(report.resources_evaluated, 1);
            assert!(report.findings.is_empty());
            assert_eq!(report.stale_resources, 0);
            assert_eq!(report.score, 100);
        }
    }

    #[test]
    fn cost_flags_missing_owner_missing_evidence_and_disabled_tiering() {
        let now = Utc::now();
        let mut missing_evidence = tiered_storage(now);
        missing_evidence.owner = None;
        missing_evidence.has_tiered_storage_evidence = false;
        let mut disabled = tiered_storage(now);
        disabled.tiering_enabled = Some(false);

        let report =
            evaluate_tiered_storage_inventory(&[missing_evidence, disabled], Pillar::Cost, now);

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
            .any(|finding| finding.reason_code == REASON_COST_TIERING_DISABLED));
    }

    #[test]
    fn resilience_flags_missing_evidence_remote_store_and_short_local_retention() {
        let now = Utc::now();
        let mut missing_evidence = tiered_storage(now);
        missing_evidence.has_tiered_storage_evidence = false;
        let mut risky = tiered_storage(now);
        risky.remote_store_evidence = false;
        risky.local_retention_ms = Some(3_599_999);

        let report =
            evaluate_tiered_storage_inventory(&[missing_evidence, risky], Pillar::Resilience, now);

        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_RES_NO_EVIDENCE));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_RES_NO_REMOTE_STORE_EVIDENCE));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_RES_SHORT_LOCAL_RETENTION));
    }

    #[test]
    fn security_flags_missing_evidence_encryption_and_plaintext() {
        let now = Utc::now();
        let mut missing_evidence = tiered_storage(now);
        missing_evidence.has_tiered_storage_evidence = false;
        let mut insecure = tiered_storage(now);
        insecure.encryption_evidence = false;
        insecure.plaintext_connection = true;

        let report =
            evaluate_tiered_storage_inventory(&[missing_evidence, insecure], Pillar::Security, now);

        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_SEC_NO_EVIDENCE));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_SEC_NO_ENCRYPTION_EVIDENCE));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_SEC_PLAINTEXT_CONNECTION));
    }

    #[test]
    fn stale_tiered_storage_inventory_is_counted_for_any_pillar() {
        let now = Utc::now();
        let mut item = tiered_storage(now);
        item.collected_at = now - Duration::hours(DEFAULT_STALE_AFTER_HOURS + 2);

        let report = evaluate_tiered_storage_inventory(&[item], Pillar::Security, now);

        assert_eq!(report.stale_resources, 1);
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_INV_STALE_DATA));
    }
}

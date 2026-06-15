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

// Deterministic Kafka retention policy inventory evaluator for roadmap rows
// 04-KAFKA-DASHBOARD-MANAGEMENT-00834/00841/00862.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::config::KafkaClusterConfig;
use crate::services::aws::inventory::types::{
    score_pillar, InventoryFinding, Pillar, PillarReport, Severity, COST_ALLOCATION_TAG_KEYS,
    DEFAULT_STALE_AFTER_HOURS,
};

pub const RESOURCE_TYPE: &str = "KafkaRetentionPolicy";
pub const REASON_COST_OWNER_NOT_RECORDED: &str = "KAFKA_RETENTION_POLICY_COST_OWNER_NOT_RECORDED";
pub const REASON_COST_NO_RETENTION_EVIDENCE: &str = "KAFKA_RETENTION_POLICY_COST_NO_EVIDENCE";
pub const REASON_COST_UNBOUNDED_RETENTION: &str = "KAFKA_RETENTION_POLICY_COST_UNBOUNDED";
pub const REASON_COST_HIGH_RETENTION_BYTES: &str = "KAFKA_RETENTION_POLICY_COST_HIGH_BYTES";
pub const REASON_RES_NO_RETENTION_EVIDENCE: &str = "KAFKA_RETENTION_POLICY_RES_NO_EVIDENCE";
pub const REASON_RES_NO_AUDIT_EVIDENCE: &str = "KAFKA_RETENTION_POLICY_RES_NO_AUDIT_EVIDENCE";
pub const REASON_RES_SHORT_RETENTION: &str = "KAFKA_RETENTION_POLICY_RES_SHORT_RETENTION";
pub const REASON_SEC_NO_RETENTION_EVIDENCE: &str = "KAFKA_RETENTION_POLICY_SEC_NO_EVIDENCE";
pub const REASON_SEC_DELETE_POLICY: &str = "KAFKA_RETENTION_POLICY_SEC_DELETE_POLICY";
pub const REASON_SEC_PLAINTEXT: &str = "KAFKA_RETENTION_POLICY_SEC_PLAINTEXT_PROTOCOL";
pub const REASON_INV_STALE_DATA: &str = "KAFKA_RETENTION_POLICY_INV_STALE_DATA";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KafkaRetentionPolicyInventoryItem {
    pub retention_policy_id: String,
    pub cluster_name: String,
    pub topic_name: String,
    pub cleanup_policy: String,
    pub retention_ms: Option<i64>,
    pub retention_bytes: Option<i64>,
    pub owner: Option<String>,
    pub labels: BTreeMap<String, String>,
    pub audit_evidence: bool,
    pub has_retention_evidence: bool,
    pub security_protocol: String,
    pub collected_at: DateTime<Utc>,
}

pub fn evaluate_kafka_retention_policy_inventory(
    items: &[KafkaRetentionPolicyInventoryItem],
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

pub fn retention_policy_inventory_item_from_config(
    cluster: &KafkaClusterConfig,
    collected_at: DateTime<Utc>,
) -> KafkaRetentionPolicyInventoryItem {
    KafkaRetentionPolicyInventoryItem {
        retention_policy_id: format!("{}:retention-policies", cluster.name),
        cluster_name: cluster.name.clone(),
        topic_name: "<uncollected>".to_string(),
        cleanup_policy: "<uncollected>".to_string(),
        retention_ms: None,
        retention_bytes: None,
        owner: None,
        labels: BTreeMap::new(),
        audit_evidence: false,
        has_retention_evidence: false,
        security_protocol: cluster.security_protocol.clone(),
        collected_at,
    }
}

fn evaluate_cost(
    item: &KafkaRetentionPolicyInventoryItem,
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
                "Kafka retention policy inventory for cluster {} has no owner, team, project, or cost-center metadata",
                item.cluster_name
            ),
            json!({
                "retention_policy_id": item.retention_policy_id,
                "cluster_name": item.cluster_name,
                "checked_keys": COST_ALLOCATION_TAG_KEYS,
            }),
        ));
    }

    if !item.has_retention_evidence {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_NO_RETENTION_EVIDENCE,
            Severity::High,
            format!(
                "Kafka retention policy inventory for cluster {} has no retention evidence",
                item.cluster_name
            ),
            json!({
                "retention_policy_id": item.retention_policy_id,
                "cluster_name": item.cluster_name,
                "recommendation": "Collect topic retention.ms, retention.bytes, cleanup policy, ownership, labels, and audit evidence before estimating retention cost posture",
            }),
        ));
    }

    if item.has_retention_evidence && item.retention_ms.is_none() && item.retention_bytes.is_none()
    {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_UNBOUNDED_RETENTION,
            Severity::High,
            format!(
                "Kafka retention policy {} has no effective retention bounds",
                item.retention_policy_id
            ),
            json!({
                "retention_policy_id": item.retention_policy_id,
                "retention_ms": item.retention_ms,
                "retention_bytes": item.retention_bytes,
                "recommendation": "Set explicit time or byte retention bounds before accepting storage cost posture",
            }),
        ));
    }

    if item
        .retention_bytes
        .is_some_and(|bytes| bytes >= 1_000_000_000_000)
    {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_HIGH_RETENTION_BYTES,
            Severity::Medium,
            format!(
                "Kafka retention policy {} has high byte retention",
                item.retention_policy_id
            ),
            json!({
                "retention_policy_id": item.retention_policy_id,
                "retention_bytes": item.retention_bytes,
                "recommendation": "Review high retention byte limits against recovery and analytics requirements before scaling broker storage",
            }),
        ));
    }
}

fn evaluate_resilience(
    item: &KafkaRetentionPolicyInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if !item.has_retention_evidence {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_NO_RETENTION_EVIDENCE,
            Severity::High,
            format!(
                "Kafka retention policy inventory for cluster {} has no resilience evidence",
                item.cluster_name
            ),
            json!({
                "retention_policy_id": item.retention_policy_id,
                "cluster_name": item.cluster_name,
                "recommendation": "Collect retention limits and audit evidence before relying on topic data availability during recovery",
            }),
        ));
    }

    if item.has_retention_evidence && !item.audit_evidence {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_NO_AUDIT_EVIDENCE,
            Severity::High,
            format!(
                "Kafka retention policy {} has no audit evidence",
                item.retention_policy_id
            ),
            json!({
                "retention_policy_id": item.retention_policy_id,
                "audit_evidence": item.audit_evidence,
                "recommendation": "Enable or collect retention policy change audit evidence before accepting recovery posture",
            }),
        ));
    }

    if item.has_retention_evidence
        && item
            .retention_ms
            .is_some_and(|retention_ms| retention_ms < 86_400_000)
    {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_SHORT_RETENTION,
            Severity::High,
            format!(
                "Kafka retention policy {} keeps data for less than one day",
                item.retention_policy_id
            ),
            json!({
                "retention_policy_id": item.retention_policy_id,
                "retention_ms": item.retention_ms,
                "recommendation": "Validate short retention against replay, incident investigation, and consumer recovery objectives",
            }),
        ));
    }
}

fn evaluate_security(
    item: &KafkaRetentionPolicyInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if !item.has_retention_evidence {
        findings.push(finding(
            item,
            pillar,
            REASON_SEC_NO_RETENTION_EVIDENCE,
            Severity::High,
            format!(
                "Kafka retention policy inventory for cluster {} has no retention evidence for security review",
                item.cluster_name
            ),
            json!({
                "retention_policy_id": item.retention_policy_id,
                "cluster_name": item.cluster_name,
                "recommendation": "Collect retention settings, cleanup policy, ownership, and audit evidence before assessing forensic data availability",
            }),
        ));
    }

    if item.has_retention_evidence && cleanup_policy_includes_delete(&item.cleanup_policy) {
        findings.push(finding(
            item,
            pillar,
            REASON_SEC_DELETE_POLICY,
            Severity::Medium,
            format!(
                "Kafka retention policy {} permits log deletion",
                item.retention_policy_id
            ),
            json!({
                "retention_policy_id": item.retention_policy_id,
                "cleanup_policy": item.cleanup_policy,
                "recommendation": "Confirm delete cleanup policies meet forensic retention and compliance requirements",
            }),
        ));
    }

    if item.security_protocol.eq_ignore_ascii_case("PLAINTEXT") {
        findings.push(finding(
            item,
            pillar,
            REASON_SEC_PLAINTEXT,
            Severity::High,
            format!(
                "Kafka retention policy inventory for cluster {} uses PLAINTEXT transport",
                item.cluster_name
            ),
            json!({
                "retention_policy_id": item.retention_policy_id,
                "security_protocol": item.security_protocol,
                "recommendation": "Use SSL or SASL_SSL before accepting retention policy evidence for sensitive topics",
            }),
        ));
    }
}

fn stale_finding(
    item: &KafkaRetentionPolicyInventoryItem,
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
            "Kafka retention policy inventory for cluster {} is {} hour(s) old",
            item.cluster_name, age_hours
        ),
        json!({
            "retention_policy_id": item.retention_policy_id,
            "cluster_name": item.cluster_name,
            "age_hours": age_hours,
            "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
            "recommendation": "Refresh Kafka retention policy inventory before acting on this posture report",
        }),
    ))
}

fn cleanup_policy_includes_delete(cleanup_policy: &str) -> bool {
    cleanup_policy
        .split(',')
        .map(str::trim)
        .any(|part| part.eq_ignore_ascii_case("delete"))
}

fn has_owner_metadata(item: &KafkaRetentionPolicyInventoryItem) -> bool {
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
    item: &KafkaRetentionPolicyInventoryItem,
    pillar: Pillar,
    reason_code: &'static str,
    severity: Severity,
    message: String,
    evidence: Value,
) -> InventoryFinding {
    InventoryFinding {
        resource_id: item.retention_policy_id.clone(),
        arn: format!("kafka:retention-policy/{}", item.retention_policy_id),
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

    fn retention(now: DateTime<Utc>) -> KafkaRetentionPolicyInventoryItem {
        KafkaRetentionPolicyInventoryItem {
            retention_policy_id: "prod:topic:orders:retention".to_string(),
            cluster_name: "prod".to_string(),
            topic_name: "orders".to_string(),
            cleanup_policy: "compact".to_string(),
            retention_ms: Some(604_800_000),
            retention_bytes: Some(100_000_000_000),
            owner: Some("orders".to_string()),
            labels: BTreeMap::new(),
            audit_evidence: true,
            has_retention_evidence: true,
            security_protocol: "SASL_SSL".to_string(),
            collected_at: now,
        }
    }

    #[test]
    fn healthy_retention_policy_passes_claimed_pillars() {
        let now = Utc::now();
        let item = retention(now);

        for pillar in [Pillar::Cost, Pillar::Resilience, Pillar::Security] {
            let report =
                evaluate_kafka_retention_policy_inventory(std::slice::from_ref(&item), pillar, now);
            assert_eq!(report.resources_evaluated, 1);
            assert!(report.findings.is_empty());
            assert_eq!(report.stale_resources, 0);
            assert_eq!(report.score, 100);
        }
    }

    #[test]
    fn cost_flags_missing_owner_missing_evidence_unbounded_and_high_bytes() {
        let now = Utc::now();
        let mut missing_evidence = retention(now);
        missing_evidence.owner = None;
        missing_evidence.has_retention_evidence = false;
        let mut expensive = retention(now);
        expensive.retention_ms = None;
        expensive.retention_bytes = Some(1_000_000_000_000);

        let report = evaluate_kafka_retention_policy_inventory(
            &[missing_evidence, expensive],
            Pillar::Cost,
            now,
        );

        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_COST_OWNER_NOT_RECORDED));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_COST_NO_RETENTION_EVIDENCE));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_COST_HIGH_RETENTION_BYTES));
    }

    #[test]
    fn resilience_flags_missing_evidence_audit_and_short_retention() {
        let now = Utc::now();
        let mut missing_evidence = retention(now);
        missing_evidence.has_retention_evidence = false;
        let mut risky = retention(now);
        risky.audit_evidence = false;
        risky.retention_ms = Some(3_600_000);

        let report = evaluate_kafka_retention_policy_inventory(
            &[missing_evidence, risky],
            Pillar::Resilience,
            now,
        );

        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_RES_NO_RETENTION_EVIDENCE));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_RES_NO_AUDIT_EVIDENCE));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_RES_SHORT_RETENTION));
    }

    #[test]
    fn security_flags_missing_evidence_delete_policy_and_plaintext() {
        let now = Utc::now();
        let mut missing_evidence = retention(now);
        missing_evidence.has_retention_evidence = false;
        let mut insecure = retention(now);
        insecure.cleanup_policy = "compact,delete".to_string();
        insecure.security_protocol = "PLAINTEXT".to_string();

        let report = evaluate_kafka_retention_policy_inventory(
            &[missing_evidence, insecure],
            Pillar::Security,
            now,
        );

        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_SEC_NO_RETENTION_EVIDENCE));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_SEC_DELETE_POLICY));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_SEC_PLAINTEXT));
    }

    #[test]
    fn stale_retention_policy_inventory_is_counted_for_any_pillar() {
        let now = Utc::now();
        let mut item = retention(now);
        item.collected_at = now - Duration::hours(DEFAULT_STALE_AFTER_HOURS + 2);

        let report = evaluate_kafka_retention_policy_inventory(&[item], Pillar::Security, now);

        assert_eq!(report.stale_resources, 1);
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_INV_STALE_DATA));
    }
}

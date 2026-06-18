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

// Deterministic Kafka compaction policy inventory evaluator for roadmap rows
// 04-KAFKA-DASHBOARD-MANAGEMENT-00883/00890/00911.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::config::KafkaClusterConfig;
use crate::services::aws::inventory::types::{
    score_pillar, InventoryFinding, Pillar, PillarReport, Severity, COST_ALLOCATION_TAG_KEYS,
    DEFAULT_STALE_AFTER_HOURS,
};

pub const RESOURCE_TYPE: &str = "KafkaCompactionPolicy";
pub const REASON_COST_OWNER_NOT_RECORDED: &str = "KAFKA_COMPACTION_POLICY_COST_OWNER_NOT_RECORDED";
pub const REASON_COST_NO_COMPACTION_EVIDENCE: &str = "KAFKA_COMPACTION_POLICY_COST_NO_EVIDENCE";
pub const REASON_COST_NOT_COMPACTED: &str = "KAFKA_COMPACTION_POLICY_COST_NOT_COMPACTED";
pub const REASON_COST_AGGRESSIVE_CLEANING: &str =
    "KAFKA_COMPACTION_POLICY_COST_AGGRESSIVE_CLEANING";
pub const REASON_COST_LONG_TOMBSTONE_RETENTION: &str =
    "KAFKA_COMPACTION_POLICY_COST_LONG_TOMBSTONE_RETENTION";
pub const REASON_RES_NO_COMPACTION_EVIDENCE: &str = "KAFKA_COMPACTION_POLICY_RES_NO_EVIDENCE";
pub const REASON_RES_NO_AUDIT_EVIDENCE: &str = "KAFKA_COMPACTION_POLICY_RES_NO_AUDIT_EVIDENCE";
pub const REASON_RES_NOT_COMPACTED: &str = "KAFKA_COMPACTION_POLICY_RES_NOT_COMPACTED";
pub const REASON_RES_UNBOUNDED_COMPACTION_LAG: &str = "KAFKA_COMPACTION_POLICY_RES_UNBOUNDED_LAG";
pub const REASON_RES_SHORT_TOMBSTONE_RETENTION: &str =
    "KAFKA_COMPACTION_POLICY_RES_SHORT_TOMBSTONE_RETENTION";
pub const REASON_SEC_NO_COMPACTION_EVIDENCE: &str = "KAFKA_COMPACTION_POLICY_SEC_NO_EVIDENCE";
pub const REASON_SEC_SHORT_TOMBSTONE_RETENTION: &str =
    "KAFKA_COMPACTION_POLICY_SEC_SHORT_TOMBSTONE_RETENTION";
pub const REASON_SEC_PLAINTEXT: &str = "KAFKA_COMPACTION_POLICY_SEC_PLAINTEXT_PROTOCOL";
pub const REASON_INV_STALE_DATA: &str = "KAFKA_COMPACTION_POLICY_INV_STALE_DATA";

const ONE_HOUR_MS: i64 = 3_600_000;
const ONE_DAY_MS: i64 = 86_400_000;
const SEVEN_DAYS_MS: i64 = 604_800_000;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KafkaCompactionPolicyInventoryItem {
    pub compaction_policy_id: String,
    pub cluster_name: String,
    pub topic_name: String,
    pub cleanup_policy: String,
    pub min_cleanable_dirty_ratio: Option<f64>,
    pub delete_retention_ms: Option<i64>,
    pub min_compaction_lag_ms: Option<i64>,
    pub max_compaction_lag_ms: Option<i64>,
    pub segment_ms: Option<i64>,
    pub owner: Option<String>,
    pub labels: BTreeMap<String, String>,
    pub audit_evidence: bool,
    pub has_compaction_evidence: bool,
    pub security_protocol: String,
    pub collected_at: DateTime<Utc>,
}

pub fn evaluate_kafka_compaction_policy_inventory(
    items: &[KafkaCompactionPolicyInventoryItem],
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

pub fn compaction_policy_inventory_item_from_config(
    cluster: &KafkaClusterConfig,
    collected_at: DateTime<Utc>,
) -> KafkaCompactionPolicyInventoryItem {
    KafkaCompactionPolicyInventoryItem {
        compaction_policy_id: format!("{}:compaction-policies", cluster.name),
        cluster_name: cluster.name.clone(),
        topic_name: "<uncollected>".to_string(),
        cleanup_policy: "<uncollected>".to_string(),
        min_cleanable_dirty_ratio: None,
        delete_retention_ms: None,
        min_compaction_lag_ms: None,
        max_compaction_lag_ms: None,
        segment_ms: None,
        owner: None,
        labels: BTreeMap::new(),
        audit_evidence: false,
        has_compaction_evidence: false,
        security_protocol: cluster.security_protocol.clone(),
        collected_at,
    }
}

fn evaluate_cost(
    item: &KafkaCompactionPolicyInventoryItem,
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
                "Kafka compaction policy inventory for cluster {} has no owner, team, project, or cost-center metadata",
                item.cluster_name
            ),
            json!({
                "compaction_policy_id": item.compaction_policy_id,
                "cluster_name": item.cluster_name,
                "checked_keys": COST_ALLOCATION_TAG_KEYS,
            }),
        ));
    }

    if !item.has_compaction_evidence {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_NO_COMPACTION_EVIDENCE,
            Severity::High,
            format!(
                "Kafka compaction policy inventory for cluster {} has no compaction evidence",
                item.cluster_name
            ),
            json!({
                "compaction_policy_id": item.compaction_policy_id,
                "cluster_name": item.cluster_name,
                "recommendation": "Collect topic cleanup.policy, min.cleanable.dirty.ratio, delete.retention.ms, compaction lag, ownership, labels, and audit evidence before estimating compaction cost posture",
            }),
        ));
    }

    if item.has_compaction_evidence && !cleanup_policy_includes_compact(&item.cleanup_policy) {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_NOT_COMPACTED,
            Severity::High,
            format!(
                "Kafka compaction policy {} does not enable log compaction",
                item.compaction_policy_id
            ),
            json!({
                "compaction_policy_id": item.compaction_policy_id,
                "cleanup_policy": item.cleanup_policy,
                "recommendation": "Enable compact cleanup policy for key-value or latest-state topics before relying on compaction storage savings",
            }),
        ));
    }

    if item
        .min_cleanable_dirty_ratio
        .is_some_and(|ratio| ratio > 0.0 && ratio < 0.10)
    {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_AGGRESSIVE_CLEANING,
            Severity::Medium,
            format!(
                "Kafka compaction policy {} has an aggressive dirty ratio",
                item.compaction_policy_id
            ),
            json!({
                "compaction_policy_id": item.compaction_policy_id,
                "min_cleanable_dirty_ratio": item.min_cleanable_dirty_ratio,
                "recommendation": "Review low min.cleanable.dirty.ratio values for broker IO and CPU cost before scaling compacted topics",
            }),
        ));
    }

    if item
        .delete_retention_ms
        .is_some_and(|retention_ms| retention_ms > SEVEN_DAYS_MS)
    {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_LONG_TOMBSTONE_RETENTION,
            Severity::Medium,
            format!(
                "Kafka compaction policy {} keeps tombstones for more than seven days",
                item.compaction_policy_id
            ),
            json!({
                "compaction_policy_id": item.compaction_policy_id,
                "delete_retention_ms": item.delete_retention_ms,
                "recommendation": "Validate long delete.retention.ms settings against consumer outage and forensic requirements before accepting storage cost",
            }),
        ));
    }
}

fn evaluate_resilience(
    item: &KafkaCompactionPolicyInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if !item.has_compaction_evidence {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_NO_COMPACTION_EVIDENCE,
            Severity::High,
            format!(
                "Kafka compaction policy inventory for cluster {} has no resilience evidence",
                item.cluster_name
            ),
            json!({
                "compaction_policy_id": item.compaction_policy_id,
                "cluster_name": item.cluster_name,
                "recommendation": "Collect compaction policy and audit evidence before relying on compacted topics during recovery",
            }),
        ));
    }

    if item.has_compaction_evidence && !item.audit_evidence {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_NO_AUDIT_EVIDENCE,
            Severity::High,
            format!(
                "Kafka compaction policy {} has no audit evidence",
                item.compaction_policy_id
            ),
            json!({
                "compaction_policy_id": item.compaction_policy_id,
                "audit_evidence": item.audit_evidence,
                "recommendation": "Enable or collect compaction policy change audit evidence before accepting recovery posture",
            }),
        ));
    }

    if item.has_compaction_evidence && !cleanup_policy_includes_compact(&item.cleanup_policy) {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_NOT_COMPACTED,
            Severity::High,
            format!(
                "Kafka compaction policy {} is missing compact cleanup policy",
                item.compaction_policy_id
            ),
            json!({
                "compaction_policy_id": item.compaction_policy_id,
                "cleanup_policy": item.cleanup_policy,
                "recommendation": "Use compact cleanup policy for topics whose recovery model depends on latest value reconstruction",
            }),
        ));
    }

    if item.has_compaction_evidence
        && item
            .max_compaction_lag_ms
            .is_none_or(|lag_ms| lag_ms > ONE_DAY_MS)
    {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_UNBOUNDED_COMPACTION_LAG,
            Severity::Medium,
            format!(
                "Kafka compaction policy {} has unbounded or long compaction lag",
                item.compaction_policy_id
            ),
            json!({
                "compaction_policy_id": item.compaction_policy_id,
                "max_compaction_lag_ms": item.max_compaction_lag_ms,
                "recommendation": "Set bounded max.compaction.lag.ms where recovery objectives require timely latest-state convergence",
            }),
        ));
    }

    if item
        .delete_retention_ms
        .is_some_and(|retention_ms| retention_ms < ONE_HOUR_MS)
    {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_SHORT_TOMBSTONE_RETENTION,
            Severity::High,
            format!(
                "Kafka compaction policy {} has very short tombstone retention",
                item.compaction_policy_id
            ),
            json!({
                "compaction_policy_id": item.compaction_policy_id,
                "delete_retention_ms": item.delete_retention_ms,
                "recommendation": "Keep delete.retention.ms long enough for offline consumers and restore jobs to observe tombstones",
            }),
        ));
    }
}

fn evaluate_security(
    item: &KafkaCompactionPolicyInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if !item.has_compaction_evidence {
        findings.push(finding(
            item,
            pillar,
            REASON_SEC_NO_COMPACTION_EVIDENCE,
            Severity::High,
            format!(
                "Kafka compaction policy inventory for cluster {} has no compaction evidence for security review",
                item.cluster_name
            ),
            json!({
                "compaction_policy_id": item.compaction_policy_id,
                "cluster_name": item.cluster_name,
                "recommendation": "Collect compaction settings, tombstone retention, ownership, and audit evidence before assessing forensic availability",
            }),
        ));
    }

    if item
        .delete_retention_ms
        .is_some_and(|retention_ms| retention_ms < ONE_DAY_MS)
    {
        findings.push(finding(
            item,
            pillar,
            REASON_SEC_SHORT_TOMBSTONE_RETENTION,
            Severity::Medium,
            format!(
                "Kafka compaction policy {} keeps tombstones for less than one day",
                item.compaction_policy_id
            ),
            json!({
                "compaction_policy_id": item.compaction_policy_id,
                "delete_retention_ms": item.delete_retention_ms,
                "recommendation": "Confirm short tombstone retention meets forensic and compliance investigation requirements",
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
                "Kafka compaction policy inventory for cluster {} uses PLAINTEXT transport",
                item.cluster_name
            ),
            json!({
                "compaction_policy_id": item.compaction_policy_id,
                "security_protocol": item.security_protocol,
                "recommendation": "Use SSL or SASL_SSL before accepting compaction policy evidence for sensitive topics",
            }),
        ));
    }
}

fn stale_finding(
    item: &KafkaCompactionPolicyInventoryItem,
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
            "Kafka compaction policy inventory for cluster {} is {} hour(s) old",
            item.cluster_name, age_hours
        ),
        json!({
            "compaction_policy_id": item.compaction_policy_id,
            "cluster_name": item.cluster_name,
            "age_hours": age_hours,
            "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
            "recommendation": "Refresh Kafka compaction policy inventory before acting on this posture report",
        }),
    ))
}

fn cleanup_policy_includes_compact(cleanup_policy: &str) -> bool {
    cleanup_policy
        .split(',')
        .map(str::trim)
        .any(|part| part.eq_ignore_ascii_case("compact"))
}

fn has_owner_metadata(item: &KafkaCompactionPolicyInventoryItem) -> bool {
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
    item: &KafkaCompactionPolicyInventoryItem,
    pillar: Pillar,
    reason_code: &'static str,
    severity: Severity,
    message: String,
    evidence: Value,
) -> InventoryFinding {
    InventoryFinding {
        resource_id: item.compaction_policy_id.clone(),
        arn: format!("kafka:compaction-policy/{}", item.compaction_policy_id),
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

    fn compaction(now: DateTime<Utc>) -> KafkaCompactionPolicyInventoryItem {
        KafkaCompactionPolicyInventoryItem {
            compaction_policy_id: "prod:topic:customer-state:compaction".to_string(),
            cluster_name: "prod".to_string(),
            topic_name: "customer-state".to_string(),
            cleanup_policy: "compact".to_string(),
            min_cleanable_dirty_ratio: Some(0.5),
            delete_retention_ms: Some(ONE_DAY_MS),
            min_compaction_lag_ms: Some(0),
            max_compaction_lag_ms: Some(ONE_HOUR_MS),
            segment_ms: Some(ONE_HOUR_MS),
            owner: Some("customer-platform".to_string()),
            labels: BTreeMap::new(),
            audit_evidence: true,
            has_compaction_evidence: true,
            security_protocol: "SASL_SSL".to_string(),
            collected_at: now,
        }
    }

    #[test]
    fn healthy_compaction_policy_passes_claimed_pillars() {
        let now = Utc::now();
        let item = compaction(now);

        for pillar in [Pillar::Cost, Pillar::Resilience, Pillar::Security] {
            let report = evaluate_kafka_compaction_policy_inventory(
                std::slice::from_ref(&item),
                pillar,
                now,
            );
            assert_eq!(report.resources_evaluated, 1);
            assert!(report.findings.is_empty());
            assert_eq!(report.stale_resources, 0);
            assert_eq!(report.score, 100);
        }
    }

    #[test]
    fn cost_flags_missing_owner_missing_evidence_non_compacted_and_expensive_settings() {
        let now = Utc::now();
        let mut missing_evidence = compaction(now);
        missing_evidence.owner = None;
        missing_evidence.has_compaction_evidence = false;
        let mut expensive = compaction(now);
        expensive.cleanup_policy = "delete".to_string();
        expensive.min_cleanable_dirty_ratio = Some(0.05);
        expensive.delete_retention_ms = Some(SEVEN_DAYS_MS + ONE_DAY_MS);

        let report = evaluate_kafka_compaction_policy_inventory(
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
            .any(|finding| finding.reason_code == REASON_COST_NO_COMPACTION_EVIDENCE));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_COST_NOT_COMPACTED));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_COST_AGGRESSIVE_CLEANING));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_COST_LONG_TOMBSTONE_RETENTION));
    }

    #[test]
    fn resilience_flags_missing_evidence_audit_policy_lag_and_tombstones() {
        let now = Utc::now();
        let mut missing_evidence = compaction(now);
        missing_evidence.has_compaction_evidence = false;
        let mut risky = compaction(now);
        risky.audit_evidence = false;
        risky.cleanup_policy = "delete".to_string();
        risky.max_compaction_lag_ms = None;
        risky.delete_retention_ms = Some(30 * 60 * 1000);

        let report = evaluate_kafka_compaction_policy_inventory(
            &[missing_evidence, risky],
            Pillar::Resilience,
            now,
        );

        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_RES_NO_COMPACTION_EVIDENCE));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_RES_NO_AUDIT_EVIDENCE));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_RES_NOT_COMPACTED));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_RES_UNBOUNDED_COMPACTION_LAG));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_RES_SHORT_TOMBSTONE_RETENTION));
    }

    #[test]
    fn security_flags_missing_evidence_short_tombstones_and_plaintext() {
        let now = Utc::now();
        let mut missing_evidence = compaction(now);
        missing_evidence.has_compaction_evidence = false;
        let mut insecure = compaction(now);
        insecure.delete_retention_ms = Some(ONE_HOUR_MS);
        insecure.security_protocol = "PLAINTEXT".to_string();

        let report = evaluate_kafka_compaction_policy_inventory(
            &[missing_evidence, insecure],
            Pillar::Security,
            now,
        );

        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_SEC_NO_COMPACTION_EVIDENCE));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_SEC_SHORT_TOMBSTONE_RETENTION));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_SEC_PLAINTEXT));
    }

    #[test]
    fn stale_compaction_policy_inventory_is_counted_for_any_pillar() {
        let now = Utc::now();
        let mut item = compaction(now);
        item.collected_at = now - Duration::hours(DEFAULT_STALE_AFTER_HOURS + 2);

        let report = evaluate_kafka_compaction_policy_inventory(&[item], Pillar::Security, now);

        assert_eq!(report.stale_resources, 1);
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_INV_STALE_DATA));
    }
}

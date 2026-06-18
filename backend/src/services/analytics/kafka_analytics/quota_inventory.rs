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

// Deterministic Kafka quota inventory evaluator for roadmap rows
// 04-KAFKA-DASHBOARD-MANAGEMENT-00785/00792/00813.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::config::KafkaClusterConfig;
use crate::services::aws::inventory::types::{
    score_pillar, InventoryFinding, Pillar, PillarReport, Severity, COST_ALLOCATION_TAG_KEYS,
    DEFAULT_STALE_AFTER_HOURS,
};

pub const RESOURCE_TYPE: &str = "KafkaQuota";
pub const REASON_COST_OWNER_NOT_RECORDED: &str = "KAFKA_QUOTA_COST_OWNER_NOT_RECORDED";
pub const REASON_COST_NO_QUOTA_EVIDENCE: &str = "KAFKA_QUOTA_COST_NO_EVIDENCE";
pub const REASON_COST_HIGH_QUOTA_LIMIT: &str = "KAFKA_QUOTA_COST_HIGH_LIMIT";
pub const REASON_RES_NO_QUOTA_EVIDENCE: &str = "KAFKA_QUOTA_RES_NO_EVIDENCE";
pub const REASON_RES_NO_AUDIT_EVIDENCE: &str = "KAFKA_QUOTA_RES_NO_AUDIT_EVIDENCE";
pub const REASON_RES_UNBOUNDED_QUOTA: &str = "KAFKA_QUOTA_RES_UNBOUNDED_QUOTA";
pub const REASON_SEC_NO_QUOTA_EVIDENCE: &str = "KAFKA_QUOTA_SEC_NO_EVIDENCE";
pub const REASON_SEC_WILDCARD_SCOPE: &str = "KAFKA_QUOTA_SEC_WILDCARD_SCOPE";
pub const REASON_SEC_PLAINTEXT: &str = "KAFKA_QUOTA_SEC_PLAINTEXT_PROTOCOL";
pub const REASON_INV_STALE_DATA: &str = "KAFKA_QUOTA_INV_STALE_DATA";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KafkaQuotaInventoryItem {
    pub quota_id: String,
    pub cluster_name: String,
    pub entity: String,
    pub quota_type: String,
    pub producer_byte_rate: Option<i64>,
    pub consumer_byte_rate: Option<i64>,
    pub request_percentage: Option<i32>,
    pub owner: Option<String>,
    pub labels: BTreeMap<String, String>,
    pub audit_evidence: bool,
    pub has_quota_evidence: bool,
    pub security_protocol: String,
    pub collected_at: DateTime<Utc>,
}

pub fn evaluate_kafka_quota_inventory(
    items: &[KafkaQuotaInventoryItem],
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

pub fn quota_inventory_item_from_config(
    cluster: &KafkaClusterConfig,
    collected_at: DateTime<Utc>,
) -> KafkaQuotaInventoryItem {
    KafkaQuotaInventoryItem {
        quota_id: format!("{}:quotas", cluster.name),
        cluster_name: cluster.name.clone(),
        entity: "<uncollected>".to_string(),
        quota_type: "<uncollected>".to_string(),
        producer_byte_rate: None,
        consumer_byte_rate: None,
        request_percentage: None,
        owner: None,
        labels: BTreeMap::new(),
        audit_evidence: false,
        has_quota_evidence: false,
        security_protocol: cluster.security_protocol.clone(),
        collected_at,
    }
}

fn evaluate_cost(
    item: &KafkaQuotaInventoryItem,
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
                "Kafka quota inventory for cluster {} has no owner, team, project, or cost-center metadata",
                item.cluster_name
            ),
            json!({
                "quota_id": item.quota_id,
                "cluster_name": item.cluster_name,
                "checked_keys": COST_ALLOCATION_TAG_KEYS,
            }),
        ));
    }

    if !item.has_quota_evidence {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_NO_QUOTA_EVIDENCE,
            Severity::High,
            format!(
                "Kafka quota inventory for cluster {} has no quota evidence",
                item.cluster_name
            ),
            json!({
                "quota_id": item.quota_id,
                "cluster_name": item.cluster_name,
                "recommendation": "Collect quota entity, quota type, byte-rate limits, request percentages, ownership, and audit evidence before estimating quota cost posture",
            }),
        ));
    }

    if item
        .producer_byte_rate
        .is_some_and(|limit| limit >= 1_000_000_000)
        || item
            .consumer_byte_rate
            .is_some_and(|limit| limit >= 1_000_000_000)
        || item.request_percentage.is_some_and(|limit| limit >= 95)
    {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_HIGH_QUOTA_LIMIT,
            Severity::Medium,
            format!("Kafka quota {} has a high configured limit", item.quota_id),
            json!({
                "quota_id": item.quota_id,
                "producer_byte_rate": item.producer_byte_rate,
                "consumer_byte_rate": item.consumer_byte_rate,
                "request_percentage": item.request_percentage,
                "recommendation": "Review high quota ceilings before scaling broker resources or attributing avoidable traffic cost",
            }),
        ));
    }
}

fn evaluate_resilience(
    item: &KafkaQuotaInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if !item.has_quota_evidence {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_NO_QUOTA_EVIDENCE,
            Severity::High,
            format!(
                "Kafka quota inventory for cluster {} has no resilience evidence",
                item.cluster_name
            ),
            json!({
                "quota_id": item.quota_id,
                "cluster_name": item.cluster_name,
                "recommendation": "Collect quota limits and audit evidence before relying on quota posture during overload or recovery events",
            }),
        ));
    }

    if item.has_quota_evidence && !item.audit_evidence {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_NO_AUDIT_EVIDENCE,
            Severity::High,
            format!("Kafka quota {} has no audit evidence", item.quota_id),
            json!({
                "quota_id": item.quota_id,
                "audit_evidence": item.audit_evidence,
                "recommendation": "Enable or collect quota change audit evidence before accepting quota recovery posture",
            }),
        ));
    }

    if item.has_quota_evidence
        && item.producer_byte_rate.is_none()
        && item.consumer_byte_rate.is_none()
        && item.request_percentage.is_none()
    {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_UNBOUNDED_QUOTA,
            Severity::High,
            format!("Kafka quota {} has no effective limits recorded", item.quota_id),
            json!({
                "quota_id": item.quota_id,
                "recommendation": "Record explicit producer, consumer, or request quota limits for overload protection",
            }),
        ));
    }
}

fn evaluate_security(
    item: &KafkaQuotaInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if !item.has_quota_evidence {
        findings.push(finding(
            item,
            pillar,
            REASON_SEC_NO_QUOTA_EVIDENCE,
            Severity::High,
            format!(
                "Kafka quota inventory for cluster {} has no quota evidence for security review",
                item.cluster_name
            ),
            json!({
                "quota_id": item.quota_id,
                "cluster_name": item.cluster_name,
                "recommendation": "Collect quota entity scope, type, limits, and audit evidence before assessing abuse or noisy-neighbor controls",
            }),
        ));
    }

    if item.has_quota_evidence && wildcard_scope(&item.entity) {
        findings.push(finding(
            item,
            pillar,
            REASON_SEC_WILDCARD_SCOPE,
            Severity::High,
            format!("Kafka quota {} applies to a wildcard or default scope", item.quota_id),
            json!({
                "quota_id": item.quota_id,
                "entity": item.entity,
                "quota_type": item.quota_type,
                "recommendation": "Prefer scoped client, user, or client-user quota entities for security-sensitive workloads",
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
                "Kafka quota inventory for cluster {} uses PLAINTEXT transport",
                item.cluster_name
            ),
            json!({
                "quota_id": item.quota_id,
                "security_protocol": item.security_protocol,
                "recommendation": "Use SSL or SASL_SSL before accepting quota posture for sensitive clients",
            }),
        ));
    }
}

fn stale_finding(
    item: &KafkaQuotaInventoryItem,
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
            "Kafka quota inventory for cluster {} is {} hour(s) old",
            item.cluster_name, age_hours
        ),
        json!({
            "quota_id": item.quota_id,
            "cluster_name": item.cluster_name,
            "age_hours": age_hours,
            "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
            "recommendation": "Refresh Kafka quota inventory before acting on this posture report",
        }),
    ))
}

fn wildcard_scope(value: &str) -> bool {
    let value = value.trim();
    value == "*" || value.eq_ignore_ascii_case("<default>") || value.eq_ignore_ascii_case("default")
}

fn has_owner_metadata(item: &KafkaQuotaInventoryItem) -> bool {
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
    item: &KafkaQuotaInventoryItem,
    pillar: Pillar,
    reason_code: &'static str,
    severity: Severity,
    message: String,
    evidence: Value,
) -> InventoryFinding {
    InventoryFinding {
        resource_id: item.quota_id.clone(),
        arn: format!("kafka:quota/{}", item.quota_id),
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

    fn quota(now: DateTime<Utc>) -> KafkaQuotaInventoryItem {
        KafkaQuotaInventoryItem {
            quota_id: "prod:quota:payments".to_string(),
            cluster_name: "prod".to_string(),
            entity: "User:payments".to_string(),
            quota_type: "client-user".to_string(),
            producer_byte_rate: Some(50_000_000),
            consumer_byte_rate: Some(50_000_000),
            request_percentage: Some(25),
            owner: Some("payments".to_string()),
            labels: BTreeMap::new(),
            audit_evidence: true,
            has_quota_evidence: true,
            security_protocol: "SASL_SSL".to_string(),
            collected_at: now,
        }
    }

    #[test]
    fn healthy_quota_passes_claimed_pillars() {
        let now = Utc::now();
        let item = quota(now);

        for pillar in [Pillar::Cost, Pillar::Resilience, Pillar::Security] {
            let report = evaluate_kafka_quota_inventory(std::slice::from_ref(&item), pillar, now);
            assert_eq!(report.resources_evaluated, 1);
            assert!(report.findings.is_empty());
            assert_eq!(report.stale_resources, 0);
            assert_eq!(report.score, 100);
        }
    }

    #[test]
    fn cost_flags_missing_owner_missing_evidence_and_high_limits() {
        let now = Utc::now();
        let mut item = quota(now);
        item.owner = None;
        item.has_quota_evidence = false;
        item.producer_byte_rate = Some(1_000_000_000);

        let report = evaluate_kafka_quota_inventory(&[item], Pillar::Cost, now);

        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_COST_OWNER_NOT_RECORDED));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_COST_NO_QUOTA_EVIDENCE));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_COST_HIGH_QUOTA_LIMIT));
    }

    #[test]
    fn resilience_flags_missing_evidence_audit_and_unbounded_quota() {
        let now = Utc::now();
        let mut missing_evidence = quota(now);
        missing_evidence.has_quota_evidence = false;
        let mut unbounded = quota(now);
        unbounded.audit_evidence = false;
        unbounded.producer_byte_rate = None;
        unbounded.consumer_byte_rate = None;
        unbounded.request_percentage = None;

        let report =
            evaluate_kafka_quota_inventory(&[missing_evidence, unbounded], Pillar::Resilience, now);

        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_RES_NO_QUOTA_EVIDENCE));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_RES_NO_AUDIT_EVIDENCE));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_RES_UNBOUNDED_QUOTA));
    }

    #[test]
    fn security_flags_missing_evidence_wildcard_scope_and_plaintext() {
        let now = Utc::now();
        let mut missing_evidence = quota(now);
        missing_evidence.has_quota_evidence = false;
        let mut insecure = quota(now);
        insecure.entity = "*".to_string();
        insecure.security_protocol = "PLAINTEXT".to_string();

        let report =
            evaluate_kafka_quota_inventory(&[missing_evidence, insecure], Pillar::Security, now);

        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_SEC_NO_QUOTA_EVIDENCE));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_SEC_WILDCARD_SCOPE));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_SEC_PLAINTEXT));
    }

    #[test]
    fn stale_quota_inventory_is_counted_for_any_pillar() {
        let now = Utc::now();
        let mut item = quota(now);
        item.collected_at = now - Duration::hours(DEFAULT_STALE_AFTER_HOURS + 2);

        let report = evaluate_kafka_quota_inventory(&[item], Pillar::Security, now);

        assert_eq!(report.stale_resources, 1);
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_INV_STALE_DATA));
    }
}

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

// Deterministic Kafka Schema Registry inventory evaluator for roadmap rows
// 04-KAFKA-DASHBOARD-MANAGEMENT-00932/00939/00960.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::config::KafkaClusterConfig;
use crate::services::aws::inventory::types::{
    score_pillar, InventoryFinding, Pillar, PillarReport, Severity, COST_ALLOCATION_TAG_KEYS,
    DEFAULT_STALE_AFTER_HOURS,
};

pub const RESOURCE_TYPE: &str = "KafkaSchemaRegistry";
pub const REASON_COST_OWNER_NOT_RECORDED: &str = "KAFKA_SCHEMA_REGISTRY_COST_OWNER_NOT_RECORDED";
pub const REASON_COST_NO_EVIDENCE: &str = "KAFKA_SCHEMA_REGISTRY_COST_NO_EVIDENCE";
pub const REASON_COST_HIGH_SUBJECT_COUNT: &str = "KAFKA_SCHEMA_REGISTRY_COST_HIGH_SUBJECT_COUNT";
pub const REASON_RES_NO_EVIDENCE: &str = "KAFKA_SCHEMA_REGISTRY_RES_NO_EVIDENCE";
pub const REASON_RES_NO_AUDIT_EVIDENCE: &str = "KAFKA_SCHEMA_REGISTRY_RES_NO_AUDIT_EVIDENCE";
pub const REASON_RES_NO_COMPATIBILITY: &str = "KAFKA_SCHEMA_REGISTRY_RES_NO_COMPATIBILITY";
pub const REASON_SEC_NO_EVIDENCE: &str = "KAFKA_SCHEMA_REGISTRY_SEC_NO_EVIDENCE";
pub const REASON_SEC_AUTH_DISABLED: &str = "KAFKA_SCHEMA_REGISTRY_SEC_AUTH_DISABLED";
pub const REASON_SEC_TLS_DISABLED: &str = "KAFKA_SCHEMA_REGISTRY_SEC_TLS_DISABLED";
pub const REASON_INV_STALE_DATA: &str = "KAFKA_SCHEMA_REGISTRY_INV_STALE_DATA";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KafkaSchemaRegistryInventoryItem {
    pub schema_registry_id: String,
    pub cluster_name: String,
    pub endpoint: Option<String>,
    pub subject_count: Option<u64>,
    pub schema_count: Option<u64>,
    pub compatibility_level: Option<String>,
    pub owner: Option<String>,
    pub labels: BTreeMap<String, String>,
    pub audit_evidence: bool,
    pub auth_enabled: bool,
    pub tls_enabled: bool,
    pub has_schema_registry_evidence: bool,
    pub collected_at: DateTime<Utc>,
}

pub fn evaluate_kafka_schema_registry_inventory(
    items: &[KafkaSchemaRegistryInventoryItem],
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

pub fn schema_registry_inventory_item_from_config(
    cluster: &KafkaClusterConfig,
    collected_at: DateTime<Utc>,
) -> KafkaSchemaRegistryInventoryItem {
    KafkaSchemaRegistryInventoryItem {
        schema_registry_id: format!("{}:schema-registry", cluster.name),
        cluster_name: cluster.name.clone(),
        endpoint: None,
        subject_count: None,
        schema_count: None,
        compatibility_level: None,
        owner: None,
        labels: BTreeMap::new(),
        audit_evidence: false,
        auth_enabled: false,
        tls_enabled: !cluster.security_protocol.eq_ignore_ascii_case("PLAINTEXT"),
        has_schema_registry_evidence: false,
        collected_at,
    }
}

fn evaluate_cost(
    item: &KafkaSchemaRegistryInventoryItem,
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
                "Kafka Schema Registry inventory for cluster {} has no owner, team, project, or cost-center metadata",
                item.cluster_name
            ),
            json!({
                "schema_registry_id": item.schema_registry_id,
                "cluster_name": item.cluster_name,
                "checked_keys": COST_ALLOCATION_TAG_KEYS,
            }),
        ));
    }

    if !item.has_schema_registry_evidence {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_NO_EVIDENCE,
            Severity::High,
            format!(
                "Kafka Schema Registry inventory for cluster {} has no registry evidence",
                item.cluster_name
            ),
            json!({
                "schema_registry_id": item.schema_registry_id,
                "cluster_name": item.cluster_name,
                "recommendation": "Collect Schema Registry endpoint, subjects, schemas, compatibility, ownership, labels, auth, TLS, and audit evidence before estimating cost posture",
            }),
        ));
    }

    if item
        .subject_count
        .is_some_and(|subject_count| subject_count >= 10_000)
    {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_HIGH_SUBJECT_COUNT,
            Severity::Medium,
            format!(
                "Kafka Schema Registry {} has a high subject count",
                item.schema_registry_id
            ),
            json!({
                "schema_registry_id": item.schema_registry_id,
                "subject_count": item.subject_count,
                "schema_count": item.schema_count,
                "recommendation": "Review subject naming, stale schemas, and retention policy before accepting registry growth cost",
            }),
        ));
    }
}

fn evaluate_resilience(
    item: &KafkaSchemaRegistryInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if !item.has_schema_registry_evidence {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_NO_EVIDENCE,
            Severity::High,
            format!(
                "Kafka Schema Registry inventory for cluster {} has no resilience evidence",
                item.cluster_name
            ),
            json!({
                "schema_registry_id": item.schema_registry_id,
                "cluster_name": item.cluster_name,
                "recommendation": "Collect Schema Registry inventory and compatibility settings before relying on schema evolution during recovery",
            }),
        ));
    }

    if item.has_schema_registry_evidence && !item.audit_evidence {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_NO_AUDIT_EVIDENCE,
            Severity::High,
            format!(
                "Kafka Schema Registry {} has no audit evidence",
                item.schema_registry_id
            ),
            json!({
                "schema_registry_id": item.schema_registry_id,
                "audit_evidence": item.audit_evidence,
                "recommendation": "Enable or collect Schema Registry change audit evidence before accepting recovery posture",
            }),
        ));
    }

    if item
        .compatibility_level
        .as_deref()
        .is_none_or(|level| level.trim().is_empty() || level.eq_ignore_ascii_case("NONE"))
    {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_NO_COMPATIBILITY,
            Severity::High,
            format!(
                "Kafka Schema Registry {} has no effective compatibility policy",
                item.schema_registry_id
            ),
            json!({
                "schema_registry_id": item.schema_registry_id,
                "compatibility_level": item.compatibility_level,
                "recommendation": "Set a compatibility level such as BACKWARD, FORWARD, or FULL according to consumer rollout policy",
            }),
        ));
    }
}

fn evaluate_security(
    item: &KafkaSchemaRegistryInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if !item.has_schema_registry_evidence {
        findings.push(finding(
            item,
            pillar,
            REASON_SEC_NO_EVIDENCE,
            Severity::High,
            format!(
                "Kafka Schema Registry inventory for cluster {} has no security evidence",
                item.cluster_name
            ),
            json!({
                "schema_registry_id": item.schema_registry_id,
                "cluster_name": item.cluster_name,
                "recommendation": "Collect Schema Registry auth, TLS, endpoint, subjects, and audit evidence before accepting security posture",
            }),
        ));
    }

    if item.has_schema_registry_evidence && !item.auth_enabled {
        findings.push(finding(
            item,
            pillar,
            REASON_SEC_AUTH_DISABLED,
            Severity::High,
            format!(
                "Kafka Schema Registry {} has authentication disabled or unverified",
                item.schema_registry_id
            ),
            json!({
                "schema_registry_id": item.schema_registry_id,
                "auth_enabled": item.auth_enabled,
                "recommendation": "Require authenticated Schema Registry access before accepting sensitive schema metadata posture",
            }),
        ));
    }

    if !item.tls_enabled {
        findings.push(finding(
            item,
            pillar,
            REASON_SEC_TLS_DISABLED,
            Severity::High,
            format!(
                "Kafka Schema Registry {} has TLS disabled or unverified",
                item.schema_registry_id
            ),
            json!({
                "schema_registry_id": item.schema_registry_id,
                "tls_enabled": item.tls_enabled,
                "endpoint": item.endpoint,
                "recommendation": "Use HTTPS/TLS for Schema Registry traffic before accepting security posture",
            }),
        ));
    }
}

fn stale_finding(
    item: &KafkaSchemaRegistryInventoryItem,
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
            "Kafka Schema Registry inventory for cluster {} is {} hour(s) old",
            item.cluster_name, age_hours
        ),
        json!({
            "schema_registry_id": item.schema_registry_id,
            "cluster_name": item.cluster_name,
            "age_hours": age_hours,
            "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
            "recommendation": "Refresh Kafka Schema Registry inventory before acting on this posture report",
        }),
    ))
}

fn has_owner_metadata(item: &KafkaSchemaRegistryInventoryItem) -> bool {
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
    item: &KafkaSchemaRegistryInventoryItem,
    pillar: Pillar,
    reason_code: &'static str,
    severity: Severity,
    message: String,
    evidence: Value,
) -> InventoryFinding {
    InventoryFinding {
        resource_id: item.schema_registry_id.clone(),
        arn: format!("kafka:schema-registry/{}", item.schema_registry_id),
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

    fn registry(now: DateTime<Utc>) -> KafkaSchemaRegistryInventoryItem {
        KafkaSchemaRegistryInventoryItem {
            schema_registry_id: "prod:schema-registry".to_string(),
            cluster_name: "prod".to_string(),
            endpoint: Some("https://schema-registry.example.com".to_string()),
            subject_count: Some(100),
            schema_count: Some(250),
            compatibility_level: Some("BACKWARD".to_string()),
            owner: Some("platform".to_string()),
            labels: BTreeMap::new(),
            audit_evidence: true,
            auth_enabled: true,
            tls_enabled: true,
            has_schema_registry_evidence: true,
            collected_at: now,
        }
    }

    #[test]
    fn healthy_schema_registry_passes_claimed_pillars() {
        let now = Utc::now();
        let item = registry(now);

        for pillar in [Pillar::Cost, Pillar::Resilience, Pillar::Security] {
            let report =
                evaluate_kafka_schema_registry_inventory(std::slice::from_ref(&item), pillar, now);
            assert_eq!(report.resources_evaluated, 1);
            assert!(report.findings.is_empty());
            assert_eq!(report.stale_resources, 0);
            assert_eq!(report.score, 100);
        }
    }

    #[test]
    fn cost_flags_missing_owner_missing_evidence_and_high_subject_count() {
        let now = Utc::now();
        let mut missing_evidence = registry(now);
        missing_evidence.owner = None;
        missing_evidence.has_schema_registry_evidence = false;
        let mut large = registry(now);
        large.subject_count = Some(10_000);

        let report =
            evaluate_kafka_schema_registry_inventory(&[missing_evidence, large], Pillar::Cost, now);

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
            .any(|finding| finding.reason_code == REASON_COST_HIGH_SUBJECT_COUNT));
    }

    #[test]
    fn resilience_flags_missing_evidence_audit_and_compatibility() {
        let now = Utc::now();
        let mut missing_evidence = registry(now);
        missing_evidence.has_schema_registry_evidence = false;
        let mut risky = registry(now);
        risky.audit_evidence = false;
        risky.compatibility_level = Some("NONE".to_string());

        let report = evaluate_kafka_schema_registry_inventory(
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
            .any(|finding| finding.reason_code == REASON_RES_NO_AUDIT_EVIDENCE));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_RES_NO_COMPATIBILITY));
    }

    #[test]
    fn security_flags_missing_evidence_disabled_auth_and_tls() {
        let now = Utc::now();
        let mut missing_evidence = registry(now);
        missing_evidence.has_schema_registry_evidence = false;
        let mut insecure = registry(now);
        insecure.auth_enabled = false;
        insecure.tls_enabled = false;

        let report = evaluate_kafka_schema_registry_inventory(
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
            .any(|finding| finding.reason_code == REASON_SEC_AUTH_DISABLED));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_SEC_TLS_DISABLED));
    }

    #[test]
    fn stale_schema_registry_inventory_is_counted_for_any_pillar() {
        let now = Utc::now();
        let mut item = registry(now);
        item.collected_at = now - Duration::hours(DEFAULT_STALE_AFTER_HOURS + 2);

        let report = evaluate_kafka_schema_registry_inventory(&[item], Pillar::Security, now);

        assert_eq!(report.stale_resources, 1);
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_INV_STALE_DATA));
    }
}

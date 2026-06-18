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

// Deterministic Kafka ACL inventory evaluator for roadmap rows
// 04-KAFKA-DASHBOARD-MANAGEMENT-00638/00645/00666.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::config::KafkaClusterConfig;
use crate::services::aws::inventory::types::{
    score_pillar, InventoryFinding, Pillar, PillarReport, Severity, COST_ALLOCATION_TAG_KEYS,
    DEFAULT_STALE_AFTER_HOURS,
};

pub const RESOURCE_TYPE: &str = "KafkaAcl";
pub const REASON_COST_OWNER_NOT_RECORDED: &str = "KAFKA_ACL_COST_OWNER_NOT_RECORDED";
pub const REASON_COST_NO_ACL_EVIDENCE: &str = "KAFKA_ACL_COST_NO_EVIDENCE";
pub const REASON_COST_HIGH_ACL_COUNT: &str = "KAFKA_ACL_COST_HIGH_ACL_COUNT";
pub const REASON_RES_NO_ACL_EVIDENCE: &str = "KAFKA_ACL_RES_NO_EVIDENCE";
pub const REASON_RES_NO_AUDIT_EVIDENCE: &str = "KAFKA_ACL_RES_NO_AUDIT_EVIDENCE";
pub const REASON_RES_WILDCARD_WRITE_ACL: &str = "KAFKA_ACL_RES_WILDCARD_WRITE_ACL";
pub const REASON_SEC_NO_ACL_EVIDENCE: &str = "KAFKA_ACL_SEC_NO_EVIDENCE";
pub const REASON_SEC_WILDCARD_PRINCIPAL: &str = "KAFKA_ACL_SEC_WILDCARD_PRINCIPAL";
pub const REASON_SEC_ALLOW_ALL_OPERATION: &str = "KAFKA_ACL_SEC_ALLOW_ALL_OPERATION";
pub const REASON_SEC_PLAINTEXT: &str = "KAFKA_ACL_SEC_PLAINTEXT_PROTOCOL";
pub const REASON_INV_STALE_DATA: &str = "KAFKA_ACL_INV_STALE_DATA";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KafkaAclInventoryItem {
    pub acl_id: String,
    pub cluster_name: String,
    pub principal: String,
    pub resource_pattern: String,
    pub operation: String,
    pub permission_type: String,
    pub host: Option<String>,
    pub owner: Option<String>,
    pub labels: BTreeMap<String, String>,
    pub acl_count: Option<i32>,
    pub audit_evidence: bool,
    pub has_acl_evidence: bool,
    pub security_protocol: String,
    pub collected_at: DateTime<Utc>,
}

pub fn evaluate_kafka_acl_inventory(
    items: &[KafkaAclInventoryItem],
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

pub fn acl_inventory_item_from_config(
    cluster: &KafkaClusterConfig,
    collected_at: DateTime<Utc>,
) -> KafkaAclInventoryItem {
    KafkaAclInventoryItem {
        acl_id: format!("{}:acls", cluster.name),
        cluster_name: cluster.name.clone(),
        principal: "<uncollected>".to_string(),
        resource_pattern: "<uncollected>".to_string(),
        operation: "<uncollected>".to_string(),
        permission_type: "<uncollected>".to_string(),
        host: None,
        owner: None,
        labels: BTreeMap::new(),
        acl_count: None,
        audit_evidence: false,
        has_acl_evidence: false,
        security_protocol: cluster.security_protocol.clone(),
        collected_at,
    }
}

fn evaluate_cost(
    item: &KafkaAclInventoryItem,
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
                "Kafka ACL inventory for cluster {} has no owner, team, project, or cost-center metadata",
                item.cluster_name
            ),
            json!({
                "acl_id": item.acl_id,
                "cluster_name": item.cluster_name,
                "checked_keys": COST_ALLOCATION_TAG_KEYS,
            }),
        ));
    }

    if !item.has_acl_evidence {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_NO_ACL_EVIDENCE,
            Severity::High,
            format!(
                "Kafka ACL inventory for cluster {} has no ACL evidence",
                item.cluster_name
            ),
            json!({
                "acl_id": item.acl_id,
                "cluster_name": item.cluster_name,
                "recommendation": "Collect ACL bindings, principals, operations, ownership, labels, and audit evidence before estimating ACL operational cost",
            }),
        ));
    }

    if item.acl_count.is_some_and(|count| count >= 1_000) {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_HIGH_ACL_COUNT,
            Severity::Medium,
            format!("Kafka ACL inventory {} has high ACL volume", item.acl_id),
            json!({
                "acl_id": item.acl_id,
                "acl_count": item.acl_count,
                "recommendation": "Review duplicate ACLs, broad patterns, and automation loops before expanding operational scope",
            }),
        ));
    }
}

fn evaluate_resilience(
    item: &KafkaAclInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if !item.has_acl_evidence {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_NO_ACL_EVIDENCE,
            Severity::High,
            format!(
                "Kafka ACL inventory for cluster {} has no resilience evidence",
                item.cluster_name
            ),
            json!({
                "acl_id": item.acl_id,
                "cluster_name": item.cluster_name,
                "recommendation": "Collect ACL bindings and audit evidence before relying on ACL posture for recovery or rollback",
            }),
        ));
    }

    if item.has_acl_evidence && !item.audit_evidence {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_NO_AUDIT_EVIDENCE,
            Severity::High,
            format!("Kafka ACL {} has no audit evidence", item.acl_id),
            json!({
                "acl_id": item.acl_id,
                "audit_evidence": item.audit_evidence,
                "recommendation": "Enable ACL change audit evidence before using ACL posture for incident rollback",
            }),
        ));
    }

    if item.has_acl_evidence
        && item.permission_type.eq_ignore_ascii_case("allow")
        && item.resource_pattern.contains('*')
        && matches!(
            item.operation.to_ascii_uppercase().as_str(),
            "WRITE" | "ALL"
        )
    {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_WILDCARD_WRITE_ACL,
            Severity::High,
            format!("Kafka ACL {} grants wildcard write authority", item.acl_id),
            json!({
                "acl_id": item.acl_id,
                "operation": item.operation,
                "resource_pattern": item.resource_pattern,
                "recommendation": "Constrain wildcard write ACLs and require approval before accepting control-plane resilience posture",
            }),
        ));
    }
}

fn evaluate_security(
    item: &KafkaAclInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if !item.has_acl_evidence {
        findings.push(finding(
            item,
            pillar,
            REASON_SEC_NO_ACL_EVIDENCE,
            Severity::High,
            format!(
                "Kafka ACL inventory for cluster {} has no ACL evidence for security review",
                item.cluster_name
            ),
            json!({
                "acl_id": item.acl_id,
                "cluster_name": item.cluster_name,
                "recommendation": "Collect ACL bindings, principals, operations, resources, and hosts before assessing access posture",
            }),
        ));
    }

    if item.has_acl_evidence && wildcard(&item.principal) {
        findings.push(finding(
            item,
            pillar,
            REASON_SEC_WILDCARD_PRINCIPAL,
            Severity::High,
            format!("Kafka ACL {} uses wildcard principal", item.acl_id),
            json!({
                "acl_id": item.acl_id,
                "principal": item.principal,
                "recommendation": "Replace wildcard principals with scoped authenticated principals",
            }),
        ));
    }

    if item.has_acl_evidence
        && item.permission_type.eq_ignore_ascii_case("allow")
        && item.operation.eq_ignore_ascii_case("all")
    {
        findings.push(finding(
            item,
            pillar,
            REASON_SEC_ALLOW_ALL_OPERATION,
            Severity::High,
            format!("Kafka ACL {} allows all operations", item.acl_id),
            json!({
                "acl_id": item.acl_id,
                "operation": item.operation,
                "permission_type": item.permission_type,
                "recommendation": "Scope ACL operations to the minimum required permissions",
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
                "Kafka ACL inventory for cluster {} uses PLAINTEXT transport",
                item.cluster_name
            ),
            json!({
                "acl_id": item.acl_id,
                "security_protocol": item.security_protocol,
                "recommendation": "Use encrypted Kafka listener protocols before accepting ACL posture",
            }),
        ));
    }
}

fn stale_finding(
    item: &KafkaAclInventoryItem,
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
            "Kafka ACL inventory for cluster {} is {} hour(s) old",
            item.cluster_name, age_hours
        ),
        json!({
            "acl_id": item.acl_id,
            "cluster_name": item.cluster_name,
            "age_hours": age_hours,
            "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
            "recommendation": "Refresh Kafka ACL inventory before acting on this posture report",
        }),
    ))
}

fn has_owner_metadata(item: &KafkaAclInventoryItem) -> bool {
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

fn wildcard(value: &str) -> bool {
    value.trim() == "*" || value.trim().eq_ignore_ascii_case("User:*")
}

fn finding(
    item: &KafkaAclInventoryItem,
    pillar: Pillar,
    reason_code: &'static str,
    severity: Severity,
    message: String,
    evidence: Value,
) -> InventoryFinding {
    InventoryFinding {
        resource_id: item.acl_id.clone(),
        arn: format!("kafka:acl/{}", item.acl_id),
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

    fn acl(now: DateTime<Utc>) -> KafkaAclInventoryItem {
        KafkaAclInventoryItem {
            acl_id: "prod:acl:payments-read".to_string(),
            cluster_name: "prod".to_string(),
            principal: "User:payments".to_string(),
            resource_pattern: "Topic:orders".to_string(),
            operation: "READ".to_string(),
            permission_type: "ALLOW".to_string(),
            host: Some("*".to_string()),
            owner: Some("payments".to_string()),
            labels: BTreeMap::new(),
            acl_count: Some(20),
            audit_evidence: true,
            has_acl_evidence: true,
            security_protocol: "SASL_SSL".to_string(),
            collected_at: now,
        }
    }

    #[test]
    fn healthy_acl_passes_claimed_pillars() {
        let now = Utc::now();
        let item = acl(now);

        for pillar in [Pillar::Cost, Pillar::Resilience, Pillar::Security] {
            let report = evaluate_kafka_acl_inventory(std::slice::from_ref(&item), pillar, now);
            assert_eq!(report.resources_evaluated, 1);
            assert!(report.findings.is_empty());
            assert_eq!(report.stale_resources, 0);
            assert_eq!(report.score, 100);
        }
    }

    #[test]
    fn cost_flags_missing_owner_missing_evidence_and_high_count() {
        let now = Utc::now();
        let mut item = acl(now);
        item.owner = None;
        item.has_acl_evidence = false;
        item.acl_count = Some(1_500);

        let report = evaluate_kafka_acl_inventory(&[item], Pillar::Cost, now);

        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_COST_OWNER_NOT_RECORDED));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_COST_NO_ACL_EVIDENCE));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_COST_HIGH_ACL_COUNT));
    }

    #[test]
    fn resilience_flags_missing_evidence_audit_and_wildcard_write() {
        let now = Utc::now();
        let mut missing_evidence = acl(now);
        missing_evidence.has_acl_evidence = false;
        let mut risky = acl(now);
        risky.audit_evidence = false;
        risky.resource_pattern = "Topic:*".to_string();
        risky.operation = "WRITE".to_string();

        let report =
            evaluate_kafka_acl_inventory(&[missing_evidence, risky], Pillar::Resilience, now);

        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_RES_NO_ACL_EVIDENCE));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_RES_NO_AUDIT_EVIDENCE));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_RES_WILDCARD_WRITE_ACL));
    }

    #[test]
    fn security_flags_missing_evidence_wildcard_allow_all_and_plaintext() {
        let now = Utc::now();
        let mut missing_evidence = acl(now);
        missing_evidence.has_acl_evidence = false;
        let mut insecure = acl(now);
        insecure.principal = "User:*".to_string();
        insecure.operation = "ALL".to_string();
        insecure.security_protocol = "PLAINTEXT".to_string();

        let report =
            evaluate_kafka_acl_inventory(&[missing_evidence, insecure], Pillar::Security, now);

        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_SEC_NO_ACL_EVIDENCE));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_SEC_WILDCARD_PRINCIPAL));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_SEC_ALLOW_ALL_OPERATION));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_SEC_PLAINTEXT));
    }

    #[test]
    fn stale_acl_inventory_is_counted_for_any_pillar() {
        let now = Utc::now();
        let mut item = acl(now);
        item.collected_at = now - Duration::hours(DEFAULT_STALE_AFTER_HOURS + 2);

        let report = evaluate_kafka_acl_inventory(&[item], Pillar::Security, now);

        assert_eq!(report.stale_resources, 1);
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_INV_STALE_DATA));
    }
}

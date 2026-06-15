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

// Deterministic Kafka Admin API inventory evaluator for roadmap rows
// 04-KAFKA-DASHBOARD-MANAGEMENT-00589/00596/00617.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::config::KafkaClusterConfig;
use crate::services::aws::inventory::types::{
    score_pillar, InventoryFinding, Pillar, PillarReport, Severity, COST_ALLOCATION_TAG_KEYS,
    DEFAULT_STALE_AFTER_HOURS,
};

pub const RESOURCE_TYPE: &str = "KafkaAdminApi";
pub const REASON_COST_OWNER_NOT_RECORDED: &str = "KAFKA_ADMIN_API_COST_OWNER_NOT_RECORDED";
pub const REASON_COST_NO_ADMIN_API_EVIDENCE: &str = "KAFKA_ADMIN_API_COST_NO_EVIDENCE";
pub const REASON_COST_HIGH_OPERATION_COUNT: &str = "KAFKA_ADMIN_API_COST_HIGH_OPERATION_COUNT";
pub const REASON_RES_NO_ADMIN_API_EVIDENCE: &str = "KAFKA_ADMIN_API_RES_NO_EVIDENCE";
pub const REASON_RES_AUDIT_LOGGING_DISABLED: &str = "KAFKA_ADMIN_API_RES_AUDIT_LOGGING_DISABLED";
pub const REASON_RES_DESTRUCTIVE_OPERATIONS_EXPOSED: &str =
    "KAFKA_ADMIN_API_RES_DESTRUCTIVE_OPERATIONS_EXPOSED";
pub const REASON_SEC_NO_ADMIN_API_EVIDENCE: &str = "KAFKA_ADMIN_API_SEC_NO_EVIDENCE";
pub const REASON_SEC_NO_PRINCIPAL_EVIDENCE: &str = "KAFKA_ADMIN_API_SEC_NO_PRINCIPAL_EVIDENCE";
pub const REASON_SEC_NO_ACL_EVIDENCE: &str = "KAFKA_ADMIN_API_SEC_NO_ACL_EVIDENCE";
pub const REASON_SEC_PLAINTEXT: &str = "KAFKA_ADMIN_API_SEC_PLAINTEXT_PROTOCOL";
pub const REASON_INV_STALE_DATA: &str = "KAFKA_ADMIN_API_INV_STALE_DATA";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KafkaAdminApiInventoryItem {
    pub admin_api_id: String,
    pub cluster_name: String,
    pub endpoint_name: String,
    pub owner: Option<String>,
    pub labels: BTreeMap<String, String>,
    pub operation_count: Option<i32>,
    pub destructive_operation_count: Option<i32>,
    pub audit_logging_enabled: Option<bool>,
    pub principal: Option<String>,
    pub has_admin_api_evidence: bool,
    pub has_acl_evidence: bool,
    pub security_protocol: String,
    pub collected_at: DateTime<Utc>,
}

pub fn evaluate_kafka_admin_api_inventory(
    items: &[KafkaAdminApiInventoryItem],
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

pub fn admin_api_inventory_item_from_config(
    cluster: &KafkaClusterConfig,
    collected_at: DateTime<Utc>,
) -> KafkaAdminApiInventoryItem {
    KafkaAdminApiInventoryItem {
        admin_api_id: format!("{}:admin-api", cluster.name),
        cluster_name: cluster.name.clone(),
        endpoint_name: "<uncollected>".to_string(),
        owner: None,
        labels: BTreeMap::new(),
        operation_count: None,
        destructive_operation_count: None,
        audit_logging_enabled: None,
        principal: None,
        has_admin_api_evidence: false,
        has_acl_evidence: false,
        security_protocol: cluster.security_protocol.clone(),
        collected_at,
    }
}

fn evaluate_cost(
    item: &KafkaAdminApiInventoryItem,
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
                "Kafka Admin API inventory for cluster {} has no owner, team, project, or cost-center metadata",
                item.cluster_name
            ),
            json!({
                "admin_api_id": item.admin_api_id,
                "cluster_name": item.cluster_name,
                "checked_keys": COST_ALLOCATION_TAG_KEYS,
            }),
        ));
    }

    if !item.has_admin_api_evidence {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_NO_ADMIN_API_EVIDENCE,
            Severity::High,
            format!(
                "Kafka Admin API inventory for cluster {} has no Admin API evidence",
                item.cluster_name
            ),
            json!({
                "admin_api_id": item.admin_api_id,
                "cluster_name": item.cluster_name,
                "recommendation": "Collect Admin API endpoints, operation counts, ownership, labels, principals, and audit state before estimating operational cost",
            }),
        ));
    }

    if item.operation_count.is_some_and(|count| count >= 10_000) {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_HIGH_OPERATION_COUNT,
            Severity::Medium,
            format!("Kafka Admin API {} has high operation volume", item.endpoint_name),
            json!({
                "admin_api_id": item.admin_api_id,
                "operation_count": item.operation_count,
                "recommendation": "Review automation loops and polling frequency before scaling control-plane access or broker resources",
            }),
        ));
    }
}

fn evaluate_resilience(
    item: &KafkaAdminApiInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if !item.has_admin_api_evidence {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_NO_ADMIN_API_EVIDENCE,
            Severity::High,
            format!(
                "Kafka Admin API inventory for cluster {} has no resilience evidence",
                item.cluster_name
            ),
            json!({
                "admin_api_id": item.admin_api_id,
                "cluster_name": item.cluster_name,
                "recommendation": "Collect Admin API operation, audit, and approval evidence before evaluating control-plane resilience",
            }),
        ));
    }

    if item.has_admin_api_evidence && item.audit_logging_enabled == Some(false) {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_AUDIT_LOGGING_DISABLED,
            Severity::High,
            format!("Kafka Admin API {} has audit logging disabled", item.endpoint_name),
            json!({
                "admin_api_id": item.admin_api_id,
                "audit_logging_enabled": item.audit_logging_enabled,
                "recommendation": "Enable audit logging before relying on Admin API changes for recovery or rollback decisions",
            }),
        ));
    }

    if item
        .destructive_operation_count
        .is_some_and(|count| count > 0)
    {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_DESTRUCTIVE_OPERATIONS_EXPOSED,
            Severity::High,
            format!(
                "Kafka Admin API {} exposes destructive operation evidence",
                item.endpoint_name
            ),
            json!({
                "admin_api_id": item.admin_api_id,
                "destructive_operation_count": item.destructive_operation_count,
                "recommendation": "Require approval, replayable audit events, and rollback notes for destructive Admin API operations",
            }),
        ));
    }
}

fn evaluate_security(
    item: &KafkaAdminApiInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if !item.has_admin_api_evidence {
        findings.push(finding(
            item,
            pillar,
            REASON_SEC_NO_ADMIN_API_EVIDENCE,
            Severity::Medium,
            format!(
                "Kafka Admin API inventory for cluster {} has no Admin API evidence for security review",
                item.cluster_name
            ),
            json!({
                "admin_api_id": item.admin_api_id,
                "cluster_name": item.cluster_name,
                "recommendation": "Collect Admin API endpoints, principals, ACLs, and audit evidence before assessing administrative access",
            }),
        ));
    }

    if item.has_admin_api_evidence && item.principal.as_deref().unwrap_or("").trim().is_empty() {
        findings.push(finding(
            item,
            pillar,
            REASON_SEC_NO_PRINCIPAL_EVIDENCE,
            Severity::High,
            format!("Kafka Admin API {} has no principal evidence", item.endpoint_name),
            json!({
                "admin_api_id": item.admin_api_id,
                "principal": item.principal,
                "recommendation": "Map Admin API access to authenticated principals before accepting administrative posture",
            }),
        ));
    }

    if item.has_admin_api_evidence && !item.has_acl_evidence {
        findings.push(finding(
            item,
            pillar,
            REASON_SEC_NO_ACL_EVIDENCE,
            Severity::High,
            format!("Kafka Admin API {} has no ACL evidence", item.endpoint_name),
            json!({
                "admin_api_id": item.admin_api_id,
                "has_acl_evidence": item.has_acl_evidence,
                "recommendation": "Collect scoped Admin API ACLs before accepting administrative access posture",
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
                "Kafka Admin API inventory for cluster {} uses PLAINTEXT transport",
                item.cluster_name
            ),
            json!({
                "admin_api_id": item.admin_api_id,
                "security_protocol": item.security_protocol,
                "recommendation": "Use encrypted Kafka listener protocols before accepting Admin API posture",
            }),
        ));
    }
}

fn stale_finding(
    item: &KafkaAdminApiInventoryItem,
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
            "Kafka Admin API inventory for cluster {} is {} hour(s) old",
            item.cluster_name, age_hours
        ),
        json!({
            "admin_api_id": item.admin_api_id,
            "cluster_name": item.cluster_name,
            "age_hours": age_hours,
            "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
            "recommendation": "Refresh Kafka Admin API inventory before acting on this posture report",
        }),
    ))
}

fn has_owner_metadata(item: &KafkaAdminApiInventoryItem) -> bool {
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
    item: &KafkaAdminApiInventoryItem,
    pillar: Pillar,
    reason_code: &'static str,
    severity: Severity,
    message: String,
    evidence: Value,
) -> InventoryFinding {
    InventoryFinding {
        resource_id: item.admin_api_id.clone(),
        arn: format!("kafka:admin-api/{}", item.admin_api_id),
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

    fn admin_api(now: DateTime<Utc>) -> KafkaAdminApiInventoryItem {
        KafkaAdminApiInventoryItem {
            admin_api_id: "prod:admin-api".to_string(),
            cluster_name: "prod".to_string(),
            endpoint_name: "admin-api".to_string(),
            owner: Some("platform".to_string()),
            labels: BTreeMap::new(),
            operation_count: Some(100),
            destructive_operation_count: Some(0),
            audit_logging_enabled: Some(true),
            principal: Some("User:platform-admin".to_string()),
            has_admin_api_evidence: true,
            has_acl_evidence: true,
            security_protocol: "SASL_SSL".to_string(),
            collected_at: now,
        }
    }

    #[test]
    fn healthy_admin_api_passes_claimed_pillars() {
        let now = Utc::now();
        let item = admin_api(now);

        for pillar in [Pillar::Cost, Pillar::Resilience, Pillar::Security] {
            let report =
                evaluate_kafka_admin_api_inventory(std::slice::from_ref(&item), pillar, now);
            assert_eq!(report.resources_evaluated, 1);
            assert!(report.findings.is_empty());
            assert_eq!(report.stale_resources, 0);
            assert_eq!(report.score, 100);
        }
    }

    #[test]
    fn cost_flags_missing_owner_missing_evidence_and_high_operations() {
        let now = Utc::now();
        let mut item = admin_api(now);
        item.owner = None;
        item.has_admin_api_evidence = false;
        item.operation_count = Some(15_000);

        let report = evaluate_kafka_admin_api_inventory(&[item], Pillar::Cost, now);

        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_COST_OWNER_NOT_RECORDED));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_COST_NO_ADMIN_API_EVIDENCE));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_COST_HIGH_OPERATION_COUNT));
    }

    #[test]
    fn resilience_flags_missing_evidence_audit_and_destructive_ops() {
        let now = Utc::now();
        let mut missing_evidence = admin_api(now);
        missing_evidence.has_admin_api_evidence = false;
        let mut risky = admin_api(now);
        risky.audit_logging_enabled = Some(false);
        risky.destructive_operation_count = Some(2);

        let report =
            evaluate_kafka_admin_api_inventory(&[missing_evidence, risky], Pillar::Resilience, now);

        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_RES_NO_ADMIN_API_EVIDENCE));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_RES_AUDIT_LOGGING_DISABLED));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_RES_DESTRUCTIVE_OPERATIONS_EXPOSED));
    }

    #[test]
    fn security_flags_missing_evidence_missing_acl_principal_and_plaintext() {
        let now = Utc::now();
        let mut missing_evidence = admin_api(now);
        missing_evidence.has_admin_api_evidence = false;
        let mut insecure = admin_api(now);
        insecure.principal = None;
        insecure.has_acl_evidence = false;
        insecure.security_protocol = "PLAINTEXT".to_string();

        let report = evaluate_kafka_admin_api_inventory(
            &[missing_evidence, insecure],
            Pillar::Security,
            now,
        );

        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_SEC_NO_ADMIN_API_EVIDENCE));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_SEC_NO_PRINCIPAL_EVIDENCE));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_SEC_NO_ACL_EVIDENCE));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_SEC_PLAINTEXT));
    }

    #[test]
    fn stale_admin_api_inventory_is_counted_for_any_pillar() {
        let now = Utc::now();
        let mut item = admin_api(now);
        item.collected_at = now - Duration::hours(DEFAULT_STALE_AFTER_HOURS + 2);

        let report = evaluate_kafka_admin_api_inventory(&[item], Pillar::Security, now);

        assert_eq!(report.stale_resources, 1);
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_INV_STALE_DATA));
    }
}

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

// Deterministic Kafka restore inventory evaluator for roadmap rows
// 04-KAFKA-DASHBOARD-MANAGEMENT-01373/01380/01401.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::config::KafkaClusterConfig;
use crate::services::aws::inventory::types::{
    score_pillar, InventoryFinding, Pillar, PillarReport, Severity, COST_ALLOCATION_TAG_KEYS,
    DEFAULT_STALE_AFTER_HOURS,
};

pub const RESOURCE_TYPE: &str = "KafkaRestore";
pub const REASON_COST_OWNER_NOT_RECORDED: &str = "KAFKA_RESTORE_COST_OWNER_NOT_RECORDED";
pub const REASON_COST_NO_EVIDENCE: &str = "KAFKA_RESTORE_COST_NO_EVIDENCE";
pub const REASON_COST_HIGH_RTO: &str = "KAFKA_RESTORE_COST_HIGH_RTO";
pub const REASON_RES_NO_EVIDENCE: &str = "KAFKA_RESTORE_RES_NO_EVIDENCE";
pub const REASON_RES_NO_RESTORE_TEST: &str = "KAFKA_RESTORE_RES_NO_RESTORE_TEST";
pub const REASON_RES_NO_DATA_VALIDATION: &str = "KAFKA_RESTORE_RES_NO_DATA_VALIDATION";
pub const REASON_SEC_NO_EVIDENCE: &str = "KAFKA_RESTORE_SEC_NO_EVIDENCE";
pub const REASON_SEC_NO_ACCESS_CONTROL_EVIDENCE: &str =
    "KAFKA_RESTORE_SEC_NO_ACCESS_CONTROL_EVIDENCE";
pub const REASON_SEC_PLAINTEXT_CONNECTION: &str = "KAFKA_RESTORE_SEC_PLAINTEXT_CONNECTION";
pub const REASON_INV_STALE_DATA: &str = "KAFKA_RESTORE_INV_STALE_DATA";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestoreInventoryItem {
    pub restore_id: String,
    pub cluster_name: String,
    pub restore_source: Option<String>,
    pub target_cluster: Option<String>,
    pub rto_minutes: Option<u32>,
    pub rpo_minutes: Option<u32>,
    pub last_restore_test_at: Option<DateTime<Utc>>,
    pub owner: Option<String>,
    pub labels: BTreeMap<String, String>,
    pub restore_test_evidence: bool,
    pub data_validation_evidence: bool,
    pub access_control_evidence: bool,
    pub plaintext_connection: bool,
    pub has_restore_evidence: bool,
    pub collected_at: DateTime<Utc>,
}

pub fn evaluate_restore_inventory(
    items: &[RestoreInventoryItem],
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

pub fn restore_inventory_item_from_config(
    cluster: &KafkaClusterConfig,
    collected_at: DateTime<Utc>,
) -> RestoreInventoryItem {
    RestoreInventoryItem {
        restore_id: format!("{}:restore", cluster.name),
        cluster_name: cluster.name.clone(),
        restore_source: None,
        target_cluster: None,
        rto_minutes: None,
        rpo_minutes: None,
        last_restore_test_at: None,
        owner: None,
        labels: BTreeMap::new(),
        restore_test_evidence: false,
        data_validation_evidence: false,
        access_control_evidence: false,
        plaintext_connection: cluster.security_protocol.eq_ignore_ascii_case("PLAINTEXT"),
        has_restore_evidence: false,
        collected_at,
    }
}

fn evaluate_cost(
    item: &RestoreInventoryItem,
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
                "Kafka restore inventory for cluster {} has no owner, team, project, or cost-center metadata",
                item.cluster_name
            ),
            json!({
                "restore_id": item.restore_id,
                "cluster_name": item.cluster_name,
                "checked_keys": COST_ALLOCATION_TAG_KEYS,
            }),
        ));
    }

    if !item.has_restore_evidence {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_NO_EVIDENCE,
            Severity::High,
            format!(
                "Kafka restore inventory for cluster {} has no restore evidence",
                item.cluster_name
            ),
            json!({
                "restore_id": item.restore_id,
                "cluster_name": item.cluster_name,
                "recommendation": "Collect restore source, target, RTO, RPO, ownership, restore test, and validation evidence before estimating restore cost posture",
            }),
        ));
    }

    if item
        .rto_minutes
        .is_some_and(|rto_minutes| rto_minutes > 240)
    {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_HIGH_RTO,
            Severity::Medium,
            format!("Kafka restore {} has a high RTO target", item.restore_id),
            json!({
                "restore_id": item.restore_id,
                "rto_minutes": item.rto_minutes,
                "recommendation": "Review restore automation effort and standby capacity before accepting a high recovery objective",
            }),
        ));
    }
}

fn evaluate_resilience(
    item: &RestoreInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if !item.has_restore_evidence {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_NO_EVIDENCE,
            Severity::High,
            format!(
                "Kafka restore inventory for cluster {} has no resilience evidence",
                item.cluster_name
            ),
            json!({
                "restore_id": item.restore_id,
                "cluster_name": item.cluster_name,
                "recommendation": "Collect restore plans, restore test runs, data validation, RTO/RPO, source durability, and target compatibility before accepting recovery posture",
            }),
        ));
    }

    if item.has_restore_evidence && !item.restore_test_evidence {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_NO_RESTORE_TEST,
            Severity::High,
            format!("Kafka restore {} has no restore test evidence", item.restore_id),
            json!({
                "restore_id": item.restore_id,
                "last_restore_test_at": item.last_restore_test_at,
                "recommendation": "Run and record restore tests before accepting Kafka restore resilience posture",
            }),
        ));
    }

    if item.has_restore_evidence && !item.data_validation_evidence {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_NO_DATA_VALIDATION,
            Severity::High,
            format!(
                "Kafka restore {} has no restored data validation evidence",
                item.restore_id
            ),
            json!({
                "restore_id": item.restore_id,
                "target_cluster": item.target_cluster,
                "recommendation": "Record topic, offset, schema, and message-count validation before accepting restore readiness",
            }),
        ));
    }
}

fn evaluate_security(
    item: &RestoreInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if !item.has_restore_evidence {
        findings.push(finding(
            item,
            pillar,
            REASON_SEC_NO_EVIDENCE,
            Severity::High,
            format!(
                "Kafka restore inventory for cluster {} has no security evidence",
                item.cluster_name
            ),
            json!({
                "restore_id": item.restore_id,
                "cluster_name": item.cluster_name,
                "recommendation": "Collect restore identities, target access policy, secrets handling, transport, and audit evidence before accepting security posture",
            }),
        ));
    }

    if item.has_restore_evidence && !item.access_control_evidence {
        findings.push(finding(
            item,
            pillar,
            REASON_SEC_NO_ACCESS_CONTROL_EVIDENCE,
            Severity::High,
            format!(
                "Kafka restore {} has no access control evidence",
                item.restore_id
            ),
            json!({
                "restore_id": item.restore_id,
                "target_cluster": item.target_cluster,
                "recommendation": "Record scoped restore credentials and target ACL validation before accepting security posture",
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
                "Kafka restore {} uses PLAINTEXT or unverified transport",
                item.restore_id
            ),
            json!({
                "restore_id": item.restore_id,
                "plaintext_connection": item.plaintext_connection,
                "recommendation": "Use encrypted Kafka and restore-target transport before accepting security posture",
            }),
        ));
    }
}

fn stale_finding(
    item: &RestoreInventoryItem,
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
            "Kafka restore inventory for cluster {} is {} hour(s) old",
            item.cluster_name, age_hours
        ),
        json!({
            "restore_id": item.restore_id,
            "cluster_name": item.cluster_name,
            "age_hours": age_hours,
            "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
            "recommendation": "Refresh Kafka restore inventory before acting on this posture report",
        }),
    ))
}

fn has_owner_metadata(item: &RestoreInventoryItem) -> bool {
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
    item: &RestoreInventoryItem,
    pillar: Pillar,
    reason_code: &'static str,
    severity: Severity,
    message: String,
    evidence: Value,
) -> InventoryFinding {
    InventoryFinding {
        resource_id: item.restore_id.clone(),
        arn: format!("kafka:restore/{}", item.restore_id),
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

    fn restore(now: DateTime<Utc>) -> RestoreInventoryItem {
        RestoreInventoryItem {
            restore_id: "prod:restore".to_string(),
            cluster_name: "prod".to_string(),
            restore_source: Some("s3://prod-kafka-backups".to_string()),
            target_cluster: Some("prod-dr".to_string()),
            rto_minutes: Some(120),
            rpo_minutes: Some(60),
            last_restore_test_at: Some(now - Duration::days(7)),
            owner: Some("platform".to_string()),
            labels: BTreeMap::new(),
            restore_test_evidence: true,
            data_validation_evidence: true,
            access_control_evidence: true,
            plaintext_connection: false,
            has_restore_evidence: true,
            collected_at: now,
        }
    }

    #[test]
    fn healthy_restore_passes_claimed_pillars() {
        let now = Utc::now();
        let item = restore(now);

        for pillar in [Pillar::Cost, Pillar::Resilience, Pillar::Security] {
            let report = evaluate_restore_inventory(std::slice::from_ref(&item), pillar, now);
            assert_eq!(report.resources_evaluated, 1);
            assert!(report.findings.is_empty());
            assert_eq!(report.stale_resources, 0);
            assert_eq!(report.score, 100);
        }
    }

    #[test]
    fn cost_flags_missing_owner_missing_evidence_and_high_rto() {
        let now = Utc::now();
        let mut missing_evidence = restore(now);
        missing_evidence.owner = None;
        missing_evidence.has_restore_evidence = false;
        let mut high_rto = restore(now);
        high_rto.rto_minutes = Some(241);

        let report = evaluate_restore_inventory(&[missing_evidence, high_rto], Pillar::Cost, now);

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
            .any(|finding| finding.reason_code == REASON_COST_HIGH_RTO));
    }

    #[test]
    fn resilience_flags_missing_evidence_restore_test_and_data_validation() {
        let now = Utc::now();
        let mut missing_evidence = restore(now);
        missing_evidence.has_restore_evidence = false;
        let mut risky = restore(now);
        risky.restore_test_evidence = false;
        risky.data_validation_evidence = false;

        let report =
            evaluate_restore_inventory(&[missing_evidence, risky], Pillar::Resilience, now);

        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_RES_NO_EVIDENCE));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_RES_NO_RESTORE_TEST));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_RES_NO_DATA_VALIDATION));
    }

    #[test]
    fn security_flags_missing_evidence_access_control_and_plaintext() {
        let now = Utc::now();
        let mut missing_evidence = restore(now);
        missing_evidence.has_restore_evidence = false;
        let mut insecure = restore(now);
        insecure.access_control_evidence = false;
        insecure.plaintext_connection = true;

        let report =
            evaluate_restore_inventory(&[missing_evidence, insecure], Pillar::Security, now);

        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_SEC_NO_EVIDENCE));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_SEC_NO_ACCESS_CONTROL_EVIDENCE));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_SEC_PLAINTEXT_CONNECTION));
    }

    #[test]
    fn stale_restore_inventory_is_counted_for_any_pillar() {
        let now = Utc::now();
        let mut item = restore(now);
        item.collected_at = now - Duration::hours(DEFAULT_STALE_AFTER_HOURS + 2);

        let report = evaluate_restore_inventory(&[item], Pillar::Security, now);

        assert_eq!(report.stale_resources, 1);
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_INV_STALE_DATA));
    }
}

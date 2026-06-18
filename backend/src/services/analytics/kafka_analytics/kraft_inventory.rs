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

// Deterministic KRaft inventory evaluator for roadmap rows
// 04-KAFKA-DASHBOARD-MANAGEMENT-01226/01233/01254.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::config::KafkaClusterConfig;
use crate::services::aws::inventory::types::{
    score_pillar, InventoryFinding, Pillar, PillarReport, Severity, COST_ALLOCATION_TAG_KEYS,
    DEFAULT_STALE_AFTER_HOURS,
};

pub const RESOURCE_TYPE: &str = "KafkaKRaft";
pub const REASON_COST_OWNER_NOT_RECORDED: &str = "KAFKA_KRAFT_COST_OWNER_NOT_RECORDED";
pub const REASON_COST_NO_EVIDENCE: &str = "KAFKA_KRAFT_COST_NO_EVIDENCE";
pub const REASON_COST_HIGH_CONTROLLER_COUNT: &str = "KAFKA_KRAFT_COST_HIGH_CONTROLLER_COUNT";
pub const REASON_RES_NO_EVIDENCE: &str = "KAFKA_KRAFT_RES_NO_EVIDENCE";
pub const REASON_RES_NO_QUORUM_EVIDENCE: &str = "KAFKA_KRAFT_RES_NO_QUORUM_EVIDENCE";
pub const REASON_RES_UNHEALTHY_VOTERS: &str = "KAFKA_KRAFT_RES_UNHEALTHY_VOTERS";
pub const REASON_SEC_NO_EVIDENCE: &str = "KAFKA_KRAFT_SEC_NO_EVIDENCE";
pub const REASON_SEC_NO_CONTROLLER_PRINCIPAL: &str = "KAFKA_KRAFT_SEC_NO_CONTROLLER_PRINCIPAL";
pub const REASON_SEC_PLAINTEXT_CONNECTION: &str = "KAFKA_KRAFT_SEC_PLAINTEXT_CONNECTION";
pub const REASON_INV_STALE_DATA: &str = "KAFKA_KRAFT_INV_STALE_DATA";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KraftInventoryItem {
    pub kraft_id: String,
    pub cluster_name: String,
    pub controller_count: Option<u32>,
    pub voter_count: Option<u32>,
    pub unhealthy_voter_count: Option<u32>,
    pub metadata_log_dir: Option<String>,
    pub metadata_quorum_id: Option<String>,
    pub owner: Option<String>,
    pub labels: BTreeMap<String, String>,
    pub quorum_evidence: bool,
    pub controller_principal: Option<String>,
    pub plaintext_connection: bool,
    pub has_kraft_evidence: bool,
    pub collected_at: DateTime<Utc>,
}

pub fn evaluate_kraft_inventory(
    items: &[KraftInventoryItem],
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

pub fn kraft_inventory_item_from_config(
    cluster: &KafkaClusterConfig,
    collected_at: DateTime<Utc>,
) -> KraftInventoryItem {
    KraftInventoryItem {
        kraft_id: format!("{}:kraft", cluster.name),
        cluster_name: cluster.name.clone(),
        controller_count: None,
        voter_count: None,
        unhealthy_voter_count: None,
        metadata_log_dir: None,
        metadata_quorum_id: None,
        owner: None,
        labels: BTreeMap::new(),
        quorum_evidence: false,
        controller_principal: None,
        plaintext_connection: cluster.security_protocol.eq_ignore_ascii_case("PLAINTEXT"),
        has_kraft_evidence: false,
        collected_at,
    }
}

fn evaluate_cost(item: &KraftInventoryItem, pillar: Pillar, findings: &mut Vec<InventoryFinding>) {
    if !has_owner_metadata(item) {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_OWNER_NOT_RECORDED,
            Severity::Medium,
            format!(
                "Kafka KRaft inventory for cluster {} has no owner, team, project, or cost-center metadata",
                item.cluster_name
            ),
            json!({
                "kraft_id": item.kraft_id,
                "cluster_name": item.cluster_name,
                "checked_keys": COST_ALLOCATION_TAG_KEYS,
            }),
        ));
    }

    if !item.has_kraft_evidence {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_NO_EVIDENCE,
            Severity::High,
            format!(
                "Kafka KRaft inventory for cluster {} has no controller evidence",
                item.cluster_name
            ),
            json!({
                "kraft_id": item.kraft_id,
                "cluster_name": item.cluster_name,
                "recommendation": "Collect controller count, quorum voters, metadata log directory, ownership, and cluster mode evidence before estimating KRaft cost posture",
            }),
        ));
    }

    if item
        .controller_count
        .is_some_and(|controller_count| controller_count >= 7)
    {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_HIGH_CONTROLLER_COUNT,
            Severity::Medium,
            format!(
                "Kafka KRaft {} has a high controller count",
                item.kraft_id
            ),
            json!({
                "kraft_id": item.kraft_id,
                "controller_count": item.controller_count,
                "recommendation": "Review controller quorum sizing before carrying avoidable controller cost",
            }),
        ));
    }
}

fn evaluate_resilience(
    item: &KraftInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if !item.has_kraft_evidence {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_NO_EVIDENCE,
            Severity::High,
            format!(
                "Kafka KRaft inventory for cluster {} has no resilience evidence",
                item.cluster_name
            ),
            json!({
                "kraft_id": item.kraft_id,
                "cluster_name": item.cluster_name,
                "recommendation": "Collect KRaft quorum status, voter count, unhealthy voters, metadata log directory, and controller health before accepting resilience posture",
            }),
        ));
    }

    if item.has_kraft_evidence && !item.quorum_evidence {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_NO_QUORUM_EVIDENCE,
            Severity::High,
            format!("Kafka KRaft {} has no quorum evidence", item.kraft_id),
            json!({
                "kraft_id": item.kraft_id,
                "voter_count": item.voter_count,
                "metadata_quorum_id": item.metadata_quorum_id,
                "recommendation": "Record metadata quorum voter and leader evidence before accepting controller resilience posture",
            }),
        ));
    }

    if item
        .unhealthy_voter_count
        .is_some_and(|unhealthy_voter_count| unhealthy_voter_count > 0)
    {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_UNHEALTHY_VOTERS,
            Severity::High,
            format!("Kafka KRaft {} has unhealthy quorum voters", item.kraft_id),
            json!({
                "kraft_id": item.kraft_id,
                "unhealthy_voter_count": item.unhealthy_voter_count,
                "voter_count": item.voter_count,
                "recommendation": "Restore KRaft quorum voter health before accepting resilience posture",
            }),
        ));
    }
}

fn evaluate_security(
    item: &KraftInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if !item.has_kraft_evidence {
        findings.push(finding(
            item,
            pillar,
            REASON_SEC_NO_EVIDENCE,
            Severity::High,
            format!(
                "Kafka KRaft inventory for cluster {} has no security evidence",
                item.cluster_name
            ),
            json!({
                "kraft_id": item.kraft_id,
                "cluster_name": item.cluster_name,
                "recommendation": "Collect controller listener, identity, authorization, encryption, and transport evidence before accepting security posture",
            }),
        ));
    }

    if item.has_kraft_evidence
        && item
            .controller_principal
            .as_deref()
            .is_none_or(|principal| principal.trim().is_empty())
    {
        findings.push(finding(
            item,
            pillar,
            REASON_SEC_NO_CONTROLLER_PRINCIPAL,
            Severity::High,
            format!(
                "Kafka KRaft {} has no controller principal evidence",
                item.kraft_id
            ),
            json!({
                "kraft_id": item.kraft_id,
                "controller_principal": item.controller_principal,
                "recommendation": "Record authenticated controller principals before accepting KRaft authorization posture",
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
                "Kafka KRaft {} uses PLAINTEXT or unverified transport",
                item.kraft_id
            ),
            json!({
                "kraft_id": item.kraft_id,
                "plaintext_connection": item.plaintext_connection,
                "recommendation": "Use encrypted controller and broker transport before accepting security posture",
            }),
        ));
    }
}

fn stale_finding(
    item: &KraftInventoryItem,
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
            "Kafka KRaft inventory for cluster {} is {} hour(s) old",
            item.cluster_name, age_hours
        ),
        json!({
            "kraft_id": item.kraft_id,
            "cluster_name": item.cluster_name,
            "age_hours": age_hours,
            "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
            "recommendation": "Refresh Kafka KRaft inventory before acting on this posture report",
        }),
    ))
}

fn has_owner_metadata(item: &KraftInventoryItem) -> bool {
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
    item: &KraftInventoryItem,
    pillar: Pillar,
    reason_code: &'static str,
    severity: Severity,
    message: String,
    evidence: Value,
) -> InventoryFinding {
    InventoryFinding {
        resource_id: item.kraft_id.clone(),
        arn: format!("kafka:kraft/{}", item.kraft_id),
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

    fn kraft(now: DateTime<Utc>) -> KraftInventoryItem {
        KraftInventoryItem {
            kraft_id: "prod:kraft".to_string(),
            cluster_name: "prod".to_string(),
            controller_count: Some(3),
            voter_count: Some(3),
            unhealthy_voter_count: Some(0),
            metadata_log_dir: Some("/var/lib/kafka/meta".to_string()),
            metadata_quorum_id: Some("prod-quorum".to_string()),
            owner: Some("platform".to_string()),
            labels: BTreeMap::new(),
            quorum_evidence: true,
            controller_principal: Some("User:kafka-controller".to_string()),
            plaintext_connection: false,
            has_kraft_evidence: true,
            collected_at: now,
        }
    }

    #[test]
    fn healthy_kraft_passes_claimed_pillars() {
        let now = Utc::now();
        let item = kraft(now);

        for pillar in [Pillar::Cost, Pillar::Resilience, Pillar::Security] {
            let report = evaluate_kraft_inventory(std::slice::from_ref(&item), pillar, now);
            assert_eq!(report.resources_evaluated, 1);
            assert!(report.findings.is_empty());
            assert_eq!(report.stale_resources, 0);
            assert_eq!(report.score, 100);
        }
    }

    #[test]
    fn cost_flags_missing_owner_missing_evidence_and_high_controller_count() {
        let now = Utc::now();
        let mut missing_evidence = kraft(now);
        missing_evidence.owner = None;
        missing_evidence.has_kraft_evidence = false;
        let mut oversized = kraft(now);
        oversized.controller_count = Some(7);

        let report = evaluate_kraft_inventory(&[missing_evidence, oversized], Pillar::Cost, now);

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
            .any(|finding| finding.reason_code == REASON_COST_HIGH_CONTROLLER_COUNT));
    }

    #[test]
    fn resilience_flags_missing_evidence_quorum_and_unhealthy_voters() {
        let now = Utc::now();
        let mut missing_evidence = kraft(now);
        missing_evidence.has_kraft_evidence = false;
        let mut risky = kraft(now);
        risky.quorum_evidence = false;
        risky.unhealthy_voter_count = Some(1);

        let report = evaluate_kraft_inventory(&[missing_evidence, risky], Pillar::Resilience, now);

        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_RES_NO_EVIDENCE));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_RES_NO_QUORUM_EVIDENCE));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_RES_UNHEALTHY_VOTERS));
    }

    #[test]
    fn security_flags_missing_evidence_principal_and_plaintext() {
        let now = Utc::now();
        let mut missing_evidence = kraft(now);
        missing_evidence.has_kraft_evidence = false;
        let mut insecure = kraft(now);
        insecure.controller_principal = Some(" ".to_string());
        insecure.plaintext_connection = true;

        let report = evaluate_kraft_inventory(&[missing_evidence, insecure], Pillar::Security, now);

        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_SEC_NO_EVIDENCE));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_SEC_NO_CONTROLLER_PRINCIPAL));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_SEC_PLAINTEXT_CONNECTION));
    }

    #[test]
    fn stale_kraft_inventory_is_counted_for_any_pillar() {
        let now = Utc::now();
        let mut item = kraft(now);
        item.collected_at = now - Duration::hours(DEFAULT_STALE_AFTER_HOURS + 2);

        let report = evaluate_kraft_inventory(&[item], Pillar::Security, now);

        assert_eq!(report.stale_resources, 1);
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_INV_STALE_DATA));
    }
}

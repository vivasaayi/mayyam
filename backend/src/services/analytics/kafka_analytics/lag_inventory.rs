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

// Deterministic Kafka lag inventory evaluator for roadmap rows
// 04-KAFKA-DASHBOARD-MANAGEMENT-00442/00449/00470.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::config::KafkaClusterConfig;
use crate::services::aws::inventory::types::{
    score_pillar, InventoryFinding, Pillar, PillarReport, Severity, COST_ALLOCATION_TAG_KEYS,
    DEFAULT_STALE_AFTER_HOURS,
};

pub const RESOURCE_TYPE: &str = "KafkaLag";
pub const REASON_COST_OWNER_NOT_RECORDED: &str = "KAFKA_LAG_COST_OWNER_NOT_RECORDED";
pub const REASON_COST_NO_LAG_EVIDENCE: &str = "KAFKA_LAG_COST_NO_EVIDENCE";
pub const REASON_COST_HIGH_TOTAL_LAG: &str = "KAFKA_LAG_COST_HIGH_TOTAL_LAG";
pub const REASON_RES_NO_LAG_EVIDENCE: &str = "KAFKA_LAG_RES_NO_EVIDENCE";
pub const REASON_RES_LAG_OVER_OBJECTIVE: &str = "KAFKA_LAG_RES_OVER_OBJECTIVE";
pub const REASON_RES_NO_SAMPLE_WINDOW: &str = "KAFKA_LAG_RES_NO_SAMPLE_WINDOW";
pub const REASON_SEC_NO_LAG_EVIDENCE: &str = "KAFKA_LAG_SEC_NO_EVIDENCE";
pub const REASON_SEC_NO_PRINCIPAL_EVIDENCE: &str = "KAFKA_LAG_SEC_NO_PRINCIPAL_EVIDENCE";
pub const REASON_SEC_PLAINTEXT: &str = "KAFKA_LAG_SEC_PLAINTEXT_PROTOCOL";
pub const REASON_INV_STALE_DATA: &str = "KAFKA_LAG_INV_STALE_DATA";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KafkaLagInventoryItem {
    pub lag_id: String,
    pub cluster_name: String,
    pub group_id: String,
    pub topic_name: String,
    pub partition_count: Option<i32>,
    pub total_lag: Option<i64>,
    pub max_partition_lag: Option<i64>,
    pub lag_objective: Option<i64>,
    pub sample_window_seconds: Option<i64>,
    pub owner: Option<String>,
    pub labels: BTreeMap<String, String>,
    pub principal: Option<String>,
    pub has_lag_evidence: bool,
    pub security_protocol: String,
    pub collected_at: DateTime<Utc>,
}

pub fn evaluate_kafka_lag_inventory(
    items: &[KafkaLagInventoryItem],
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

pub fn lag_inventory_item_from_config(
    cluster: &KafkaClusterConfig,
    collected_at: DateTime<Utc>,
) -> KafkaLagInventoryItem {
    KafkaLagInventoryItem {
        lag_id: format!("{}:lag", cluster.name),
        cluster_name: cluster.name.clone(),
        group_id: "<uncollected>".to_string(),
        topic_name: "<uncollected>".to_string(),
        partition_count: None,
        total_lag: None,
        max_partition_lag: None,
        lag_objective: None,
        sample_window_seconds: None,
        owner: None,
        labels: BTreeMap::new(),
        principal: None,
        has_lag_evidence: false,
        security_protocol: cluster.security_protocol.clone(),
        collected_at,
    }
}

fn evaluate_cost(
    item: &KafkaLagInventoryItem,
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
                "Kafka lag inventory for cluster {} has no owner, team, project, or cost-center metadata",
                item.cluster_name
            ),
            json!({
                "lag_id": item.lag_id,
                "cluster_name": item.cluster_name,
                "checked_keys": COST_ALLOCATION_TAG_KEYS,
            }),
        ));
    }

    if !item.has_lag_evidence {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_NO_LAG_EVIDENCE,
            Severity::High,
            format!(
                "Kafka lag inventory for cluster {} has no lag evidence",
                item.cluster_name
            ),
            json!({
                "lag_id": item.lag_id,
                "cluster_name": item.cluster_name,
                "recommendation": "Collect consumer lag, partition lag, sample window, ownership, and labels before estimating lag-driven capacity cost",
            }),
        ));
    }

    if item.total_lag.is_some_and(|lag| lag >= 100_000) {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_HIGH_TOTAL_LAG,
            Severity::Medium,
            format!(
                "Kafka lag {} has total lag {}",
                item.lag_id,
                item.total_lag.unwrap_or_default()
            ),
            json!({
                "lag_id": item.lag_id,
                "total_lag": item.total_lag,
                "max_partition_lag": item.max_partition_lag,
                "recommendation": "Confirm lag trend and consumer throughput before increasing broker or consumer spend",
            }),
        ));
    }
}

fn evaluate_resilience(
    item: &KafkaLagInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if !item.has_lag_evidence {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_NO_LAG_EVIDENCE,
            Severity::High,
            format!(
                "Kafka lag inventory for cluster {} has no resilience evidence",
                item.cluster_name
            ),
            json!({
                "lag_id": item.lag_id,
                "cluster_name": item.cluster_name,
                "recommendation": "Collect lag and sampling evidence before evaluating consumer recovery readiness",
            }),
        ));
    }

    if item.has_lag_evidence && item.sample_window_seconds.unwrap_or_default() <= 0 {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_NO_SAMPLE_WINDOW,
            Severity::Medium,
            format!("Kafka lag {} has no sample window", item.lag_id),
            json!({
                "lag_id": item.lag_id,
                "sample_window_seconds": item.sample_window_seconds,
                "recommendation": "Record lag sampling windows so operators can distinguish transient spikes from sustained backlog",
            }),
        ));
    }

    if item
        .total_lag
        .zip(item.lag_objective)
        .is_some_and(|(lag, objective)| lag > objective)
    {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_LAG_OVER_OBJECTIVE,
            Severity::High,
            format!("Kafka lag {} exceeds the lag objective", item.lag_id),
            json!({
                "lag_id": item.lag_id,
                "total_lag": item.total_lag,
                "lag_objective": item.lag_objective,
                "recommendation": "Investigate consumer throughput, rebalance health, and broker saturation before accepting resilience posture",
            }),
        ));
    }
}

fn evaluate_security(
    item: &KafkaLagInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if !item.has_lag_evidence {
        findings.push(finding(
            item,
            pillar,
            REASON_SEC_NO_LAG_EVIDENCE,
            Severity::Medium,
            format!(
                "Kafka lag inventory for cluster {} has no lag evidence for security review",
                item.cluster_name
            ),
            json!({
                "lag_id": item.lag_id,
                "cluster_name": item.cluster_name,
                "recommendation": "Collect consumer group, topic, lag, and principal evidence before assessing lag visibility and access",
            }),
        ));
    }

    if item.has_lag_evidence && item.principal.as_deref().unwrap_or("").trim().is_empty() {
        findings.push(finding(
            item,
            pillar,
            REASON_SEC_NO_PRINCIPAL_EVIDENCE,
            Severity::Medium,
            format!("Kafka lag {} has no principal evidence", item.lag_id),
            json!({
                "lag_id": item.lag_id,
                "principal": item.principal,
                "recommendation": "Map lag observations to authenticated principals before exposing consumer backlog diagnostics",
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
                "Kafka lag inventory for cluster {} uses PLAINTEXT transport",
                item.cluster_name
            ),
            json!({
                "lag_id": item.lag_id,
                "security_protocol": item.security_protocol,
                "recommendation": "Use encrypted Kafka listener protocols before exposing lag and consumer group diagnostics",
            }),
        ));
    }
}

fn stale_finding(
    item: &KafkaLagInventoryItem,
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
            "Kafka lag inventory for cluster {} is {} hour(s) old",
            item.cluster_name, age_hours
        ),
        json!({
            "lag_id": item.lag_id,
            "cluster_name": item.cluster_name,
            "age_hours": age_hours,
            "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
            "recommendation": "Refresh Kafka lag inventory before acting on this posture report",
        }),
    ))
}

fn has_owner_metadata(item: &KafkaLagInventoryItem) -> bool {
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
    item: &KafkaLagInventoryItem,
    pillar: Pillar,
    reason_code: &'static str,
    severity: Severity,
    message: String,
    evidence: Value,
) -> InventoryFinding {
    InventoryFinding {
        resource_id: item.lag_id.clone(),
        arn: format!("kafka:lag/{}", item.lag_id),
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

    fn lag_item(now: DateTime<Utc>) -> KafkaLagInventoryItem {
        KafkaLagInventoryItem {
            lag_id: "prod:checkout:orders".to_string(),
            cluster_name: "prod".to_string(),
            group_id: "checkout".to_string(),
            topic_name: "orders".to_string(),
            partition_count: Some(12),
            total_lag: Some(10),
            max_partition_lag: Some(3),
            lag_objective: Some(100),
            sample_window_seconds: Some(60),
            owner: Some("payments".to_string()),
            labels: BTreeMap::new(),
            principal: Some("User:checkout".to_string()),
            has_lag_evidence: true,
            security_protocol: "SASL_SSL".to_string(),
            collected_at: now,
        }
    }

    #[test]
    fn healthy_lag_passes_claimed_pillars() {
        let now = Utc::now();
        let item = lag_item(now);

        for pillar in [Pillar::Cost, Pillar::Resilience, Pillar::Security] {
            let report = evaluate_kafka_lag_inventory(std::slice::from_ref(&item), pillar, now);
            assert_eq!(report.resources_evaluated, 1);
            assert!(report.findings.is_empty());
            assert_eq!(report.stale_resources, 0);
            assert_eq!(report.score, 100);
        }
    }

    #[test]
    fn cost_flags_missing_owner_missing_evidence_and_high_lag() {
        let now = Utc::now();
        let mut item = lag_item(now);
        item.owner = None;
        item.total_lag = Some(250_000);
        item.has_lag_evidence = false;

        let report = evaluate_kafka_lag_inventory(&[item], Pillar::Cost, now);

        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_COST_OWNER_NOT_RECORDED));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_COST_NO_LAG_EVIDENCE));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_COST_HIGH_TOTAL_LAG));
    }

    #[test]
    fn resilience_flags_missing_evidence_sample_window_and_objective_breach() {
        let now = Utc::now();
        let mut missing_evidence = lag_item(now);
        missing_evidence.has_lag_evidence = false;
        let mut objective_breach = lag_item(now);
        objective_breach.total_lag = Some(500);
        objective_breach.lag_objective = Some(100);
        objective_breach.sample_window_seconds = None;

        let report = evaluate_kafka_lag_inventory(
            &[missing_evidence, objective_breach],
            Pillar::Resilience,
            now,
        );

        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_RES_NO_LAG_EVIDENCE));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_RES_NO_SAMPLE_WINDOW));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_RES_LAG_OVER_OBJECTIVE));
    }

    #[test]
    fn security_flags_missing_evidence_missing_principal_and_plaintext() {
        let now = Utc::now();
        let mut missing_evidence = lag_item(now);
        missing_evidence.has_lag_evidence = false;
        let mut plaintext = lag_item(now);
        plaintext.principal = None;
        plaintext.security_protocol = "PLAINTEXT".to_string();

        let report =
            evaluate_kafka_lag_inventory(&[missing_evidence, plaintext], Pillar::Security, now);

        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_SEC_NO_LAG_EVIDENCE));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_SEC_NO_PRINCIPAL_EVIDENCE));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_SEC_PLAINTEXT));
    }

    #[test]
    fn stale_lag_inventory_is_counted_for_any_pillar() {
        let now = Utc::now();
        let mut item = lag_item(now);
        item.collected_at = now - Duration::hours(DEFAULT_STALE_AFTER_HOURS + 2);

        let report = evaluate_kafka_lag_inventory(&[item], Pillar::Security, now);

        assert_eq!(report.stale_resources, 1);
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_INV_STALE_DATA));
    }
}

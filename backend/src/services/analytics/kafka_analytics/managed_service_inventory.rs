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

// Deterministic managed Kafka services inventory evaluator for roadmap rows
// 04-KAFKA-DASHBOARD-MANAGEMENT-01716/01723/01744.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::config::KafkaClusterConfig;
use crate::services::aws::inventory::types::{
    score_pillar, InventoryFinding, Pillar, PillarReport, Severity, COST_ALLOCATION_TAG_KEYS,
    DEFAULT_STALE_AFTER_HOURS,
};

pub const RESOURCE_TYPE: &str = "KafkaManagedService";
pub const REASON_COST_OWNER_NOT_RECORDED: &str = "KAFKA_MANAGED_SERVICE_COST_OWNER_NOT_RECORDED";
pub const REASON_COST_NO_EVIDENCE: &str = "KAFKA_MANAGED_SERVICE_COST_NO_EVIDENCE";
pub const REASON_COST_NO_RIGHTSIZING_EVIDENCE: &str =
    "KAFKA_MANAGED_SERVICE_COST_NO_RIGHTSIZING_EVIDENCE";
pub const REASON_RES_NO_EVIDENCE: &str = "KAFKA_MANAGED_SERVICE_RES_NO_EVIDENCE";
pub const REASON_RES_NO_MULTI_AZ_EVIDENCE: &str = "KAFKA_MANAGED_SERVICE_RES_NO_MULTI_AZ_EVIDENCE";
pub const REASON_RES_LOW_BROKER_COUNT: &str = "KAFKA_MANAGED_SERVICE_RES_LOW_BROKER_COUNT";
pub const REASON_SEC_NO_EVIDENCE: &str = "KAFKA_MANAGED_SERVICE_SEC_NO_EVIDENCE";
pub const REASON_SEC_NO_ENCRYPTION_EVIDENCE: &str =
    "KAFKA_MANAGED_SERVICE_SEC_NO_ENCRYPTION_EVIDENCE";
pub const REASON_SEC_PUBLIC_ACCESS: &str = "KAFKA_MANAGED_SERVICE_SEC_PUBLIC_ACCESS";
pub const REASON_SEC_PLAINTEXT_CONNECTION: &str = "KAFKA_MANAGED_SERVICE_SEC_PLAINTEXT_CONNECTION";
pub const REASON_INV_STALE_DATA: &str = "KAFKA_MANAGED_SERVICE_INV_STALE_DATA";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManagedServiceInventoryItem {
    pub service_id: String,
    pub cluster_name: String,
    pub provider: Option<String>,
    pub service_plan: Option<String>,
    pub broker_count: Option<i32>,
    pub instance_type: Option<String>,
    pub kafka_version: Option<String>,
    pub monthly_cost_usd: Option<f64>,
    pub owner: Option<String>,
    pub labels: BTreeMap<String, String>,
    pub multi_az_evidence: bool,
    pub rightsizing_evidence: bool,
    pub encryption_evidence: bool,
    pub public_access: bool,
    pub plaintext_connection: bool,
    pub has_managed_service_evidence: bool,
    pub collected_at: DateTime<Utc>,
}

pub fn evaluate_managed_service_inventory(
    items: &[ManagedServiceInventoryItem],
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

pub fn managed_service_inventory_item_from_config(
    cluster: &KafkaClusterConfig,
    collected_at: DateTime<Utc>,
) -> ManagedServiceInventoryItem {
    ManagedServiceInventoryItem {
        service_id: format!("{}:managed-service", cluster.name),
        cluster_name: cluster.name.clone(),
        provider: None,
        service_plan: None,
        broker_count: None,
        instance_type: None,
        kafka_version: None,
        monthly_cost_usd: None,
        owner: None,
        labels: BTreeMap::new(),
        multi_az_evidence: false,
        rightsizing_evidence: false,
        encryption_evidence: false,
        public_access: false,
        plaintext_connection: cluster.security_protocol.eq_ignore_ascii_case("PLAINTEXT"),
        has_managed_service_evidence: false,
        collected_at,
    }
}

fn evaluate_cost(
    item: &ManagedServiceInventoryItem,
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
                "Managed Kafka service inventory for cluster {} has no owner, team, project, or cost-center metadata",
                item.cluster_name
            ),
            json!({
                "service_id": item.service_id,
                "cluster_name": item.cluster_name,
                "checked_keys": COST_ALLOCATION_TAG_KEYS,
            }),
        ));
    }

    if !item.has_managed_service_evidence {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_NO_EVIDENCE,
            Severity::High,
            format!(
                "Managed Kafka service inventory for cluster {} has no managed-service cost evidence",
                item.cluster_name
            ),
            json!({
                "service_id": item.service_id,
                "cluster_name": item.cluster_name,
                "recommendation": "Collect managed Kafka provider, plan, broker count, instance type, storage, transfer, ownership, and monthly cost evidence before accepting cost posture",
            }),
        ));
    }

    if item.has_managed_service_evidence && !item.rightsizing_evidence {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_NO_RIGHTSIZING_EVIDENCE,
            Severity::Medium,
            format!(
                "Managed Kafka service {} has no rightsizing evidence",
                item.service_id
            ),
            json!({
                "service_id": item.service_id,
                "monthly_cost_usd": item.monthly_cost_usd,
                "instance_type": item.instance_type,
                "broker_count": item.broker_count,
                "recommendation": "Record utilization, tier, broker, storage, and retention evidence before accepting managed-service spend",
            }),
        ));
    }
}

fn evaluate_resilience(
    item: &ManagedServiceInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if !item.has_managed_service_evidence {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_NO_EVIDENCE,
            Severity::High,
            format!(
                "Managed Kafka service inventory for cluster {} has no resilience evidence",
                item.cluster_name
            ),
            json!({
                "service_id": item.service_id,
                "cluster_name": item.cluster_name,
                "recommendation": "Collect managed-service deployment mode, availability zones, broker count, version, maintenance, backup, and failover evidence before accepting resilience posture",
            }),
        ));
    }

    if item.has_managed_service_evidence && !item.multi_az_evidence {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_NO_MULTI_AZ_EVIDENCE,
            Severity::High,
            format!("Managed Kafka service {} has no multi-AZ evidence", item.service_id),
            json!({
                "service_id": item.service_id,
                "provider": item.provider,
                "service_plan": item.service_plan,
                "recommendation": "Record managed-service zone placement, replication, and failover evidence before accepting resilience posture",
            }),
        ));
    }

    if item
        .broker_count
        .is_some_and(|broker_count| broker_count < 3)
    {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_LOW_BROKER_COUNT,
            Severity::High,
            format!(
                "Managed Kafka service {} has fewer than three brokers",
                item.service_id
            ),
            json!({
                "service_id": item.service_id,
                "broker_count": item.broker_count,
                "recommendation": "Verify quorum, partition placement, and failover objectives before accepting a managed Kafka service with fewer than three brokers",
            }),
        ));
    }
}

fn evaluate_security(
    item: &ManagedServiceInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if !item.has_managed_service_evidence {
        findings.push(finding(
            item,
            pillar,
            REASON_SEC_NO_EVIDENCE,
            Severity::High,
            format!(
                "Managed Kafka service inventory for cluster {} has no security evidence",
                item.cluster_name
            ),
            json!({
                "service_id": item.service_id,
                "cluster_name": item.cluster_name,
                "recommendation": "Collect managed-service encryption, network exposure, authentication, authorization, audit, and provider control-plane evidence before accepting security posture",
            }),
        ));
    }

    if item.has_managed_service_evidence && !item.encryption_evidence {
        findings.push(finding(
            item,
            pillar,
            REASON_SEC_NO_ENCRYPTION_EVIDENCE,
            Severity::High,
            format!(
                "Managed Kafka service {} has no encryption evidence",
                item.service_id
            ),
            json!({
                "service_id": item.service_id,
                "provider": item.provider,
                "recommendation": "Record encryption in transit, encryption at rest, and key-management evidence for the managed Kafka service",
            }),
        ));
    }

    if item.public_access {
        findings.push(finding(
            item,
            pillar,
            REASON_SEC_PUBLIC_ACCESS,
            Severity::High,
            format!("Managed Kafka service {} allows public access", item.service_id),
            json!({
                "service_id": item.service_id,
                "public_access": item.public_access,
                "recommendation": "Restrict managed Kafka endpoints to private networking or explicitly approved ingress paths",
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
                "Managed Kafka service {} belongs to a PLAINTEXT or unverified Kafka transport cluster",
                item.service_id
            ),
            json!({
                "service_id": item.service_id,
                "plaintext_connection": item.plaintext_connection,
                "recommendation": "Use encrypted Kafka transport before accepting managed-service security posture",
            }),
        ));
    }
}

fn stale_finding(
    item: &ManagedServiceInventoryItem,
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
            "Managed Kafka service inventory for cluster {} is {} hour(s) old",
            item.cluster_name, age_hours
        ),
        json!({
            "service_id": item.service_id,
            "cluster_name": item.cluster_name,
            "age_hours": age_hours,
            "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
            "recommendation": "Refresh managed Kafka service inventory before acting on this posture report",
        }),
    ))
}

fn has_owner_metadata(item: &ManagedServiceInventoryItem) -> bool {
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
    item: &ManagedServiceInventoryItem,
    pillar: Pillar,
    reason_code: &'static str,
    severity: Severity,
    message: String,
    evidence: Value,
) -> InventoryFinding {
    InventoryFinding {
        resource_id: item.service_id.clone(),
        arn: format!("kafka:managed-service/{}", item.service_id),
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

    fn managed_service(now: DateTime<Utc>) -> ManagedServiceInventoryItem {
        ManagedServiceInventoryItem {
            service_id: "prod:managed-service".to_string(),
            cluster_name: "prod".to_string(),
            provider: Some("aws-msk".to_string()),
            service_plan: Some("provisioned".to_string()),
            broker_count: Some(3),
            instance_type: Some("kafka.m7g.large".to_string()),
            kafka_version: Some("3.7.x".to_string()),
            monthly_cost_usd: Some(1200.0),
            owner: Some("platform".to_string()),
            labels: BTreeMap::new(),
            multi_az_evidence: true,
            rightsizing_evidence: true,
            encryption_evidence: true,
            public_access: false,
            plaintext_connection: false,
            has_managed_service_evidence: true,
            collected_at: now,
        }
    }

    #[test]
    fn healthy_managed_service_passes_claimed_pillars() {
        let now = Utc::now();
        let item = managed_service(now);

        for pillar in [Pillar::Cost, Pillar::Resilience, Pillar::Security] {
            let report =
                evaluate_managed_service_inventory(std::slice::from_ref(&item), pillar, now);
            assert_eq!(report.resources_evaluated, 1);
            assert!(report.findings.is_empty());
            assert_eq!(report.stale_resources, 0);
            assert_eq!(report.score, 100);
        }
    }

    #[test]
    fn cost_flags_missing_owner_evidence_and_rightsizing() {
        let now = Utc::now();
        let mut missing_evidence = managed_service(now);
        missing_evidence.owner = None;
        missing_evidence.has_managed_service_evidence = false;
        let mut no_rightsizing = managed_service(now);
        no_rightsizing.rightsizing_evidence = false;

        let report = evaluate_managed_service_inventory(
            &[missing_evidence, no_rightsizing],
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
            .any(|finding| finding.reason_code == REASON_COST_NO_EVIDENCE));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_COST_NO_RIGHTSIZING_EVIDENCE));
    }

    #[test]
    fn resilience_flags_missing_evidence_multi_az_and_low_broker_count() {
        let now = Utc::now();
        let mut missing_evidence = managed_service(now);
        missing_evidence.has_managed_service_evidence = false;
        let mut risky = managed_service(now);
        risky.multi_az_evidence = false;
        risky.broker_count = Some(2);

        let report =
            evaluate_managed_service_inventory(&[missing_evidence, risky], Pillar::Resilience, now);

        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_RES_NO_EVIDENCE));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_RES_NO_MULTI_AZ_EVIDENCE));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_RES_LOW_BROKER_COUNT));
    }

    #[test]
    fn security_flags_missing_evidence_encryption_public_access_and_plaintext() {
        let now = Utc::now();
        let mut missing_evidence = managed_service(now);
        missing_evidence.has_managed_service_evidence = false;
        let mut risky = managed_service(now);
        risky.encryption_evidence = false;
        risky.public_access = true;
        risky.plaintext_connection = true;

        let report =
            evaluate_managed_service_inventory(&[missing_evidence, risky], Pillar::Security, now);

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
            .any(|finding| finding.reason_code == REASON_SEC_PUBLIC_ACCESS));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_SEC_PLAINTEXT_CONNECTION));
    }

    #[test]
    fn config_item_produces_missing_evidence_and_plaintext_security_findings() {
        let now = Utc::now();
        let cluster = KafkaClusterConfig {
            name: "orders".to_string(),
            bootstrap_servers: vec!["broker-1:9092".to_string()],
            sasl_username: None,
            sasl_password: None,
            sasl_mechanism: None,
            security_protocol: "PLAINTEXT".to_string(),
        };

        let item = managed_service_inventory_item_from_config(&cluster, now);
        let report = evaluate_managed_service_inventory(&[item], Pillar::Security, now);

        assert_eq!(report.resources_evaluated, 1);
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_SEC_NO_EVIDENCE));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_SEC_PLAINTEXT_CONNECTION));
    }

    #[test]
    fn stale_inventory_is_reported_per_pillar() {
        let now = Utc::now();
        let mut item = managed_service(now);
        item.collected_at = now - Duration::hours(DEFAULT_STALE_AFTER_HOURS + 1);

        let report = evaluate_managed_service_inventory(&[item], Pillar::Cost, now);

        assert_eq!(report.stale_resources, 1);
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_INV_STALE_DATA));
    }
}

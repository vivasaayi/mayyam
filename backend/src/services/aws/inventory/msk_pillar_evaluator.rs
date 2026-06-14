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

// Deterministic MSK cluster inventory evaluators for the cost, resilience,
// and security pillars (roadmap rows 01-AWS-CLOUD-02080/02089/02116).
//
// Evaluates fields persisted by msk_control_plane: state, cluster_type,
// serverless, kafka_version, instance_type, number_of_broker_nodes,
// storage_per_broker_gb, encryption_in_transit_*, encryption_at_rest (kms_key_id),
// sasl_scram_enabled, sasl_iam_enabled, tls_enabled, unauthenticated_enabled,
// enhanced_monitoring, cloudwatch_logs_enabled, s3_logs_enabled, plus tags.

use chrono::{DateTime, Utc};
use serde_json::{json, Value};

use crate::models::aws_resource::Model as AwsResourceModel;
use crate::services::aws::inventory::types::{
    check_stale, score_pillar, InventoryFinding, Pillar, PillarReport, Severity,
};

pub const RESOURCE_TYPE: &str = "MskCluster";

pub const REASON_COST_NO_TAGS: &str = "MSK_COST_NO_TAGS";
pub const REASON_COST_OVER_PROVISIONED_MONITORING: &str = "MSK_COST_OVER_PROVISIONED_MONITORING";
pub const REASON_RES_NO_LOGS: &str = "MSK_RES_NO_LOGS";
pub const REASON_RES_SINGLE_BROKER: &str = "MSK_RES_SINGLE_BROKER";
pub const REASON_RES_CLUSTER_NOT_ACTIVE: &str = "MSK_RES_CLUSTER_NOT_ACTIVE";
pub const REASON_SEC_UNAUTHENTICATED_ENABLED: &str = "MSK_SEC_UNAUTHENTICATED_ENABLED";
pub const REASON_SEC_PLAINTEXT_IN_TRANSIT: &str = "MSK_SEC_PLAINTEXT_IN_TRANSIT";
pub const REASON_SEC_NO_KMS_ENCRYPTION: &str = "MSK_SEC_NO_KMS_ENCRYPTION";
pub const REASON_INV_STALE_DATA: &str = "MSK_INV_STALE_DATA";

pub fn evaluate_msk_fleet(
    resources: &[AwsResourceModel],
    pillar: Pillar,
    now: DateTime<Utc>,
) -> PillarReport {
    let mut findings: Vec<InventoryFinding> = Vec::new();
    let mut stale_resources = 0usize;
    let mut evaluated = 0usize;

    for resource in resources {
        if resource.resource_type != RESOURCE_TYPE {
            continue;
        }
        evaluated += 1;

        if let Some(stale) = check_stale(resource, pillar, REASON_INV_STALE_DATA, now) {
            stale_resources += 1;
            findings.push(stale);
        }
        match pillar {
            Pillar::Cost => evaluate_cost(resource, &mut findings),
            Pillar::Resilience => evaluate_resilience(resource, &mut findings),
            Pillar::Security => evaluate_security(resource, &mut findings),
            _ => {}
        }
    }

    let score = score_pillar(&findings);
    PillarReport {
        pillar,
        resources_evaluated: evaluated,
        stale_resources,
        score,
        findings,
    }
}

fn data_bool(resource_data: &Value, key: &str) -> Option<bool> {
    resource_data.get(key).and_then(|v| v.as_bool())
}

fn data_i64(resource_data: &Value, key: &str) -> Option<i64> {
    resource_data.get(key).and_then(|v| v.as_i64())
}

fn data_str<'a>(resource_data: &'a Value, key: &str) -> Option<&'a str> {
    resource_data.get(key).and_then(|v| v.as_str())
}

fn evaluate_cost(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    let tags_empty = resource
        .tags
        .as_object()
        .map(|m| m.is_empty())
        .unwrap_or(true);
    if tags_empty {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Cost,
            reason_code: REASON_COST_NO_TAGS.to_string(),
            severity: Severity::Medium,
            message: format!(
                "MSK cluster {} has no tags; cost allocation cannot be assessed",
                resource.resource_id
            ),
            evidence: json!({ "tags": resource.tags }),
        });
    }

    // PER_TOPIC_PER_PARTITION monitoring is the most expensive tier and
    // rarely needed unless you have per-topic alerting requirements.
    if let Some(monitoring) = data_str(&resource.resource_data, "enhanced_monitoring") {
        if monitoring == "PER_TOPIC_PER_PARTITION" {
            findings.push(InventoryFinding {
                resource_id: resource.resource_id.clone(),
                arn: resource.arn.clone(),
                pillar: Pillar::Cost,
                reason_code: REASON_COST_OVER_PROVISIONED_MONITORING.to_string(),
                severity: Severity::Low,
                message: format!(
                    "MSK cluster {} uses PER_TOPIC_PER_PARTITION enhanced monitoring, the most expensive tier; downgrade to DEFAULT or PER_BROKER if per-topic metrics are not required",
                    resource.resource_id
                ),
                evidence: json!({ "enhanced_monitoring": monitoring }),
            });
        }
    }
}

fn evaluate_resilience(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    if let Some(state) = data_str(&resource.resource_data, "state") {
        if state != "ACTIVE" {
            findings.push(InventoryFinding {
                resource_id: resource.resource_id.clone(),
                arn: resource.arn.clone(),
                pillar: Pillar::Resilience,
                reason_code: REASON_RES_CLUSTER_NOT_ACTIVE.to_string(),
                severity: Severity::High,
                message: format!(
                    "MSK cluster {} is in state '{}' rather than ACTIVE; investigate and restore it to avoid data-stream disruption",
                    resource.resource_id, state
                ),
                evidence: json!({ "state": state }),
            });
        }
    }

    // Serverless clusters are multi-AZ by design; only check provisioned.
    let is_serverless = data_bool(&resource.resource_data, "serverless").unwrap_or(false);
    if !is_serverless {
        if let Some(brokers) = data_i64(&resource.resource_data, "number_of_broker_nodes") {
            if brokers < 3 {
                findings.push(InventoryFinding {
                    resource_id: resource.resource_id.clone(),
                    arn: resource.arn.clone(),
                    pillar: Pillar::Resilience,
                    reason_code: REASON_RES_SINGLE_BROKER.to_string(),
                    severity: Severity::Medium,
                    message: format!(
                        "MSK cluster {} has only {} broker(s); use at least 3 brokers across 3 AZs for zone-level fault tolerance",
                        resource.resource_id, brokers
                    ),
                    evidence: json!({ "number_of_broker_nodes": brokers }),
                });
            }
        }
    }

    let cw_logs = data_bool(&resource.resource_data, "cloudwatch_logs_enabled").unwrap_or(false);
    let s3_logs = data_bool(&resource.resource_data, "s3_logs_enabled").unwrap_or(false);
    if !is_serverless && !cw_logs && !s3_logs {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Resilience,
            reason_code: REASON_RES_NO_LOGS.to_string(),
            severity: Severity::Medium,
            message: format!(
                "MSK cluster {} has no broker log destination (CloudWatch Logs or S3) configured; broker logs are essential for diagnosing replication and consumer lag issues",
                resource.resource_id
            ),
            evidence: json!({
                "cloudwatch_logs_enabled": cw_logs,
                "s3_logs_enabled": s3_logs,
            }),
        });
    }
}

fn evaluate_security(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    // Unauthenticated access allows any client to produce or consume.
    if data_bool(&resource.resource_data, "unauthenticated_enabled").unwrap_or(false) {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Security,
            reason_code: REASON_SEC_UNAUTHENTICATED_ENABLED.to_string(),
            severity: Severity::High,
            message: format!(
                "MSK cluster {} allows unauthenticated client access; disable unauthenticated access and enforce SASL/IAM or TLS mutual auth",
                resource.resource_id
            ),
            evidence: json!({ "unauthenticated_enabled": true }),
        });
    }

    // PLAINTEXT means traffic between clients and brokers is unencrypted.
    if let Some(client_broker) = data_str(
        &resource.resource_data,
        "encryption_in_transit_client_broker",
    ) {
        if client_broker == "PLAINTEXT" {
            findings.push(InventoryFinding {
                resource_id: resource.resource_id.clone(),
                arn: resource.arn.clone(),
                pillar: Pillar::Security,
                reason_code: REASON_SEC_PLAINTEXT_IN_TRANSIT.to_string(),
                severity: Severity::High,
                message: format!(
                    "MSK cluster {} uses PLAINTEXT for client-to-broker encryption in transit; set to TLS to prevent eavesdropping",
                    resource.resource_id
                ),
                evidence: json!({ "encryption_in_transit_client_broker": client_broker }),
            });
        }
    }

    // No KMS key means AWS-managed key (still encrypted, but less control).
    let is_serverless = data_bool(&resource.resource_data, "serverless").unwrap_or(false);
    if !is_serverless && resource.resource_data.get("kms_key_id").is_none() {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Security,
            reason_code: REASON_SEC_NO_KMS_ENCRYPTION.to_string(),
            severity: Severity::Low,
            message: format!(
                "MSK cluster {} does not use a customer-managed KMS key for encryption at rest; use a CMK for key rotation control and audit visibility",
                resource.resource_id
            ),
            evidence: json!({ "kms_key_id": null }),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;
    use uuid::Uuid;

    fn fixture(
        resource_id: &str,
        tags: Value,
        resource_data: Value,
        now: DateTime<Utc>,
    ) -> AwsResourceModel {
        let refreshed = now - Duration::hours(1);
        AwsResourceModel {
            id: Uuid::new_v4(),
            sync_id: None,
            account_id: "123456789012".to_string(),
            profile: None,
            region: "us-east-1".to_string(),
            resource_type: RESOURCE_TYPE.to_string(),
            resource_id: resource_id.to_string(),
            arn: format!(
                "arn:aws:kafka:us-east-1:123456789012:cluster/{}/abc",
                resource_id
            ),
            name: Some(resource_id.to_string()),
            tags,
            resource_data,
            created_at: refreshed,
            updated_at: refreshed,
            last_refreshed: refreshed,
        }
    }

    fn now() -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2026-06-11T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc)
    }

    fn healthy_data() -> Value {
        json!({
            "state": "ACTIVE",
            "cluster_type": "PROVISIONED",
            "serverless": false,
            "kafka_version": "3.5.1",
            "instance_type": "kafka.m5.large",
            "number_of_broker_nodes": 3,
            "storage_per_broker_gb": 1000,
            "encryption_in_transit_client_broker": "TLS",
            "encryption_in_transit_in_cluster": true,
            "kms_key_id": "arn:aws:kms:us-east-1:123456789012:key/abc",
            "sasl_iam_enabled": true,
            "sasl_scram_enabled": false,
            "tls_enabled": false,
            "unauthenticated_enabled": false,
            "enhanced_monitoring": "DEFAULT",
            "cloudwatch_logs_enabled": true,
            "s3_logs_enabled": false,
        })
    }

    fn codes(report: &PillarReport) -> Vec<&str> {
        report
            .findings
            .iter()
            .map(|f| f.reason_code.as_str())
            .collect()
    }

    #[test]
    fn healthy_cluster_passes_all_pillars() {
        let r = fixture("my-msk", json!({"team": "data"}), healthy_data(), now());
        for pillar in [Pillar::Cost, Pillar::Resilience, Pillar::Security] {
            let report = evaluate_msk_fleet(std::slice::from_ref(&r), pillar, now());
            assert!(
                report.findings.is_empty(),
                "unexpected for {:?}: {:?}",
                pillar,
                report.findings
            );
            assert_eq!(report.score, 100);
        }
    }

    #[test]
    fn cost_flags_untagged_cluster() {
        let r = fixture("untagged", json!({}), healthy_data(), now());
        let report = evaluate_msk_fleet(&[r], Pillar::Cost, now());
        assert_eq!(codes(&report), vec![REASON_COST_NO_TAGS]);
    }

    #[test]
    fn cost_flags_over_provisioned_monitoring() {
        let mut data = healthy_data();
        data["enhanced_monitoring"] = json!("PER_TOPIC_PER_PARTITION");
        let r = fixture("verbose-mon", json!({"team": "data"}), data, now());
        let report = evaluate_msk_fleet(&[r], Pillar::Cost, now());
        assert!(codes(&report).contains(&REASON_COST_OVER_PROVISIONED_MONITORING));
    }

    #[test]
    fn resilience_flags_inactive_cluster() {
        let mut data = healthy_data();
        data["state"] = json!("FAILED");
        let r = fixture("failed-msk", json!({"team": "data"}), data, now());
        let report = evaluate_msk_fleet(&[r], Pillar::Resilience, now());
        assert!(codes(&report).contains(&REASON_RES_CLUSTER_NOT_ACTIVE));
    }

    #[test]
    fn resilience_flags_single_broker() {
        let mut data = healthy_data();
        data["number_of_broker_nodes"] = json!(1);
        let r = fixture("single-broker", json!({"team": "data"}), data, now());
        let report = evaluate_msk_fleet(&[r], Pillar::Resilience, now());
        assert!(codes(&report).contains(&REASON_RES_SINGLE_BROKER));
    }

    #[test]
    fn resilience_flags_no_logs() {
        let mut data = healthy_data();
        data["cloudwatch_logs_enabled"] = json!(false);
        data["s3_logs_enabled"] = json!(false);
        let r = fixture("no-logs", json!({"team": "data"}), data, now());
        let report = evaluate_msk_fleet(&[r], Pillar::Resilience, now());
        assert!(codes(&report).contains(&REASON_RES_NO_LOGS));
    }

    #[test]
    fn security_flags_unauthenticated_access() {
        let mut data = healthy_data();
        data["unauthenticated_enabled"] = json!(true);
        let r = fixture("open-msk", json!({"team": "data"}), data, now());
        let report = evaluate_msk_fleet(&[r], Pillar::Security, now());
        assert!(codes(&report).contains(&REASON_SEC_UNAUTHENTICATED_ENABLED));
    }

    #[test]
    fn security_flags_plaintext_in_transit() {
        let mut data = healthy_data();
        data["encryption_in_transit_client_broker"] = json!("PLAINTEXT");
        let r = fixture("plaintext-msk", json!({"team": "data"}), data, now());
        let report = evaluate_msk_fleet(&[r], Pillar::Security, now());
        assert!(codes(&report).contains(&REASON_SEC_PLAINTEXT_IN_TRANSIT));
    }

    #[test]
    fn security_flags_no_kms_key() {
        let mut data = healthy_data();
        data.as_object_mut().unwrap().remove("kms_key_id");
        let r = fixture("no-kms", json!({"team": "data"}), data, now());
        let report = evaluate_msk_fleet(&[r], Pillar::Security, now());
        assert!(codes(&report).contains(&REASON_SEC_NO_KMS_ENCRYPTION));
    }

    #[test]
    fn serverless_cluster_skips_broker_and_kms_checks() {
        let mut data = healthy_data();
        data["serverless"] = json!(true);
        data["cluster_type"] = json!("SERVERLESS");
        data.as_object_mut()
            .unwrap()
            .remove("number_of_broker_nodes");
        data.as_object_mut().unwrap().remove("kms_key_id");
        let r = fixture("serverless-msk", json!({"team": "data"}), data, now());
        let res_report = evaluate_msk_fleet(&[r.clone()], Pillar::Resilience, now());
        assert!(!codes(&res_report).contains(&REASON_RES_SINGLE_BROKER));
        let sec_report = evaluate_msk_fleet(&[r], Pillar::Security, now());
        assert!(!codes(&sec_report).contains(&REASON_SEC_NO_KMS_ENCRYPTION));
    }

    #[test]
    fn stale_resource_is_flagged() {
        let mut r = fixture("stale-msk", json!({"team": "data"}), healthy_data(), now());
        r.last_refreshed = now() - Duration::hours(48);
        let report = evaluate_msk_fleet(&[r], Pillar::Resilience, now());
        assert_eq!(report.stale_resources, 1);
        assert!(codes(&report).contains(&REASON_INV_STALE_DATA));
    }

    #[test]
    fn non_msk_resources_are_skipped() {
        let mut r = fixture("queue-1", json!({}), json!({}), now());
        r.resource_type = "SqsQueue".to_string();
        let report = evaluate_msk_fleet(&[r], Pillar::Cost, now());
        assert_eq!(report.resources_evaluated, 0);
        assert!(report.findings.is_empty());
    }
}

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

// Deterministic App Runner inventory evaluators for the cost, security, and
// resilience pillars.
//
// Evaluates fields persisted by apprunner_control_plane: status, instance_cpu,
// instance_memory, instance_role_arn, instance_configuration_collected,
// customer_managed_kms, kms_key, health_check_* fields,
// auto_scaling_configuration_name, is_publicly_accessible, egress_type, and
// observability_enabled. App Runner only returns an encryption configuration
// for customer-managed keys, so the collector persists the explicit
// `customer_managed_kms` boolean to keep "default key" distinct from a
// collection gap.

use chrono::{DateTime, Utc};
use serde_json::json;

use crate::models::aws_resource::Model as AwsResourceModel;
use crate::services::aws::inventory::types::{
    check_stale, data_str, score_pillar, InventoryFinding, Pillar, PillarReport, Severity,
};

/// Only rows of this resource type are evaluated.
pub const RESOURCE_TYPE: &str = "AppRunnerService";

/// The largest App Runner instance size; either spelling is returned by the API.
const MAX_CPU_VALUES: &[&str] = &["4096", "4 vCPU"];
/// Name of the account-default auto scaling configuration.
const DEFAULT_AUTOSCALING_NAME: &str = "DefaultConfiguration";

// Reason codes are the stable contract for findings; never reuse or rename.
pub const REASON_COST_PAUSED_SERVICE: &str = "APPRUNNER_COST_PAUSED_SERVICE";
pub const REASON_COST_MAX_SIZE_INSTANCE: &str = "APPRUNNER_COST_MAX_SIZE_INSTANCE";
pub const REASON_COST_INSTANCE_DATA_NOT_COLLECTED: &str =
    "APPRUNNER_COST_INSTANCE_DATA_NOT_COLLECTED";
pub const REASON_COST_DEFAULT_AUTOSCALING: &str = "APPRUNNER_COST_DEFAULT_AUTOSCALING";
pub const REASON_COST_AUTOSCALING_DATA_NOT_COLLECTED: &str =
    "APPRUNNER_COST_AUTOSCALING_DATA_NOT_COLLECTED";
pub const REASON_RES_SERVICE_FAILED_STATE: &str = "APPRUNNER_RES_SERVICE_FAILED_STATE";
pub const REASON_RES_STATUS_DATA_NOT_COLLECTED: &str = "APPRUNNER_RES_STATUS_DATA_NOT_COLLECTED";
pub const REASON_RES_WEAK_UNHEALTHY_THRESHOLD: &str = "APPRUNNER_RES_WEAK_UNHEALTHY_THRESHOLD";
pub const REASON_RES_HEALTH_CHECK_DATA_NOT_COLLECTED: &str =
    "APPRUNNER_RES_HEALTH_CHECK_DATA_NOT_COLLECTED";
pub const REASON_RES_OBSERVABILITY_DISABLED: &str = "APPRUNNER_RES_OBSERVABILITY_DISABLED";
pub const REASON_RES_OBSERVABILITY_DATA_NOT_COLLECTED: &str =
    "APPRUNNER_RES_OBSERVABILITY_DATA_NOT_COLLECTED";
pub const REASON_SEC_PUBLIC_INGRESS: &str = "APPRUNNER_SEC_PUBLIC_INGRESS";
pub const REASON_SEC_INGRESS_DATA_NOT_COLLECTED: &str = "APPRUNNER_SEC_INGRESS_DATA_NOT_COLLECTED";
pub const REASON_SEC_DEFAULT_ENCRYPTION_KEY: &str = "APPRUNNER_SEC_DEFAULT_ENCRYPTION_KEY";
pub const REASON_SEC_ENCRYPTION_DATA_NOT_COLLECTED: &str =
    "APPRUNNER_SEC_ENCRYPTION_DATA_NOT_COLLECTED";
pub const REASON_SEC_NO_INSTANCE_ROLE: &str = "APPRUNNER_SEC_NO_INSTANCE_ROLE";
pub const REASON_INV_STALE_DATA: &str = "APPRUNNER_INV_STALE_DATA";

/// Evaluate every App Runner service in the fleet for one pillar. Rows whose
/// `resource_type` is not `AppRunnerService` are skipped and not counted.
pub fn evaluate_apprunner_fleet(
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
            Pillar::Security => evaluate_security(resource, &mut findings),
            Pillar::Resilience => evaluate_resilience(resource, &mut findings),
            // Pillars without checks for this service yet produce no findings.
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

fn status(resource: &AwsResourceModel) -> Option<String> {
    data_str(&resource.resource_data, "status")
}

fn data_bool(resource: &AwsResourceModel, key: &str) -> Option<bool> {
    resource.resource_data.get(key).and_then(|v| v.as_bool())
}

fn data_i64(resource: &AwsResourceModel, key: &str) -> Option<i64> {
    resource.resource_data.get(key).and_then(|v| v.as_i64())
}

fn finding(
    resource: &AwsResourceModel,
    pillar: Pillar,
    reason_code: &str,
    severity: Severity,
    message: String,
    evidence: serde_json::Value,
) -> InventoryFinding {
    InventoryFinding {
        resource_id: resource.resource_id.clone(),
        arn: resource.arn.clone(),
        pillar,
        reason_code: reason_code.to_string(),
        severity,
        message,
        evidence,
    }
}

fn evaluate_cost(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    // A paused service no longer bills for active compute but keeps the
    // provisioned-memory charge and counts against the service quota.
    let state = status(resource);
    if state.as_deref() == Some("PAUSED") {
        findings.push(finding(
            resource,
            Pillar::Cost,
            REASON_COST_PAUSED_SERVICE,
            Severity::Low,
            format!(
                "App Runner service {} is paused but still bills for provisioned container memory; delete it if it is no longer needed",
                resource.resource_id
            ),
            json!({ "status": state }),
        ));
    }

    match data_str(&resource.resource_data, "instance_cpu") {
        Some(cpu) => {
            if MAX_CPU_VALUES.contains(&cpu.as_str()) {
                let memory = data_str(&resource.resource_data, "instance_memory");
                findings.push(finding(
                    resource,
                    Pillar::Cost,
                    REASON_COST_MAX_SIZE_INSTANCE,
                    Severity::Low,
                    format!(
                        "App Runner service {} uses the largest instance size (4 vCPU); confirm the workload needs it or right-size to cut per-instance cost",
                        resource.resource_id
                    ),
                    json!({ "instance_cpu": cpu, "instance_memory": memory }),
                ));
            }
        }
        None => {
            findings.push(finding(
                resource,
                Pillar::Cost,
                REASON_COST_INSTANCE_DATA_NOT_COLLECTED,
                Severity::Low,
                format!(
                    "Instance CPU/memory size for App Runner service {} is not collected yet; cost pillar cannot be fully assessed",
                    resource.resource_id
                ),
                json!({ "instance_cpu_collected": false }),
            ));
        }
    }

    match data_str(&resource.resource_data, "auto_scaling_configuration_name") {
        Some(name) => {
            if name == DEFAULT_AUTOSCALING_NAME {
                findings.push(finding(
                    resource,
                    Pillar::Cost,
                    REASON_COST_DEFAULT_AUTOSCALING,
                    Severity::Low,
                    format!(
                        "App Runner service {} uses the account-default auto scaling configuration; tune min/max instance limits to match the workload's spend envelope",
                        resource.resource_id
                    ),
                    json!({ "auto_scaling_configuration_name": name }),
                ));
            }
        }
        None => {
            findings.push(finding(
                resource,
                Pillar::Cost,
                REASON_COST_AUTOSCALING_DATA_NOT_COLLECTED,
                Severity::Low,
                format!(
                    "Auto scaling configuration for App Runner service {} is not collected yet; scaling-limit cost posture cannot be assessed",
                    resource.resource_id
                ),
                json!({ "auto_scaling_configuration_collected": false }),
            ));
        }
    }
}

fn evaluate_resilience(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    match status(resource).as_deref() {
        Some("CREATE_FAILED") | Some("DELETE_FAILED") => {
            let state = status(resource);
            findings.push(finding(
                resource,
                Pillar::Resilience,
                REASON_RES_SERVICE_FAILED_STATE,
                Severity::High,
                format!(
                    "App Runner service {} is in state {}; it is not serving traffic and needs operator intervention",
                    resource.resource_id,
                    state.as_deref().unwrap_or("UNKNOWN")
                ),
                json!({ "status": state }),
            ));
        }
        Some(_) => {}
        None => {
            findings.push(finding(
                resource,
                Pillar::Resilience,
                REASON_RES_STATUS_DATA_NOT_COLLECTED,
                Severity::Low,
                format!(
                    "Status for App Runner service {} is not collected yet; resilience pillar cannot be fully assessed",
                    resource.resource_id
                ),
                json!({ "status_collected": false }),
            ));
        }
    }

    match data_str(&resource.resource_data, "health_check_protocol") {
        Some(protocol) => {
            // An unhealthy threshold of 1 marks an instance unhealthy on a
            // single failed probe, causing replacement flapping under load.
            if data_i64(resource, "health_check_unhealthy_threshold") == Some(1) {
                findings.push(finding(
                    resource,
                    Pillar::Resilience,
                    REASON_RES_WEAK_UNHEALTHY_THRESHOLD,
                    Severity::Low,
                    format!(
                        "App Runner service {} marks instances unhealthy after a single failed health check; raise the unhealthy threshold to avoid replacement flapping",
                        resource.resource_id
                    ),
                    json!({
                        "health_check_protocol": protocol,
                        "health_check_unhealthy_threshold": 1,
                    }),
                ));
            }
        }
        None => {
            findings.push(finding(
                resource,
                Pillar::Resilience,
                REASON_RES_HEALTH_CHECK_DATA_NOT_COLLECTED,
                Severity::Low,
                format!(
                    "Health check configuration for App Runner service {} is not collected yet; resilience pillar cannot be fully assessed",
                    resource.resource_id
                ),
                json!({ "health_check_collected": false }),
            ));
        }
    }

    match data_bool(resource, "observability_enabled") {
        Some(false) => {
            findings.push(finding(
                resource,
                Pillar::Resilience,
                REASON_RES_OBSERVABILITY_DISABLED,
                Severity::Low,
                format!(
                    "App Runner service {} has observability (tracing) disabled; incidents will lack request-level evidence",
                    resource.resource_id
                ),
                json!({ "observability_enabled": false }),
            ));
        }
        Some(true) => {}
        None => {
            findings.push(finding(
                resource,
                Pillar::Resilience,
                REASON_RES_OBSERVABILITY_DATA_NOT_COLLECTED,
                Severity::Low,
                format!(
                    "Observability configuration for App Runner service {} is not collected yet; resilience pillar cannot be fully assessed",
                    resource.resource_id
                ),
                json!({ "observability_collected": false }),
            ));
        }
    }
}

fn evaluate_security(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    match data_bool(resource, "is_publicly_accessible") {
        Some(true) => {
            findings.push(finding(
                resource,
                Pillar::Security,
                REASON_SEC_PUBLIC_INGRESS,
                Severity::Medium,
                format!(
                    "App Runner service {} is publicly accessible on the internet; confirm it is meant to be public or switch ingress to private (VPC endpoint)",
                    resource.resource_id
                ),
                json!({ "is_publicly_accessible": true }),
            ));
        }
        Some(false) => {}
        None => {
            findings.push(finding(
                resource,
                Pillar::Security,
                REASON_SEC_INGRESS_DATA_NOT_COLLECTED,
                Severity::Low,
                format!(
                    "Ingress configuration for App Runner service {} is not collected yet; public-exposure posture cannot be assessed",
                    resource.resource_id
                ),
                json!({ "ingress_collected": false }),
            ));
        }
    }

    match data_bool(resource, "customer_managed_kms") {
        Some(false) => {
            findings.push(finding(
                resource,
                Pillar::Security,
                REASON_SEC_DEFAULT_ENCRYPTION_KEY,
                Severity::Low,
                format!(
                    "App Runner service {} encrypts source and secrets with the default AWS-managed key; use a customer-managed KMS key for auditable key control",
                    resource.resource_id
                ),
                json!({ "customer_managed_kms": false }),
            ));
        }
        Some(true) => {}
        None => {
            findings.push(finding(
                resource,
                Pillar::Security,
                REASON_SEC_ENCRYPTION_DATA_NOT_COLLECTED,
                Severity::Low,
                format!(
                    "Encryption configuration for App Runner service {} is not collected yet; key-management posture cannot be assessed",
                    resource.resource_id
                ),
                json!({ "encryption_collected": false }),
            ));
        }
    }

    // Only judge the instance role when the instance configuration block was
    // actually collected; otherwise the cost pillar already reports the gap.
    let instance_collected = data_bool(resource, "instance_configuration_collected") == Some(true);
    if instance_collected && data_str(&resource.resource_data, "instance_role_arn").is_none() {
        findings.push(finding(
            resource,
            Pillar::Security,
            REASON_SEC_NO_INSTANCE_ROLE,
            Severity::Low,
            format!(
                "App Runner service {} has no instance role; application code cannot use scoped IAM credentials and may resort to static secrets",
                resource.resource_id
            ),
            json!({ "instance_role_arn_present": false }),
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;
    use serde_json::Value;
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
                "arn:aws:apprunner:us-east-1:123456789012:service/{}/{}",
                resource_id, resource_id
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
        DateTime::parse_from_rfc3339("2026-06-10T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc)
    }

    fn healthy_service_data() -> Value {
        json!({
            "service_name": "web-api",
            "service_arn": "arn:aws:apprunner:us-east-1:123456789012:service/web-api/svc-ok",
            "service_id": "svc-ok",
            "service_url": "abc123.us-east-1.awsapprunner.com",
            "status": "RUNNING",
            "created_at": "2026-01-01T00:00:00Z",
            "updated_at": "2026-01-02T00:00:00Z",
            "source_type": "IMAGE",
            "auto_deployments_enabled": true,
            "instance_configuration_collected": true,
            "instance_cpu": "1024",
            "instance_memory": "2048",
            "instance_role_arn": "arn:aws:iam::123456789012:role/apprunner-instance",
            "customer_managed_kms": true,
            "kms_key": "arn:aws:kms:us-east-1:123456789012:key/abc",
            "health_check_protocol": "HTTP",
            "health_check_path": "/health",
            "health_check_interval": 10,
            "health_check_timeout": 5,
            "health_check_healthy_threshold": 1,
            "health_check_unhealthy_threshold": 3,
            "auto_scaling_configuration_arn": "arn:aws:apprunner:us-east-1:123456789012:autoscalingconfiguration/prod-asc/1/xyz",
            "auto_scaling_configuration_name": "prod-asc",
            "egress_type": "VPC",
            "vpc_connector_arn": "arn:aws:apprunner:us-east-1:123456789012:vpcconnector/prod/1/abc",
            "is_publicly_accessible": false,
            "ip_address_type": "IPV4",
            "observability_enabled": true,
            "observability_configuration_arn": "arn:aws:apprunner:us-east-1:123456789012:observabilityconfiguration/prod/1/abc",
        })
    }

    fn codes(report: &PillarReport) -> Vec<&str> {
        report
            .findings
            .iter()
            .map(|f| f.reason_code.as_str())
            .collect()
    }

    fn remove(data: &mut Value, key: &str) {
        data.as_object_mut().unwrap().remove(key);
    }

    #[test]
    fn healthy_service_passes_all_pillars() {
        let r = fixture(
            "svc-ok",
            json!({"team": "web"}),
            healthy_service_data(),
            now(),
        );
        for pillar in [Pillar::Cost, Pillar::Security, Pillar::Resilience] {
            let report = evaluate_apprunner_fleet(std::slice::from_ref(&r), pillar, now());
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
    fn cost_flags_paused_service() {
        let mut data = healthy_service_data();
        data["status"] = json!("PAUSED");
        let r = fixture("svc-paused", json!({}), data, now());
        let report = evaluate_apprunner_fleet(&[r], Pillar::Cost, now());
        assert_eq!(codes(&report), vec![REASON_COST_PAUSED_SERVICE]);
        assert!(report.findings[0].message.contains("provisioned"));
    }

    #[test]
    fn cost_flags_max_size_instance() {
        for cpu in ["4096", "4 vCPU"] {
            let mut data = healthy_service_data();
            data["instance_cpu"] = json!(cpu);
            let r = fixture("svc-big", json!({}), data, now());
            let report = evaluate_apprunner_fleet(&[r], Pillar::Cost, now());
            assert_eq!(
                codes(&report),
                vec![REASON_COST_MAX_SIZE_INSTANCE],
                "cpu={}",
                cpu
            );
        }
    }

    #[test]
    fn cost_reports_gap_when_instance_data_missing() {
        let mut data = healthy_service_data();
        remove(&mut data, "instance_cpu");
        let r = fixture("svc-nocpu", json!({}), data, now());
        let report = evaluate_apprunner_fleet(&[r], Pillar::Cost, now());
        assert_eq!(
            codes(&report),
            vec![REASON_COST_INSTANCE_DATA_NOT_COLLECTED]
        );
    }

    #[test]
    fn cost_flags_default_autoscaling_configuration() {
        let mut data = healthy_service_data();
        data["auto_scaling_configuration_name"] = json!("DefaultConfiguration");
        let r = fixture("svc-defasc", json!({}), data, now());
        let report = evaluate_apprunner_fleet(&[r], Pillar::Cost, now());
        assert_eq!(codes(&report), vec![REASON_COST_DEFAULT_AUTOSCALING]);
    }

    #[test]
    fn cost_reports_gap_when_autoscaling_missing() {
        let mut data = healthy_service_data();
        remove(&mut data, "auto_scaling_configuration_name");
        let r = fixture("svc-noasc", json!({}), data, now());
        let report = evaluate_apprunner_fleet(&[r], Pillar::Cost, now());
        assert_eq!(
            codes(&report),
            vec![REASON_COST_AUTOSCALING_DATA_NOT_COLLECTED]
        );
    }

    #[test]
    fn resilience_flags_failed_state_as_high() {
        for state in ["CREATE_FAILED", "DELETE_FAILED"] {
            let mut data = healthy_service_data();
            data["status"] = json!(state);
            let r = fixture("svc-failed", json!({}), data, now());
            let report = evaluate_apprunner_fleet(&[r], Pillar::Resilience, now());
            assert_eq!(
                codes(&report),
                vec![REASON_RES_SERVICE_FAILED_STATE],
                "state={}",
                state
            );
            assert!(matches!(report.findings[0].severity, Severity::High));
        }
    }

    #[test]
    fn resilience_does_not_flag_running_or_paused_status() {
        for state in ["RUNNING", "PAUSED", "OPERATION_IN_PROGRESS"] {
            let mut data = healthy_service_data();
            data["status"] = json!(state);
            let r = fixture("svc-state", json!({}), data, now());
            let report = evaluate_apprunner_fleet(&[r], Pillar::Resilience, now());
            assert!(
                report.findings.is_empty(),
                "unexpected for {}: {:?}",
                state,
                report.findings
            );
        }
    }

    #[test]
    fn resilience_reports_gap_when_status_missing() {
        let mut data = healthy_service_data();
        remove(&mut data, "status");
        let r = fixture("svc-nostatus", json!({}), data, now());
        let report = evaluate_apprunner_fleet(&[r], Pillar::Resilience, now());
        assert_eq!(codes(&report), vec![REASON_RES_STATUS_DATA_NOT_COLLECTED]);
    }

    #[test]
    fn resilience_flags_weak_unhealthy_threshold() {
        let mut data = healthy_service_data();
        data["health_check_unhealthy_threshold"] = json!(1);
        let r = fixture("svc-flappy", json!({}), data, now());
        let report = evaluate_apprunner_fleet(&[r], Pillar::Resilience, now());
        assert_eq!(codes(&report), vec![REASON_RES_WEAK_UNHEALTHY_THRESHOLD]);
    }

    #[test]
    fn resilience_reports_gap_when_health_check_missing() {
        let mut data = healthy_service_data();
        remove(&mut data, "health_check_protocol");
        remove(&mut data, "health_check_unhealthy_threshold");
        let r = fixture("svc-nohc", json!({}), data, now());
        let report = evaluate_apprunner_fleet(&[r], Pillar::Resilience, now());
        assert_eq!(
            codes(&report),
            vec![REASON_RES_HEALTH_CHECK_DATA_NOT_COLLECTED]
        );
    }

    #[test]
    fn resilience_flags_observability_disabled() {
        let mut data = healthy_service_data();
        data["observability_enabled"] = json!(false);
        let r = fixture("svc-noobs", json!({}), data, now());
        let report = evaluate_apprunner_fleet(&[r], Pillar::Resilience, now());
        assert_eq!(codes(&report), vec![REASON_RES_OBSERVABILITY_DISABLED]);
    }

    #[test]
    fn resilience_reports_gap_when_observability_missing() {
        let mut data = healthy_service_data();
        remove(&mut data, "observability_enabled");
        let r = fixture("svc-obsgap", json!({}), data, now());
        let report = evaluate_apprunner_fleet(&[r], Pillar::Resilience, now());
        assert_eq!(
            codes(&report),
            vec![REASON_RES_OBSERVABILITY_DATA_NOT_COLLECTED]
        );
    }

    #[test]
    fn security_flags_public_ingress_as_medium() {
        let mut data = healthy_service_data();
        data["is_publicly_accessible"] = json!(true);
        let r = fixture("svc-public", json!({}), data, now());
        let report = evaluate_apprunner_fleet(&[r], Pillar::Security, now());
        assert_eq!(codes(&report), vec![REASON_SEC_PUBLIC_INGRESS]);
        assert!(matches!(report.findings[0].severity, Severity::Medium));
    }

    #[test]
    fn security_reports_gap_when_ingress_missing() {
        let mut data = healthy_service_data();
        remove(&mut data, "is_publicly_accessible");
        let r = fixture("svc-ingressgap", json!({}), data, now());
        let report = evaluate_apprunner_fleet(&[r], Pillar::Security, now());
        assert_eq!(codes(&report), vec![REASON_SEC_INGRESS_DATA_NOT_COLLECTED]);
    }

    #[test]
    fn security_flags_default_encryption_key() {
        let mut data = healthy_service_data();
        data["customer_managed_kms"] = json!(false);
        remove(&mut data, "kms_key");
        let r = fixture("svc-defkey", json!({}), data, now());
        let report = evaluate_apprunner_fleet(&[r], Pillar::Security, now());
        assert_eq!(codes(&report), vec![REASON_SEC_DEFAULT_ENCRYPTION_KEY]);
    }

    #[test]
    fn security_reports_gap_when_encryption_missing() {
        let mut data = healthy_service_data();
        remove(&mut data, "customer_managed_kms");
        remove(&mut data, "kms_key");
        let r = fixture("svc-encgap", json!({}), data, now());
        let report = evaluate_apprunner_fleet(&[r], Pillar::Security, now());
        assert_eq!(
            codes(&report),
            vec![REASON_SEC_ENCRYPTION_DATA_NOT_COLLECTED]
        );
    }

    #[test]
    fn security_flags_missing_instance_role() {
        let mut data = healthy_service_data();
        remove(&mut data, "instance_role_arn");
        let r = fixture("svc-norole", json!({}), data, now());
        let report = evaluate_apprunner_fleet(&[r], Pillar::Security, now());
        assert_eq!(codes(&report), vec![REASON_SEC_NO_INSTANCE_ROLE]);
    }

    #[test]
    fn security_skips_role_check_when_instance_data_missing() {
        let mut data = healthy_service_data();
        remove(&mut data, "instance_configuration_collected");
        remove(&mut data, "instance_role_arn");
        let r = fixture("svc-rolegap", json!({}), data, now());
        let report = evaluate_apprunner_fleet(&[r], Pillar::Security, now());
        // The cost pillar reports the instance-data gap; security must not
        // accuse the service of running without a role on missing evidence.
        assert!(
            !codes(&report).contains(&REASON_SEC_NO_INSTANCE_ROLE),
            "unexpected: {:?}",
            report.findings
        );
    }

    #[test]
    fn stale_inventory_is_flagged() {
        let mut r = fixture("svc-stale", json!({}), healthy_service_data(), now());
        r.last_refreshed = now() - Duration::hours(48);
        let report = evaluate_apprunner_fleet(&[r], Pillar::Resilience, now());
        assert_eq!(report.stale_resources, 1);
        assert!(codes(&report).contains(&REASON_INV_STALE_DATA));
    }

    #[test]
    fn non_apprunner_resources_are_skipped_and_not_counted() {
        let mut r = fixture("queue-1", json!({}), json!({}), now());
        r.resource_type = "SqsQueue".to_string();
        let report = evaluate_apprunner_fleet(&[r], Pillar::Cost, now());
        assert_eq!(report.resources_evaluated, 0);
        assert!(report.findings.is_empty());
    }
}

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

// Deterministic Lambda inventory evaluators for the cost, security, and
// resilience pillars (roadmap rows 01-AWS-CLOUD-00127/00136/00163).
//
// Pure domain logic over collected `aws_resources` rows; no AWS calls,
// no database access, no LLM. Evaluates fields persisted by
// lambda_control_plane: runtime, memory_size, timeout, architectures.

use chrono::{DateTime, Utc};
use serde_json::json;

use crate::models::aws_resource::Model as AwsResourceModel;
use crate::services::aws::inventory::types::{
    check_stale, has_any_tag, score_pillar, InventoryFinding, Pillar, PillarReport, Severity,
    COST_ALLOCATION_TAG_KEYS, OWNER_TAG_KEYS,
};

// Reason codes are the stable contract for findings; never reuse or rename.
pub const REASON_COST_MISSING_ALLOCATION_TAGS: &str = "LAMBDA_COST_MISSING_ALLOCATION_TAGS";
pub const REASON_COST_X86_ONLY_ARCHITECTURE: &str = "LAMBDA_COST_X86_ONLY_ARCHITECTURE";
pub const REASON_SEC_DEPRECATED_RUNTIME: &str = "LAMBDA_SEC_DEPRECATED_RUNTIME";
pub const REASON_SEC_MISSING_OWNER_TAG: &str = "LAMBDA_SEC_MISSING_OWNER_TAG";
pub const REASON_RES_MISSING_CONFIG_DATA: &str = "LAMBDA_RES_MISSING_CONFIG_DATA";
pub const REASON_INV_STALE_DATA: &str = "LAMBDA_INV_STALE_DATA";

/// Runtimes AWS has deprecated (no more security patches). Kept as an
/// explicit deterministic list; extend when AWS announces new deprecations.
pub const DEPRECATED_RUNTIMES: &[&str] = &[
    "python2.7",
    "python3.6",
    "python3.7",
    "nodejs10.x",
    "nodejs12.x",
    "nodejs14.x",
    "nodejs16.x",
    "dotnetcore2.1",
    "dotnetcore3.1",
    "dotnet5.0",
    "ruby2.5",
    "ruby2.7",
    "go1.x",
    "java8",
    "provided",
];

/// Evaluate every Lambda function in the fleet for one pillar.
pub fn evaluate_lambda_fleet(
    resources: &[AwsResourceModel],
    pillar: Pillar,
    now: DateTime<Utc>,
) -> PillarReport {
    let mut findings: Vec<InventoryFinding> = Vec::new();
    let mut stale_resources = 0usize;

    for resource in resources {
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
        resources_evaluated: resources.len(),
        stale_resources,
        score,
        findings,
    }
}

fn evaluate_cost(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    if !has_any_tag(&resource.tags, COST_ALLOCATION_TAG_KEYS) {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Cost,
            reason_code: REASON_COST_MISSING_ALLOCATION_TAGS.to_string(),
            severity: Severity::Medium,
            message: format!(
                "Function {} has no cost allocation tag (expected one of: {})",
                resource.resource_id,
                COST_ALLOCATION_TAG_KEYS.join(", ")
            ),
            evidence: json!({ "tags": resource.tags }),
        });
    }

    // arm64 (Graviton) is cheaper per GB-second; x86_64-only functions are a
    // deterministic savings opportunity worth review.
    let architectures: Vec<String> = resource
        .resource_data
        .get("architectures")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|a| a.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();
    if !architectures.is_empty() && architectures.iter().all(|a| a == "x86_64") {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Cost,
            reason_code: REASON_COST_X86_ONLY_ARCHITECTURE.to_string(),
            severity: Severity::Low,
            message: format!(
                "Function {} runs only on x86_64; evaluate arm64 (Graviton) for lower per-GB-second cost",
                resource.resource_id
            ),
            evidence: json!({ "architectures": architectures }),
        });
    }
}

fn evaluate_security(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    if let Some(runtime) = resource
        .resource_data
        .get("runtime")
        .and_then(|v| v.as_str())
    {
        if DEPRECATED_RUNTIMES.contains(&runtime) {
            findings.push(InventoryFinding {
                resource_id: resource.resource_id.clone(),
                arn: resource.arn.clone(),
                pillar: Pillar::Security,
                reason_code: REASON_SEC_DEPRECATED_RUNTIME.to_string(),
                severity: Severity::High,
                message: format!(
                    "Function {} uses deprecated runtime {}; it no longer receives security patches",
                    resource.resource_id, runtime
                ),
                evidence: json!({ "runtime": runtime }),
            });
        }
    }

    if !has_any_tag(&resource.tags, OWNER_TAG_KEYS) {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Security,
            reason_code: REASON_SEC_MISSING_OWNER_TAG.to_string(),
            severity: Severity::Low,
            message: format!(
                "Function {} has no owner/team tag; security findings cannot be routed to an owner",
                resource.resource_id
            ),
            evidence: json!({ "tags": resource.tags }),
        });
    }
}

fn evaluate_resilience(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    let timeout = resource
        .resource_data
        .get("timeout")
        .and_then(|v| v.as_i64());
    let memory_size = resource
        .resource_data
        .get("memory_size")
        .and_then(|v| v.as_i64());
    if timeout.is_none() || memory_size.is_none() {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Resilience,
            reason_code: REASON_RES_MISSING_CONFIG_DATA.to_string(),
            severity: Severity::Medium,
            message: format!(
                "Function {} is missing timeout or memory configuration in inventory; resilience limits cannot be assessed",
                resource.resource_id
            ),
            evidence: json!({ "timeout": timeout, "memory_size": memory_size }),
        });
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
        refreshed_hours_ago: i64,
        now: DateTime<Utc>,
    ) -> AwsResourceModel {
        let refreshed = now - Duration::hours(refreshed_hours_ago);
        AwsResourceModel {
            id: Uuid::new_v4(),
            sync_id: None,
            account_id: "123456789012".to_string(),
            profile: None,
            region: "us-east-1".to_string(),
            resource_type: "LambdaFunction".to_string(),
            resource_id: resource_id.to_string(),
            arn: format!(
                "arn:aws:lambda:us-east-1:123456789012:function:{}",
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
        DateTime::parse_from_rfc3339("2026-06-10T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc)
    }

    fn healthy_data() -> Value {
        json!({
            "function_name": "fn",
            "runtime": "python3.12",
            "timeout": 30,
            "memory_size": 256,
            "architectures": ["arm64"],
        })
    }

    #[test]
    fn cost_flags_missing_allocation_tags_and_x86_only_architecture() {
        let mut data = healthy_data();
        data["architectures"] = json!(["x86_64"]);
        let r = fixture("fn-untagged", json!({}), data, 1, now());
        let report = evaluate_lambda_fleet(&[r], Pillar::Cost, now());
        let codes: Vec<&str> = report
            .findings
            .iter()
            .map(|f| f.reason_code.as_str())
            .collect();
        assert!(codes.contains(&REASON_COST_MISSING_ALLOCATION_TAGS));
        assert!(codes.contains(&REASON_COST_X86_ONLY_ARCHITECTURE));
        let arch = report
            .findings
            .iter()
            .find(|f| f.reason_code == REASON_COST_X86_ONLY_ARCHITECTURE)
            .unwrap();
        assert_eq!(arch.evidence["architectures"], json!(["x86_64"]));
    }

    #[test]
    fn cost_passes_for_tagged_arm64_function() {
        let r = fixture(
            "fn-good",
            json!({"team": "payments"}),
            healthy_data(),
            1,
            now(),
        );
        let report = evaluate_lambda_fleet(&[r], Pillar::Cost, now());
        assert!(
            report.findings.is_empty(),
            "unexpected: {:?}",
            report.findings
        );
        assert_eq!(report.score, 100);
    }

    #[test]
    fn security_flags_deprecated_runtime_as_high() {
        let mut data = healthy_data();
        data["runtime"] = json!("python2.7");
        let r = fixture("fn-old", json!({"owner": "sre"}), data, 1, now());
        let report = evaluate_lambda_fleet(&[r], Pillar::Security, now());
        let finding = report
            .findings
            .iter()
            .find(|f| f.reason_code == REASON_SEC_DEPRECATED_RUNTIME)
            .expect("deprecated runtime finding");
        assert_eq!(finding.severity, Severity::High);
        assert_eq!(finding.evidence["runtime"], json!("python2.7"));
    }

    #[test]
    fn security_flags_missing_owner_tag_as_low() {
        let r = fixture("fn-orphan", json!({}), healthy_data(), 1, now());
        let report = evaluate_lambda_fleet(&[r], Pillar::Security, now());
        assert_eq!(
            report
                .findings
                .iter()
                .map(|f| f.reason_code.as_str())
                .collect::<Vec<_>>(),
            vec![REASON_SEC_MISSING_OWNER_TAG]
        );
    }

    #[test]
    fn security_passes_for_owned_current_runtime() {
        let r = fixture("fn-ok", json!({"owner": "sre"}), healthy_data(), 1, now());
        let report = evaluate_lambda_fleet(&[r], Pillar::Security, now());
        assert!(
            report.findings.is_empty(),
            "unexpected: {:?}",
            report.findings
        );
    }

    #[test]
    fn resilience_flags_missing_timeout_or_memory_config() {
        let r = fixture(
            "fn-noconf",
            json!({"owner": "sre"}),
            json!({"function_name": "fn-noconf", "runtime": "python3.12"}),
            1,
            now(),
        );
        let report = evaluate_lambda_fleet(&[r], Pillar::Resilience, now());
        let finding = report
            .findings
            .iter()
            .find(|f| f.reason_code == REASON_RES_MISSING_CONFIG_DATA)
            .expect("missing config finding");
        assert_eq!(finding.evidence["timeout"], json!(null));
    }

    #[test]
    fn stale_inventory_is_reported_as_failure_path() {
        let r = fixture(
            "fn-stale",
            json!({"owner": "sre", "project": "mayyam"}),
            healthy_data(),
            48,
            now(),
        );
        let report = evaluate_lambda_fleet(&[r], Pillar::Cost, now());
        assert_eq!(report.stale_resources, 1);
        assert!(report
            .findings
            .iter()
            .any(|f| f.reason_code == REASON_INV_STALE_DATA));
    }
}

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

// Deterministic API Gateway inventory evaluators for the cost, security,
// and resilience pillars (roadmap rows 01-AWS-CLOUD-02710/02719/02746).
//
// Evaluates ApiGatewayRestApi, ApiGatewayStage, and ApiGatewayMethod rows
// persisted by api_gateway_control_plane (snake_case keys:
// endpoint_configuration, cache_cluster_enabled, cache_cluster_size,
// access_log_settings, tracing_enabled, web_acl_arn, authorization_type,
// api_key_required, ...). ApiGatewayResource rows carry only path layout
// and are not scored.

use chrono::{DateTime, Utc};
use serde_json::json;

use crate::models::aws_resource::Model as AwsResourceModel;
use crate::services::aws::inventory::types::{
    check_stale, data_str, score_pillar, InventoryFinding, Pillar, PillarReport, Severity,
};

// Reason codes are the stable contract for findings; never reuse or rename.
pub const REASON_COST_NO_TAGS: &str = "APIGW_COST_NO_TAGS";
pub const REASON_COST_CACHE_CLUSTER_ENABLED: &str = "APIGW_COST_CACHE_CLUSTER_ENABLED";
pub const REASON_SEC_METHOD_AUTH_NONE: &str = "APIGW_SEC_METHOD_AUTH_NONE";
pub const REASON_SEC_OPEN_METHOD_NO_API_KEY: &str = "APIGW_SEC_OPEN_METHOD_NO_API_KEY";
pub const REASON_SEC_AUTH_DATA_NOT_COLLECTED: &str = "APIGW_SEC_AUTH_DATA_NOT_COLLECTED";
pub const REASON_SEC_STAGE_NO_WAF: &str = "APIGW_SEC_STAGE_NO_WAF";
pub const REASON_RES_STAGE_ACCESS_LOGS_DISABLED: &str = "APIGW_RES_STAGE_ACCESS_LOGS_DISABLED";
pub const REASON_RES_STAGE_TRACING_DISABLED: &str = "APIGW_RES_STAGE_TRACING_DISABLED";
pub const REASON_RES_ENDPOINT_DATA_NOT_COLLECTED: &str = "APIGW_RES_ENDPOINT_DATA_NOT_COLLECTED";
pub const REASON_INV_STALE_DATA: &str = "APIGW_INV_STALE_DATA";

const TYPE_REST_API: &str = "ApiGatewayRestApi";
const TYPE_STAGE: &str = "ApiGatewayStage";
const TYPE_METHOD: &str = "ApiGatewayMethod";

fn data_bool(resource: &AwsResourceModel, key: &str) -> Option<bool> {
    resource.resource_data.get(key).and_then(|v| v.as_bool())
}

/// True when the key is absent or persisted as JSON null.
fn data_is_null(resource: &AwsResourceModel, key: &str) -> bool {
    resource
        .resource_data
        .get(key)
        .map(|v| v.is_null())
        .unwrap_or(true)
}

/// Evaluate every API Gateway REST API, stage, and method in the fleet for
/// one pillar. ApiGatewayResource rows are stale-checked but not scored.
pub fn evaluate_api_gateway_fleet(
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
    // Tag posture is scored once per API, not per stage/method, to avoid
    // multiplying the same gap across child rows.
    if resource.resource_type == TYPE_REST_API {
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
                    "REST API {} has no tags recorded (untagged resource or tag collection gap); cost allocation cannot be assessed",
                    resource.resource_id
                ),
                evidence: json!({ "tags": resource.tags }),
            });
        }
        return;
    }

    if resource.resource_type == TYPE_STAGE {
        if data_bool(resource, "cache_cluster_enabled") == Some(true) {
            let size = data_str(&resource.resource_data, "cache_cluster_size");
            findings.push(InventoryFinding {
                resource_id: resource.resource_id.clone(),
                arn: resource.arn.clone(),
                pillar: Pillar::Cost,
                reason_code: REASON_COST_CACHE_CLUSTER_ENABLED.to_string(),
                severity: Severity::Low,
                message: format!(
                    "Stage {} has a cache cluster enabled (size {}); stage caches bill hourly regardless of hit rate, verify utilization justifies the size",
                    resource.resource_id,
                    size.as_deref().unwrap_or("unknown")
                ),
                evidence: json!({
                    "cache_cluster_enabled": true,
                    "cache_cluster_size": size,
                }),
            });
        }
    }
}

fn evaluate_security(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    if resource.resource_type == TYPE_METHOD {
        let auth = data_str(&resource.resource_data, "authorization_type");
        match auth {
            None => {
                findings.push(InventoryFinding {
                    resource_id: resource.resource_id.clone(),
                    arn: resource.arn.clone(),
                    pillar: Pillar::Security,
                    reason_code: REASON_SEC_AUTH_DATA_NOT_COLLECTED.to_string(),
                    severity: Severity::Medium,
                    message: format!(
                        "Authorization type for method {} is not collected yet; security pillar cannot be fully assessed",
                        resource.resource_id
                    ),
                    evidence: json!({ "authorization_type_collected": false }),
                });
            }
            Some(auth) if auth == "NONE" => {
                findings.push(InventoryFinding {
                    resource_id: resource.resource_id.clone(),
                    arn: resource.arn.clone(),
                    pillar: Pillar::Security,
                    reason_code: REASON_SEC_METHOD_AUTH_NONE.to_string(),
                    severity: Severity::High,
                    message: format!(
                        "Method {} has authorization type NONE; it is callable without IAM, Cognito, or a custom authorizer",
                        resource.resource_id
                    ),
                    evidence: json!({ "authorization_type": auth }),
                });
                if data_bool(resource, "api_key_required") == Some(false) {
                    findings.push(InventoryFinding {
                        resource_id: resource.resource_id.clone(),
                        arn: resource.arn.clone(),
                        pillar: Pillar::Security,
                        reason_code: REASON_SEC_OPEN_METHOD_NO_API_KEY.to_string(),
                        severity: Severity::Medium,
                        message: format!(
                            "Open method {} also has no API key requirement; there is no caller gating at all",
                            resource.resource_id
                        ),
                        evidence: json!({
                            "authorization_type": "NONE",
                            "api_key_required": false,
                        }),
                    });
                }
            }
            Some(_) => {}
        }
        return;
    }

    if resource.resource_type == TYPE_STAGE && data_is_null(resource, "web_acl_arn") {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Security,
            reason_code: REASON_SEC_STAGE_NO_WAF.to_string(),
            severity: Severity::Low,
            message: format!(
                "Stage {} has no WAF web ACL associated; requests are not filtered before reaching the API",
                resource.resource_id
            ),
            evidence: json!({ "web_acl_arn": null }),
        });
    }
}

fn evaluate_resilience(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    if resource.resource_type == TYPE_REST_API {
        if data_is_null(resource, "endpoint_configuration") {
            findings.push(InventoryFinding {
                resource_id: resource.resource_id.clone(),
                arn: resource.arn.clone(),
                pillar: Pillar::Resilience,
                reason_code: REASON_RES_ENDPOINT_DATA_NOT_COLLECTED.to_string(),
                severity: Severity::Low,
                message: format!(
                    "Endpoint configuration for REST API {} is not collected yet; edge/regional placement cannot be assessed",
                    resource.resource_id
                ),
                evidence: json!({ "endpoint_configuration_collected": false }),
            });
        }
        return;
    }

    if resource.resource_type != TYPE_STAGE {
        return;
    }

    if data_is_null(resource, "access_log_settings") {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Resilience,
            reason_code: REASON_RES_STAGE_ACCESS_LOGS_DISABLED.to_string(),
            severity: Severity::Medium,
            message: format!(
                "Stage {} has no access log settings configured; incidents cannot be diagnosed from request evidence",
                resource.resource_id
            ),
            evidence: json!({ "access_log_settings": null }),
        });
    }

    if data_bool(resource, "tracing_enabled") == Some(false) {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Resilience,
            reason_code: REASON_RES_STAGE_TRACING_DISABLED.to_string(),
            severity: Severity::Low,
            message: format!(
                "Stage {} has X-Ray tracing disabled; latency faults cannot be traced end to end",
                resource.resource_id
            ),
            evidence: json!({ "tracing_enabled": false }),
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
        resource_type: &str,
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
            resource_type: resource_type.to_string(),
            resource_id: resource_id.to_string(),
            arn: format!("arn:aws:apigateway:us-east-1::/restapis/{}", resource_id),
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

    fn healthy_rest_api_data() -> Value {
        json!({
            "description": "orders API",
            "api_key_source_type": "HEADER",
            "endpoint_configuration": { "types": ["REGIONAL"], "vpc_endpoint_ids": null },
            "disable_execute_api_endpoint": false,
            "minimum_compression_size": null,
            "policy": null,
            "version": "1",
        })
    }

    fn healthy_stage_data() -> Value {
        json!({
            "rest_api_id": "abc123",
            "deployment_id": "dep-1",
            "cache_cluster_enabled": false,
            "cache_cluster_size": null,
            "cache_cluster_status": "NOT_AVAILABLE",
            "access_log_settings": {
                "format": "$context.requestId",
                "destination_arn": "arn:aws:logs:us-east-1:123456789012:log-group:apigw",
            },
            "tracing_enabled": true,
            "web_acl_arn": "arn:aws:wafv2:us-east-1:123456789012:regional/webacl/x/1",
        })
    }

    fn healthy_method_data() -> Value {
        json!({
            "rest_api_id": "abc123",
            "resource_id": "res1",
            "http_method": "GET",
            "authorization_type": "AWS_IAM",
            "authorizer_id": null,
            "api_key_required": false,
        })
    }

    #[test]
    fn cost_flags_untagged_rest_api() {
        let r = fixture(
            "ApiGatewayRestApi",
            "abc123",
            json!({}),
            healthy_rest_api_data(),
            now(),
        );
        let report = evaluate_api_gateway_fleet(&[r], Pillar::Cost, now());
        assert_eq!(
            report
                .findings
                .iter()
                .map(|f| f.reason_code.as_str())
                .collect::<Vec<_>>(),
            vec![REASON_COST_NO_TAGS]
        );
    }

    #[test]
    fn cost_flags_stage_cache_cluster_enabled() {
        let mut data = healthy_stage_data();
        data["cache_cluster_enabled"] = json!(true);
        data["cache_cluster_size"] = json!("6.1");
        let r = fixture(
            "ApiGatewayStage",
            "abc123/prod",
            json!({"team": "api"}),
            data,
            now(),
        );
        let report = evaluate_api_gateway_fleet(&[r], Pillar::Cost, now());
        assert_eq!(
            report
                .findings
                .iter()
                .map(|f| f.reason_code.as_str())
                .collect::<Vec<_>>(),
            vec![REASON_COST_CACHE_CLUSTER_ENABLED]
        );
    }

    #[test]
    fn security_flags_open_method_without_api_key() {
        let mut data = healthy_method_data();
        data["authorization_type"] = json!("NONE");
        data["api_key_required"] = json!(false);
        let r = fixture(
            "ApiGatewayMethod",
            "abc123/res1/GET/GET",
            json!({"team": "api"}),
            data,
            now(),
        );
        let report = evaluate_api_gateway_fleet(&[r], Pillar::Security, now());
        let codes: Vec<&str> = report
            .findings
            .iter()
            .map(|f| f.reason_code.as_str())
            .collect();
        assert!(codes.contains(&REASON_SEC_METHOD_AUTH_NONE));
        assert!(codes.contains(&REASON_SEC_OPEN_METHOD_NO_API_KEY));
        let auth_none = report
            .findings
            .iter()
            .find(|f| f.reason_code == REASON_SEC_METHOD_AUTH_NONE)
            .unwrap();
        assert_eq!(auth_none.severity, Severity::High);
    }

    #[test]
    fn security_reports_gap_when_method_auth_not_collected() {
        let r = fixture(
            "ApiGatewayMethod",
            "abc123/res1/GET/GET",
            json!({"team": "api"}),
            json!({"rest_api_id": "abc123", "resource_id": "res1", "http_method": "GET", "authorization_type": null}),
            now(),
        );
        let report = evaluate_api_gateway_fleet(&[r], Pillar::Security, now());
        assert_eq!(
            report
                .findings
                .iter()
                .map(|f| f.reason_code.as_str())
                .collect::<Vec<_>>(),
            vec![REASON_SEC_AUTH_DATA_NOT_COLLECTED]
        );
    }

    #[test]
    fn security_flags_stage_without_waf() {
        let mut data = healthy_stage_data();
        data["web_acl_arn"] = json!(null);
        let r = fixture(
            "ApiGatewayStage",
            "abc123/prod",
            json!({"team": "api"}),
            data,
            now(),
        );
        let report = evaluate_api_gateway_fleet(&[r], Pillar::Security, now());
        assert_eq!(
            report
                .findings
                .iter()
                .map(|f| f.reason_code.as_str())
                .collect::<Vec<_>>(),
            vec![REASON_SEC_STAGE_NO_WAF]
        );
    }

    #[test]
    fn resilience_flags_stage_without_logs_and_tracing() {
        let mut data = healthy_stage_data();
        data["access_log_settings"] = json!(null);
        data["tracing_enabled"] = json!(false);
        let r = fixture(
            "ApiGatewayStage",
            "abc123/prod",
            json!({"team": "api"}),
            data,
            now(),
        );
        let report = evaluate_api_gateway_fleet(&[r], Pillar::Resilience, now());
        let codes: Vec<&str> = report
            .findings
            .iter()
            .map(|f| f.reason_code.as_str())
            .collect();
        assert!(codes.contains(&REASON_RES_STAGE_ACCESS_LOGS_DISABLED));
        assert!(codes.contains(&REASON_RES_STAGE_TRACING_DISABLED));
    }

    #[test]
    fn resilience_reports_gap_when_endpoint_configuration_not_collected() {
        let mut data = healthy_rest_api_data();
        data["endpoint_configuration"] = json!(null);
        let r = fixture(
            "ApiGatewayRestApi",
            "abc123",
            json!({"team": "api"}),
            data,
            now(),
        );
        let report = evaluate_api_gateway_fleet(&[r], Pillar::Resilience, now());
        assert_eq!(
            report
                .findings
                .iter()
                .map(|f| f.reason_code.as_str())
                .collect::<Vec<_>>(),
            vec![REASON_RES_ENDPOINT_DATA_NOT_COLLECTED]
        );
    }

    #[test]
    fn healthy_resources_pass_all_pillars_and_path_rows_are_ignored() {
        let api = fixture(
            "ApiGatewayRestApi",
            "abc123",
            json!({"team": "api"}),
            healthy_rest_api_data(),
            now(),
        );
        let stage = fixture(
            "ApiGatewayStage",
            "abc123/prod",
            json!({"team": "api"}),
            healthy_stage_data(),
            now(),
        );
        let method = fixture(
            "ApiGatewayMethod",
            "abc123/res1/GET/GET",
            json!({"team": "api"}),
            healthy_method_data(),
            now(),
        );
        let path_row = fixture(
            "ApiGatewayResource",
            "abc123/res1",
            json!({}),
            json!({"rest_api_id": "abc123", "path": "/orders", "path_part": "orders"}),
            now(),
        );
        let fleet = vec![api, stage, method, path_row];
        for pillar in [Pillar::Cost, Pillar::Security, Pillar::Resilience] {
            let report = evaluate_api_gateway_fleet(&fleet, pillar, now());
            assert!(
                report.findings.is_empty(),
                "unexpected for {:?}: {:?}",
                pillar,
                report.findings
            );
            assert_eq!(report.resources_evaluated, 4);
            assert_eq!(report.stale_resources, 0);
            assert_eq!(report.score, 100);
        }
    }
}

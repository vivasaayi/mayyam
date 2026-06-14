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

// Deterministic CloudFront inventory evaluators for the cost, security,
// and resilience pillars (roadmap rows 01-AWS-CLOUD-03277/03286/03313).
//
// Evaluates fields persisted by cloudfront_control_plane for
// CloudFrontDistribution rows: enabled, price_class, origins,
// default_cache_behavior.viewer_protocol_policy and
// cache_behaviors[].viewer_protocol_policy. CloudFrontFunction rows are
// ignored. Viewer certificate and access logging are not collected yet,
// so no findings are derived from them.

use chrono::{DateTime, Utc};
use serde_json::json;

use crate::models::aws_resource::Model as AwsResourceModel;
use crate::services::aws::inventory::types::{
    check_stale, score_pillar, InventoryFinding, Pillar, PillarReport, Severity,
};

const DISTRIBUTION_RESOURCE_TYPE: &str = "CloudFrontDistribution";

// Reason codes are the stable contract for findings; never reuse or rename.
pub const REASON_COST_NO_TAGS: &str = "CLOUDFRONT_COST_NO_TAGS";
pub const REASON_COST_DISABLED_DISTRIBUTION: &str = "CLOUDFRONT_COST_DISABLED_DISTRIBUTION";
pub const REASON_COST_PRICE_CLASS_ALL: &str = "CLOUDFRONT_COST_PRICE_CLASS_ALL";
pub const REASON_SEC_VIEWER_ALLOWS_HTTP: &str = "CLOUDFRONT_SEC_VIEWER_ALLOWS_HTTP";
pub const REASON_SEC_CACHE_BEHAVIOR_ALLOWS_HTTP: &str = "CLOUDFRONT_SEC_CACHE_BEHAVIOR_ALLOWS_HTTP";
pub const REASON_SEC_VIEWER_POLICY_DATA_NOT_COLLECTED: &str =
    "CLOUDFRONT_SEC_VIEWER_POLICY_DATA_NOT_COLLECTED";
pub const REASON_RES_SINGLE_ORIGIN: &str = "CLOUDFRONT_RES_SINGLE_ORIGIN";
pub const REASON_RES_ORIGIN_DATA_NOT_COLLECTED: &str = "CLOUDFRONT_RES_ORIGIN_DATA_NOT_COLLECTED";
pub const REASON_INV_STALE_DATA: &str = "CLOUDFRONT_INV_STALE_DATA";

/// Evaluate every CloudFront distribution in the fleet for one pillar.
pub fn evaluate_cloudfront_fleet(
    resources: &[AwsResourceModel],
    pillar: Pillar,
    now: DateTime<Utc>,
) -> PillarReport {
    let mut findings: Vec<InventoryFinding> = Vec::new();
    let mut stale_resources = 0usize;
    let mut evaluated = 0usize;

    for resource in resources {
        // CloudFrontFunction rows share the collector; skip them gracefully.
        if resource.resource_type != DISTRIBUTION_RESOURCE_TYPE {
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

/// Normalize an SDK enum string (Debug-formatted by the collector, e.g.
/// `AllowAll` or `allow-all`) for deterministic comparison.
fn normalize_enum(value: &str) -> String {
    value
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .collect::<String>()
        .to_ascii_lowercase()
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
                "Distribution {} has no tags recorded (untagged resource or tag collection gap); cost allocation cannot be assessed",
                resource.resource_id
            ),
            evidence: json!({ "tags": resource.tags }),
        });
    }

    let enabled = resource
        .resource_data
        .get("enabled")
        .and_then(|v| v.as_bool());
    if enabled == Some(false) {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Cost,
            reason_code: REASON_COST_DISABLED_DISTRIBUTION.to_string(),
            severity: Severity::Medium,
            message: format!(
                "Distribution {} is disabled but still provisioned; delete it or re-enable it to avoid paying for an unused configuration",
                resource.resource_id
            ),
            evidence: json!({ "enabled": false }),
        });
    }

    let price_class = resource
        .resource_data
        .get("price_class")
        .and_then(|v| v.as_str());
    if let Some(pc) = price_class {
        if normalize_enum(pc) == "priceclassall" {
            findings.push(InventoryFinding {
                resource_id: resource.resource_id.clone(),
                arn: resource.arn.clone(),
                pillar: Pillar::Cost,
                reason_code: REASON_COST_PRICE_CLASS_ALL.to_string(),
                severity: Severity::Low,
                message: format!(
                    "Distribution {} uses PriceClass_All (all edge locations); a restricted price class is cheaper when the audience is regional",
                    resource.resource_id
                ),
                evidence: json!({ "price_class": pc }),
            });
        }
    }
}

fn evaluate_security(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    let default_policy = resource
        .resource_data
        .get("default_cache_behavior")
        .and_then(|b| b.get("viewer_protocol_policy"))
        .and_then(|v| v.as_str());

    match default_policy {
        None => {
            findings.push(InventoryFinding {
                resource_id: resource.resource_id.clone(),
                arn: resource.arn.clone(),
                pillar: Pillar::Security,
                reason_code: REASON_SEC_VIEWER_POLICY_DATA_NOT_COLLECTED.to_string(),
                severity: Severity::Medium,
                message: format!(
                    "Viewer protocol policy for distribution {} is not collected yet; security pillar cannot be fully assessed",
                    resource.resource_id
                ),
                evidence: json!({ "default_cache_behavior_collected": false }),
            });
        }
        Some(policy) => {
            if normalize_enum(policy) == "allowall" {
                findings.push(InventoryFinding {
                    resource_id: resource.resource_id.clone(),
                    arn: resource.arn.clone(),
                    pillar: Pillar::Security,
                    reason_code: REASON_SEC_VIEWER_ALLOWS_HTTP.to_string(),
                    severity: Severity::High,
                    message: format!(
                        "Distribution {} default cache behavior allows plain HTTP from viewers (allow-all); traffic can be served unencrypted",
                        resource.resource_id
                    ),
                    evidence: json!({ "default_cache_behavior": { "viewer_protocol_policy": policy } }),
                });
            }
        }
    }

    // cache_behaviors is null when none are configured; that is healthy.
    let cache_behaviors = resource
        .resource_data
        .get("cache_behaviors")
        .and_then(|v| v.as_array());
    if let Some(behaviors) = cache_behaviors {
        let http_paths: Vec<String> = behaviors
            .iter()
            .filter(|b| {
                b.get("viewer_protocol_policy")
                    .and_then(|v| v.as_str())
                    .map(|p| normalize_enum(p) == "allowall")
                    .unwrap_or(false)
            })
            .map(|b| {
                b.get("path_pattern")
                    .and_then(|v| v.as_str())
                    .unwrap_or("<unknown>")
                    .to_string()
            })
            .collect();
        if !http_paths.is_empty() {
            findings.push(InventoryFinding {
                resource_id: resource.resource_id.clone(),
                arn: resource.arn.clone(),
                pillar: Pillar::Security,
                reason_code: REASON_SEC_CACHE_BEHAVIOR_ALLOWS_HTTP.to_string(),
                severity: Severity::Medium,
                message: format!(
                    "Distribution {} has {} cache behavior(s) allowing plain HTTP from viewers (allow-all)",
                    resource.resource_id,
                    http_paths.len()
                ),
                evidence: json!({ "allow_all_path_patterns": http_paths }),
            });
        }
    }
}

fn evaluate_resilience(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    let origins = resource
        .resource_data
        .get("origins")
        .and_then(|v| v.as_array());

    match origins {
        None => {
            findings.push(InventoryFinding {
                resource_id: resource.resource_id.clone(),
                arn: resource.arn.clone(),
                pillar: Pillar::Resilience,
                reason_code: REASON_RES_ORIGIN_DATA_NOT_COLLECTED.to_string(),
                severity: Severity::Medium,
                message: format!(
                    "Origin configuration for distribution {} is not collected yet; resilience pillar cannot be fully assessed",
                    resource.resource_id
                ),
                evidence: json!({ "origins_collected": false }),
            });
        }
        Some(items) => {
            if items.len() == 1 {
                let origin_domain = items[0]
                    .get("domain_name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("<unknown>");
                findings.push(InventoryFinding {
                    resource_id: resource.resource_id.clone(),
                    arn: resource.arn.clone(),
                    pillar: Pillar::Resilience,
                    reason_code: REASON_RES_SINGLE_ORIGIN.to_string(),
                    severity: Severity::Medium,
                    message: format!(
                        "Distribution {} has a single origin with no failover origin; an origin outage causes a full content outage",
                        resource.resource_id
                    ),
                    evidence: json!({ "origin_count": 1, "origin_domain_name": origin_domain }),
                });
            }
        }
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
            region: "global".to_string(),
            resource_type: "CloudFrontDistribution".to_string(),
            resource_id: resource_id.to_string(),
            arn: format!(
                "arn:aws:cloudfront::123456789012:distribution/{}",
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
            "domain_name": "d111111abcdef8.cloudfront.net",
            "enabled": true,
            "status": "Deployed",
            "origins": [
                { "id": "primary", "domain_name": "origin-a.example.com" },
                { "id": "failover", "domain_name": "origin-b.example.com" },
            ],
            "default_cache_behavior": {
                "target_origin_id": "primary",
                "viewer_protocol_policy": "RedirectToHttps",
            },
            "cache_behaviors": [
                {
                    "path_pattern": "/api/*",
                    "target_origin_id": "primary",
                    "viewer_protocol_policy": "HttpsOnly",
                },
            ],
            "http_version": "Http2",
            "is_ipv6_enabled": true,
            "price_class": "PriceClass100",
        })
    }

    #[test]
    fn cost_flags_no_tags_disabled_distribution_and_price_class_all() {
        let mut data = healthy_data();
        data["enabled"] = json!(false);
        data["price_class"] = json!("PriceClassAll");
        let r = fixture("E1IDLE", json!({}), data, now());
        let report = evaluate_cloudfront_fleet(&[r], Pillar::Cost, now());
        let codes: Vec<&str> = report
            .findings
            .iter()
            .map(|f| f.reason_code.as_str())
            .collect();
        assert!(codes.contains(&REASON_COST_NO_TAGS));
        assert!(codes.contains(&REASON_COST_DISABLED_DISTRIBUTION));
        assert!(codes.contains(&REASON_COST_PRICE_CLASS_ALL));
    }

    #[test]
    fn security_flags_viewer_protocol_allowing_http() {
        let mut data = healthy_data();
        data["default_cache_behavior"]["viewer_protocol_policy"] = json!("AllowAll");
        data["cache_behaviors"][0]["viewer_protocol_policy"] = json!("allow-all");
        let r = fixture("E1HTTP", json!({"team": "edge"}), data, now());
        let report = evaluate_cloudfront_fleet(&[r], Pillar::Security, now());
        let codes: Vec<&str> = report
            .findings
            .iter()
            .map(|f| f.reason_code.as_str())
            .collect();
        assert!(codes.contains(&REASON_SEC_VIEWER_ALLOWS_HTTP));
        assert!(codes.contains(&REASON_SEC_CACHE_BEHAVIOR_ALLOWS_HTTP));
    }

    #[test]
    fn security_reports_gap_when_viewer_policy_not_collected() {
        let r = fixture(
            "E1GAP",
            json!({"team": "edge"}),
            json!({"enabled": true, "default_cache_behavior": null, "cache_behaviors": null}),
            now(),
        );
        let report = evaluate_cloudfront_fleet(&[r], Pillar::Security, now());
        assert_eq!(
            report
                .findings
                .iter()
                .map(|f| f.reason_code.as_str())
                .collect::<Vec<_>>(),
            vec![REASON_SEC_VIEWER_POLICY_DATA_NOT_COLLECTED]
        );
    }

    #[test]
    fn resilience_flags_single_origin() {
        let mut data = healthy_data();
        data["origins"] = json!([{ "id": "only", "domain_name": "origin-a.example.com" }]);
        let r = fixture("E1SOLO", json!({"team": "edge"}), data, now());
        let report = evaluate_cloudfront_fleet(&[r], Pillar::Resilience, now());
        assert_eq!(
            report
                .findings
                .iter()
                .map(|f| f.reason_code.as_str())
                .collect::<Vec<_>>(),
            vec![REASON_RES_SINGLE_ORIGIN]
        );
    }

    #[test]
    fn resilience_reports_gap_when_origins_not_collected() {
        let r = fixture(
            "E1NOORIG",
            json!({"team": "edge"}),
            json!({"enabled": true, "origins": null}),
            now(),
        );
        let report = evaluate_cloudfront_fleet(&[r], Pillar::Resilience, now());
        assert_eq!(
            report
                .findings
                .iter()
                .map(|f| f.reason_code.as_str())
                .collect::<Vec<_>>(),
            vec![REASON_RES_ORIGIN_DATA_NOT_COLLECTED]
        );
    }

    #[test]
    fn stale_inventory_is_flagged() {
        let r = fixture("E1STALE", json!({"team": "edge"}), healthy_data(), now());
        let later = now() + Duration::hours(48);
        let report = evaluate_cloudfront_fleet(&[r], Pillar::Resilience, later);
        assert_eq!(report.stale_resources, 1);
        assert!(report
            .findings
            .iter()
            .any(|f| f.reason_code == REASON_INV_STALE_DATA));
    }

    #[test]
    fn cloudfront_function_rows_are_ignored() {
        let mut function_row = fixture(
            "edge-fn",
            json!({}),
            json!({"Name": "edge-fn", "Status": "DEPLOYED"}),
            now(),
        );
        function_row.resource_type = "CloudFrontFunction".to_string();
        let report = evaluate_cloudfront_fleet(&[function_row], Pillar::Cost, now());
        assert_eq!(report.resources_evaluated, 0);
        assert!(report.findings.is_empty());
        assert_eq!(report.score, 100);
    }

    #[test]
    fn healthy_distribution_passes_all_pillars() {
        let r = fixture("E1OK", json!({"team": "edge"}), healthy_data(), now());
        for pillar in [Pillar::Cost, Pillar::Security, Pillar::Resilience] {
            let report = evaluate_cloudfront_fleet(std::slice::from_ref(&r), pillar, now());
            assert!(
                report.findings.is_empty(),
                "unexpected for {:?}: {:?}",
                pillar,
                report.findings
            );
        }
    }
}

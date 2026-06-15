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

// Deterministic AI/LLM error-rate inventory evaluator for roadmap rows
// 25-AI-LLM-OBSERVABILITY-00197/00204/00225.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::models::llm_model::Model as LlmProviderModel;
use crate::services::aws::inventory::types::{
    score_pillar, InventoryFinding, Pillar, PillarReport, Severity, COST_ALLOCATION_TAG_KEYS,
    DEFAULT_STALE_AFTER_HOURS,
};

pub const RESOURCE_TYPE: &str = "AiLlmErrorRate";
pub const REASON_COST_OWNER_NOT_RECORDED: &str = "AI_LLM_ERROR_RATE_COST_OWNER_NOT_RECORDED";
pub const REASON_COST_ERROR_BUDGET_MISSING: &str = "AI_LLM_ERROR_RATE_COST_ERROR_BUDGET_MISSING";
pub const REASON_COST_HIGH_ERROR_RATE: &str = "AI_LLM_ERROR_RATE_COST_HIGH_ERROR_RATE";
pub const REASON_RES_SAMPLE_MISSING: &str = "AI_LLM_ERROR_RATE_RES_SAMPLE_MISSING";
pub const REASON_RES_RETRY_MISSING: &str = "AI_LLM_ERROR_RATE_RES_RETRY_MISSING";
pub const REASON_RES_ERROR_BUDGET_BREACH: &str = "AI_LLM_ERROR_RATE_RES_ERROR_BUDGET_BREACH";
pub const REASON_SEC_AUDIT_MISSING: &str = "AI_LLM_ERROR_RATE_SEC_AUDIT_MISSING";
pub const REASON_SEC_REDACTION_MISSING: &str = "AI_LLM_ERROR_RATE_SEC_REDACTION_MISSING";
pub const REASON_INV_STALE_DATA: &str = "AI_LLM_ERROR_RATE_INV_STALE_DATA";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorRateInventoryItem {
    pub route_id: String,
    pub provider_id: String,
    pub model_name: String,
    pub enabled: bool,
    pub owner: Option<String>,
    pub labels: Vec<String>,
    pub request_count: Option<u64>,
    pub failed_request_count: Option<u64>,
    pub error_rate_pct: Option<f64>,
    pub error_budget_pct: Option<f64>,
    pub retry_policy: Option<String>,
    pub fallback_model: Option<String>,
    pub alert_policy: Option<String>,
    pub audit_enabled: bool,
    pub redaction_policy: Option<String>,
    pub updated_at: DateTime<Utc>,
}

pub fn evaluate_error_rate_inventory(
    items: &[ErrorRateInventoryItem],
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

pub fn error_rate_inventory_item_from_model(model: &LlmProviderModel) -> ErrorRateInventoryItem {
    ErrorRateInventoryItem {
        route_id: model.id.to_string(),
        provider_id: model.provider_id.to_string(),
        model_name: model.model_name.clone(),
        enabled: model.enabled,
        owner: string_field(&model.model_config, &["owner", "team", "cost_owner"]),
        labels: string_array_field(&model.model_config, "labels"),
        request_count: integer_field(&model.model_config, &["request_count", "total_requests"]),
        failed_request_count: integer_field(
            &model.model_config,
            &["failed_request_count", "error_count", "failed_requests"],
        ),
        error_rate_pct: number_field(&model.model_config, &["error_rate_pct", "failure_rate_pct"]),
        error_budget_pct: number_field(
            &model.model_config,
            &["error_budget_pct", "max_error_rate_pct"],
        ),
        retry_policy: string_field(&model.model_config, &["retry_policy", "retry_strategy"]),
        fallback_model: string_field(
            &model.model_config,
            &["fallback_model", "fallback_model_id"],
        ),
        alert_policy: string_field(&model.model_config, &["alert_policy", "alerting_policy"]),
        audit_enabled: bool_field(
            &model.model_config,
            &["audit_enabled", "error_audit_enabled", "trace_enabled"],
        ),
        redaction_policy: string_field(
            &model.model_config,
            &[
                "telemetry_redaction_policy",
                "redaction_policy",
                "data_policy",
            ],
        ),
        updated_at: model.updated_at,
    }
}

fn evaluate_cost(
    item: &ErrorRateInventoryItem,
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
                "AI/LLM error-rate route {} has no owner metadata",
                item.model_name
            ),
            json!({"route_id": item.route_id, "labels": item.labels, "checked_keys": COST_ALLOCATION_TAG_KEYS}),
        ));
    }

    if item.error_budget_pct.is_none() {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_ERROR_BUDGET_MISSING,
            Severity::Medium,
            format!(
                "AI/LLM error-rate route {} has no error budget",
                item.model_name
            ),
            json!({"route_id": item.route_id, "recommendation": "Record maximum acceptable error rate so retries and failed-token spend can be quantified"}),
        ));
    }

    if item
        .error_budget_pct
        .zip(item.error_rate_pct)
        .map(|(budget, rate)| rate > budget)
        .unwrap_or(false)
    {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_HIGH_ERROR_RATE,
            Severity::High,
            format!("AI/LLM route {} error rate exceeds budget", item.model_name),
            json!({"route_id": item.route_id, "error_rate_pct": item.error_rate_pct, "error_budget_pct": item.error_budget_pct, "failed_request_count": item.failed_request_count, "estimated_monthly_impact": "review failed requests and retries that consume tokens without successful outcomes"}),
        ));
    }
}

fn evaluate_resilience(
    item: &ErrorRateInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if item.request_count.unwrap_or(0) == 0 || item.error_rate_pct.is_none() {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_SAMPLE_MISSING,
            Severity::High,
            format!(
                "AI/LLM error-rate route {} has no usable error-rate sample",
                item.model_name
            ),
            json!({"route_id": item.route_id, "request_count": item.request_count, "error_rate_pct": item.error_rate_pct}),
        ));
    }

    if item
        .retry_policy
        .as_deref()
        .unwrap_or_default()
        .trim()
        .is_empty()
        && item
            .fallback_model
            .as_deref()
            .unwrap_or_default()
            .trim()
            .is_empty()
    {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_RETRY_MISSING,
            Severity::High,
            format!(
                "AI/LLM error-rate route {} has no retry or fallback policy",
                item.model_name
            ),
            json!({"route_id": item.route_id}),
        ));
    }

    if item
        .error_budget_pct
        .zip(item.error_rate_pct)
        .map(|(budget, rate)| rate > budget)
        .unwrap_or(false)
    {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_ERROR_BUDGET_BREACH,
            Severity::High,
            format!(
                "AI/LLM error-rate route {} breaches error budget",
                item.model_name
            ),
            json!({"route_id": item.route_id, "error_rate_pct": item.error_rate_pct, "error_budget_pct": item.error_budget_pct, "alert_policy": item.alert_policy}),
        ));
    }
}

fn evaluate_security(
    item: &ErrorRateInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if !item.audit_enabled {
        findings.push(finding(
            item,
            pillar,
            REASON_SEC_AUDIT_MISSING,
            Severity::High,
            format!(
                "AI/LLM error-rate route {} has no audit trail for failures",
                item.model_name
            ),
            json!({"route_id": item.route_id, "recommendation": "Enable replayable error and tool-call audit evidence before routing agents through this model"}),
        ));
    }

    if item
        .redaction_policy
        .as_deref()
        .unwrap_or_default()
        .trim()
        .is_empty()
    {
        findings.push(finding(
            item,
            pillar,
            REASON_SEC_REDACTION_MISSING,
            Severity::High,
            format!(
                "AI/LLM error-rate route {} has no telemetry redaction policy",
                item.model_name
            ),
            json!({"route_id": item.route_id}),
        ));
    }
}

fn stale_finding(
    item: &ErrorRateInventoryItem,
    pillar: Pillar,
    now: DateTime<Utc>,
) -> Option<InventoryFinding> {
    let age_hours = now.signed_duration_since(item.updated_at).num_hours();
    if age_hours <= DEFAULT_STALE_AFTER_HOURS {
        return None;
    }

    Some(finding(
        item,
        pillar,
        REASON_INV_STALE_DATA,
        Severity::Medium,
        format!(
            "AI/LLM error-rate route {} inventory data is stale by {} hours",
            item.model_name, age_hours
        ),
        json!({"route_id": item.route_id, "age_hours": age_hours, "stale_after_hours": DEFAULT_STALE_AFTER_HOURS}),
    ))
}

fn finding(
    item: &ErrorRateInventoryItem,
    pillar: Pillar,
    reason_code: &str,
    severity: Severity,
    message: String,
    evidence: Value,
) -> InventoryFinding {
    InventoryFinding {
        resource_id: item.route_id.clone(),
        arn: format!("ai-llm-error-rate/{}", item.route_id),
        pillar,
        reason_code: reason_code.to_string(),
        severity,
        message,
        evidence,
    }
}

fn has_owner_metadata(item: &ErrorRateInventoryItem) -> bool {
    item.owner
        .as_deref()
        .map(|owner| !owner.trim().is_empty())
        .unwrap_or(false)
        || item.labels.iter().any(|label| {
            let normalized = label.to_ascii_lowercase();
            COST_ALLOCATION_TAG_KEYS
                .iter()
                .any(|key| normalized.starts_with(&format!("{key}=")))
        })
}

fn string_field(value: &Value, keys: &[&str]) -> Option<String> {
    keys.iter()
        .filter_map(|key| value.get(*key))
        .filter_map(Value::as_str)
        .map(str::trim)
        .find(|part| !part.is_empty())
        .map(ToString::to_string)
}

fn string_array_field(value: &Value, key: &str) -> Vec<String> {
    value
        .get(key)
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::trim)
                .filter(|item| !item.is_empty())
                .map(ToString::to_string)
                .collect()
        })
        .unwrap_or_default()
}

fn number_field(value: &Value, keys: &[&str]) -> Option<f64> {
    keys.iter()
        .filter_map(|key| value.get(*key))
        .find_map(Value::as_f64)
}

fn integer_field(value: &Value, keys: &[&str]) -> Option<u64> {
    keys.iter()
        .filter_map(|key| value.get(*key))
        .find_map(Value::as_u64)
}

fn bool_field(value: &Value, keys: &[&str]) -> bool {
    keys.iter()
        .filter_map(|key| value.get(*key))
        .find_map(Value::as_bool)
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    fn item() -> ErrorRateInventoryItem {
        ErrorRateInventoryItem {
            route_id: "route-1".to_string(),
            provider_id: "provider-1".to_string(),
            model_name: "deepseek-chat".to_string(),
            enabled: true,
            owner: Some("sre-ai".to_string()),
            labels: vec!["cost-center=ai-platform".to_string()],
            request_count: Some(10_000),
            failed_request_count: Some(12),
            error_rate_pct: Some(0.12),
            error_budget_pct: Some(1.0),
            retry_policy: Some("retry once on 429".to_string()),
            fallback_model: Some("deepseek-reasoner".to_string()),
            alert_policy: Some("page on sustained breach".to_string()),
            audit_enabled: true,
            redaction_policy: Some("redact prompt and response text".to_string()),
            updated_at: Utc::now(),
        }
    }

    fn codes(report: &PillarReport) -> Vec<String> {
        report
            .findings
            .iter()
            .map(|finding| finding.reason_code.clone())
            .collect()
    }

    #[test]
    fn healthy_error_rate_inventory_passes_claimed_pillars() {
        let now = Utc::now();
        for pillar in [Pillar::Cost, Pillar::Resilience, Pillar::Security] {
            let report = evaluate_error_rate_inventory(&[item()], pillar, now);
            assert_eq!(report.resources_evaluated, 1);
            assert!(report.findings.is_empty());
        }
    }

    #[test]
    fn cost_flags_missing_owner_budget_and_high_error_rate() {
        let mut item = item();
        item.owner = None;
        item.labels.clear();
        item.error_rate_pct = Some(5.0);

        let report = evaluate_error_rate_inventory(&[item], Pillar::Cost, Utc::now());
        let codes = codes(&report);

        assert!(codes.contains(&REASON_COST_OWNER_NOT_RECORDED.to_string()));
        assert!(codes.contains(&REASON_COST_HIGH_ERROR_RATE.to_string()));
    }

    #[test]
    fn resilience_flags_missing_samples_retry_and_error_budget_breach() {
        let mut item = item();
        item.request_count = Some(0);
        item.error_rate_pct = Some(5.0);
        item.retry_policy = None;
        item.fallback_model = None;

        let report = evaluate_error_rate_inventory(&[item], Pillar::Resilience, Utc::now());
        let codes = codes(&report);

        assert!(codes.contains(&REASON_RES_SAMPLE_MISSING.to_string()));
        assert!(codes.contains(&REASON_RES_RETRY_MISSING.to_string()));
        assert!(codes.contains(&REASON_RES_ERROR_BUDGET_BREACH.to_string()));
    }

    #[test]
    fn security_flags_missing_audit_and_redaction() {
        let mut item = item();
        item.audit_enabled = false;
        item.redaction_policy = None;

        let report = evaluate_error_rate_inventory(&[item], Pillar::Security, Utc::now());
        let codes = codes(&report);

        assert!(codes.contains(&REASON_SEC_AUDIT_MISSING.to_string()));
        assert!(codes.contains(&REASON_SEC_REDACTION_MISSING.to_string()));
    }

    #[test]
    fn stale_error_rate_inventory_is_counted_for_any_pillar() {
        let mut item = item();
        item.updated_at = Utc::now() - Duration::hours(DEFAULT_STALE_AFTER_HOURS + 1);

        let report = evaluate_error_rate_inventory(&[item], Pillar::Cost, Utc::now());

        assert_eq!(report.stale_resources, 1);
        assert!(codes(&report).contains(&REASON_INV_STALE_DATA.to_string()));
    }
}

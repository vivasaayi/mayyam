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

// Deterministic AI/LLM token usage inventory evaluator for roadmap rows
// 25-AI-LLM-OBSERVABILITY-00246/00253/00274.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::models::llm_model::Model as LlmProviderModel;
use crate::services::aws::inventory::types::{
    score_pillar, InventoryFinding, Pillar, PillarReport, Severity, COST_ALLOCATION_TAG_KEYS,
    DEFAULT_STALE_AFTER_HOURS,
};

pub const RESOURCE_TYPE: &str = "AiLlmTokenUsage";
pub const REASON_COST_OWNER_NOT_RECORDED: &str = "AI_LLM_TOKEN_USAGE_COST_OWNER_NOT_RECORDED";
pub const REASON_COST_TOKEN_BUDGET_MISSING: &str = "AI_LLM_TOKEN_USAGE_COST_BUDGET_MISSING";
pub const REASON_COST_USAGE_OVER_BUDGET: &str = "AI_LLM_TOKEN_USAGE_COST_OVER_BUDGET";
pub const REASON_RES_USAGE_SAMPLE_MISSING: &str = "AI_LLM_TOKEN_USAGE_RES_SAMPLE_MISSING";
pub const REASON_RES_RATE_LIMIT_MISSING: &str = "AI_LLM_TOKEN_USAGE_RES_RATE_LIMIT_MISSING";
pub const REASON_RES_BURST_GUARDRAIL_MISSING: &str =
    "AI_LLM_TOKEN_USAGE_RES_BURST_GUARDRAIL_MISSING";
pub const REASON_SEC_AUDIT_MISSING: &str = "AI_LLM_TOKEN_USAGE_SEC_AUDIT_MISSING";
pub const REASON_SEC_REDACTION_MISSING: &str = "AI_LLM_TOKEN_USAGE_SEC_REDACTION_MISSING";
pub const REASON_INV_STALE_DATA: &str = "AI_LLM_TOKEN_USAGE_INV_STALE_DATA";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenUsageInventoryItem {
    pub route_id: String,
    pub provider_id: String,
    pub model_name: String,
    pub enabled: bool,
    pub owner: Option<String>,
    pub labels: Vec<String>,
    pub prompt_tokens: Option<u64>,
    pub completion_tokens: Option<u64>,
    pub total_tokens: Option<u64>,
    pub monthly_token_budget: Option<u64>,
    pub max_tokens_per_request: Option<u64>,
    pub rate_limit_tokens_per_minute: Option<u64>,
    pub burst_guardrail: Option<String>,
    pub audit_enabled: bool,
    pub redaction_policy: Option<String>,
    pub updated_at: DateTime<Utc>,
}

pub fn evaluate_token_usage_inventory(
    items: &[TokenUsageInventoryItem],
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

pub fn token_usage_inventory_item_from_model(model: &LlmProviderModel) -> TokenUsageInventoryItem {
    let prompt_tokens = integer_field(&model.model_config, &["prompt_tokens", "input_tokens"]);
    let completion_tokens =
        integer_field(&model.model_config, &["completion_tokens", "output_tokens"]);
    let total_tokens = integer_field(&model.model_config, &["total_tokens", "token_count"])
        .or_else(|| {
            prompt_tokens
                .zip(completion_tokens)
                .map(|(prompt, completion)| prompt + completion)
        });

    TokenUsageInventoryItem {
        route_id: model.id.to_string(),
        provider_id: model.provider_id.to_string(),
        model_name: model.model_name.clone(),
        enabled: model.enabled,
        owner: string_field(&model.model_config, &["owner", "team", "cost_owner"]),
        labels: string_array_field(&model.model_config, "labels"),
        prompt_tokens,
        completion_tokens,
        total_tokens,
        monthly_token_budget: integer_field(
            &model.model_config,
            &["monthly_token_budget", "token_budget", "max_monthly_tokens"],
        ),
        max_tokens_per_request: integer_field(
            &model.model_config,
            &["max_tokens_per_request", "max_tokens_per_run", "max_tokens"],
        ),
        rate_limit_tokens_per_minute: integer_field(
            &model.model_config,
            &["rate_limit_tokens_per_minute", "tokens_per_minute"],
        ),
        burst_guardrail: string_field(
            &model.model_config,
            &["burst_guardrail", "throttle_policy", "quota_policy"],
        ),
        audit_enabled: bool_field(
            &model.model_config,
            &["audit_enabled", "usage_audit_enabled", "trace_enabled"],
        ),
        redaction_policy: string_field(
            &model.model_config,
            &[
                "usage_redaction_policy",
                "telemetry_redaction_policy",
                "redaction_policy",
                "data_policy",
            ],
        ),
        updated_at: model.updated_at,
    }
}

fn evaluate_cost(
    item: &TokenUsageInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if !has_owner_metadata(item) {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_OWNER_NOT_RECORDED,
            Severity::Medium,
            format!("AI/LLM token usage route {} has no owner metadata", item.model_name),
            json!({"route_id": item.route_id, "labels": item.labels, "checked_keys": COST_ALLOCATION_TAG_KEYS}),
        ));
    }

    if item.monthly_token_budget.is_none() {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_TOKEN_BUDGET_MISSING,
            Severity::High,
            format!(
                "AI/LLM token usage route {} has no monthly token budget",
                item.model_name
            ),
            json!({"route_id": item.route_id, "recommendation": "Record a monthly token budget or routing quota before spend reports are trusted"}),
        ));
    }

    if item
        .monthly_token_budget
        .zip(item.total_tokens)
        .map(|(budget, usage)| usage > budget)
        .unwrap_or(false)
    {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_USAGE_OVER_BUDGET,
            Severity::High,
            format!("AI/LLM token usage route {} is over budget", item.model_name),
            json!({"route_id": item.route_id, "total_tokens": item.total_tokens, "monthly_token_budget": item.monthly_token_budget, "estimated_monthly_impact": "review routing, prompt size, retries, and cache opportunities for high-token routes"}),
        ));
    }
}

fn evaluate_resilience(
    item: &TokenUsageInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if item.total_tokens.unwrap_or(0) == 0 {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_USAGE_SAMPLE_MISSING,
            Severity::High,
            format!(
                "AI/LLM token usage route {} has no usable token sample",
                item.model_name
            ),
            json!({"route_id": item.route_id, "prompt_tokens": item.prompt_tokens, "completion_tokens": item.completion_tokens, "total_tokens": item.total_tokens}),
        ));
    }

    if item.rate_limit_tokens_per_minute.is_none() {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_RATE_LIMIT_MISSING,
            Severity::Medium,
            format!(
                "AI/LLM token usage route {} has no token rate limit",
                item.model_name
            ),
            json!({"route_id": item.route_id, "recommendation": "Record token-per-minute limits so overload and throttling are predictable"}),
        ));
    }

    if item
        .burst_guardrail
        .as_deref()
        .unwrap_or_default()
        .trim()
        .is_empty()
        && item.max_tokens_per_request.is_none()
    {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_BURST_GUARDRAIL_MISSING,
            Severity::High,
            format!(
                "AI/LLM token usage route {} has no burst guardrail",
                item.model_name
            ),
            json!({"route_id": item.route_id, "recommendation": "Declare max tokens per request or throttling policy for bounded agentic investigation"}),
        ));
    }
}

fn evaluate_security(
    item: &TokenUsageInventoryItem,
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
                "AI/LLM token usage route {} has no usage audit trail",
                item.model_name
            ),
            json!({"route_id": item.route_id, "recommendation": "Enable replayable token usage audit evidence before routing agents through this model"}),
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
                "AI/LLM token usage route {} has no telemetry redaction policy",
                item.model_name
            ),
            json!({"route_id": item.route_id}),
        ));
    }
}

fn stale_finding(
    item: &TokenUsageInventoryItem,
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
            "AI/LLM token usage route {} inventory data is stale by {} hours",
            item.model_name, age_hours
        ),
        json!({"route_id": item.route_id, "age_hours": age_hours, "stale_after_hours": DEFAULT_STALE_AFTER_HOURS}),
    ))
}

fn finding(
    item: &TokenUsageInventoryItem,
    pillar: Pillar,
    reason_code: &str,
    severity: Severity,
    message: String,
    evidence: Value,
) -> InventoryFinding {
    InventoryFinding {
        resource_id: item.route_id.clone(),
        arn: format!("ai-llm-token-usage/{}", item.route_id),
        pillar,
        reason_code: reason_code.to_string(),
        severity,
        message,
        evidence,
    }
}

fn has_owner_metadata(item: &TokenUsageInventoryItem) -> bool {
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

    fn item() -> TokenUsageInventoryItem {
        TokenUsageInventoryItem {
            route_id: "route-1".to_string(),
            provider_id: "provider-1".to_string(),
            model_name: "deepseek-chat".to_string(),
            enabled: true,
            owner: Some("sre-ai".to_string()),
            labels: vec!["cost-center=ai-platform".to_string()],
            prompt_tokens: Some(40_000),
            completion_tokens: Some(20_000),
            total_tokens: Some(60_000),
            monthly_token_budget: Some(100_000),
            max_tokens_per_request: Some(8000),
            rate_limit_tokens_per_minute: Some(60_000),
            burst_guardrail: Some("throttle large prompts".to_string()),
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
    fn healthy_token_usage_inventory_passes_claimed_pillars() {
        let now = Utc::now();
        for pillar in [Pillar::Cost, Pillar::Resilience, Pillar::Security] {
            let report = evaluate_token_usage_inventory(&[item()], pillar, now);
            assert_eq!(report.resources_evaluated, 1);
            assert!(report.findings.is_empty());
        }
    }

    #[test]
    fn cost_flags_missing_owner_budget_and_usage_over_budget() {
        let mut item = item();
        item.owner = None;
        item.labels.clear();
        item.total_tokens = Some(120_000);

        let report = evaluate_token_usage_inventory(&[item], Pillar::Cost, Utc::now());
        let codes = codes(&report);

        assert!(codes.contains(&REASON_COST_OWNER_NOT_RECORDED.to_string()));
        assert!(codes.contains(&REASON_COST_USAGE_OVER_BUDGET.to_string()));
    }

    #[test]
    fn resilience_flags_missing_sample_rate_limit_and_burst_guardrail() {
        let mut item = item();
        item.total_tokens = Some(0);
        item.rate_limit_tokens_per_minute = None;
        item.burst_guardrail = None;
        item.max_tokens_per_request = None;

        let report = evaluate_token_usage_inventory(&[item], Pillar::Resilience, Utc::now());
        let codes = codes(&report);

        assert!(codes.contains(&REASON_RES_USAGE_SAMPLE_MISSING.to_string()));
        assert!(codes.contains(&REASON_RES_RATE_LIMIT_MISSING.to_string()));
        assert!(codes.contains(&REASON_RES_BURST_GUARDRAIL_MISSING.to_string()));
    }

    #[test]
    fn security_flags_missing_audit_and_redaction() {
        let mut item = item();
        item.audit_enabled = false;
        item.redaction_policy = None;

        let report = evaluate_token_usage_inventory(&[item], Pillar::Security, Utc::now());
        let codes = codes(&report);

        assert!(codes.contains(&REASON_SEC_AUDIT_MISSING.to_string()));
        assert!(codes.contains(&REASON_SEC_REDACTION_MISSING.to_string()));
    }

    #[test]
    fn stale_token_usage_inventory_is_counted_for_any_pillar() {
        let mut item = item();
        item.updated_at = Utc::now() - Duration::hours(DEFAULT_STALE_AFTER_HOURS + 1);

        let report = evaluate_token_usage_inventory(&[item], Pillar::Cost, Utc::now());

        assert_eq!(report.stale_resources, 1);
        assert!(codes(&report).contains(&REASON_INV_STALE_DATA.to_string()));
    }
}

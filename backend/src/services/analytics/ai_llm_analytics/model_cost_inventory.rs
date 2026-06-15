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

// Deterministic AI/LLM model cost inventory evaluator for roadmap rows
// 25-AI-LLM-OBSERVABILITY-00295/00302/00323.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::models::llm_model::Model as LlmProviderModel;
use crate::services::aws::inventory::types::{
    score_pillar, InventoryFinding, Pillar, PillarReport, Severity, COST_ALLOCATION_TAG_KEYS,
    DEFAULT_STALE_AFTER_HOURS,
};

pub const RESOURCE_TYPE: &str = "AiLlmModelCost";
pub const REASON_COST_OWNER_NOT_RECORDED: &str = "AI_LLM_MODEL_COST_OWNER_NOT_RECORDED";
pub const REASON_COST_PRICE_MISSING: &str = "AI_LLM_MODEL_COST_PRICE_MISSING";
pub const REASON_COST_BUDGET_MISSING: &str = "AI_LLM_MODEL_COST_BUDGET_MISSING";
pub const REASON_COST_ESTIMATE_OVER_BUDGET: &str = "AI_LLM_MODEL_COST_ESTIMATE_OVER_BUDGET";
pub const REASON_RES_USAGE_SAMPLE_MISSING: &str = "AI_LLM_MODEL_COST_RES_USAGE_SAMPLE_MISSING";
pub const REASON_RES_COST_GUARDRAIL_MISSING: &str = "AI_LLM_MODEL_COST_RES_GUARDRAIL_MISSING";
pub const REASON_RES_FALLBACK_MISSING: &str = "AI_LLM_MODEL_COST_RES_FALLBACK_MISSING";
pub const REASON_SEC_AUDIT_MISSING: &str = "AI_LLM_MODEL_COST_SEC_AUDIT_MISSING";
pub const REASON_SEC_REDACTION_MISSING: &str = "AI_LLM_MODEL_COST_SEC_REDACTION_MISSING";
pub const REASON_INV_STALE_DATA: &str = "AI_LLM_MODEL_COST_INV_STALE_DATA";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelCostInventoryItem {
    pub model_id: String,
    pub provider_id: String,
    pub model_name: String,
    pub enabled: bool,
    pub owner: Option<String>,
    pub labels: Vec<String>,
    pub input_token_price_usd: Option<f64>,
    pub output_token_price_usd: Option<f64>,
    pub prompt_tokens: Option<u64>,
    pub completion_tokens: Option<u64>,
    pub total_tokens: Option<u64>,
    pub estimated_monthly_cost_usd: Option<f64>,
    pub monthly_budget_usd: Option<f64>,
    pub cost_guardrail: Option<String>,
    pub fallback_model: Option<String>,
    pub audit_enabled: bool,
    pub redaction_policy: Option<String>,
    pub updated_at: DateTime<Utc>,
}

pub fn evaluate_model_cost_inventory(
    items: &[ModelCostInventoryItem],
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

pub fn model_cost_inventory_item_from_model(model: &LlmProviderModel) -> ModelCostInventoryItem {
    let prompt_tokens = integer_field(&model.model_config, &["prompt_tokens", "input_tokens"]);
    let completion_tokens =
        integer_field(&model.model_config, &["completion_tokens", "output_tokens"]);
    let total_tokens = integer_field(&model.model_config, &["total_tokens", "token_count"])
        .or_else(|| {
            prompt_tokens
                .zip(completion_tokens)
                .map(|(prompt, completion)| prompt + completion)
        });
    let input_token_price_usd = number_field(
        &model.model_config,
        &[
            "input_token_price_usd",
            "prompt_token_price_usd",
            "input_cost_per_token_usd",
        ],
    );
    let output_token_price_usd = number_field(
        &model.model_config,
        &[
            "output_token_price_usd",
            "completion_token_price_usd",
            "output_cost_per_token_usd",
        ],
    );
    let estimated_monthly_cost_usd = number_field(
        &model.model_config,
        &["estimated_monthly_cost_usd", "monthly_cost_usd"],
    )
    .or_else(|| {
        prompt_tokens
            .zip(input_token_price_usd)
            .zip(completion_tokens.zip(output_token_price_usd))
            .map(|((prompt, input_price), (completion, output_price))| {
                prompt as f64 * input_price + completion as f64 * output_price
            })
    });

    ModelCostInventoryItem {
        model_id: model.id.to_string(),
        provider_id: model.provider_id.to_string(),
        model_name: model.model_name.clone(),
        enabled: model.enabled,
        owner: string_field(&model.model_config, &["owner", "team", "cost_owner"]),
        labels: string_array_field(&model.model_config, "labels"),
        input_token_price_usd,
        output_token_price_usd,
        prompt_tokens,
        completion_tokens,
        total_tokens,
        estimated_monthly_cost_usd,
        monthly_budget_usd: number_field(
            &model.model_config,
            &["monthly_budget_usd", "budget_usd", "cost_budget_usd"],
        ),
        cost_guardrail: string_field(
            &model.model_config,
            &[
                "cost_guardrail",
                "routing_cost_guardrail",
                "spend_guardrail",
            ],
        ),
        fallback_model: string_field(
            &model.model_config,
            &[
                "fallback_model",
                "fallback_model_id",
                "cheaper_fallback_model",
            ],
        ),
        audit_enabled: bool_field(
            &model.model_config,
            &["audit_enabled", "cost_audit_enabled", "usage_audit_enabled"],
        ),
        redaction_policy: string_field(
            &model.model_config,
            &[
                "cost_redaction_policy",
                "telemetry_redaction_policy",
                "redaction_policy",
                "data_policy",
            ],
        ),
        updated_at: model.updated_at,
    }
}

fn evaluate_cost(
    item: &ModelCostInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if !has_owner_metadata(item) {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_OWNER_NOT_RECORDED,
            Severity::Medium,
            format!("AI/LLM model cost {} has no owner metadata", item.model_name),
            json!({"model_id": item.model_id, "labels": item.labels, "checked_keys": COST_ALLOCATION_TAG_KEYS}),
        ));
    }

    if item.input_token_price_usd.is_none() || item.output_token_price_usd.is_none() {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_PRICE_MISSING,
            Severity::High,
            format!(
                "AI/LLM model cost {} is missing token price metadata",
                item.model_name
            ),
            json!({"model_id": item.model_id, "input_token_price_usd": item.input_token_price_usd, "output_token_price_usd": item.output_token_price_usd}),
        ));
    }

    if item.monthly_budget_usd.is_none() {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_BUDGET_MISSING,
            Severity::Medium,
            format!(
                "AI/LLM model cost {} has no monthly budget",
                item.model_name
            ),
            json!({"model_id": item.model_id, "recommendation": "Record a monthly model cost budget before routing spend posture is trusted"}),
        ));
    }

    if item
        .estimated_monthly_cost_usd
        .zip(item.monthly_budget_usd)
        .map(|(estimate, budget)| estimate > budget)
        .unwrap_or(false)
    {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_ESTIMATE_OVER_BUDGET,
            Severity::High,
            format!(
                "AI/LLM model cost {} is over its monthly budget",
                item.model_name
            ),
            json!({"model_id": item.model_id, "estimated_monthly_cost_usd": item.estimated_monthly_cost_usd, "monthly_budget_usd": item.monthly_budget_usd, "recommendation": "Review model routing, cache hits, prompt size, and lower-cost fallback opportunities"}),
        ));
    }
}

fn evaluate_resilience(
    item: &ModelCostInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if item.total_tokens.unwrap_or(0) == 0 && item.estimated_monthly_cost_usd.is_none() {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_USAGE_SAMPLE_MISSING,
            Severity::High,
            format!(
                "AI/LLM model cost {} has no usable usage or cost sample",
                item.model_name
            ),
            json!({"model_id": item.model_id, "prompt_tokens": item.prompt_tokens, "completion_tokens": item.completion_tokens, "total_tokens": item.total_tokens, "estimated_monthly_cost_usd": item.estimated_monthly_cost_usd}),
        ));
    }

    if item
        .cost_guardrail
        .as_deref()
        .unwrap_or_default()
        .trim()
        .is_empty()
        && item.monthly_budget_usd.is_none()
    {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_COST_GUARDRAIL_MISSING,
            Severity::High,
            format!(
                "AI/LLM model cost {} has no spend guardrail",
                item.model_name
            ),
            json!({"model_id": item.model_id, "recommendation": "Declare cost guardrails or budget caps so retries and failovers do not create hidden spend regressions"}),
        ));
    }

    if item
        .fallback_model
        .as_deref()
        .unwrap_or_default()
        .trim()
        .is_empty()
    {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_FALLBACK_MISSING,
            Severity::Medium,
            format!(
                "AI/LLM model cost {} has no fallback model",
                item.model_name
            ),
            json!({"model_id": item.model_id, "recommendation": "Record a fallback route so cost-aware recovery has a deterministic alternative"}),
        ));
    }
}

fn evaluate_security(
    item: &ModelCostInventoryItem,
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
                "AI/LLM model cost {} has no cost audit trail",
                item.model_name
            ),
            json!({"model_id": item.model_id, "recommendation": "Enable replayable model cost and usage audit evidence before agentic routing decisions use this model"}),
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
                "AI/LLM model cost {} has no telemetry redaction policy",
                item.model_name
            ),
            json!({"model_id": item.model_id}),
        ));
    }
}

fn stale_finding(
    item: &ModelCostInventoryItem,
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
            "AI/LLM model cost {} inventory data is stale by {} hours",
            item.model_name, age_hours
        ),
        json!({"model_id": item.model_id, "age_hours": age_hours, "stale_after_hours": DEFAULT_STALE_AFTER_HOURS}),
    ))
}

fn finding(
    item: &ModelCostInventoryItem,
    pillar: Pillar,
    reason_code: &str,
    severity: Severity,
    message: String,
    evidence: Value,
) -> InventoryFinding {
    InventoryFinding {
        resource_id: item.model_id.clone(),
        arn: format!("ai-llm-model-cost/{}", item.model_id),
        pillar,
        reason_code: reason_code.to_string(),
        severity,
        message,
        evidence,
    }
}

fn has_owner_metadata(item: &ModelCostInventoryItem) -> bool {
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

    fn item() -> ModelCostInventoryItem {
        ModelCostInventoryItem {
            model_id: "model-1".to_string(),
            provider_id: "provider-1".to_string(),
            model_name: "deepseek-chat".to_string(),
            enabled: true,
            owner: Some("sre-ai".to_string()),
            labels: vec!["cost-center=ai-platform".to_string()],
            input_token_price_usd: Some(0.000001),
            output_token_price_usd: Some(0.000002),
            prompt_tokens: Some(40_000),
            completion_tokens: Some(20_000),
            total_tokens: Some(60_000),
            estimated_monthly_cost_usd: Some(80.0),
            monthly_budget_usd: Some(150.0),
            cost_guardrail: Some("route high-volume traffic through cached model".to_string()),
            fallback_model: Some("deepseek-reasoner".to_string()),
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
    fn healthy_model_cost_inventory_passes_claimed_pillars() {
        let now = Utc::now();
        for pillar in [Pillar::Cost, Pillar::Resilience, Pillar::Security] {
            let report = evaluate_model_cost_inventory(&[item()], pillar, now);
            assert_eq!(report.resources_evaluated, 1);
            assert!(report.findings.is_empty());
        }
    }

    #[test]
    fn cost_flags_missing_owner_prices_budget_and_overage() {
        let mut item = item();
        item.owner = None;
        item.labels.clear();
        item.input_token_price_usd = None;
        item.monthly_budget_usd = Some(50.0);
        item.estimated_monthly_cost_usd = Some(120.0);

        let report = evaluate_model_cost_inventory(&[item], Pillar::Cost, Utc::now());
        let codes = codes(&report);

        assert!(codes.contains(&REASON_COST_OWNER_NOT_RECORDED.to_string()));
        assert!(codes.contains(&REASON_COST_PRICE_MISSING.to_string()));
        assert!(codes.contains(&REASON_COST_ESTIMATE_OVER_BUDGET.to_string()));
    }

    #[test]
    fn resilience_flags_missing_sample_guardrail_and_fallback() {
        let mut item = item();
        item.prompt_tokens = None;
        item.completion_tokens = None;
        item.total_tokens = None;
        item.estimated_monthly_cost_usd = None;
        item.monthly_budget_usd = None;
        item.cost_guardrail = None;
        item.fallback_model = None;

        let report = evaluate_model_cost_inventory(&[item], Pillar::Resilience, Utc::now());
        let codes = codes(&report);

        assert!(codes.contains(&REASON_RES_USAGE_SAMPLE_MISSING.to_string()));
        assert!(codes.contains(&REASON_RES_COST_GUARDRAIL_MISSING.to_string()));
        assert!(codes.contains(&REASON_RES_FALLBACK_MISSING.to_string()));
    }

    #[test]
    fn security_flags_missing_audit_and_redaction() {
        let mut item = item();
        item.audit_enabled = false;
        item.redaction_policy = None;

        let report = evaluate_model_cost_inventory(&[item], Pillar::Security, Utc::now());
        let codes = codes(&report);

        assert!(codes.contains(&REASON_SEC_AUDIT_MISSING.to_string()));
        assert!(codes.contains(&REASON_SEC_REDACTION_MISSING.to_string()));
    }

    #[test]
    fn stale_model_cost_inventory_is_counted_for_any_pillar() {
        let mut item = item();
        item.updated_at = Utc::now() - Duration::hours(DEFAULT_STALE_AFTER_HOURS + 1);

        let report = evaluate_model_cost_inventory(&[item], Pillar::Cost, Utc::now());

        assert_eq!(report.stale_resources, 1);
        assert!(codes(&report).contains(&REASON_INV_STALE_DATA.to_string()));
    }
}

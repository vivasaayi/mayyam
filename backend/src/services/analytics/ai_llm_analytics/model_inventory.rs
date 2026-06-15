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

// Deterministic AI/LLM model inventory evaluator for roadmap rows
// 25-AI-LLM-OBSERVABILITY-00001/00008/00029.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::models::llm_model::Model as LlmProviderModel;
use crate::services::aws::inventory::types::{
    score_pillar, InventoryFinding, Pillar, PillarReport, Severity, COST_ALLOCATION_TAG_KEYS,
    DEFAULT_STALE_AFTER_HOURS,
};

pub const RESOURCE_TYPE: &str = "AiLlmModel";
pub const REASON_COST_OWNER_NOT_RECORDED: &str = "AI_LLM_MODEL_COST_OWNER_NOT_RECORDED";
pub const REASON_COST_TOKEN_PRICE_MISSING: &str = "AI_LLM_MODEL_COST_TOKEN_PRICE_MISSING";
pub const REASON_COST_BUDGET_MISSING: &str = "AI_LLM_MODEL_COST_BUDGET_MISSING";
pub const REASON_RES_DISABLED: &str = "AI_LLM_MODEL_RES_DISABLED";
pub const REASON_RES_FALLBACK_MISSING: &str = "AI_LLM_MODEL_RES_FALLBACK_MISSING";
pub const REASON_RES_TIMEOUT_MISSING: &str = "AI_LLM_MODEL_RES_TIMEOUT_MISSING";
pub const REASON_SEC_APPROVAL_MISSING: &str = "AI_LLM_MODEL_SEC_APPROVAL_MISSING";
pub const REASON_SEC_DATA_POLICY_MISSING: &str = "AI_LLM_MODEL_SEC_DATA_POLICY_MISSING";
pub const REASON_SEC_AUDIT_MISSING: &str = "AI_LLM_MODEL_SEC_AUDIT_MISSING";
pub const REASON_INV_STALE_DATA: &str = "AI_LLM_MODEL_INV_STALE_DATA";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInventoryItem {
    pub model_id: String,
    pub provider_id: String,
    pub model_name: String,
    pub enabled: bool,
    pub owner: Option<String>,
    pub labels: Vec<String>,
    pub input_token_price_usd: Option<f64>,
    pub output_token_price_usd: Option<f64>,
    pub monthly_budget_usd: Option<f64>,
    pub fallback_model: Option<String>,
    pub timeout_ms: Option<u64>,
    pub approval_policy: Option<String>,
    pub data_policy: Option<String>,
    pub audit_enabled: bool,
    pub updated_at: DateTime<Utc>,
}

pub fn evaluate_model_inventory(
    items: &[ModelInventoryItem],
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

pub fn model_inventory_item_from_model(model: &LlmProviderModel) -> ModelInventoryItem {
    ModelInventoryItem {
        model_id: model.id.to_string(),
        provider_id: model.provider_id.to_string(),
        model_name: model.model_name.clone(),
        enabled: model.enabled,
        owner: string_field(&model.model_config, &["owner", "team", "cost_owner"]),
        labels: string_array_field(&model.model_config, "labels"),
        input_token_price_usd: number_field(
            &model.model_config,
            &["input_token_price_usd", "prompt_token_price_usd"],
        ),
        output_token_price_usd: number_field(
            &model.model_config,
            &["output_token_price_usd", "completion_token_price_usd"],
        ),
        monthly_budget_usd: number_field(
            &model.model_config,
            &["monthly_budget_usd", "budget_usd"],
        ),
        fallback_model: string_field(
            &model.model_config,
            &["fallback_model", "fallback_model_id"],
        ),
        timeout_ms: integer_field(&model.model_config, &["timeout_ms", "request_timeout_ms"]),
        approval_policy: string_field(
            &model.model_config,
            &["approval_policy", "approval_required"],
        ),
        data_policy: string_field(
            &model.model_config,
            &["data_policy", "sensitive_data_policy"],
        ),
        audit_enabled: bool_field(
            &model.model_config,
            &["audit_enabled", "tool_audit_enabled"],
        ),
        updated_at: model.updated_at,
    }
}

fn evaluate_cost(item: &ModelInventoryItem, pillar: Pillar, findings: &mut Vec<InventoryFinding>) {
    if !has_owner_metadata(item) {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_OWNER_NOT_RECORDED,
            Severity::Medium,
            format!(
                "AI/LLM model {} has no owner, team, or cost-owner metadata",
                item.model_name
            ),
            json!({
                "model_id": item.model_id,
                "provider_id": item.provider_id,
                "checked_keys": COST_ALLOCATION_TAG_KEYS,
                "labels": item.labels,
            }),
        ));
    }

    if item.input_token_price_usd.is_none() || item.output_token_price_usd.is_none() {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_TOKEN_PRICE_MISSING,
            Severity::High,
            format!(
                "AI/LLM model {} is missing token price metadata",
                item.model_name
            ),
            json!({
                "model_id": item.model_id,
                "input_token_price_usd": item.input_token_price_usd,
                "output_token_price_usd": item.output_token_price_usd,
                "recommendation": "Record input and output token prices before calculating AI spend and routing cost posture",
            }),
        ));
    }

    if item.monthly_budget_usd.is_none() {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_BUDGET_MISSING,
            Severity::Medium,
            format!(
                "AI/LLM model {} has no monthly budget guardrail",
                item.model_name
            ),
            json!({
                "model_id": item.model_id,
                "recommendation": "Attach a monthly budget or routing cap so token usage can produce actionable spend alerts",
            }),
        ));
    }
}

fn evaluate_resilience(
    item: &ModelInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if !item.enabled {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_DISABLED,
            Severity::Medium,
            format!("AI/LLM model {} is disabled", item.model_name),
            json!({
                "model_id": item.model_id,
                "recommendation": "Keep disabled models out of active routing or document their recovery role",
            }),
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
            Severity::High,
            format!(
                "AI/LLM model {} has no fallback model configured",
                item.model_name
            ),
            json!({
                "model_id": item.model_id,
                "recommendation": "Declare a fallback model or provider route for provider outage and throttling events",
            }),
        ));
    }

    if item.timeout_ms.is_none() {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_TIMEOUT_MISSING,
            Severity::Medium,
            format!("AI/LLM model {} has no timeout metadata", item.model_name),
            json!({
                "model_id": item.model_id,
                "recommendation": "Record request timeout policy so agent stop conditions are deterministic",
            }),
        ));
    }
}

fn evaluate_security(
    item: &ModelInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if item
        .approval_policy
        .as_deref()
        .unwrap_or_default()
        .trim()
        .is_empty()
    {
        findings.push(finding(
            item,
            pillar,
            REASON_SEC_APPROVAL_MISSING,
            Severity::High,
            format!(
                "AI/LLM model {} has no approval policy metadata",
                item.model_name
            ),
            json!({
                "model_id": item.model_id,
                "recommendation": "Record approval policy for tool-using or mutation-capable model routes",
            }),
        ));
    }

    if item
        .data_policy
        .as_deref()
        .unwrap_or_default()
        .trim()
        .is_empty()
    {
        findings.push(finding(
            item,
            pillar,
            REASON_SEC_DATA_POLICY_MISSING,
            Severity::High,
            format!(
                "AI/LLM model {} has no sensitive-data policy metadata",
                item.model_name
            ),
            json!({
                "model_id": item.model_id,
                "recommendation": "Record sensitive-data handling, redaction, and retention policy for model calls",
            }),
        ));
    }

    if !item.audit_enabled {
        findings.push(finding(
            item,
            pillar,
            REASON_SEC_AUDIT_MISSING,
            Severity::Medium,
            format!("AI/LLM model {} has no audit evidence", item.model_name),
            json!({
                "model_id": item.model_id,
                "recommendation": "Enable request and tool-call audit evidence before the model route is used by agents",
            }),
        ));
    }
}

fn stale_finding(
    item: &ModelInventoryItem,
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
            "AI/LLM model {} inventory data is stale by {} hours",
            item.model_name, age_hours
        ),
        json!({
            "model_id": item.model_id,
            "age_hours": age_hours,
            "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
        }),
    ))
}

fn finding(
    item: &ModelInventoryItem,
    pillar: Pillar,
    reason_code: &str,
    severity: Severity,
    message: String,
    evidence: Value,
) -> InventoryFinding {
    InventoryFinding {
        resource_id: item.model_id.clone(),
        arn: format!("ai-llm-model/{}", item.model_id),
        pillar,
        reason_code: reason_code.to_string(),
        severity,
        message,
        evidence,
    }
}

fn has_owner_metadata(item: &ModelInventoryItem) -> bool {
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

fn string_field(config: &Value, keys: &[&str]) -> Option<String> {
    keys.iter().find_map(|key| {
        config
            .get(*key)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToString::to_string)
    })
}

fn number_field(config: &Value, keys: &[&str]) -> Option<f64> {
    keys.iter()
        .find_map(|key| config.get(*key).and_then(Value::as_f64))
}

fn integer_field(config: &Value, keys: &[&str]) -> Option<u64> {
    keys.iter()
        .find_map(|key| config.get(*key).and_then(Value::as_u64))
}

fn bool_field(config: &Value, keys: &[&str]) -> bool {
    keys.iter()
        .find_map(|key| config.get(*key).and_then(Value::as_bool))
        .unwrap_or(false)
}

fn string_array_field(config: &Value, key: &str) -> Vec<String> {
    config
        .get(key)
        .and_then(Value::as_array)
        .map(|values| {
            values
                .iter()
                .filter_map(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToString::to_string)
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;
    use uuid::Uuid;

    fn item() -> ModelInventoryItem {
        ModelInventoryItem {
            model_id: "model-1".to_string(),
            provider_id: "provider-1".to_string(),
            model_name: "gpt-ops".to_string(),
            enabled: true,
            owner: Some("sre-ai".to_string()),
            labels: vec!["cost-center=ai-platform".to_string()],
            input_token_price_usd: Some(0.000001),
            output_token_price_usd: Some(0.000002),
            monthly_budget_usd: Some(500.0),
            fallback_model: Some("gpt-ops-fallback".to_string()),
            timeout_ms: Some(30_000),
            approval_policy: Some("approval-required-for-mutations".to_string()),
            data_policy: Some("redact-sensitive-data".to_string()),
            audit_enabled: true,
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
    fn healthy_model_inventory_passes_claimed_pillars() {
        let now = Utc::now();
        for pillar in [Pillar::Cost, Pillar::Resilience, Pillar::Security] {
            let report = evaluate_model_inventory(&[item()], pillar, now);
            assert_eq!(report.resources_evaluated, 1);
            assert!(report.findings.is_empty());
        }
    }

    #[test]
    fn cost_flags_missing_owner_prices_and_budget() {
        let mut item = item();
        item.owner = None;
        item.labels.clear();
        item.input_token_price_usd = None;
        item.monthly_budget_usd = None;

        let report = evaluate_model_inventory(&[item], Pillar::Cost, Utc::now());
        let codes = codes(&report);

        assert!(codes.contains(&REASON_COST_OWNER_NOT_RECORDED.to_string()));
        assert!(codes.contains(&REASON_COST_TOKEN_PRICE_MISSING.to_string()));
        assert!(codes.contains(&REASON_COST_BUDGET_MISSING.to_string()));
    }

    #[test]
    fn resilience_flags_disabled_model_without_fallback_or_timeout() {
        let mut item = item();
        item.enabled = false;
        item.fallback_model = None;
        item.timeout_ms = None;

        let report = evaluate_model_inventory(&[item], Pillar::Resilience, Utc::now());
        let codes = codes(&report);

        assert!(codes.contains(&REASON_RES_DISABLED.to_string()));
        assert!(codes.contains(&REASON_RES_FALLBACK_MISSING.to_string()));
        assert!(codes.contains(&REASON_RES_TIMEOUT_MISSING.to_string()));
    }

    #[test]
    fn security_flags_missing_policy_and_audit_evidence() {
        let mut item = item();
        item.approval_policy = None;
        item.data_policy = None;
        item.audit_enabled = false;

        let report = evaluate_model_inventory(&[item], Pillar::Security, Utc::now());
        let codes = codes(&report);

        assert!(codes.contains(&REASON_SEC_APPROVAL_MISSING.to_string()));
        assert!(codes.contains(&REASON_SEC_DATA_POLICY_MISSING.to_string()));
        assert!(codes.contains(&REASON_SEC_AUDIT_MISSING.to_string()));
    }

    #[test]
    fn model_config_maps_to_inventory_fields() {
        let model = LlmProviderModel {
            id: Uuid::new_v4(),
            provider_id: Uuid::new_v4(),
            model_name: "claude-route".to_string(),
            model_config: json!({
                "team": "ops-ai",
                "labels": ["env=prod", "cost-center=llm"],
                "input_token_price_usd": 0.000003,
                "completion_token_price_usd": 0.000015,
                "monthly_budget_usd": 1000.0,
                "fallback_model": "claude-fallback",
                "timeout_ms": 45000,
                "approval_policy": "human approval for write tools",
                "sensitive_data_policy": "redact secrets",
                "audit_enabled": true
            }),
            enabled: true,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        let item = model_inventory_item_from_model(&model);

        assert_eq!(item.owner.as_deref(), Some("ops-ai"));
        assert_eq!(item.output_token_price_usd, Some(0.000015));
        assert_eq!(item.timeout_ms, Some(45_000));
        assert!(item.audit_enabled);
    }

    #[test]
    fn stale_model_inventory_is_counted_for_any_pillar() {
        let mut item = item();
        item.updated_at = Utc::now() - Duration::hours(DEFAULT_STALE_AFTER_HOURS + 1);

        let report = evaluate_model_inventory(&[item], Pillar::Cost, Utc::now());

        assert_eq!(report.stale_resources, 1);
        assert!(codes(&report).contains(&REASON_INV_STALE_DATA.to_string()));
    }
}

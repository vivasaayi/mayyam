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

// Deterministic AI/LLM agent inventory evaluator for roadmap rows
// 25-AI-LLM-OBSERVABILITY-00099/00106/00127.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::models::llm_model::Model as LlmProviderModel;
use crate::models::prompt_template::Model as PromptTemplateModel;
use crate::services::aws::inventory::types::{
    score_pillar, InventoryFinding, Pillar, PillarReport, Severity, COST_ALLOCATION_TAG_KEYS,
    DEFAULT_STALE_AFTER_HOURS,
};

pub const RESOURCE_TYPE: &str = "AiLlmAgent";
pub const REASON_COST_OWNER_NOT_RECORDED: &str = "AI_LLM_AGENT_COST_OWNER_NOT_RECORDED";
pub const REASON_COST_BUDGET_MISSING: &str = "AI_LLM_AGENT_COST_BUDGET_MISSING";
pub const REASON_COST_TOKEN_LIMIT_MISSING: &str = "AI_LLM_AGENT_COST_TOKEN_LIMIT_MISSING";
pub const REASON_RES_DISABLED: &str = "AI_LLM_AGENT_RES_DISABLED";
pub const REASON_RES_PROMPT_MISSING: &str = "AI_LLM_AGENT_RES_PROMPT_MISSING";
pub const REASON_RES_STOP_CONDITION_MISSING: &str = "AI_LLM_AGENT_RES_STOP_CONDITION_MISSING";
pub const REASON_RES_TIMEOUT_MISSING: &str = "AI_LLM_AGENT_RES_TIMEOUT_MISSING";
pub const REASON_SEC_TOOL_POLICY_MISSING: &str = "AI_LLM_AGENT_SEC_TOOL_POLICY_MISSING";
pub const REASON_SEC_APPROVAL_MISSING: &str = "AI_LLM_AGENT_SEC_APPROVAL_MISSING";
pub const REASON_SEC_AUDIT_MISSING: &str = "AI_LLM_AGENT_SEC_AUDIT_MISSING";
pub const REASON_INV_STALE_DATA: &str = "AI_LLM_AGENT_INV_STALE_DATA";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentInventoryItem {
    pub agent_id: String,
    pub provider_id: String,
    pub model_name: String,
    pub enabled: bool,
    pub owner: Option<String>,
    pub labels: Vec<String>,
    pub prompt_count: usize,
    pub tool_count: usize,
    pub monthly_budget_usd: Option<f64>,
    pub max_tokens_per_run: Option<u64>,
    pub timeout_ms: Option<u64>,
    pub max_iterations: Option<u64>,
    pub stop_condition: Option<String>,
    pub tool_policy: Option<String>,
    pub approval_policy: Option<String>,
    pub audit_enabled: bool,
    pub updated_at: DateTime<Utc>,
}

pub fn evaluate_agent_inventory(
    items: &[AgentInventoryItem],
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

pub fn agent_inventory_items_from_models(
    models: &[LlmProviderModel],
    prompts: &[PromptTemplateModel],
) -> Vec<AgentInventoryItem> {
    let agent_prompt_count = prompts
        .iter()
        .filter(|prompt| is_agent_prompt(prompt))
        .count();
    models
        .iter()
        .map(|model| agent_inventory_item_from_model(model, agent_prompt_count))
        .collect()
}

fn agent_inventory_item_from_model(
    model: &LlmProviderModel,
    agent_prompt_count: usize,
) -> AgentInventoryItem {
    AgentInventoryItem {
        agent_id: model.id.to_string(),
        provider_id: model.provider_id.to_string(),
        model_name: model.model_name.clone(),
        enabled: model.enabled,
        owner: string_field(&model.model_config, &["owner", "team", "cost_owner"]),
        labels: string_array_field(&model.model_config, "labels"),
        prompt_count: integer_field(&model.model_config, &["prompt_count"])
            .map(|value| value as usize)
            .unwrap_or(agent_prompt_count),
        tool_count: integer_field(
            &model.model_config,
            &["tool_count", "registered_tool_count"],
        )
        .map(|value| value as usize)
        .or_else(|| array_len(&model.model_config, "allowed_tools"))
        .unwrap_or(0),
        monthly_budget_usd: number_field(
            &model.model_config,
            &[
                "agent_monthly_budget_usd",
                "monthly_budget_usd",
                "budget_usd",
            ],
        ),
        max_tokens_per_run: integer_field(
            &model.model_config,
            &["max_tokens_per_run", "token_budget", "max_tokens"],
        ),
        timeout_ms: integer_field(&model.model_config, &["timeout_ms", "request_timeout_ms"]),
        max_iterations: integer_field(&model.model_config, &["max_iterations", "max_steps"]),
        stop_condition: string_field(
            &model.model_config,
            &["stop_condition", "stop_conditions", "completion_policy"],
        ),
        tool_policy: string_field(
            &model.model_config,
            &["tool_policy", "read_only_policy", "tool_scope"],
        ),
        approval_policy: string_field(
            &model.model_config,
            &["approval_policy", "approval_required"],
        ),
        audit_enabled: bool_field(
            &model.model_config,
            &["audit_enabled", "tool_audit_enabled", "trace_enabled"],
        ),
        updated_at: model.updated_at,
    }
}

fn evaluate_cost(item: &AgentInventoryItem, pillar: Pillar, findings: &mut Vec<InventoryFinding>) {
    if !has_owner_metadata(item) {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_OWNER_NOT_RECORDED,
            Severity::Medium,
            format!("AI/LLM agent route {} has no owner metadata", item.model_name),
            json!({"agent_id": item.agent_id, "labels": item.labels, "checked_keys": COST_ALLOCATION_TAG_KEYS}),
        ));
    }

    if item.monthly_budget_usd.is_none() {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_BUDGET_MISSING,
            Severity::High,
            format!(
                "AI/LLM agent route {} has no monthly budget guardrail",
                item.model_name
            ),
            json!({"agent_id": item.agent_id, "recommendation": "Record a monthly budget for autonomous or assisted agent usage"}),
        ));
    }

    if item.max_tokens_per_run.is_none() {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_TOKEN_LIMIT_MISSING,
            Severity::High,
            format!(
                "AI/LLM agent route {} has no per-run token limit",
                item.model_name
            ),
            json!({"agent_id": item.agent_id, "recommendation": "Declare max tokens per run before enabling agentic investigation"}),
        ));
    }
}

fn evaluate_resilience(
    item: &AgentInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if !item.enabled {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_DISABLED,
            Severity::Medium,
            format!("AI/LLM agent route {} is disabled", item.model_name),
            json!({"agent_id": item.agent_id}),
        ));
    }

    if item.prompt_count == 0 {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_PROMPT_MISSING,
            Severity::High,
            format!(
                "AI/LLM agent route {} has no agent prompt template inventory",
                item.model_name
            ),
            json!({"agent_id": item.agent_id, "recommendation": "Register at least one active agent prompt template with evidence variables"}),
        ));
    }

    if item.timeout_ms.is_none() {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_TIMEOUT_MISSING,
            Severity::Medium,
            format!(
                "AI/LLM agent route {} has no timeout metadata",
                item.model_name
            ),
            json!({"agent_id": item.agent_id}),
        ));
    }

    if item.max_iterations.is_none()
        && item
            .stop_condition
            .as_deref()
            .unwrap_or_default()
            .trim()
            .is_empty()
    {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_STOP_CONDITION_MISSING,
            Severity::High,
            format!(
                "AI/LLM agent route {} has no deterministic stop condition",
                item.model_name
            ),
            json!({"agent_id": item.agent_id, "recommendation": "Record max iterations, stop condition, or completion policy for bounded investigation"}),
        ));
    }
}

fn evaluate_security(
    item: &AgentInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if item.tool_count == 0
        || item
            .tool_policy
            .as_deref()
            .unwrap_or_default()
            .trim()
            .is_empty()
    {
        findings.push(finding(
            item,
            pillar,
            REASON_SEC_TOOL_POLICY_MISSING,
            Severity::High,
            format!(
                "AI/LLM agent route {} has no registered tool policy",
                item.model_name
            ),
            json!({"agent_id": item.agent_id, "tool_count": item.tool_count, "recommendation": "Register scoped read-only tools before allowing agentic investigation"}),
        ));
    }

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
                "AI/LLM agent route {} has no approval policy",
                item.model_name
            ),
            json!({"agent_id": item.agent_id, "recommendation": "Require explicit approval before mutations or broad-scope diagnostics"}),
        ));
    }

    if !item.audit_enabled {
        findings.push(finding(
            item,
            pillar,
            REASON_SEC_AUDIT_MISSING,
            Severity::High,
            format!(
                "AI/LLM agent route {} has no replayable audit trace",
                item.model_name
            ),
            json!({"agent_id": item.agent_id, "recommendation": "Enable tool-call audit logging before production agent use"}),
        ));
    }
}

fn stale_finding(
    item: &AgentInventoryItem,
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
            "AI/LLM agent route {} inventory data is stale by {} hours",
            item.model_name, age_hours
        ),
        json!({"agent_id": item.agent_id, "age_hours": age_hours, "stale_after_hours": DEFAULT_STALE_AFTER_HOURS}),
    ))
}

fn finding(
    item: &AgentInventoryItem,
    pillar: Pillar,
    reason_code: &str,
    severity: Severity,
    message: String,
    evidence: Value,
) -> InventoryFinding {
    InventoryFinding {
        resource_id: item.agent_id.clone(),
        arn: format!("ai-llm-agent/{}", item.agent_id),
        pillar,
        reason_code: reason_code.to_string(),
        severity,
        message,
        evidence,
    }
}

fn has_owner_metadata(item: &AgentInventoryItem) -> bool {
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

fn is_agent_prompt(prompt: &PromptTemplateModel) -> bool {
    if !prompt.is_active {
        return false;
    }

    let tags = string_array_field(&prompt.tags, "");
    let searchable_text = format!(
        "{} {} {} {} {} {}",
        prompt.name,
        prompt.category,
        prompt.resource_type.clone().unwrap_or_default(),
        prompt.workflow_type.clone().unwrap_or_default(),
        prompt.prompt_template,
        tags.join(" ")
    );
    contains_any(
        &searchable_text,
        &["agent", "agentic", "tool", "diagnose", "investigation"],
    )
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
    let source = if key.is_empty() {
        value
    } else {
        value.get(key).unwrap_or(&Value::Null)
    };
    source
        .as_array()
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

fn array_len(value: &Value, key: &str) -> Option<usize> {
    value.get(key).and_then(Value::as_array).map(Vec::len)
}

fn contains_any(text: &str, needles: &[&str]) -> bool {
    let normalized = text.to_ascii_lowercase();
    needles.iter().any(|needle| normalized.contains(needle))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    fn item() -> AgentInventoryItem {
        AgentInventoryItem {
            agent_id: "agent-1".to_string(),
            provider_id: "provider-1".to_string(),
            model_name: "deepseek-agent".to_string(),
            enabled: true,
            owner: Some("sre-ai".to_string()),
            labels: vec!["cost-center=ai-platform".to_string()],
            prompt_count: 1,
            tool_count: 2,
            monthly_budget_usd: Some(1000.0),
            max_tokens_per_run: Some(8000),
            timeout_ms: Some(30000),
            max_iterations: Some(8),
            stop_condition: Some("stop after evidence-backed diagnosis".to_string()),
            tool_policy: Some("read-only diagnostics by default".to_string()),
            approval_policy: Some("approval required for mutations".to_string()),
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
    fn healthy_agent_inventory_passes_claimed_pillars() {
        let now = Utc::now();
        for pillar in [Pillar::Cost, Pillar::Resilience, Pillar::Security] {
            let report = evaluate_agent_inventory(&[item()], pillar, now);
            assert_eq!(report.resources_evaluated, 1);
            assert!(report.findings.is_empty());
        }
    }

    #[test]
    fn cost_flags_missing_owner_budget_and_token_limit() {
        let mut item = item();
        item.owner = None;
        item.labels.clear();
        item.monthly_budget_usd = None;
        item.max_tokens_per_run = None;

        let report = evaluate_agent_inventory(&[item], Pillar::Cost, Utc::now());
        let codes = codes(&report);

        assert!(codes.contains(&REASON_COST_OWNER_NOT_RECORDED.to_string()));
        assert!(codes.contains(&REASON_COST_BUDGET_MISSING.to_string()));
        assert!(codes.contains(&REASON_COST_TOKEN_LIMIT_MISSING.to_string()));
    }

    #[test]
    fn resilience_flags_disabled_route_without_prompt_or_stop_condition() {
        let mut item = item();
        item.enabled = false;
        item.prompt_count = 0;
        item.timeout_ms = None;
        item.max_iterations = None;
        item.stop_condition = None;

        let report = evaluate_agent_inventory(&[item], Pillar::Resilience, Utc::now());
        let codes = codes(&report);

        assert!(codes.contains(&REASON_RES_DISABLED.to_string()));
        assert!(codes.contains(&REASON_RES_PROMPT_MISSING.to_string()));
        assert!(codes.contains(&REASON_RES_TIMEOUT_MISSING.to_string()));
        assert!(codes.contains(&REASON_RES_STOP_CONDITION_MISSING.to_string()));
    }

    #[test]
    fn security_flags_missing_tool_policy_approval_and_audit() {
        let mut item = item();
        item.tool_count = 0;
        item.tool_policy = None;
        item.approval_policy = None;
        item.audit_enabled = false;

        let report = evaluate_agent_inventory(&[item], Pillar::Security, Utc::now());
        let codes = codes(&report);

        assert!(codes.contains(&REASON_SEC_TOOL_POLICY_MISSING.to_string()));
        assert!(codes.contains(&REASON_SEC_APPROVAL_MISSING.to_string()));
        assert!(codes.contains(&REASON_SEC_AUDIT_MISSING.to_string()));
    }

    #[test]
    fn stale_agent_inventory_is_counted_for_any_pillar() {
        let mut item = item();
        item.updated_at = Utc::now() - Duration::hours(DEFAULT_STALE_AFTER_HOURS + 1);

        let report = evaluate_agent_inventory(&[item], Pillar::Cost, Utc::now());

        assert_eq!(report.stale_resources, 1);
        assert!(codes(&report).contains(&REASON_INV_STALE_DATA.to_string()));
    }
}

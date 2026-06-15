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

// Deterministic AI/LLM prompt inventory evaluator for roadmap rows
// 25-AI-LLM-OBSERVABILITY-00050/00057/00078.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::models::prompt_template::Model as PromptTemplateModel;
use crate::services::aws::inventory::types::{
    score_pillar, InventoryFinding, Pillar, PillarReport, Severity, COST_ALLOCATION_TAG_KEYS,
    DEFAULT_STALE_AFTER_HOURS,
};

pub const RESOURCE_TYPE: &str = "AiLlmPromptTemplate";
pub const REASON_COST_OWNER_NOT_RECORDED: &str = "AI_LLM_PROMPT_COST_OWNER_NOT_RECORDED";
pub const REASON_COST_TOKEN_BUDGET_MISSING: &str = "AI_LLM_PROMPT_COST_TOKEN_BUDGET_MISSING";
pub const REASON_RES_VARIABLE_CONTRACT_MISSING: &str =
    "AI_LLM_PROMPT_RES_VARIABLE_CONTRACT_MISSING";
pub const REASON_RES_DESCRIPTION_MISSING: &str = "AI_LLM_PROMPT_RES_DESCRIPTION_MISSING";
pub const REASON_RES_INACTIVE: &str = "AI_LLM_PROMPT_RES_INACTIVE";
pub const REASON_SEC_GUARDRAILS_MISSING: &str = "AI_LLM_PROMPT_SEC_GUARDRAILS_MISSING";
pub const REASON_SEC_OWNER_NOT_RECORDED: &str = "AI_LLM_PROMPT_SEC_OWNER_NOT_RECORDED";
pub const REASON_SEC_SYSTEM_PROMPT_INACTIVE: &str = "AI_LLM_PROMPT_SEC_SYSTEM_PROMPT_INACTIVE";
pub const REASON_INV_STALE_DATA: &str = "AI_LLM_PROMPT_INV_STALE_DATA";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptInventoryItem {
    pub template_id: String,
    pub name: String,
    pub category: String,
    pub resource_type: Option<String>,
    pub workflow_type: Option<String>,
    pub version: String,
    pub owner: Option<String>,
    pub tags: Vec<String>,
    pub is_active: bool,
    pub is_system: bool,
    pub has_description: bool,
    pub variable_count: usize,
    pub prompt_bytes: usize,
    pub has_token_budget_control: bool,
    pub has_guardrails: bool,
    pub updated_at: DateTime<Utc>,
}

pub fn evaluate_prompt_inventory(
    items: &[PromptInventoryItem],
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

pub fn prompt_inventory_item_from_model(model: &PromptTemplateModel) -> PromptInventoryItem {
    let tags = parse_tags(&model.tags);
    let searchable_text = format!(
        "{} {} {} {} {}",
        model.name,
        model.category,
        model.resource_type.clone().unwrap_or_default(),
        model.workflow_type.clone().unwrap_or_default(),
        model.prompt_template
    );

    PromptInventoryItem {
        template_id: model.id.to_string(),
        name: model.name.clone(),
        category: model.category.clone(),
        resource_type: model.resource_type.clone(),
        workflow_type: model.workflow_type.clone(),
        version: model.version.clone(),
        owner: model.created_by.map(|id| id.to_string()),
        tags,
        is_active: model.is_active,
        is_system: model.is_system,
        has_description: model
            .description
            .as_deref()
            .map(|description| !description.trim().is_empty())
            .unwrap_or(false),
        variable_count: variable_count(&model.variables),
        prompt_bytes: model.prompt_template.len(),
        has_token_budget_control: contains_any(
            &searchable_text,
            &["token budget", "token_budget", "max_tokens", "cost limit"],
        ),
        has_guardrails: contains_any(
            &searchable_text,
            &[
                "evidence",
                "approval",
                "read-only",
                "permission",
                "audit",
                "redact",
            ],
        ),
        updated_at: model.updated_at,
    }
}

fn evaluate_cost(item: &PromptInventoryItem, pillar: Pillar, findings: &mut Vec<InventoryFinding>) {
    if !has_owner_metadata(item) {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_OWNER_NOT_RECORDED,
            Severity::Medium,
            format!("AI/LLM prompt template {} has no owner metadata", item.name),
            json!({"template_id": item.template_id, "tags": item.tags, "checked_keys": COST_ALLOCATION_TAG_KEYS}),
        ));
    }

    if !item.has_token_budget_control {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_TOKEN_BUDGET_MISSING,
            Severity::High,
            format!(
                "AI/LLM prompt template {} does not declare token budget controls",
                item.name
            ),
            json!({
                "template_id": item.template_id,
                "prompt_bytes": item.prompt_bytes,
                "recommendation": "Record token-budget or max-token expectations before using this prompt in AI spend reports",
            }),
        ));
    }
}

fn evaluate_resilience(
    item: &PromptInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if !item.is_active {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_INACTIVE,
            Severity::Medium,
            format!("AI/LLM prompt template {} is inactive", item.name),
            json!({"template_id": item.template_id}),
        ));
    }

    if item.variable_count == 0 {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_VARIABLE_CONTRACT_MISSING,
            Severity::High,
            format!("AI/LLM prompt template {} has no variable contract", item.name),
            json!({"template_id": item.template_id, "recommendation": "Define required evidence variables so prompt execution remains replayable"}),
        ));
    }

    if !item.has_description {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_DESCRIPTION_MISSING,
            Severity::Medium,
            format!("AI/LLM prompt template {} has no description", item.name),
            json!({"template_id": item.template_id}),
        ));
    }
}

fn evaluate_security(
    item: &PromptInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if !has_owner_metadata(item) {
        findings.push(finding(
            item,
            pillar,
            REASON_SEC_OWNER_NOT_RECORDED,
            Severity::Medium,
            format!(
                "AI/LLM prompt template {} has no owner metadata for security review",
                item.name
            ),
            json!({"template_id": item.template_id, "tags": item.tags}),
        ));
    }

    if !item.has_guardrails {
        findings.push(finding(
            item,
            pillar,
            REASON_SEC_GUARDRAILS_MISSING,
            Severity::High,
            format!(
                "AI/LLM prompt template {} does not include evidence, approval, audit, or read-only guardrails",
                item.name
            ),
            json!({"template_id": item.template_id}),
        ));
    }

    if item.is_system && !item.is_active {
        findings.push(finding(
            item,
            pillar,
            REASON_SEC_SYSTEM_PROMPT_INACTIVE,
            Severity::Medium,
            format!("System prompt template {} is inactive", item.name),
            json!({"template_id": item.template_id}),
        ));
    }
}

fn stale_finding(
    item: &PromptInventoryItem,
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
            "AI/LLM prompt template {} inventory data is stale by {} hours",
            item.name, age_hours
        ),
        json!({"template_id": item.template_id, "age_hours": age_hours, "stale_after_hours": DEFAULT_STALE_AFTER_HOURS}),
    ))
}

fn finding(
    item: &PromptInventoryItem,
    pillar: Pillar,
    reason_code: &str,
    severity: Severity,
    message: String,
    evidence: Value,
) -> InventoryFinding {
    InventoryFinding {
        resource_id: item.template_id.clone(),
        arn: format!("ai-llm-prompt-template/{}", item.template_id),
        pillar,
        reason_code: reason_code.to_string(),
        severity,
        message,
        evidence,
    }
}

fn has_owner_metadata(item: &PromptInventoryItem) -> bool {
    item.owner
        .as_deref()
        .map(|owner| !owner.trim().is_empty())
        .unwrap_or(false)
        || item.tags.iter().any(|tag| {
            let normalized = tag.to_ascii_lowercase();
            COST_ALLOCATION_TAG_KEYS
                .iter()
                .any(|key| normalized.starts_with(&format!("{key}=")))
        })
}

fn parse_tags(value: &Value) -> Vec<String> {
    value
        .as_array()
        .map(|tags| {
            tags.iter()
                .filter_map(Value::as_str)
                .map(str::trim)
                .filter(|tag| !tag.is_empty())
                .map(ToString::to_string)
                .collect()
        })
        .unwrap_or_default()
}

fn variable_count(value: &Value) -> usize {
    value.as_array().map(Vec::len).unwrap_or(0)
}

fn contains_any(text: &str, needles: &[&str]) -> bool {
    let normalized = text.to_ascii_lowercase();
    needles.iter().any(|needle| normalized.contains(needle))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    fn item() -> PromptInventoryItem {
        PromptInventoryItem {
            template_id: "prompt-1".to_string(),
            name: "incident-summary".to_string(),
            category: "Operations".to_string(),
            resource_type: Some("AI".to_string()),
            workflow_type: Some("Triage".to_string()),
            version: "1.0".to_string(),
            owner: Some("sre-ai".to_string()),
            tags: vec!["cost-center=ai-platform".to_string()],
            is_active: true,
            is_system: true,
            has_description: true,
            variable_count: 2,
            prompt_bytes: 240,
            has_token_budget_control: true,
            has_guardrails: true,
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
    fn healthy_prompt_inventory_passes_claimed_pillars() {
        let now = Utc::now();
        for pillar in [Pillar::Cost, Pillar::Resilience, Pillar::Security] {
            let report = evaluate_prompt_inventory(&[item()], pillar, now);
            assert_eq!(report.resources_evaluated, 1);
            assert!(report.findings.is_empty());
        }
    }

    #[test]
    fn cost_flags_missing_owner_and_token_budget() {
        let mut item = item();
        item.owner = None;
        item.tags.clear();
        item.has_token_budget_control = false;

        let report = evaluate_prompt_inventory(&[item], Pillar::Cost, Utc::now());
        let codes = codes(&report);

        assert!(codes.contains(&REASON_COST_OWNER_NOT_RECORDED.to_string()));
        assert!(codes.contains(&REASON_COST_TOKEN_BUDGET_MISSING.to_string()));
    }

    #[test]
    fn resilience_flags_inactive_prompt_without_contract_or_description() {
        let mut item = item();
        item.is_active = false;
        item.variable_count = 0;
        item.has_description = false;

        let report = evaluate_prompt_inventory(&[item], Pillar::Resilience, Utc::now());
        let codes = codes(&report);

        assert!(codes.contains(&REASON_RES_INACTIVE.to_string()));
        assert!(codes.contains(&REASON_RES_VARIABLE_CONTRACT_MISSING.to_string()));
        assert!(codes.contains(&REASON_RES_DESCRIPTION_MISSING.to_string()));
    }

    #[test]
    fn security_flags_missing_owner_guardrails_and_inactive_system_prompt() {
        let mut item = item();
        item.owner = None;
        item.tags.clear();
        item.has_guardrails = false;
        item.is_active = false;

        let report = evaluate_prompt_inventory(&[item], Pillar::Security, Utc::now());
        let codes = codes(&report);

        assert!(codes.contains(&REASON_SEC_OWNER_NOT_RECORDED.to_string()));
        assert!(codes.contains(&REASON_SEC_GUARDRAILS_MISSING.to_string()));
        assert!(codes.contains(&REASON_SEC_SYSTEM_PROMPT_INACTIVE.to_string()));
    }

    #[test]
    fn stale_prompt_inventory_is_counted_for_any_pillar() {
        let mut item = item();
        item.updated_at = Utc::now() - Duration::hours(DEFAULT_STALE_AFTER_HOURS + 1);

        let report = evaluate_prompt_inventory(&[item], Pillar::Cost, Utc::now());

        assert_eq!(report.stale_resources, 1);
        assert!(codes(&report).contains(&REASON_INV_STALE_DATA.to_string()));
    }
}

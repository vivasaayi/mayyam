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

// Deterministic AI prompt-template inventory evaluator for roadmap rows
// 03-MYSQL-AI-TRIAGER-01569/01576/01597.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::models::prompt_template::Model as PromptTemplateModel;
use crate::services::aws::inventory::types::{
    score_pillar, InventoryFinding, Pillar, PillarReport, Severity, DEFAULT_STALE_AFTER_HOURS,
};

pub const RESOURCE_TYPE: &str = "MySqlAiPromptTemplate";
pub const REASON_COST_OWNER_NOT_RECORDED: &str = "MYSQL_AI_PROMPT_TEMPLATE_COST_OWNER_NOT_RECORDED";
pub const REASON_COST_TOKEN_BUDGET_MISSING: &str =
    "MYSQL_AI_PROMPT_TEMPLATE_COST_TOKEN_BUDGET_MISSING";
pub const REASON_RES_CONTEXT_CONTRACT_MISSING: &str =
    "MYSQL_AI_PROMPT_TEMPLATE_RES_CONTEXT_CONTRACT_MISSING";
pub const REASON_RES_DESCRIPTION_MISSING: &str = "MYSQL_AI_PROMPT_TEMPLATE_RES_DESCRIPTION_MISSING";
pub const REASON_SEC_OWNER_NOT_RECORDED: &str = "MYSQL_AI_PROMPT_TEMPLATE_SEC_OWNER_NOT_RECORDED";
pub const REASON_SEC_GUARDRAILS_MISSING: &str = "MYSQL_AI_PROMPT_TEMPLATE_SEC_GUARDRAILS_MISSING";
pub const REASON_SEC_SYSTEM_PROMPT_INACTIVE: &str =
    "MYSQL_AI_PROMPT_TEMPLATE_SEC_SYSTEM_PROMPT_INACTIVE";
pub const REASON_INV_STALE_DATA: &str = "MYSQL_AI_PROMPT_TEMPLATE_INV_STALE_DATA";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiPromptTemplateInventoryItem {
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

pub fn evaluate_mysql_ai_prompt_templates_inventory(
    items: &[AiPromptTemplateInventoryItem],
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

pub fn ai_prompt_template_item_from_model(
    model: &PromptTemplateModel,
) -> Option<AiPromptTemplateInventoryItem> {
    if !model.is_active || !is_mysql_scoped(model) {
        return None;
    }

    let tags = parse_tags(&model.tags);
    let searchable_text = format!(
        "{} {} {} {} {}",
        model.name,
        model.category,
        model.resource_type.clone().unwrap_or_default(),
        model.workflow_type.clone().unwrap_or_default(),
        model.prompt_template
    );

    Some(AiPromptTemplateInventoryItem {
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
                "do not",
            ],
        ),
        updated_at: model.updated_at,
    })
}

fn evaluate_cost(
    item: &AiPromptTemplateInventoryItem,
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
                "AI prompt template {} has no owner or team metadata for cost accountability",
                item.name
            ),
            json!({
                "template_id": item.template_id,
                "tags": item.tags,
            }),
        ));
    }

    if !item.has_token_budget_control {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_TOKEN_BUDGET_MISSING,
            Severity::High,
            format!(
                "AI prompt template {} does not declare token budget or max-token controls",
                item.name
            ),
            json!({
                "template_id": item.template_id,
                "prompt_bytes": item.prompt_bytes,
                "recommendation": "Record token-budget expectations so MySQL triage prompts have deterministic cost controls",
            }),
        ));
    }
}

fn evaluate_resilience(
    item: &AiPromptTemplateInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if item.variable_count == 0 {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_CONTEXT_CONTRACT_MISSING,
            Severity::High,
            format!(
                "AI prompt template {} has no variable contract for required MySQL evidence",
                item.name
            ),
            json!({
                "template_id": item.template_id,
                "recommendation": "Define required evidence variables so triage remains replayable when telemetry changes",
            }),
        ));
    }

    if !item.has_description {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_DESCRIPTION_MISSING,
            Severity::Medium,
            format!(
                "AI prompt template {} has no operator-facing description",
                item.name
            ),
            json!({
                "template_id": item.template_id,
                "recommendation": "Describe the prompt purpose, expected inputs, and fallback behavior before relying on it during incidents",
            }),
        ));
    }
}

fn evaluate_security(
    item: &AiPromptTemplateInventoryItem,
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
                "AI prompt template {} has no owner or team metadata for security review",
                item.name
            ),
            json!({
                "template_id": item.template_id,
                "tags": item.tags,
            }),
        ));
    }

    if !item.has_guardrails {
        findings.push(finding(
            item,
            pillar,
            REASON_SEC_GUARDRAILS_MISSING,
            Severity::High,
            format!(
                "AI prompt template {} does not include evidence, approval, audit, or read-only guardrails",
                item.name
            ),
            json!({
                "template_id": item.template_id,
                "recommendation": "Require evidence-grounded answers, explicit approval for mutations, and auditable tool-call boundaries",
            }),
        ));
    }

    if item.is_system && !item.is_active {
        findings.push(finding(
            item,
            pillar,
            REASON_SEC_SYSTEM_PROMPT_INACTIVE,
            Severity::High,
            format!("System AI prompt template {} is inactive", item.name),
            json!({
                "template_id": item.template_id,
                "version": item.version,
            }),
        ));
    }
}

fn stale_finding(
    item: &AiPromptTemplateInventoryItem,
    pillar: Pillar,
    now: DateTime<Utc>,
) -> Option<InventoryFinding> {
    let age_hours = (now - item.updated_at).num_hours();
    if age_hours <= DEFAULT_STALE_AFTER_HOURS {
        return None;
    }

    Some(finding(
        item,
        pillar,
        REASON_INV_STALE_DATA,
        Severity::Medium,
        format!(
            "AI prompt template {} was last updated {} hours ago (threshold {} hours)",
            item.name, age_hours, DEFAULT_STALE_AFTER_HOURS
        ),
        json!({
            "updated_at": item.updated_at,
            "age_hours": age_hours,
            "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
        }),
    ))
}

fn finding(
    item: &AiPromptTemplateInventoryItem,
    pillar: Pillar,
    reason_code: &str,
    severity: Severity,
    message: String,
    evidence: Value,
) -> InventoryFinding {
    InventoryFinding {
        resource_id: item.template_id.clone(),
        arn: format!("mysql-ai-prompt-template:{}", item.template_id),
        pillar,
        reason_code: reason_code.to_string(),
        severity,
        message,
        evidence,
    }
}

fn has_owner_metadata(item: &AiPromptTemplateInventoryItem) -> bool {
    item.owner.as_deref().map(has_text).unwrap_or(false)
        || item.tags.iter().any(|tag| {
            let lower = tag.to_ascii_lowercase();
            lower.starts_with("owner:")
                || lower.starts_with("owner=")
                || lower.starts_with("team:")
                || lower.starts_with("team=")
                || lower.starts_with("cost-center:")
                || lower.starts_with("cost-center=")
                || lower.starts_with("cost_center:")
                || lower.starts_with("cost_center=")
        })
}

fn is_mysql_scoped(model: &PromptTemplateModel) -> bool {
    model.resource_type.as_ref().map_or(true, |resource_type| {
        let lower = resource_type.to_ascii_lowercase();
        lower.contains("mysql") || lower == "database" || lower == "sql"
    }) || contains_any(&model.name, &["mysql"])
        || contains_any(&model.category, &["mysql", "database"])
        || parse_tags(&model.tags)
            .iter()
            .any(|tag| contains_any(tag, &["mysql", "database"]))
}

fn parse_tags(tags: &Value) -> Vec<String> {
    match tags {
        Value::Array(values) => values
            .iter()
            .filter_map(|value| value.as_str().map(|tag| tag.trim().to_string()))
            .filter(|tag| !tag.is_empty())
            .collect(),
        Value::Object(values) => values
            .iter()
            .filter_map(|(key, value)| value.as_str().map(|v| format!("{}={}", key, v)))
            .collect(),
        _ => Vec::new(),
    }
}

fn variable_count(variables: &Value) -> usize {
    match variables {
        Value::Array(values) => values.len(),
        Value::Object(values) => values.len(),
        _ => 0,
    }
}

fn contains_any(text: &str, needles: &[&str]) -> bool {
    let lower = text.to_ascii_lowercase();
    needles.iter().any(|needle| lower.contains(needle))
}

fn has_text(value: &str) -> bool {
    !value.trim().is_empty()
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    fn item() -> AiPromptTemplateInventoryItem {
        AiPromptTemplateInventoryItem {
            template_id: "prompt-1".to_string(),
            name: "mysql-performance-triage".to_string(),
            category: "database_optimization".to_string(),
            resource_type: Some("MySQL".to_string()),
            workflow_type: Some("performance".to_string()),
            version: "1.0".to_string(),
            owner: Some("sre".to_string()),
            tags: vec!["team=db".to_string()],
            is_active: true,
            is_system: true,
            has_description: true,
            variable_count: 2,
            prompt_bytes: 2048,
            has_token_budget_control: true,
            has_guardrails: true,
            updated_at: Utc::now(),
        }
    }

    #[test]
    fn cost_flags_missing_owner_and_token_budget() {
        let now = Utc::now();
        let mut item = item();
        item.owner = None;
        item.tags.clear();
        item.has_token_budget_control = false;

        let report = evaluate_mysql_ai_prompt_templates_inventory(&[item], Pillar::Cost, now);

        assert_eq!(report.resources_evaluated, 1);
        assert_eq!(report.findings.len(), 2);
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_COST_OWNER_NOT_RECORDED));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_COST_TOKEN_BUDGET_MISSING));
        assert!(report.score < 100);
    }

    #[test]
    fn resilience_flags_missing_context_contract_and_description() {
        let now = Utc::now();
        let mut item = item();
        item.variable_count = 0;
        item.has_description = false;

        let report = evaluate_mysql_ai_prompt_templates_inventory(&[item], Pillar::Resilience, now);

        assert_eq!(report.findings.len(), 2);
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_RES_CONTEXT_CONTRACT_MISSING));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_RES_DESCRIPTION_MISSING));
    }

    #[test]
    fn security_flags_missing_owner_and_guardrails() {
        let now = Utc::now();
        let mut item = item();
        item.owner = None;
        item.tags.clear();
        item.has_guardrails = false;

        let report = evaluate_mysql_ai_prompt_templates_inventory(&[item], Pillar::Security, now);

        assert_eq!(report.findings.len(), 2);
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_SEC_OWNER_NOT_RECORDED));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_SEC_GUARDRAILS_MISSING));
    }

    #[test]
    fn stale_prompt_template_is_counted_for_any_pillar() {
        let now = Utc::now();
        let mut item = item();
        item.updated_at = now - Duration::hours(DEFAULT_STALE_AFTER_HOURS + 2);

        let report = evaluate_mysql_ai_prompt_templates_inventory(&[item], Pillar::Cost, now);

        assert_eq!(report.stale_resources, 1);
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_INV_STALE_DATA));
    }

    #[test]
    fn healthy_prompt_template_passes_claimed_pillars() {
        let now = Utc::now();
        let items = vec![item()];

        for pillar in [Pillar::Cost, Pillar::Resilience, Pillar::Security] {
            let report = evaluate_mysql_ai_prompt_templates_inventory(&items, pillar, now);
            assert_eq!(report.score, 100);
            assert!(report.findings.is_empty());
        }
    }
}

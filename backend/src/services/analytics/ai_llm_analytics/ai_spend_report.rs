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

use std::collections::{BTreeSet, HashMap};

use chrono::{DateTime, Utc};
use serde::Serialize;
use serde_json::Value;

use crate::services::analytics::ai_llm_analytics::model_cost_inventory::{
    evaluate_model_cost_inventory, ModelCostInventoryItem, REASON_COST_BUDGET_MISSING,
    REASON_COST_ESTIMATE_OVER_BUDGET, REASON_COST_PRICE_MISSING, REASON_INV_STALE_DATA,
};
use crate::services::analytics::ai_llm_analytics::token_usage_inventory::{
    evaluate_token_usage_inventory, TokenUsageInventoryItem, REASON_COST_TOKEN_BUDGET_MISSING,
    REASON_COST_USAGE_OVER_BUDGET, REASON_RES_USAGE_SAMPLE_MISSING,
};
use crate::services::aws::inventory::types::{InventoryFinding, Pillar, Severity};

pub const WORKFLOW_ID: &str = "ai_llm_spend_reporting";
pub const SAVED_VIEW_ID: &str = "ai-llm-spend-report";

#[derive(Debug, Clone, Serialize)]
pub struct AiSpendExecutiveSummary {
    pub report_id: &'static str,
    pub score: u8,
    pub resources_evaluated: usize,
    pub stale_resources: usize,
    pub total_estimated_monthly_cost_usd: f64,
    pub total_monthly_budget_usd: f64,
    pub total_tokens: u64,
    pub affected_resources: Vec<String>,
    pub models_over_budget: usize,
    pub routes_over_budget: usize,
    pub top_reason_codes: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AiSpendReportRow {
    pub resource_id: String,
    pub model_name: String,
    pub source_inventory: &'static str,
    pub severity: Severity,
    pub reason_code: String,
    pub message: String,
    pub evidence: Value,
}

#[derive(Debug, Clone, Serialize)]
pub struct AiSpendReportBacklog {
    pub report_id: &'static str,
    pub page: usize,
    pub page_size: usize,
    pub total: usize,
    pub rows: Vec<AiSpendReportRow>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AiSpendReportBundle {
    pub workflow_id: &'static str,
    pub read_only_mode: bool,
    pub scheduled_delivery_state: &'static str,
    pub saved_view_id: &'static str,
    pub evaluated_at: DateTime<Utc>,
    pub resources_evaluated: usize,
    pub model_cost_resources_evaluated: usize,
    pub token_usage_resources_evaluated: usize,
    pub executive_summary: AiSpendExecutiveSummary,
    pub engineering_backlog: AiSpendReportBacklog,
    pub incident_review: AiSpendReportBacklog,
    pub missing_data_reason_codes: Vec<String>,
    pub evidence_reason_codes: Vec<String>,
}

pub fn ai_spend_report_bundle(
    model_cost_items: &[ModelCostInventoryItem],
    token_usage_items: &[TokenUsageInventoryItem],
    now: DateTime<Utc>,
) -> AiSpendReportBundle {
    let model_cost_report = evaluate_model_cost_inventory(model_cost_items, Pillar::Cost, now);
    let token_usage_report = evaluate_token_usage_inventory(token_usage_items, Pillar::Cost, now);
    let model_names = model_name_index(model_cost_items, token_usage_items);

    let model_cost_rows = model_cost_report
        .findings
        .iter()
        .map(|finding| report_row(finding, &model_names, "model_cost_inventory"));
    let token_usage_rows = token_usage_report
        .findings
        .iter()
        .map(|finding| report_row(finding, &model_names, "token_usage_inventory"));
    let rows: Vec<_> = model_cost_rows.chain(token_usage_rows).collect();

    let evidence_reason_codes =
        sorted_unique_reason_codes([&model_cost_report.findings, &token_usage_report.findings]);
    let missing_data_reason_codes = rows
        .iter()
        .filter(|row| {
            matches!(
                row.reason_code.as_str(),
                REASON_INV_STALE_DATA
                    | REASON_COST_PRICE_MISSING
                    | REASON_COST_BUDGET_MISSING
                    | REASON_COST_TOKEN_BUDGET_MISSING
                    | REASON_RES_USAGE_SAMPLE_MISSING
            )
        })
        .map(|row| row.reason_code.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let stale_resources = model_cost_report
        .stale_resources
        .max(token_usage_report.stale_resources);
    let models_over_budget = count_reason(
        &model_cost_report.findings,
        REASON_COST_ESTIMATE_OVER_BUDGET,
    );
    let routes_over_budget =
        count_reason(&token_usage_report.findings, REASON_COST_USAGE_OVER_BUDGET);
    let affected_resources = rows
        .iter()
        .map(|row| row.model_name.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let total_estimated_monthly_cost_usd = model_cost_items
        .iter()
        .filter_map(|item| item.estimated_monthly_cost_usd)
        .sum::<f64>();
    let total_monthly_budget_usd = model_cost_items
        .iter()
        .filter_map(|item| item.monthly_budget_usd)
        .sum::<f64>();
    let total_tokens = token_usage_items
        .iter()
        .filter_map(|item| item.total_tokens)
        .sum::<u64>();
    let scheduled_delivery_state = if stale_resources > 0 {
        "blocked_until_fresh_cost_evidence"
    } else if missing_data_reason_codes.is_empty() {
        "ready_for_schedule"
    } else {
        "ready_with_cost_evidence_gaps"
    };

    AiSpendReportBundle {
        workflow_id: WORKFLOW_ID,
        read_only_mode: true,
        scheduled_delivery_state,
        saved_view_id: SAVED_VIEW_ID,
        evaluated_at: now,
        resources_evaluated: model_cost_items.len().max(token_usage_items.len()),
        model_cost_resources_evaluated: model_cost_report.resources_evaluated,
        token_usage_resources_evaluated: token_usage_report.resources_evaluated,
        executive_summary: AiSpendExecutiveSummary {
            report_id: "ai-llm-spend-executive-summary",
            score: model_cost_report.score.min(token_usage_report.score),
            resources_evaluated: model_cost_items.len().max(token_usage_items.len()),
            stale_resources,
            total_estimated_monthly_cost_usd,
            total_monthly_budget_usd,
            total_tokens,
            affected_resources,
            models_over_budget,
            routes_over_budget,
            top_reason_codes: evidence_reason_codes.clone(),
        },
        engineering_backlog: AiSpendReportBacklog {
            report_id: "ai-llm-spend-engineering-backlog",
            page: 0,
            page_size: 50,
            total: rows.len(),
            rows: rows.clone(),
        },
        incident_review: AiSpendReportBacklog {
            report_id: "ai-llm-spend-incident-review",
            page: 0,
            page_size: 50,
            total: rows.len(),
            rows,
        },
        missing_data_reason_codes,
        evidence_reason_codes,
    }
}

fn report_row(
    finding: &InventoryFinding,
    model_names: &HashMap<String, String>,
    source_inventory: &'static str,
) -> AiSpendReportRow {
    AiSpendReportRow {
        resource_id: finding.resource_id.clone(),
        model_name: model_names
            .get(&finding.resource_id)
            .cloned()
            .unwrap_or_else(|| finding.resource_id.clone()),
        source_inventory,
        severity: finding.severity,
        reason_code: finding.reason_code.clone(),
        message: finding.message.clone(),
        evidence: finding.evidence.clone(),
    }
}

fn model_name_index(
    model_cost_items: &[ModelCostInventoryItem],
    token_usage_items: &[TokenUsageInventoryItem],
) -> HashMap<String, String> {
    let mut index = HashMap::new();
    for item in model_cost_items {
        index.insert(item.model_id.clone(), item.model_name.clone());
    }
    for item in token_usage_items {
        index.insert(item.route_id.clone(), item.model_name.clone());
    }
    index
}

fn sorted_unique_reason_codes<'a>(
    finding_sets: impl IntoIterator<Item = &'a Vec<InventoryFinding>>,
) -> Vec<String> {
    finding_sets
        .into_iter()
        .flat_map(|findings| findings.iter().map(|finding| finding.reason_code.clone()))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn count_reason(findings: &[InventoryFinding], reason_code: &str) -> usize {
    findings
        .iter()
        .filter(|finding| finding.reason_code == reason_code)
        .count()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::analytics::ai_llm_analytics::model_cost_inventory::ModelCostInventoryItem;
    use crate::services::analytics::ai_llm_analytics::token_usage_inventory::TokenUsageInventoryItem;

    fn model_cost_item() -> ModelCostInventoryItem {
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
            monthly_budget_usd: Some(120.0),
            cost_guardrail: Some("cache prompts".to_string()),
            fallback_model: Some("deepseek-lite".to_string()),
            audit_enabled: true,
            redaction_policy: Some("redact prompt text".to_string()),
            updated_at: Utc::now(),
        }
    }

    fn token_usage_item() -> TokenUsageInventoryItem {
        TokenUsageInventoryItem {
            route_id: "model-1".to_string(),
            provider_id: "provider-1".to_string(),
            model_name: "deepseek-chat".to_string(),
            enabled: true,
            owner: Some("sre-ai".to_string()),
            labels: vec!["cost-center=ai-platform".to_string()],
            prompt_tokens: Some(40_000),
            completion_tokens: Some(20_000),
            total_tokens: Some(60_000),
            monthly_token_budget: Some(100_000),
            max_tokens_per_request: Some(8_000),
            rate_limit_tokens_per_minute: Some(60_000),
            burst_guardrail: Some("throttle large prompts".to_string()),
            audit_enabled: true,
            redaction_policy: Some("redact prompt text".to_string()),
            updated_at: Utc::now(),
        }
    }

    #[test]
    fn healthy_ai_spend_report_is_ready_for_schedule() {
        let bundle =
            ai_spend_report_bundle(&[model_cost_item()], &[token_usage_item()], Utc::now());
        assert_eq!(bundle.workflow_id, WORKFLOW_ID);
        assert_eq!(bundle.scheduled_delivery_state, "ready_for_schedule");
        assert_eq!(bundle.executive_summary.score, 100);
        assert!(bundle.missing_data_reason_codes.is_empty());
        assert_eq!(
            bundle.executive_summary.total_estimated_monthly_cost_usd,
            80.0
        );
        assert_eq!(bundle.executive_summary.total_tokens, 60_000);
    }

    #[test]
    fn spend_report_surfaces_cost_gaps_from_both_inventories() {
        let mut model_cost = model_cost_item();
        model_cost.monthly_budget_usd = None;
        model_cost.estimated_monthly_cost_usd = Some(200.0);
        let mut token_usage = token_usage_item();
        token_usage.monthly_token_budget = None;

        let bundle = ai_spend_report_bundle(&[model_cost], &[token_usage], Utc::now());

        assert_eq!(
            bundle.scheduled_delivery_state,
            "ready_with_cost_evidence_gaps"
        );
        assert!(bundle
            .evidence_reason_codes
            .contains(&REASON_COST_BUDGET_MISSING.to_string()));
        assert!(bundle
            .evidence_reason_codes
            .contains(&REASON_COST_TOKEN_BUDGET_MISSING.to_string()));
        assert_eq!(bundle.engineering_backlog.total, 2);
    }
}

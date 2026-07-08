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

// Deterministic AI/LLM tool call trace inventory evaluator for roadmap rows
// 25-AI-LLM-OBSERVABILITY-00344/00351/00372.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::models::llm_model::Model as LlmProviderModel;
use crate::services::aws::inventory::types::{
    score_pillar, InventoryFinding, Pillar, PillarReport, Severity, COST_ALLOCATION_TAG_KEYS,
    DEFAULT_STALE_AFTER_HOURS,
};

#[cfg(test)]
#[path = "tool_call_trace_inventory_tests.rs"]
mod tool_call_trace_inventory_tests;

pub const RESOURCE_TYPE: &str = "AiLlmToolCallTrace";
pub const REASON_COST_OWNER_NOT_RECORDED: &str = "AI_LLM_TOOL_CALL_TRACE_COST_OWNER_NOT_RECORDED";
pub const REASON_COST_BUDGET_MISSING: &str = "AI_LLM_TOOL_CALL_TRACE_COST_BUDGET_MISSING";
pub const REASON_COST_ESTIMATE_OVER_BUDGET: &str =
    "AI_LLM_TOOL_CALL_TRACE_COST_ESTIMATE_OVER_BUDGET";
pub const REASON_RES_TRACE_SAMPLE_MISSING: &str = "AI_LLM_TOOL_CALL_TRACE_RES_SAMPLE_MISSING";
pub const REASON_RES_SAMPLING_POLICY_MISSING: &str =
    "AI_LLM_TOOL_CALL_TRACE_RES_SAMPLING_POLICY_MISSING";
pub const REASON_RES_REPLAY_MISSING: &str = "AI_LLM_TOOL_CALL_TRACE_RES_REPLAY_MISSING";
pub const REASON_SEC_REGISTRY_MISSING: &str = "AI_LLM_TOOL_CALL_TRACE_SEC_REGISTRY_MISSING";
pub const REASON_SEC_AUDIT_MISSING: &str = "AI_LLM_TOOL_CALL_TRACE_SEC_AUDIT_MISSING";
pub const REASON_SEC_REDACTION_MISSING: &str = "AI_LLM_TOOL_CALL_TRACE_SEC_REDACTION_MISSING";
pub const REASON_INV_STALE_DATA: &str = "AI_LLM_TOOL_CALL_TRACE_INV_STALE_DATA";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallTraceInventoryItem {
    pub trace_id: String,
    pub provider_id: String,
    pub model_name: String,
    pub enabled: bool,
    pub owner: Option<String>,
    pub labels: Vec<String>,
    pub trace_count: Option<u64>,
    pub failed_tool_call_count: Option<u64>,
    pub estimated_monthly_trace_cost_usd: Option<f64>,
    pub monthly_trace_budget_usd: Option<f64>,
    pub sampling_policy: Option<String>,
    pub replay_enabled: bool,
    pub approved_tool_registry: Option<String>,
    pub audit_enabled: bool,
    pub redaction_policy: Option<String>,
    pub updated_at: DateTime<Utc>,
}

pub fn evaluate_tool_call_trace_inventory(
    items: &[ToolCallTraceInventoryItem],
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

pub fn tool_call_trace_inventory_item_from_model(
    model: &LlmProviderModel,
) -> ToolCallTraceInventoryItem {
    ToolCallTraceInventoryItem {
        trace_id: model.id.to_string(),
        provider_id: model.provider_id.to_string(),
        model_name: model.model_name.clone(),
        enabled: model.enabled,
        owner: string_field(&model.model_config, &["owner", "team", "cost_owner"]),
        labels: string_array_field(&model.model_config, "labels"),
        trace_count: integer_field(
            &model.model_config,
            &["tool_call_trace_count", "trace_count", "tool_call_count"],
        ),
        failed_tool_call_count: integer_field(
            &model.model_config,
            &["failed_tool_call_count", "tool_call_error_count"],
        ),
        estimated_monthly_trace_cost_usd: number_field(
            &model.model_config,
            &[
                "estimated_monthly_trace_cost_usd",
                "trace_monthly_cost_usd",
                "telemetry_cost_usd",
            ],
        ),
        monthly_trace_budget_usd: number_field(
            &model.model_config,
            &[
                "monthly_trace_budget_usd",
                "trace_budget_usd",
                "telemetry_budget_usd",
            ],
        ),
        sampling_policy: string_field(
            &model.model_config,
            &[
                "trace_sampling_policy",
                "sampling_policy",
                "retention_policy",
            ],
        ),
        replay_enabled: bool_field(
            &model.model_config,
            &[
                "trace_replay_enabled",
                "replay_enabled",
                "tool_replay_enabled",
            ],
        ),
        approved_tool_registry: string_field(
            &model.model_config,
            &[
                "approved_tool_registry",
                "tool_registry",
                "allowed_tools_policy",
            ],
        ),
        audit_enabled: bool_field(
            &model.model_config,
            &[
                "audit_enabled",
                "tool_call_audit_enabled",
                "trace_audit_enabled",
            ],
        ),
        redaction_policy: string_field(
            &model.model_config,
            &[
                "trace_redaction_policy",
                "telemetry_redaction_policy",
                "redaction_policy",
                "data_policy",
            ],
        ),
        updated_at: model.updated_at,
    }
}

fn evaluate_cost(
    item: &ToolCallTraceInventoryItem,
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
                "AI/LLM tool call trace {} has no owner metadata",
                item.model_name
            ),
            json!({"trace_id": item.trace_id, "labels": item.labels, "checked_keys": COST_ALLOCATION_TAG_KEYS}),
        ));
    }

    if item.monthly_trace_budget_usd.is_none() {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_BUDGET_MISSING,
            Severity::Medium,
            format!(
                "AI/LLM tool call trace {} has no trace budget",
                item.model_name
            ),
            json!({"trace_id": item.trace_id, "recommendation": "Record telemetry or trace budget before high-volume tool call traces are enabled"}),
        ));
    }

    if item
        .estimated_monthly_trace_cost_usd
        .zip(item.monthly_trace_budget_usd)
        .map(|(estimate, budget)| estimate > budget)
        .unwrap_or(false)
    {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_ESTIMATE_OVER_BUDGET,
            Severity::High,
            format!(
                "AI/LLM tool call trace {} is over trace budget",
                item.model_name
            ),
            json!({"trace_id": item.trace_id, "estimated_monthly_trace_cost_usd": item.estimated_monthly_trace_cost_usd, "monthly_trace_budget_usd": item.monthly_trace_budget_usd}),
        ));
    }
}

fn evaluate_resilience(
    item: &ToolCallTraceInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if item.trace_count.unwrap_or(0) == 0 {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_TRACE_SAMPLE_MISSING,
            Severity::High,
            format!(
                "AI/LLM tool call trace {} has no usable trace sample",
                item.model_name
            ),
            json!({"trace_id": item.trace_id, "trace_count": item.trace_count, "failed_tool_call_count": item.failed_tool_call_count}),
        ));
    }

    if item
        .sampling_policy
        .as_deref()
        .unwrap_or_default()
        .trim()
        .is_empty()
    {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_SAMPLING_POLICY_MISSING,
            Severity::Medium,
            format!(
                "AI/LLM tool call trace {} has no sampling policy",
                item.model_name
            ),
            json!({"trace_id": item.trace_id, "recommendation": "Record trace sampling and retention so bounded investigations have deterministic evidence"}),
        ));
    }

    if !item.replay_enabled {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_REPLAY_MISSING,
            Severity::High,
            format!(
                "AI/LLM tool call trace {} is not replayable",
                item.model_name
            ),
            json!({"trace_id": item.trace_id, "recommendation": "Enable replayable trace evidence for incident triage and rollback notes"}),
        ));
    }
}

fn evaluate_security(
    item: &ToolCallTraceInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if item
        .approved_tool_registry
        .as_deref()
        .unwrap_or_default()
        .trim()
        .is_empty()
    {
        findings.push(finding(
            item,
            pillar,
            REASON_SEC_REGISTRY_MISSING,
            Severity::High,
            format!(
                "AI/LLM tool call trace {} has no approved tool registry",
                item.model_name
            ),
            json!({"trace_id": item.trace_id, "recommendation": "Tie tool traces to an approved registry so read-only defaults and mutation approvals are auditable"}),
        ));
    }

    if !item.audit_enabled {
        findings.push(finding(
            item,
            pillar,
            REASON_SEC_AUDIT_MISSING,
            Severity::High,
            format!(
                "AI/LLM tool call trace {} has no audit trail",
                item.model_name
            ),
            json!({"trace_id": item.trace_id}),
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
                "AI/LLM tool call trace {} has no redaction policy",
                item.model_name
            ),
            json!({"trace_id": item.trace_id}),
        ));
    }
}

fn stale_finding(
    item: &ToolCallTraceInventoryItem,
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
            "AI/LLM tool call trace {} inventory data is stale by {} hours",
            item.model_name, age_hours
        ),
        json!({"trace_id": item.trace_id, "age_hours": age_hours, "stale_after_hours": DEFAULT_STALE_AFTER_HOURS}),
    ))
}

fn finding(
    item: &ToolCallTraceInventoryItem,
    pillar: Pillar,
    reason_code: &str,
    severity: Severity,
    message: String,
    evidence: Value,
) -> InventoryFinding {
    InventoryFinding {
        resource_id: item.trace_id.clone(),
        arn: format!("ai-llm-tool-call-trace/{}", item.trace_id),
        pillar,
        reason_code: reason_code.to_string(),
        severity,
        message,
        evidence,
    }
}

fn has_owner_metadata(item: &ToolCallTraceInventoryItem) -> bool {
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

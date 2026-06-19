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

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::models::llm_model::Model as LlmProviderModel;
use crate::services::aws::inventory::types::{
    score_pillar, InventoryFinding, Pillar, PillarReport, Severity, COST_ALLOCATION_TAG_KEYS,
    DEFAULT_STALE_AFTER_HOURS,
};

pub const RESOURCE_TYPE: &str = "AiLlmUnsafeToolCallPrevention";
pub const REASON_COST_OWNER_NOT_RECORDED: &str = "AI_LLM_UNSAFE_TOOL_CALL_COST_OWNER_NOT_RECORDED";
pub const REASON_COST_BUDGET_MISSING: &str = "AI_LLM_UNSAFE_TOOL_CALL_COST_BUDGET_MISSING";
pub const REASON_COST_ESTIMATE_OVER_BUDGET: &str =
    "AI_LLM_UNSAFE_TOOL_CALL_COST_ESTIMATE_OVER_BUDGET";
pub const REASON_RES_PREVENTION_DISABLED: &str = "AI_LLM_UNSAFE_TOOL_CALL_RES_PREVENTION_DISABLED";
pub const REASON_RES_REGISTRY_MISSING: &str = "AI_LLM_UNSAFE_TOOL_CALL_RES_REGISTRY_MISSING";
pub const REASON_RES_REVIEW_WINDOW_MISSING: &str =
    "AI_LLM_UNSAFE_TOOL_CALL_RES_REVIEW_WINDOW_MISSING";
pub const REASON_RES_DRY_RUN_MISSING: &str = "AI_LLM_UNSAFE_TOOL_CALL_RES_DRY_RUN_MISSING";
pub const REASON_SEC_READ_ONLY_DEFAULT_MISSING: &str =
    "AI_LLM_UNSAFE_TOOL_CALL_SEC_READ_ONLY_DEFAULT_MISSING";
pub const REASON_SEC_MUTATION_APPROVAL_MISSING: &str =
    "AI_LLM_UNSAFE_TOOL_CALL_SEC_MUTATION_APPROVAL_MISSING";
pub const REASON_SEC_AUDIT_MISSING: &str = "AI_LLM_UNSAFE_TOOL_CALL_SEC_AUDIT_MISSING";
pub const REASON_SEC_REDACTION_MISSING: &str = "AI_LLM_UNSAFE_TOOL_CALL_SEC_REDACTION_MISSING";
pub const REASON_INV_STALE_DATA: &str = "AI_LLM_UNSAFE_TOOL_CALL_INV_STALE_DATA";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnsafeToolCallInventoryItem {
    pub policy_id: String,
    pub provider_id: String,
    pub model_name: String,
    pub enabled: bool,
    pub owner: Option<String>,
    pub labels: Vec<String>,
    pub prevention_enabled: bool,
    pub approved_tool_registry: Option<String>,
    pub blocked_tool_call_count: Option<u64>,
    pub review_window: Option<String>,
    pub estimated_monthly_cost_usd: Option<f64>,
    pub monthly_budget_usd: Option<f64>,
    pub read_only_default: bool,
    pub mutation_approval_required: bool,
    pub dry_run_supported: bool,
    pub audit_enabled: bool,
    pub redaction_policy: Option<String>,
    pub updated_at: DateTime<Utc>,
}

pub fn evaluate_unsafe_tool_call_inventory(
    items: &[UnsafeToolCallInventoryItem],
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

pub fn unsafe_tool_call_inventory_item_from_model(
    model: &LlmProviderModel,
) -> UnsafeToolCallInventoryItem {
    UnsafeToolCallInventoryItem {
        policy_id: model.id.to_string(),
        provider_id: model.provider_id.to_string(),
        model_name: model.model_name.clone(),
        enabled: model.enabled,
        owner: string_field(&model.model_config, &["owner", "team", "cost_owner"]),
        labels: string_array_field(&model.model_config, "labels"),
        prevention_enabled: bool_field(
            &model.model_config,
            &[
                "unsafe_tool_call_prevention_enabled",
                "tool_call_prevention_enabled",
                "tool_firewall_enabled",
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
        blocked_tool_call_count: integer_field(
            &model.model_config,
            &[
                "blocked_tool_call_count",
                "prevented_tool_call_count",
                "unsafe_tool_call_count",
            ],
        ),
        review_window: string_field(
            &model.model_config,
            &[
                "unsafe_tool_call_review_window",
                "tool_call_review_window",
                "review_window",
            ],
        ),
        estimated_monthly_cost_usd: number_field(
            &model.model_config,
            &[
                "unsafe_tool_call_monthly_cost_usd",
                "tool_firewall_cost_usd",
                "monthly_cost_usd",
            ],
        ),
        monthly_budget_usd: number_field(
            &model.model_config,
            &[
                "unsafe_tool_call_budget_usd",
                "tool_firewall_budget_usd",
                "monthly_budget_usd",
            ],
        ),
        read_only_default: bool_field(
            &model.model_config,
            &[
                "read_only_tool_default",
                "tool_read_only_default",
                "read_only_default",
            ],
        ),
        mutation_approval_required: bool_field(
            &model.model_config,
            &[
                "tool_mutation_approval_required",
                "mutation_approval_required",
                "tool_approval_required",
            ],
        ),
        dry_run_supported: bool_field(
            &model.model_config,
            &[
                "tool_dry_run_supported",
                "dry_run_supported",
                "tool_preview_supported",
            ],
        ),
        audit_enabled: bool_field(
            &model.model_config,
            &[
                "audit_enabled",
                "tool_call_audit_enabled",
                "tool_policy_audit_enabled",
            ],
        ),
        redaction_policy: string_field(
            &model.model_config,
            &[
                "tool_call_redaction_policy",
                "trace_redaction_policy",
                "redaction_policy",
            ],
        ),
        updated_at: model.updated_at,
    }
}

fn evaluate_cost(
    item: &UnsafeToolCallInventoryItem,
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
                "AI/LLM unsafe tool-call policy {} has no owner metadata",
                item.model_name
            ),
            json!({"policy_id": item.policy_id, "labels": item.labels, "checked_keys": COST_ALLOCATION_TAG_KEYS}),
        ));
    }

    if item.monthly_budget_usd.is_none() {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_BUDGET_MISSING,
            Severity::Medium,
            format!(
                "AI/LLM unsafe tool-call policy {} has no budget",
                item.model_name
            ),
            json!({"policy_id": item.policy_id, "recommendation": "Record prevention and review budget before unsafe-tool policy is trusted"}),
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
                "AI/LLM unsafe tool-call policy {} is over budget",
                item.model_name
            ),
            json!({"policy_id": item.policy_id, "estimated_monthly_cost_usd": item.estimated_monthly_cost_usd, "monthly_budget_usd": item.monthly_budget_usd}),
        ));
    }
}

fn evaluate_resilience(
    item: &UnsafeToolCallInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if !item.prevention_enabled {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_PREVENTION_DISABLED,
            Severity::High,
            format!(
                "AI/LLM unsafe tool-call policy {} is disabled",
                item.model_name
            ),
            json!({"policy_id": item.policy_id}),
        ));
    }

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
            REASON_RES_REGISTRY_MISSING,
            Severity::High,
            format!(
                "AI/LLM unsafe tool-call policy {} has no approved tool registry",
                item.model_name
            ),
            json!({"policy_id": item.policy_id}),
        ));
    }

    if item
        .review_window
        .as_deref()
        .unwrap_or_default()
        .trim()
        .is_empty()
    {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_REVIEW_WINDOW_MISSING,
            Severity::Medium,
            format!(
                "AI/LLM unsafe tool-call policy {} has no review window",
                item.model_name
            ),
            json!({"policy_id": item.policy_id}),
        ));
    }

    if !item.dry_run_supported {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_DRY_RUN_MISSING,
            Severity::Medium,
            format!(
                "AI/LLM unsafe tool-call policy {} has no dry-run support",
                item.model_name
            ),
            json!({"policy_id": item.policy_id}),
        ));
    }
}

fn evaluate_security(
    item: &UnsafeToolCallInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if !item.read_only_default {
        findings.push(finding(
            item,
            pillar,
            REASON_SEC_READ_ONLY_DEFAULT_MISSING,
            Severity::High,
            format!(
                "AI/LLM unsafe tool-call policy {} is not read-only by default",
                item.model_name
            ),
            json!({"policy_id": item.policy_id}),
        ));
    }

    if !item.mutation_approval_required {
        findings.push(finding(
            item,
            pillar,
            REASON_SEC_MUTATION_APPROVAL_MISSING,
            Severity::High,
            format!(
                "AI/LLM unsafe tool-call policy {} does not require mutation approval",
                item.model_name
            ),
            json!({"policy_id": item.policy_id}),
        ));
    }

    if !item.audit_enabled {
        findings.push(finding(
            item,
            pillar,
            REASON_SEC_AUDIT_MISSING,
            Severity::High,
            format!(
                "AI/LLM unsafe tool-call policy {} has no audit trail",
                item.model_name
            ),
            json!({"policy_id": item.policy_id}),
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
            Severity::Medium,
            format!(
                "AI/LLM unsafe tool-call policy {} has no redaction policy",
                item.model_name
            ),
            json!({"policy_id": item.policy_id}),
        ));
    }
}

fn stale_finding(
    item: &UnsafeToolCallInventoryItem,
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
            "AI/LLM unsafe tool-call policy {} inventory data is stale by {} hours",
            item.model_name, age_hours
        ),
        json!({"policy_id": item.policy_id, "age_hours": age_hours, "stale_after_hours": DEFAULT_STALE_AFTER_HOURS}),
    ))
}

fn finding(
    item: &UnsafeToolCallInventoryItem,
    pillar: Pillar,
    reason_code: &str,
    severity: Severity,
    message: String,
    evidence: Value,
) -> InventoryFinding {
    InventoryFinding {
        resource_id: item.policy_id.clone(),
        arn: format!("ai-llm-unsafe-tool-call/{}", item.policy_id),
        pillar,
        reason_code: reason_code.to_string(),
        severity,
        message,
        evidence,
    }
}

fn has_owner_metadata(item: &UnsafeToolCallInventoryItem) -> bool {
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

    fn item() -> UnsafeToolCallInventoryItem {
        UnsafeToolCallInventoryItem {
            policy_id: "policy-1".to_string(),
            provider_id: "provider-1".to_string(),
            model_name: "ops-agent".to_string(),
            enabled: true,
            owner: Some("sre-ai".to_string()),
            labels: vec!["cost-center=ai-platform".to_string()],
            prevention_enabled: true,
            approved_tool_registry: Some("ops-readonly-tools".to_string()),
            blocked_tool_call_count: Some(4),
            review_window: Some("rolling 7d".to_string()),
            estimated_monthly_cost_usd: Some(30.0),
            monthly_budget_usd: Some(100.0),
            read_only_default: true,
            mutation_approval_required: true,
            dry_run_supported: true,
            audit_enabled: true,
            redaction_policy: Some("redact prompt, tool args, and outputs".to_string()),
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
    fn healthy_unsafe_tool_call_inventory_passes_claimed_pillars() {
        let now = Utc::now();
        for pillar in [Pillar::Cost, Pillar::Resilience, Pillar::Security] {
            let report = evaluate_unsafe_tool_call_inventory(&[item()], pillar, now);
            assert_eq!(report.resources_evaluated, 1);
            assert!(report.findings.is_empty());
        }
    }

    #[test]
    fn security_flags_missing_read_only_approval_audit_and_redaction() {
        let mut item = item();
        item.read_only_default = false;
        item.mutation_approval_required = false;
        item.audit_enabled = false;
        item.redaction_policy = None;

        let report = evaluate_unsafe_tool_call_inventory(&[item], Pillar::Security, Utc::now());
        let codes = codes(&report);

        assert!(codes.contains(&REASON_SEC_READ_ONLY_DEFAULT_MISSING.to_string()));
        assert!(codes.contains(&REASON_SEC_MUTATION_APPROVAL_MISSING.to_string()));
        assert!(codes.contains(&REASON_SEC_AUDIT_MISSING.to_string()));
        assert!(codes.contains(&REASON_SEC_REDACTION_MISSING.to_string()));
    }

    #[test]
    fn stale_unsafe_tool_call_inventory_is_counted_for_any_pillar() {
        let mut item = item();
        item.updated_at = Utc::now() - Duration::hours(DEFAULT_STALE_AFTER_HOURS + 2);

        let report = evaluate_unsafe_tool_call_inventory(&[item], Pillar::Security, Utc::now());
        assert_eq!(report.stale_resources, 1);
        assert!(codes(&report).contains(&REASON_INV_STALE_DATA.to_string()));
    }
}

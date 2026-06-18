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

// Deterministic AI/LLM evaluation dataset inventory evaluator for roadmap rows
// 25-AI-LLM-OBSERVABILITY-00393/00400/00421.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::models::llm_model::Model as LlmProviderModel;
use crate::services::aws::inventory::types::{
    score_pillar, InventoryFinding, Pillar, PillarReport, Severity, COST_ALLOCATION_TAG_KEYS,
    DEFAULT_STALE_AFTER_HOURS,
};

pub const RESOURCE_TYPE: &str = "AiLlmEvaluationDataset";
pub const REASON_COST_OWNER_NOT_RECORDED: &str =
    "AI_LLM_EVALUATION_DATASET_COST_OWNER_NOT_RECORDED";
pub const REASON_COST_BUDGET_MISSING: &str = "AI_LLM_EVALUATION_DATASET_COST_BUDGET_MISSING";
pub const REASON_COST_ESTIMATE_OVER_BUDGET: &str =
    "AI_LLM_EVALUATION_DATASET_COST_ESTIMATE_OVER_BUDGET";
pub const REASON_RES_SAMPLE_MISSING: &str = "AI_LLM_EVALUATION_DATASET_RES_SAMPLE_MISSING";
pub const REASON_RES_VERSION_MISSING: &str = "AI_LLM_EVALUATION_DATASET_RES_VERSION_MISSING";
pub const REASON_RES_REFRESH_POLICY_MISSING: &str =
    "AI_LLM_EVALUATION_DATASET_RES_REFRESH_POLICY_MISSING";
pub const REASON_SEC_LICENSE_MISSING: &str = "AI_LLM_EVALUATION_DATASET_SEC_LICENSE_MISSING";
pub const REASON_SEC_AUDIT_MISSING: &str = "AI_LLM_EVALUATION_DATASET_SEC_AUDIT_MISSING";
pub const REASON_SEC_REDACTION_MISSING: &str = "AI_LLM_EVALUATION_DATASET_SEC_REDACTION_MISSING";
pub const REASON_INV_STALE_DATA: &str = "AI_LLM_EVALUATION_DATASET_INV_STALE_DATA";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvaluationDatasetInventoryItem {
    pub dataset_id: String,
    pub provider_id: String,
    pub model_name: String,
    pub enabled: bool,
    pub owner: Option<String>,
    pub labels: Vec<String>,
    pub sample_count: Option<u64>,
    pub dataset_version: Option<String>,
    pub refresh_policy: Option<String>,
    pub estimated_monthly_cost_usd: Option<f64>,
    pub monthly_budget_usd: Option<f64>,
    pub license_policy: Option<String>,
    pub audit_enabled: bool,
    pub redaction_policy: Option<String>,
    pub updated_at: DateTime<Utc>,
}

pub fn evaluate_evaluation_dataset_inventory(
    items: &[EvaluationDatasetInventoryItem],
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

pub fn evaluation_dataset_inventory_item_from_model(
    model: &LlmProviderModel,
) -> EvaluationDatasetInventoryItem {
    EvaluationDatasetInventoryItem {
        dataset_id: model.id.to_string(),
        provider_id: model.provider_id.to_string(),
        model_name: model.model_name.clone(),
        enabled: model.enabled,
        owner: string_field(&model.model_config, &["owner", "team", "cost_owner"]),
        labels: string_array_field(&model.model_config, "labels"),
        sample_count: integer_field(
            &model.model_config,
            &[
                "evaluation_sample_count",
                "dataset_sample_count",
                "sample_count",
            ],
        ),
        dataset_version: string_field(
            &model.model_config,
            &["evaluation_dataset_version", "dataset_version", "version"],
        ),
        refresh_policy: string_field(
            &model.model_config,
            &[
                "dataset_refresh_policy",
                "refresh_policy",
                "recertification_policy",
            ],
        ),
        estimated_monthly_cost_usd: number_field(
            &model.model_config,
            &[
                "evaluation_dataset_cost_usd",
                "dataset_monthly_cost_usd",
                "monthly_cost_usd",
            ],
        ),
        monthly_budget_usd: number_field(
            &model.model_config,
            &[
                "evaluation_dataset_budget_usd",
                "dataset_budget_usd",
                "monthly_budget_usd",
            ],
        ),
        license_policy: string_field(
            &model.model_config,
            &["dataset_license_policy", "license_policy", "data_license"],
        ),
        audit_enabled: bool_field(
            &model.model_config,
            &[
                "audit_enabled",
                "dataset_audit_enabled",
                "evaluation_audit_enabled",
            ],
        ),
        redaction_policy: string_field(
            &model.model_config,
            &[
                "dataset_redaction_policy",
                "telemetry_redaction_policy",
                "redaction_policy",
                "data_policy",
            ],
        ),
        updated_at: model.updated_at,
    }
}

fn evaluate_cost(
    item: &EvaluationDatasetInventoryItem,
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
                "AI/LLM evaluation dataset {} has no owner metadata",
                item.model_name
            ),
            json!({"dataset_id": item.dataset_id, "labels": item.labels, "checked_keys": COST_ALLOCATION_TAG_KEYS}),
        ));
    }

    if item.monthly_budget_usd.is_none() {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_BUDGET_MISSING,
            Severity::Medium,
            format!(
                "AI/LLM evaluation dataset {} has no budget",
                item.model_name
            ),
            json!({"dataset_id": item.dataset_id, "recommendation": "Record evaluation dataset storage, labeling, and refresh budget before quality gates are trusted"}),
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
                "AI/LLM evaluation dataset {} is over budget",
                item.model_name
            ),
            json!({"dataset_id": item.dataset_id, "estimated_monthly_cost_usd": item.estimated_monthly_cost_usd, "monthly_budget_usd": item.monthly_budget_usd}),
        ));
    }
}

fn evaluate_resilience(
    item: &EvaluationDatasetInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if item.sample_count.unwrap_or(0) == 0 {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_SAMPLE_MISSING,
            Severity::High,
            format!(
                "AI/LLM evaluation dataset {} has no usable samples",
                item.model_name
            ),
            json!({"dataset_id": item.dataset_id, "sample_count": item.sample_count}),
        ));
    }

    if item
        .dataset_version
        .as_deref()
        .unwrap_or_default()
        .trim()
        .is_empty()
    {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_VERSION_MISSING,
            Severity::High,
            format!(
                "AI/LLM evaluation dataset {} has no version metadata",
                item.model_name
            ),
            json!({"dataset_id": item.dataset_id}),
        ));
    }

    if item
        .refresh_policy
        .as_deref()
        .unwrap_or_default()
        .trim()
        .is_empty()
    {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_REFRESH_POLICY_MISSING,
            Severity::Medium,
            format!(
                "AI/LLM evaluation dataset {} has no refresh policy",
                item.model_name
            ),
            json!({"dataset_id": item.dataset_id, "recommendation": "Record dataset refresh or recertification cadence so stale quality gates are detectable"}),
        ));
    }
}

fn evaluate_security(
    item: &EvaluationDatasetInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if item
        .license_policy
        .as_deref()
        .unwrap_or_default()
        .trim()
        .is_empty()
    {
        findings.push(finding(
            item,
            pillar,
            REASON_SEC_LICENSE_MISSING,
            Severity::High,
            format!(
                "AI/LLM evaluation dataset {} has no license policy",
                item.model_name
            ),
            json!({"dataset_id": item.dataset_id}),
        ));
    }

    if !item.audit_enabled {
        findings.push(finding(
            item,
            pillar,
            REASON_SEC_AUDIT_MISSING,
            Severity::High,
            format!(
                "AI/LLM evaluation dataset {} has no audit trail",
                item.model_name
            ),
            json!({"dataset_id": item.dataset_id}),
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
                "AI/LLM evaluation dataset {} has no redaction policy",
                item.model_name
            ),
            json!({"dataset_id": item.dataset_id}),
        ));
    }
}

fn stale_finding(
    item: &EvaluationDatasetInventoryItem,
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
            "AI/LLM evaluation dataset {} inventory data is stale by {} hours",
            item.model_name, age_hours
        ),
        json!({"dataset_id": item.dataset_id, "age_hours": age_hours, "stale_after_hours": DEFAULT_STALE_AFTER_HOURS}),
    ))
}

fn finding(
    item: &EvaluationDatasetInventoryItem,
    pillar: Pillar,
    reason_code: &str,
    severity: Severity,
    message: String,
    evidence: Value,
) -> InventoryFinding {
    InventoryFinding {
        resource_id: item.dataset_id.clone(),
        arn: format!("ai-llm-evaluation-dataset/{}", item.dataset_id),
        pillar,
        reason_code: reason_code.to_string(),
        severity,
        message,
        evidence,
    }
}

fn has_owner_metadata(item: &EvaluationDatasetInventoryItem) -> bool {
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

    fn item() -> EvaluationDatasetInventoryItem {
        EvaluationDatasetInventoryItem {
            dataset_id: "dataset-1".to_string(),
            provider_id: "provider-1".to_string(),
            model_name: "quality-eval".to_string(),
            enabled: true,
            owner: Some("sre-ai".to_string()),
            labels: vec!["cost-center=ai-platform".to_string()],
            sample_count: Some(5000),
            dataset_version: Some("2026-06".to_string()),
            refresh_policy: Some("monthly recertification".to_string()),
            estimated_monthly_cost_usd: Some(80.0),
            monthly_budget_usd: Some(120.0),
            license_policy: Some("internal approved data only".to_string()),
            audit_enabled: true,
            redaction_policy: Some("redact secrets and customer identifiers".to_string()),
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
    fn healthy_evaluation_dataset_inventory_passes_claimed_pillars() {
        let now = Utc::now();
        for pillar in [Pillar::Cost, Pillar::Resilience, Pillar::Security] {
            let report = evaluate_evaluation_dataset_inventory(&[item()], pillar, now);
            assert_eq!(report.resources_evaluated, 1);
            assert!(report.findings.is_empty());
        }
    }

    #[test]
    fn cost_flags_missing_owner_budget_and_overage() {
        let mut item = item();
        item.owner = None;
        item.labels.clear();
        item.estimated_monthly_cost_usd = Some(200.0);

        let report = evaluate_evaluation_dataset_inventory(&[item], Pillar::Cost, Utc::now());
        let codes = codes(&report);

        assert!(codes.contains(&REASON_COST_OWNER_NOT_RECORDED.to_string()));
        assert!(codes.contains(&REASON_COST_ESTIMATE_OVER_BUDGET.to_string()));
    }

    #[test]
    fn resilience_flags_missing_sample_version_and_refresh_policy() {
        let mut item = item();
        item.sample_count = Some(0);
        item.dataset_version = None;
        item.refresh_policy = None;

        let report = evaluate_evaluation_dataset_inventory(&[item], Pillar::Resilience, Utc::now());
        let codes = codes(&report);

        assert!(codes.contains(&REASON_RES_SAMPLE_MISSING.to_string()));
        assert!(codes.contains(&REASON_RES_VERSION_MISSING.to_string()));
        assert!(codes.contains(&REASON_RES_REFRESH_POLICY_MISSING.to_string()));
    }

    #[test]
    fn security_flags_missing_license_audit_and_redaction() {
        let mut item = item();
        item.license_policy = None;
        item.audit_enabled = false;
        item.redaction_policy = None;

        let report = evaluate_evaluation_dataset_inventory(&[item], Pillar::Security, Utc::now());
        let codes = codes(&report);

        assert!(codes.contains(&REASON_SEC_LICENSE_MISSING.to_string()));
        assert!(codes.contains(&REASON_SEC_AUDIT_MISSING.to_string()));
        assert!(codes.contains(&REASON_SEC_REDACTION_MISSING.to_string()));
    }

    #[test]
    fn stale_evaluation_dataset_inventory_is_counted_for_any_pillar() {
        let mut item = item();
        item.updated_at = Utc::now() - Duration::hours(DEFAULT_STALE_AFTER_HOURS + 1);

        let report = evaluate_evaluation_dataset_inventory(&[item], Pillar::Cost, Utc::now());

        assert_eq!(report.stale_resources, 1);
        assert!(codes(&report).contains(&REASON_INV_STALE_DATA.to_string()));
    }
}

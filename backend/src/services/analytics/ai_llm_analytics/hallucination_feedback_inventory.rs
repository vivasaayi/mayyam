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

// Deterministic AI/LLM hallucination feedback inventory evaluator for roadmap
// rows 25-AI-LLM-OBSERVABILITY-00540/00547/00568.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::models::llm_model::Model as LlmProviderModel;
use crate::services::aws::inventory::types::{
    score_pillar, InventoryFinding, Pillar, PillarReport, Severity, COST_ALLOCATION_TAG_KEYS,
    DEFAULT_STALE_AFTER_HOURS,
};

pub const RESOURCE_TYPE: &str = "AiLlmHallucinationFeedback";
pub const REASON_COST_OWNER_NOT_RECORDED: &str =
    "AI_LLM_HALLUCINATION_FEEDBACK_COST_OWNER_NOT_RECORDED";
pub const REASON_COST_BUDGET_MISSING: &str = "AI_LLM_HALLUCINATION_FEEDBACK_COST_BUDGET_MISSING";
pub const REASON_COST_ESTIMATE_OVER_BUDGET: &str =
    "AI_LLM_HALLUCINATION_FEEDBACK_COST_ESTIMATE_OVER_BUDGET";
pub const REASON_RES_FEEDBACK_RATE_MISSING: &str =
    "AI_LLM_HALLUCINATION_FEEDBACK_RES_FEEDBACK_RATE_MISSING";
pub const REASON_RES_FEEDBACK_RATE_ABOVE_THRESHOLD: &str =
    "AI_LLM_HALLUCINATION_FEEDBACK_RES_FEEDBACK_RATE_ABOVE_THRESHOLD";
pub const REASON_RES_SAMPLE_MISSING: &str = "AI_LLM_HALLUCINATION_FEEDBACK_RES_SAMPLE_MISSING";
pub const REASON_RES_WINDOW_MISSING: &str = "AI_LLM_HALLUCINATION_FEEDBACK_RES_WINDOW_MISSING";
pub const REASON_SEC_AUDIT_MISSING: &str = "AI_LLM_HALLUCINATION_FEEDBACK_SEC_AUDIT_MISSING";
pub const REASON_SEC_REVIEW_POLICY_MISSING: &str =
    "AI_LLM_HALLUCINATION_FEEDBACK_SEC_REVIEW_POLICY_MISSING";
pub const REASON_SEC_TAMPER_EVIDENCE_MISSING: &str =
    "AI_LLM_HALLUCINATION_FEEDBACK_SEC_TAMPER_EVIDENCE_MISSING";
pub const REASON_INV_STALE_DATA: &str = "AI_LLM_HALLUCINATION_FEEDBACK_INV_STALE_DATA";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HallucinationFeedbackInventoryItem {
    pub feedback_id: String,
    pub provider_id: String,
    pub model_name: String,
    pub enabled: bool,
    pub owner: Option<String>,
    pub labels: Vec<String>,
    pub feedback_rate: Option<f64>,
    pub maximum_feedback_rate: Option<f64>,
    pub sample_count: Option<u64>,
    pub review_window: Option<String>,
    pub estimated_monthly_cost_usd: Option<f64>,
    pub monthly_budget_usd: Option<f64>,
    pub audit_enabled: bool,
    pub review_policy: Option<String>,
    pub tamper_evidence_enabled: bool,
    pub updated_at: DateTime<Utc>,
}

pub fn evaluate_hallucination_feedback_inventory(
    items: &[HallucinationFeedbackInventoryItem],
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

pub fn hallucination_feedback_inventory_item_from_model(
    model: &LlmProviderModel,
) -> HallucinationFeedbackInventoryItem {
    HallucinationFeedbackInventoryItem {
        feedback_id: model.id.to_string(),
        provider_id: model.provider_id.to_string(),
        model_name: model.model_name.clone(),
        enabled: model.enabled,
        owner: string_field(&model.model_config, &["owner", "team", "cost_owner"]),
        labels: string_array_field(&model.model_config, "labels"),
        feedback_rate: number_field(
            &model.model_config,
            &[
                "hallucination_feedback_rate",
                "hallucination_rate",
                "false_answer_feedback_rate",
            ],
        ),
        maximum_feedback_rate: number_field(
            &model.model_config,
            &[
                "maximum_hallucination_feedback_rate",
                "maximum_hallucination_rate",
                "hallucination_feedback_threshold",
            ],
        ),
        sample_count: integer_field(
            &model.model_config,
            &[
                "hallucination_feedback_sample_count",
                "hallucination_sample_count",
                "sample_count",
            ],
        ),
        review_window: string_field(
            &model.model_config,
            &[
                "hallucination_feedback_window",
                "hallucination_review_window",
                "review_window",
            ],
        ),
        estimated_monthly_cost_usd: number_field(
            &model.model_config,
            &[
                "hallucination_feedback_cost_usd",
                "hallucination_review_cost_usd",
                "monthly_cost_usd",
            ],
        ),
        monthly_budget_usd: number_field(
            &model.model_config,
            &[
                "hallucination_feedback_budget_usd",
                "hallucination_review_budget_usd",
                "monthly_budget_usd",
            ],
        ),
        audit_enabled: bool_field(
            &model.model_config,
            &[
                "hallucination_feedback_audit_enabled",
                "hallucination_audit_enabled",
                "audit_enabled",
            ],
        ),
        review_policy: string_field(
            &model.model_config,
            &[
                "hallucination_feedback_review_policy",
                "hallucination_review_policy",
                "review_policy",
            ],
        ),
        tamper_evidence_enabled: bool_field(
            &model.model_config,
            &[
                "hallucination_feedback_tamper_evidence_enabled",
                "hallucination_tamper_evidence_enabled",
                "tamper_evidence_enabled",
            ],
        ),
        updated_at: model.updated_at,
    }
}

fn evaluate_cost(
    item: &HallucinationFeedbackInventoryItem,
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
                "AI/LLM hallucination feedback {} has no owner metadata",
                item.model_name
            ),
            json!({"feedback_id": item.feedback_id, "labels": item.labels, "checked_keys": COST_ALLOCATION_TAG_KEYS}),
        ));
    }

    if item.monthly_budget_usd.is_none() {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_BUDGET_MISSING,
            Severity::Medium,
            format!(
                "AI/LLM hallucination feedback {} has no budget",
                item.model_name
            ),
            json!({"feedback_id": item.feedback_id, "recommendation": "Record review, labeling, and storage budget before hallucination feedback gates are trusted"}),
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
                "AI/LLM hallucination feedback {} is over budget",
                item.model_name
            ),
            json!({"feedback_id": item.feedback_id, "estimated_monthly_cost_usd": item.estimated_monthly_cost_usd, "monthly_budget_usd": item.monthly_budget_usd}),
        ));
    }
}

fn evaluate_resilience(
    item: &HallucinationFeedbackInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    match (item.feedback_rate, item.maximum_feedback_rate) {
        (None, _) => findings.push(finding(
            item,
            pillar,
            REASON_RES_FEEDBACK_RATE_MISSING,
            Severity::High,
            format!(
                "AI/LLM hallucination feedback {} has no measured feedback rate",
                item.model_name
            ),
            json!({"feedback_id": item.feedback_id, "recommendation": "Record hallucination feedback rate before using this gate for quality decisions"}),
        )),
        (Some(rate), Some(maximum)) if rate > maximum => findings.push(finding(
            item,
            pillar,
            REASON_RES_FEEDBACK_RATE_ABOVE_THRESHOLD,
            Severity::High,
            format!(
                "AI/LLM hallucination feedback {} is above threshold",
                item.model_name
            ),
            json!({"feedback_id": item.feedback_id, "feedback_rate": rate, "maximum_feedback_rate": maximum}),
        )),
        _ => {}
    }

    if item.sample_count.unwrap_or(0) == 0 {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_SAMPLE_MISSING,
            Severity::High,
            format!(
                "AI/LLM hallucination feedback {} has no sample evidence",
                item.model_name
            ),
            json!({"feedback_id": item.feedback_id, "recommendation": "Retain sampled feedback evidence before quality gates are trusted"}),
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
            REASON_RES_WINDOW_MISSING,
            Severity::Medium,
            format!(
                "AI/LLM hallucination feedback {} has no review window",
                item.model_name
            ),
            json!({"feedback_id": item.feedback_id, "recommendation": "Record the rolling review window so drift and regressions are detectable"}),
        ));
    }
}

fn evaluate_security(
    item: &HallucinationFeedbackInventoryItem,
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
                "AI/LLM hallucination feedback {} has no audit trail",
                item.model_name
            ),
            json!({"feedback_id": item.feedback_id, "recommendation": "Record audit evidence for feedback ingestion, suppression, and threshold changes"}),
        ));
    }

    if item
        .review_policy
        .as_deref()
        .unwrap_or_default()
        .trim()
        .is_empty()
    {
        findings.push(finding(
            item,
            pillar,
            REASON_SEC_REVIEW_POLICY_MISSING,
            Severity::High,
            format!(
                "AI/LLM hallucination feedback {} has no review policy",
                item.model_name
            ),
            json!({"feedback_id": item.feedback_id, "recommendation": "Require documented reviewer and suppression policy before escalating hallucination feedback"}),
        ));
    }

    if !item.tamper_evidence_enabled {
        findings.push(finding(
            item,
            pillar,
            REASON_SEC_TAMPER_EVIDENCE_MISSING,
            Severity::Medium,
            format!(
                "AI/LLM hallucination feedback {} has no tamper evidence",
                item.model_name
            ),
            json!({"feedback_id": item.feedback_id, "recommendation": "Enable immutable or tamper-evident storage for feedback signals that drive quality gates"}),
        ));
    }
}

fn stale_finding(
    item: &HallucinationFeedbackInventoryItem,
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
            "AI/LLM hallucination feedback {} inventory data is stale by {} hours",
            item.model_name, age_hours
        ),
        json!({"feedback_id": item.feedback_id, "age_hours": age_hours, "stale_after_hours": DEFAULT_STALE_AFTER_HOURS}),
    ))
}

fn finding(
    item: &HallucinationFeedbackInventoryItem,
    pillar: Pillar,
    reason_code: &str,
    severity: Severity,
    message: String,
    evidence: Value,
) -> InventoryFinding {
    InventoryFinding {
        resource_id: item.feedback_id.clone(),
        arn: format!("ai-llm-hallucination-feedback/{}", item.feedback_id),
        pillar,
        reason_code: reason_code.to_string(),
        severity,
        message,
        evidence,
    }
}

fn has_owner_metadata(item: &HallucinationFeedbackInventoryItem) -> bool {
    item.owner
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .is_some()
        || item.labels.iter().any(|label| {
            let normalized = label.to_ascii_lowercase();
            normalized.starts_with("owner=")
                || normalized.starts_with("team=")
                || normalized.starts_with("cost-center=")
        })
}

fn string_field(config: &Value, keys: &[&str]) -> Option<String> {
    keys.iter().find_map(|key| {
        config
            .get(key)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned)
    })
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
                .map(ToOwned::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

fn number_field(config: &Value, keys: &[&str]) -> Option<f64> {
    keys.iter()
        .find_map(|key| config.get(key).and_then(Value::as_f64))
}

fn integer_field(config: &Value, keys: &[&str]) -> Option<u64> {
    keys.iter()
        .find_map(|key| config.get(key).and_then(Value::as_u64))
}

fn bool_field(config: &Value, keys: &[&str]) -> bool {
    keys.iter()
        .find_map(|key| config.get(key).and_then(Value::as_bool))
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    fn item() -> HallucinationFeedbackInventoryItem {
        HallucinationFeedbackInventoryItem {
            feedback_id: "feedback-1".to_string(),
            provider_id: "provider-1".to_string(),
            model_name: "support-answer-model".to_string(),
            enabled: true,
            owner: Some("sre-ai".to_string()),
            labels: vec!["cost-center=ai-platform".to_string()],
            feedback_rate: Some(0.04),
            maximum_feedback_rate: Some(0.08),
            sample_count: Some(800),
            review_window: Some("rolling 7d".to_string()),
            estimated_monthly_cost_usd: Some(40.0),
            monthly_budget_usd: Some(100.0),
            audit_enabled: true,
            review_policy: Some("review flagged answers before suppression".to_string()),
            tamper_evidence_enabled: true,
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
    fn healthy_hallucination_feedback_inventory_passes_claimed_pillars() {
        let now = Utc::now();
        for pillar in [Pillar::Cost, Pillar::Resilience, Pillar::Security] {
            let report = evaluate_hallucination_feedback_inventory(&[item()], pillar, now);
            assert_eq!(report.resources_evaluated, 1);
            assert!(report.findings.is_empty());
        }
    }

    #[test]
    fn cost_flags_missing_owner_budget_and_over_budget() {
        let mut item = item();
        item.owner = None;
        item.labels.clear();
        item.monthly_budget_usd = None;
        item.estimated_monthly_cost_usd = Some(200.0);

        let report = evaluate_hallucination_feedback_inventory(&[item], Pillar::Cost, Utc::now());
        let codes = codes(&report);

        assert!(codes.contains(&REASON_COST_OWNER_NOT_RECORDED.to_string()));
        assert!(codes.contains(&REASON_COST_BUDGET_MISSING.to_string()));
    }

    #[test]
    fn resilience_flags_missing_rate_sample_and_window() {
        let mut item = item();
        item.feedback_rate = None;
        item.sample_count = None;
        item.review_window = None;

        let report =
            evaluate_hallucination_feedback_inventory(&[item], Pillar::Resilience, Utc::now());
        let codes = codes(&report);

        assert!(codes.contains(&REASON_RES_FEEDBACK_RATE_MISSING.to_string()));
        assert!(codes.contains(&REASON_RES_SAMPLE_MISSING.to_string()));
        assert!(codes.contains(&REASON_RES_WINDOW_MISSING.to_string()));
    }

    #[test]
    fn security_flags_missing_audit_review_and_tamper_evidence() {
        let mut item = item();
        item.audit_enabled = false;
        item.review_policy = None;
        item.tamper_evidence_enabled = false;

        let report =
            evaluate_hallucination_feedback_inventory(&[item], Pillar::Security, Utc::now());
        let codes = codes(&report);

        assert!(codes.contains(&REASON_SEC_AUDIT_MISSING.to_string()));
        assert!(codes.contains(&REASON_SEC_REVIEW_POLICY_MISSING.to_string()));
        assert!(codes.contains(&REASON_SEC_TAMPER_EVIDENCE_MISSING.to_string()));
    }

    #[test]
    fn stale_hallucination_feedback_inventory_is_counted_for_any_pillar() {
        let mut item = item();
        item.updated_at = Utc::now() - Duration::hours(DEFAULT_STALE_AFTER_HOURS + 1);

        let report = evaluate_hallucination_feedback_inventory(&[item], Pillar::Cost, Utc::now());

        assert_eq!(report.stale_resources, 1);
        assert!(codes(&report).contains(&REASON_INV_STALE_DATA.to_string()));
    }
}

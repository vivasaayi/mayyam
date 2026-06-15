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

// Deterministic AI/LLM response quality score inventory evaluator for roadmap
// rows 25-AI-LLM-OBSERVABILITY-00442/00449/00470.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::models::llm_model::Model as LlmProviderModel;
use crate::services::aws::inventory::types::{
    score_pillar, InventoryFinding, Pillar, PillarReport, Severity, COST_ALLOCATION_TAG_KEYS,
    DEFAULT_STALE_AFTER_HOURS,
};

pub const RESOURCE_TYPE: &str = "AiLlmResponseQualityScore";
pub const REASON_COST_OWNER_NOT_RECORDED: &str =
    "AI_LLM_RESPONSE_QUALITY_SCORE_COST_OWNER_NOT_RECORDED";
pub const REASON_COST_BUDGET_MISSING: &str = "AI_LLM_RESPONSE_QUALITY_SCORE_COST_BUDGET_MISSING";
pub const REASON_COST_ESTIMATE_OVER_BUDGET: &str =
    "AI_LLM_RESPONSE_QUALITY_SCORE_COST_ESTIMATE_OVER_BUDGET";
pub const REASON_RES_SCORE_MISSING: &str = "AI_LLM_RESPONSE_QUALITY_SCORE_RES_SCORE_MISSING";
pub const REASON_RES_SCORE_BELOW_THRESHOLD: &str =
    "AI_LLM_RESPONSE_QUALITY_SCORE_RES_SCORE_BELOW_THRESHOLD";
pub const REASON_RES_SAMPLE_MISSING: &str = "AI_LLM_RESPONSE_QUALITY_SCORE_RES_SAMPLE_MISSING";
pub const REASON_RES_SCORER_VERSION_MISSING: &str =
    "AI_LLM_RESPONSE_QUALITY_SCORE_RES_SCORER_VERSION_MISSING";
pub const REASON_RES_SCORING_WINDOW_MISSING: &str =
    "AI_LLM_RESPONSE_QUALITY_SCORE_RES_SCORING_WINDOW_MISSING";
pub const REASON_SEC_AUDIT_MISSING: &str = "AI_LLM_RESPONSE_QUALITY_SCORE_SEC_AUDIT_MISSING";
pub const REASON_SEC_PII_REVIEW_MISSING: &str =
    "AI_LLM_RESPONSE_QUALITY_SCORE_SEC_PII_REVIEW_MISSING";
pub const REASON_SEC_TAMPER_EVIDENCE_MISSING: &str =
    "AI_LLM_RESPONSE_QUALITY_SCORE_SEC_TAMPER_EVIDENCE_MISSING";
pub const REASON_INV_STALE_DATA: &str = "AI_LLM_RESPONSE_QUALITY_SCORE_INV_STALE_DATA";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponseQualityScoreInventoryItem {
    pub score_id: String,
    pub provider_id: String,
    pub model_name: String,
    pub enabled: bool,
    pub owner: Option<String>,
    pub labels: Vec<String>,
    pub quality_score: Option<f64>,
    pub minimum_quality_score: Option<f64>,
    pub sample_count: Option<u64>,
    pub scorer_version: Option<String>,
    pub scoring_window: Option<String>,
    pub estimated_monthly_cost_usd: Option<f64>,
    pub monthly_budget_usd: Option<f64>,
    pub audit_enabled: bool,
    pub pii_review_policy: Option<String>,
    pub tamper_evidence_enabled: bool,
    pub updated_at: DateTime<Utc>,
}

pub fn evaluate_response_quality_score_inventory(
    items: &[ResponseQualityScoreInventoryItem],
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

pub fn response_quality_score_inventory_item_from_model(
    model: &LlmProviderModel,
) -> ResponseQualityScoreInventoryItem {
    ResponseQualityScoreInventoryItem {
        score_id: model.id.to_string(),
        provider_id: model.provider_id.to_string(),
        model_name: model.model_name.clone(),
        enabled: model.enabled,
        owner: string_field(&model.model_config, &["owner", "team", "cost_owner"]),
        labels: string_array_field(&model.model_config, "labels"),
        quality_score: number_field(
            &model.model_config,
            &[
                "response_quality_score",
                "quality_score",
                "answer_quality_score",
            ],
        ),
        minimum_quality_score: number_field(
            &model.model_config,
            &[
                "minimum_response_quality_score",
                "minimum_quality_score",
                "quality_score_threshold",
            ],
        ),
        sample_count: integer_field(
            &model.model_config,
            &[
                "response_quality_sample_count",
                "quality_score_sample_count",
                "sample_count",
            ],
        ),
        scorer_version: string_field(
            &model.model_config,
            &[
                "response_quality_scorer_version",
                "quality_scorer_version",
                "scorer_version",
            ],
        ),
        scoring_window: string_field(
            &model.model_config,
            &[
                "response_quality_scoring_window",
                "quality_scoring_window",
                "scoring_window",
            ],
        ),
        estimated_monthly_cost_usd: number_field(
            &model.model_config,
            &[
                "response_quality_cost_usd",
                "quality_score_monthly_cost_usd",
                "monthly_cost_usd",
            ],
        ),
        monthly_budget_usd: number_field(
            &model.model_config,
            &[
                "response_quality_budget_usd",
                "quality_score_budget_usd",
                "monthly_budget_usd",
            ],
        ),
        audit_enabled: bool_field(
            &model.model_config,
            &[
                "response_quality_audit_enabled",
                "quality_score_audit_enabled",
                "audit_enabled",
            ],
        ),
        pii_review_policy: string_field(
            &model.model_config,
            &[
                "response_quality_pii_review_policy",
                "quality_score_pii_review_policy",
                "pii_review_policy",
            ],
        ),
        tamper_evidence_enabled: bool_field(
            &model.model_config,
            &[
                "response_quality_tamper_evidence_enabled",
                "quality_score_tamper_evidence_enabled",
                "tamper_evidence_enabled",
            ],
        ),
        updated_at: model.updated_at,
    }
}

fn evaluate_cost(
    item: &ResponseQualityScoreInventoryItem,
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
                "AI/LLM response quality score {} has no owner metadata",
                item.model_name
            ),
            json!({"score_id": item.score_id, "labels": item.labels, "checked_keys": COST_ALLOCATION_TAG_KEYS}),
        ));
    }

    if item.monthly_budget_usd.is_none() {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_BUDGET_MISSING,
            Severity::Medium,
            format!("AI/LLM response quality score {} has no budget", item.model_name),
            json!({"score_id": item.score_id, "recommendation": "Record evaluator, sampling, storage, and review budget before response quality gates are trusted"}),
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
                "AI/LLM response quality score {} is over budget",
                item.model_name
            ),
            json!({"score_id": item.score_id, "estimated_monthly_cost_usd": item.estimated_monthly_cost_usd, "monthly_budget_usd": item.monthly_budget_usd}),
        ));
    }
}

fn evaluate_resilience(
    item: &ResponseQualityScoreInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    match (item.quality_score, item.minimum_quality_score) {
        (None, _) => findings.push(finding(
            item,
            pillar,
            REASON_RES_SCORE_MISSING,
            Severity::High,
            format!(
                "AI/LLM response quality score {} has no score value",
                item.model_name
            ),
            json!({"score_id": item.score_id}),
        )),
        (Some(score), Some(minimum)) if score < minimum => findings.push(finding(
            item,
            pillar,
            REASON_RES_SCORE_BELOW_THRESHOLD,
            Severity::High,
            format!(
                "AI/LLM response quality score {} is below threshold",
                item.model_name
            ),
            json!({"score_id": item.score_id, "quality_score": score, "minimum_quality_score": minimum}),
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
                "AI/LLM response quality score {} has no sample evidence",
                item.model_name
            ),
            json!({"score_id": item.score_id, "sample_count": item.sample_count}),
        ));
    }

    if item
        .scorer_version
        .as_deref()
        .unwrap_or_default()
        .trim()
        .is_empty()
    {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_SCORER_VERSION_MISSING,
            Severity::Medium,
            format!(
                "AI/LLM response quality score {} has no scorer version",
                item.model_name
            ),
            json!({"score_id": item.score_id}),
        ));
    }

    if item
        .scoring_window
        .as_deref()
        .unwrap_or_default()
        .trim()
        .is_empty()
    {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_SCORING_WINDOW_MISSING,
            Severity::Medium,
            format!(
                "AI/LLM response quality score {} has no scoring window",
                item.model_name
            ),
            json!({"score_id": item.score_id, "recommendation": "Record the scoring lookback window so regressions can be compared deterministically"}),
        ));
    }
}

fn evaluate_security(
    item: &ResponseQualityScoreInventoryItem,
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
                "AI/LLM response quality score {} has no audit trail",
                item.model_name
            ),
            json!({"score_id": item.score_id}),
        ));
    }

    if item
        .pii_review_policy
        .as_deref()
        .unwrap_or_default()
        .trim()
        .is_empty()
    {
        findings.push(finding(
            item,
            pillar,
            REASON_SEC_PII_REVIEW_MISSING,
            Severity::High,
            format!(
                "AI/LLM response quality score {} has no PII review policy",
                item.model_name
            ),
            json!({"score_id": item.score_id}),
        ));
    }

    if !item.tamper_evidence_enabled {
        findings.push(finding(
            item,
            pillar,
            REASON_SEC_TAMPER_EVIDENCE_MISSING,
            Severity::High,
            format!(
                "AI/LLM response quality score {} has no tamper evidence",
                item.model_name
            ),
            json!({"score_id": item.score_id}),
        ));
    }
}

fn stale_finding(
    item: &ResponseQualityScoreInventoryItem,
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
            "AI/LLM response quality score {} inventory data is stale by {} hours",
            item.model_name, age_hours
        ),
        json!({"score_id": item.score_id, "age_hours": age_hours, "stale_after_hours": DEFAULT_STALE_AFTER_HOURS}),
    ))
}

fn finding(
    item: &ResponseQualityScoreInventoryItem,
    pillar: Pillar,
    reason_code: &str,
    severity: Severity,
    message: String,
    evidence: Value,
) -> InventoryFinding {
    InventoryFinding {
        resource_id: item.score_id.clone(),
        arn: format!("ai-llm-response-quality-score/{}", item.score_id),
        pillar,
        reason_code: reason_code.to_string(),
        severity,
        message,
        evidence,
    }
}

fn has_owner_metadata(item: &ResponseQualityScoreInventoryItem) -> bool {
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

    fn item() -> ResponseQualityScoreInventoryItem {
        ResponseQualityScoreInventoryItem {
            score_id: "quality-score-1".to_string(),
            provider_id: "provider-1".to_string(),
            model_name: "support-answer-model".to_string(),
            enabled: true,
            owner: Some("sre-ai".to_string()),
            labels: vec!["cost-center=ai-platform".to_string()],
            quality_score: Some(0.94),
            minimum_quality_score: Some(0.90),
            sample_count: Some(1200),
            scorer_version: Some("rubric-2026-06".to_string()),
            scoring_window: Some("rolling 7d".to_string()),
            estimated_monthly_cost_usd: Some(60.0),
            monthly_budget_usd: Some(100.0),
            audit_enabled: true,
            pii_review_policy: Some("review and redact customer identifiers".to_string()),
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
    fn healthy_response_quality_score_inventory_passes_claimed_pillars() {
        let now = Utc::now();
        for pillar in [Pillar::Cost, Pillar::Resilience, Pillar::Security] {
            let report = evaluate_response_quality_score_inventory(&[item()], pillar, now);
            assert_eq!(report.resources_evaluated, 1);
            assert!(report.findings.is_empty());
        }
    }

    #[test]
    fn cost_flags_missing_owner_budget_and_overage() {
        let mut item = item();
        item.owner = None;
        item.labels.clear();
        item.monthly_budget_usd = None;
        item.estimated_monthly_cost_usd = Some(200.0);

        let report = evaluate_response_quality_score_inventory(&[item], Pillar::Cost, Utc::now());
        let codes = codes(&report);

        assert!(codes.contains(&REASON_COST_OWNER_NOT_RECORDED.to_string()));
        assert!(codes.contains(&REASON_COST_BUDGET_MISSING.to_string()));
    }

    #[test]
    fn resilience_flags_missing_and_below_threshold_score_evidence() {
        let mut missing = item();
        missing.quality_score = None;
        missing.sample_count = Some(0);
        missing.scorer_version = None;
        missing.scoring_window = None;

        let report =
            evaluate_response_quality_score_inventory(&[missing], Pillar::Resilience, Utc::now());
        let reason_codes = codes(&report);
        assert!(reason_codes.contains(&REASON_RES_SCORE_MISSING.to_string()));
        assert!(reason_codes.contains(&REASON_RES_SAMPLE_MISSING.to_string()));
        assert!(reason_codes.contains(&REASON_RES_SCORER_VERSION_MISSING.to_string()));
        assert!(reason_codes.contains(&REASON_RES_SCORING_WINDOW_MISSING.to_string()));

        let mut below_threshold = item();
        below_threshold.quality_score = Some(0.72);
        let report = evaluate_response_quality_score_inventory(
            &[below_threshold],
            Pillar::Resilience,
            Utc::now(),
        );
        assert!(codes(&report).contains(&REASON_RES_SCORE_BELOW_THRESHOLD.to_string()));
    }

    #[test]
    fn security_flags_missing_audit_pii_review_and_tamper_evidence() {
        let mut item = item();
        item.audit_enabled = false;
        item.pii_review_policy = None;
        item.tamper_evidence_enabled = false;

        let report =
            evaluate_response_quality_score_inventory(&[item], Pillar::Security, Utc::now());
        let codes = codes(&report);

        assert!(codes.contains(&REASON_SEC_AUDIT_MISSING.to_string()));
        assert!(codes.contains(&REASON_SEC_PII_REVIEW_MISSING.to_string()));
        assert!(codes.contains(&REASON_SEC_TAMPER_EVIDENCE_MISSING.to_string()));
    }

    #[test]
    fn stale_response_quality_score_inventory_is_counted_for_any_pillar() {
        let mut item = item();
        item.updated_at = Utc::now() - Duration::hours(DEFAULT_STALE_AFTER_HOURS + 1);

        let report = evaluate_response_quality_score_inventory(&[item], Pillar::Cost, Utc::now());

        assert_eq!(report.stale_resources, 1);
        assert!(codes(&report).contains(&REASON_INV_STALE_DATA.to_string()));
    }
}

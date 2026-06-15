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

// Deterministic AI/LLM grounding score inventory evaluator for roadmap rows
// 25-AI-LLM-OBSERVABILITY-00491/00498/00519.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::models::llm_model::Model as LlmProviderModel;
use crate::services::aws::inventory::types::{
    score_pillar, InventoryFinding, Pillar, PillarReport, Severity, COST_ALLOCATION_TAG_KEYS,
    DEFAULT_STALE_AFTER_HOURS,
};

pub const RESOURCE_TYPE: &str = "AiLlmGroundingScore";
pub const REASON_COST_OWNER_NOT_RECORDED: &str = "AI_LLM_GROUNDING_SCORE_COST_OWNER_NOT_RECORDED";
pub const REASON_COST_BUDGET_MISSING: &str = "AI_LLM_GROUNDING_SCORE_COST_BUDGET_MISSING";
pub const REASON_COST_ESTIMATE_OVER_BUDGET: &str =
    "AI_LLM_GROUNDING_SCORE_COST_ESTIMATE_OVER_BUDGET";
pub const REASON_RES_SCORE_MISSING: &str = "AI_LLM_GROUNDING_SCORE_RES_SCORE_MISSING";
pub const REASON_RES_SCORE_BELOW_THRESHOLD: &str =
    "AI_LLM_GROUNDING_SCORE_RES_SCORE_BELOW_THRESHOLD";
pub const REASON_RES_SAMPLE_MISSING: &str = "AI_LLM_GROUNDING_SCORE_RES_SAMPLE_MISSING";
pub const REASON_RES_SOURCE_COVERAGE_MISSING: &str =
    "AI_LLM_GROUNDING_SCORE_RES_SOURCE_COVERAGE_MISSING";
pub const REASON_RES_SCORER_VERSION_MISSING: &str =
    "AI_LLM_GROUNDING_SCORE_RES_SCORER_VERSION_MISSING";
pub const REASON_SEC_AUDIT_MISSING: &str = "AI_LLM_GROUNDING_SCORE_SEC_AUDIT_MISSING";
pub const REASON_SEC_SOURCE_ACCESS_POLICY_MISSING: &str =
    "AI_LLM_GROUNDING_SCORE_SEC_SOURCE_ACCESS_POLICY_MISSING";
pub const REASON_SEC_EVIDENCE_RETENTION_MISSING: &str =
    "AI_LLM_GROUNDING_SCORE_SEC_EVIDENCE_RETENTION_MISSING";
pub const REASON_INV_STALE_DATA: &str = "AI_LLM_GROUNDING_SCORE_INV_STALE_DATA";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroundingScoreInventoryItem {
    pub score_id: String,
    pub provider_id: String,
    pub model_name: String,
    pub enabled: bool,
    pub owner: Option<String>,
    pub labels: Vec<String>,
    pub grounding_score: Option<f64>,
    pub minimum_grounding_score: Option<f64>,
    pub citation_coverage: Option<f64>,
    pub sample_count: Option<u64>,
    pub evidence_source_count: Option<u64>,
    pub scorer_version: Option<String>,
    pub scoring_window: Option<String>,
    pub estimated_monthly_cost_usd: Option<f64>,
    pub monthly_budget_usd: Option<f64>,
    pub audit_enabled: bool,
    pub source_access_policy: Option<String>,
    pub evidence_retention_policy: Option<String>,
    pub updated_at: DateTime<Utc>,
}

pub fn evaluate_grounding_score_inventory(
    items: &[GroundingScoreInventoryItem],
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

pub fn grounding_score_inventory_item_from_model(
    model: &LlmProviderModel,
) -> GroundingScoreInventoryItem {
    GroundingScoreInventoryItem {
        score_id: model.id.to_string(),
        provider_id: model.provider_id.to_string(),
        model_name: model.model_name.clone(),
        enabled: model.enabled,
        owner: string_field(&model.model_config, &["owner", "team", "cost_owner"]),
        labels: string_array_field(&model.model_config, "labels"),
        grounding_score: number_field(
            &model.model_config,
            &[
                "grounding_score",
                "response_grounding_score",
                "citation_grounding_score",
            ],
        ),
        minimum_grounding_score: number_field(
            &model.model_config,
            &[
                "minimum_grounding_score",
                "grounding_score_threshold",
                "minimum_response_grounding_score",
            ],
        ),
        citation_coverage: number_field(
            &model.model_config,
            &[
                "citation_coverage",
                "grounding_citation_coverage",
                "source_coverage",
            ],
        ),
        sample_count: integer_field(
            &model.model_config,
            &[
                "grounding_sample_count",
                "grounding_score_sample_count",
                "sample_count",
            ],
        ),
        evidence_source_count: integer_field(
            &model.model_config,
            &[
                "grounding_evidence_source_count",
                "evidence_source_count",
                "source_count",
            ],
        ),
        scorer_version: string_field(
            &model.model_config,
            &[
                "grounding_scorer_version",
                "grounding_score_scorer_version",
                "scorer_version",
            ],
        ),
        scoring_window: string_field(
            &model.model_config,
            &[
                "grounding_scoring_window",
                "grounding_score_scoring_window",
                "scoring_window",
            ],
        ),
        estimated_monthly_cost_usd: number_field(
            &model.model_config,
            &[
                "grounding_score_cost_usd",
                "grounding_monthly_cost_usd",
                "monthly_cost_usd",
            ],
        ),
        monthly_budget_usd: number_field(
            &model.model_config,
            &[
                "grounding_score_budget_usd",
                "grounding_budget_usd",
                "monthly_budget_usd",
            ],
        ),
        audit_enabled: bool_field(
            &model.model_config,
            &[
                "grounding_audit_enabled",
                "grounding_score_audit_enabled",
                "audit_enabled",
            ],
        ),
        source_access_policy: string_field(
            &model.model_config,
            &[
                "grounding_source_access_policy",
                "source_access_policy",
                "retrieval_access_policy",
            ],
        ),
        evidence_retention_policy: string_field(
            &model.model_config,
            &[
                "grounding_evidence_retention_policy",
                "evidence_retention_policy",
                "source_retention_policy",
            ],
        ),
        updated_at: model.updated_at,
    }
}

fn evaluate_cost(
    item: &GroundingScoreInventoryItem,
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
                "AI/LLM grounding score {} has no owner metadata",
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
            format!("AI/LLM grounding score {} has no budget", item.model_name),
            json!({"score_id": item.score_id, "recommendation": "Record retrieval, citation scoring, and evidence storage budget for grounding controls"}),
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
            format!("AI/LLM grounding score {} is over budget", item.model_name),
            json!({"score_id": item.score_id, "estimated_monthly_cost_usd": item.estimated_monthly_cost_usd, "monthly_budget_usd": item.monthly_budget_usd}),
        ));
    }
}

fn evaluate_resilience(
    item: &GroundingScoreInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    match (item.grounding_score, item.minimum_grounding_score) {
        (None, _) => findings.push(finding(
            item,
            pillar,
            REASON_RES_SCORE_MISSING,
            Severity::High,
            format!(
                "AI/LLM grounding score {} has no score value",
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
                "AI/LLM grounding score {} is below threshold",
                item.model_name
            ),
            json!({"score_id": item.score_id, "grounding_score": score, "minimum_grounding_score": minimum}),
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
                "AI/LLM grounding score {} has no sample evidence",
                item.model_name
            ),
            json!({"score_id": item.score_id, "sample_count": item.sample_count}),
        ));
    }

    if item.evidence_source_count.unwrap_or(0) == 0 || item.citation_coverage.unwrap_or(0.0) <= 0.0
    {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_SOURCE_COVERAGE_MISSING,
            Severity::High,
            format!(
                "AI/LLM grounding score {} has no source coverage evidence",
                item.model_name
            ),
            json!({"score_id": item.score_id, "citation_coverage": item.citation_coverage, "evidence_source_count": item.evidence_source_count}),
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
                "AI/LLM grounding score {} has no scorer version",
                item.model_name
            ),
            json!({"score_id": item.score_id, "scoring_window": item.scoring_window}),
        ));
    }
}

fn evaluate_security(
    item: &GroundingScoreInventoryItem,
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
                "AI/LLM grounding score {} has no audit trail",
                item.model_name
            ),
            json!({"score_id": item.score_id}),
        ));
    }

    if item
        .source_access_policy
        .as_deref()
        .unwrap_or_default()
        .trim()
        .is_empty()
    {
        findings.push(finding(
            item,
            pillar,
            REASON_SEC_SOURCE_ACCESS_POLICY_MISSING,
            Severity::High,
            format!(
                "AI/LLM grounding score {} has no source access policy",
                item.model_name
            ),
            json!({"score_id": item.score_id}),
        ));
    }

    if item
        .evidence_retention_policy
        .as_deref()
        .unwrap_or_default()
        .trim()
        .is_empty()
    {
        findings.push(finding(
            item,
            pillar,
            REASON_SEC_EVIDENCE_RETENTION_MISSING,
            Severity::Medium,
            format!(
                "AI/LLM grounding score {} has no evidence retention policy",
                item.model_name
            ),
            json!({"score_id": item.score_id}),
        ));
    }
}

fn stale_finding(
    item: &GroundingScoreInventoryItem,
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
            "AI/LLM grounding score {} inventory data is stale by {} hours",
            item.model_name, age_hours
        ),
        json!({"score_id": item.score_id, "age_hours": age_hours, "stale_after_hours": DEFAULT_STALE_AFTER_HOURS}),
    ))
}

fn finding(
    item: &GroundingScoreInventoryItem,
    pillar: Pillar,
    reason_code: &str,
    severity: Severity,
    message: String,
    evidence: Value,
) -> InventoryFinding {
    InventoryFinding {
        resource_id: item.score_id.clone(),
        arn: format!("ai-llm-grounding-score/{}", item.score_id),
        pillar,
        reason_code: reason_code.to_string(),
        severity,
        message,
        evidence,
    }
}

fn has_owner_metadata(item: &GroundingScoreInventoryItem) -> bool {
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

    fn item() -> GroundingScoreInventoryItem {
        GroundingScoreInventoryItem {
            score_id: "grounding-score-1".to_string(),
            provider_id: "provider-1".to_string(),
            model_name: "rag-answer-model".to_string(),
            enabled: true,
            owner: Some("sre-ai".to_string()),
            labels: vec!["cost-center=ai-platform".to_string()],
            grounding_score: Some(0.91),
            minimum_grounding_score: Some(0.85),
            citation_coverage: Some(0.96),
            sample_count: Some(900),
            evidence_source_count: Some(12),
            scorer_version: Some("grounding-rubric-2026-06".to_string()),
            scoring_window: Some("rolling 7d".to_string()),
            estimated_monthly_cost_usd: Some(70.0),
            monthly_budget_usd: Some(120.0),
            audit_enabled: true,
            source_access_policy: Some("approved retrieval indexes only".to_string()),
            evidence_retention_policy: Some("retain cited evidence hashes for 30d".to_string()),
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
    fn healthy_grounding_score_inventory_passes_claimed_pillars() {
        let now = Utc::now();
        for pillar in [Pillar::Cost, Pillar::Resilience, Pillar::Security] {
            let report = evaluate_grounding_score_inventory(&[item()], pillar, now);
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

        let report = evaluate_grounding_score_inventory(&[item], Pillar::Cost, Utc::now());
        let codes = codes(&report);

        assert!(codes.contains(&REASON_COST_OWNER_NOT_RECORDED.to_string()));
        assert!(codes.contains(&REASON_COST_BUDGET_MISSING.to_string()));
    }

    #[test]
    fn resilience_flags_missing_and_below_threshold_grounding_evidence() {
        let mut missing = item();
        missing.grounding_score = None;
        missing.sample_count = Some(0);
        missing.evidence_source_count = Some(0);
        missing.citation_coverage = None;
        missing.scorer_version = None;

        let report = evaluate_grounding_score_inventory(&[missing], Pillar::Resilience, Utc::now());
        let reason_codes = codes(&report);
        assert!(reason_codes.contains(&REASON_RES_SCORE_MISSING.to_string()));
        assert!(reason_codes.contains(&REASON_RES_SAMPLE_MISSING.to_string()));
        assert!(reason_codes.contains(&REASON_RES_SOURCE_COVERAGE_MISSING.to_string()));
        assert!(reason_codes.contains(&REASON_RES_SCORER_VERSION_MISSING.to_string()));

        let mut below_threshold = item();
        below_threshold.grounding_score = Some(0.65);
        let report =
            evaluate_grounding_score_inventory(&[below_threshold], Pillar::Resilience, Utc::now());
        assert!(codes(&report).contains(&REASON_RES_SCORE_BELOW_THRESHOLD.to_string()));
    }

    #[test]
    fn security_flags_missing_audit_source_access_and_retention() {
        let mut item = item();
        item.audit_enabled = false;
        item.source_access_policy = None;
        item.evidence_retention_policy = None;

        let report = evaluate_grounding_score_inventory(&[item], Pillar::Security, Utc::now());
        let codes = codes(&report);

        assert!(codes.contains(&REASON_SEC_AUDIT_MISSING.to_string()));
        assert!(codes.contains(&REASON_SEC_SOURCE_ACCESS_POLICY_MISSING.to_string()));
        assert!(codes.contains(&REASON_SEC_EVIDENCE_RETENTION_MISSING.to_string()));
    }

    #[test]
    fn stale_grounding_score_inventory_is_counted_for_any_pillar() {
        let mut item = item();
        item.updated_at = Utc::now() - Duration::hours(DEFAULT_STALE_AFTER_HOURS + 1);

        let report = evaluate_grounding_score_inventory(&[item], Pillar::Cost, Utc::now());

        assert_eq!(report.stale_resources, 1);
        assert!(codes(&report).contains(&REASON_INV_STALE_DATA.to_string()));
    }
}

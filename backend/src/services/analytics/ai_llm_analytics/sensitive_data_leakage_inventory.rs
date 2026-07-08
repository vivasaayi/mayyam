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

pub const RESOURCE_TYPE: &str = "AiLlmSensitiveDataLeakage";
pub const REASON_COST_OWNER_NOT_RECORDED: &str =
    "AI_LLM_SENSITIVE_DATA_LEAKAGE_COST_OWNER_NOT_RECORDED";
pub const REASON_COST_BUDGET_MISSING: &str = "AI_LLM_SENSITIVE_DATA_LEAKAGE_COST_BUDGET_MISSING";
pub const REASON_COST_ESTIMATE_OVER_BUDGET: &str =
    "AI_LLM_SENSITIVE_DATA_LEAKAGE_COST_ESTIMATE_OVER_BUDGET";
pub const REASON_RES_DETECTION_DISABLED: &str =
    "AI_LLM_SENSITIVE_DATA_LEAKAGE_RES_DETECTION_DISABLED";
pub const REASON_RES_SAMPLE_MISSING: &str = "AI_LLM_SENSITIVE_DATA_LEAKAGE_RES_SAMPLE_MISSING";
pub const REASON_RES_PATTERN_SET_MISSING: &str =
    "AI_LLM_SENSITIVE_DATA_LEAKAGE_RES_PATTERN_SET_MISSING";
pub const REASON_RES_REVIEW_WINDOW_MISSING: &str =
    "AI_LLM_SENSITIVE_DATA_LEAKAGE_RES_REVIEW_WINDOW_MISSING";
pub const REASON_SEC_AUDIT_MISSING: &str = "AI_LLM_SENSITIVE_DATA_LEAKAGE_SEC_AUDIT_MISSING";
pub const REASON_SEC_REDACTION_POLICY_MISSING: &str =
    "AI_LLM_SENSITIVE_DATA_LEAKAGE_SEC_REDACTION_POLICY_MISSING";
pub const REASON_SEC_QUARANTINE_ACTION_MISSING: &str =
    "AI_LLM_SENSITIVE_DATA_LEAKAGE_SEC_QUARANTINE_ACTION_MISSING";
pub const REASON_SEC_TAMPER_EVIDENCE_MISSING: &str =
    "AI_LLM_SENSITIVE_DATA_LEAKAGE_SEC_TAMPER_EVIDENCE_MISSING";
pub const REASON_INV_STALE_DATA: &str = "AI_LLM_SENSITIVE_DATA_LEAKAGE_INV_STALE_DATA";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SensitiveDataLeakageInventoryItem {
    pub detector_id: String,
    pub provider_id: String,
    pub model_name: String,
    pub enabled: bool,
    pub owner: Option<String>,
    pub labels: Vec<String>,
    pub detection_enabled: bool,
    pub pattern_set_version: Option<String>,
    pub sample_count: Option<u64>,
    pub review_window: Option<String>,
    pub estimated_monthly_cost_usd: Option<f64>,
    pub monthly_budget_usd: Option<f64>,
    pub audit_enabled: bool,
    pub redaction_policy: Option<String>,
    pub quarantine_action: Option<String>,
    pub tamper_evidence_enabled: bool,
    pub updated_at: DateTime<Utc>,
}

pub fn evaluate_sensitive_data_leakage_inventory(
    items: &[SensitiveDataLeakageInventoryItem],
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

pub fn sensitive_data_leakage_inventory_item_from_model(
    model: &LlmProviderModel,
) -> SensitiveDataLeakageInventoryItem {
    SensitiveDataLeakageInventoryItem {
        detector_id: model.id.to_string(),
        provider_id: model.provider_id.to_string(),
        model_name: model.model_name.clone(),
        enabled: model.enabled,
        owner: string_field(&model.model_config, &["owner", "team", "cost_owner"]),
        labels: string_array_field(&model.model_config, "labels"),
        detection_enabled: bool_field(
            &model.model_config,
            &[
                "sensitive_data_leakage_detection_enabled",
                "pii_leakage_detection_enabled",
                "data_leakage_detection_enabled",
            ],
        ),
        pattern_set_version: string_field(
            &model.model_config,
            &[
                "sensitive_data_pattern_set_version",
                "pii_pattern_set_version",
                "pattern_set_version",
            ],
        ),
        sample_count: integer_field(
            &model.model_config,
            &[
                "sensitive_data_leakage_sample_count",
                "pii_sample_count",
                "sample_count",
            ],
        ),
        review_window: string_field(
            &model.model_config,
            &[
                "sensitive_data_review_window",
                "pii_review_window",
                "review_window",
            ],
        ),
        estimated_monthly_cost_usd: number_field(
            &model.model_config,
            &[
                "sensitive_data_leakage_monthly_cost_usd",
                "pii_leakage_cost_usd",
                "monthly_cost_usd",
            ],
        ),
        monthly_budget_usd: number_field(
            &model.model_config,
            &[
                "sensitive_data_leakage_budget_usd",
                "pii_leakage_budget_usd",
                "monthly_budget_usd",
            ],
        ),
        audit_enabled: bool_field(
            &model.model_config,
            &[
                "sensitive_data_leakage_audit_enabled",
                "pii_audit_enabled",
                "audit_enabled",
            ],
        ),
        redaction_policy: string_field(
            &model.model_config,
            &[
                "sensitive_data_redaction_policy",
                "pii_redaction_policy",
                "redaction_policy",
            ],
        ),
        quarantine_action: string_field(
            &model.model_config,
            &[
                "sensitive_data_quarantine_action",
                "pii_quarantine_action",
                "quarantine_action",
            ],
        ),
        tamper_evidence_enabled: bool_field(
            &model.model_config,
            &[
                "sensitive_data_tamper_evidence_enabled",
                "pii_tamper_evidence_enabled",
                "tamper_evidence_enabled",
            ],
        ),
        updated_at: model.updated_at,
    }
}

fn evaluate_cost(
    item: &SensitiveDataLeakageInventoryItem,
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
                "AI/LLM sensitive-data leakage detector {} has no owner metadata",
                item.model_name
            ),
            json!({"detector_id": item.detector_id, "labels": item.labels, "checked_keys": COST_ALLOCATION_TAG_KEYS}),
        ));
    }

    if item.monthly_budget_usd.is_none() {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_BUDGET_MISSING,
            Severity::Medium,
            format!(
                "AI/LLM sensitive-data leakage detector {} has no budget",
                item.model_name
            ),
            json!({"detector_id": item.detector_id, "recommendation": "Record leakage review and quarantine budget before safety posture is trusted"}),
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
                "AI/LLM sensitive-data leakage detector {} is over budget",
                item.model_name
            ),
            json!({"detector_id": item.detector_id, "estimated_monthly_cost_usd": item.estimated_monthly_cost_usd, "monthly_budget_usd": item.monthly_budget_usd}),
        ));
    }
}

fn evaluate_resilience(
    item: &SensitiveDataLeakageInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if !item.detection_enabled {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_DETECTION_DISABLED,
            Severity::High,
            format!(
                "AI/LLM sensitive-data leakage detector {} is disabled",
                item.model_name
            ),
            json!({"detector_id": item.detector_id}),
        ));
    }

    if item.sample_count.unwrap_or(0) == 0 {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_SAMPLE_MISSING,
            Severity::High,
            format!(
                "AI/LLM sensitive-data leakage detector {} has no sample evidence",
                item.model_name
            ),
            json!({"detector_id": item.detector_id}),
        ));
    }

    if item
        .pattern_set_version
        .as_deref()
        .unwrap_or_default()
        .trim()
        .is_empty()
    {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_PATTERN_SET_MISSING,
            Severity::Medium,
            format!(
                "AI/LLM sensitive-data leakage detector {} has no pattern set version",
                item.model_name
            ),
            json!({"detector_id": item.detector_id}),
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
                "AI/LLM sensitive-data leakage detector {} has no review window",
                item.model_name
            ),
            json!({"detector_id": item.detector_id}),
        ));
    }
}

fn evaluate_security(
    item: &SensitiveDataLeakageInventoryItem,
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
                "AI/LLM sensitive-data leakage detector {} has no audit trail",
                item.model_name
            ),
            json!({"detector_id": item.detector_id}),
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
            REASON_SEC_REDACTION_POLICY_MISSING,
            Severity::High,
            format!(
                "AI/LLM sensitive-data leakage detector {} has no redaction policy",
                item.model_name
            ),
            json!({"detector_id": item.detector_id}),
        ));
    }

    if item
        .quarantine_action
        .as_deref()
        .unwrap_or_default()
        .trim()
        .is_empty()
    {
        findings.push(finding(
            item,
            pillar,
            REASON_SEC_QUARANTINE_ACTION_MISSING,
            Severity::High,
            format!(
                "AI/LLM sensitive-data leakage detector {} has no quarantine action",
                item.model_name
            ),
            json!({"detector_id": item.detector_id}),
        ));
    }

    if !item.tamper_evidence_enabled {
        findings.push(finding(
            item,
            pillar,
            REASON_SEC_TAMPER_EVIDENCE_MISSING,
            Severity::Medium,
            format!(
                "AI/LLM sensitive-data leakage detector {} has no tamper evidence",
                item.model_name
            ),
            json!({"detector_id": item.detector_id}),
        ));
    }
}

fn stale_finding(
    item: &SensitiveDataLeakageInventoryItem,
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
            "AI/LLM sensitive-data leakage detector {} inventory data is stale by {} hours",
            item.model_name, age_hours
        ),
        json!({"detector_id": item.detector_id, "age_hours": age_hours, "stale_after_hours": DEFAULT_STALE_AFTER_HOURS}),
    ))
}

fn finding(
    item: &SensitiveDataLeakageInventoryItem,
    pillar: Pillar,
    reason_code: &str,
    severity: Severity,
    message: String,
    evidence: Value,
) -> InventoryFinding {
    InventoryFinding {
        resource_id: item.detector_id.clone(),
        arn: format!("ai-llm-sensitive-data-leakage/{}", item.detector_id),
        pillar,
        reason_code: reason_code.to_string(),
        severity,
        message,
        evidence,
    }
}

fn has_owner_metadata(item: &SensitiveDataLeakageInventoryItem) -> bool {
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

    fn item() -> SensitiveDataLeakageInventoryItem {
        SensitiveDataLeakageInventoryItem {
            detector_id: "detector-1".to_string(),
            provider_id: "provider-1".to_string(),
            model_name: "guard-model".to_string(),
            enabled: true,
            owner: Some("sre-ai".to_string()),
            labels: vec!["cost-center=ai-platform".to_string()],
            detection_enabled: true,
            pattern_set_version: Some("pii-patterns-2026-06".to_string()),
            sample_count: Some(900),
            review_window: Some("rolling 7d".to_string()),
            estimated_monthly_cost_usd: Some(70.0),
            monthly_budget_usd: Some(120.0),
            audit_enabled: true,
            redaction_policy: Some("redact secrets and customer identifiers".to_string()),
            quarantine_action: Some("block delivery and quarantine transcript".to_string()),
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
    fn healthy_sensitive_data_leakage_inventory_passes_claimed_pillars() {
        let now = Utc::now();
        for pillar in [Pillar::Cost, Pillar::Resilience, Pillar::Security] {
            let report = evaluate_sensitive_data_leakage_inventory(&[item()], pillar, now);
            assert_eq!(report.resources_evaluated, 1);
            assert!(report.findings.is_empty());
        }
    }

    #[test]
    fn security_flags_missing_redaction_quarantine_and_tamper_evidence() {
        let mut item = item();
        item.audit_enabled = false;
        item.redaction_policy = None;
        item.quarantine_action = None;
        item.tamper_evidence_enabled = false;

        let report =
            evaluate_sensitive_data_leakage_inventory(&[item], Pillar::Security, Utc::now());
        let codes = codes(&report);

        assert!(codes.contains(&REASON_SEC_AUDIT_MISSING.to_string()));
        assert!(codes.contains(&REASON_SEC_REDACTION_POLICY_MISSING.to_string()));
        assert!(codes.contains(&REASON_SEC_QUARANTINE_ACTION_MISSING.to_string()));
        assert!(codes.contains(&REASON_SEC_TAMPER_EVIDENCE_MISSING.to_string()));
    }

    #[test]
    fn stale_sensitive_data_inventory_is_counted_for_any_pillar() {
        let mut item = item();
        item.updated_at = Utc::now() - Duration::hours(DEFAULT_STALE_AFTER_HOURS + 2);

        let report =
            evaluate_sensitive_data_leakage_inventory(&[item], Pillar::Security, Utc::now());
        assert_eq!(report.stale_resources, 1);
        assert!(codes(&report).contains(&REASON_INV_STALE_DATA.to_string()));
    }
}

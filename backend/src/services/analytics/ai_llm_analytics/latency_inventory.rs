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

// Deterministic AI/LLM latency inventory evaluator for roadmap rows
// 25-AI-LLM-OBSERVABILITY-00148/00155/00176.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::models::llm_model::Model as LlmProviderModel;
use crate::services::aws::inventory::types::{
    score_pillar, InventoryFinding, Pillar, PillarReport, Severity, COST_ALLOCATION_TAG_KEYS,
    DEFAULT_STALE_AFTER_HOURS,
};

pub const RESOURCE_TYPE: &str = "AiLlmLatency";
pub const REASON_COST_OWNER_NOT_RECORDED: &str = "AI_LLM_LATENCY_COST_OWNER_NOT_RECORDED";
pub const REASON_COST_SLO_MISSING: &str = "AI_LLM_LATENCY_COST_SLO_MISSING";
pub const REASON_COST_HIGH_LATENCY: &str = "AI_LLM_LATENCY_COST_HIGH_LATENCY";
pub const REASON_RES_SAMPLE_MISSING: &str = "AI_LLM_LATENCY_RES_SAMPLE_MISSING";
pub const REASON_RES_TIMEOUT_MISSING: &str = "AI_LLM_LATENCY_RES_TIMEOUT_MISSING";
pub const REASON_RES_FALLBACK_MISSING: &str = "AI_LLM_LATENCY_RES_FALLBACK_MISSING";
pub const REASON_RES_SLO_BREACH: &str = "AI_LLM_LATENCY_RES_SLO_BREACH";
pub const REASON_SEC_TELEMETRY_DISABLED: &str = "AI_LLM_LATENCY_SEC_TELEMETRY_DISABLED";
pub const REASON_SEC_REDACTION_MISSING: &str = "AI_LLM_LATENCY_SEC_REDACTION_MISSING";
pub const REASON_INV_STALE_DATA: &str = "AI_LLM_LATENCY_INV_STALE_DATA";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LatencyInventoryItem {
    pub route_id: String,
    pub provider_id: String,
    pub model_name: String,
    pub enabled: bool,
    pub owner: Option<String>,
    pub labels: Vec<String>,
    pub p50_ms: Option<f64>,
    pub p95_ms: Option<f64>,
    pub p99_ms: Option<f64>,
    pub latency_slo_ms: Option<f64>,
    pub sample_count: Option<u64>,
    pub timeout_ms: Option<u64>,
    pub retry_policy: Option<String>,
    pub fallback_model: Option<String>,
    pub telemetry_enabled: bool,
    pub redaction_policy: Option<String>,
    pub updated_at: DateTime<Utc>,
}

pub fn evaluate_latency_inventory(
    items: &[LatencyInventoryItem],
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

pub fn latency_inventory_item_from_model(model: &LlmProviderModel) -> LatencyInventoryItem {
    LatencyInventoryItem {
        route_id: model.id.to_string(),
        provider_id: model.provider_id.to_string(),
        model_name: model.model_name.clone(),
        enabled: model.enabled,
        owner: string_field(&model.model_config, &["owner", "team", "cost_owner"]),
        labels: string_array_field(&model.model_config, "labels"),
        p50_ms: number_field(&model.model_config, &["latency_p50_ms", "p50_ms"]),
        p95_ms: number_field(&model.model_config, &["latency_p95_ms", "p95_ms"]),
        p99_ms: number_field(&model.model_config, &["latency_p99_ms", "p99_ms"]),
        latency_slo_ms: number_field(&model.model_config, &["latency_slo_ms", "slo_ms"]),
        sample_count: integer_field(
            &model.model_config,
            &["latency_sample_count", "sample_count"],
        ),
        timeout_ms: integer_field(&model.model_config, &["timeout_ms", "request_timeout_ms"]),
        retry_policy: string_field(&model.model_config, &["retry_policy", "retry_strategy"]),
        fallback_model: string_field(
            &model.model_config,
            &["fallback_model", "fallback_model_id"],
        ),
        telemetry_enabled: bool_field(
            &model.model_config,
            &["latency_telemetry_enabled", "telemetry_enabled"],
        ),
        redaction_policy: string_field(
            &model.model_config,
            &[
                "telemetry_redaction_policy",
                "redaction_policy",
                "data_policy",
            ],
        ),
        updated_at: model.updated_at,
    }
}

fn evaluate_cost(
    item: &LatencyInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if !has_owner_metadata(item) {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_OWNER_NOT_RECORDED,
            Severity::Medium,
            format!("AI/LLM latency route {} has no owner metadata", item.model_name),
            json!({"route_id": item.route_id, "labels": item.labels, "checked_keys": COST_ALLOCATION_TAG_KEYS}),
        ));
    }

    if item.latency_slo_ms.is_none() {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_SLO_MISSING,
            Severity::Medium,
            format!("AI/LLM latency route {} has no latency SLO", item.model_name),
            json!({"route_id": item.route_id, "recommendation": "Record a latency SLO so slow model routes can be tied to cost and retry impact"}),
        ));
    }

    if item
        .latency_slo_ms
        .zip(item.p95_ms)
        .map(|(slo, p95)| p95 > slo)
        .unwrap_or(false)
    {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_HIGH_LATENCY,
            Severity::High,
            format!(
                "AI/LLM latency route {} has p95 latency above SLO",
                item.model_name
            ),
            json!({"route_id": item.route_id, "p95_ms": item.p95_ms, "latency_slo_ms": item.latency_slo_ms, "estimated_monthly_impact": "review retry, timeout, and routing cost caused by slow responses"}),
        ));
    }
}

fn evaluate_resilience(
    item: &LatencyInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if item.sample_count.unwrap_or(0) == 0 || item.p95_ms.is_none() {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_SAMPLE_MISSING,
            Severity::High,
            format!(
                "AI/LLM latency route {} has no usable latency sample",
                item.model_name
            ),
            json!({"route_id": item.route_id, "sample_count": item.sample_count, "p95_ms": item.p95_ms}),
        ));
    }

    if item.timeout_ms.is_none() {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_TIMEOUT_MISSING,
            Severity::Medium,
            format!(
                "AI/LLM latency route {} has no timeout metadata",
                item.model_name
            ),
            json!({"route_id": item.route_id}),
        ));
    }

    if item
        .fallback_model
        .as_deref()
        .unwrap_or_default()
        .trim()
        .is_empty()
        && item
            .retry_policy
            .as_deref()
            .unwrap_or_default()
            .trim()
            .is_empty()
    {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_FALLBACK_MISSING,
            Severity::High,
            format!(
                "AI/LLM latency route {} has no fallback or retry policy",
                item.model_name
            ),
            json!({"route_id": item.route_id, "recommendation": "Declare fallback model or retry policy for slow provider responses"}),
        ));
    }

    if item
        .latency_slo_ms
        .zip(item.p99_ms.or(item.p95_ms))
        .map(|(slo, tail)| tail > slo)
        .unwrap_or(false)
    {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_SLO_BREACH,
            Severity::High,
            format!("AI/LLM latency route {} breaches latency SLO", item.model_name),
            json!({"route_id": item.route_id, "p95_ms": item.p95_ms, "p99_ms": item.p99_ms, "latency_slo_ms": item.latency_slo_ms}),
        ));
    }
}

fn evaluate_security(
    item: &LatencyInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if !item.telemetry_enabled {
        findings.push(finding(
            item,
            pillar,
            REASON_SEC_TELEMETRY_DISABLED,
            Severity::Medium,
            format!(
                "AI/LLM latency route {} has latency telemetry disabled",
                item.model_name
            ),
            json!({"route_id": item.route_id}),
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
                "AI/LLM latency route {} has no telemetry redaction policy",
                item.model_name
            ),
            json!({"route_id": item.route_id, "recommendation": "Record redaction policy before storing prompt, response, or tool timing dimensions"}),
        ));
    }
}

fn stale_finding(
    item: &LatencyInventoryItem,
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
            "AI/LLM latency route {} inventory data is stale by {} hours",
            item.model_name, age_hours
        ),
        json!({"route_id": item.route_id, "age_hours": age_hours, "stale_after_hours": DEFAULT_STALE_AFTER_HOURS}),
    ))
}

fn finding(
    item: &LatencyInventoryItem,
    pillar: Pillar,
    reason_code: &str,
    severity: Severity,
    message: String,
    evidence: Value,
) -> InventoryFinding {
    InventoryFinding {
        resource_id: item.route_id.clone(),
        arn: format!("ai-llm-latency/{}", item.route_id),
        pillar,
        reason_code: reason_code.to_string(),
        severity,
        message,
        evidence,
    }
}

fn has_owner_metadata(item: &LatencyInventoryItem) -> bool {
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

    fn item() -> LatencyInventoryItem {
        LatencyInventoryItem {
            route_id: "route-1".to_string(),
            provider_id: "provider-1".to_string(),
            model_name: "deepseek-chat".to_string(),
            enabled: true,
            owner: Some("sre-ai".to_string()),
            labels: vec!["cost-center=ai-platform".to_string()],
            p50_ms: Some(450.0),
            p95_ms: Some(900.0),
            p99_ms: Some(1200.0),
            latency_slo_ms: Some(1500.0),
            sample_count: Some(1000),
            timeout_ms: Some(30000),
            retry_policy: Some("retry once on timeout".to_string()),
            fallback_model: Some("deepseek-reasoner".to_string()),
            telemetry_enabled: true,
            redaction_policy: Some("redact prompt and response text".to_string()),
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
    fn healthy_latency_inventory_passes_claimed_pillars() {
        let now = Utc::now();
        for pillar in [Pillar::Cost, Pillar::Resilience, Pillar::Security] {
            let report = evaluate_latency_inventory(&[item()], pillar, now);
            assert_eq!(report.resources_evaluated, 1);
            assert!(report.findings.is_empty());
        }
    }

    #[test]
    fn cost_flags_missing_owner_slo_and_high_latency() {
        let mut item = item();
        item.owner = None;
        item.labels.clear();
        item.latency_slo_ms = Some(500.0);

        let report = evaluate_latency_inventory(&[item], Pillar::Cost, Utc::now());
        let codes = codes(&report);

        assert!(codes.contains(&REASON_COST_OWNER_NOT_RECORDED.to_string()));
        assert!(codes.contains(&REASON_COST_HIGH_LATENCY.to_string()));
    }

    #[test]
    fn resilience_flags_missing_samples_timeout_and_fallback() {
        let mut item = item();
        item.sample_count = Some(0);
        item.p95_ms = None;
        item.timeout_ms = None;
        item.retry_policy = None;
        item.fallback_model = None;

        let report = evaluate_latency_inventory(&[item], Pillar::Resilience, Utc::now());
        let codes = codes(&report);

        assert!(codes.contains(&REASON_RES_SAMPLE_MISSING.to_string()));
        assert!(codes.contains(&REASON_RES_TIMEOUT_MISSING.to_string()));
        assert!(codes.contains(&REASON_RES_FALLBACK_MISSING.to_string()));
    }

    #[test]
    fn security_flags_disabled_telemetry_and_missing_redaction() {
        let mut item = item();
        item.telemetry_enabled = false;
        item.redaction_policy = None;

        let report = evaluate_latency_inventory(&[item], Pillar::Security, Utc::now());
        let codes = codes(&report);

        assert!(codes.contains(&REASON_SEC_TELEMETRY_DISABLED.to_string()));
        assert!(codes.contains(&REASON_SEC_REDACTION_MISSING.to_string()));
    }

    #[test]
    fn stale_latency_inventory_is_counted_for_any_pillar() {
        let mut item = item();
        item.updated_at = Utc::now() - Duration::hours(DEFAULT_STALE_AFTER_HOURS + 1);

        let report = evaluate_latency_inventory(&[item], Pillar::Cost, Utc::now());

        assert_eq!(report.stale_resources, 1);
        assert!(codes(&report).contains(&REASON_INV_STALE_DATA.to_string()));
    }
}

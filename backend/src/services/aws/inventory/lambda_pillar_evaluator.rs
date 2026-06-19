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

// Deterministic Lambda inventory evaluators for the cost, security, and
// resilience pillars (roadmap rows 01-AWS-CLOUD-00127/00136/00163).
//
// Pure domain logic over collected `aws_resources` rows; no AWS calls,
// no database access, no LLM. Evaluates fields persisted by
// lambda_control_plane: runtime, memory_size, timeout, architectures.

use chrono::{DateTime, Utc};
use serde::Serialize;
use serde_json::{json, Value};
use std::collections::BTreeSet;

use crate::models::aws_resource::Model as AwsResourceModel;
use crate::services::aws::inventory::types::{
    check_stale, has_any_tag, score_pillar, InventoryFinding, Pillar, PillarReport, Severity,
    COST_ALLOCATION_TAG_KEYS, OWNER_TAG_KEYS,
};

// Reason codes are the stable contract for findings; never reuse or rename.
pub const REASON_COST_MISSING_ALLOCATION_TAGS: &str = "LAMBDA_COST_MISSING_ALLOCATION_TAGS";
pub const REASON_COST_X86_ONLY_ARCHITECTURE: &str = "LAMBDA_COST_X86_ONLY_ARCHITECTURE";
pub const REASON_COST_MISSING_TELEMETRY_COLLECTION_METADATA: &str =
    "LAMBDA_COST_MISSING_TELEMETRY_COLLECTION_METADATA";
pub const REASON_COST_TELEMETRY_COLLECTION_ERRORS: &str = "LAMBDA_COST_TELEMETRY_COLLECTION_ERRORS";
pub const REASON_COST_MISSING_CLOUDWATCH_TELEMETRY: &str =
    "LAMBDA_COST_MISSING_CLOUDWATCH_TELEMETRY";
pub const REASON_COST_NO_INVOCATIONS_TELEMETRY: &str = "LAMBDA_COST_NO_INVOCATIONS_TELEMETRY";
pub const REASON_COST_ERROR_OR_THROTTLE_TELEMETRY: &str = "LAMBDA_COST_ERROR_OR_THROTTLE_TELEMETRY";
pub const REASON_SEC_DEPRECATED_RUNTIME: &str = "LAMBDA_SEC_DEPRECATED_RUNTIME";
pub const REASON_SEC_MISSING_OWNER_TAG: &str = "LAMBDA_SEC_MISSING_OWNER_TAG";
pub const REASON_RES_MISSING_CONFIG_DATA: &str = "LAMBDA_RES_MISSING_CONFIG_DATA";
pub const REASON_RES_MISSING_TELEMETRY_COLLECTION_METADATA: &str =
    "LAMBDA_RES_MISSING_TELEMETRY_COLLECTION_METADATA";
pub const REASON_RES_TELEMETRY_COLLECTION_ERRORS: &str = "LAMBDA_RES_TELEMETRY_COLLECTION_ERRORS";
pub const REASON_RES_MISSING_CLOUDWATCH_TELEMETRY: &str = "LAMBDA_RES_MISSING_CLOUDWATCH_TELEMETRY";
pub const REASON_RES_MISSING_LOG_EVENT_EVIDENCE: &str = "LAMBDA_RES_MISSING_LOG_EVENT_EVIDENCE";
pub const REASON_RES_MISSING_QUOTA_LIMIT_EVIDENCE: &str = "LAMBDA_RES_MISSING_QUOTA_LIMIT_EVIDENCE";
pub const REASON_RES_ERROR_OR_THROTTLE_HEALTH_SIGNAL: &str =
    "LAMBDA_RES_ERROR_OR_THROTTLE_HEALTH_SIGNAL";
pub const REASON_RES_LOW_TIMEOUT_HEADROOM: &str = "LAMBDA_RES_LOW_TIMEOUT_HEADROOM";
pub const REASON_PERF_MISSING_CONFIG_DATA: &str = "LAMBDA_PERF_MISSING_CONFIG_DATA";
pub const REASON_PERF_MISSING_TELEMETRY_COLLECTION_METADATA: &str =
    "LAMBDA_PERF_MISSING_TELEMETRY_COLLECTION_METADATA";
pub const REASON_PERF_TELEMETRY_COLLECTION_ERRORS: &str = "LAMBDA_PERF_TELEMETRY_COLLECTION_ERRORS";
pub const REASON_PERF_MISSING_CLOUDWATCH_TELEMETRY: &str =
    "LAMBDA_PERF_MISSING_CLOUDWATCH_TELEMETRY";
pub const REASON_PERF_HIGH_DURATION_PRESSURE: &str = "LAMBDA_PERF_HIGH_DURATION_PRESSURE";
pub const REASON_PERF_ERROR_OR_THROTTLE_PRESSURE: &str = "LAMBDA_PERF_ERROR_OR_THROTTLE_PRESSURE";
pub const REASON_SCAL_MISSING_CONCURRENCY_LIMIT_EVIDENCE: &str =
    "LAMBDA_SCAL_MISSING_CONCURRENCY_LIMIT_EVIDENCE";
pub const REASON_SCAL_MISSING_TELEMETRY_COLLECTION_METADATA: &str =
    "LAMBDA_SCAL_MISSING_TELEMETRY_COLLECTION_METADATA";
pub const REASON_SCAL_TELEMETRY_COLLECTION_ERRORS: &str = "LAMBDA_SCAL_TELEMETRY_COLLECTION_ERRORS";
pub const REASON_SCAL_MISSING_CLOUDWATCH_TELEMETRY: &str =
    "LAMBDA_SCAL_MISSING_CLOUDWATCH_TELEMETRY";
pub const REASON_SCAL_HIGH_CONCURRENCY_UTILIZATION: &str =
    "LAMBDA_SCAL_HIGH_CONCURRENCY_UTILIZATION";
pub const REASON_SCAL_THROTTLE_PRESSURE: &str = "LAMBDA_SCAL_THROTTLE_PRESSURE";
pub const REASON_INV_STALE_DATA: &str = "LAMBDA_INV_STALE_DATA";

/// Runtimes AWS has deprecated (no more security patches). Kept as an
/// explicit deterministic list; extend when AWS announces new deprecations.
pub const DEPRECATED_RUNTIMES: &[&str] = &[
    "python2.7",
    "python3.6",
    "python3.7",
    "nodejs10.x",
    "nodejs12.x",
    "nodejs14.x",
    "nodejs16.x",
    "dotnetcore2.1",
    "dotnetcore3.1",
    "dotnet5.0",
    "ruby2.5",
    "ruby2.7",
    "go1.x",
    "java8",
    "provided",
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum LambdaPostureStatus {
    Pass,
    Fail,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LambdaPostureRule {
    pub rule_id: &'static str,
    pub status: LambdaPostureStatus,
    pub reason_codes: Vec<&'static str>,
    pub affected_resources: Vec<String>,
    pub suppression_supported: bool,
    pub assignment_supported: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LambdaCostPostureSummary {
    pub workflow_id: &'static str,
    pub rule_pack_id: &'static str,
    pub evidence_serializer: &'static str,
    pub severity_model: &'static str,
    pub audit_event_type: &'static str,
    pub read_only_mode: bool,
    pub status: LambdaPostureStatus,
    pub rules_evaluated: usize,
    pub rules_failed: usize,
    pub affected_resources: Vec<String>,
    pub rules: Vec<LambdaPostureRule>,
    pub suppression_policy: LambdaSuppressionPolicy,
    pub assignment_policy: LambdaAssignmentPolicy,
    pub recommendations: Vec<LambdaCostPostureRecommendation>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LambdaSuppressionPolicy {
    pub supported: bool,
    pub scope: &'static str,
    pub requires_reason: bool,
    pub audit_event_type: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LambdaAssignmentPolicy {
    pub supported: bool,
    pub owner_sources: Vec<&'static str>,
    pub fallback_owner: &'static str,
    pub audit_event_type: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LambdaCostPostureRecommendation {
    pub resource_id: String,
    pub reason_code: String,
    pub recommendation: &'static str,
    pub owner: Option<String>,
    pub confidence: &'static str,
    pub effort: &'static str,
    pub risk: &'static str,
    pub suppression_key: String,
    pub audit_event_type: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LambdaCostTelemetrySummary {
    pub workflow_id: &'static str,
    pub read_only_mode: bool,
    pub freshness_required: bool,
    pub telemetry_collection_required: bool,
    pub cloudwatch_namespace: &'static str,
    pub cloudwatch_dimension: &'static str,
    pub required_metrics: Vec<&'static str>,
    pub export_formats: Vec<&'static str>,
    pub missing_data_reason_codes: Vec<String>,
    pub evidence_reason_codes: Vec<String>,
    pub stale_data_blocks_delivery: bool,
    pub telemetry_quality_score: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LambdaAiTriageGuardrails {
    pub read_only_mode: bool,
    pub evidence_required: bool,
    pub separate_facts_from_hypotheses: bool,
    pub ask_for_missing_data: bool,
    pub no_llm_invocation: bool,
    pub no_mutation_planning: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct LambdaEvidenceCitation {
    pub reason_code: String,
    pub resource_id: String,
    pub severity: Severity,
    pub evidence: Value,
}

#[derive(Debug, Clone, Serialize)]
pub struct LambdaCostTriageContext {
    pub workflow_id: &'static str,
    pub pillar: Pillar,
    pub api_path: &'static str,
    pub context_builder_id: &'static str,
    pub prompt_template_id: &'static str,
    pub generation_mode: &'static str,
    pub max_prompt_tokens: u16,
    pub provider_routing: Vec<&'static str>,
    pub audit_event_type: &'static str,
    pub audit_id_prefix: &'static str,
    pub pagination: LambdaTriagePagination,
    pub freshness: LambdaTriageFreshness,
    pub export_formats: Vec<&'static str>,
    pub error_codes: Vec<&'static str>,
    pub guardrails: LambdaAiTriageGuardrails,
    pub facts: Vec<String>,
    pub hypotheses: Vec<String>,
    pub missing_data_questions: Vec<String>,
    pub follow_up_questions: Vec<String>,
    pub runbook_copy_markdown: String,
    pub feedback_capture: LambdaTriageFeedbackCapture,
    pub evidence_citations: Vec<LambdaEvidenceCitation>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LambdaTriagePagination {
    pub default_limit: u16,
    pub max_limit: u16,
    pub evidence_cursor: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LambdaTriageFreshness {
    pub stale_data_blocks_ai_summary: bool,
    pub stale_resources: usize,
    pub freshness_source: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LambdaTriageFeedbackCapture {
    pub supported: bool,
    pub feedback_event_type: &'static str,
    pub fields: Vec<&'static str>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LambdaInvestigationStepKind {
    Inspect,
    Compare,
    Diagnose,
    ProposeMutationPlan,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LambdaInvestigationToolMode {
    ReadOnly,
    ApprovalRequired,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct LambdaInvestigationStep {
    pub step_id: String,
    pub kind: LambdaInvestigationStepKind,
    pub tool_name: &'static str,
    pub tool_mode: LambdaInvestigationToolMode,
    pub target_resource_id: String,
    pub reason_code: String,
    pub stop_condition: String,
    pub evidence: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct LambdaMutationApprovalGate {
    pub gate_id: String,
    pub target_resource_id: String,
    pub required_approval: &'static str,
    pub blast_radius: String,
    pub rollback_note_required: bool,
    pub evidence_reason_codes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct LambdaCostAgenticInvestigationPlan {
    pub workflow_id: &'static str,
    pub default_tool_mode: LambdaInvestigationToolMode,
    pub max_tool_calls: usize,
    pub max_evidence_citations: usize,
    pub replay_required: bool,
    pub steps: Vec<LambdaInvestigationStep>,
    pub approval_gates: Vec<LambdaMutationApprovalGate>,
    pub evidence_citations: Vec<LambdaEvidenceCitation>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LambdaRemediationActionKind {
    AssignCostTags,
    PlanArm64Migration,
    ReviewUnusedFunctionCleanup,
    ReviewRetryThrottleCostControls,
    ReviewTelemetryCoverage,
    ReviewTimeoutHeadroom,
    ReviewFailureAndThrottleRecovery,
    ReviewConcurrencyLimitGuardrails,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LambdaRemediationStatus {
    DryRunPendingApproval,
    BlockedMissingEvidence,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct LambdaCostRemediationAction {
    pub action_id: String,
    pub kind: LambdaRemediationActionKind,
    pub status: LambdaRemediationStatus,
    pub target_resource_id: String,
    pub dry_run: bool,
    pub requires_approval: bool,
    pub approval_gate_id: Option<String>,
    pub audit_event_type: &'static str,
    pub idempotency_key: String,
    pub blast_radius: String,
    pub rollback_note: String,
    pub validation_steps: Vec<&'static str>,
    pub evidence_reason_codes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct LambdaCostRemediationWorkflow {
    pub workflow_id: &'static str,
    pub read_only_mode: bool,
    pub rbac_permission: &'static str,
    pub audit_stream: &'static str,
    pub stale_data_blocks_execution: bool,
    pub actions: Vec<LambdaCostRemediationAction>,
    pub approval_gates: Vec<LambdaMutationApprovalGate>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LambdaCostObjectiveStatus {
    OnTrack,
    AtRisk,
    Breached,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LambdaCostTrendDirection {
    Stable,
    Degrading,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct LambdaCostPolicyObjective {
    pub objective_id: &'static str,
    pub status: LambdaCostObjectiveStatus,
    pub target_score_min: u8,
    pub current_score: u8,
    pub trend_direction: LambdaCostTrendDirection,
    pub failed_rule_count: usize,
    pub affected_resource_count: usize,
    pub owner_filters: Vec<String>,
    pub environment_filters: Vec<String>,
    pub application_filters: Vec<String>,
    pub notification_targets: Vec<String>,
    pub policy_state: &'static str,
    pub status_history: Vec<&'static str>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct LambdaCostSloPolicySnapshot {
    pub workflow_id: &'static str,
    pub read_only_mode: bool,
    pub freshness_required: bool,
    pub objective: LambdaCostPolicyObjective,
    pub evidence_reason_codes: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LambdaCostForecastRisk {
    Low,
    Moderate,
    High,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LambdaCostForecastBand {
    pub horizon_days: u16,
    pub lower_monthly_cost_index: u16,
    pub expected_monthly_cost_index: u16,
    pub upper_monthly_cost_index: u16,
    pub confidence_level: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LambdaCostForecastRiskDriver {
    pub reason_code: String,
    pub affected_resources: Vec<String>,
    pub monthly_cost_index_delta: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LambdaCostForecastSnapshot {
    pub workflow_id: &'static str,
    pub read_only_mode: bool,
    pub baseline_window_days: u16,
    pub forecast_horizon_days: u16,
    pub confidence_level: u8,
    pub forecast_band: LambdaCostForecastBand,
    pub risk_level: LambdaCostForecastRisk,
    pub capacity_risk: &'static str,
    pub backtesting_fixture_status: &'static str,
    pub threshold_controls: Vec<&'static str>,
    pub what_if_inputs: Vec<&'static str>,
    pub blocked_by_stale_data: bool,
    pub blast_radius_summary: String,
    pub missing_data_reason_codes: Vec<String>,
    pub risk_drivers: Vec<LambdaCostForecastRiskDriver>,
    pub evidence_reason_codes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LambdaCostReportRow {
    pub resource_id: String,
    pub severity: Severity,
    pub reason_code: String,
    pub message: String,
    pub recovery_note: String,
    pub suppression_supported: bool,
    pub evidence: Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LambdaCostExecutiveSummary {
    pub report_id: &'static str,
    pub score: u8,
    pub resources_evaluated: usize,
    pub stale_resources: usize,
    pub rules_failed: usize,
    pub affected_resources: Vec<String>,
    pub top_reason_codes: Vec<String>,
    pub blast_radius_summary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LambdaCostEngineeringBacklog {
    pub report_id: &'static str,
    pub page: u16,
    pub page_size: u16,
    pub total: usize,
    pub rows: Vec<LambdaCostReportRow>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LambdaCostIncidentReview {
    pub report_id: &'static str,
    pub page: u16,
    pub page_size: u16,
    pub total: usize,
    pub rows: Vec<LambdaCostReportRow>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LambdaCostReportingBundle {
    pub workflow_id: &'static str,
    pub read_only_mode: bool,
    pub scheduled_delivery_state: &'static str,
    pub stale_data_blocks_delivery: bool,
    pub portfolio_summary_ready: bool,
    pub workload_summary_ready: bool,
    pub export_formats: Vec<&'static str>,
    pub saved_view_id: &'static str,
    pub executive_summary: LambdaCostExecutiveSummary,
    pub engineering_backlog: LambdaCostEngineeringBacklog,
    pub incident_review: LambdaCostIncidentReview,
    pub missing_data_reason_codes: Vec<String>,
    pub evidence_reason_codes: Vec<String>,
}

pub type LambdaResiliencePostureSummary = LambdaCostPostureSummary;
pub type LambdaResilienceTriageContext = LambdaCostTriageContext;
pub type LambdaResilienceAgenticInvestigationPlan = LambdaCostAgenticInvestigationPlan;
pub type LambdaResilienceRemediationWorkflow = LambdaCostRemediationWorkflow;
pub type LambdaResilienceSloPolicySnapshot = LambdaCostSloPolicySnapshot;
pub type LambdaResiliencePolicyObjective = LambdaCostPolicyObjective;
pub type LambdaResilienceObjectiveStatus = LambdaCostObjectiveStatus;
pub type LambdaResilienceTrendDirection = LambdaCostTrendDirection;
pub type LambdaResilienceReportRow = LambdaCostReportRow;
pub type LambdaResilienceExecutiveSummary = LambdaCostExecutiveSummary;
pub type LambdaResilienceEngineeringBacklog = LambdaCostEngineeringBacklog;
pub type LambdaResilienceIncidentReview = LambdaCostIncidentReview;
pub type LambdaResilienceReportingBundle = LambdaCostReportingBundle;
pub type LambdaPerformancePostureSummary = LambdaCostPostureSummary;
pub type LambdaPerformanceTriageContext = LambdaCostTriageContext;
pub type LambdaPerformanceTelemetrySummary = LambdaCostTelemetrySummary;
pub type LambdaScalabilityPostureSummary = LambdaCostPostureSummary;
pub type LambdaScalabilityTriageContext = LambdaCostTriageContext;
pub type LambdaScalabilityTelemetrySummary = LambdaCostTelemetrySummary;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LambdaResilienceForecastRisk {
    Low,
    Moderate,
    High,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LambdaResilienceForecastBand {
    pub horizon_days: u16,
    pub lower_recovery_exposure_index: u16,
    pub expected_recovery_exposure_index: u16,
    pub upper_recovery_exposure_index: u16,
    pub confidence_level: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LambdaResilienceForecastRiskDriver {
    pub reason_code: String,
    pub affected_resources: Vec<String>,
    pub recovery_exposure_index_delta: u16,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct LambdaResilienceForecastSnapshot {
    pub workflow_id: &'static str,
    pub read_only_mode: bool,
    pub baseline_window_days: u16,
    pub forecast_horizon_days: u16,
    pub confidence_level: u8,
    pub forecast_band: LambdaResilienceForecastBand,
    pub risk_level: LambdaResilienceForecastRisk,
    pub recovery_capacity_risk: &'static str,
    pub backtesting_fixture_status: &'static str,
    pub threshold_controls: Vec<&'static str>,
    pub what_if_inputs: Vec<&'static str>,
    pub blocked_by_stale_data: bool,
    pub blast_radius_summary: String,
    pub recovery_note: &'static str,
    pub missing_data_reason_codes: Vec<String>,
    pub risk_drivers: Vec<LambdaResilienceForecastRiskDriver>,
    pub evidence_reason_codes: Vec<String>,
}

/// Evaluate every Lambda function in the fleet for one pillar.
pub fn evaluate_lambda_fleet(
    resources: &[AwsResourceModel],
    pillar: Pillar,
    now: DateTime<Utc>,
) -> PillarReport {
    let mut findings: Vec<InventoryFinding> = Vec::new();
    let mut stale_resources = 0usize;

    for resource in resources {
        if let Some(stale) = check_stale(resource, pillar, REASON_INV_STALE_DATA, now) {
            stale_resources += 1;
            findings.push(stale);
        }
        match pillar {
            Pillar::Cost => evaluate_cost(resource, &mut findings),
            Pillar::Security => evaluate_security(resource, &mut findings),
            Pillar::Resilience => evaluate_resilience(resource, &mut findings),
            Pillar::Performance => evaluate_performance(resource, &mut findings),
            Pillar::Scalability => evaluate_scalability(resource, &mut findings),
            // Pillars without checks for this service yet produce no findings.
            _ => {}
        }
    }

    let score = score_pillar(&findings);
    PillarReport {
        pillar,
        resources_evaluated: resources.len(),
        stale_resources,
        score,
        findings,
    }
}

pub fn lambda_scalability_triage_context(report: &PillarReport) -> LambdaScalabilityTriageContext {
    let mut facts = Vec::new();
    let mut hypotheses = Vec::new();
    let mut missing_data_questions = Vec::new();
    let mut follow_up_questions = Vec::new();
    let mut evidence_citations = Vec::new();

    for finding in &report.findings {
        facts.push(format!(
            "{} affects {} with {:?} severity",
            finding.reason_code, finding.resource_id, finding.severity
        ));
        evidence_citations.push(LambdaEvidenceCitation {
            reason_code: finding.reason_code.clone(),
            resource_id: finding.resource_id.clone(),
            severity: finding.severity,
            evidence: finding.evidence.clone(),
        });

        match finding.reason_code.as_str() {
            REASON_INV_STALE_DATA => missing_data_questions.push(format!(
                "Refresh Lambda inventory and scalability telemetry for {} before explaining concurrency posture",
                finding.resource_id
            )),
            REASON_SCAL_MISSING_CONCURRENCY_LIMIT_EVIDENCE => missing_data_questions.push(format!(
                "Collect reserved concurrency and account concurrency limit evidence for {} before scoring Lambda scalability",
                finding.resource_id
            )),
            REASON_SCAL_MISSING_TELEMETRY_COLLECTION_METADATA => {
                missing_data_questions.push(format!(
                    "Collect Lambda scalability telemetry collection metadata for {} before trusting concurrency evidence",
                    finding.resource_id
                ))
            }
            REASON_SCAL_TELEMETRY_COLLECTION_ERRORS => hypotheses.push(format!(
                "{} has Lambda scalability telemetry collection errors; inspect CloudWatch permissions, throttling, and retry evidence before changing concurrency settings",
                finding.resource_id
            )),
            REASON_SCAL_MISSING_CLOUDWATCH_TELEMETRY => missing_data_questions.push(format!(
                "Collect Invocations, Throttles, and ConcurrentExecutions telemetry for {} before explaining Lambda scalability",
                finding.resource_id
            )),
            REASON_SCAL_HIGH_CONCURRENCY_UTILIZATION => hypotheses.push(format!(
                "{} is using most of its concurrency budget; inspect burst traffic, event source scaling, and quota headroom",
                finding.resource_id
            )),
            REASON_SCAL_THROTTLE_PRESSURE => hypotheses.push(format!(
                "{} has throttle pressure; inspect reserved concurrency, account limits, event source batch size, and upstream retry behavior",
                finding.resource_id
            )),
            _ => {}
        }

        follow_up_questions.push(lambda_scalability_follow_up_question(
            finding.reason_code.as_str(),
        ));
    }

    LambdaCostTriageContext {
        workflow_id: "lambda_scalability_triage_context",
        pillar: report.pillar,
        api_path: "/api/aws/inventory/lambda/pillars",
        context_builder_id: "lambda-scalability-deterministic-context-v1",
        prompt_template_id: "lambda-scalability-ai-triage-v1",
        generation_mode: "deterministic_no_llm",
        max_prompt_tokens: 1800,
        provider_routing: vec!["none"],
        audit_event_type: "lambda_scalability_ai_triage_context_built",
        audit_id_prefix: "lambda-scalability-ai-triage",
        pagination: LambdaTriagePagination {
            default_limit: 50,
            max_limit: 200,
            evidence_cursor: "evidence_citations",
        },
        freshness: LambdaTriageFreshness {
            stale_data_blocks_ai_summary: report.stale_resources > 0,
            stale_resources: report.stale_resources,
            freshness_source: "lambda_inventory_last_synced_at",
        },
        export_formats: vec!["json"],
        error_codes: vec!["STALE_LAMBDA_DATA", "MISSING_LAMBDA_SCALABILITY_TELEMETRY"],
        guardrails: LambdaAiTriageGuardrails {
            read_only_mode: true,
            evidence_required: true,
            separate_facts_from_hypotheses: true,
            ask_for_missing_data: true,
            no_llm_invocation: true,
            no_mutation_planning: true,
        },
        facts,
        hypotheses,
        missing_data_questions,
        follow_up_questions,
        runbook_copy_markdown: lambda_scalability_runbook_copy(report),
        feedback_capture: LambdaTriageFeedbackCapture {
            supported: true,
            feedback_event_type: "lambda_scalability_ai_triage_feedback_captured",
            fields: vec!["useful", "missing_evidence", "operator_note"],
        },
        evidence_citations,
    }
}

pub fn lambda_scalability_posture_summary(
    report: &PillarReport,
) -> LambdaScalabilityPostureSummary {
    let rules = vec![
        lambda_cost_posture_rule(
            report,
            "lambda-scalability-inventory-freshness",
            &[REASON_INV_STALE_DATA],
        ),
        lambda_cost_posture_rule(
            report,
            "lambda-scalability-concurrency-limits-present",
            &[REASON_SCAL_MISSING_CONCURRENCY_LIMIT_EVIDENCE],
        ),
        lambda_cost_posture_rule(
            report,
            "lambda-scalability-telemetry-collection-present",
            &[
                REASON_SCAL_MISSING_TELEMETRY_COLLECTION_METADATA,
                REASON_SCAL_TELEMETRY_COLLECTION_ERRORS,
            ],
        ),
        lambda_cost_posture_rule(
            report,
            "lambda-scalability-core-cloudwatch-telemetry-present",
            &[REASON_SCAL_MISSING_CLOUDWATCH_TELEMETRY],
        ),
        lambda_cost_posture_rule(
            report,
            "lambda-scalability-concurrency-and-throttle-headroom",
            &[
                REASON_SCAL_HIGH_CONCURRENCY_UTILIZATION,
                REASON_SCAL_THROTTLE_PRESSURE,
            ],
        ),
    ];
    let affected_resources = sorted_unique_resources(
        report
            .findings
            .iter()
            .map(|finding| finding.resource_id.clone()),
    );
    let rules_failed = rules
        .iter()
        .filter(|rule| rule.status == LambdaPostureStatus::Fail)
        .count();

    LambdaCostPostureSummary {
        workflow_id: "lambda_scalability_posture",
        rule_pack_id: "lambda-scalability-posture-rules-v1",
        evidence_serializer: "lambda-scalability-evidence-v1",
        severity_model: "lambda-scalability-severity-v1",
        audit_event_type: "lambda_scalability_posture_evaluated",
        read_only_mode: true,
        status: if rules_failed == 0 {
            LambdaPostureStatus::Pass
        } else {
            LambdaPostureStatus::Fail
        },
        rules_evaluated: rules.len(),
        rules_failed,
        affected_resources,
        rules,
        suppression_policy: LambdaSuppressionPolicy {
            supported: true,
            scope: "resource_reason_code",
            requires_reason: true,
            audit_event_type: "lambda_scalability_posture_suppression_requested",
        },
        assignment_policy: LambdaAssignmentPolicy {
            supported: true,
            owner_sources: vec!["owner", "team", "application", "service", "cost-center"],
            fallback_owner: "unassigned",
            audit_event_type: "lambda_scalability_posture_assignment_requested",
        },
        recommendations: lambda_scalability_posture_recommendations(report),
    }
}

pub fn lambda_scalability_telemetry_summary(
    report: &PillarReport,
) -> LambdaScalabilityTelemetrySummary {
    let missing_data_reason_codes = sorted_unique_reasons(
        report
            .findings
            .iter()
            .filter(|finding| {
                matches!(
                    finding.reason_code.as_str(),
                    REASON_SCAL_MISSING_TELEMETRY_COLLECTION_METADATA
                        | REASON_SCAL_TELEMETRY_COLLECTION_ERRORS
                        | REASON_SCAL_MISSING_CLOUDWATCH_TELEMETRY
                        | REASON_INV_STALE_DATA
                )
            })
            .map(|finding| finding.reason_code.clone()),
    );
    let evidence_reason_codes = sorted_unique_reasons(
        report
            .findings
            .iter()
            .map(|finding| finding.reason_code.clone()),
    );
    let stale_data_blocks_delivery = report.stale_resources > 0
        || report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_SCAL_TELEMETRY_COLLECTION_ERRORS);

    LambdaCostTelemetrySummary {
        workflow_id: "lambda_scalability_telemetry",
        read_only_mode: true,
        freshness_required: true,
        telemetry_collection_required: true,
        cloudwatch_namespace: "AWS/Lambda",
        cloudwatch_dimension: "FunctionName",
        required_metrics: lambda_scalability_metric_names().to_vec(),
        export_formats: vec!["json"],
        missing_data_reason_codes,
        evidence_reason_codes,
        stale_data_blocks_delivery,
        telemetry_quality_score: report.score,
    }
}

pub fn lambda_performance_triage_context(report: &PillarReport) -> LambdaPerformanceTriageContext {
    let mut facts = Vec::new();
    let mut hypotheses = Vec::new();
    let mut missing_data_questions = Vec::new();
    let mut follow_up_questions = Vec::new();
    let mut evidence_citations = Vec::new();

    for finding in &report.findings {
        facts.push(format!(
            "{} affects {} with {:?} severity",
            finding.reason_code, finding.resource_id, finding.severity
        ));
        evidence_citations.push(LambdaEvidenceCitation {
            reason_code: finding.reason_code.clone(),
            resource_id: finding.resource_id.clone(),
            severity: finding.severity,
            evidence: finding.evidence.clone(),
        });

        match finding.reason_code.as_str() {
            REASON_INV_STALE_DATA => missing_data_questions.push(format!(
                "Refresh Lambda inventory and performance telemetry for {} before explaining current latency posture",
                finding.resource_id
            )),
            REASON_PERF_MISSING_CONFIG_DATA => missing_data_questions.push(format!(
                "Collect timeout and memory configuration for {} before calculating Lambda performance headroom",
                finding.resource_id
            )),
            REASON_PERF_MISSING_TELEMETRY_COLLECTION_METADATA => {
                missing_data_questions.push(format!(
                    "Collect Lambda performance telemetry collection metadata for {} before trusting signal freshness",
                    finding.resource_id
                ))
            }
            REASON_PERF_TELEMETRY_COLLECTION_ERRORS => hypotheses.push(format!(
                "{} has Lambda performance telemetry collection errors; inspect CloudWatch permissions, throttling, and retry evidence before diagnosing latency",
                finding.resource_id
            )),
            REASON_PERF_MISSING_CLOUDWATCH_TELEMETRY => missing_data_questions.push(format!(
                "Collect Duration, Errors, Throttles, and ConcurrentExecutions telemetry for {} before explaining Lambda performance",
                finding.resource_id
            )),
            REASON_PERF_HIGH_DURATION_PRESSURE => hypotheses.push(format!(
                "{} is using a high share of its timeout budget; inspect memory sizing, cold starts, downstream latency, and payload growth before tuning",
                finding.resource_id
            )),
            REASON_PERF_ERROR_OR_THROTTLE_PRESSURE => hypotheses.push(format!(
                "{} has error or throttle pressure that can degrade request latency; inspect concurrency limits, event source pressure, and retry loops",
                finding.resource_id
            )),
            _ => {}
        }

        follow_up_questions.push(lambda_performance_follow_up_question(
            finding.reason_code.as_str(),
        ));
    }

    LambdaCostTriageContext {
        workflow_id: "lambda_performance_triage_context",
        pillar: report.pillar,
        api_path: "/api/aws/inventory/lambda/pillars",
        context_builder_id: "lambda-performance-deterministic-context-v1",
        prompt_template_id: "lambda-performance-ai-triage-v1",
        generation_mode: "deterministic_no_llm",
        max_prompt_tokens: 1800,
        provider_routing: vec!["none"],
        audit_event_type: "lambda_performance_ai_triage_context_built",
        audit_id_prefix: "lambda-performance-ai-triage",
        pagination: LambdaTriagePagination {
            default_limit: 50,
            max_limit: 200,
            evidence_cursor: "evidence_citations",
        },
        freshness: LambdaTriageFreshness {
            stale_data_blocks_ai_summary: report.stale_resources > 0,
            stale_resources: report.stale_resources,
            freshness_source: "lambda_inventory_last_synced_at",
        },
        export_formats: vec!["json"],
        error_codes: vec!["STALE_LAMBDA_DATA", "MISSING_LAMBDA_PERFORMANCE_TELEMETRY"],
        guardrails: LambdaAiTriageGuardrails {
            read_only_mode: true,
            evidence_required: true,
            separate_facts_from_hypotheses: true,
            ask_for_missing_data: true,
            no_llm_invocation: true,
            no_mutation_planning: true,
        },
        facts,
        hypotheses,
        missing_data_questions,
        follow_up_questions,
        runbook_copy_markdown: lambda_performance_runbook_copy(report),
        feedback_capture: LambdaTriageFeedbackCapture {
            supported: true,
            feedback_event_type: "lambda_performance_ai_triage_feedback_captured",
            fields: vec!["useful", "missing_evidence", "operator_note"],
        },
        evidence_citations,
    }
}

pub fn lambda_performance_posture_summary(
    report: &PillarReport,
) -> LambdaPerformancePostureSummary {
    let rules = vec![
        lambda_cost_posture_rule(
            report,
            "lambda-performance-inventory-freshness",
            &[REASON_INV_STALE_DATA],
        ),
        lambda_cost_posture_rule(
            report,
            "lambda-performance-config-present",
            &[REASON_PERF_MISSING_CONFIG_DATA],
        ),
        lambda_cost_posture_rule(
            report,
            "lambda-performance-telemetry-collection-present",
            &[
                REASON_PERF_MISSING_TELEMETRY_COLLECTION_METADATA,
                REASON_PERF_TELEMETRY_COLLECTION_ERRORS,
            ],
        ),
        lambda_cost_posture_rule(
            report,
            "lambda-performance-core-cloudwatch-telemetry-present",
            &[REASON_PERF_MISSING_CLOUDWATCH_TELEMETRY],
        ),
        lambda_cost_posture_rule(
            report,
            "lambda-performance-latency-and-throttle-headroom",
            &[
                REASON_PERF_HIGH_DURATION_PRESSURE,
                REASON_PERF_ERROR_OR_THROTTLE_PRESSURE,
            ],
        ),
    ];
    let affected_resources = sorted_unique_resources(
        report
            .findings
            .iter()
            .map(|finding| finding.resource_id.clone()),
    );
    let rules_failed = rules
        .iter()
        .filter(|rule| rule.status == LambdaPostureStatus::Fail)
        .count();

    LambdaCostPostureSummary {
        workflow_id: "lambda_performance_posture",
        rule_pack_id: "lambda-performance-posture-rules-v1",
        evidence_serializer: "lambda-performance-evidence-v1",
        severity_model: "lambda-performance-severity-v1",
        audit_event_type: "lambda_performance_posture_evaluated",
        read_only_mode: true,
        status: if rules_failed == 0 {
            LambdaPostureStatus::Pass
        } else {
            LambdaPostureStatus::Fail
        },
        rules_evaluated: rules.len(),
        rules_failed,
        affected_resources,
        rules,
        suppression_policy: LambdaSuppressionPolicy {
            supported: true,
            scope: "resource_reason_code",
            requires_reason: true,
            audit_event_type: "lambda_performance_posture_suppression_requested",
        },
        assignment_policy: LambdaAssignmentPolicy {
            supported: true,
            owner_sources: vec!["owner", "team", "application", "service", "cost-center"],
            fallback_owner: "unassigned",
            audit_event_type: "lambda_performance_posture_assignment_requested",
        },
        recommendations: lambda_performance_posture_recommendations(report),
    }
}

pub fn lambda_performance_telemetry_summary(
    report: &PillarReport,
) -> LambdaPerformanceTelemetrySummary {
    let missing_data_reason_codes = sorted_unique_reasons(
        report
            .findings
            .iter()
            .filter(|finding| {
                matches!(
                    finding.reason_code.as_str(),
                    REASON_PERF_MISSING_TELEMETRY_COLLECTION_METADATA
                        | REASON_PERF_TELEMETRY_COLLECTION_ERRORS
                        | REASON_PERF_MISSING_CLOUDWATCH_TELEMETRY
                        | REASON_INV_STALE_DATA
                )
            })
            .map(|finding| finding.reason_code.clone()),
    );
    let evidence_reason_codes = sorted_unique_reasons(
        report
            .findings
            .iter()
            .map(|finding| finding.reason_code.clone()),
    );
    let stale_data_blocks_delivery = report.stale_resources > 0
        || report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_PERF_TELEMETRY_COLLECTION_ERRORS);

    LambdaCostTelemetrySummary {
        workflow_id: "lambda_performance_telemetry",
        read_only_mode: true,
        freshness_required: true,
        telemetry_collection_required: true,
        cloudwatch_namespace: "AWS/Lambda",
        cloudwatch_dimension: "FunctionName",
        required_metrics: lambda_performance_metric_names().to_vec(),
        export_formats: vec!["json"],
        missing_data_reason_codes,
        evidence_reason_codes,
        stale_data_blocks_delivery,
        telemetry_quality_score: report.score,
    }
}

pub fn lambda_cost_triage_context(report: &PillarReport) -> LambdaCostTriageContext {
    let mut facts = Vec::new();
    let mut hypotheses = Vec::new();
    let mut missing_data_questions = Vec::new();
    let mut follow_up_questions = Vec::new();
    let mut evidence_citations = Vec::new();

    for finding in &report.findings {
        facts.push(format!(
            "{} affects {} with {:?} severity",
            finding.reason_code, finding.resource_id, finding.severity
        ));
        evidence_citations.push(LambdaEvidenceCitation {
            reason_code: finding.reason_code.clone(),
            resource_id: finding.resource_id.clone(),
            severity: finding.severity,
            evidence: finding.evidence.clone(),
        });

        match finding.reason_code.as_str() {
            REASON_INV_STALE_DATA => missing_data_questions.push(format!(
                "Refresh Lambda inventory and cost telemetry for {} before explaining current cost posture",
                finding.resource_id
            )),
            REASON_COST_MISSING_TELEMETRY_COLLECTION_METADATA => missing_data_questions.push(
                format!(
                    "Collect Lambda telemetry collection metadata for {} before trusting cost posture evidence",
                    finding.resource_id
                ),
            ),
            REASON_COST_TELEMETRY_COLLECTION_ERRORS => hypotheses.push(format!(
                "{} has Lambda telemetry collection errors; inspect CloudWatch permissions, API throttling, and collector retry evidence before changing cost posture",
                finding.resource_id
            )),
            REASON_COST_MISSING_CLOUDWATCH_TELEMETRY => missing_data_questions.push(format!(
                "Collect Invocations, Duration, Errors, and Throttles telemetry for {} before explaining Lambda cost behavior",
                finding.resource_id
            )),
            REASON_COST_MISSING_ALLOCATION_TAGS => missing_data_questions.push(format!(
                "Assign owner, team, project, or cost-center metadata for {} so Lambda savings can be routed",
                finding.resource_id
            )),
            REASON_COST_X86_ONLY_ARCHITECTURE => hypotheses.push(format!(
                "{} may reduce GB-second cost with arm64, but runtime dependencies and native extensions must be checked before migration",
                finding.resource_id
            )),
            REASON_COST_NO_INVOCATIONS_TELEMETRY => hypotheses.push(format!(
                "{} has no observed invocations in the telemetry window; verify schedule, event source mapping, and retention expectations before cleanup",
                finding.resource_id
            )),
            REASON_COST_ERROR_OR_THROTTLE_TELEMETRY => hypotheses.push(format!(
                "{} has error or throttle telemetry that can increase retry and duration spend; inspect concurrency limits, upstream retry policy, and error budget before tuning",
                finding.resource_id
            )),
            _ => {}
        }

        follow_up_questions.push(lambda_cost_follow_up_question(finding.reason_code.as_str()));
    }

    LambdaCostTriageContext {
        workflow_id: "lambda_cost_triage_context",
        pillar: report.pillar,
        api_path: "/api/aws/inventory/lambda/pillars",
        context_builder_id: "lambda-cost-deterministic-context-v1",
        prompt_template_id: "lambda-cost-ai-triage-v1",
        generation_mode: "deterministic_no_llm",
        max_prompt_tokens: 1200,
        provider_routing: vec!["primary_ops_llm", "fallback_ops_llm"],
        audit_event_type: "lambda_cost_ai_triage_context_built",
        audit_id_prefix: "lambda-cost-ai-triage",
        pagination: LambdaTriagePagination {
            default_limit: 50,
            max_limit: 200,
            evidence_cursor: "evidence_citations",
        },
        freshness: LambdaTriageFreshness {
            stale_data_blocks_ai_summary: report.stale_resources > 0,
            stale_resources: report.stale_resources,
            freshness_source: "lambda_inventory_last_synced_at",
        },
        export_formats: vec!["json", "markdown_runbook"],
        error_codes: vec![
            "LAMBDA_COST_AI_TRIAGE_STALE_DATA",
            "LAMBDA_COST_AI_TRIAGE_MISSING_EVIDENCE",
            "LAMBDA_COST_AI_TRIAGE_RBAC_DENIED",
        ],
        guardrails: LambdaAiTriageGuardrails {
            read_only_mode: true,
            evidence_required: true,
            separate_facts_from_hypotheses: true,
            ask_for_missing_data: true,
            no_llm_invocation: true,
            no_mutation_planning: true,
        },
        facts,
        hypotheses,
        missing_data_questions,
        follow_up_questions: sorted_unique_strings(follow_up_questions),
        runbook_copy_markdown: lambda_cost_runbook_copy(report),
        feedback_capture: LambdaTriageFeedbackCapture {
            supported: true,
            feedback_event_type: "lambda_cost_ai_triage_feedback_captured",
            fields: vec!["helpful", "accuracy", "missing_evidence", "operator_note"],
        },
        evidence_citations,
    }
}

fn lambda_cost_follow_up_question(reason_code: &str) -> String {
    match reason_code {
        REASON_INV_STALE_DATA => {
            "Has Lambda inventory and CloudWatch telemetry been refreshed in the current sync window?"
        }
        REASON_COST_MISSING_TELEMETRY_COLLECTION_METADATA => {
            "Which collector run should be used as evidence for Lambda cost telemetry completeness?"
        }
        REASON_COST_TELEMETRY_COLLECTION_ERRORS => {
            "Which CloudWatch permission, throttling, or retry failure prevented Lambda cost telemetry collection?"
        }
        REASON_COST_MISSING_CLOUDWATCH_TELEMETRY => {
            "Which Invocations, Duration, Errors, and Throttles datapoints are missing for the affected Lambda function?"
        }
        REASON_COST_MISSING_ALLOCATION_TAGS => {
            "Who owns the Lambda savings review when owner, team, project, or cost-center tags are missing?"
        }
        REASON_COST_X86_ONLY_ARCHITECTURE => {
            "Do runtime dependencies and native extensions allow an arm64 compatibility test?"
        }
        REASON_COST_NO_INVOCATIONS_TELEMETRY => {
            "Is the Lambda function intentionally dormant, scheduled rarely, or safe to retire after owner confirmation?"
        }
        REASON_COST_ERROR_OR_THROTTLE_TELEMETRY => {
            "Which retry, timeout, or concurrency setting is driving error or throttle-related Lambda spend?"
        }
        _ => "What additional evidence is required before explaining this Lambda cost finding?",
    }
    .to_string()
}

fn lambda_cost_runbook_copy(report: &PillarReport) -> String {
    let reason_codes = sorted_unique_strings(
        report
            .findings
            .iter()
            .map(|finding| finding.reason_code.clone())
            .collect(),
    );
    format!(
        "Lambda cost AI triage: score {} across {} function(s), {} stale. Evidence reason codes: {}.",
        report.score,
        report.resources_evaluated,
        report.stale_resources,
        if reason_codes.is_empty() {
            "none".to_string()
        } else {
            reason_codes.join(", ")
        }
    )
}

pub fn lambda_cost_posture_summary(report: &PillarReport) -> LambdaCostPostureSummary {
    let rules = vec![
        lambda_cost_posture_rule(
            report,
            "lambda-cost-inventory-freshness",
            &[REASON_INV_STALE_DATA],
        ),
        lambda_cost_posture_rule(
            report,
            "lambda-cost-telemetry-collection-metadata-present",
            &[REASON_COST_MISSING_TELEMETRY_COLLECTION_METADATA],
        ),
        lambda_cost_posture_rule(
            report,
            "lambda-cost-telemetry-collection-errors-clear",
            &[REASON_COST_TELEMETRY_COLLECTION_ERRORS],
        ),
        lambda_cost_posture_rule(
            report,
            "lambda-cost-cloudwatch-metrics-present",
            &[REASON_COST_MISSING_CLOUDWATCH_TELEMETRY],
        ),
        lambda_cost_posture_rule(
            report,
            "lambda-cost-allocation-tags-present",
            &[REASON_COST_MISSING_ALLOCATION_TAGS],
        ),
        lambda_cost_posture_rule(
            report,
            "lambda-cost-arm64-review",
            &[REASON_COST_X86_ONLY_ARCHITECTURE],
        ),
        lambda_cost_posture_rule(
            report,
            "lambda-cost-unused-or-failing-functions-reviewed",
            &[
                REASON_COST_NO_INVOCATIONS_TELEMETRY,
                REASON_COST_ERROR_OR_THROTTLE_TELEMETRY,
            ],
        ),
    ];
    let rules_failed = rules
        .iter()
        .filter(|rule| rule.status == LambdaPostureStatus::Fail)
        .count();
    let affected_resources = sorted_unique_resources(
        rules
            .iter()
            .flat_map(|rule| rule.affected_resources.iter().cloned()),
    );

    LambdaCostPostureSummary {
        workflow_id: "lambda_cost_posture",
        rule_pack_id: "lambda-cost-posture-rules-v1",
        evidence_serializer: "lambda-cost-evidence-v1",
        severity_model: "deterministic-high-medium-low-v1",
        audit_event_type: "lambda_cost_posture_evaluated",
        read_only_mode: true,
        status: if rules_failed == 0 {
            LambdaPostureStatus::Pass
        } else {
            LambdaPostureStatus::Fail
        },
        rules_evaluated: rules.len(),
        rules_failed,
        affected_resources,
        rules,
        suppression_policy: LambdaSuppressionPolicy {
            supported: true,
            scope: "resource_reason_code",
            requires_reason: true,
            audit_event_type: "lambda_cost_posture_suppression_requested",
        },
        assignment_policy: LambdaAssignmentPolicy {
            supported: true,
            owner_sources: vec!["tag:owner", "tag:team", "tag:cost-center", "tag:project"],
            fallback_owner: "unassigned",
            audit_event_type: "lambda_cost_posture_assignment_requested",
        },
        recommendations: lambda_cost_posture_recommendations(report),
    }
}

pub fn lambda_cost_agentic_investigation_plan(
    report: &PillarReport,
) -> LambdaCostAgenticInvestigationPlan {
    let triage = lambda_cost_triage_context(report);
    let mut steps = Vec::new();
    let mut approval_gates = Vec::new();

    for citation in &triage.evidence_citations {
        match citation.reason_code.as_str() {
            REASON_INV_STALE_DATA => {
                steps.push(lambda_investigation_step(
                    &steps,
                    LambdaInvestigationStepKind::Inspect,
                    "lambda.inventory.refresh_status",
                    LambdaInvestigationToolMode::ReadOnly,
                    citation,
                    "stop when Lambda inventory and CloudWatch telemetry freshness are confirmed",
                ));
            }
            REASON_COST_MISSING_TELEMETRY_COLLECTION_METADATA => {
                steps.push(lambda_investigation_step(
                    &steps,
                    LambdaInvestigationStepKind::Inspect,
                    "lambda.telemetry.inspect_collection_metadata",
                    LambdaInvestigationToolMode::ReadOnly,
                    citation,
                    "stop when collection start, completion, duration, success, failure, and error counts are recorded",
                ));
            }
            REASON_COST_TELEMETRY_COLLECTION_ERRORS => {
                steps.push(lambda_investigation_step(
                    &steps,
                    LambdaInvestigationStepKind::Diagnose,
                    "lambda.cloudwatch.inspect_collection_errors",
                    LambdaInvestigationToolMode::ReadOnly,
                    citation,
                    "stop when CloudWatch permissions, throttling, and collector retry evidence explain the telemetry gap",
                ));
            }
            REASON_COST_MISSING_CLOUDWATCH_TELEMETRY => {
                steps.push(lambda_investigation_step(
                    &steps,
                    LambdaInvestigationStepKind::Inspect,
                    "lambda.cloudwatch.get_cost_metrics",
                    LambdaInvestigationToolMode::ReadOnly,
                    citation,
                    "stop when Invocations, Duration, Errors, and Throttles datapoints are collected or confirmed absent",
                ));
            }
            REASON_COST_MISSING_ALLOCATION_TAGS => {
                steps.push(lambda_investigation_step(
                    &steps,
                    LambdaInvestigationStepKind::Diagnose,
                    "lambda.resource_groups.get_tagging_context",
                    LambdaInvestigationToolMode::ReadOnly,
                    citation,
                    "stop when owner, team, project, or cost-center can be inferred or the gap is assigned",
                ));
                approval_gates.push(lambda_mutation_gate(
                    &approval_gates,
                    citation,
                    "Approve Lambda tag writes after ownership is verified",
                ));
            }
            REASON_COST_X86_ONLY_ARCHITECTURE => {
                steps.push(lambda_investigation_step(
                    &steps,
                    LambdaInvestigationStepKind::Compare,
                    "lambda.compare_architecture_cost",
                    LambdaInvestigationToolMode::ReadOnly,
                    citation,
                    "stop when arm64 compatibility, native dependencies, and GB-second cost deltas are recorded",
                ));
                approval_gates.push(lambda_mutation_gate(
                    &approval_gates,
                    citation,
                    "Approve arm64 migration plan after compatibility testing",
                ));
            }
            REASON_COST_NO_INVOCATIONS_TELEMETRY => {
                steps.push(lambda_investigation_step(
                    &steps,
                    LambdaInvestigationStepKind::Diagnose,
                    "lambda.event_sources.inspect_invocation_paths",
                    LambdaInvestigationToolMode::ReadOnly,
                    citation,
                    "stop when schedules, event source mappings, and retention expectations explain zero invocation telemetry",
                ));
                approval_gates.push(lambda_mutation_gate(
                    &approval_gates,
                    citation,
                    "Approve disable or cleanup plan after owner confirmation",
                ));
            }
            REASON_COST_ERROR_OR_THROTTLE_TELEMETRY => {
                steps.push(lambda_investigation_step(
                    &steps,
                    LambdaInvestigationStepKind::Diagnose,
                    "lambda.cloudwatch.diagnose_retry_throttle_spend",
                    LambdaInvestigationToolMode::ReadOnly,
                    citation,
                    "stop when retry policy, timeout, reserved concurrency, and error budget evidence explain cost waste",
                ));
                approval_gates.push(lambda_mutation_gate(
                    &approval_gates,
                    citation,
                    "Approve concurrency, timeout, or retry changes after blast-radius review",
                ));
            }
            _ => {}
        }
    }

    steps.push(LambdaInvestigationStep {
        step_id: format!("lambda-cost-step-{:02}", steps.len() + 1),
        kind: LambdaInvestigationStepKind::ProposeMutationPlan,
        tool_name: "lambda.cost.prepare_approval_plan",
        tool_mode: LambdaInvestigationToolMode::ApprovalRequired,
        target_resource_id: "investigation".to_string(),
        reason_code: "LAMBDA_COST_APPROVAL_PLAN_REQUIRED".to_string(),
        stop_condition:
            "stop before mutation; require explicit operator approval, blast-radius summary, and rollback note"
                .to_string(),
        evidence: json!({
            "approval_gate_count": approval_gates.len(),
            "read_only_step_count": steps.len(),
        }),
    });

    LambdaCostAgenticInvestigationPlan {
        workflow_id: "lambda_cost_agentic_investigation",
        default_tool_mode: LambdaInvestigationToolMode::ReadOnly,
        max_tool_calls: steps.len().min(12),
        max_evidence_citations: triage.evidence_citations.len(),
        replay_required: true,
        steps,
        approval_gates,
        evidence_citations: triage.evidence_citations,
    }
}

pub fn lambda_cost_remediation_workflow(report: &PillarReport) -> LambdaCostRemediationWorkflow {
    let investigation = lambda_cost_agentic_investigation_plan(report);
    let has_stale_data = report
        .findings
        .iter()
        .any(|finding| finding.reason_code == REASON_INV_STALE_DATA);
    let mut actions = Vec::new();

    for gate in &investigation.approval_gates {
        for reason_code in &gate.evidence_reason_codes {
            if let Some(kind) = lambda_remediation_action_kind(reason_code) {
                actions.push(lambda_remediation_action(
                    &actions,
                    kind,
                    gate,
                    if has_stale_data {
                        LambdaRemediationStatus::BlockedMissingEvidence
                    } else {
                        LambdaRemediationStatus::DryRunPendingApproval
                    },
                ));
            }
        }
    }

    LambdaCostRemediationWorkflow {
        workflow_id: "lambda_cost_safe_remediation",
        read_only_mode: true,
        rbac_permission: "aws.lambda.cost.remediation.approve",
        audit_stream: "lambda_cost_remediation_audit",
        stale_data_blocks_execution: has_stale_data,
        actions,
        approval_gates: investigation.approval_gates,
    }
}

pub fn lambda_cost_slo_policy_snapshot(report: &PillarReport) -> LambdaCostSloPolicySnapshot {
    let posture = lambda_cost_posture_summary(report);
    let failed_rule_count = posture.rules_failed;
    let affected_resource_count = posture.affected_resources.len();
    let status =
        lambda_cost_objective_status(report.score, failed_rule_count, report.stale_resources);
    let owner_filters = sorted_unique_evidence_values(report, &["owner", "team", "cost-center"]);
    let environment_filters = sorted_unique_evidence_values(report, &["environment", "env"]);
    let application_filters = sorted_unique_evidence_values(report, &["application", "app"]);
    let notification_targets = lambda_notification_targets(&owner_filters, &environment_filters);

    LambdaCostSloPolicySnapshot {
        workflow_id: "lambda_cost_slo_policy",
        read_only_mode: true,
        freshness_required: true,
        objective: LambdaCostPolicyObjective {
            objective_id: "lambda-cost-score-min-90",
            status,
            target_score_min: 90,
            current_score: report.score,
            trend_direction: lambda_cost_trend_direction(status, failed_rule_count),
            failed_rule_count,
            affected_resource_count,
            owner_filters,
            environment_filters,
            application_filters,
            notification_targets,
            policy_state: if report.stale_resources > 0 {
                "blocked_stale_data"
            } else if failed_rule_count > 0 {
                "active_with_findings"
            } else {
                "active"
            },
            status_history: vec![
                "snapshot_collected",
                "policy_evaluated",
                "notification_targets_resolved",
            ],
        },
        evidence_reason_codes: sorted_unique_reason_codes(report),
    }
}

pub fn lambda_cost_forecast_snapshot(report: &PillarReport) -> LambdaCostForecastSnapshot {
    const BASELINE_WINDOW_DAYS: u16 = 30;
    const FORECAST_HORIZON_DAYS: u16 = 30;
    const CONFIDENCE_LEVEL: u8 = 80;

    let stale_count = count_reason(report, REASON_INV_STALE_DATA);
    let telemetry_error_count = count_reason(report, REASON_COST_TELEMETRY_COLLECTION_ERRORS);
    let missing_collection_metadata_count =
        count_reason(report, REASON_COST_MISSING_TELEMETRY_COLLECTION_METADATA);
    let missing_cloudwatch_count = count_reason(report, REASON_COST_MISSING_CLOUDWATCH_TELEMETRY);
    let no_invocations_count = count_reason(report, REASON_COST_NO_INVOCATIONS_TELEMETRY);
    let error_or_throttle_count = count_reason(report, REASON_COST_ERROR_OR_THROTTLE_TELEMETRY);
    let x86_only_count = count_reason(report, REASON_COST_X86_ONLY_ARCHITECTURE);
    let missing_tag_count = count_reason(report, REASON_COST_MISSING_ALLOCATION_TAGS);
    let blocked_by_stale_data = report.stale_resources > 0 || stale_count > 0;
    let forecast_blocked = blocked_by_stale_data || telemetry_error_count > 0;

    let expected_monthly_cost_index = 100u16
        + (error_or_throttle_count as u16 * 20)
        + (no_invocations_count as u16 * 18)
        + (missing_cloudwatch_count as u16 * 16)
        + (missing_collection_metadata_count as u16 * 12)
        + (x86_only_count as u16 * 10)
        + (missing_tag_count as u16 * 4)
        + (stale_count as u16 * 25)
        + (telemetry_error_count as u16 * 20);
    let uncertainty = 8u16
        + (missing_cloudwatch_count as u16 * 7)
        + (missing_collection_metadata_count as u16 * 5)
        + (missing_tag_count as u16 * 2)
        + (report.stale_resources as u16 * 10)
        + (report.resources_evaluated == 0) as u16 * 20;
    let lower_monthly_cost_index = expected_monthly_cost_index.saturating_sub(uncertainty);
    let upper_monthly_cost_index = expected_monthly_cost_index + uncertainty;
    let risk_level = if forecast_blocked {
        LambdaCostForecastRisk::Blocked
    } else if upper_monthly_cost_index >= 145 {
        LambdaCostForecastRisk::High
    } else if expected_monthly_cost_index > 100 {
        LambdaCostForecastRisk::Moderate
    } else {
        LambdaCostForecastRisk::Low
    };
    let risk_drivers = lambda_cost_forecast_risk_drivers(report);
    let impacted_functions = sorted_unique_resources(
        risk_drivers
            .iter()
            .flat_map(|driver| driver.affected_resources.iter().cloned()),
    );

    LambdaCostForecastSnapshot {
        workflow_id: "lambda_cost_forecasting",
        read_only_mode: true,
        baseline_window_days: BASELINE_WINDOW_DAYS,
        forecast_horizon_days: FORECAST_HORIZON_DAYS,
        confidence_level: CONFIDENCE_LEVEL,
        forecast_band: LambdaCostForecastBand {
            horizon_days: FORECAST_HORIZON_DAYS,
            lower_monthly_cost_index,
            expected_monthly_cost_index,
            upper_monthly_cost_index,
            confidence_level: CONFIDENCE_LEVEL,
        },
        risk_level,
        capacity_risk: lambda_cost_capacity_risk(
            forecast_blocked,
            no_invocations_count,
            error_or_throttle_count,
            missing_cloudwatch_count,
            missing_collection_metadata_count,
        ),
        backtesting_fixture_status: if report.findings.is_empty() {
            "ready_clean_baseline"
        } else if forecast_blocked || missing_cloudwatch_count > 0 {
            "needs_fresh_lambda_cost_fixture"
        } else {
            "ready_findings_baseline"
        },
        threshold_controls: vec![
            "monthly_cost_index_warning_threshold",
            "monthly_cost_index_critical_threshold",
        ],
        what_if_inputs: vec![
            "migrate_x86_functions_to_arm64",
            "review_unused_function_cleanup",
            "restore_lambda_cost_telemetry",
            "tune_retry_and_throttle_controls",
            "apply_cost_allocation_tags",
        ],
        blocked_by_stale_data,
        blast_radius_summary: if impacted_functions.is_empty() {
            "No Lambda functions have cost forecast risk in the current evidence.".to_string()
        } else {
            format!(
                "{} Lambda function(s) have cost forecast risk across architecture, invocation, error, throttle, or telemetry evidence.",
                impacted_functions.len()
            )
        },
        missing_data_reason_codes: lambda_cost_forecast_missing_data_reason_codes(report),
        risk_drivers,
        evidence_reason_codes: sorted_unique_reason_codes(report),
    }
}

pub fn lambda_cost_telemetry_summary(report: &PillarReport) -> LambdaCostTelemetrySummary {
    let missing_data_reason_codes = sorted_unique_reasons(
        report
            .findings
            .iter()
            .filter(|finding| {
                matches!(
                    finding.reason_code.as_str(),
                    REASON_COST_MISSING_TELEMETRY_COLLECTION_METADATA
                        | REASON_COST_TELEMETRY_COLLECTION_ERRORS
                        | REASON_COST_MISSING_CLOUDWATCH_TELEMETRY
                        | REASON_INV_STALE_DATA
                )
            })
            .map(|finding| finding.reason_code.clone()),
    );
    let evidence_reason_codes = sorted_unique_reasons(
        report
            .findings
            .iter()
            .map(|finding| finding.reason_code.clone()),
    );
    let stale_data_blocks_delivery = report.stale_resources > 0
        || report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_COST_TELEMETRY_COLLECTION_ERRORS);

    LambdaCostTelemetrySummary {
        workflow_id: "lambda_cost_telemetry",
        read_only_mode: true,
        freshness_required: true,
        telemetry_collection_required: true,
        cloudwatch_namespace: "AWS/Lambda",
        cloudwatch_dimension: "FunctionName",
        required_metrics: lambda_cost_metric_names().to_vec(),
        export_formats: vec!["json"],
        missing_data_reason_codes,
        evidence_reason_codes,
        stale_data_blocks_delivery,
        telemetry_quality_score: report.score,
    }
}

pub fn lambda_cost_reporting_bundle(report: &PillarReport) -> LambdaCostReportingBundle {
    let posture = lambda_cost_posture_summary(report);
    let reason_codes = sorted_unique_reason_codes(report);
    let rows = lambda_cost_report_rows(report);
    let missing_data_reason_codes = lambda_cost_reporting_missing_data_reason_codes(report);
    let stale_data_blocks_delivery = report.stale_resources > 0
        || report.findings.iter().any(|finding| {
            matches!(
                finding.reason_code.as_str(),
                REASON_INV_STALE_DATA
                    | REASON_COST_MISSING_TELEMETRY_COLLECTION_METADATA
                    | REASON_COST_TELEMETRY_COLLECTION_ERRORS
                    | REASON_COST_MISSING_CLOUDWATCH_TELEMETRY
            )
        });
    let blast_radius_summary = if posture.affected_resources.is_empty() {
        "No Lambda functions require cost reporting review.".to_string()
    } else {
        format!(
            "{} Lambda function(s) require cost reporting review.",
            posture.affected_resources.len()
        )
    };

    LambdaCostReportingBundle {
        workflow_id: "lambda_cost_reporting",
        read_only_mode: true,
        scheduled_delivery_state: if stale_data_blocks_delivery {
            "blocked_until_fresh_cost_evidence"
        } else {
            "ready_for_schedule"
        },
        stale_data_blocks_delivery,
        portfolio_summary_ready: !stale_data_blocks_delivery,
        workload_summary_ready: !stale_data_blocks_delivery && report.resources_evaluated > 0,
        export_formats: vec!["json", "csv"],
        saved_view_id: "lambda-cost-posture-report",
        executive_summary: LambdaCostExecutiveSummary {
            report_id: "lambda-cost-executive-summary",
            score: report.score,
            resources_evaluated: report.resources_evaluated,
            stale_resources: report.stale_resources,
            rules_failed: posture.rules_failed,
            affected_resources: posture.affected_resources,
            top_reason_codes: reason_codes.clone(),
            blast_radius_summary,
        },
        engineering_backlog: LambdaCostEngineeringBacklog {
            report_id: "lambda-cost-engineering-backlog",
            page: 0,
            page_size: 50,
            total: rows.len(),
            rows: rows.clone(),
        },
        incident_review: LambdaCostIncidentReview {
            report_id: "lambda-cost-incident-review",
            page: 0,
            page_size: 50,
            total: rows.len(),
            rows,
        },
        missing_data_reason_codes,
        evidence_reason_codes: reason_codes,
    }
}

pub fn lambda_resilience_triage_context(report: &PillarReport) -> LambdaResilienceTriageContext {
    let mut facts = Vec::new();
    let mut hypotheses = Vec::new();
    let mut missing_data_questions = Vec::new();
    let mut follow_up_questions = Vec::new();
    let mut evidence_citations = Vec::new();

    for finding in &report.findings {
        facts.push(format!(
            "{} affects {} with {:?} severity",
            finding.reason_code, finding.resource_id, finding.severity
        ));
        evidence_citations.push(LambdaEvidenceCitation {
            reason_code: finding.reason_code.clone(),
            resource_id: finding.resource_id.clone(),
            severity: finding.severity,
            evidence: finding.evidence.clone(),
        });

        match finding.reason_code.as_str() {
            REASON_INV_STALE_DATA => missing_data_questions.push(format!(
                "Refresh Lambda inventory and resilience telemetry for {} before explaining current failure posture",
                finding.resource_id
            )),
            REASON_RES_MISSING_CONFIG_DATA => missing_data_questions.push(format!(
                "Collect timeout and memory configuration for {} before assessing resilience limits",
                finding.resource_id
            )),
            REASON_RES_MISSING_TELEMETRY_COLLECTION_METADATA => missing_data_questions.push(
                format!(
                    "Collect Lambda resilience telemetry collection metadata for {} before trusting evidence freshness",
                    finding.resource_id
                ),
            ),
            REASON_RES_TELEMETRY_COLLECTION_ERRORS => hypotheses.push(format!(
                "{} has resilience telemetry collection errors; inspect CloudWatch permissions, collector retries, and ingestion failures before recommending recovery changes",
                finding.resource_id
            )),
            REASON_RES_MISSING_CLOUDWATCH_TELEMETRY => missing_data_questions.push(format!(
                "Collect Duration, Errors, and Throttles telemetry for {} before explaining Lambda resilience behavior",
                finding.resource_id
            )),
            REASON_RES_MISSING_LOG_EVENT_EVIDENCE => missing_data_questions.push(format!(
                "Collect log, event-source, and dead-letter evidence for {} before explaining Lambda recovery paths",
                finding.resource_id
            )),
            REASON_RES_MISSING_QUOTA_LIMIT_EVIDENCE => missing_data_questions.push(format!(
                "Collect reserved concurrency and account concurrency limits for {} before evaluating throttle resilience",
                finding.resource_id
            )),
            REASON_RES_ERROR_OR_THROTTLE_HEALTH_SIGNAL => hypotheses.push(format!(
                "{} has Lambda error or throttle signals that can degrade retries and recovery; inspect event source pressure, concurrency, and upstream error paths before recommending changes",
                finding.resource_id
            )),
            REASON_RES_LOW_TIMEOUT_HEADROOM => hypotheses.push(format!(
                "{} is running close to its timeout budget; verify duration percentile, memory headroom, and downstream latency before tuning resilience limits",
                finding.resource_id
            )),
            _ => {}
        }

        follow_up_questions.push(lambda_resilience_follow_up_question(
            finding.reason_code.as_str(),
        ));
    }

    LambdaCostTriageContext {
        workflow_id: "lambda_resilience_triage_context",
        pillar: report.pillar,
        api_path: "/api/aws/inventory/lambda/pillars",
        context_builder_id: "lambda-resilience-deterministic-context-v1",
        prompt_template_id: "lambda-resilience-ai-triage-v1",
        generation_mode: "deterministic_no_llm",
        max_prompt_tokens: 1200,
        provider_routing: vec!["primary_ops_llm", "fallback_ops_llm"],
        audit_event_type: "lambda_resilience_ai_triage_context_built",
        audit_id_prefix: "lambda-resilience-ai-triage",
        pagination: LambdaTriagePagination {
            default_limit: 50,
            max_limit: 200,
            evidence_cursor: "evidence_citations",
        },
        freshness: LambdaTriageFreshness {
            stale_data_blocks_ai_summary: report.stale_resources > 0,
            stale_resources: report.stale_resources,
            freshness_source: "lambda_inventory_last_synced_at",
        },
        export_formats: vec!["json", "markdown_runbook"],
        error_codes: vec![
            "LAMBDA_RESILIENCE_AI_TRIAGE_STALE_DATA",
            "LAMBDA_RESILIENCE_AI_TRIAGE_MISSING_EVIDENCE",
            "LAMBDA_RESILIENCE_AI_TRIAGE_RBAC_DENIED",
        ],
        guardrails: LambdaAiTriageGuardrails {
            read_only_mode: true,
            evidence_required: true,
            separate_facts_from_hypotheses: true,
            ask_for_missing_data: true,
            no_llm_invocation: true,
            no_mutation_planning: true,
        },
        facts,
        hypotheses,
        missing_data_questions,
        follow_up_questions: sorted_unique_strings(follow_up_questions),
        runbook_copy_markdown: lambda_resilience_runbook_copy(report),
        feedback_capture: LambdaTriageFeedbackCapture {
            supported: true,
            feedback_event_type: "lambda_resilience_ai_triage_feedback_captured",
            fields: vec!["helpful", "accuracy", "missing_evidence", "operator_note"],
        },
        evidence_citations,
    }
}

pub fn lambda_resilience_posture_summary(report: &PillarReport) -> LambdaResiliencePostureSummary {
    let rules = vec![
        lambda_cost_posture_rule(
            report,
            "lambda-resilience-inventory-freshness",
            &[REASON_INV_STALE_DATA],
        ),
        lambda_cost_posture_rule(
            report,
            "lambda-resilience-config-present",
            &[REASON_RES_MISSING_CONFIG_DATA],
        ),
        lambda_cost_posture_rule(
            report,
            "lambda-resilience-telemetry-collection-metadata-present",
            &[REASON_RES_MISSING_TELEMETRY_COLLECTION_METADATA],
        ),
        lambda_cost_posture_rule(
            report,
            "lambda-resilience-telemetry-collection-errors-clear",
            &[REASON_RES_TELEMETRY_COLLECTION_ERRORS],
        ),
        lambda_cost_posture_rule(
            report,
            "lambda-resilience-cloudwatch-metrics-present",
            &[REASON_RES_MISSING_CLOUDWATCH_TELEMETRY],
        ),
        lambda_cost_posture_rule(
            report,
            "lambda-resilience-log-and-event-evidence-present",
            &[REASON_RES_MISSING_LOG_EVENT_EVIDENCE],
        ),
        lambda_cost_posture_rule(
            report,
            "lambda-resilience-quota-limit-evidence-present",
            &[REASON_RES_MISSING_QUOTA_LIMIT_EVIDENCE],
        ),
        lambda_cost_posture_rule(
            report,
            "lambda-resilience-timeout-headroom-safe",
            &[REASON_RES_LOW_TIMEOUT_HEADROOM],
        ),
        lambda_cost_posture_rule(
            report,
            "lambda-resilience-errors-and-throttles-reviewed",
            &[REASON_RES_ERROR_OR_THROTTLE_HEALTH_SIGNAL],
        ),
    ];
    let rules_failed = rules
        .iter()
        .filter(|rule| rule.status == LambdaPostureStatus::Fail)
        .count();
    let affected_resources = sorted_unique_resources(
        rules
            .iter()
            .flat_map(|rule| rule.affected_resources.iter().cloned()),
    );

    LambdaCostPostureSummary {
        workflow_id: "lambda_resilience_posture",
        rule_pack_id: "lambda-resilience-posture-rules-v1",
        evidence_serializer: "lambda-resilience-evidence-v1",
        severity_model: "deterministic-high-medium-low-v1",
        audit_event_type: "lambda_resilience_posture_evaluated",
        read_only_mode: true,
        status: if rules_failed == 0 {
            LambdaPostureStatus::Pass
        } else {
            LambdaPostureStatus::Fail
        },
        rules_evaluated: rules.len(),
        rules_failed,
        affected_resources,
        rules,
        suppression_policy: LambdaSuppressionPolicy {
            supported: true,
            scope: "resource_reason_code",
            requires_reason: true,
            audit_event_type: "lambda_resilience_posture_suppression_requested",
        },
        assignment_policy: LambdaAssignmentPolicy {
            supported: true,
            owner_sources: vec![
                "tag:owner",
                "tag:team",
                "tag:environment",
                "tag:application",
            ],
            fallback_owner: "unassigned",
            audit_event_type: "lambda_resilience_posture_assignment_requested",
        },
        recommendations: lambda_resilience_posture_recommendations(report),
    }
}

pub fn lambda_resilience_agentic_investigation_plan(
    report: &PillarReport,
) -> LambdaResilienceAgenticInvestigationPlan {
    let triage = lambda_resilience_triage_context(report);
    let mut steps = Vec::new();
    let mut approval_gates = Vec::new();

    for citation in &triage.evidence_citations {
        match citation.reason_code.as_str() {
            REASON_INV_STALE_DATA => steps.push(lambda_investigation_step(
                &steps,
                LambdaInvestigationStepKind::Inspect,
                "lambda.resilience.inspect_inventory_freshness",
                LambdaInvestigationToolMode::ReadOnly,
                citation,
                "stop when Lambda inventory freshness and resilience evidence timestamps are confirmed",
            )),
            REASON_RES_MISSING_CONFIG_DATA => steps.push(lambda_investigation_step(
                &steps,
                LambdaInvestigationStepKind::Inspect,
                "lambda.resilience.inspect_runtime_limits",
                LambdaInvestigationToolMode::ReadOnly,
                citation,
                "stop when timeout, memory, and architecture limits are recorded",
            )),
            REASON_RES_MISSING_TELEMETRY_COLLECTION_METADATA => steps.push(
                lambda_investigation_step(
                    &steps,
                    LambdaInvestigationStepKind::Inspect,
                    "lambda.resilience.inspect_collection_metadata",
                    LambdaInvestigationToolMode::ReadOnly,
                    citation,
                    "stop when collection start, completion, duration, success, failure, and error counts are recorded",
                ),
            ),
            REASON_RES_TELEMETRY_COLLECTION_ERRORS => steps.push(lambda_investigation_step(
                &steps,
                LambdaInvestigationStepKind::Diagnose,
                "lambda.resilience.inspect_collector_errors",
                LambdaInvestigationToolMode::ReadOnly,
                citation,
                "stop when collector logs, permissions, throttling, and retry evidence explain the resilience telemetry gap",
            )),
            REASON_RES_MISSING_CLOUDWATCH_TELEMETRY => steps.push(lambda_investigation_step(
                &steps,
                LambdaInvestigationStepKind::Inspect,
                "lambda.resilience.get_health_metrics",
                LambdaInvestigationToolMode::ReadOnly,
                citation,
                "stop when Duration, Errors, and Throttles datapoints are collected or confirmed absent",
            )),
            REASON_RES_MISSING_LOG_EVENT_EVIDENCE => steps.push(lambda_investigation_step(
                &steps,
                LambdaInvestigationStepKind::Inspect,
                "lambda.resilience.inspect_logs_and_event_sources",
                LambdaInvestigationToolMode::ReadOnly,
                citation,
                "stop when log subscription, DLQ, and event source mapping evidence are recorded",
            )),
            REASON_RES_MISSING_QUOTA_LIMIT_EVIDENCE => steps.push(lambda_investigation_step(
                &steps,
                LambdaInvestigationStepKind::Inspect,
                "lambda.resilience.inspect_concurrency_limits",
                LambdaInvestigationToolMode::ReadOnly,
                citation,
                "stop when reserved concurrency and account concurrency limit evidence are recorded",
            )),
            REASON_RES_ERROR_OR_THROTTLE_HEALTH_SIGNAL => {
                steps.push(lambda_investigation_step(
                    &steps,
                    LambdaInvestigationStepKind::Diagnose,
                    "lambda.resilience.diagnose_failure_and_throttle_path",
                    LambdaInvestigationToolMode::ReadOnly,
                    citation,
                    "stop when error, throttle, event retry, and downstream dependency evidence explain the recovery gap",
                ));
                approval_gates.push(lambda_mutation_gate(
                    &approval_gates,
                    citation,
                    "Approve concurrency, retry, or event-source changes after resilience review",
                ));
            }
            REASON_RES_LOW_TIMEOUT_HEADROOM => {
                steps.push(lambda_investigation_step(
                    &steps,
                    LambdaInvestigationStepKind::Compare,
                    "lambda.resilience.compare_timeout_headroom",
                    LambdaInvestigationToolMode::ReadOnly,
                    citation,
                    "stop when duration percentile, timeout budget, and downstream latency evidence explain the headroom risk",
                ));
                approval_gates.push(lambda_mutation_gate(
                    &approval_gates,
                    citation,
                    "Approve timeout, memory, or concurrency changes after blast-radius review",
                ));
            }
            _ => {}
        }
    }

    if !approval_gates.is_empty() {
        steps.push(LambdaInvestigationStep {
            step_id: format!("lambda-resilience-step-{:02}", steps.len() + 1),
            kind: LambdaInvestigationStepKind::ProposeMutationPlan,
            tool_name: "lambda.resilience.prepare_approval_plan",
            tool_mode: LambdaInvestigationToolMode::ApprovalRequired,
            target_resource_id: "investigation".to_string(),
            reason_code: "LAMBDA_RESILIENCE_APPROVAL_PLAN_REQUIRED".to_string(),
            stop_condition:
                "stop before mutation; require explicit operator approval, blast-radius summary, and rollback note"
                    .to_string(),
            evidence: json!({
                "approval_gate_count": approval_gates.len(),
                "read_only_step_count": steps.len(),
            }),
        });
    }

    LambdaCostAgenticInvestigationPlan {
        workflow_id: "lambda_resilience_agentic_investigation",
        default_tool_mode: LambdaInvestigationToolMode::ReadOnly,
        max_tool_calls: steps.len().min(12),
        max_evidence_citations: triage.evidence_citations.len(),
        replay_required: true,
        steps,
        approval_gates,
        evidence_citations: triage.evidence_citations,
    }
}

pub fn lambda_resilience_remediation_workflow(
    report: &PillarReport,
) -> LambdaResilienceRemediationWorkflow {
    let investigation = lambda_resilience_agentic_investigation_plan(report);
    let has_stale_data = report
        .findings
        .iter()
        .any(|finding| finding.reason_code == REASON_INV_STALE_DATA);
    let mut actions = Vec::new();

    for gate in &investigation.approval_gates {
        for reason_code in &gate.evidence_reason_codes {
            if let Some(kind) = lambda_resilience_remediation_action_kind(reason_code) {
                actions.push(lambda_remediation_action_with_contract(
                    "lambda-resilience",
                    "lambda.resilience.remediation.dry_run_planned",
                    &[
                        "refresh Lambda config, metric, log, event, and concurrency evidence",
                        "verify owner, blast radius, downstream dependency impact, and recovery path intent",
                        "capture operator approval, rollback note, and audit id before execution",
                    ],
                    &actions,
                    kind,
                    gate,
                    if has_stale_data {
                        LambdaRemediationStatus::BlockedMissingEvidence
                    } else {
                        LambdaRemediationStatus::DryRunPendingApproval
                    },
                ));
            }
        }
    }

    LambdaCostRemediationWorkflow {
        workflow_id: "lambda_resilience_safe_remediation",
        read_only_mode: true,
        rbac_permission: "aws.lambda.resilience.remediation.approve",
        audit_stream: "lambda_resilience_remediation_audit",
        stale_data_blocks_execution: has_stale_data,
        actions,
        approval_gates: investigation.approval_gates,
    }
}

pub fn lambda_resilience_slo_policy_snapshot(
    report: &PillarReport,
) -> LambdaResilienceSloPolicySnapshot {
    let posture = lambda_resilience_posture_summary(report);
    let failed_rule_count = posture.rules_failed;
    let affected_resource_count = posture.affected_resources.len();
    let status =
        lambda_cost_objective_status(report.score, failed_rule_count, report.stale_resources);
    let owner_filters = sorted_unique_evidence_values(report, &["owner", "team"]);
    let environment_filters = sorted_unique_evidence_values(report, &["environment", "env"]);
    let application_filters = sorted_unique_evidence_values(report, &["application", "app"]);
    let notification_targets = lambda_notification_targets(&owner_filters, &environment_filters);

    LambdaCostSloPolicySnapshot {
        workflow_id: "lambda_resilience_slo_policy",
        read_only_mode: true,
        freshness_required: true,
        objective: LambdaCostPolicyObjective {
            objective_id: "lambda-resilience-score-min-95",
            status,
            target_score_min: 95,
            current_score: report.score,
            trend_direction: lambda_resilience_trend_direction(status, failed_rule_count),
            failed_rule_count,
            affected_resource_count,
            owner_filters,
            environment_filters,
            application_filters,
            notification_targets,
            policy_state: if report.stale_resources > 0 {
                "blocked_stale_data"
            } else if failed_rule_count > 0 {
                "active_with_findings"
            } else {
                "active"
            },
            status_history: vec![
                "snapshot_collected",
                "resilience_policy_evaluated",
                "notification_targets_resolved",
            ],
        },
        evidence_reason_codes: sorted_unique_reason_codes(report),
    }
}

pub fn lambda_resilience_forecast_snapshot(
    report: &PillarReport,
) -> LambdaResilienceForecastSnapshot {
    const BASELINE_WINDOW_DAYS: u16 = 30;
    const FORECAST_HORIZON_DAYS: u16 = 30;
    const CONFIDENCE_LEVEL: u8 = 75;

    let stale_count = count_reason(report, REASON_INV_STALE_DATA);
    let missing_config_count = count_reason(report, REASON_RES_MISSING_CONFIG_DATA);
    let telemetry_error_count = count_reason(report, REASON_RES_TELEMETRY_COLLECTION_ERRORS);
    let missing_metadata_count =
        count_reason(report, REASON_RES_MISSING_TELEMETRY_COLLECTION_METADATA);
    let missing_metrics_count = count_reason(report, REASON_RES_MISSING_CLOUDWATCH_TELEMETRY);
    let missing_log_event_count = count_reason(report, REASON_RES_MISSING_LOG_EVENT_EVIDENCE);
    let missing_quota_count = count_reason(report, REASON_RES_MISSING_QUOTA_LIMIT_EVIDENCE);
    let error_or_throttle_count = count_reason(report, REASON_RES_ERROR_OR_THROTTLE_HEALTH_SIGNAL);
    let timeout_headroom_count = count_reason(report, REASON_RES_LOW_TIMEOUT_HEADROOM);
    let blocked_by_stale_data = report.stale_resources > 0 || stale_count > 0;

    let expected_recovery_exposure_index = 100u16
        + (error_or_throttle_count as u16 * 28)
        + (timeout_headroom_count as u16 * 20)
        + (missing_metrics_count as u16 * 18)
        + (missing_log_event_count as u16 * 16)
        + (missing_quota_count as u16 * 14)
        + (missing_config_count as u16 * 12)
        + (missing_metadata_count as u16 * 10)
        + (telemetry_error_count as u16 * 20)
        + (report.stale_resources as u16 * 28);
    let uncertainty = 10u16
        + (missing_metrics_count as u16 * 8)
        + (missing_log_event_count as u16 * 7)
        + (missing_quota_count as u16 * 6)
        + (missing_metadata_count as u16 * 5)
        + (telemetry_error_count as u16 * 8)
        + (report.stale_resources as u16 * 12)
        + (report.resources_evaluated == 0) as u16 * 25;
    let lower_recovery_exposure_index =
        expected_recovery_exposure_index.saturating_sub(uncertainty);
    let upper_recovery_exposure_index = expected_recovery_exposure_index + uncertainty;
    let risk_level = if blocked_by_stale_data {
        LambdaResilienceForecastRisk::Blocked
    } else if error_or_throttle_count > 0 || upper_recovery_exposure_index >= 155 {
        LambdaResilienceForecastRisk::High
    } else if timeout_headroom_count > 0
        || missing_metrics_count > 0
        || missing_log_event_count > 0
        || missing_quota_count > 0
        || missing_config_count > 0
        || telemetry_error_count > 0
        || expected_recovery_exposure_index > 100
    {
        LambdaResilienceForecastRisk::Moderate
    } else {
        LambdaResilienceForecastRisk::Low
    };
    let risk_drivers = lambda_resilience_forecast_risk_drivers(report);
    let impacted_functions = sorted_unique_resources(
        risk_drivers
            .iter()
            .flat_map(|driver| driver.affected_resources.iter().cloned()),
    );

    LambdaResilienceForecastSnapshot {
        workflow_id: "lambda_resilience_forecasting",
        read_only_mode: true,
        baseline_window_days: BASELINE_WINDOW_DAYS,
        forecast_horizon_days: FORECAST_HORIZON_DAYS,
        confidence_level: CONFIDENCE_LEVEL,
        forecast_band: LambdaResilienceForecastBand {
            horizon_days: FORECAST_HORIZON_DAYS,
            lower_recovery_exposure_index,
            expected_recovery_exposure_index,
            upper_recovery_exposure_index,
            confidence_level: CONFIDENCE_LEVEL,
        },
        risk_level,
        recovery_capacity_risk: lambda_resilience_recovery_capacity_risk(
            blocked_by_stale_data,
            error_or_throttle_count,
            timeout_headroom_count,
            missing_metrics_count,
            missing_log_event_count,
            missing_quota_count,
        ),
        backtesting_fixture_status: if report.findings.is_empty() {
            "ready_clean_resilience_baseline"
        } else if blocked_by_stale_data
            || missing_metrics_count > 0
            || missing_log_event_count > 0
            || missing_quota_count > 0
            || telemetry_error_count > 0
        {
            "needs_fresh_resilience_telemetry_fixture"
        } else {
            "ready_resilience_findings_baseline"
        },
        threshold_controls: vec![
            "recovery_exposure_index_warning_threshold",
            "recovery_exposure_index_critical_threshold",
        ],
        what_if_inputs: vec![
            "restore_lambda_resilience_telemetry",
            "review_timeout_and_memory_headroom",
            "inspect_event_source_retry_and_dlq_configuration",
            "review_reserved_concurrency_and_account_limit_guardrails",
        ],
        blocked_by_stale_data,
        blast_radius_summary: if impacted_functions.is_empty() {
            "No Lambda functions have resilience forecast risk in the current evidence.".to_string()
        } else {
            format!(
                "{} Lambda function(s) have resilience forecast risk across timeout, throttle, log, event, and quota evidence.",
                impacted_functions.len()
            )
        },
        recovery_note:
            "Forecast is read-only; resilience changes require remediation approval and rollback notes.",
        missing_data_reason_codes: lambda_resilience_forecast_missing_data_reason_codes(report),
        risk_drivers,
        evidence_reason_codes: sorted_unique_reason_codes(report),
    }
}

pub fn lambda_resilience_reporting_bundle(
    report: &PillarReport,
) -> LambdaResilienceReportingBundle {
    let posture = lambda_resilience_posture_summary(report);
    let forecast = lambda_resilience_forecast_snapshot(report);
    let reason_codes = sorted_unique_reason_codes(report);
    let missing_data_reason_codes = lambda_resilience_reporting_missing_data_reason_codes(report);
    let rows = lambda_resilience_report_rows(report);
    let stale_data_blocks_delivery = report.stale_resources > 0
        || report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_INV_STALE_DATA);

    LambdaCostReportingBundle {
        workflow_id: "lambda_resilience_reporting",
        read_only_mode: true,
        scheduled_delivery_state: if stale_data_blocks_delivery {
            "blocked_until_fresh_resilience_evidence"
        } else if !missing_data_reason_codes.is_empty() {
            "ready_with_resilience_evidence_gaps"
        } else {
            "ready_for_schedule"
        },
        stale_data_blocks_delivery,
        portfolio_summary_ready: !stale_data_blocks_delivery,
        workload_summary_ready: !stale_data_blocks_delivery && report.resources_evaluated > 0,
        export_formats: vec!["json", "csv"],
        saved_view_id: "lambda-resilience-posture-report",
        executive_summary: LambdaCostExecutiveSummary {
            report_id: "lambda-resilience-executive-summary",
            score: report.score,
            resources_evaluated: report.resources_evaluated,
            stale_resources: report.stale_resources,
            rules_failed: posture.rules_failed,
            affected_resources: posture.affected_resources,
            top_reason_codes: reason_codes.clone(),
            blast_radius_summary: forecast.blast_radius_summary,
        },
        engineering_backlog: LambdaCostEngineeringBacklog {
            report_id: "lambda-resilience-engineering-backlog",
            page: 0,
            page_size: 50,
            total: rows.len(),
            rows: rows.clone(),
        },
        incident_review: LambdaCostIncidentReview {
            report_id: "lambda-resilience-incident-review",
            page: 0,
            page_size: 50,
            total: rows.len(),
            rows,
        },
        missing_data_reason_codes,
        evidence_reason_codes: reason_codes,
    }
}

fn evaluate_cost(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    evaluate_cost_telemetry(resource, findings);

    if !has_any_tag(&resource.tags, COST_ALLOCATION_TAG_KEYS) {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Cost,
            reason_code: REASON_COST_MISSING_ALLOCATION_TAGS.to_string(),
            severity: Severity::Medium,
            message: format!(
                "Function {} has no cost allocation tag (expected one of: {})",
                resource.resource_id,
                COST_ALLOCATION_TAG_KEYS.join(", ")
            ),
            evidence: json!({ "tags": resource.tags }),
        });
    }

    // arm64 (Graviton) is cheaper per GB-second; x86_64-only functions are a
    // deterministic savings opportunity worth review.
    let architectures: Vec<String> = resource
        .resource_data
        .get("architectures")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|a| a.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();
    if !architectures.is_empty() && architectures.iter().all(|a| a == "x86_64") {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Cost,
            reason_code: REASON_COST_X86_ONLY_ARCHITECTURE.to_string(),
            severity: Severity::Low,
            message: format!(
                "Function {} runs only on x86_64; evaluate arm64 (Graviton) for lower per-GB-second cost",
                resource.resource_id
            ),
            evidence: json!({
                "architectures": architectures,
                "tags": resource.tags,
            }),
        });
    }
}

fn evaluate_cost_telemetry(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    let required_fields = [
        "telemetry_collection_started_at",
        "telemetry_collection_completed_at",
        "telemetry_collection_duration_ms",
        "telemetry_collection_success_count",
        "telemetry_collection_failure_count",
        "telemetry_collection_error_count",
    ];
    let missing_collection_fields = missing_fields(resource, &required_fields);
    if !missing_collection_fields.is_empty() {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Cost,
            reason_code: REASON_COST_MISSING_TELEMETRY_COLLECTION_METADATA.to_string(),
            severity: Severity::Medium,
            message: format!(
                "Function {} is missing Lambda telemetry collection metadata needed to trust cost evidence",
                resource.resource_id
            ),
            evidence: json!({
                "required_fields": required_fields,
                "missing_fields": missing_collection_fields,
                "resource_data_keys": resource_data_keys(resource),
                "tags": resource.tags,
            }),
        });
    }

    let error_count =
        data_u64(&resource.resource_data, "telemetry_collection_error_count").unwrap_or(0);
    let errors = resource
        .resource_data
        .get("telemetry_collection_errors")
        .and_then(|value| value.as_array())
        .cloned()
        .unwrap_or_default();
    if error_count > 0 || !errors.is_empty() {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Cost,
            reason_code: REASON_COST_TELEMETRY_COLLECTION_ERRORS.to_string(),
            severity: Severity::High,
            message: format!(
                "Function {} has Lambda telemetry collection errors; cost posture may be incomplete",
                resource.resource_id
            ),
            evidence: json!({
                "telemetry_collection_error_count": error_count,
                "telemetry_collection_errors": errors,
                "tags": resource.tags,
            }),
        });
    }

    let missing_metrics = missing_metrics(resource, &lambda_cost_metric_names());
    if !missing_metrics.is_empty() {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Cost,
            reason_code: REASON_COST_MISSING_CLOUDWATCH_TELEMETRY.to_string(),
            severity: Severity::Medium,
            message: format!(
                "Function {} is missing Lambda CloudWatch cost telemetry for {}",
                resource.resource_id,
                missing_metrics.join(", ")
            ),
            evidence: json!({
                "required_metrics": lambda_cost_metric_names(),
                "missing_metrics": missing_metrics,
                "cloudwatch_metric_names": resource.resource_data.get("cloudwatch_metric_names"),
                "tags": resource.tags,
            }),
        });
    }

    if metric_max(resource, "Invocations") == Some(0.0) {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Cost,
            reason_code: REASON_COST_NO_INVOCATIONS_TELEMETRY.to_string(),
            severity: Severity::Low,
            message: format!(
                "Function {} has zero observed invocations in Lambda telemetry; review it as an unused-cost candidate",
                resource.resource_id
            ),
            evidence: json!({
                "invocations_max": 0.0,
                "tags": resource.tags,
            }),
        });
    }

    let errors_max = metric_max(resource, "Errors").unwrap_or(0.0);
    let throttles_max = metric_max(resource, "Throttles").unwrap_or(0.0);
    if errors_max > 0.0 || throttles_max > 0.0 {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Cost,
            reason_code: REASON_COST_ERROR_OR_THROTTLE_TELEMETRY.to_string(),
            severity: Severity::Low,
            message: format!(
                "Function {} has Lambda error or throttle telemetry that can waste retry and duration spend",
                resource.resource_id
            ),
            evidence: json!({
                "errors_max": errors_max,
                "throttles_max": throttles_max,
                "tags": resource.tags,
            }),
        });
    }
}

fn evaluate_security(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    if let Some(runtime) = resource
        .resource_data
        .get("runtime")
        .and_then(|v| v.as_str())
    {
        if DEPRECATED_RUNTIMES.contains(&runtime) {
            findings.push(InventoryFinding {
                resource_id: resource.resource_id.clone(),
                arn: resource.arn.clone(),
                pillar: Pillar::Security,
                reason_code: REASON_SEC_DEPRECATED_RUNTIME.to_string(),
                severity: Severity::High,
                message: format!(
                    "Function {} uses deprecated runtime {}; it no longer receives security patches",
                    resource.resource_id, runtime
                ),
                evidence: json!({ "runtime": runtime }),
            });
        }
    }

    if !has_any_tag(&resource.tags, OWNER_TAG_KEYS) {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Security,
            reason_code: REASON_SEC_MISSING_OWNER_TAG.to_string(),
            severity: Severity::Low,
            message: format!(
                "Function {} has no owner/team tag; security findings cannot be routed to an owner",
                resource.resource_id
            ),
            evidence: json!({ "tags": resource.tags }),
        });
    }
}

fn evaluate_resilience(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    let timeout = resource
        .resource_data
        .get("timeout")
        .and_then(|v| v.as_i64());
    let memory_size = resource
        .resource_data
        .get("memory_size")
        .and_then(|v| v.as_i64());
    if timeout.is_none() || memory_size.is_none() {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Resilience,
            reason_code: REASON_RES_MISSING_CONFIG_DATA.to_string(),
            severity: Severity::Medium,
            message: format!(
                "Function {} is missing timeout or memory configuration in inventory; resilience limits cannot be assessed",
                resource.resource_id
            ),
            evidence: json!({ "timeout": timeout, "memory_size": memory_size }),
        });
    }

    let required_fields = [
        "telemetry_collection_started_at",
        "telemetry_collection_completed_at",
        "telemetry_collection_duration_ms",
        "telemetry_collection_success_count",
        "telemetry_collection_failure_count",
        "telemetry_collection_error_count",
    ];
    let missing_resilience_collection_fields = missing_fields(resource, &required_fields);
    if !missing_resilience_collection_fields.is_empty() {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Resilience,
            reason_code: REASON_RES_MISSING_TELEMETRY_COLLECTION_METADATA.to_string(),
            severity: Severity::Medium,
            message: format!(
                "Function {} is missing Lambda resilience telemetry collection metadata needed to trust recovery evidence",
                resource.resource_id
            ),
            evidence: json!({
                "required_fields": required_fields,
                "missing_fields": missing_resilience_collection_fields,
                "resource_data_keys": resource_data_keys(resource),
                "tags": resource.tags,
            }),
        });
    }

    let telemetry_error_count =
        data_u64(&resource.resource_data, "telemetry_collection_error_count").unwrap_or(0);
    let telemetry_errors = resource
        .resource_data
        .get("telemetry_collection_errors")
        .and_then(|value| value.as_array())
        .cloned()
        .unwrap_or_default();
    if telemetry_error_count > 0 || !telemetry_errors.is_empty() {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Resilience,
            reason_code: REASON_RES_TELEMETRY_COLLECTION_ERRORS.to_string(),
            severity: Severity::High,
            message: format!(
                "Function {} has Lambda resilience telemetry collection errors; recovery posture may be incomplete",
                resource.resource_id
            ),
            evidence: json!({
                "telemetry_collection_error_count": telemetry_error_count,
                "telemetry_collection_errors": telemetry_errors,
                "tags": resource.tags,
            }),
        });
    }

    let missing_metrics = missing_metrics(resource, &lambda_resilience_metric_names());
    if !missing_metrics.is_empty() {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Resilience,
            reason_code: REASON_RES_MISSING_CLOUDWATCH_TELEMETRY.to_string(),
            severity: Severity::Medium,
            message: format!(
                "Function {} is missing Lambda CloudWatch resilience telemetry for {}",
                resource.resource_id,
                missing_metrics.join(", ")
            ),
            evidence: json!({
                "required_metrics": lambda_resilience_metric_names(),
                "missing_metrics": missing_metrics,
                "cloudwatch_metric_names": resource.resource_data.get("cloudwatch_metric_names"),
                "tags": resource.tags,
            }),
        });
    }

    let missing_log_event = missing_fields(
        resource,
        &[
            "recent_error_log_count",
            "event_source_mapping_count",
            "dead_letter_queue_configured",
        ],
    );
    if !missing_log_event.is_empty() {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Resilience,
            reason_code: REASON_RES_MISSING_LOG_EVENT_EVIDENCE.to_string(),
            severity: Severity::Medium,
            message: format!(
                "Function {} is missing log or event-path evidence needed to explain Lambda recovery behavior",
                resource.resource_id
            ),
            evidence: json!({
                "required_fields": [
                    "recent_error_log_count",
                    "event_source_mapping_count",
                    "dead_letter_queue_configured"
                ],
                "missing_fields": missing_log_event,
                "resource_data_keys": resource_data_keys(resource),
                "tags": resource.tags,
            }),
        });
    }

    let missing_quota_limits = missing_fields(
        resource,
        &[
            "reserved_concurrent_executions",
            "account_concurrency_limit",
        ],
    );
    if !missing_quota_limits.is_empty() {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Resilience,
            reason_code: REASON_RES_MISSING_QUOTA_LIMIT_EVIDENCE.to_string(),
            severity: Severity::Medium,
            message: format!(
                "Function {} is missing concurrency quota or limit evidence needed to explain throttle resilience",
                resource.resource_id
            ),
            evidence: json!({
                "required_fields": ["reserved_concurrent_executions", "account_concurrency_limit"],
                "missing_fields": missing_quota_limits,
                "resource_data_keys": resource_data_keys(resource),
                "tags": resource.tags,
            }),
        });
    }

    let duration_max = metric_max(resource, "Duration").unwrap_or(0.0);
    if let Some(timeout_seconds) = timeout {
        let timeout_millis = (timeout_seconds as f64) * 1000.0;
        if timeout_millis > 0.0 && duration_max >= timeout_millis * 0.8 {
            findings.push(InventoryFinding {
                resource_id: resource.resource_id.clone(),
                arn: resource.arn.clone(),
                pillar: Pillar::Resilience,
                reason_code: REASON_RES_LOW_TIMEOUT_HEADROOM.to_string(),
                severity: Severity::High,
                message: format!(
                    "Function {} is running close to its timeout budget; resilience headroom is low",
                    resource.resource_id
                ),
                evidence: json!({
                    "duration_max_ms": duration_max,
                    "timeout_seconds": timeout_seconds,
                    "timeout_headroom_ratio": ((timeout_millis - duration_max) / timeout_millis).max(0.0),
                    "tags": resource.tags,
                }),
            });
        }
    }

    let errors_max = metric_max(resource, "Errors").unwrap_or(0.0);
    let throttles_max = metric_max(resource, "Throttles").unwrap_or(0.0);
    if errors_max > 0.0 || throttles_max > 0.0 {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Resilience,
            reason_code: REASON_RES_ERROR_OR_THROTTLE_HEALTH_SIGNAL.to_string(),
            severity: Severity::High,
            message: format!(
                "Function {} has Lambda error or throttle telemetry that can degrade recovery behavior",
                resource.resource_id
            ),
            evidence: json!({
                "errors_max": errors_max,
                "throttles_max": throttles_max,
                "reserved_concurrent_executions": resource.resource_data.get("reserved_concurrent_executions"),
                "account_concurrency_limit": resource.resource_data.get("account_concurrency_limit"),
                "tags": resource.tags,
            }),
        });
    }
}

fn evaluate_performance(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    let timeout = resource
        .resource_data
        .get("timeout")
        .and_then(|value| value.as_i64());
    let memory_size = resource
        .resource_data
        .get("memory_size")
        .and_then(|value| value.as_i64());
    if timeout.is_none() || memory_size.is_none() {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Performance,
            reason_code: REASON_PERF_MISSING_CONFIG_DATA.to_string(),
            severity: Severity::Medium,
            message: format!(
                "Function {} is missing timeout or memory configuration in inventory; performance headroom cannot be assessed",
                resource.resource_id
            ),
            evidence: json!({ "timeout": timeout, "memory_size": memory_size }),
        });
    }

    let required_fields = [
        "telemetry_collection_started_at",
        "telemetry_collection_completed_at",
        "telemetry_collection_duration_ms",
        "telemetry_collection_success_count",
        "telemetry_collection_failure_count",
        "telemetry_collection_error_count",
    ];
    let missing_collection_fields = missing_fields(resource, &required_fields);
    if !missing_collection_fields.is_empty() {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Performance,
            reason_code: REASON_PERF_MISSING_TELEMETRY_COLLECTION_METADATA.to_string(),
            severity: Severity::Medium,
            message: format!(
                "Function {} is missing Lambda performance telemetry collection metadata needed to trust latency evidence",
                resource.resource_id
            ),
            evidence: json!({
                "required_fields": required_fields,
                "missing_fields": missing_collection_fields,
                "resource_data_keys": resource_data_keys(resource),
                "tags": resource.tags,
            }),
        });
    }

    let telemetry_error_count =
        data_u64(&resource.resource_data, "telemetry_collection_error_count").unwrap_or(0);
    let telemetry_errors = resource
        .resource_data
        .get("telemetry_collection_errors")
        .and_then(|value| value.as_array())
        .cloned()
        .unwrap_or_default();
    if telemetry_error_count > 0 || !telemetry_errors.is_empty() {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Performance,
            reason_code: REASON_PERF_TELEMETRY_COLLECTION_ERRORS.to_string(),
            severity: Severity::High,
            message: format!(
                "Function {} has Lambda performance telemetry collection errors; latency posture may be incomplete",
                resource.resource_id
            ),
            evidence: json!({
                "telemetry_collection_error_count": telemetry_error_count,
                "telemetry_collection_errors": telemetry_errors,
                "tags": resource.tags,
            }),
        });
    }

    let missing_metrics = missing_metrics(resource, &lambda_performance_metric_names());
    if !missing_metrics.is_empty() {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Performance,
            reason_code: REASON_PERF_MISSING_CLOUDWATCH_TELEMETRY.to_string(),
            severity: Severity::Medium,
            message: format!(
                "Function {} is missing Lambda CloudWatch performance telemetry for {}",
                resource.resource_id,
                missing_metrics.join(", ")
            ),
            evidence: json!({
                "required_metrics": lambda_performance_metric_names(),
                "missing_metrics": missing_metrics,
                "cloudwatch_metric_names": resource.resource_data.get("cloudwatch_metric_names"),
                "tags": resource.tags,
            }),
        });
    }

    let duration_max = metric_max(resource, "Duration").unwrap_or(0.0);
    if let Some(timeout_seconds) = timeout {
        let timeout_millis = (timeout_seconds as f64) * 1000.0;
        if timeout_millis > 0.0 && duration_max >= timeout_millis * 0.7 {
            findings.push(InventoryFinding {
                resource_id: resource.resource_id.clone(),
                arn: resource.arn.clone(),
                pillar: Pillar::Performance,
                reason_code: REASON_PERF_HIGH_DURATION_PRESSURE.to_string(),
                severity: Severity::High,
                message: format!(
                    "Function {} is using a high share of its timeout budget; latency headroom is constrained",
                    resource.resource_id
                ),
                evidence: json!({
                    "duration_max_ms": duration_max,
                    "timeout_seconds": timeout_seconds,
                    "timeout_budget_used_ratio": (duration_max / timeout_millis).min(1.0),
                    "memory_size": memory_size,
                    "tags": resource.tags,
                }),
            });
        }
    }

    let errors_max = metric_max(resource, "Errors").unwrap_or(0.0);
    let throttles_max = metric_max(resource, "Throttles").unwrap_or(0.0);
    if errors_max > 0.0 || throttles_max > 0.0 {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Performance,
            reason_code: REASON_PERF_ERROR_OR_THROTTLE_PRESSURE.to_string(),
            severity: Severity::High,
            message: format!(
                "Function {} has Lambda error or throttle pressure that can degrade request latency",
                resource.resource_id
            ),
            evidence: json!({
                "errors_max": errors_max,
                "throttles_max": throttles_max,
                "reserved_concurrent_executions": resource.resource_data.get("reserved_concurrent_executions"),
                "tags": resource.tags,
            }),
        });
    }
}

fn evaluate_scalability(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    let reserved_concurrency = resource
        .resource_data
        .get("reserved_concurrent_executions")
        .and_then(|value| value.as_i64());
    let account_concurrency_limit = resource
        .resource_data
        .get("account_concurrency_limit")
        .and_then(|value| value.as_i64());
    if reserved_concurrency.is_none() || account_concurrency_limit.is_none() {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Scalability,
            reason_code: REASON_SCAL_MISSING_CONCURRENCY_LIMIT_EVIDENCE.to_string(),
            severity: Severity::Medium,
            message: format!(
                "Function {} is missing reserved or account concurrency limit evidence; scalability headroom cannot be assessed",
                resource.resource_id
            ),
            evidence: json!({
                "reserved_concurrent_executions": reserved_concurrency,
                "account_concurrency_limit": account_concurrency_limit,
                "tags": resource.tags,
            }),
        });
    }

    let required_fields = [
        "telemetry_collection_started_at",
        "telemetry_collection_completed_at",
        "telemetry_collection_duration_ms",
        "telemetry_collection_success_count",
        "telemetry_collection_failure_count",
        "telemetry_collection_error_count",
    ];
    let missing_collection_fields = missing_fields(resource, &required_fields);
    if !missing_collection_fields.is_empty() {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Scalability,
            reason_code: REASON_SCAL_MISSING_TELEMETRY_COLLECTION_METADATA.to_string(),
            severity: Severity::Medium,
            message: format!(
                "Function {} is missing Lambda scalability telemetry collection metadata needed to trust concurrency evidence",
                resource.resource_id
            ),
            evidence: json!({
                "required_fields": required_fields,
                "missing_fields": missing_collection_fields,
                "resource_data_keys": resource_data_keys(resource),
                "tags": resource.tags,
            }),
        });
    }

    let telemetry_error_count =
        data_u64(&resource.resource_data, "telemetry_collection_error_count").unwrap_or(0);
    let telemetry_errors = resource
        .resource_data
        .get("telemetry_collection_errors")
        .and_then(|value| value.as_array())
        .cloned()
        .unwrap_or_default();
    if telemetry_error_count > 0 || !telemetry_errors.is_empty() {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Scalability,
            reason_code: REASON_SCAL_TELEMETRY_COLLECTION_ERRORS.to_string(),
            severity: Severity::High,
            message: format!(
                "Function {} has Lambda scalability telemetry collection errors; concurrency posture may be incomplete",
                resource.resource_id
            ),
            evidence: json!({
                "telemetry_collection_error_count": telemetry_error_count,
                "telemetry_collection_errors": telemetry_errors,
                "tags": resource.tags,
            }),
        });
    }

    let missing_metrics = missing_metrics(resource, &lambda_scalability_metric_names());
    if !missing_metrics.is_empty() {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Scalability,
            reason_code: REASON_SCAL_MISSING_CLOUDWATCH_TELEMETRY.to_string(),
            severity: Severity::Medium,
            message: format!(
                "Function {} is missing Lambda CloudWatch scalability telemetry for {}",
                resource.resource_id,
                missing_metrics.join(", ")
            ),
            evidence: json!({
                "required_metrics": lambda_scalability_metric_names(),
                "missing_metrics": missing_metrics,
                "cloudwatch_metric_names": resource.resource_data.get("cloudwatch_metric_names"),
                "tags": resource.tags,
            }),
        });
    }

    let concurrent_max = metric_max(resource, "ConcurrentExecutions").unwrap_or(0.0);
    let concurrency_limit = reserved_concurrency
        .or(account_concurrency_limit)
        .map(|value| value as f64)
        .unwrap_or(0.0);
    if concurrency_limit > 0.0 && concurrent_max >= concurrency_limit * 0.8 {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Scalability,
            reason_code: REASON_SCAL_HIGH_CONCURRENCY_UTILIZATION.to_string(),
            severity: Severity::High,
            message: format!(
                "Function {} is using most of its Lambda concurrency budget; scalability headroom is constrained",
                resource.resource_id
            ),
            evidence: json!({
                "concurrent_executions_max": concurrent_max,
                "concurrency_limit": concurrency_limit,
                "concurrency_utilization_ratio": (concurrent_max / concurrency_limit).min(1.0),
                "reserved_concurrent_executions": reserved_concurrency,
                "account_concurrency_limit": account_concurrency_limit,
                "tags": resource.tags,
            }),
        });
    }

    let throttles_max = metric_max(resource, "Throttles").unwrap_or(0.0);
    if throttles_max > 0.0 {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Scalability,
            reason_code: REASON_SCAL_THROTTLE_PRESSURE.to_string(),
            severity: Severity::High,
            message: format!(
                "Function {} has Lambda throttle pressure that can block scale-out",
                resource.resource_id
            ),
            evidence: json!({
                "throttles_max": throttles_max,
                "reserved_concurrent_executions": reserved_concurrency,
                "account_concurrency_limit": account_concurrency_limit,
                "tags": resource.tags,
            }),
        });
    }
}

fn lambda_cost_posture_rule(
    report: &PillarReport,
    rule_id: &'static str,
    reason_codes: &[&'static str],
) -> LambdaPostureRule {
    let affected_resources = sorted_unique_resources(
        report
            .findings
            .iter()
            .filter(|finding| reason_codes.contains(&finding.reason_code.as_str()))
            .map(|finding| finding.resource_id.clone()),
    );

    LambdaPostureRule {
        rule_id,
        status: if affected_resources.is_empty() {
            LambdaPostureStatus::Pass
        } else {
            LambdaPostureStatus::Fail
        },
        reason_codes: reason_codes.to_vec(),
        affected_resources,
        suppression_supported: true,
        assignment_supported: true,
    }
}

fn lambda_cost_posture_recommendations(
    report: &PillarReport,
) -> Vec<LambdaCostPostureRecommendation> {
    report
        .findings
        .iter()
        .filter_map(|finding| {
            let recommendation = match finding.reason_code.as_str() {
                REASON_INV_STALE_DATA => "refresh_lambda_inventory_and_cost_telemetry",
                REASON_COST_MISSING_TELEMETRY_COLLECTION_METADATA => {
                    "restore_lambda_cost_collection_metadata"
                }
                REASON_COST_TELEMETRY_COLLECTION_ERRORS => {
                    "inspect_lambda_cloudwatch_collection_errors"
                }
                REASON_COST_MISSING_CLOUDWATCH_TELEMETRY => {
                    "collect_lambda_invocation_duration_error_and_throttle_metrics"
                }
                REASON_COST_MISSING_ALLOCATION_TAGS => "assign_lambda_cost_owner_metadata",
                REASON_COST_X86_ONLY_ARCHITECTURE => "evaluate_arm64_runtime_compatibility",
                REASON_COST_NO_INVOCATIONS_TELEMETRY => "review_unused_lambda_function",
                REASON_COST_ERROR_OR_THROTTLE_TELEMETRY => {
                    "diagnose_retry_error_and_throttle_cost_waste"
                }
                _ => return None,
            };

            Some(LambdaCostPostureRecommendation {
                resource_id: finding.resource_id.clone(),
                reason_code: finding.reason_code.clone(),
                recommendation,
                owner: owner_from_evidence(&finding.evidence),
                confidence: "medium",
                effort: recommendation_effort(finding.reason_code.as_str()),
                risk: recommendation_risk(finding.reason_code.as_str()),
                suppression_key: format!("{}:{}", finding.resource_id, finding.reason_code),
                audit_event_type: "lambda_cost_posture_recommendation_emitted",
            })
        })
        .collect()
}

fn recommendation_effort(reason_code: &str) -> &'static str {
    match reason_code {
        REASON_COST_X86_ONLY_ARCHITECTURE | REASON_COST_ERROR_OR_THROTTLE_TELEMETRY => "medium",
        _ => "low",
    }
}

fn recommendation_risk(reason_code: &str) -> &'static str {
    match reason_code {
        REASON_COST_X86_ONLY_ARCHITECTURE | REASON_COST_ERROR_OR_THROTTLE_TELEMETRY => "medium",
        _ => "low",
    }
}

fn owner_from_evidence(evidence: &Value) -> Option<String> {
    evidence
        .get("tags")
        .and_then(|tags| tag_string(tags, &["owner", "team", "cost-center", "project"]))
}

fn tag_string(tags: &Value, keys: &[&str]) -> Option<String> {
    keys.iter()
        .filter_map(|key| tags.get(key))
        .filter_map(Value::as_str)
        .map(str::trim)
        .find(|value| !value.is_empty())
        .map(str::to_string)
}

fn lambda_investigation_step(
    existing_steps: &[LambdaInvestigationStep],
    kind: LambdaInvestigationStepKind,
    tool_name: &'static str,
    tool_mode: LambdaInvestigationToolMode,
    citation: &LambdaEvidenceCitation,
    stop_condition: &'static str,
) -> LambdaInvestigationStep {
    LambdaInvestigationStep {
        step_id: format!("lambda-cost-step-{:02}", existing_steps.len() + 1),
        kind,
        tool_name,
        tool_mode,
        target_resource_id: citation.resource_id.clone(),
        reason_code: citation.reason_code.clone(),
        stop_condition: stop_condition.to_string(),
        evidence: citation.evidence.clone(),
    }
}

fn lambda_mutation_gate(
    existing_gates: &[LambdaMutationApprovalGate],
    citation: &LambdaEvidenceCitation,
    required_approval: &'static str,
) -> LambdaMutationApprovalGate {
    LambdaMutationApprovalGate {
        gate_id: format!("lambda-cost-gate-{:02}", existing_gates.len() + 1),
        target_resource_id: citation.resource_id.clone(),
        required_approval,
        blast_radius: format!(
            "Potential Lambda cost mutation affects {} for {}",
            citation.resource_id, citation.reason_code
        ),
        rollback_note_required: true,
        evidence_reason_codes: vec![citation.reason_code.clone()],
    }
}

fn lambda_remediation_action_kind(reason_code: &str) -> Option<LambdaRemediationActionKind> {
    match reason_code {
        REASON_COST_MISSING_ALLOCATION_TAGS => Some(LambdaRemediationActionKind::AssignCostTags),
        REASON_COST_X86_ONLY_ARCHITECTURE => Some(LambdaRemediationActionKind::PlanArm64Migration),
        REASON_COST_NO_INVOCATIONS_TELEMETRY => {
            Some(LambdaRemediationActionKind::ReviewUnusedFunctionCleanup)
        }
        REASON_COST_ERROR_OR_THROTTLE_TELEMETRY => {
            Some(LambdaRemediationActionKind::ReviewRetryThrottleCostControls)
        }
        _ => None,
    }
}

fn lambda_resilience_remediation_action_kind(
    reason_code: &str,
) -> Option<LambdaRemediationActionKind> {
    match reason_code {
        REASON_RES_MISSING_LOG_EVENT_EVIDENCE => {
            Some(LambdaRemediationActionKind::ReviewTelemetryCoverage)
        }
        REASON_RES_MISSING_QUOTA_LIMIT_EVIDENCE => {
            Some(LambdaRemediationActionKind::ReviewConcurrencyLimitGuardrails)
        }
        REASON_RES_ERROR_OR_THROTTLE_HEALTH_SIGNAL => {
            Some(LambdaRemediationActionKind::ReviewFailureAndThrottleRecovery)
        }
        REASON_RES_LOW_TIMEOUT_HEADROOM | REASON_RES_MISSING_CONFIG_DATA => {
            Some(LambdaRemediationActionKind::ReviewTimeoutHeadroom)
        }
        _ => None,
    }
}

fn lambda_remediation_action(
    existing_actions: &[LambdaCostRemediationAction],
    kind: LambdaRemediationActionKind,
    gate: &LambdaMutationApprovalGate,
    status: LambdaRemediationStatus,
) -> LambdaCostRemediationAction {
    let action_number = existing_actions.len() + 1;
    let action_slug = match kind {
        LambdaRemediationActionKind::AssignCostTags => "assign-cost-tags",
        LambdaRemediationActionKind::PlanArm64Migration => "plan-arm64-migration",
        LambdaRemediationActionKind::ReviewUnusedFunctionCleanup => {
            "review-unused-function-cleanup"
        }
        LambdaRemediationActionKind::ReviewRetryThrottleCostControls => {
            "review-retry-throttle-cost-controls"
        }
        LambdaRemediationActionKind::ReviewTelemetryCoverage => "review-telemetry-coverage",
        LambdaRemediationActionKind::ReviewTimeoutHeadroom => "review-timeout-headroom",
        LambdaRemediationActionKind::ReviewFailureAndThrottleRecovery => {
            "review-failure-and-throttle-recovery"
        }
        LambdaRemediationActionKind::ReviewConcurrencyLimitGuardrails => {
            "review-concurrency-limit-guardrails"
        }
    };

    LambdaCostRemediationAction {
        action_id: format!("lambda-cost-remediation-{:02}", action_number),
        kind,
        status,
        target_resource_id: gate.target_resource_id.clone(),
        dry_run: true,
        requires_approval: true,
        approval_gate_id: Some(gate.gate_id.clone()),
        audit_event_type: "lambda.cost.remediation.dry_run_planned",
        idempotency_key: format!("lambda-cost-{}-{}", gate.target_resource_id, action_slug),
        blast_radius: gate.blast_radius.clone(),
        rollback_note: format!(
            "Before approval, record rollback or recovery notes for {} on {}.",
            action_slug, gate.target_resource_id
        ),
        validation_steps: vec![
            "refresh Lambda inventory, tag, architecture, invocation, error, and throttle evidence",
            "verify owner, blast radius, estimated savings, compatibility, and traffic expectations",
            "capture operator approval, rollback note, and audit id before execution",
        ],
        evidence_reason_codes: gate.evidence_reason_codes.clone(),
    }
}

fn lambda_remediation_action_with_contract(
    id_prefix: &str,
    audit_event_type: &'static str,
    validation_steps: &[&'static str],
    existing_actions: &[LambdaCostRemediationAction],
    kind: LambdaRemediationActionKind,
    gate: &LambdaMutationApprovalGate,
    status: LambdaRemediationStatus,
) -> LambdaCostRemediationAction {
    let action_number = existing_actions.len() + 1;
    let action_slug = match kind {
        LambdaRemediationActionKind::AssignCostTags => "assign-cost-tags",
        LambdaRemediationActionKind::PlanArm64Migration => "plan-arm64-migration",
        LambdaRemediationActionKind::ReviewUnusedFunctionCleanup => {
            "review-unused-function-cleanup"
        }
        LambdaRemediationActionKind::ReviewRetryThrottleCostControls => {
            "review-retry-throttle-cost-controls"
        }
        LambdaRemediationActionKind::ReviewTelemetryCoverage => "review-telemetry-coverage",
        LambdaRemediationActionKind::ReviewTimeoutHeadroom => "review-timeout-headroom",
        LambdaRemediationActionKind::ReviewFailureAndThrottleRecovery => {
            "review-failure-and-throttle-recovery"
        }
        LambdaRemediationActionKind::ReviewConcurrencyLimitGuardrails => {
            "review-concurrency-limit-guardrails"
        }
    };

    LambdaCostRemediationAction {
        action_id: format!("{id_prefix}-remediation-{action_number:02}"),
        kind,
        status,
        target_resource_id: gate.target_resource_id.clone(),
        dry_run: true,
        requires_approval: true,
        approval_gate_id: Some(gate.gate_id.clone()),
        audit_event_type,
        idempotency_key: format!("{id_prefix}-{}-{action_slug}", gate.target_resource_id),
        blast_radius: gate.blast_radius.clone(),
        rollback_note: format!(
            "Before approval, record rollback or recovery notes for {action_slug} on {}.",
            gate.target_resource_id
        ),
        validation_steps: validation_steps.to_vec(),
        evidence_reason_codes: gate.evidence_reason_codes.clone(),
    }
}

fn lambda_cost_objective_status(
    score: u8,
    failed_rule_count: usize,
    stale_resources: usize,
) -> LambdaCostObjectiveStatus {
    if stale_resources > 0 || score < 70 {
        LambdaCostObjectiveStatus::Breached
    } else if failed_rule_count > 0 || score < 90 {
        LambdaCostObjectiveStatus::AtRisk
    } else {
        LambdaCostObjectiveStatus::OnTrack
    }
}

fn lambda_cost_trend_direction(
    status: LambdaCostObjectiveStatus,
    failed_rule_count: usize,
) -> LambdaCostTrendDirection {
    match status {
        LambdaCostObjectiveStatus::OnTrack => LambdaCostTrendDirection::Stable,
        LambdaCostObjectiveStatus::AtRisk if failed_rule_count <= 1 => {
            LambdaCostTrendDirection::Stable
        }
        LambdaCostObjectiveStatus::AtRisk | LambdaCostObjectiveStatus::Breached => {
            LambdaCostTrendDirection::Degrading
        }
    }
}

fn lambda_resilience_trend_direction(
    status: LambdaResilienceObjectiveStatus,
    failed_rule_count: usize,
) -> LambdaResilienceTrendDirection {
    match status {
        LambdaResilienceObjectiveStatus::OnTrack => LambdaResilienceTrendDirection::Stable,
        LambdaResilienceObjectiveStatus::AtRisk | LambdaResilienceObjectiveStatus::Breached => {
            if failed_rule_count <= 1 {
                LambdaResilienceTrendDirection::Stable
            } else {
                LambdaResilienceTrendDirection::Degrading
            }
        }
    }
}

fn sorted_unique_reason_codes(report: &PillarReport) -> Vec<String> {
    sorted_unique_reasons(
        report
            .findings
            .iter()
            .map(|finding| finding.reason_code.clone()),
    )
}

fn lambda_cost_reporting_missing_data_reason_codes(report: &PillarReport) -> Vec<String> {
    [
        REASON_INV_STALE_DATA,
        REASON_COST_MISSING_TELEMETRY_COLLECTION_METADATA,
        REASON_COST_TELEMETRY_COLLECTION_ERRORS,
        REASON_COST_MISSING_CLOUDWATCH_TELEMETRY,
    ]
    .into_iter()
    .filter(|reason_code| {
        report
            .findings
            .iter()
            .any(|finding| finding.reason_code == *reason_code)
    })
    .map(str::to_string)
    .collect()
}

fn lambda_resilience_reporting_missing_data_reason_codes(report: &PillarReport) -> Vec<String> {
    [
        REASON_INV_STALE_DATA,
        REASON_RES_MISSING_CONFIG_DATA,
        REASON_RES_MISSING_TELEMETRY_COLLECTION_METADATA,
        REASON_RES_TELEMETRY_COLLECTION_ERRORS,
        REASON_RES_MISSING_CLOUDWATCH_TELEMETRY,
        REASON_RES_MISSING_LOG_EVENT_EVIDENCE,
        REASON_RES_MISSING_QUOTA_LIMIT_EVIDENCE,
    ]
    .into_iter()
    .filter(|reason_code| {
        report
            .findings
            .iter()
            .any(|finding| finding.reason_code == *reason_code)
    })
    .map(str::to_string)
    .collect()
}

fn lambda_cost_report_rows(report: &PillarReport) -> Vec<LambdaCostReportRow> {
    report
        .findings
        .iter()
        .map(|finding| LambdaCostReportRow {
            resource_id: finding.resource_id.clone(),
            severity: finding.severity,
            reason_code: finding.reason_code.clone(),
            message: finding.message.clone(),
            recovery_note: lambda_cost_reporting_recovery_note(&finding.reason_code).to_string(),
            suppression_supported: true,
            evidence: finding.evidence.clone(),
        })
        .collect()
}

fn lambda_resilience_report_rows(report: &PillarReport) -> Vec<LambdaResilienceReportRow> {
    report
        .findings
        .iter()
        .map(|finding| LambdaResilienceReportRow {
            resource_id: finding.resource_id.clone(),
            severity: finding.severity,
            reason_code: finding.reason_code.clone(),
            message: finding.message.clone(),
            recovery_note: lambda_resilience_reporting_recovery_note(&finding.reason_code)
                .to_string(),
            suppression_supported: true,
            evidence: finding.evidence.clone(),
        })
        .collect()
}

fn lambda_cost_reporting_recovery_note(reason_code: &str) -> &'static str {
    match reason_code {
        REASON_INV_STALE_DATA => {
            "Refresh Lambda inventory and telemetry before sharing the cost report."
        }
        REASON_COST_MISSING_TELEMETRY_COLLECTION_METADATA => {
            "Collect telemetry run metadata before scheduling Lambda cost report delivery."
        }
        REASON_COST_TELEMETRY_COLLECTION_ERRORS => {
            "Resolve Lambda collector errors before publishing cost report findings."
        }
        REASON_COST_MISSING_CLOUDWATCH_TELEMETRY => {
            "Collect Invocations, Duration, Errors, and Throttles before quantifying Lambda cost action."
        }
        REASON_COST_MISSING_ALLOCATION_TAGS => {
            "Add owner, environment, and application tags before routing Lambda cost findings."
        }
        REASON_COST_X86_ONLY_ARCHITECTURE => {
            "Review arm64 compatibility and GB-second savings before scheduling architecture changes."
        }
        REASON_COST_NO_INVOCATIONS_TELEMETRY => {
            "Confirm the function is unused across retention windows before cleanup is scheduled."
        }
        REASON_COST_ERROR_OR_THROTTLE_TELEMETRY => {
            "Review retry, timeout, and concurrency behavior before scheduling cost remediation."
        }
        _ => "Collect fresh evidence before sharing this Lambda cost report row.",
    }
}

fn lambda_resilience_reporting_recovery_note(reason_code: &str) -> &'static str {
    match reason_code {
        REASON_INV_STALE_DATA => {
            "Refresh Lambda inventory and resilience telemetry before sharing the resilience report."
        }
        REASON_RES_MISSING_CONFIG_DATA => {
            "Collect timeout and memory configuration before assessing Lambda recovery headroom."
        }
        REASON_RES_MISSING_TELEMETRY_COLLECTION_METADATA => {
            "Collect resilience telemetry run metadata before scheduling Lambda resilience report delivery."
        }
        REASON_RES_TELEMETRY_COLLECTION_ERRORS => {
            "Resolve Lambda resilience collector errors before publishing recovery findings."
        }
        REASON_RES_MISSING_CLOUDWATCH_TELEMETRY => {
            "Collect Duration, Errors, and Throttles before quantifying Lambda resilience action."
        }
        REASON_RES_MISSING_LOG_EVENT_EVIDENCE => {
            "Collect log subscription, event source mapping, and DLQ evidence before scheduling resilience remediation."
        }
        REASON_RES_MISSING_QUOTA_LIMIT_EVIDENCE => {
            "Collect reserved concurrency and account limit evidence before scheduling throttle resilience changes."
        }
        REASON_RES_ERROR_OR_THROTTLE_HEALTH_SIGNAL => {
            "Review error, throttle, retry, and downstream dependency behavior before scheduling resilience remediation."
        }
        REASON_RES_LOW_TIMEOUT_HEADROOM => {
            "Review timeout, memory, and latency headroom before scheduling resilience limit changes."
        }
        _ => "Collect fresh evidence before sharing this Lambda resilience report row.",
    }
}

fn count_reason(report: &PillarReport, reason_code: &str) -> usize {
    report
        .findings
        .iter()
        .filter(|finding| finding.reason_code == reason_code)
        .count()
}

fn resources_for_reason(report: &PillarReport, reason_code: &str) -> Vec<String> {
    sorted_unique_resources(
        report
            .findings
            .iter()
            .filter(|finding| finding.reason_code == reason_code)
            .map(|finding| finding.resource_id.clone()),
    )
}

fn lambda_cost_forecast_missing_data_reason_codes(report: &PillarReport) -> Vec<String> {
    sorted_unique_reasons(
        report
            .findings
            .iter()
            .filter(|finding| {
                matches!(
                    finding.reason_code.as_str(),
                    REASON_INV_STALE_DATA
                        | REASON_COST_MISSING_TELEMETRY_COLLECTION_METADATA
                        | REASON_COST_TELEMETRY_COLLECTION_ERRORS
                        | REASON_COST_MISSING_CLOUDWATCH_TELEMETRY
                )
            })
            .map(|finding| finding.reason_code.clone()),
    )
}

fn lambda_resilience_forecast_missing_data_reason_codes(report: &PillarReport) -> Vec<String> {
    sorted_unique_reasons(
        report
            .findings
            .iter()
            .filter(|finding| {
                matches!(
                    finding.reason_code.as_str(),
                    REASON_INV_STALE_DATA
                        | REASON_RES_MISSING_CONFIG_DATA
                        | REASON_RES_MISSING_TELEMETRY_COLLECTION_METADATA
                        | REASON_RES_TELEMETRY_COLLECTION_ERRORS
                        | REASON_RES_MISSING_CLOUDWATCH_TELEMETRY
                        | REASON_RES_MISSING_LOG_EVENT_EVIDENCE
                        | REASON_RES_MISSING_QUOTA_LIMIT_EVIDENCE
                )
            })
            .map(|finding| finding.reason_code.clone()),
    )
}

fn lambda_cost_forecast_risk_drivers(report: &PillarReport) -> Vec<LambdaCostForecastRiskDriver> {
    [
        (REASON_INV_STALE_DATA, 25u16),
        (REASON_COST_TELEMETRY_COLLECTION_ERRORS, 20u16),
        (REASON_COST_ERROR_OR_THROTTLE_TELEMETRY, 20u16),
        (REASON_COST_NO_INVOCATIONS_TELEMETRY, 18u16),
        (REASON_COST_MISSING_CLOUDWATCH_TELEMETRY, 16u16),
        (REASON_COST_MISSING_TELEMETRY_COLLECTION_METADATA, 12u16),
        (REASON_COST_X86_ONLY_ARCHITECTURE, 10u16),
        (REASON_COST_MISSING_ALLOCATION_TAGS, 4u16),
    ]
    .into_iter()
    .filter_map(|(reason_code, delta)| {
        let affected_resources = resources_for_reason(report, reason_code);
        if affected_resources.is_empty() {
            None
        } else {
            Some(LambdaCostForecastRiskDriver {
                reason_code: reason_code.to_string(),
                monthly_cost_index_delta: delta * affected_resources.len() as u16,
                affected_resources,
            })
        }
    })
    .collect()
}

fn lambda_resilience_forecast_risk_drivers(
    report: &PillarReport,
) -> Vec<LambdaResilienceForecastRiskDriver> {
    [
        (REASON_INV_STALE_DATA, 28u16),
        (REASON_RES_TELEMETRY_COLLECTION_ERRORS, 20u16),
        (REASON_RES_ERROR_OR_THROTTLE_HEALTH_SIGNAL, 28u16),
        (REASON_RES_LOW_TIMEOUT_HEADROOM, 20u16),
        (REASON_RES_MISSING_CLOUDWATCH_TELEMETRY, 18u16),
        (REASON_RES_MISSING_LOG_EVENT_EVIDENCE, 16u16),
        (REASON_RES_MISSING_QUOTA_LIMIT_EVIDENCE, 14u16),
        (REASON_RES_MISSING_CONFIG_DATA, 12u16),
        (REASON_RES_MISSING_TELEMETRY_COLLECTION_METADATA, 10u16),
    ]
    .into_iter()
    .filter_map(|(reason_code, delta)| {
        let affected_resources = resources_for_reason(report, reason_code);
        if affected_resources.is_empty() {
            None
        } else {
            Some(LambdaResilienceForecastRiskDriver {
                reason_code: reason_code.to_string(),
                recovery_exposure_index_delta: delta * affected_resources.len() as u16,
                affected_resources,
            })
        }
    })
    .collect()
}

fn lambda_cost_capacity_risk(
    forecast_blocked: bool,
    no_invocations_count: usize,
    error_or_throttle_count: usize,
    missing_cloudwatch_count: usize,
    missing_collection_metadata_count: usize,
) -> &'static str {
    if forecast_blocked {
        "blocked_by_stale_or_failed_collection"
    } else if error_or_throttle_count > 0 {
        "retry_or_throttle_cost_pressure"
    } else if no_invocations_count > 0 {
        "unused_function_savings_opportunity"
    } else if missing_cloudwatch_count > 0 || missing_collection_metadata_count > 0 {
        "telemetry_gap_limits_forecast"
    } else {
        "within_lambda_cost_forecast_threshold"
    }
}

fn lambda_resilience_recovery_capacity_risk(
    blocked_by_stale_data: bool,
    error_or_throttle_count: usize,
    timeout_headroom_count: usize,
    missing_metrics_count: usize,
    missing_log_event_count: usize,
    missing_quota_count: usize,
) -> &'static str {
    if blocked_by_stale_data {
        "blocked_until_inventory_refresh"
    } else if error_or_throttle_count > 0 {
        "active_error_or_throttle_recovery_exposure"
    } else if timeout_headroom_count > 0 {
        "timeout_headroom_recovery_pressure"
    } else if missing_metrics_count > 0 || missing_log_event_count > 0 || missing_quota_count > 0 {
        "telemetry_gap_limits_resilience_forecast"
    } else {
        "within_lambda_resilience_forecast_threshold"
    }
}

fn sorted_unique_evidence_values(report: &PillarReport, keys: &[&str]) -> Vec<String> {
    report
        .findings
        .iter()
        .filter_map(|finding| finding.evidence.get("tags"))
        .filter_map(|tags| tag_string(tags, keys))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn lambda_notification_targets(
    owner_filters: &[String],
    environment_filters: &[String],
) -> Vec<String> {
    let mut targets = Vec::new();
    targets.extend(
        environment_filters
            .iter()
            .map(|environment| format!("environment:{environment}")),
    );
    targets.extend(owner_filters.iter().map(|owner| format!("owner:{owner}")));
    if targets.is_empty() {
        targets.push("owner:unassigned".to_string());
    }
    targets
}

fn lambda_cost_metric_names() -> [&'static str; 4] {
    ["Invocations", "Duration", "Errors", "Throttles"]
}

fn lambda_resilience_metric_names() -> [&'static str; 3] {
    ["Duration", "Errors", "Throttles"]
}

fn lambda_performance_metric_names() -> [&'static str; 4] {
    ["Duration", "Errors", "Throttles", "ConcurrentExecutions"]
}

fn lambda_scalability_metric_names() -> [&'static str; 3] {
    ["Invocations", "Throttles", "ConcurrentExecutions"]
}

fn lambda_scalability_follow_up_question(reason_code: &str) -> String {
    match reason_code {
        REASON_INV_STALE_DATA => {
            "Has Lambda inventory and scalability telemetry been refreshed in the current sync window?"
        }
        REASON_SCAL_MISSING_CONCURRENCY_LIMIT_EVIDENCE => {
            "Which reserved concurrency or account concurrency limit evidence is missing?"
        }
        REASON_SCAL_MISSING_TELEMETRY_COLLECTION_METADATA => {
            "Which collector run should be used as evidence for Lambda scalability telemetry completeness?"
        }
        REASON_SCAL_TELEMETRY_COLLECTION_ERRORS => {
            "Which CloudWatch permission, throttling, or retry failure prevented Lambda scalability telemetry collection?"
        }
        REASON_SCAL_MISSING_CLOUDWATCH_TELEMETRY => {
            "Which Invocations, Throttles, or ConcurrentExecutions datapoints are missing?"
        }
        REASON_SCAL_HIGH_CONCURRENCY_UTILIZATION => {
            "Is concurrency constrained by reserved limits, account quota, burst traffic, or event source scaling?"
        }
        REASON_SCAL_THROTTLE_PRESSURE => {
            "Which event source, reserved concurrency limit, or upstream retry loop is driving Lambda throttles?"
        }
        _ => "What additional evidence is required before explaining this Lambda scalability finding?",
    }
    .to_string()
}

fn lambda_scalability_runbook_copy(report: &PillarReport) -> String {
    let reason_codes = sorted_unique_strings(
        report
            .findings
            .iter()
            .map(|finding| finding.reason_code.clone())
            .collect(),
    );
    format!(
        "Lambda scalability AI triage: score {} across {} function(s), {} stale. Evidence reason codes: {}.",
        report.score,
        report.resources_evaluated,
        report.stale_resources,
        if reason_codes.is_empty() {
            "none".to_string()
        } else {
            reason_codes.join(", ")
        }
    )
}

fn lambda_scalability_posture_recommendations(
    report: &PillarReport,
) -> Vec<LambdaCostPostureRecommendation> {
    report
        .findings
        .iter()
        .filter_map(|finding| {
            let recommendation = match finding.reason_code.as_str() {
                REASON_INV_STALE_DATA => "refresh_lambda_inventory_and_scalability_telemetry",
                REASON_SCAL_MISSING_CONCURRENCY_LIMIT_EVIDENCE => {
                    "collect_lambda_concurrency_limit_evidence"
                }
                REASON_SCAL_MISSING_TELEMETRY_COLLECTION_METADATA => {
                    "restore_lambda_scalability_collection_metadata"
                }
                REASON_SCAL_TELEMETRY_COLLECTION_ERRORS => {
                    "inspect_lambda_scalability_collection_errors"
                }
                REASON_SCAL_MISSING_CLOUDWATCH_TELEMETRY => {
                    "collect_lambda_invocation_throttle_and_concurrency_metrics"
                }
                REASON_SCAL_HIGH_CONCURRENCY_UTILIZATION => {
                    "review_lambda_concurrency_quota_and_event_source_scaling"
                }
                REASON_SCAL_THROTTLE_PRESSURE => {
                    "diagnose_lambda_throttle_and_retry_scaling_pressure"
                }
                _ => return None,
            };

            Some(LambdaCostPostureRecommendation {
                resource_id: finding.resource_id.clone(),
                reason_code: finding.reason_code.clone(),
                recommendation,
                owner: owner_from_evidence(&finding.evidence),
                confidence: "medium",
                effort: "medium",
                risk: if matches!(
                    finding.reason_code.as_str(),
                    REASON_SCAL_HIGH_CONCURRENCY_UTILIZATION | REASON_SCAL_THROTTLE_PRESSURE
                ) {
                    "medium"
                } else {
                    "low"
                },
                suppression_key: format!("{}:{}", finding.resource_id, finding.reason_code),
                audit_event_type: "lambda_scalability_posture_recommendation_emitted",
            })
        })
        .collect()
}

fn lambda_performance_follow_up_question(reason_code: &str) -> String {
    match reason_code {
        REASON_INV_STALE_DATA => {
            "Has Lambda inventory and performance telemetry been refreshed in the current sync window?"
        }
        REASON_PERF_MISSING_CONFIG_DATA => {
            "Which timeout and memory settings are missing from the Lambda function inventory?"
        }
        REASON_PERF_MISSING_TELEMETRY_COLLECTION_METADATA => {
            "Which collector run should be used as evidence for Lambda performance telemetry completeness?"
        }
        REASON_PERF_TELEMETRY_COLLECTION_ERRORS => {
            "Which CloudWatch permission, throttling, or retry failure prevented Lambda performance telemetry collection?"
        }
        REASON_PERF_MISSING_CLOUDWATCH_TELEMETRY => {
            "Which Duration, Errors, Throttles, and ConcurrentExecutions datapoints are missing for this Lambda function?"
        }
        REASON_PERF_HIGH_DURATION_PRESSURE => {
            "Is the function constrained by memory size, cold starts, downstream latency, or payload growth?"
        }
        REASON_PERF_ERROR_OR_THROTTLE_PRESSURE => {
            "Which concurrency limit, event source pressure, or retry loop is driving Lambda latency pressure?"
        }
        _ => "What additional evidence is required before explaining this Lambda performance finding?",
    }
    .to_string()
}

fn lambda_performance_runbook_copy(report: &PillarReport) -> String {
    let reason_codes = sorted_unique_strings(
        report
            .findings
            .iter()
            .map(|finding| finding.reason_code.clone())
            .collect(),
    );
    format!(
        "Lambda performance AI triage: score {} across {} function(s), {} stale. Evidence reason codes: {}.",
        report.score,
        report.resources_evaluated,
        report.stale_resources,
        if reason_codes.is_empty() {
            "none".to_string()
        } else {
            reason_codes.join(", ")
        }
    )
}

fn lambda_performance_posture_recommendations(
    report: &PillarReport,
) -> Vec<LambdaCostPostureRecommendation> {
    report
        .findings
        .iter()
        .filter_map(|finding| {
            let recommendation = match finding.reason_code.as_str() {
                REASON_INV_STALE_DATA => "refresh_lambda_inventory_and_performance_telemetry",
                REASON_PERF_MISSING_CONFIG_DATA => {
                    "collect_lambda_timeout_and_memory_configuration"
                }
                REASON_PERF_MISSING_TELEMETRY_COLLECTION_METADATA => {
                    "restore_lambda_performance_collection_metadata"
                }
                REASON_PERF_TELEMETRY_COLLECTION_ERRORS => {
                    "inspect_lambda_performance_collection_errors"
                }
                REASON_PERF_MISSING_CLOUDWATCH_TELEMETRY => {
                    "collect_lambda_duration_error_throttle_and_concurrency_metrics"
                }
                REASON_PERF_HIGH_DURATION_PRESSURE => {
                    "review_lambda_memory_timeout_and_downstream_latency"
                }
                REASON_PERF_ERROR_OR_THROTTLE_PRESSURE => {
                    "diagnose_lambda_error_throttle_and_concurrency_pressure"
                }
                _ => return None,
            };

            Some(LambdaCostPostureRecommendation {
                resource_id: finding.resource_id.clone(),
                reason_code: finding.reason_code.clone(),
                recommendation,
                owner: owner_from_evidence(&finding.evidence),
                confidence: "medium",
                effort: "medium",
                risk: if matches!(
                    finding.reason_code.as_str(),
                    REASON_PERF_HIGH_DURATION_PRESSURE | REASON_PERF_ERROR_OR_THROTTLE_PRESSURE
                ) {
                    "medium"
                } else {
                    "low"
                },
                suppression_key: format!("{}:{}", finding.resource_id, finding.reason_code),
                audit_event_type: "lambda_performance_posture_recommendation_emitted",
            })
        })
        .collect()
}

fn lambda_resilience_follow_up_question(reason_code: &str) -> String {
    match reason_code {
        REASON_INV_STALE_DATA => {
            "Has Lambda inventory and resilience telemetry been refreshed in the current sync window?"
        }
        REASON_RES_MISSING_CONFIG_DATA => {
            "Which timeout and memory settings are missing from the Lambda function inventory?"
        }
        REASON_RES_MISSING_TELEMETRY_COLLECTION_METADATA => {
            "Which collector run should be used as evidence for Lambda resilience telemetry completeness?"
        }
        REASON_RES_TELEMETRY_COLLECTION_ERRORS => {
            "Which collector permission, throttling, or retry failure prevented Lambda resilience telemetry collection?"
        }
        REASON_RES_MISSING_CLOUDWATCH_TELEMETRY => {
            "Which Duration, Errors, and Throttles datapoints are missing for the affected Lambda function?"
        }
        REASON_RES_MISSING_LOG_EVENT_EVIDENCE => {
            "Which log subscription, DLQ, or event source mapping evidence is missing for the affected Lambda function?"
        }
        REASON_RES_MISSING_QUOTA_LIMIT_EVIDENCE => {
            "Which reserved concurrency or account concurrency limit is missing for the affected Lambda function?"
        }
        REASON_RES_ERROR_OR_THROTTLE_HEALTH_SIGNAL => {
            "Which retry, concurrency, or upstream dependency path is driving Lambda error or throttle recovery pressure?"
        }
        REASON_RES_LOW_TIMEOUT_HEADROOM => {
            "Does the current timeout leave enough headroom for tail latency and downstream retries?"
        }
        _ => "What additional evidence is required before explaining this Lambda resilience finding?",
    }
    .to_string()
}

fn lambda_resilience_runbook_copy(report: &PillarReport) -> String {
    let reason_codes = sorted_unique_strings(
        report
            .findings
            .iter()
            .map(|finding| finding.reason_code.clone())
            .collect(),
    );
    format!(
        "Lambda resilience AI triage: score {} across {} function(s), {} stale. Evidence reason codes: {}.",
        report.score,
        report.resources_evaluated,
        report.stale_resources,
        if reason_codes.is_empty() {
            "none".to_string()
        } else {
            reason_codes.join(", ")
        }
    )
}

fn lambda_resilience_posture_recommendations(
    report: &PillarReport,
) -> Vec<LambdaCostPostureRecommendation> {
    report
        .findings
        .iter()
        .filter_map(|finding| {
            let recommendation = match finding.reason_code.as_str() {
                REASON_INV_STALE_DATA => "refresh_lambda_inventory_and_resilience_telemetry",
                REASON_RES_MISSING_CONFIG_DATA => "collect_lambda_timeout_and_memory_configuration",
                REASON_RES_MISSING_TELEMETRY_COLLECTION_METADATA => {
                    "restore_lambda_resilience_collection_metadata"
                }
                REASON_RES_TELEMETRY_COLLECTION_ERRORS => {
                    "inspect_lambda_resilience_collection_errors"
                }
                REASON_RES_MISSING_CLOUDWATCH_TELEMETRY => {
                    "collect_lambda_duration_error_and_throttle_metrics"
                }
                REASON_RES_MISSING_LOG_EVENT_EVIDENCE => {
                    "collect_lambda_log_event_and_dlq_evidence"
                }
                REASON_RES_MISSING_QUOTA_LIMIT_EVIDENCE => {
                    "collect_lambda_concurrency_limit_evidence"
                }
                REASON_RES_ERROR_OR_THROTTLE_HEALTH_SIGNAL => {
                    "diagnose_lambda_failure_and_throttle_recovery_pressure"
                }
                REASON_RES_LOW_TIMEOUT_HEADROOM => {
                    "review_lambda_timeout_memory_and_latency_headroom"
                }
                _ => return None,
            };

            Some(LambdaCostPostureRecommendation {
                resource_id: finding.resource_id.clone(),
                reason_code: finding.reason_code.clone(),
                recommendation,
                owner: owner_from_evidence(&finding.evidence),
                confidence: "medium",
                effort: "medium",
                risk: if matches!(
                    finding.reason_code.as_str(),
                    REASON_RES_ERROR_OR_THROTTLE_HEALTH_SIGNAL | REASON_RES_LOW_TIMEOUT_HEADROOM
                ) {
                    "medium"
                } else {
                    "low"
                },
                suppression_key: format!("{}:{}", finding.resource_id, finding.reason_code),
                audit_event_type: "lambda_resilience_posture_recommendation_emitted",
            })
        })
        .collect()
}

fn missing_fields<'a>(resource: &AwsResourceModel, required_fields: &'a [&str]) -> Vec<&'a str> {
    required_fields
        .iter()
        .copied()
        .filter(|field| resource.resource_data.get(*field).is_none())
        .collect()
}

fn missing_metrics(resource: &AwsResourceModel, required_metrics: &[&str]) -> Vec<String> {
    required_metrics
        .iter()
        .filter(|metric| metric_max(resource, metric).is_none())
        .map(|metric| (*metric).to_string())
        .collect()
}

fn metric_max(resource: &AwsResourceModel, metric_name: &str) -> Option<f64> {
    metric_values(&resource.resource_data, metric_name)
        .into_iter()
        .reduce(f64::max)
}

fn metric_values(resource_data: &Value, metric_name: &str) -> Vec<f64> {
    let mut values = Vec::new();
    collect_metric_values(resource_data, metric_name, &mut values);

    if let Some(cloudwatch_metrics) = resource_data.get("cloudwatch_metrics") {
        collect_metric_values(cloudwatch_metrics, metric_name, &mut values);
    }
    if let Some(cloudwatch_metrics) = resource_data.pointer("/telemetry/cloudwatch") {
        collect_metric_values(cloudwatch_metrics, metric_name, &mut values);
    }

    values
}

fn collect_metric_values(value: &Value, metric_name: &str, values: &mut Vec<f64>) {
    if metric_name_matches(value, metric_name) {
        collect_metric_payload_values(value, values);
    }

    if let Some(metrics) = value.get("metrics").and_then(|metrics| metrics.as_array()) {
        for metric in metrics {
            if metric_name_matches(metric, metric_name) {
                collect_metric_payload_values(metric, values);
            }
        }
    }

    if let Some(metrics) = value.get("metrics").and_then(|metrics| metrics.as_object()) {
        if let Some(metric) = metrics.get(metric_name) {
            collect_metric_payload_values(metric, values);
        }
    }

    if let Some(metric) = value.get(metric_name) {
        collect_metric_payload_values(metric, values);
    }
}

fn metric_name_matches(value: &Value, metric_name: &str) -> bool {
    value
        .get("metric_name")
        .or_else(|| value.get("MetricName"))
        .and_then(|name| name.as_str())
        .map(|name| name == metric_name)
        .unwrap_or(false)
}

fn collect_metric_payload_values(value: &Value, values: &mut Vec<f64>) {
    for key in ["value", "latest", "max", "average", "Value"] {
        if let Some(number) = value.get(key).and_then(|number| number.as_f64()) {
            values.push(number);
        }
    }

    if let Some(datapoints) = value
        .get("datapoints")
        .or_else(|| value.get("Datapoints"))
        .and_then(|datapoints| datapoints.as_array())
    {
        for datapoint in datapoints {
            for key in ["value", "Value", "average", "Average", "maximum", "Maximum"] {
                if let Some(number) = datapoint.get(key).and_then(|number| number.as_f64()) {
                    values.push(number);
                    break;
                }
            }
        }
    }
}

fn data_u64(data: &Value, key: &str) -> Option<u64> {
    data.get(key).and_then(|value| {
        value
            .as_u64()
            .or_else(|| value.as_i64().map(|n| n.max(0) as u64))
    })
}

fn resource_data_keys(resource: &AwsResourceModel) -> Vec<String> {
    resource
        .resource_data
        .as_object()
        .map(|object| object.keys().cloned().collect())
        .unwrap_or_default()
}

fn sorted_unique_resources(resources: impl Iterator<Item = String>) -> Vec<String> {
    resources
        .filter(|resource_id| !resource_id.is_empty())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn sorted_unique_reasons(reasons: impl Iterator<Item = String>) -> Vec<String> {
    reasons.collect::<BTreeSet<_>>().into_iter().collect()
}

fn sorted_unique_strings(values: Vec<String>) -> Vec<String> {
    values
        .into_iter()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;
    use serde_json::Value;
    use uuid::Uuid;

    fn fixture(
        resource_id: &str,
        tags: Value,
        resource_data: Value,
        refreshed_hours_ago: i64,
        now: DateTime<Utc>,
    ) -> AwsResourceModel {
        let refreshed = now - Duration::hours(refreshed_hours_ago);
        AwsResourceModel {
            id: Uuid::new_v4(),
            sync_id: None,
            account_id: "123456789012".to_string(),
            profile: None,
            region: "us-east-1".to_string(),
            resource_type: "LambdaFunction".to_string(),
            resource_id: resource_id.to_string(),
            arn: format!(
                "arn:aws:lambda:us-east-1:123456789012:function:{}",
                resource_id
            ),
            name: Some(resource_id.to_string()),
            tags,
            resource_data,
            created_at: refreshed,
            updated_at: refreshed,
            last_refreshed: refreshed,
        }
    }

    fn now() -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2026-06-10T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc)
    }

    fn healthy_data() -> Value {
        json!({
            "function_name": "fn",
            "runtime": "python3.12",
            "timeout": 30,
            "memory_size": 256,
            "architectures": ["arm64"],
            "recent_error_log_count": 0,
            "event_source_mapping_count": 1,
            "dead_letter_queue_configured": true,
            "reserved_concurrent_executions": 50,
            "account_concurrency_limit": 1000,
            "telemetry_collection_started_at": "2026-06-10T00:00:00Z",
            "telemetry_collection_completed_at": "2026-06-10T00:00:03Z",
            "telemetry_collection_duration_ms": 3000,
            "telemetry_collection_success_count": 1,
            "telemetry_collection_failure_count": 0,
            "telemetry_collection_error_count": 0,
            "telemetry_collection_errors": [],
            "cloudwatch_metric_names": ["Invocations", "Duration", "Errors", "Throttles"],
            "cloudwatch_metrics": {
                "source": "CloudWatch",
                "namespace": "AWS/Lambda",
                "resource_id": "fn",
                "dimension_name": "FunctionName",
                "metrics": [
                    {
                        "metric_name": "Invocations",
                        "datapoints": [{"value": 12.0}]
                    },
                    {
                        "metric_name": "Duration",
                        "datapoints": [{"value": 120.0}]
                    },
                    {
                        "metric_name": "Errors",
                        "datapoints": [{"value": 0.0}]
                    },
                    {
                        "metric_name": "Throttles",
                        "datapoints": [{"value": 0.0}]
                    }
                ]
            },
        })
    }

    #[test]
    fn cost_flags_missing_allocation_tags_and_x86_only_architecture() {
        let mut data = healthy_data();
        data["architectures"] = json!(["x86_64"]);
        let r = fixture("fn-untagged", json!({}), data, 1, now());
        let report = evaluate_lambda_fleet(&[r], Pillar::Cost, now());
        let codes: Vec<&str> = report
            .findings
            .iter()
            .map(|f| f.reason_code.as_str())
            .collect();
        assert!(codes.contains(&REASON_COST_MISSING_ALLOCATION_TAGS));
        assert!(codes.contains(&REASON_COST_X86_ONLY_ARCHITECTURE));
        let arch = report
            .findings
            .iter()
            .find(|f| f.reason_code == REASON_COST_X86_ONLY_ARCHITECTURE)
            .unwrap();
        assert_eq!(arch.evidence["architectures"], json!(["x86_64"]));
    }

    #[test]
    fn cost_passes_for_tagged_arm64_function() {
        let r = fixture(
            "fn-good",
            json!({"team": "payments"}),
            healthy_data(),
            1,
            now(),
        );
        let report = evaluate_lambda_fleet(&[r], Pillar::Cost, now());
        assert!(
            report.findings.is_empty(),
            "unexpected: {:?}",
            report.findings
        );
        assert_eq!(report.score, 100);
    }

    #[test]
    fn lambda_cost_telemetry_flags_missing_collection_and_metric_gaps() {
        let mut data = healthy_data();
        data.as_object_mut()
            .unwrap()
            .remove("telemetry_collection_started_at");
        data.as_object_mut().unwrap().remove("cloudwatch_metrics");
        let r = fixture(
            "fn-no-telemetry",
            json!({"team": "payments"}),
            data,
            1,
            now(),
        );

        let report = evaluate_lambda_fleet(&[r], Pillar::Cost, now());
        let codes: Vec<&str> = report
            .findings
            .iter()
            .map(|f| f.reason_code.as_str())
            .collect();

        assert!(codes.contains(&REASON_COST_MISSING_TELEMETRY_COLLECTION_METADATA));
        assert!(codes.contains(&REASON_COST_MISSING_CLOUDWATCH_TELEMETRY));
        let telemetry = lambda_cost_telemetry_summary(&report);
        assert_eq!(telemetry.workflow_id, "lambda_cost_telemetry");
        assert!(telemetry
            .missing_data_reason_codes
            .contains(&REASON_COST_MISSING_CLOUDWATCH_TELEMETRY.to_string()));
    }

    #[test]
    fn lambda_performance_telemetry_and_triage_explain_latency_pressure() {
        let mut data = healthy_data();
        data["cloudwatch_metric_names"] =
            json!(["Duration", "Errors", "Throttles", "ConcurrentExecutions"]);
        data["cloudwatch_metrics"]["metrics"] = json!([
            {"metric_name": "Duration", "datapoints": [{"value": 23_000.0}]},
            {"metric_name": "Errors", "datapoints": [{"value": 1.0}]},
            {"metric_name": "Throttles", "datapoints": [{"value": 2.0}]},
            {"metric_name": "ConcurrentExecutions", "datapoints": [{"value": 40.0}]}
        ]);
        let r = fixture(
            "fn-latency-pressure",
            json!({"team": "payments", "environment": "prod"}),
            data,
            1,
            now(),
        );

        let report = evaluate_lambda_fleet(&[r], Pillar::Performance, now());
        let codes: Vec<&str> = report
            .findings
            .iter()
            .map(|finding| finding.reason_code.as_str())
            .collect();
        assert!(codes.contains(&REASON_PERF_HIGH_DURATION_PRESSURE));
        assert!(codes.contains(&REASON_PERF_ERROR_OR_THROTTLE_PRESSURE));

        let posture = lambda_performance_posture_summary(&report);
        assert_eq!(posture.workflow_id, "lambda_performance_posture");
        assert_eq!(posture.rules_evaluated, 5);
        assert_eq!(posture.status, LambdaPostureStatus::Fail);
        assert!(posture
            .recommendations
            .iter()
            .any(|recommendation| recommendation.recommendation
                == "review_lambda_memory_timeout_and_downstream_latency"));

        let triage = lambda_performance_triage_context(&report);
        assert_eq!(triage.workflow_id, "lambda_performance_triage_context");
        assert_eq!(
            triage.context_builder_id,
            "lambda-performance-deterministic-context-v1"
        );
        assert!(triage
            .hypotheses
            .iter()
            .any(|hypothesis| hypothesis.contains("high share of its timeout budget")));

        let telemetry = lambda_performance_telemetry_summary(&report);
        assert_eq!(telemetry.workflow_id, "lambda_performance_telemetry");
        assert!(telemetry.required_metrics.contains(&"ConcurrentExecutions"));
        assert!(telemetry.evidence_reason_codes.iter().any(|code| {
            code == REASON_PERF_HIGH_DURATION_PRESSURE
                || code == REASON_PERF_ERROR_OR_THROTTLE_PRESSURE
        }));
    }

    #[test]
    fn lambda_scalability_telemetry_and_triage_explain_concurrency_pressure() {
        let mut data = healthy_data();
        data["reserved_concurrent_executions"] = json!(50);
        data["account_concurrency_limit"] = json!(1000);
        data["cloudwatch_metric_names"] =
            json!(["Invocations", "Throttles", "ConcurrentExecutions"]);
        data["cloudwatch_metrics"]["metrics"] = json!([
            {"metric_name": "Invocations", "datapoints": [{"value": 900.0}]},
            {"metric_name": "Throttles", "datapoints": [{"value": 4.0}]},
            {"metric_name": "ConcurrentExecutions", "datapoints": [{"value": 45.0}]}
        ]);
        let r = fixture(
            "fn-concurrency-pressure",
            json!({"team": "payments", "environment": "prod"}),
            data,
            1,
            now(),
        );

        let report = evaluate_lambda_fleet(&[r], Pillar::Scalability, now());
        let codes: Vec<&str> = report
            .findings
            .iter()
            .map(|finding| finding.reason_code.as_str())
            .collect();
        assert!(codes.contains(&REASON_SCAL_HIGH_CONCURRENCY_UTILIZATION));
        assert!(codes.contains(&REASON_SCAL_THROTTLE_PRESSURE));

        let posture = lambda_scalability_posture_summary(&report);
        assert_eq!(posture.workflow_id, "lambda_scalability_posture");
        assert_eq!(posture.rules_evaluated, 5);
        assert_eq!(posture.status, LambdaPostureStatus::Fail);
        assert!(posture
            .recommendations
            .iter()
            .any(|recommendation| recommendation.recommendation
                == "review_lambda_concurrency_quota_and_event_source_scaling"));

        let triage = lambda_scalability_triage_context(&report);
        assert_eq!(triage.workflow_id, "lambda_scalability_triage_context");
        assert_eq!(
            triage.context_builder_id,
            "lambda-scalability-deterministic-context-v1"
        );
        assert!(triage
            .hypotheses
            .iter()
            .any(|hypothesis| hypothesis.contains("concurrency budget")));

        let telemetry = lambda_scalability_telemetry_summary(&report);
        assert_eq!(telemetry.workflow_id, "lambda_scalability_telemetry");
        assert!(telemetry.required_metrics.contains(&"ConcurrentExecutions"));
        assert!(telemetry
            .evidence_reason_codes
            .iter()
            .any(|code| code == REASON_SCAL_THROTTLE_PRESSURE));
    }

    #[test]
    fn lambda_cost_telemetry_reports_collection_errors_and_cost_waste() {
        let mut data = healthy_data();
        data["telemetry_collection_success_count"] = json!(0);
        data["telemetry_collection_failure_count"] = json!(1);
        data["telemetry_collection_error_count"] = json!(1);
        data["telemetry_collection_errors"] = json!([
            {
                "source": "cloudwatch",
                "operation": "GetMetricData",
                "error": "Throttling"
            }
        ]);
        data["cloudwatch_metrics"]["metrics"][0]["datapoints"] = json!([{ "value": 0.0 }]);
        data["cloudwatch_metrics"]["metrics"][2]["datapoints"] = json!([{ "value": 2.0 }]);
        data["cloudwatch_metrics"]["metrics"][3]["datapoints"] = json!([{ "value": 1.0 }]);
        let r = fixture("fn-cost-waste", json!({"team": "payments"}), data, 1, now());

        let report = evaluate_lambda_fleet(&[r], Pillar::Cost, now());
        let codes: Vec<&str> = report
            .findings
            .iter()
            .map(|f| f.reason_code.as_str())
            .collect();

        assert!(codes.contains(&REASON_COST_TELEMETRY_COLLECTION_ERRORS));
        assert!(codes.contains(&REASON_COST_NO_INVOCATIONS_TELEMETRY));
        assert!(codes.contains(&REASON_COST_ERROR_OR_THROTTLE_TELEMETRY));
        let posture = lambda_cost_posture_summary(&report);
        assert_eq!(posture.workflow_id, "lambda_cost_posture");
        assert_eq!(posture.rule_pack_id, "lambda-cost-posture-rules-v1");
        assert_eq!(posture.audit_event_type, "lambda_cost_posture_evaluated");
        assert!(posture.read_only_mode);
        assert_eq!(posture.status, LambdaPostureStatus::Fail);
        assert_eq!(posture.rules_evaluated, 7);
        assert!(posture.suppression_policy.supported);
        assert!(posture.assignment_policy.supported);
        assert!(posture
            .affected_resources
            .contains(&"fn-cost-waste".to_string()));
        assert!(posture.recommendations.iter().any(|recommendation| {
            recommendation.resource_id == "fn-cost-waste"
                && recommendation.reason_code == REASON_COST_ERROR_OR_THROTTLE_TELEMETRY
                && recommendation.recommendation == "diagnose_retry_error_and_throttle_cost_waste"
                && recommendation.suppression_key
                    == "fn-cost-waste:LAMBDA_COST_ERROR_OR_THROTTLE_TELEMETRY"
        }));
    }

    #[test]
    fn lambda_cost_triage_context_separates_facts_hypotheses_and_questions() {
        let mut collection_error_data = healthy_data();
        collection_error_data["telemetry_collection_success_count"] = json!(0);
        collection_error_data["telemetry_collection_failure_count"] = json!(1);
        collection_error_data["telemetry_collection_error_count"] = json!(1);
        collection_error_data["telemetry_collection_errors"] = json!([
            {
                "source": "cloudwatch",
                "operation": "GetMetricData",
                "error": "AccessDenied"
            }
        ]);
        let collection_error = fixture(
            "fn-collection-error",
            json!({"team": "payments"}),
            collection_error_data,
            1,
            now(),
        );

        let mut missing_telemetry_data = healthy_data();
        missing_telemetry_data
            .as_object_mut()
            .unwrap()
            .remove("cloudwatch_metrics");
        let missing_telemetry = fixture(
            "fn-missing-telemetry",
            json!({}),
            missing_telemetry_data,
            1,
            now(),
        );

        let report =
            evaluate_lambda_fleet(&[collection_error, missing_telemetry], Pillar::Cost, now());
        let triage = lambda_cost_triage_context(&report);

        assert_eq!(triage.workflow_id, "lambda_cost_triage_context");
        assert_eq!(triage.api_path, "/api/aws/inventory/lambda/pillars");
        assert_eq!(
            triage.context_builder_id,
            "lambda-cost-deterministic-context-v1"
        );
        assert_eq!(triage.prompt_template_id, "lambda-cost-ai-triage-v1");
        assert_eq!(triage.generation_mode, "deterministic_no_llm");
        assert_eq!(triage.max_prompt_tokens, 1200);
        assert_eq!(triage.audit_id_prefix, "lambda-cost-ai-triage");
        assert_eq!(triage.pagination.default_limit, 50);
        assert_eq!(triage.pagination.max_limit, 200);
        assert_eq!(triage.pagination.evidence_cursor, "evidence_citations");
        assert!(!triage.freshness.stale_data_blocks_ai_summary);
        assert_eq!(
            triage.freshness.freshness_source,
            "lambda_inventory_last_synced_at"
        );
        assert!(triage.export_formats.contains(&"markdown_runbook"));
        assert!(triage
            .error_codes
            .contains(&"LAMBDA_COST_AI_TRIAGE_MISSING_EVIDENCE"));
        assert!(triage.guardrails.read_only_mode);
        assert!(triage.guardrails.evidence_required);
        assert!(triage.guardrails.separate_facts_from_hypotheses);
        assert!(triage.guardrails.ask_for_missing_data);
        assert!(triage.guardrails.no_llm_invocation);
        assert!(triage.guardrails.no_mutation_planning);
        assert!(triage
            .facts
            .iter()
            .any(|fact| fact.contains(REASON_COST_TELEMETRY_COLLECTION_ERRORS)));
        assert!(triage
            .hypotheses
            .iter()
            .any(|hypothesis| hypothesis.contains("collection errors")));
        assert!(triage
            .missing_data_questions
            .iter()
            .any(|question| question.contains("Invocations, Duration, Errors, and Throttles")));
        assert!(triage
            .missing_data_questions
            .iter()
            .any(|question| question.contains("owner, team, project, or cost-center")));
        assert!(triage
            .follow_up_questions
            .iter()
            .any(|question| { question.contains("Invocations, Duration, Errors, and Throttles") }));
        assert!(triage
            .runbook_copy_markdown
            .contains("Lambda cost AI triage"));
        assert!(triage.feedback_capture.supported);
        assert_eq!(
            triage.feedback_capture.feedback_event_type,
            "lambda_cost_ai_triage_feedback_captured"
        );
        assert!(triage.evidence_citations.iter().any(|citation| {
            citation.reason_code == REASON_COST_TELEMETRY_COLLECTION_ERRORS
                && citation.resource_id == "fn-collection-error"
                && citation.evidence["telemetry_collection_error_count"] == 1
        }));
    }

    #[test]
    fn lambda_cost_agentic_investigation_plan_is_read_only_until_approval() {
        let mut data = healthy_data();
        data["cloudwatch_metrics"]["metrics"][2]["datapoints"] = json!([{ "value": 8.0 }]);
        data["cloudwatch_metrics"]["metrics"][3]["datapoints"] = json!([{ "value": 2.0 }]);
        let resource = fixture("fn-cost-investigate", json!({}), data, 1, now());
        let report = evaluate_lambda_fleet(&[resource], Pillar::Cost, now());
        let plan = lambda_cost_agentic_investigation_plan(&report);

        assert_eq!(plan.workflow_id, "lambda_cost_agentic_investigation");
        assert_eq!(
            plan.default_tool_mode,
            LambdaInvestigationToolMode::ReadOnly
        );
        assert!(plan.replay_required);
        assert!(plan.steps.iter().any(|step| {
            step.tool_name == "lambda.resource_groups.get_tagging_context"
                && step.tool_mode == LambdaInvestigationToolMode::ReadOnly
        }));
        assert!(plan.steps.iter().any(|step| {
            step.tool_name == "lambda.cloudwatch.diagnose_retry_throttle_spend"
                && step.tool_mode == LambdaInvestigationToolMode::ReadOnly
        }));
        assert_eq!(
            plan.steps.last().map(|step| step.tool_mode),
            Some(LambdaInvestigationToolMode::ApprovalRequired)
        );
        assert_eq!(
            plan.steps.last().map(|step| step.tool_name),
            Some("lambda.cost.prepare_approval_plan")
        );
        assert!(plan.steps.iter().all(|step| {
            step.tool_mode == LambdaInvestigationToolMode::ReadOnly
                || step.tool_name == "lambda.cost.prepare_approval_plan"
        }));
        assert!(plan.steps.iter().all(|step| {
            !step.tool_name.contains("execute")
                && !step.tool_name.contains("delete")
                && !step.tool_name.contains("update_function")
        }));
        assert!(plan.approval_gates.iter().any(|gate| {
            gate.target_resource_id == "fn-cost-investigate" && gate.rollback_note_required
        }));
    }

    #[test]
    fn lambda_cost_remediation_workflow_plans_dry_run_actions_until_approved() {
        let mut spend_data = healthy_data();
        spend_data["cloudwatch_metrics"]["metrics"][2]["datapoints"] = json!([{ "value": 8.0 }]);
        spend_data["cloudwatch_metrics"]["metrics"][3]["datapoints"] = json!([{ "value": 2.0 }]);
        let spend = fixture("fn-cost-remediate", json!({}), spend_data, 1, now());

        let report = evaluate_lambda_fleet(&[spend], Pillar::Cost, now());
        let workflow = lambda_cost_remediation_workflow(&report);

        assert_eq!(workflow.workflow_id, "lambda_cost_safe_remediation");
        assert!(workflow.read_only_mode);
        assert_eq!(
            workflow.rbac_permission,
            "aws.lambda.cost.remediation.approve"
        );
        assert_eq!(workflow.audit_stream, "lambda_cost_remediation_audit");
        assert!(!workflow.stale_data_blocks_execution);
        assert!(workflow.actions.len() >= 2);
        assert!(workflow.actions.iter().all(|action| {
            action.dry_run
                && action.requires_approval
                && action.approval_gate_id.is_some()
                && action.status == LambdaRemediationStatus::DryRunPendingApproval
                && action.audit_event_type == "lambda.cost.remediation.dry_run_planned"
                && action.rollback_note.contains("rollback")
                && action.idempotency_key.starts_with("lambda-cost-")
        }));
        assert!(workflow.actions.iter().any(|action| {
            action.kind == LambdaRemediationActionKind::AssignCostTags
                && action.target_resource_id == "fn-cost-remediate"
                && action
                    .evidence_reason_codes
                    .contains(&REASON_COST_MISSING_ALLOCATION_TAGS.to_string())
        }));
        assert!(workflow.actions.iter().any(|action| {
            action.kind == LambdaRemediationActionKind::ReviewRetryThrottleCostControls
                && action.target_resource_id == "fn-cost-remediate"
                && action
                    .evidence_reason_codes
                    .contains(&REASON_COST_ERROR_OR_THROTTLE_TELEMETRY.to_string())
        }));
    }

    #[test]
    fn lambda_cost_remediation_workflow_blocks_execution_when_cost_data_is_stale() {
        let stale = fixture(
            "fn-cost-stale-remediate",
            json!({}),
            healthy_data(),
            1,
            now() - chrono::Duration::hours(30),
        );

        let report = evaluate_lambda_fleet(&[stale], Pillar::Cost, now());
        let workflow = lambda_cost_remediation_workflow(&report);

        assert!(workflow.stale_data_blocks_execution);
        assert!(workflow.actions.iter().all(|action| {
            action.dry_run && action.status == LambdaRemediationStatus::BlockedMissingEvidence
        }));
    }

    #[test]
    fn lambda_cost_slo_policy_snapshot_tracks_owner_policy_and_notifications() {
        let mut spend_data = healthy_data();
        spend_data["cloudwatch_metrics"]["metrics"][2]["datapoints"] = json!([{ "value": 8.0 }]);
        spend_data["cloudwatch_metrics"]["metrics"][3]["datapoints"] = json!([{ "value": 2.0 }]);
        let spend = fixture(
            "fn-cost-slo",
            json!({
                "owner": "sre",
                "environment": "prod",
                "application": "checkout"
            }),
            spend_data,
            1,
            now(),
        );

        let report = evaluate_lambda_fleet(&[spend], Pillar::Cost, now());
        let snapshot = lambda_cost_slo_policy_snapshot(&report);

        assert_eq!(snapshot.workflow_id, "lambda_cost_slo_policy");
        assert!(snapshot.read_only_mode);
        assert!(snapshot.freshness_required);
        assert_eq!(snapshot.objective.objective_id, "lambda-cost-score-min-90");
        assert_eq!(snapshot.objective.status, LambdaCostObjectiveStatus::AtRisk);
        assert_eq!(snapshot.objective.target_score_min, 90);
        assert_eq!(snapshot.objective.owner_filters, vec!["sre"]);
        assert_eq!(snapshot.objective.environment_filters, vec!["prod"]);
        assert_eq!(snapshot.objective.application_filters, vec!["checkout"]);
        assert_eq!(
            snapshot.objective.notification_targets,
            vec!["environment:prod", "owner:sre"]
        );
        assert_eq!(snapshot.objective.policy_state, "active_with_findings");
        assert!(snapshot
            .evidence_reason_codes
            .contains(&REASON_COST_ERROR_OR_THROTTLE_TELEMETRY.to_string()));
    }

    #[test]
    fn lambda_cost_slo_policy_snapshot_marks_stale_data_as_breached() {
        let stale = fixture(
            "fn-cost-stale-slo",
            json!({"owner": "sre"}),
            healthy_data(),
            1,
            now() - chrono::Duration::hours(30),
        );

        let report = evaluate_lambda_fleet(&[stale], Pillar::Cost, now());
        let snapshot = lambda_cost_slo_policy_snapshot(&report);

        assert_eq!(
            snapshot.objective.status,
            LambdaCostObjectiveStatus::Breached
        );
        assert_eq!(
            snapshot.objective.trend_direction,
            LambdaCostTrendDirection::Degrading
        );
        assert_eq!(snapshot.objective.policy_state, "blocked_stale_data");
        assert!(snapshot
            .evidence_reason_codes
            .contains(&REASON_INV_STALE_DATA.to_string()));
    }

    #[test]
    fn lambda_cost_forecast_snapshot_builds_read_only_cost_band_from_telemetry_evidence() {
        let mut spend_data = healthy_data();
        spend_data["cloudwatch_metrics"]["metrics"][2]["datapoints"] = json!([{ "value": 8.0 }]);
        spend_data["cloudwatch_metrics"]["metrics"][3]["datapoints"] = json!([{ "value": 2.0 }]);
        let spend = fixture(
            "fn-cost-forecast",
            json!({"owner": "sre"}),
            spend_data,
            1,
            now(),
        );

        let report = evaluate_lambda_fleet(&[spend], Pillar::Cost, now());
        let forecast = lambda_cost_forecast_snapshot(&report);

        assert_eq!(forecast.workflow_id, "lambda_cost_forecasting");
        assert!(forecast.read_only_mode);
        assert_eq!(forecast.baseline_window_days, 30);
        assert_eq!(forecast.forecast_horizon_days, 30);
        assert_eq!(forecast.confidence_level, 80);
        assert_eq!(forecast.risk_level, LambdaCostForecastRisk::Moderate);
        assert_eq!(forecast.capacity_risk, "retry_or_throttle_cost_pressure");
        assert_eq!(forecast.forecast_band.horizon_days, 30);
        assert!(forecast.forecast_band.expected_monthly_cost_index > 100);
        assert!(
            forecast.forecast_band.upper_monthly_cost_index
                > forecast.forecast_band.lower_monthly_cost_index
        );
        assert!(forecast
            .risk_drivers
            .iter()
            .any(|driver| driver.reason_code == REASON_COST_ERROR_OR_THROTTLE_TELEMETRY));
        assert!(forecast
            .blast_radius_summary
            .contains("1 Lambda function(s)"));
    }

    #[test]
    fn lambda_cost_forecast_snapshot_blocks_on_stale_or_missing_telemetry() {
        let stale = fixture(
            "fn-cost-stale-forecast",
            json!({"owner": "sre"}),
            healthy_data(),
            1,
            now() - chrono::Duration::hours(30),
        );

        let report = evaluate_lambda_fleet(&[stale], Pillar::Cost, now());
        let forecast = lambda_cost_forecast_snapshot(&report);

        assert_eq!(forecast.risk_level, LambdaCostForecastRisk::Blocked);
        assert_eq!(
            forecast.capacity_risk,
            "blocked_by_stale_or_failed_collection"
        );
        assert!(forecast.blocked_by_stale_data);
        assert!(forecast
            .missing_data_reason_codes
            .contains(&REASON_INV_STALE_DATA.to_string()));
        assert!(forecast.forecast_band.upper_monthly_cost_index > 100);
    }

    #[test]
    fn lambda_cost_reporting_bundle_materializes_executive_engineering_and_incident_views() {
        let mut throttle_data = healthy_data();
        throttle_data["cloudwatch_metrics"]["metrics"][2]["datapoints"] = json!([{ "value": 6.0 }]);
        throttle_data["cloudwatch_metrics"]["metrics"][3]["datapoints"] = json!([{ "value": 1.0 }]);
        let throttle = fixture(
            "fn-cost-report",
            json!({"owner": "sre", "environment": "prod", "application": "checkout"}),
            throttle_data,
            1,
            now(),
        );
        let missing_tags = fixture("fn-cost-missing-tags", json!({}), healthy_data(), 1, now());

        let report = evaluate_lambda_fleet(&[throttle, missing_tags], Pillar::Cost, now());
        let bundle = lambda_cost_reporting_bundle(&report);

        assert_eq!(bundle.workflow_id, "lambda_cost_reporting");
        assert!(bundle.read_only_mode);
        assert_eq!(bundle.scheduled_delivery_state, "ready_for_schedule");
        assert!(bundle.portfolio_summary_ready);
        assert!(bundle.workload_summary_ready);
        assert_eq!(bundle.export_formats, vec!["json", "csv"]);
        assert_eq!(
            bundle.executive_summary.report_id,
            "lambda-cost-executive-summary"
        );
        assert_eq!(bundle.executive_summary.score, report.score);
        assert_eq!(bundle.executive_summary.resources_evaluated, 2);
        assert_eq!(bundle.executive_summary.stale_resources, 0);
        assert!(bundle
            .executive_summary
            .affected_resources
            .contains(&"fn-cost-report".to_string()));
        assert!(bundle
            .executive_summary
            .top_reason_codes
            .contains(&REASON_COST_ERROR_OR_THROTTLE_TELEMETRY.to_string()));
        assert_eq!(
            bundle.engineering_backlog.report_id,
            "lambda-cost-engineering-backlog"
        );
        assert_eq!(bundle.engineering_backlog.page, 0);
        assert_eq!(bundle.engineering_backlog.page_size, 50);
        assert_eq!(bundle.engineering_backlog.total, report.findings.len());
        assert_eq!(
            bundle.incident_review.report_id,
            "lambda-cost-incident-review"
        );
        assert!(bundle.incident_review.rows.iter().any(|row| {
            row.resource_id == "fn-cost-report"
                && row.reason_code == REASON_COST_ERROR_OR_THROTTLE_TELEMETRY
                && row.suppression_supported
                && row
                    .recovery_note
                    .contains("Review retry, timeout, and concurrency behavior")
        }));
    }

    #[test]
    fn lambda_cost_reporting_bundle_blocks_delivery_for_stale_or_missing_evidence() {
        let stale = fixture(
            "fn-cost-stale-report",
            json!({"owner": "sre"}),
            healthy_data(),
            1,
            now() - chrono::Duration::hours(30),
        );

        let report = evaluate_lambda_fleet(&[stale], Pillar::Cost, now());
        let bundle = lambda_cost_reporting_bundle(&report);

        assert_eq!(
            bundle.scheduled_delivery_state,
            "blocked_until_fresh_cost_evidence"
        );
        assert!(bundle.stale_data_blocks_delivery);
        assert!(!bundle.portfolio_summary_ready);
        assert!(!bundle.workload_summary_ready);
        assert_eq!(bundle.executive_summary.stale_resources, 1);
        assert!(bundle
            .missing_data_reason_codes
            .contains(&REASON_INV_STALE_DATA.to_string()));
        assert!(bundle
            .evidence_reason_codes
            .contains(&REASON_INV_STALE_DATA.to_string()));
    }

    #[test]
    fn security_flags_deprecated_runtime_as_high() {
        let mut data = healthy_data();
        data["runtime"] = json!("python2.7");
        let r = fixture("fn-old", json!({"owner": "sre"}), data, 1, now());
        let report = evaluate_lambda_fleet(&[r], Pillar::Security, now());
        let finding = report
            .findings
            .iter()
            .find(|f| f.reason_code == REASON_SEC_DEPRECATED_RUNTIME)
            .expect("deprecated runtime finding");
        assert_eq!(finding.severity, Severity::High);
        assert_eq!(finding.evidence["runtime"], json!("python2.7"));
    }

    #[test]
    fn security_flags_missing_owner_tag_as_low() {
        let r = fixture("fn-orphan", json!({}), healthy_data(), 1, now());
        let report = evaluate_lambda_fleet(&[r], Pillar::Security, now());
        assert_eq!(
            report
                .findings
                .iter()
                .map(|f| f.reason_code.as_str())
                .collect::<Vec<_>>(),
            vec![REASON_SEC_MISSING_OWNER_TAG]
        );
    }

    #[test]
    fn security_passes_for_owned_current_runtime() {
        let r = fixture("fn-ok", json!({"owner": "sre"}), healthy_data(), 1, now());
        let report = evaluate_lambda_fleet(&[r], Pillar::Security, now());
        assert!(
            report.findings.is_empty(),
            "unexpected: {:?}",
            report.findings
        );
    }

    #[test]
    fn resilience_flags_missing_timeout_or_memory_config() {
        let r = fixture(
            "fn-noconf",
            json!({"owner": "sre"}),
            json!({"function_name": "fn-noconf", "runtime": "python3.12"}),
            1,
            now(),
        );
        let report = evaluate_lambda_fleet(&[r], Pillar::Resilience, now());
        let finding = report
            .findings
            .iter()
            .find(|f| f.reason_code == REASON_RES_MISSING_CONFIG_DATA)
            .expect("missing config finding");
        assert_eq!(finding.evidence["timeout"], json!(null));
    }

    #[test]
    fn lambda_resilience_triage_context_separates_facts_hypotheses_and_questions() {
        let mut timeout_data = healthy_data();
        timeout_data["cloudwatch_metrics"]["metrics"][1]["datapoints"] =
            json!([{ "value": 29500.0 }]);
        timeout_data["cloudwatch_metrics"]["metrics"][2]["datapoints"] = json!([{ "value": 3.0 }]);
        let timeout_pressure = fixture(
            "fn-res-timeout-triage",
            json!({"owner": "sre"}),
            timeout_data,
            1,
            now(),
        );

        let mut missing_evidence_data = healthy_data();
        for field in [
            "recent_error_log_count",
            "event_source_mapping_count",
            "dead_letter_queue_configured",
        ] {
            missing_evidence_data
                .as_object_mut()
                .expect("object")
                .remove(field);
        }
        let missing_evidence = fixture(
            "fn-res-missing-evidence",
            json!({"owner": "sre"}),
            missing_evidence_data,
            1,
            now(),
        );

        let report = evaluate_lambda_fleet(
            &[timeout_pressure, missing_evidence],
            Pillar::Resilience,
            now(),
        );
        let triage = lambda_resilience_triage_context(&report);

        assert_eq!(triage.workflow_id, "lambda_resilience_triage_context");
        assert_eq!(
            triage.context_builder_id,
            "lambda-resilience-deterministic-context-v1"
        );
        assert_eq!(triage.prompt_template_id, "lambda-resilience-ai-triage-v1");
        assert_eq!(
            triage.audit_event_type,
            "lambda_resilience_ai_triage_context_built"
        );
        assert!(triage.guardrails.read_only_mode);
        assert!(triage.guardrails.no_llm_invocation);
        assert!(triage
            .facts
            .iter()
            .any(|fact| fact.contains(REASON_RES_LOW_TIMEOUT_HEADROOM)));
        assert!(triage
            .hypotheses
            .iter()
            .any(|hypothesis| hypothesis.contains("timeout budget")));
        assert!(triage
            .missing_data_questions
            .iter()
            .any(|question| question.contains("log, event-source, and dead-letter evidence")));
        assert!(triage
            .runbook_copy_markdown
            .contains("Lambda resilience AI triage"));
    }

    #[test]
    fn lambda_resilience_agentic_investigation_plan_is_read_only_until_approval() {
        let mut data = healthy_data();
        data["cloudwatch_metrics"]["metrics"][1]["datapoints"] = json!([{ "value": 29500.0 }]);
        data["cloudwatch_metrics"]["metrics"][2]["datapoints"] = json!([{ "value": 4.0 }]);
        data["cloudwatch_metrics"]["metrics"][3]["datapoints"] = json!([{ "value": 2.0 }]);
        let resource = fixture(
            "fn-res-investigate",
            json!({"owner": "sre"}),
            data,
            1,
            now(),
        );
        let report = evaluate_lambda_fleet(&[resource], Pillar::Resilience, now());
        let plan = lambda_resilience_agentic_investigation_plan(&report);

        assert_eq!(plan.workflow_id, "lambda_resilience_agentic_investigation");
        assert_eq!(
            plan.default_tool_mode,
            LambdaInvestigationToolMode::ReadOnly
        );
        assert!(plan.replay_required);
        assert!(plan.steps.iter().any(|step| {
            step.tool_name == "lambda.resilience.diagnose_failure_and_throttle_path"
                && step.tool_mode == LambdaInvestigationToolMode::ReadOnly
        }));
        assert!(plan.steps.iter().any(|step| {
            step.tool_name == "lambda.resilience.compare_timeout_headroom"
                && step.tool_mode == LambdaInvestigationToolMode::ReadOnly
        }));
        assert_eq!(
            plan.steps.last().map(|step| step.tool_name),
            Some("lambda.resilience.prepare_approval_plan")
        );
        assert!(plan.approval_gates.iter().any(|gate| {
            gate.target_resource_id == "fn-res-investigate"
                && gate.rollback_note_required
                && gate
                    .evidence_reason_codes
                    .contains(&REASON_RES_LOW_TIMEOUT_HEADROOM.to_string())
        }));
    }

    #[test]
    fn lambda_resilience_remediation_workflow_plans_dry_run_actions_until_approved() {
        let mut data = healthy_data();
        data["cloudwatch_metrics"]["metrics"][1]["datapoints"] = json!([{ "value": 29500.0 }]);
        data["cloudwatch_metrics"]["metrics"][2]["datapoints"] = json!([{ "value": 4.0 }]);
        data["cloudwatch_metrics"]["metrics"][3]["datapoints"] = json!([{ "value": 2.0 }]);
        let resource = fixture("fn-res-remediate", json!({"owner": "sre"}), data, 1, now());
        let report = evaluate_lambda_fleet(&[resource], Pillar::Resilience, now());
        let workflow = lambda_resilience_remediation_workflow(&report);

        assert_eq!(workflow.workflow_id, "lambda_resilience_safe_remediation");
        assert!(workflow.read_only_mode);
        assert_eq!(
            workflow.rbac_permission,
            "aws.lambda.resilience.remediation.approve"
        );
        assert_eq!(workflow.audit_stream, "lambda_resilience_remediation_audit");
        assert!(!workflow.stale_data_blocks_execution);
        assert!(!workflow.actions.is_empty());
        assert!(workflow.actions.iter().all(|action| {
            action.dry_run
                && action.requires_approval
                && action.approval_gate_id.is_some()
                && action.status == LambdaRemediationStatus::DryRunPendingApproval
                && action.audit_event_type == "lambda.resilience.remediation.dry_run_planned"
                && action.rollback_note.contains("rollback")
        }));
        assert!(workflow.actions.iter().any(|action| {
            action.kind == LambdaRemediationActionKind::ReviewFailureAndThrottleRecovery
                && action.target_resource_id == "fn-res-remediate"
        }));
        assert!(workflow.actions.iter().any(|action| {
            action.kind == LambdaRemediationActionKind::ReviewTimeoutHeadroom
                && action.target_resource_id == "fn-res-remediate"
        }));
    }

    #[test]
    fn lambda_resilience_slo_policy_snapshot_marks_findings_and_notifications() {
        let mut data = healthy_data();
        data["cloudwatch_metrics"]["metrics"][1]["datapoints"] = json!([{ "value": 29500.0 }]);
        let resource = fixture(
            "fn-res-slo",
            json!({"owner": "sre", "environment": "prod", "application": "checkout"}),
            data,
            1,
            now(),
        );
        let report = evaluate_lambda_fleet(&[resource], Pillar::Resilience, now());
        let snapshot = lambda_resilience_slo_policy_snapshot(&report);

        assert_eq!(snapshot.workflow_id, "lambda_resilience_slo_policy");
        assert_eq!(
            snapshot.objective.objective_id,
            "lambda-resilience-score-min-95"
        );
        assert_eq!(
            snapshot.objective.status,
            LambdaResilienceObjectiveStatus::AtRisk
        );
        assert_eq!(
            snapshot.objective.notification_targets,
            vec!["environment:prod", "owner:sre"]
        );
        assert!(snapshot
            .objective
            .status_history
            .contains(&"resilience_policy_evaluated"));
        assert!(snapshot
            .evidence_reason_codes
            .contains(&REASON_RES_LOW_TIMEOUT_HEADROOM.to_string()));
    }

    #[test]
    fn lambda_resilience_forecast_snapshot_builds_read_only_recovery_exposure() {
        let mut data = healthy_data();
        data["cloudwatch_metrics"]["metrics"][1]["datapoints"] = json!([{ "value": 29500.0 }]);
        data["cloudwatch_metrics"]["metrics"][2]["datapoints"] = json!([{ "value": 4.0 }]);
        data["cloudwatch_metrics"]["metrics"][3]["datapoints"] = json!([{ "value": 2.0 }]);
        let resource = fixture("fn-res-forecast", json!({"owner": "sre"}), data, 1, now());
        let report = evaluate_lambda_fleet(&[resource], Pillar::Resilience, now());
        let forecast = lambda_resilience_forecast_snapshot(&report);

        assert_eq!(forecast.workflow_id, "lambda_resilience_forecasting");
        assert!(forecast.read_only_mode);
        assert_eq!(forecast.baseline_window_days, 30);
        assert_eq!(forecast.forecast_horizon_days, 30);
        assert_eq!(forecast.confidence_level, 75);
        assert_eq!(forecast.risk_level, LambdaResilienceForecastRisk::High);
        assert_eq!(
            forecast.recovery_capacity_risk,
            "active_error_or_throttle_recovery_exposure"
        );
        assert!(forecast
            .what_if_inputs
            .contains(&"review_timeout_and_memory_headroom"));
        assert!(forecast.forecast_band.expected_recovery_exposure_index > 100);
        assert!(forecast.blast_radius_summary.contains("Lambda function"));
        assert!(forecast.recovery_note.contains("read-only"));
        assert!(forecast.risk_drivers.iter().any(|driver| {
            driver.reason_code == REASON_RES_ERROR_OR_THROTTLE_HEALTH_SIGNAL
                && driver.affected_resources == vec!["fn-res-forecast"]
        }));
    }

    #[test]
    fn lambda_resilience_reporting_bundle_blocks_delivery_for_stale_or_missing_evidence() {
        let mut data = healthy_data();
        for field in [
            "recent_error_log_count",
            "event_source_mapping_count",
            "dead_letter_queue_configured",
        ] {
            data.as_object_mut().expect("object").remove(field);
        }
        let stale = fixture("fn-res-report", json!({"owner": "sre"}), data, 30, now());
        let report = evaluate_lambda_fleet(&[stale], Pillar::Resilience, now());
        let bundle = lambda_resilience_reporting_bundle(&report);

        assert_eq!(
            bundle.scheduled_delivery_state,
            "blocked_until_fresh_resilience_evidence"
        );
        assert!(bundle.stale_data_blocks_delivery);
        assert!(!bundle.portfolio_summary_ready);
        assert!(!bundle.workload_summary_ready);
        assert_eq!(bundle.saved_view_id, "lambda-resilience-posture-report");
        assert!(bundle
            .missing_data_reason_codes
            .contains(&REASON_INV_STALE_DATA.to_string()));
        assert!(bundle
            .missing_data_reason_codes
            .contains(&REASON_RES_MISSING_LOG_EVENT_EVIDENCE.to_string()));
    }

    #[test]
    fn stale_inventory_is_reported_as_failure_path() {
        let r = fixture(
            "fn-stale",
            json!({"owner": "sre", "project": "mayyam"}),
            healthy_data(),
            48,
            now(),
        );
        let report = evaluate_lambda_fleet(&[r], Pillar::Cost, now());
        assert_eq!(report.stale_resources, 1);
        assert!(report
            .findings
            .iter()
            .any(|f| f.reason_code == REASON_INV_STALE_DATA));
    }
}

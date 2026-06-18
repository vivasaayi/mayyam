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
            evidence: json!({ "architectures": architectures }),
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
    let missing_fields = missing_fields(resource, &required_fields);
    if !missing_fields.is_empty() {
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
                "missing_fields": missing_fields,
                "resource_data_keys": resource_data_keys(resource),
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
            evidence: json!({ "invocations_max": 0.0 }),
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

fn lambda_cost_metric_names() -> [&'static str; 4] {
    ["Invocations", "Duration", "Errors", "Throttles"]
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

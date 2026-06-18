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

// Deterministic Auto Scaling group inventory and telemetry evaluators for the
// cost, resilience, and security pillars (roadmap rows
// 01-AWS-CLOUD-00064/00073/00100 plus 01-AWS-CLOUD-00065/00074/00101).
//
// Evaluates fields persisted by autoscaling_control_plane: min_size, max_size,
// desired_capacity, availability_zones, health_check_type, load_balancer_names,
// target_group_arns, launch_configuration_name, uses_launch_template,
// uses_mixed_instances_policy, suspended_process_count, plus the tags column.

use chrono::{DateTime, Utc};
use serde::Serialize;
use serde_json::{json, Value};
use std::collections::BTreeSet;

use crate::models::aws_resource::Model as AwsResourceModel;
use crate::services::aws::inventory::types::{
    check_stale, data_str, score_pillar, InventoryFinding, Pillar, PillarReport, Severity,
};

/// Only rows of this resource type are evaluated.
pub const RESOURCE_TYPE: &str = "AutoScalingGroup";

// Reason codes are the stable contract for findings; never reuse or rename.
pub const REASON_COST_NO_TAGS: &str = "ASG_COST_NO_TAGS";
pub const REASON_COST_FIXED_SIZE: &str = "ASG_COST_FIXED_SIZE";
pub const REASON_COST_MISSING_GROUP_METRICS_TELEMETRY: &str =
    "ASG_COST_MISSING_GROUP_METRICS_TELEMETRY";
pub const REASON_COST_MISSING_CAPACITY_TELEMETRY: &str = "ASG_COST_MISSING_CAPACITY_TELEMETRY";
pub const REASON_RES_SINGLE_AZ: &str = "ASG_RES_SINGLE_AZ";
pub const REASON_RES_ELB_HEALTH_CHECK_EC2_ONLY: &str = "ASG_RES_ELB_HEALTH_CHECK_EC2_ONLY";
pub const REASON_RES_SUSPENDED_PROCESSES: &str = "ASG_RES_SUSPENDED_PROCESSES";
pub const REASON_RES_DESIRED_BELOW_MIN: &str = "ASG_RES_DESIRED_BELOW_MIN";
pub const REASON_RES_MISSING_REPLACEMENT_TELEMETRY: &str = "ASG_RES_MISSING_REPLACEMENT_TELEMETRY";
pub const REASON_RES_MISSING_INSTANCE_HEALTH_TELEMETRY: &str =
    "ASG_RES_MISSING_INSTANCE_HEALTH_TELEMETRY";
pub const REASON_RES_UNHEALTHY_INSTANCE_TELEMETRY: &str = "ASG_RES_UNHEALTHY_INSTANCE_TELEMETRY";
pub const REASON_SEC_LEGACY_LAUNCH_CONFIGURATION: &str = "ASG_SEC_LEGACY_LAUNCH_CONFIGURATION";
pub const REASON_SEC_LAUNCH_SOURCE_DATA_NOT_COLLECTED: &str =
    "ASG_SEC_LAUNCH_SOURCE_DATA_NOT_COLLECTED";
pub const REASON_SEC_MISSING_INSTANCE_TELEMETRY: &str = "ASG_SEC_MISSING_INSTANCE_TELEMETRY";
pub const REASON_SEC_TELEMETRY_COLLECTION_ERRORS: &str = "ASG_SEC_TELEMETRY_COLLECTION_ERRORS";
pub const REASON_TEL_MISSING_COLLECTION_METADATA: &str = "ASG_TEL_MISSING_COLLECTION_METADATA";
pub const REASON_TEL_COLLECTION_ERRORS: &str = "ASG_TEL_COLLECTION_ERRORS";
pub const REASON_INV_STALE_DATA: &str = "ASG_INV_STALE_DATA";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum AsgPostureStatus {
    Pass,
    Fail,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AsgPostureRule {
    pub rule_id: &'static str,
    pub status: AsgPostureStatus,
    pub reason_codes: Vec<&'static str>,
    pub affected_resources: Vec<String>,
    pub suppression_supported: bool,
    pub assignment_supported: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AsgPostureSummary {
    pub status: AsgPostureStatus,
    pub rules_evaluated: usize,
    pub rules_failed: usize,
    pub affected_resources: Vec<String>,
    pub rules: Vec<AsgPostureRule>,
}

pub type AsgCostPostureSummary = AsgPostureSummary;
pub type AsgResiliencePostureSummary = AsgPostureSummary;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AsgEvidenceCitation {
    pub reason_code: String,
    pub resource_id: String,
    pub severity: Severity,
    pub evidence: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AsgAiTriageGuardrails {
    pub read_only_mode: bool,
    pub evidence_required: bool,
    pub separate_facts_from_hypotheses: bool,
    pub ask_for_missing_data: bool,
    pub no_llm_invocation: bool,
    pub no_mutation_planning: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AsgTriageContext {
    pub workflow_id: &'static str,
    pub pillar: Pillar,
    pub context_builder_id: &'static str,
    pub prompt_template_id: &'static str,
    pub generation_mode: &'static str,
    pub max_prompt_tokens: u16,
    pub provider_routing: Vec<&'static str>,
    pub audit_event_type: &'static str,
    pub guardrails: AsgAiTriageGuardrails,
    pub facts: Vec<String>,
    pub hypotheses: Vec<String>,
    pub missing_data_questions: Vec<String>,
    pub evidence_citations: Vec<AsgEvidenceCitation>,
}

pub type AsgCostTriageContext = AsgTriageContext;
pub type AsgResilienceTriageContext = AsgTriageContext;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AsgInvestigationStepKind {
    Inspect,
    Compare,
    Diagnose,
    ProposeMutationPlan,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AsgInvestigationToolMode {
    ReadOnly,
    ApprovalRequired,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AsgInvestigationStep {
    pub step_id: String,
    pub kind: AsgInvestigationStepKind,
    pub tool_name: &'static str,
    pub tool_mode: AsgInvestigationToolMode,
    pub target_resource_id: String,
    pub reason_code: String,
    pub stop_condition: String,
    pub evidence: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AsgMutationApprovalGate {
    pub gate_id: String,
    pub target_resource_id: String,
    pub required_approval: &'static str,
    pub blast_radius: String,
    pub rollback_note_required: bool,
    pub evidence_reason_codes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AsgAgenticInvestigationPlan {
    pub workflow_id: &'static str,
    pub default_tool_mode: AsgInvestigationToolMode,
    pub max_tool_calls: usize,
    pub max_evidence_citations: usize,
    pub replay_required: bool,
    pub steps: Vec<AsgInvestigationStep>,
    pub approval_gates: Vec<AsgMutationApprovalGate>,
    pub evidence_citations: Vec<AsgEvidenceCitation>,
}

pub type AsgCostAgenticInvestigationPlan = AsgAgenticInvestigationPlan;
pub type AsgResilienceAgenticInvestigationPlan = AsgAgenticInvestigationPlan;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AsgRemediationActionKind {
    ReviewCostAllocationTags,
    ReviewScalingPolicyCapacity,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AsgRemediationStatus {
    DryRunPendingApproval,
    BlockedMissingEvidence,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AsgCostRemediationAction {
    pub action_id: String,
    pub kind: AsgRemediationActionKind,
    pub status: AsgRemediationStatus,
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
pub struct AsgCostRemediationWorkflow {
    pub workflow_id: &'static str,
    pub read_only_mode: bool,
    pub rbac_permission: &'static str,
    pub audit_stream: &'static str,
    pub stale_data_blocks_execution: bool,
    pub actions: Vec<AsgCostRemediationAction>,
    pub approval_gates: Vec<AsgMutationApprovalGate>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AsgCostObjectiveStatus {
    OnTrack,
    AtRisk,
    Breached,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AsgCostTrendDirection {
    Stable,
    Degrading,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AsgCostPolicyObjective {
    pub objective_id: &'static str,
    pub status: AsgCostObjectiveStatus,
    pub target_score_min: u8,
    pub current_score: u8,
    pub trend_direction: AsgCostTrendDirection,
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
pub struct AsgCostSloPolicySnapshot {
    pub workflow_id: &'static str,
    pub read_only_mode: bool,
    pub freshness_required: bool,
    pub objective: AsgCostPolicyObjective,
    pub evidence_reason_codes: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AsgCostForecastRisk {
    Low,
    Moderate,
    High,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AsgCostForecastBand {
    pub horizon_days: u16,
    pub lower_monthly_cost_index: u16,
    pub expected_monthly_cost_index: u16,
    pub upper_monthly_cost_index: u16,
    pub confidence_level: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AsgCostForecastRiskDriver {
    pub reason_code: String,
    pub affected_resources: Vec<String>,
    pub monthly_cost_index_delta: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AsgCostForecastSnapshot {
    pub workflow_id: &'static str,
    pub read_only_mode: bool,
    pub baseline_window_days: u16,
    pub forecast_horizon_days: u16,
    pub confidence_level: u8,
    pub forecast_band: AsgCostForecastBand,
    pub risk_level: AsgCostForecastRisk,
    pub capacity_risk: &'static str,
    pub backtesting_fixture_status: &'static str,
    pub threshold_controls: Vec<&'static str>,
    pub what_if_inputs: Vec<&'static str>,
    pub blocked_by_stale_data: bool,
    pub blast_radius_summary: String,
    pub missing_data_reason_codes: Vec<String>,
    pub risk_drivers: Vec<AsgCostForecastRiskDriver>,
    pub evidence_reason_codes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AsgCostExecutiveSummary {
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
pub struct AsgCostReportRow {
    pub resource_id: String,
    pub severity: Severity,
    pub reason_code: String,
    pub message: String,
    pub recovery_note: String,
    pub suppression_supported: bool,
    pub evidence: Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AsgCostEngineeringBacklog {
    pub report_id: &'static str,
    pub page: u16,
    pub page_size: u16,
    pub total: usize,
    pub rows: Vec<AsgCostReportRow>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AsgCostIncidentReview {
    pub report_id: &'static str,
    pub page: u16,
    pub page_size: u16,
    pub total: usize,
    pub rows: Vec<AsgCostReportRow>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AsgCostReportingBundle {
    pub workflow_id: &'static str,
    pub read_only_mode: bool,
    pub scheduled_delivery_state: &'static str,
    pub stale_data_blocks_delivery: bool,
    pub portfolio_summary_ready: bool,
    pub workload_summary_ready: bool,
    pub export_formats: Vec<&'static str>,
    pub saved_view_id: &'static str,
    pub executive_summary: AsgCostExecutiveSummary,
    pub engineering_backlog: AsgCostEngineeringBacklog,
    pub incident_review: AsgCostIncidentReview,
    pub missing_data_reason_codes: Vec<String>,
    pub evidence_reason_codes: Vec<String>,
}

/// Evaluate every Auto Scaling group in the fleet for one pillar. Rows whose
/// `resource_type` is not `AutoScalingGroup` are skipped and not counted.
pub fn evaluate_autoscaling_fleet(
    resources: &[AwsResourceModel],
    pillar: Pillar,
    now: DateTime<Utc>,
) -> PillarReport {
    let mut findings: Vec<InventoryFinding> = Vec::new();
    let mut stale_resources = 0usize;
    let mut evaluated = 0usize;

    for resource in resources {
        if resource.resource_type != RESOURCE_TYPE {
            continue;
        }
        evaluated += 1;

        if let Some(stale) = check_stale(resource, pillar, REASON_INV_STALE_DATA, now) {
            stale_resources += 1;
            findings.push(stale);
        }
        match pillar {
            Pillar::Cost => {
                evaluate_telemetry_collection(resource, pillar, &mut findings);
                evaluate_cost(resource, &mut findings);
            }
            Pillar::Security => {
                evaluate_telemetry_collection(resource, pillar, &mut findings);
                evaluate_security(resource, &mut findings);
            }
            Pillar::Resilience => {
                evaluate_telemetry_collection(resource, pillar, &mut findings);
                evaluate_resilience(resource, &mut findings);
            }
            // Pillars without checks for this service yet produce no findings.
            _ => {}
        }
    }

    let score = score_pillar(&findings);
    PillarReport {
        pillar,
        resources_evaluated: evaluated,
        stale_resources,
        score,
        findings,
    }
}

pub fn asg_cost_agentic_investigation_plan(
    report: &PillarReport,
) -> AsgCostAgenticInvestigationPlan {
    let triage = asg_cost_triage_context(report);
    let mut steps = Vec::new();
    let mut approval_gates = Vec::new();

    for citation in &triage.evidence_citations {
        if citation.resource_id == "fleet" {
            continue;
        }

        match citation.reason_code.as_str() {
            REASON_INV_STALE_DATA => {
                steps.push(asg_investigation_step(
                    &steps,
                    AsgInvestigationStepKind::Inspect,
                    "autoscaling.describe_group_inventory",
                    AsgInvestigationToolMode::ReadOnly,
                    citation,
                    "stop when the Auto Scaling group inventory is refreshed or stale evidence is confirmed",
                ));
            }
            REASON_TEL_MISSING_COLLECTION_METADATA => {
                steps.push(asg_investigation_step(
                    &steps,
                    AsgInvestigationStepKind::Inspect,
                    "autoscaling.inspect_collection_metadata",
                    AsgInvestigationToolMode::ReadOnly,
                    citation,
                    "stop when collection start, completion, duration, success, failure, and error counts are recorded",
                ));
            }
            REASON_TEL_COLLECTION_ERRORS => {
                steps.push(asg_investigation_step(
                    &steps,
                    AsgInvestigationStepKind::Diagnose,
                    "autoscaling.inspect_collector_errors",
                    AsgInvestigationToolMode::ReadOnly,
                    citation,
                    "stop when collector logs, throttling, permissions, and retry evidence explain the collection gap",
                ));
            }
            REASON_COST_MISSING_CAPACITY_TELEMETRY => {
                steps.push(asg_investigation_step(
                    &steps,
                    AsgInvestigationStepKind::Inspect,
                    "autoscaling.describe_group_capacity",
                    AsgInvestigationToolMode::ReadOnly,
                    citation,
                    "stop when min, max, desired, and instance counts are recorded",
                ));
            }
            REASON_COST_MISSING_GROUP_METRICS_TELEMETRY => {
                steps.push(asg_investigation_step(
                    &steps,
                    AsgInvestigationStepKind::Inspect,
                    "autoscaling.describe_enabled_metrics",
                    AsgInvestigationToolMode::ReadOnly,
                    citation,
                    "stop when enabled group metrics are collected or confirmed disabled",
                ));
            }
            REASON_COST_NO_TAGS => {
                steps.push(asg_investigation_step(
                    &steps,
                    AsgInvestigationStepKind::Diagnose,
                    "autoscaling.resource_groups.get_tagging_context",
                    AsgInvestigationToolMode::ReadOnly,
                    citation,
                    "stop when owner, team, project, or cost-center can be inferred or the gap is assigned",
                ));
                approval_gates.push(asg_mutation_gate(
                    &approval_gates,
                    citation,
                    "Approve tag writes after ownership is verified",
                ));
            }
            REASON_COST_FIXED_SIZE => {
                steps.push(asg_investigation_step(
                    &steps,
                    AsgInvestigationStepKind::Compare,
                    "autoscaling.compare_scaling_policy_capacity",
                    AsgInvestigationToolMode::ReadOnly,
                    citation,
                    "stop when fixed capacity is compared with demand, scheduled scaling, and scaling policy evidence",
                ));
                approval_gates.push(asg_mutation_gate(
                    &approval_gates,
                    citation,
                    "Approve scaling policy or capacity changes after owner review",
                ));
            }
            _ => {}
        }
    }

    steps.push(AsgInvestigationStep {
        step_id: format!("autoscaling-cost-step-{:02}", steps.len() + 1),
        kind: AsgInvestigationStepKind::ProposeMutationPlan,
        tool_name: "autoscaling.cost.prepare_approval_plan",
        tool_mode: AsgInvestigationToolMode::ApprovalRequired,
        target_resource_id: "investigation".to_string(),
        reason_code: "ASG_COST_APPROVAL_PLAN_REQUIRED".to_string(),
        stop_condition:
            "stop before mutation; require explicit operator approval, blast-radius summary, and rollback note"
                .to_string(),
        evidence: json!({
            "approval_gate_count": approval_gates.len(),
            "read_only_step_count": steps.len(),
        }),
    });

    AsgCostAgenticInvestigationPlan {
        workflow_id: "autoscaling_cost_agentic_investigation",
        default_tool_mode: AsgInvestigationToolMode::ReadOnly,
        max_tool_calls: steps.len().min(12),
        max_evidence_citations: triage.evidence_citations.len(),
        replay_required: true,
        steps,
        approval_gates,
        evidence_citations: triage.evidence_citations,
    }
}

pub fn asg_resilience_agentic_investigation_plan(
    report: &PillarReport,
) -> AsgResilienceAgenticInvestigationPlan {
    let triage = asg_resilience_triage_context(report);
    let mut steps = Vec::new();
    let mut approval_gates = Vec::new();

    for citation in &triage.evidence_citations {
        if citation.resource_id == "fleet" {
            continue;
        }

        match citation.reason_code.as_str() {
            REASON_INV_STALE_DATA => {
                steps.push(asg_resilience_investigation_step(
                    &steps,
                    AsgInvestigationStepKind::Inspect,
                    "autoscaling.describe_group_inventory",
                    AsgInvestigationToolMode::ReadOnly,
                    citation,
                    "stop when the Auto Scaling group inventory is refreshed or stale evidence is confirmed",
                ));
            }
            REASON_TEL_MISSING_COLLECTION_METADATA => {
                steps.push(asg_resilience_investigation_step(
                    &steps,
                    AsgInvestigationStepKind::Inspect,
                    "autoscaling.inspect_collection_metadata",
                    AsgInvestigationToolMode::ReadOnly,
                    citation,
                    "stop when collection start, completion, duration, success, failure, and error counts are recorded",
                ));
            }
            REASON_TEL_COLLECTION_ERRORS => {
                steps.push(asg_resilience_investigation_step(
                    &steps,
                    AsgInvestigationStepKind::Diagnose,
                    "autoscaling.inspect_collector_errors",
                    AsgInvestigationToolMode::ReadOnly,
                    citation,
                    "stop when collector logs, throttling, permissions, and retry evidence explain the resilience evidence gap",
                ));
            }
            REASON_RES_MISSING_REPLACEMENT_TELEMETRY => {
                steps.push(asg_resilience_investigation_step(
                    &steps,
                    AsgInvestigationStepKind::Inspect,
                    "autoscaling.describe_replacement_activity",
                    AsgInvestigationToolMode::ReadOnly,
                    citation,
                    "stop when recent launch, terminate, lifecycle, and replacement activity evidence is recorded",
                ));
            }
            REASON_RES_MISSING_INSTANCE_HEALTH_TELEMETRY => {
                steps.push(asg_resilience_investigation_step(
                    &steps,
                    AsgInvestigationStepKind::Inspect,
                    "autoscaling.describe_instance_health",
                    AsgInvestigationToolMode::ReadOnly,
                    citation,
                    "stop when healthy, unhealthy, lifecycle, and protected instance evidence is recorded",
                ));
            }
            REASON_RES_UNHEALTHY_INSTANCE_TELEMETRY => {
                steps.push(asg_resilience_investigation_step(
                    &steps,
                    AsgInvestigationStepKind::Diagnose,
                    "autoscaling.diagnose_unhealthy_replacement",
                    AsgInvestigationToolMode::ReadOnly,
                    citation,
                    "stop when instance health, lifecycle hooks, termination policy, and replacement timing explain unhealthy capacity",
                ));
                approval_gates.push(asg_resilience_mutation_gate(
                    &approval_gates,
                    citation,
                    "Approve replacement policy changes after resilience owner review",
                ));
            }
            REASON_RES_SINGLE_AZ => {
                steps.push(asg_resilience_investigation_step(
                    &steps,
                    AsgInvestigationStepKind::Compare,
                    "autoscaling.compare_availability_zone_coverage",
                    AsgInvestigationToolMode::ReadOnly,
                    citation,
                    "stop when subnet, AZ, target capacity, and cross-zone evidence explain replacement exposure",
                ));
                approval_gates.push(asg_resilience_mutation_gate(
                    &approval_gates,
                    citation,
                    "Approve subnet or capacity changes after resilience owner review",
                ));
            }
            REASON_RES_ELB_HEALTH_CHECK_EC2_ONLY => {
                steps.push(asg_resilience_investigation_step(
                    &steps,
                    AsgInvestigationStepKind::Inspect,
                    "autoscaling.inspect_elb_health_check_policy",
                    AsgInvestigationToolMode::ReadOnly,
                    citation,
                    "stop when load balancer attachment and EC2 versus ELB health check evidence are recorded",
                ));
                approval_gates.push(asg_resilience_mutation_gate(
                    &approval_gates,
                    citation,
                    "Approve health check policy changes after application owner review",
                ));
            }
            REASON_RES_SUSPENDED_PROCESSES => {
                steps.push(asg_resilience_investigation_step(
                    &steps,
                    AsgInvestigationStepKind::Diagnose,
                    "autoscaling.inspect_suspended_processes",
                    AsgInvestigationToolMode::ReadOnly,
                    citation,
                    "stop when suspended process names, suspension reason, and owner intent are recorded",
                ));
                approval_gates.push(asg_resilience_mutation_gate(
                    &approval_gates,
                    citation,
                    "Approve scaling process resume after owner review",
                ));
            }
            REASON_RES_DESIRED_BELOW_MIN => {
                steps.push(asg_resilience_investigation_step(
                    &steps,
                    AsgInvestigationStepKind::Compare,
                    "autoscaling.compare_capacity_bounds",
                    AsgInvestigationToolMode::ReadOnly,
                    citation,
                    "stop when desired, min, max, warm pool, and pending capacity evidence explain the inconsistency",
                ));
                approval_gates.push(asg_resilience_mutation_gate(
                    &approval_gates,
                    citation,
                    "Approve capacity correction after resilience owner review",
                ));
            }
            _ => {}
        }
    }

    steps.push(AsgInvestigationStep {
        step_id: format!("autoscaling-resilience-step-{:02}", steps.len() + 1),
        kind: AsgInvestigationStepKind::ProposeMutationPlan,
        tool_name: "autoscaling.resilience.prepare_approval_plan",
        tool_mode: AsgInvestigationToolMode::ApprovalRequired,
        target_resource_id: "investigation".to_string(),
        reason_code: "ASG_RESILIENCE_APPROVAL_PLAN_REQUIRED".to_string(),
        stop_condition:
            "stop before mutation; require explicit operator approval, blast-radius summary, and rollback note"
                .to_string(),
        evidence: json!({
            "approval_gate_count": approval_gates.len(),
            "read_only_step_count": steps.len(),
        }),
    });

    AsgAgenticInvestigationPlan {
        workflow_id: "autoscaling_resilience_agentic_investigation",
        default_tool_mode: AsgInvestigationToolMode::ReadOnly,
        max_tool_calls: steps.len().min(12),
        max_evidence_citations: triage.evidence_citations.len(),
        replay_required: true,
        steps,
        approval_gates,
        evidence_citations: triage.evidence_citations,
    }
}

pub fn asg_cost_remediation_workflow(report: &PillarReport) -> AsgCostRemediationWorkflow {
    let investigation = asg_cost_agentic_investigation_plan(report);
    let has_stale_data = report
        .findings
        .iter()
        .any(|finding| finding.reason_code == REASON_INV_STALE_DATA);
    let mut actions = Vec::new();

    for gate in &investigation.approval_gates {
        for reason_code in &gate.evidence_reason_codes {
            if let Some(kind) = asg_remediation_action_kind(reason_code) {
                actions.push(asg_remediation_action(
                    &actions,
                    kind,
                    gate,
                    if has_stale_data {
                        AsgRemediationStatus::BlockedMissingEvidence
                    } else {
                        AsgRemediationStatus::DryRunPendingApproval
                    },
                ));
            }
        }
    }

    AsgCostRemediationWorkflow {
        workflow_id: "autoscaling_cost_safe_remediation",
        read_only_mode: true,
        rbac_permission: "aws.autoscaling.cost.remediation.approve",
        audit_stream: "autoscaling_cost_remediation_audit",
        stale_data_blocks_execution: has_stale_data,
        actions,
        approval_gates: investigation.approval_gates,
    }
}

pub fn asg_cost_slo_policy_snapshot(report: &PillarReport) -> AsgCostSloPolicySnapshot {
    let posture = asg_cost_posture_summary(report);
    let failed_rule_count = posture.rules_failed;
    let affected_resource_count = posture.affected_resources.len();
    let status = asg_cost_objective_status(report.score, failed_rule_count, report.stale_resources);
    let owner_filters = sorted_unique_evidence_values(report, &["owner", "team"]);
    let environment_filters = sorted_unique_evidence_values(report, &["environment", "env"]);
    let application_filters = sorted_unique_evidence_values(report, &["application", "app"]);
    let notification_targets = notification_targets(&owner_filters, &environment_filters);

    AsgCostSloPolicySnapshot {
        workflow_id: "autoscaling_cost_slo_policy",
        read_only_mode: true,
        freshness_required: true,
        objective: AsgCostPolicyObjective {
            objective_id: "autoscaling-cost-score-min-90",
            status,
            target_score_min: 90,
            current_score: report.score,
            trend_direction: asg_cost_trend_direction(status, failed_rule_count),
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

pub fn asg_cost_forecast_snapshot(report: &PillarReport) -> AsgCostForecastSnapshot {
    const BASELINE_WINDOW_DAYS: u16 = 30;
    const FORECAST_HORIZON_DAYS: u16 = 30;
    const CONFIDENCE_LEVEL: u8 = 80;

    let stale_count = count_reason(report, REASON_INV_STALE_DATA);
    let telemetry_error_count = count_reason(report, REASON_TEL_COLLECTION_ERRORS);
    let missing_capacity_count = count_reason(report, REASON_COST_MISSING_CAPACITY_TELEMETRY);
    let missing_metrics_count = count_reason(report, REASON_COST_MISSING_GROUP_METRICS_TELEMETRY);
    let fixed_size_count = count_reason(report, REASON_COST_FIXED_SIZE);
    let missing_tag_count = count_reason(report, REASON_COST_NO_TAGS);
    let missing_collection_metadata_count =
        count_reason(report, REASON_TEL_MISSING_COLLECTION_METADATA);
    let blocked_by_stale_data = report.stale_resources > 0 || stale_count > 0;
    let forecast_blocked = blocked_by_stale_data || telemetry_error_count > 0;

    let expected_monthly_cost_index = 100u16
        + (fixed_size_count as u16 * 22)
        + (missing_capacity_count as u16 * 18)
        + (missing_metrics_count as u16 * 16)
        + (missing_collection_metadata_count as u16 * 10)
        + (missing_tag_count as u16 * 5)
        + (stale_count as u16 * 25)
        + (telemetry_error_count as u16 * 20);
    let uncertainty = 8u16
        + (missing_capacity_count as u16 * 8)
        + (missing_metrics_count as u16 * 6)
        + (missing_collection_metadata_count as u16 * 5)
        + (missing_tag_count as u16 * 2)
        + (report.stale_resources as u16 * 10)
        + (report.resources_evaluated == 0) as u16 * 20;
    let lower_monthly_cost_index = expected_monthly_cost_index.saturating_sub(uncertainty);
    let upper_monthly_cost_index = expected_monthly_cost_index + uncertainty;
    let risk_level = if forecast_blocked {
        AsgCostForecastRisk::Blocked
    } else if upper_monthly_cost_index >= 145 {
        AsgCostForecastRisk::High
    } else if expected_monthly_cost_index > 100 {
        AsgCostForecastRisk::Moderate
    } else {
        AsgCostForecastRisk::Low
    };
    let risk_drivers = asg_cost_forecast_risk_drivers(report);
    let impacted_groups = sorted_unique_resources(
        risk_drivers
            .iter()
            .flat_map(|driver| driver.affected_resources.iter().cloned()),
    );

    AsgCostForecastSnapshot {
        workflow_id: "autoscaling_cost_forecasting",
        read_only_mode: true,
        baseline_window_days: BASELINE_WINDOW_DAYS,
        forecast_horizon_days: FORECAST_HORIZON_DAYS,
        confidence_level: CONFIDENCE_LEVEL,
        forecast_band: AsgCostForecastBand {
            horizon_days: FORECAST_HORIZON_DAYS,
            lower_monthly_cost_index,
            expected_monthly_cost_index,
            upper_monthly_cost_index,
            confidence_level: CONFIDENCE_LEVEL,
        },
        risk_level,
        capacity_risk: asg_cost_capacity_risk(
            forecast_blocked,
            fixed_size_count,
            missing_capacity_count,
            missing_metrics_count,
            missing_collection_metadata_count,
        ),
        backtesting_fixture_status: if report.findings.is_empty() {
            "ready_clean_baseline"
        } else if forecast_blocked || missing_capacity_count > 0 || missing_metrics_count > 0 {
            "needs_fresh_capacity_fixture"
        } else {
            "ready_findings_baseline"
        },
        threshold_controls: vec![
            "monthly_cost_index_warning_threshold",
            "monthly_cost_index_critical_threshold",
        ],
        what_if_inputs: vec![
            "allow_scale_in_for_fixed_groups",
            "restore_missing_capacity_telemetry",
            "enable_group_metrics_collection",
            "apply_cost_allocation_tags",
        ],
        blocked_by_stale_data,
        blast_radius_summary: if impacted_groups.is_empty() {
            "No Auto Scaling groups have cost forecast risk in the current evidence.".to_string()
        } else {
            format!(
                "{} Auto Scaling group(s) have cost forecast risk across capacity and telemetry findings.",
                impacted_groups.len()
            )
        },
        missing_data_reason_codes: asg_cost_forecast_missing_data_reason_codes(report),
        risk_drivers,
        evidence_reason_codes: sorted_unique_reason_codes(report),
    }
}

pub fn asg_cost_reporting_bundle(report: &PillarReport) -> AsgCostReportingBundle {
    let posture = asg_cost_posture_summary(report);
    let reason_codes = sorted_unique_reason_codes(report);
    let rows = asg_cost_report_rows(report);
    let stale_data_blocks_delivery = report.stale_resources > 0
        || report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_INV_STALE_DATA);
    let blast_radius_summary = if posture.affected_resources.is_empty() {
        "No Auto Scaling groups require cost reporting review.".to_string()
    } else {
        format!(
            "{} Auto Scaling group(s) require cost reporting review.",
            posture.affected_resources.len()
        )
    };

    AsgCostReportingBundle {
        workflow_id: "autoscaling_cost_reporting",
        read_only_mode: true,
        scheduled_delivery_state: if stale_data_blocks_delivery {
            "blocked_until_fresh_inventory"
        } else {
            "ready_for_schedule"
        },
        stale_data_blocks_delivery,
        portfolio_summary_ready: !stale_data_blocks_delivery,
        workload_summary_ready: !stale_data_blocks_delivery,
        export_formats: vec!["json", "csv"],
        saved_view_id: "autoscaling-cost-posture-report",
        executive_summary: AsgCostExecutiveSummary {
            report_id: "autoscaling-cost-executive-summary",
            score: report.score,
            resources_evaluated: report.resources_evaluated,
            stale_resources: report.stale_resources,
            rules_failed: posture.rules_failed,
            affected_resources: posture.affected_resources,
            top_reason_codes: reason_codes.clone(),
            blast_radius_summary,
        },
        engineering_backlog: AsgCostEngineeringBacklog {
            report_id: "autoscaling-cost-engineering-backlog",
            page: 0,
            page_size: 50,
            total: rows.len(),
            rows: rows.clone(),
        },
        incident_review: AsgCostIncidentReview {
            report_id: "autoscaling-cost-incident-review",
            page: 0,
            page_size: 50,
            total: rows.len(),
            rows,
        },
        missing_data_reason_codes: asg_cost_reporting_missing_data_reason_codes(report),
        evidence_reason_codes: reason_codes,
    }
}

fn asg_cost_objective_status(
    score: u8,
    failed_rule_count: usize,
    stale_resources: usize,
) -> AsgCostObjectiveStatus {
    if stale_resources > 0 || score < 70 {
        AsgCostObjectiveStatus::Breached
    } else if failed_rule_count > 0 || score < 90 {
        AsgCostObjectiveStatus::AtRisk
    } else {
        AsgCostObjectiveStatus::OnTrack
    }
}

fn asg_cost_trend_direction(
    status: AsgCostObjectiveStatus,
    failed_rule_count: usize,
) -> AsgCostTrendDirection {
    match status {
        AsgCostObjectiveStatus::OnTrack => AsgCostTrendDirection::Stable,
        AsgCostObjectiveStatus::AtRisk if failed_rule_count <= 1 => AsgCostTrendDirection::Stable,
        AsgCostObjectiveStatus::AtRisk | AsgCostObjectiveStatus::Breached => {
            AsgCostTrendDirection::Degrading
        }
    }
}

fn sorted_unique_reason_codes(report: &PillarReport) -> Vec<String> {
    report
        .findings
        .iter()
        .map(|finding| finding.reason_code.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn sorted_unique_evidence_values(report: &PillarReport, keys: &[&str]) -> Vec<String> {
    report
        .findings
        .iter()
        .filter_map(|finding| evidence_string(&finding.evidence, keys))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn evidence_string(evidence: &Value, keys: &[&str]) -> Option<String> {
    keys.iter()
        .find_map(|key| {
            evidence
                .get("tags")
                .and_then(|tags| tags.get(*key))
                .or_else(|| evidence.get(*key))
                .and_then(Value::as_str)
        })
        .filter(|value| !value.trim().is_empty())
        .map(|value| value.trim().to_string())
}

fn notification_targets(owner_filters: &[String], environment_filters: &[String]) -> Vec<String> {
    let mut targets: BTreeSet<String> = owner_filters
        .iter()
        .map(|owner| format!("owner:{}", owner))
        .collect();
    targets.extend(
        environment_filters
            .iter()
            .map(|environment| format!("environment:{}", environment)),
    );
    if targets.is_empty() {
        targets.insert("cost-operations".to_string());
    }
    targets.into_iter().collect()
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

fn asg_cost_forecast_missing_data_reason_codes(report: &PillarReport) -> Vec<String> {
    [
        REASON_INV_STALE_DATA,
        REASON_TEL_MISSING_COLLECTION_METADATA,
        REASON_TEL_COLLECTION_ERRORS,
        REASON_COST_MISSING_CAPACITY_TELEMETRY,
        REASON_COST_MISSING_GROUP_METRICS_TELEMETRY,
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

fn asg_cost_forecast_risk_drivers(report: &PillarReport) -> Vec<AsgCostForecastRiskDriver> {
    [
        (REASON_INV_STALE_DATA, 25u16),
        (REASON_TEL_COLLECTION_ERRORS, 20u16),
        (REASON_COST_FIXED_SIZE, 22u16),
        (REASON_COST_MISSING_CAPACITY_TELEMETRY, 18u16),
        (REASON_COST_MISSING_GROUP_METRICS_TELEMETRY, 16u16),
        (REASON_TEL_MISSING_COLLECTION_METADATA, 10u16),
        (REASON_COST_NO_TAGS, 5u16),
    ]
    .into_iter()
    .filter_map(|(reason_code, delta)| {
        let affected_resources = resources_for_reason(report, reason_code);
        if affected_resources.is_empty() {
            None
        } else {
            Some(AsgCostForecastRiskDriver {
                reason_code: reason_code.to_string(),
                monthly_cost_index_delta: delta * affected_resources.len() as u16,
                affected_resources,
            })
        }
    })
    .collect()
}

fn asg_cost_capacity_risk(
    forecast_blocked: bool,
    fixed_size_count: usize,
    missing_capacity_count: usize,
    missing_metrics_count: usize,
    missing_collection_metadata_count: usize,
) -> &'static str {
    if forecast_blocked {
        "blocked_until_inventory_refresh"
    } else if fixed_size_count > 0 && missing_metrics_count > 0 {
        "fixed_capacity_and_missing_group_metrics"
    } else if fixed_size_count > 0 {
        "fixed_capacity_scale_in_disabled"
    } else if missing_capacity_count > 0 {
        "unknown_due_to_missing_capacity_telemetry"
    } else if missing_metrics_count > 0 {
        "unknown_due_to_missing_group_metrics"
    } else if missing_collection_metadata_count > 0 {
        "unknown_due_to_missing_collection_metadata"
    } else {
        "cost_capacity_within_current_thresholds"
    }
}

fn asg_cost_reporting_missing_data_reason_codes(report: &PillarReport) -> Vec<String> {
    [
        REASON_INV_STALE_DATA,
        REASON_TEL_MISSING_COLLECTION_METADATA,
        REASON_TEL_COLLECTION_ERRORS,
        REASON_COST_MISSING_CAPACITY_TELEMETRY,
        REASON_COST_MISSING_GROUP_METRICS_TELEMETRY,
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

fn asg_cost_report_rows(report: &PillarReport) -> Vec<AsgCostReportRow> {
    report
        .findings
        .iter()
        .map(|finding| AsgCostReportRow {
            resource_id: finding.resource_id.clone(),
            severity: finding.severity,
            reason_code: finding.reason_code.clone(),
            message: finding.message.clone(),
            recovery_note: asg_cost_reporting_recovery_note(&finding.reason_code).to_string(),
            suppression_supported: true,
            evidence: finding.evidence.clone(),
        })
        .collect()
}

fn asg_cost_reporting_recovery_note(reason_code: &str) -> &'static str {
    match reason_code {
        REASON_INV_STALE_DATA => {
            "Refresh Auto Scaling inventory before sharing the cost report."
        }
        REASON_TEL_MISSING_COLLECTION_METADATA => {
            "Collect telemetry run metadata before scheduling report delivery."
        }
        REASON_TEL_COLLECTION_ERRORS => {
            "Resolve Auto Scaling collector errors before publishing report findings."
        }
        REASON_COST_MISSING_CAPACITY_TELEMETRY => {
            "Collect min, max, desired, and instance-count evidence before prioritizing cost actions."
        }
        REASON_COST_MISSING_GROUP_METRICS_TELEMETRY => {
            "Enable or collect group metrics before quantifying utilization and scale-in opportunity."
        }
        REASON_COST_NO_TAGS => {
            "Add owner, environment, and application tags before routing cost findings."
        }
        REASON_COST_FIXED_SIZE => {
            "Review scaling policy and capacity history before changing min or max size."
        }
        _ => "Review Auto Scaling cost evidence and owner context before action.",
    }
}

fn asg_remediation_action_kind(reason_code: &str) -> Option<AsgRemediationActionKind> {
    match reason_code {
        REASON_COST_NO_TAGS => Some(AsgRemediationActionKind::ReviewCostAllocationTags),
        REASON_COST_FIXED_SIZE => Some(AsgRemediationActionKind::ReviewScalingPolicyCapacity),
        _ => None,
    }
}

fn asg_remediation_action(
    existing_actions: &[AsgCostRemediationAction],
    kind: AsgRemediationActionKind,
    gate: &AsgMutationApprovalGate,
    status: AsgRemediationStatus,
) -> AsgCostRemediationAction {
    let action_number = existing_actions.len() + 1;
    let action_slug = match kind {
        AsgRemediationActionKind::ReviewCostAllocationTags => "review-cost-allocation-tags",
        AsgRemediationActionKind::ReviewScalingPolicyCapacity => "review-scaling-policy-capacity",
    };

    AsgCostRemediationAction {
        action_id: format!("autoscaling-cost-remediation-{:02}", action_number),
        kind,
        status,
        target_resource_id: gate.target_resource_id.clone(),
        dry_run: true,
        requires_approval: true,
        approval_gate_id: Some(gate.gate_id.clone()),
        audit_event_type: "autoscaling.cost.remediation.dry_run_planned",
        idempotency_key: format!(
            "autoscaling-cost-{}-{}",
            gate.target_resource_id, action_slug
        ),
        blast_radius: gate.blast_radius.clone(),
        rollback_note: format!(
            "Before approval, record rollback or recovery notes for {} on {}.",
            action_slug, gate.target_resource_id
        ),
        validation_steps: vec![
            "refresh Auto Scaling capacity, tag, and group metric evidence",
            "verify owner, blast radius, budget impact, and scaling-policy intent",
            "capture operator approval, rollback note, and audit id before execution",
        ],
        evidence_reason_codes: gate.evidence_reason_codes.clone(),
    }
}

fn asg_investigation_step(
    existing_steps: &[AsgInvestigationStep],
    kind: AsgInvestigationStepKind,
    tool_name: &'static str,
    tool_mode: AsgInvestigationToolMode,
    citation: &AsgEvidenceCitation,
    stop_condition: &str,
) -> AsgInvestigationStep {
    AsgInvestigationStep {
        step_id: format!("autoscaling-cost-step-{:02}", existing_steps.len() + 1),
        kind,
        tool_name,
        tool_mode,
        target_resource_id: citation.resource_id.clone(),
        reason_code: citation.reason_code.clone(),
        stop_condition: stop_condition.to_string(),
        evidence: citation.evidence.clone(),
    }
}

fn asg_resilience_investigation_step(
    existing_steps: &[AsgInvestigationStep],
    kind: AsgInvestigationStepKind,
    tool_name: &'static str,
    tool_mode: AsgInvestigationToolMode,
    citation: &AsgEvidenceCitation,
    stop_condition: &str,
) -> AsgInvestigationStep {
    AsgInvestigationStep {
        step_id: format!(
            "autoscaling-resilience-step-{:02}",
            existing_steps.len() + 1
        ),
        kind,
        tool_name,
        tool_mode,
        target_resource_id: citation.resource_id.clone(),
        reason_code: citation.reason_code.clone(),
        stop_condition: stop_condition.to_string(),
        evidence: citation.evidence.clone(),
    }
}

fn asg_mutation_gate(
    existing_gates: &[AsgMutationApprovalGate],
    citation: &AsgEvidenceCitation,
    required_approval: &'static str,
) -> AsgMutationApprovalGate {
    AsgMutationApprovalGate {
        gate_id: format!("autoscaling-cost-approval-{:02}", existing_gates.len() + 1),
        target_resource_id: citation.resource_id.clone(),
        required_approval,
        blast_radius: format!(
            "single Auto Scaling group {}; no mutation is executable from the investigation plan",
            citation.resource_id
        ),
        rollback_note_required: true,
        evidence_reason_codes: vec![citation.reason_code.clone()],
    }
}

fn asg_resilience_mutation_gate(
    existing_gates: &[AsgMutationApprovalGate],
    citation: &AsgEvidenceCitation,
    required_approval: &'static str,
) -> AsgMutationApprovalGate {
    AsgMutationApprovalGate {
        gate_id: format!(
            "autoscaling-resilience-approval-{:02}",
            existing_gates.len() + 1
        ),
        target_resource_id: citation.resource_id.clone(),
        required_approval,
        blast_radius: format!(
            "single Auto Scaling group {}; no mutation is executable from the investigation plan",
            citation.resource_id
        ),
        rollback_note_required: true,
        evidence_reason_codes: vec![citation.reason_code.clone()],
    }
}

pub fn asg_cost_triage_context(report: &PillarReport) -> AsgCostTriageContext {
    let mut facts = Vec::new();
    let mut hypotheses = Vec::new();
    let mut missing_data_questions = Vec::new();
    let mut evidence_citations = Vec::new();

    for finding in &report.findings {
        facts.push(format!(
            "{} affects {} with {:?} severity",
            finding.reason_code, finding.resource_id, finding.severity
        ));
        evidence_citations.push(AsgEvidenceCitation {
            reason_code: finding.reason_code.clone(),
            resource_id: finding.resource_id.clone(),
            severity: finding.severity,
            evidence: finding.evidence.clone(),
        });

        match finding.reason_code.as_str() {
            REASON_INV_STALE_DATA => missing_data_questions.push(format!(
                "Refresh Auto Scaling inventory for {} before generating cost triage",
                finding.resource_id
            )),
            REASON_TEL_MISSING_COLLECTION_METADATA => missing_data_questions.push(format!(
                "Collect telemetry collection metadata for {} before trusting Auto Scaling cost evidence",
                finding.resource_id
            )),
            REASON_TEL_COLLECTION_ERRORS => hypotheses.push(format!(
                "{} has telemetry collection errors; inspect collector logs, Auto Scaling API throttling, permissions, and retry evidence before changing scaling policy or capacity",
                finding.resource_id
            )),
            REASON_COST_MISSING_CAPACITY_TELEMETRY => missing_data_questions.push(format!(
                "Collect capacity telemetry for {} before quantifying ASG cost posture",
                finding.resource_id
            )),
            REASON_COST_MISSING_GROUP_METRICS_TELEMETRY => missing_data_questions.push(format!(
                "Enable or collect Auto Scaling group metrics for {} before explaining scaling cost behavior",
                finding.resource_id
            )),
            REASON_COST_NO_TAGS => missing_data_questions.push(format!(
                "Assign owner, team, project, or cost-center metadata for {} so Auto Scaling savings can be routed",
                finding.resource_id
            )),
            REASON_COST_FIXED_SIZE => hypotheses.push(format!(
                "{} may be paying for fixed capacity because scale-in is disabled by min == max; verify steady-state demand, scheduled scaling, and availability requirements before recommending capacity changes",
                finding.resource_id
            )),
            _ => {}
        }
    }

    AsgTriageContext {
        workflow_id: "autoscaling_cost_triage_context",
        pillar: report.pillar,
        context_builder_id: "autoscaling-cost-deterministic-context-v1",
        prompt_template_id: "autoscaling-cost-ai-triage-v1",
        generation_mode: "deterministic_no_llm",
        max_prompt_tokens: 1200,
        provider_routing: vec!["primary_ops_llm", "fallback_ops_llm"],
        audit_event_type: "autoscaling_ai_triage_context_built",
        guardrails: AsgAiTriageGuardrails {
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
        evidence_citations,
    }
}

pub fn asg_resilience_triage_context(report: &PillarReport) -> AsgResilienceTriageContext {
    let mut facts = Vec::new();
    let mut hypotheses = Vec::new();
    let mut missing_data_questions = Vec::new();
    let mut evidence_citations = Vec::new();

    for finding in &report.findings {
        facts.push(format!(
            "{} affects {} with {:?} severity",
            finding.reason_code, finding.resource_id, finding.severity
        ));
        evidence_citations.push(AsgEvidenceCitation {
            reason_code: finding.reason_code.clone(),
            resource_id: finding.resource_id.clone(),
            severity: finding.severity,
            evidence: finding.evidence.clone(),
        });

        match finding.reason_code.as_str() {
            REASON_INV_STALE_DATA => missing_data_questions.push(format!(
                "Refresh Auto Scaling inventory for {} before generating resilience triage",
                finding.resource_id
            )),
            REASON_TEL_MISSING_COLLECTION_METADATA => missing_data_questions.push(format!(
                "Collect telemetry collection metadata for {} before trusting Auto Scaling resilience evidence",
                finding.resource_id
            )),
            REASON_TEL_COLLECTION_ERRORS => hypotheses.push(format!(
                "{} has telemetry collection errors; inspect collector logs, Auto Scaling API throttling, permissions, and retry evidence before changing replacement behavior",
                finding.resource_id
            )),
            REASON_RES_MISSING_REPLACEMENT_TELEMETRY => missing_data_questions.push(format!(
                "Collect replacement telemetry for {} before explaining resilience posture",
                finding.resource_id
            )),
            REASON_RES_MISSING_INSTANCE_HEALTH_TELEMETRY => missing_data_questions.push(format!(
                "Collect instance health telemetry for {} before explaining replacement behavior",
                finding.resource_id
            )),
            REASON_RES_UNHEALTHY_INSTANCE_TELEMETRY => hypotheses.push(format!(
                "{} may not be replacing unhealthy instances quickly enough; verify instance health, lifecycle state, and termination policy evidence before recommending changes",
                finding.resource_id
            )),
            REASON_RES_SINGLE_AZ => hypotheses.push(format!(
                "{} may lose replacement capacity during an AZ outage; verify cross-zone target capacity and load balancer health checks before recommending changes",
                finding.resource_id
            )),
            REASON_RES_ELB_HEALTH_CHECK_EC2_ONLY => hypotheses.push(format!(
                "{} may keep instances that fail application health checks because the group uses EC2 health checks while load balanced; verify ELB health check configuration before recommending changes",
                finding.resource_id
            )),
            REASON_RES_SUSPENDED_PROCESSES => hypotheses.push(format!(
                "{} may not replace unhealthy capacity while scaling processes are suspended; verify suspension reason and owner intent before recommending changes",
                finding.resource_id
            )),
            REASON_RES_DESIRED_BELOW_MIN => missing_data_questions.push(format!(
                "Re-sync capacity telemetry for {} because desired capacity is below min size in the latest snapshot",
                finding.resource_id
            )),
            _ => {}
        }
    }

    AsgTriageContext {
        workflow_id: "autoscaling_resilience_triage_context",
        pillar: report.pillar,
        context_builder_id: "autoscaling-resilience-deterministic-context-v1",
        prompt_template_id: "autoscaling-resilience-ai-triage-v1",
        generation_mode: "deterministic_no_llm",
        max_prompt_tokens: 1200,
        provider_routing: vec!["primary_ops_llm", "fallback_ops_llm"],
        audit_event_type: "autoscaling_resilience_ai_triage_context_built",
        guardrails: AsgAiTriageGuardrails {
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
        evidence_citations,
    }
}

pub fn asg_cost_posture_summary(report: &PillarReport) -> AsgCostPostureSummary {
    let rules = vec![
        asg_posture_rule(
            report,
            "asg-cost-inventory-freshness",
            &[REASON_INV_STALE_DATA],
        ),
        asg_posture_rule(
            report,
            "asg-cost-telemetry-collection-metadata-present",
            &[REASON_TEL_MISSING_COLLECTION_METADATA],
        ),
        asg_posture_rule(
            report,
            "asg-cost-telemetry-collection-errors-clear",
            &[REASON_TEL_COLLECTION_ERRORS],
        ),
        asg_posture_rule(
            report,
            "asg-cost-capacity-telemetry-present",
            &[REASON_COST_MISSING_CAPACITY_TELEMETRY],
        ),
        asg_posture_rule(
            report,
            "asg-cost-group-metrics-telemetry-present",
            &[REASON_COST_MISSING_GROUP_METRICS_TELEMETRY],
        ),
        asg_posture_rule(
            report,
            "asg-cost-allocation-tags-present",
            &[REASON_COST_NO_TAGS],
        ),
        asg_posture_rule(
            report,
            "asg-cost-scale-in-capable",
            &[REASON_COST_FIXED_SIZE],
        ),
    ];
    asg_posture_summary(rules)
}

pub fn asg_resilience_posture_summary(report: &PillarReport) -> AsgResiliencePostureSummary {
    let rules = vec![
        asg_posture_rule(
            report,
            "asg-resilience-inventory-freshness",
            &[REASON_INV_STALE_DATA],
        ),
        asg_posture_rule(
            report,
            "asg-resilience-telemetry-collection-metadata-present",
            &[REASON_TEL_MISSING_COLLECTION_METADATA],
        ),
        asg_posture_rule(
            report,
            "asg-resilience-telemetry-collection-errors-clear",
            &[REASON_TEL_COLLECTION_ERRORS],
        ),
        asg_posture_rule(
            report,
            "asg-resilience-replacement-telemetry-present",
            &[REASON_RES_MISSING_REPLACEMENT_TELEMETRY],
        ),
        asg_posture_rule(
            report,
            "asg-resilience-instance-health-telemetry-present",
            &[REASON_RES_MISSING_INSTANCE_HEALTH_TELEMETRY],
        ),
        asg_posture_rule(
            report,
            "asg-resilience-instance-health-clean",
            &[REASON_RES_UNHEALTHY_INSTANCE_TELEMETRY],
        ),
        asg_posture_rule(
            report,
            "asg-resilience-multi-az-placement",
            &[REASON_RES_SINGLE_AZ],
        ),
        asg_posture_rule(
            report,
            "asg-resilience-elb-health-checks",
            &[REASON_RES_ELB_HEALTH_CHECK_EC2_ONLY],
        ),
        asg_posture_rule(
            report,
            "asg-resilience-scaling-processes-active",
            &[REASON_RES_SUSPENDED_PROCESSES],
        ),
        asg_posture_rule(
            report,
            "asg-resilience-desired-capacity-at-or-above-min",
            &[REASON_RES_DESIRED_BELOW_MIN],
        ),
    ];
    asg_posture_summary(rules)
}

fn asg_posture_summary(rules: Vec<AsgPostureRule>) -> AsgPostureSummary {
    let affected_resources = sorted_unique_resources(
        rules
            .iter()
            .flat_map(|rule| rule.affected_resources.iter().cloned()),
    );
    let rules_failed = rules
        .iter()
        .filter(|rule| rule.status == AsgPostureStatus::Fail)
        .count();

    AsgPostureSummary {
        status: if rules_failed == 0 {
            AsgPostureStatus::Pass
        } else {
            AsgPostureStatus::Fail
        },
        rules_evaluated: rules.len(),
        rules_failed,
        affected_resources,
        rules,
    }
}

fn asg_posture_rule(
    report: &PillarReport,
    rule_id: &'static str,
    reason_codes: &[&'static str],
) -> AsgPostureRule {
    let affected_resources = sorted_unique_resources(
        report
            .findings
            .iter()
            .filter(|finding| reason_codes.contains(&finding.reason_code.as_str()))
            .map(|finding| finding.resource_id.clone()),
    );

    AsgPostureRule {
        rule_id,
        status: if affected_resources.is_empty() {
            AsgPostureStatus::Pass
        } else {
            AsgPostureStatus::Fail
        },
        reason_codes: reason_codes.to_vec(),
        affected_resources,
        suppression_supported: true,
        assignment_supported: true,
    }
}

fn sorted_unique_resources(resources: impl Iterator<Item = String>) -> Vec<String> {
    resources
        .filter(|resource_id| !resource_id.is_empty())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn data_i64(resource_data: &Value, key: &str) -> Option<i64> {
    resource_data.get(key).and_then(|v| v.as_i64())
}

fn data_array_len(resource_data: &Value, key: &str) -> Option<usize> {
    resource_data
        .get(key)
        .and_then(|v| v.as_array())
        .map(|a| a.len())
}

fn data_u64(resource_data: &Value, key: &str) -> Option<u64> {
    resource_data.get(key).and_then(|value| {
        value
            .as_u64()
            .or_else(|| value.as_i64().map(|n| n.max(0) as u64))
    })
}

fn resource_data_keys(resource: &AwsResourceModel) -> Vec<String> {
    let mut keys = resource
        .resource_data
        .as_object()
        .map(|data| data.keys().cloned().collect::<Vec<_>>())
        .unwrap_or_default();
    keys.sort();
    keys
}

fn missing_fields<'a>(resource: &AwsResourceModel, required_fields: &'a [&str]) -> Vec<&'a str> {
    required_fields
        .iter()
        .copied()
        .filter(|field| resource.resource_data.get(*field).is_none())
        .collect()
}

fn is_elb_attached(resource: &AwsResourceModel) -> bool {
    data_array_len(&resource.resource_data, "load_balancer_names").unwrap_or(0) > 0
        || data_array_len(&resource.resource_data, "target_group_arns").unwrap_or(0) > 0
}

fn evaluate_telemetry_collection(
    resource: &AwsResourceModel,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    let required_fields = [
        "telemetry_collection_started_at",
        "telemetry_collection_completed_at",
        "telemetry_collection_duration_ms",
        "telemetry_collection_success_count",
        "telemetry_collection_failure_count",
        "telemetry_collection_error_count",
    ];
    let missing = missing_fields(resource, &required_fields);
    if !missing.is_empty() {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar,
            reason_code: REASON_TEL_MISSING_COLLECTION_METADATA.to_string(),
            severity: Severity::Medium,
            message: format!(
                "Auto Scaling group {} is missing telemetry collection metadata needed to trust the latest evidence",
                resource.resource_id
            ),
            evidence: json!({
                "required_fields": required_fields,
                "missing_fields": missing,
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
            pillar,
            reason_code: REASON_TEL_COLLECTION_ERRORS.to_string(),
            severity: Severity::High,
            message: format!(
                "Auto Scaling group {} has telemetry collection errors; the pillar evidence may be incomplete",
                resource.resource_id
            ),
            evidence: json!({
                "telemetry_collection_error_count": error_count,
                "telemetry_collection_errors": errors,
            }),
        });
    }
}

fn evaluate_cost(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    let capacity_fields = ["min_size", "max_size", "desired_capacity", "instance_count"];
    let missing_capacity = missing_fields(resource, &capacity_fields);
    if !missing_capacity.is_empty() {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Cost,
            reason_code: REASON_COST_MISSING_CAPACITY_TELEMETRY.to_string(),
            severity: Severity::Medium,
            message: format!(
                "Auto Scaling group {} is missing capacity telemetry needed to explain cost posture",
                resource.resource_id
            ),
            evidence: json!({
                "required_fields": capacity_fields,
                "missing_fields": missing_capacity,
                "resource_data_keys": resource_data_keys(resource),
            }),
        });
    }

    let group_metric_fields = ["enabled_metrics", "enabled_metric_count"];
    let missing_group_metrics = missing_fields(resource, &group_metric_fields);
    if !missing_group_metrics.is_empty() {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Cost,
            reason_code: REASON_COST_MISSING_GROUP_METRICS_TELEMETRY.to_string(),
            severity: Severity::Medium,
            message: format!(
                "Auto Scaling group {} is missing enabled group metrics telemetry needed to quantify scaling cost behavior",
                resource.resource_id
            ),
            evidence: json!({
                "required_fields": group_metric_fields,
                "missing_fields": missing_group_metrics,
                "resource_data_keys": resource_data_keys(resource),
            }),
        });
    }

    let tags_empty = resource
        .tags
        .as_object()
        .map(|m| m.is_empty())
        .unwrap_or(true);
    if tags_empty {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Cost,
            reason_code: REASON_COST_NO_TAGS.to_string(),
            severity: Severity::Medium,
            message: format!(
                "Auto Scaling group {} has no tags recorded (untagged resource or tag collection gap); cost allocation cannot be assessed",
                resource.resource_id
            ),
            evidence: json!({ "tags": resource.tags }),
        });
    }

    // A group pinned to min == max can never scale in, so capacity is paid for
    // even when idle. Empty fixed groups (max == 0) hold no instances and are
    // not flagged.
    let min_size = data_i64(&resource.resource_data, "min_size");
    let max_size = data_i64(&resource.resource_data, "max_size");
    if let (Some(min), Some(max)) = (min_size, max_size) {
        if min == max && max > 0 {
            findings.push(InventoryFinding {
                resource_id: resource.resource_id.clone(),
                arn: resource.arn.clone(),
                pillar: Pillar::Cost,
                reason_code: REASON_COST_FIXED_SIZE.to_string(),
                severity: Severity::Low,
                message: format!(
                    "Auto Scaling group {} is pinned to a fixed size (min == max == {}); it can never scale in, so idle capacity is still billed",
                    resource.resource_id, max
                ),
                evidence: json!({ "min_size": min, "max_size": max, "tags": resource.tags }),
            });
        }
    }
}

fn evaluate_security(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    if resource.resource_data.get("instance_health").is_none() {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Security,
            reason_code: REASON_SEC_MISSING_INSTANCE_TELEMETRY.to_string(),
            severity: Severity::Medium,
            message: format!(
                "Auto Scaling group {} is missing instance telemetry needed to validate security exposure of running members",
                resource.resource_id
            ),
            evidence: json!({
                "required_fields": ["instance_health"],
                "resource_data_keys": resource_data_keys(resource),
            }),
        });
    }

    let telemetry_error_count =
        data_u64(&resource.resource_data, "telemetry_collection_error_count").unwrap_or(0);
    if telemetry_error_count > 0 {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Security,
            reason_code: REASON_SEC_TELEMETRY_COLLECTION_ERRORS.to_string(),
            severity: Severity::High,
            message: format!(
                "Auto Scaling group {} has telemetry collection errors that can hide security evidence",
                resource.resource_id
            ),
            evidence: json!({
                "telemetry_collection_error_count": telemetry_error_count,
                "telemetry_collection_errors": resource.resource_data.get("telemetry_collection_errors"),
            }),
        });
    }

    let legacy_launch_configuration =
        data_str(&resource.resource_data, "launch_configuration_name").is_some();
    let uses_launch_template = resource
        .resource_data
        .get("uses_launch_template")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let uses_mixed_instances_policy = resource
        .resource_data
        .get("uses_mixed_instances_policy")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    if legacy_launch_configuration {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Security,
            reason_code: REASON_SEC_LEGACY_LAUNCH_CONFIGURATION.to_string(),
            severity: Severity::Medium,
            message: format!(
                "Auto Scaling group {} uses a deprecated launch configuration; migrate to a launch template to enforce IMDSv2 and current instance security features",
                resource.resource_id
            ),
            evidence: json!({
                "launch_configuration_name":
                    data_str(&resource.resource_data, "launch_configuration_name")
            }),
        });
    } else if !uses_launch_template && !uses_mixed_instances_policy {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Security,
            reason_code: REASON_SEC_LAUNCH_SOURCE_DATA_NOT_COLLECTED.to_string(),
            severity: Severity::Low,
            message: format!(
                "Launch source for Auto Scaling group {} is not collected yet (no launch configuration, launch template, or mixed instances policy recorded); security pillar cannot be fully assessed",
                resource.resource_id
            ),
            evidence: json!({ "launch_source_collected": false }),
        });
    }
}

fn evaluate_resilience(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    let replacement_fields = [
        "availability_zones",
        "health_check_type",
        "load_balancer_names",
        "target_group_arns",
        "suspended_process_count",
        "instance_count",
    ];
    let missing_replacement = missing_fields(resource, &replacement_fields);
    if !missing_replacement.is_empty() {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Resilience,
            reason_code: REASON_RES_MISSING_REPLACEMENT_TELEMETRY.to_string(),
            severity: Severity::Medium,
            message: format!(
                "Auto Scaling group {} is missing replacement telemetry needed to explain resilience posture",
                resource.resource_id
            ),
            evidence: json!({
                "required_fields": replacement_fields,
                "missing_fields": missing_replacement,
                "resource_data_keys": resource_data_keys(resource),
            }),
        });
    }

    let instance_health_fields = [
        "instance_health",
        "healthy_instance_count",
        "unhealthy_instance_count",
        "in_service_instance_count",
    ];
    let missing_instance_health = missing_fields(resource, &instance_health_fields);
    if !missing_instance_health.is_empty() {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Resilience,
            reason_code: REASON_RES_MISSING_INSTANCE_HEALTH_TELEMETRY.to_string(),
            severity: Severity::Medium,
            message: format!(
                "Auto Scaling group {} is missing instance health telemetry needed to assess replacement behavior",
                resource.resource_id
            ),
            evidence: json!({
                "required_fields": instance_health_fields,
                "missing_fields": missing_instance_health,
                "resource_data_keys": resource_data_keys(resource),
            }),
        });
    }

    let unhealthy = data_i64(&resource.resource_data, "unhealthy_instance_count").unwrap_or(0);
    if unhealthy > 0 {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Resilience,
            reason_code: REASON_RES_UNHEALTHY_INSTANCE_TELEMETRY.to_string(),
            severity: Severity::High,
            message: format!(
                "Auto Scaling group {} has {} unhealthy instance(s) in the latest telemetry",
                resource.resource_id, unhealthy
            ),
            evidence: json!({
                "unhealthy_instance_count": unhealthy,
                "instance_health": resource.resource_data.get("instance_health"),
            }),
        });
    }

    if let Some(az_count) = data_array_len(&resource.resource_data, "availability_zones") {
        if az_count <= 1 {
            findings.push(InventoryFinding {
                resource_id: resource.resource_id.clone(),
                arn: resource.arn.clone(),
                pillar: Pillar::Resilience,
                reason_code: REASON_RES_SINGLE_AZ.to_string(),
                severity: Severity::High,
                message: format!(
                    "Auto Scaling group {} spans {} availability zone(s); an AZ outage takes down all capacity",
                    resource.resource_id, az_count
                ),
                evidence: json!({
                    "availability_zones": resource.resource_data.get("availability_zones")
                }),
            });
        }
    }

    // A load-balanced group with EC2-only health checks keeps instances that
    // fail application health checks in service.
    let health_check_type = data_str(&resource.resource_data, "health_check_type");
    if is_elb_attached(resource) && health_check_type.as_deref() == Some("EC2") {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Resilience,
            reason_code: REASON_RES_ELB_HEALTH_CHECK_EC2_ONLY.to_string(),
            severity: Severity::Medium,
            message: format!(
                "Auto Scaling group {} is attached to a load balancer but uses EC2-only health checks; instances failing application health checks are not replaced",
                resource.resource_id
            ),
            evidence: json!({ "health_check_type": "EC2", "elb_attached": true }),
        });
    }

    let suspended = data_i64(&resource.resource_data, "suspended_process_count").unwrap_or(0);
    if suspended > 0 {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Resilience,
            reason_code: REASON_RES_SUSPENDED_PROCESSES.to_string(),
            severity: Severity::Medium,
            message: format!(
                "Auto Scaling group {} has {} scaling process(es) suspended; unhealthy instances may not be replaced while suspension is in effect",
                resource.resource_id, suspended
            ),
            evidence: json!({ "suspended_process_count": suspended }),
        });
    }

    let desired = data_i64(&resource.resource_data, "desired_capacity");
    let min_size = data_i64(&resource.resource_data, "min_size");
    if let (Some(desired), Some(min)) = (desired, min_size) {
        if desired < min {
            findings.push(InventoryFinding {
                resource_id: resource.resource_id.clone(),
                arn: resource.arn.clone(),
                pillar: Pillar::Resilience,
                reason_code: REASON_RES_DESIRED_BELOW_MIN.to_string(),
                severity: Severity::Low,
                message: format!(
                    "Auto Scaling group {} reports desired capacity {} below min size {}; this is an inconsistent collection snapshot worth re-syncing",
                    resource.resource_id, desired, min
                ),
                evidence: json!({ "desired_capacity": desired, "min_size": min }),
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;
    use uuid::Uuid;

    fn fixture(
        resource_id: &str,
        tags: Value,
        resource_data: Value,
        now: DateTime<Utc>,
    ) -> AwsResourceModel {
        let refreshed = now - Duration::hours(1);
        AwsResourceModel {
            id: Uuid::new_v4(),
            sync_id: None,
            account_id: "123456789012".to_string(),
            profile: None,
            region: "us-east-1".to_string(),
            resource_type: RESOURCE_TYPE.to_string(),
            resource_id: resource_id.to_string(),
            arn: format!(
                "arn:aws:autoscaling:us-east-1:123456789012:autoScalingGroup:uuid:autoScalingGroupName/{}",
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
            "auto_scaling_group_name": "asg-ok",
            "min_size": 1,
            "max_size": 4,
            "desired_capacity": 2,
            "availability_zones": ["us-east-1a", "us-east-1b"],
            "health_check_type": "ELB",
            "health_check_grace_period": 300,
            "load_balancer_names": [],
            "target_group_arns": ["arn:aws:elasticloadbalancing:us-east-1:123456789012:targetgroup/tg/abc"],
            "uses_launch_template": true,
            "uses_mixed_instances_policy": false,
            "instance_count": 2,
            "enabled_metrics": [
                "GroupDesiredCapacity",
                "GroupInServiceInstances",
                "GroupTotalInstances",
                "GroupPendingInstances",
                "GroupStandbyInstances",
                "GroupTerminatingInstances"
            ],
            "enabled_metric_count": 6,
            "instance_health": [
                {
                    "instance_id": "i-healthy-1",
                    "health_status": "Healthy",
                    "lifecycle_state": "InService"
                },
                {
                    "instance_id": "i-healthy-2",
                    "health_status": "Healthy",
                    "lifecycle_state": "InService"
                }
            ],
            "healthy_instance_count": 2,
            "unhealthy_instance_count": 0,
            "in_service_instance_count": 2,
            "suspended_process_count": 0,
            "telemetry_collection_started_at": "2026-06-10T00:00:00Z",
            "telemetry_collection_completed_at": "2026-06-10T00:00:03Z",
            "telemetry_collection_duration_ms": 3000,
            "telemetry_collection_success_count": 1,
            "telemetry_collection_failure_count": 0,
            "telemetry_collection_error_count": 0,
            "telemetry_collection_errors": [],
        })
    }

    fn codes(report: &PillarReport) -> Vec<&str> {
        report
            .findings
            .iter()
            .map(|f| f.reason_code.as_str())
            .collect()
    }

    #[test]
    fn cost_flags_untagged_group() {
        let r = fixture("asg-untagged", json!({}), healthy_data(), now());
        let report = evaluate_autoscaling_fleet(&[r], Pillar::Cost, now());
        assert_eq!(codes(&report), vec![REASON_COST_NO_TAGS]);
    }

    #[test]
    fn cost_flags_fixed_size_group() {
        let mut data = healthy_data();
        data["min_size"] = json!(3);
        data["max_size"] = json!(3);
        data["desired_capacity"] = json!(3);
        let r = fixture("asg-fixed", json!({"team": "core"}), data, now());
        let report = evaluate_autoscaling_fleet(&[r], Pillar::Cost, now());
        assert_eq!(codes(&report), vec![REASON_COST_FIXED_SIZE]);
    }

    #[test]
    fn cost_does_not_flag_empty_fixed_group() {
        let mut data = healthy_data();
        data["min_size"] = json!(0);
        data["max_size"] = json!(0);
        data["desired_capacity"] = json!(0);
        let r = fixture("asg-empty", json!({"team": "core"}), data, now());
        let report = evaluate_autoscaling_fleet(&[r], Pillar::Cost, now());
        assert!(
            report.findings.is_empty(),
            "unexpected: {:?}",
            report.findings
        );
    }

    #[test]
    fn telemetry_cost_flags_missing_group_metrics() {
        let mut data = healthy_data();
        data.as_object_mut().unwrap().remove("enabled_metrics");
        data.as_object_mut().unwrap().remove("enabled_metric_count");
        let r = fixture("asg-nometrics", json!({"team": "core"}), data, now());
        let report = evaluate_autoscaling_fleet(&[r], Pillar::Cost, now());
        assert!(codes(&report).contains(&REASON_COST_MISSING_GROUP_METRICS_TELEMETRY));
    }

    #[test]
    fn resilience_flags_single_az_as_high() {
        let mut data = healthy_data();
        data["availability_zones"] = json!(["us-east-1a"]);
        let r = fixture("asg-1az", json!({"team": "core"}), data, now());
        let report = evaluate_autoscaling_fleet(&[r], Pillar::Resilience, now());
        assert_eq!(codes(&report), vec![REASON_RES_SINGLE_AZ]);
        assert!(matches!(report.findings[0].severity, Severity::High));
    }

    #[test]
    fn resilience_flags_ec2_health_check_on_load_balanced_group() {
        let mut data = healthy_data();
        data["health_check_type"] = json!("EC2");
        let r = fixture("asg-ec2hc", json!({"team": "core"}), data, now());
        let report = evaluate_autoscaling_fleet(&[r], Pillar::Resilience, now());
        assert_eq!(codes(&report), vec![REASON_RES_ELB_HEALTH_CHECK_EC2_ONLY]);
    }

    #[test]
    fn resilience_allows_ec2_health_check_without_load_balancer() {
        let mut data = healthy_data();
        data["health_check_type"] = json!("EC2");
        data["target_group_arns"] = json!([]);
        data["load_balancer_names"] = json!([]);
        let r = fixture("asg-nolb", json!({"team": "core"}), data, now());
        let report = evaluate_autoscaling_fleet(&[r], Pillar::Resilience, now());
        assert!(
            report.findings.is_empty(),
            "unexpected: {:?}",
            report.findings
        );
    }

    #[test]
    fn resilience_flags_suspended_processes() {
        let mut data = healthy_data();
        data["suspended_process_count"] = json!(2);
        let r = fixture("asg-susp", json!({"team": "core"}), data, now());
        let report = evaluate_autoscaling_fleet(&[r], Pillar::Resilience, now());
        assert_eq!(codes(&report), vec![REASON_RES_SUSPENDED_PROCESSES]);
    }

    #[test]
    fn resilience_flags_desired_below_min_snapshot() {
        let mut data = healthy_data();
        data["desired_capacity"] = json!(0);
        let r = fixture("asg-below", json!({"team": "core"}), data, now());
        let report = evaluate_autoscaling_fleet(&[r], Pillar::Resilience, now());
        assert_eq!(codes(&report), vec![REASON_RES_DESIRED_BELOW_MIN]);
    }

    #[test]
    fn telemetry_resilience_flags_missing_and_unhealthy_instance_health() {
        let mut missing_health = healthy_data();
        missing_health
            .as_object_mut()
            .unwrap()
            .remove("instance_health");
        missing_health
            .as_object_mut()
            .unwrap()
            .remove("healthy_instance_count");
        missing_health
            .as_object_mut()
            .unwrap()
            .remove("unhealthy_instance_count");
        let missing = fixture(
            "asg-no-health-telemetry",
            json!({"team": "core"}),
            missing_health,
            now(),
        );

        let mut unhealthy_data = healthy_data();
        unhealthy_data["healthy_instance_count"] = json!(1);
        unhealthy_data["unhealthy_instance_count"] = json!(1);
        unhealthy_data["instance_health"] = json!([
            {
                "instance_id": "i-healthy",
                "health_status": "Healthy",
                "lifecycle_state": "InService"
            },
            {
                "instance_id": "i-unhealthy",
                "health_status": "Unhealthy",
                "lifecycle_state": "InService"
            }
        ]);
        let unhealthy = fixture(
            "asg-unhealthy",
            json!({"team": "core"}),
            unhealthy_data,
            now(),
        );

        let report = evaluate_autoscaling_fleet(&[missing, unhealthy], Pillar::Resilience, now());
        let codes = codes(&report);
        assert!(codes.contains(&REASON_RES_MISSING_INSTANCE_HEALTH_TELEMETRY));
        assert!(codes.contains(&REASON_RES_UNHEALTHY_INSTANCE_TELEMETRY));
    }

    #[test]
    fn security_flags_legacy_launch_configuration() {
        let mut data = healthy_data();
        data["launch_configuration_name"] = json!("legacy-lc");
        data["uses_launch_template"] = json!(false);
        let r = fixture("asg-lc", json!({"team": "core"}), data, now());
        let report = evaluate_autoscaling_fleet(&[r], Pillar::Security, now());
        assert_eq!(codes(&report), vec![REASON_SEC_LEGACY_LAUNCH_CONFIGURATION]);
    }

    #[test]
    fn security_reports_gap_when_launch_source_missing() {
        let mut data = healthy_data();
        data["uses_launch_template"] = json!(false);
        data["uses_mixed_instances_policy"] = json!(false);
        let r = fixture("asg-gap", json!({"team": "core"}), data, now());
        let report = evaluate_autoscaling_fleet(&[r], Pillar::Security, now());
        assert_eq!(
            codes(&report),
            vec![REASON_SEC_LAUNCH_SOURCE_DATA_NOT_COLLECTED]
        );
    }

    #[test]
    fn security_passes_launch_template_group() {
        let r = fixture("asg-lt", json!({"team": "core"}), healthy_data(), now());
        let report = evaluate_autoscaling_fleet(&[r], Pillar::Security, now());
        assert!(
            report.findings.is_empty(),
            "unexpected: {:?}",
            report.findings
        );
    }

    #[test]
    fn telemetry_security_flags_missing_instance_telemetry_and_collection_errors() {
        let mut missing_instance_telemetry = healthy_data();
        missing_instance_telemetry
            .as_object_mut()
            .unwrap()
            .remove("instance_health");
        let missing = fixture(
            "asg-no-instance-telemetry",
            json!({"team": "core"}),
            missing_instance_telemetry,
            now(),
        );

        let mut collection_error = healthy_data();
        collection_error["telemetry_collection_success_count"] = json!(0);
        collection_error["telemetry_collection_failure_count"] = json!(1);
        collection_error["telemetry_collection_error_count"] = json!(1);
        collection_error["telemetry_collection_errors"] = json!([
            {
                "source": "autoscaling",
                "operation": "DescribeAutoScalingGroups",
                "error": "throttled"
            }
        ]);
        let errored = fixture(
            "asg-collection-error",
            json!({"team": "core"}),
            collection_error,
            now(),
        );

        let report = evaluate_autoscaling_fleet(&[missing, errored], Pillar::Security, now());
        let codes = codes(&report);
        assert!(codes.contains(&REASON_SEC_MISSING_INSTANCE_TELEMETRY));
        assert!(codes.contains(&REASON_SEC_TELEMETRY_COLLECTION_ERRORS));
    }

    #[test]
    fn stale_inventory_is_flagged() {
        let mut r = fixture("asg-stale", json!({"team": "core"}), healthy_data(), now());
        r.last_refreshed = now() - Duration::hours(48);
        let report = evaluate_autoscaling_fleet(&[r], Pillar::Resilience, now());
        assert_eq!(report.stale_resources, 1);
        assert!(codes(&report).contains(&REASON_INV_STALE_DATA));
    }

    #[test]
    fn non_asg_resources_are_skipped_and_not_counted() {
        let mut r = fixture("queue-1", json!({}), json!({}), now());
        r.resource_type = "SqsQueue".to_string();
        let report = evaluate_autoscaling_fleet(&[r], Pillar::Cost, now());
        assert_eq!(report.resources_evaluated, 0);
        assert!(report.findings.is_empty());
    }

    #[test]
    fn healthy_group_passes_all_pillars() {
        let r = fixture("asg-ok", json!({"team": "core"}), healthy_data(), now());
        for pillar in [Pillar::Cost, Pillar::Security, Pillar::Resilience] {
            let report = evaluate_autoscaling_fleet(std::slice::from_ref(&r), pillar, now());
            assert!(
                report.findings.is_empty(),
                "unexpected for {:?}: {:?}",
                pillar,
                report.findings
            );
            assert_eq!(report.score, 100);
        }
    }

    #[test]
    fn telemetry_metadata_gaps_are_reported_for_claimed_pillars() {
        let mut data = healthy_data();
        data.as_object_mut()
            .expect("object")
            .remove("telemetry_collection_duration_ms");
        let r = fixture(
            "asg-no-telemetry-metadata",
            json!({"team": "core"}),
            data,
            now(),
        );

        for pillar in [Pillar::Cost, Pillar::Security, Pillar::Resilience] {
            let report = evaluate_autoscaling_fleet(std::slice::from_ref(&r), pillar, now());
            assert!(
                codes(&report).contains(&"ASG_TEL_MISSING_COLLECTION_METADATA"),
                "expected missing telemetry metadata for {:?}: {:?}",
                pillar,
                report.findings
            );
        }
    }

    #[test]
    fn telemetry_collection_errors_are_reported_for_claimed_pillars() {
        let mut data = healthy_data();
        data["telemetry_collection_success_count"] = json!(0);
        data["telemetry_collection_failure_count"] = json!(1);
        data["telemetry_collection_error_count"] = json!(1);
        data["telemetry_collection_errors"] = json!([
            {
                "source": "autoscaling",
                "operation": "DescribeAutoScalingGroups",
                "error": "throttled"
            }
        ]);
        let r = fixture("asg-telemetry-error", json!({"team": "core"}), data, now());

        for pillar in [Pillar::Cost, Pillar::Security, Pillar::Resilience] {
            let report = evaluate_autoscaling_fleet(std::slice::from_ref(&r), pillar, now());
            assert!(
                codes(&report).contains(&"ASG_TEL_COLLECTION_ERRORS"),
                "expected telemetry collection error for {:?}: {:?}",
                pillar,
                report.findings
            );
        }
    }

    #[test]
    fn cost_flags_missing_capacity_telemetry() {
        let mut data = healthy_data();
        data.as_object_mut()
            .expect("object")
            .remove("desired_capacity");
        let r = fixture("asg-no-capacity", json!({"team": "core"}), data, now());

        let report = evaluate_autoscaling_fleet(&[r], Pillar::Cost, now());
        assert!(
            codes(&report).contains(&"ASG_COST_MISSING_CAPACITY_TELEMETRY"),
            "expected capacity telemetry gap: {:?}",
            report.findings
        );
    }

    #[test]
    fn asg_cost_posture_summary_flags_cost_rules() {
        let mut missing_data = healthy_data();
        for field in [
            "enabled_metrics",
            "enabled_metric_count",
            "desired_capacity",
            "telemetry_collection_duration_ms",
        ] {
            missing_data.as_object_mut().expect("object").remove(field);
        }
        let missing = fixture("asg-cost-missing", json!({}), missing_data, now());

        let mut fixed_data = healthy_data();
        fixed_data["min_size"] = json!(3);
        fixed_data["max_size"] = json!(3);
        fixed_data["desired_capacity"] = json!(3);
        let fixed = fixture("asg-fixed", json!({"team": "core"}), fixed_data, now());

        let report = evaluate_autoscaling_fleet(&[missing, fixed], Pillar::Cost, now());
        let posture = asg_cost_posture_summary(&report);

        assert_eq!(posture.status, AsgPostureStatus::Fail);
        assert_eq!(posture.rules_evaluated, 7);
        assert_eq!(posture.rules_failed, 5);
        assert_eq!(
            posture.affected_resources,
            vec!["asg-cost-missing".to_string(), "asg-fixed".to_string()]
        );
        assert!(posture
            .rules
            .iter()
            .all(|rule| { rule.suppression_supported && rule.assignment_supported }));
        assert!(posture.rules.iter().any(|rule| {
            rule.rule_id == "asg-cost-telemetry-collection-metadata-present"
                && rule.status == AsgPostureStatus::Fail
                && rule.reason_codes == vec![REASON_TEL_MISSING_COLLECTION_METADATA]
                && rule.affected_resources == vec!["asg-cost-missing".to_string()]
        }));
        assert!(posture.rules.iter().any(|rule| {
            rule.rule_id == "asg-cost-capacity-telemetry-present"
                && rule.status == AsgPostureStatus::Fail
                && rule.reason_codes == vec![REASON_COST_MISSING_CAPACITY_TELEMETRY]
                && rule.affected_resources == vec!["asg-cost-missing".to_string()]
        }));
        assert!(posture.rules.iter().any(|rule| {
            rule.rule_id == "asg-cost-group-metrics-telemetry-present"
                && rule.status == AsgPostureStatus::Fail
                && rule.reason_codes == vec![REASON_COST_MISSING_GROUP_METRICS_TELEMETRY]
                && rule.affected_resources == vec!["asg-cost-missing".to_string()]
        }));
        assert!(posture.rules.iter().any(|rule| {
            rule.rule_id == "asg-cost-allocation-tags-present"
                && rule.status == AsgPostureStatus::Fail
                && rule.reason_codes == vec![REASON_COST_NO_TAGS]
                && rule.affected_resources == vec!["asg-cost-missing".to_string()]
        }));
        assert!(posture.rules.iter().any(|rule| {
            rule.rule_id == "asg-cost-scale-in-capable"
                && rule.status == AsgPostureStatus::Fail
                && rule.reason_codes == vec![REASON_COST_FIXED_SIZE]
                && rule.affected_resources == vec!["asg-fixed".to_string()]
        }));
    }

    #[test]
    fn asg_cost_posture_summary_tracks_stale_inventory() {
        let mut stale = fixture("asg-stale", json!({"team": "core"}), healthy_data(), now());
        stale.last_refreshed = now() - Duration::hours(48);
        let report = evaluate_autoscaling_fleet(&[stale], Pillar::Cost, now());
        let posture = asg_cost_posture_summary(&report);

        assert_eq!(posture.status, AsgPostureStatus::Fail);
        assert_eq!(posture.rules_evaluated, 7);
        assert!(posture.rules.iter().any(|rule| {
            rule.rule_id == "asg-cost-inventory-freshness"
                && rule.status == AsgPostureStatus::Fail
                && rule.reason_codes == vec![REASON_INV_STALE_DATA]
                && rule.affected_resources == vec!["asg-stale".to_string()]
        }));
    }

    #[test]
    fn asg_cost_posture_summary_passes_for_healthy_group() {
        let healthy = fixture("asg-ok", json!({"team": "core"}), healthy_data(), now());
        let report = evaluate_autoscaling_fleet(&[healthy], Pillar::Cost, now());
        let posture = asg_cost_posture_summary(&report);

        assert_eq!(posture.status, AsgPostureStatus::Pass);
        assert_eq!(posture.rules_evaluated, 7);
        assert_eq!(posture.rules_failed, 0);
        assert!(posture.affected_resources.is_empty());
        assert!(posture
            .rules
            .iter()
            .all(|rule| rule.status == AsgPostureStatus::Pass));
    }

    #[test]
    fn asg_resilience_posture_summary_flags_replacement_rules() {
        let mut single_az_data = healthy_data();
        single_az_data["availability_zones"] = json!(["us-east-1a"]);
        single_az_data["health_check_type"] = json!("EC2");
        single_az_data["suspended_process_count"] = json!(1);
        let single_az = fixture(
            "asg-single-az",
            json!({"team": "core"}),
            single_az_data,
            now(),
        );

        let mut unhealthy_data = healthy_data();
        unhealthy_data["unhealthy_instance_count"] = json!(1);
        unhealthy_data["instance_health"] = json!([
            {
                "instance_id": "i-unhealthy",
                "health_status": "Unhealthy",
                "lifecycle_state": "InService"
            }
        ]);
        let unhealthy = fixture(
            "asg-unhealthy-posture",
            json!({"team": "core"}),
            unhealthy_data,
            now(),
        );

        let report = evaluate_autoscaling_fleet(&[single_az, unhealthy], Pillar::Resilience, now());
        let posture = asg_resilience_posture_summary(&report);

        assert_eq!(posture.status, AsgPostureStatus::Fail);
        assert_eq!(posture.rules_evaluated, 10);
        assert_eq!(posture.rules_failed, 4);
        assert_eq!(
            posture.affected_resources,
            vec![
                "asg-single-az".to_string(),
                "asg-unhealthy-posture".to_string()
            ]
        );
        assert!(posture
            .rules
            .iter()
            .all(|rule| { rule.suppression_supported && rule.assignment_supported }));
        assert!(posture.rules.iter().any(|rule| {
            rule.rule_id == "asg-resilience-multi-az-placement"
                && rule.status == AsgPostureStatus::Fail
                && rule.reason_codes == vec![REASON_RES_SINGLE_AZ]
                && rule.affected_resources == vec!["asg-single-az".to_string()]
        }));
        assert!(posture.rules.iter().any(|rule| {
            rule.rule_id == "asg-resilience-elb-health-checks"
                && rule.status == AsgPostureStatus::Fail
                && rule.reason_codes == vec![REASON_RES_ELB_HEALTH_CHECK_EC2_ONLY]
                && rule.affected_resources == vec!["asg-single-az".to_string()]
        }));
        assert!(posture.rules.iter().any(|rule| {
            rule.rule_id == "asg-resilience-scaling-processes-active"
                && rule.status == AsgPostureStatus::Fail
                && rule.reason_codes == vec![REASON_RES_SUSPENDED_PROCESSES]
                && rule.affected_resources == vec!["asg-single-az".to_string()]
        }));
        assert!(posture.rules.iter().any(|rule| {
            rule.rule_id == "asg-resilience-instance-health-clean"
                && rule.status == AsgPostureStatus::Fail
                && rule.reason_codes == vec![REASON_RES_UNHEALTHY_INSTANCE_TELEMETRY]
                && rule.affected_resources == vec!["asg-unhealthy-posture".to_string()]
        }));
    }

    #[test]
    fn asg_resilience_posture_summary_passes_for_healthy_multi_az_group() {
        let healthy = fixture("asg-res-ok", json!({"team": "core"}), healthy_data(), now());
        let report = evaluate_autoscaling_fleet(&[healthy], Pillar::Resilience, now());
        let posture = asg_resilience_posture_summary(&report);

        assert_eq!(posture.status, AsgPostureStatus::Pass);
        assert_eq!(posture.rules_evaluated, 10);
        assert_eq!(posture.rules_failed, 0);
        assert!(posture.affected_resources.is_empty());
        assert!(posture
            .rules
            .iter()
            .all(|rule| rule.status == AsgPostureStatus::Pass));
    }

    #[test]
    fn asg_resilience_posture_summary_tracks_stale_inventory() {
        let mut stale = fixture(
            "asg-res-stale",
            json!({"team": "core"}),
            healthy_data(),
            now(),
        );
        stale.last_refreshed = now() - Duration::hours(48);
        let report = evaluate_autoscaling_fleet(&[stale], Pillar::Resilience, now());
        let posture = asg_resilience_posture_summary(&report);

        assert_eq!(posture.status, AsgPostureStatus::Fail);
        assert!(posture.rules.iter().any(|rule| {
            rule.rule_id == "asg-resilience-inventory-freshness"
                && rule.status == AsgPostureStatus::Fail
                && rule.reason_codes == vec![REASON_INV_STALE_DATA]
                && rule.affected_resources == vec!["asg-res-stale".to_string()]
        }));
    }

    #[test]
    fn asg_resilience_triage_context_separates_facts_hypotheses_and_questions() {
        let mut single_az_data = healthy_data();
        single_az_data["availability_zones"] = json!(["us-east-1a"]);
        single_az_data["health_check_type"] = json!("EC2");
        let single_az = fixture(
            "asg-res-single-az-triage",
            json!({"team": "core"}),
            single_az_data,
            now(),
        );

        let mut missing_health_data = healthy_data();
        missing_health_data
            .as_object_mut()
            .expect("object")
            .remove("instance_health");
        missing_health_data
            .as_object_mut()
            .expect("object")
            .remove("healthy_instance_count");
        missing_health_data
            .as_object_mut()
            .expect("object")
            .remove("unhealthy_instance_count");
        let missing_health = fixture(
            "asg-res-missing-health-triage",
            json!({"team": "core"}),
            missing_health_data,
            now(),
        );

        let report =
            evaluate_autoscaling_fleet(&[single_az, missing_health], Pillar::Resilience, now());
        let triage = asg_resilience_triage_context(&report);

        assert_eq!(triage.workflow_id, "autoscaling_resilience_triage_context");
        assert_eq!(
            triage.context_builder_id,
            "autoscaling-resilience-deterministic-context-v1"
        );
        assert_eq!(
            triage.prompt_template_id,
            "autoscaling-resilience-ai-triage-v1"
        );
        assert_eq!(triage.generation_mode, "deterministic_no_llm");
        assert_eq!(triage.max_prompt_tokens, 1200);
        assert_eq!(
            triage.audit_event_type,
            "autoscaling_resilience_ai_triage_context_built"
        );
        assert!(triage.guardrails.read_only_mode);
        assert!(triage.guardrails.evidence_required);
        assert!(triage.guardrails.no_llm_invocation);
        assert!(triage.guardrails.no_mutation_planning);
        assert!(triage
            .facts
            .iter()
            .any(|fact| fact.contains(REASON_RES_SINGLE_AZ)));
        assert!(triage
            .hypotheses
            .iter()
            .any(|hypothesis| hypothesis.contains("AZ outage")));
        assert!(triage
            .missing_data_questions
            .iter()
            .any(|question| question.contains("instance health telemetry")));
        assert!(triage.evidence_citations.iter().any(|citation| {
            citation.reason_code == REASON_RES_ELB_HEALTH_CHECK_EC2_ONLY
                && citation.resource_id == "asg-res-single-az-triage"
        }));
    }

    #[test]
    fn asg_resilience_agentic_investigation_plan_is_read_only_until_approval() {
        let mut single_az_data = healthy_data();
        single_az_data["availability_zones"] = json!(["us-east-1a"]);
        single_az_data["health_check_type"] = json!("EC2");
        let single_az = fixture(
            "asg-res-single-az-investigation",
            json!({"team": "core"}),
            single_az_data,
            now(),
        );

        let mut missing_health_data = healthy_data();
        for field in [
            "health_check_type",
            "instance_health",
            "healthy_instance_count",
            "unhealthy_instance_count",
        ] {
            missing_health_data
                .as_object_mut()
                .expect("object")
                .remove(field);
        }
        let missing_health = fixture(
            "asg-res-missing-health-investigation",
            json!({"team": "core"}),
            missing_health_data,
            now(),
        );

        let report =
            evaluate_autoscaling_fleet(&[single_az, missing_health], Pillar::Resilience, now());
        let plan = asg_resilience_agentic_investigation_plan(&report);

        assert_eq!(
            plan.workflow_id,
            "autoscaling_resilience_agentic_investigation"
        );
        assert_eq!(plan.default_tool_mode, AsgInvestigationToolMode::ReadOnly);
        assert!(plan.replay_required);
        assert!(plan.steps.iter().any(|step| {
            step.tool_name == "autoscaling.describe_replacement_activity"
                && step.tool_mode == AsgInvestigationToolMode::ReadOnly
                && step.target_resource_id == "asg-res-missing-health-investigation"
        }));
        assert!(plan.steps.iter().any(|step| {
            step.tool_name == "autoscaling.describe_instance_health"
                && step.tool_mode == AsgInvestigationToolMode::ReadOnly
                && step.target_resource_id == "asg-res-missing-health-investigation"
        }));
        assert!(plan.steps.iter().any(|step| {
            step.tool_name == "autoscaling.compare_availability_zone_coverage"
                && step.tool_mode == AsgInvestigationToolMode::ReadOnly
                && step.target_resource_id == "asg-res-single-az-investigation"
        }));
        assert!(plan.steps.iter().any(|step| {
            step.tool_name == "autoscaling.inspect_elb_health_check_policy"
                && step.tool_mode == AsgInvestigationToolMode::ReadOnly
                && step.target_resource_id == "asg-res-single-az-investigation"
        }));
        assert_eq!(
            plan.steps.last().map(|step| step.tool_mode),
            Some(AsgInvestigationToolMode::ApprovalRequired)
        );
        assert!(plan.steps.iter().all(|step| {
            step.tool_mode == AsgInvestigationToolMode::ReadOnly
                || step.tool_name == "autoscaling.resilience.prepare_approval_plan"
        }));
        assert!(plan.approval_gates.iter().any(|gate| {
            gate.target_resource_id == "asg-res-single-az-investigation"
                && gate.rollback_note_required
                && gate
                    .evidence_reason_codes
                    .contains(&REASON_RES_SINGLE_AZ.to_string())
        }));
        assert_eq!(plan.max_evidence_citations, report.findings.len());
    }

    #[test]
    fn asg_cost_triage_context_separates_facts_hypotheses_and_questions() {
        let mut missing_data = healthy_data();
        for field in [
            "enabled_metrics",
            "enabled_metric_count",
            "desired_capacity",
            "telemetry_collection_duration_ms",
        ] {
            missing_data.as_object_mut().expect("object").remove(field);
        }
        let missing = fixture("asg-cost-missing", json!({}), missing_data, now());

        let mut fixed_data = healthy_data();
        fixed_data["min_size"] = json!(3);
        fixed_data["max_size"] = json!(3);
        fixed_data["desired_capacity"] = json!(3);
        let fixed = fixture("asg-fixed", json!({"team": "core"}), fixed_data, now());

        let mut errored_data = healthy_data();
        errored_data["telemetry_collection_success_count"] = json!(0);
        errored_data["telemetry_collection_failure_count"] = json!(1);
        errored_data["telemetry_collection_error_count"] = json!(1);
        errored_data["telemetry_collection_errors"] = json!([
            {
                "source": "autoscaling",
                "operation": "DescribeAutoScalingGroups",
                "error": "throttled"
            }
        ]);
        let errored = fixture(
            "asg-telemetry-error",
            json!({"team": "core"}),
            errored_data,
            now(),
        );

        let report = evaluate_autoscaling_fleet(&[missing, fixed, errored], Pillar::Cost, now());
        let triage = asg_cost_triage_context(&report);

        assert_eq!(triage.workflow_id, "autoscaling_cost_triage_context");
        assert_eq!(
            triage.context_builder_id,
            "autoscaling-cost-deterministic-context-v1"
        );
        assert_eq!(triage.prompt_template_id, "autoscaling-cost-ai-triage-v1");
        assert_eq!(triage.generation_mode, "deterministic_no_llm");
        assert_eq!(triage.max_prompt_tokens, 1200);
        assert_eq!(
            triage.provider_routing,
            vec!["primary_ops_llm", "fallback_ops_llm"]
        );
        assert_eq!(
            triage.audit_event_type,
            "autoscaling_ai_triage_context_built"
        );
        assert!(triage.guardrails.read_only_mode);
        assert!(triage.guardrails.evidence_required);
        assert!(triage.guardrails.separate_facts_from_hypotheses);
        assert!(triage.guardrails.ask_for_missing_data);
        assert!(triage.guardrails.no_llm_invocation);
        assert!(triage.guardrails.no_mutation_planning);
        assert!(triage
            .facts
            .iter()
            .any(|fact| fact.contains(REASON_COST_MISSING_CAPACITY_TELEMETRY)));
        assert!(triage
            .hypotheses
            .iter()
            .any(|hypothesis| hypothesis.contains("scale-in")));
        assert!(triage
            .hypotheses
            .iter()
            .any(|hypothesis| hypothesis.contains("collector logs")));
        assert!(triage
            .missing_data_questions
            .iter()
            .any(|question| question.contains("capacity telemetry")));
        assert!(triage
            .missing_data_questions
            .iter()
            .any(|question| question.contains("group metrics")));
        assert!(triage
            .missing_data_questions
            .iter()
            .any(|question| question.contains("owner, team, project, or cost-center")));
        assert_eq!(triage.evidence_citations.len(), report.findings.len());
        assert!(triage.evidence_citations.iter().any(|citation| {
            citation.reason_code == REASON_TEL_COLLECTION_ERRORS
                && citation.resource_id == "asg-telemetry-error"
        }));
    }

    #[test]
    fn asg_cost_agentic_investigation_plan_is_read_only_until_approval() {
        let mut fixed_data = healthy_data();
        fixed_data["min_size"] = json!(3);
        fixed_data["max_size"] = json!(3);
        fixed_data["desired_capacity"] = json!(3);
        let fixed = fixture("asg-fixed", json!({"team": "core"}), fixed_data, now());

        let mut missing_data = healthy_data();
        for field in [
            "enabled_metrics",
            "enabled_metric_count",
            "desired_capacity",
        ] {
            missing_data.as_object_mut().expect("object").remove(field);
        }
        let missing = fixture("asg-cost-missing", json!({}), missing_data, now());

        let report = evaluate_autoscaling_fleet(&[fixed, missing], Pillar::Cost, now());
        let plan = asg_cost_agentic_investigation_plan(&report);

        assert_eq!(plan.workflow_id, "autoscaling_cost_agentic_investigation");
        assert_eq!(plan.default_tool_mode, AsgInvestigationToolMode::ReadOnly);
        assert!(plan.replay_required);
        assert!(plan.steps.iter().any(|step| {
            step.tool_name == "autoscaling.describe_group_capacity"
                && step.tool_mode == AsgInvestigationToolMode::ReadOnly
                && step.target_resource_id == "asg-cost-missing"
        }));
        assert!(plan.steps.iter().any(|step| {
            step.tool_name == "autoscaling.describe_enabled_metrics"
                && step.tool_mode == AsgInvestigationToolMode::ReadOnly
                && step.target_resource_id == "asg-cost-missing"
        }));
        assert!(plan.steps.iter().any(|step| {
            step.tool_name == "autoscaling.compare_scaling_policy_capacity"
                && step.tool_mode == AsgInvestigationToolMode::ReadOnly
                && step.target_resource_id == "asg-fixed"
        }));
        assert_eq!(
            plan.steps.last().map(|step| step.tool_mode),
            Some(AsgInvestigationToolMode::ApprovalRequired)
        );
        assert!(plan.steps.iter().all(|step| {
            step.tool_mode == AsgInvestigationToolMode::ReadOnly
                || step.tool_name == "autoscaling.cost.prepare_approval_plan"
        }));
        assert_eq!(plan.approval_gates.len(), 2);
        assert!(plan
            .approval_gates
            .iter()
            .all(|gate| gate.rollback_note_required));
        assert_eq!(plan.max_evidence_citations, report.findings.len());
    }

    #[test]
    fn asg_cost_remediation_workflow_plans_dry_run_actions_until_approved() {
        let mut fixed_data = healthy_data();
        fixed_data["min_size"] = json!(3);
        fixed_data["max_size"] = json!(3);
        fixed_data["desired_capacity"] = json!(3);
        let fixed = fixture("asg-fixed", json!({"team": "core"}), fixed_data, now());

        let mut missing_tags_data = healthy_data();
        missing_tags_data["enabled_metric_count"] = json!(2);
        let missing_tags = fixture("asg-missing-tags", json!({}), missing_tags_data, now());

        let report = evaluate_autoscaling_fleet(&[fixed, missing_tags], Pillar::Cost, now());
        let workflow = asg_cost_remediation_workflow(&report);

        assert_eq!(workflow.workflow_id, "autoscaling_cost_safe_remediation");
        assert!(workflow.read_only_mode);
        assert_eq!(
            workflow.rbac_permission,
            "aws.autoscaling.cost.remediation.approve"
        );
        assert_eq!(workflow.audit_stream, "autoscaling_cost_remediation_audit");
        assert!(!workflow.stale_data_blocks_execution);
        assert_eq!(workflow.actions.len(), 2);
        assert!(workflow.actions.iter().all(|action| {
            action.dry_run
                && action.requires_approval
                && action.approval_gate_id.is_some()
                && action.status == AsgRemediationStatus::DryRunPendingApproval
                && action.audit_event_type == "autoscaling.cost.remediation.dry_run_planned"
                && action.rollback_note.contains("rollback")
        }));
        assert!(workflow.actions.iter().any(|action| {
            action.kind == AsgRemediationActionKind::ReviewCostAllocationTags
                && action.target_resource_id == "asg-missing-tags"
                && action
                    .evidence_reason_codes
                    .contains(&REASON_COST_NO_TAGS.to_string())
        }));
        assert!(workflow.actions.iter().any(|action| {
            action.kind == AsgRemediationActionKind::ReviewScalingPolicyCapacity
                && action.target_resource_id == "asg-fixed"
                && action
                    .evidence_reason_codes
                    .contains(&REASON_COST_FIXED_SIZE.to_string())
        }));
    }

    #[test]
    fn asg_cost_remediation_workflow_blocks_execution_when_cost_data_is_stale() {
        let stale = fixture(
            "asg-stale",
            json!({"team": "core"}),
            healthy_data(),
            now() - chrono::Duration::hours(30),
        );

        let report = evaluate_autoscaling_fleet(&[stale], Pillar::Cost, now());
        let workflow = asg_cost_remediation_workflow(&report);

        assert!(workflow.stale_data_blocks_execution);
        assert!(workflow.actions.iter().all(|action| {
            action.dry_run && action.status == AsgRemediationStatus::BlockedMissingEvidence
        }));
    }

    #[test]
    fn asg_cost_slo_policy_snapshot_tracks_owner_policy_and_notifications() {
        let mut fixed_data = healthy_data();
        fixed_data["min_size"] = json!(3);
        fixed_data["max_size"] = json!(3);
        fixed_data["desired_capacity"] = json!(3);
        let fixed = fixture(
            "asg-fixed",
            json!({
                "owner": "sre",
                "environment": "prod",
                "application": "checkout"
            }),
            fixed_data,
            now(),
        );
        let missing_tags = fixture("asg-missing-tags", json!({}), healthy_data(), now());

        let report = evaluate_autoscaling_fleet(&[fixed, missing_tags], Pillar::Cost, now());
        let snapshot = asg_cost_slo_policy_snapshot(&report);

        assert_eq!(snapshot.workflow_id, "autoscaling_cost_slo_policy");
        assert!(snapshot.read_only_mode);
        assert!(snapshot.freshness_required);
        assert_eq!(
            snapshot.objective.objective_id,
            "autoscaling-cost-score-min-90"
        );
        assert_eq!(snapshot.objective.status, AsgCostObjectiveStatus::AtRisk);
        assert_eq!(snapshot.objective.target_score_min, 90);
        assert_eq!(snapshot.objective.failed_rule_count, 2);
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
            .contains(&REASON_COST_FIXED_SIZE.to_string()));
    }

    #[test]
    fn asg_cost_slo_policy_snapshot_marks_stale_data_as_breached() {
        let stale = fixture(
            "asg-stale",
            json!({"owner": "sre"}),
            healthy_data(),
            now() - chrono::Duration::hours(30),
        );

        let report = evaluate_autoscaling_fleet(&[stale], Pillar::Cost, now());
        let snapshot = asg_cost_slo_policy_snapshot(&report);

        assert_eq!(snapshot.objective.status, AsgCostObjectiveStatus::Breached);
        assert_eq!(
            snapshot.objective.trend_direction,
            AsgCostTrendDirection::Degrading
        );
        assert_eq!(snapshot.objective.policy_state, "blocked_stale_data");
        assert!(snapshot
            .evidence_reason_codes
            .contains(&REASON_INV_STALE_DATA.to_string()));
    }

    #[test]
    fn asg_cost_forecast_snapshot_builds_read_only_cost_band_from_capacity_evidence() {
        let mut fixed_data = healthy_data();
        fixed_data["min_size"] = json!(4);
        fixed_data["max_size"] = json!(4);
        fixed_data["desired_capacity"] = json!(4);
        let fixed = fixture("asg-fixed", json!({"owner": "sre"}), fixed_data, now());
        let missing_metrics = fixture(
            "asg-missing-metrics",
            json!({"owner": "platform"}),
            json!({
                "min_size": 1,
                "max_size": 4,
                "desired_capacity": 2
            }),
            now(),
        );

        let report = evaluate_autoscaling_fleet(&[fixed, missing_metrics], Pillar::Cost, now());
        let forecast = asg_cost_forecast_snapshot(&report);

        assert_eq!(forecast.workflow_id, "autoscaling_cost_forecasting");
        assert!(forecast.read_only_mode);
        assert_eq!(forecast.baseline_window_days, 30);
        assert_eq!(forecast.forecast_horizon_days, 30);
        assert_eq!(forecast.confidence_level, 80);
        assert_eq!(forecast.risk_level, AsgCostForecastRisk::High);
        assert_eq!(
            forecast.capacity_risk,
            "fixed_capacity_and_missing_group_metrics"
        );
        assert_eq!(forecast.forecast_band.horizon_days, 30);
        assert!(forecast.forecast_band.expected_monthly_cost_index > 130);
        assert!(
            forecast.forecast_band.upper_monthly_cost_index
                > forecast.forecast_band.lower_monthly_cost_index
        );
        assert_eq!(
            forecast.backtesting_fixture_status,
            "needs_fresh_capacity_fixture"
        );
        assert!(forecast
            .what_if_inputs
            .contains(&"enable_group_metrics_collection"));
        assert!(forecast.risk_drivers.iter().any(|driver| {
            driver.reason_code == REASON_COST_FIXED_SIZE
                && driver.affected_resources == vec!["asg-fixed"]
        }));
        assert!(forecast.risk_drivers.iter().any(|driver| {
            driver.reason_code == REASON_COST_MISSING_GROUP_METRICS_TELEMETRY
                && driver.affected_resources == vec!["asg-missing-metrics"]
        }));
        assert!(forecast
            .missing_data_reason_codes
            .contains(&REASON_COST_MISSING_GROUP_METRICS_TELEMETRY.to_string()));
    }

    #[test]
    fn asg_cost_forecast_snapshot_blocks_on_stale_or_missing_capacity_data() {
        let stale = fixture(
            "asg-stale-forecast",
            json!({"owner": "sre"}),
            healthy_data(),
            now() - chrono::Duration::hours(30),
        );

        let report = evaluate_autoscaling_fleet(&[stale], Pillar::Cost, now());
        let forecast = asg_cost_forecast_snapshot(&report);

        assert_eq!(forecast.risk_level, AsgCostForecastRisk::Blocked);
        assert!(forecast.blocked_by_stale_data);
        assert_eq!(forecast.capacity_risk, "blocked_until_inventory_refresh");
        assert_eq!(
            forecast.backtesting_fixture_status,
            "needs_fresh_capacity_fixture"
        );
        assert!(forecast
            .missing_data_reason_codes
            .contains(&REASON_INV_STALE_DATA.to_string()));
        assert!(forecast.forecast_band.upper_monthly_cost_index > 100);
    }

    #[test]
    fn asg_cost_reporting_bundle_materializes_executive_engineering_and_incident_views() {
        let mut fixed_data = healthy_data();
        fixed_data["min_size"] = json!(3);
        fixed_data["max_size"] = json!(3);
        fixed_data["desired_capacity"] = json!(3);
        let fixed = fixture(
            "asg-fixed-report",
            json!({"owner": "sre", "environment": "prod"}),
            fixed_data,
            now(),
        );
        let missing_tags = fixture("asg-missing-tags-report", json!({}), healthy_data(), now());

        let report = evaluate_autoscaling_fleet(&[fixed, missing_tags], Pillar::Cost, now());
        let bundle = asg_cost_reporting_bundle(&report);

        assert_eq!(bundle.workflow_id, "autoscaling_cost_reporting");
        assert!(bundle.read_only_mode);
        assert_eq!(bundle.scheduled_delivery_state, "ready_for_schedule");
        assert!(bundle.portfolio_summary_ready);
        assert!(bundle.workload_summary_ready);
        assert_eq!(bundle.export_formats, vec!["json", "csv"]);
        assert_eq!(
            bundle.executive_summary.report_id,
            "autoscaling-cost-executive-summary"
        );
        assert_eq!(bundle.executive_summary.score, report.score);
        assert_eq!(bundle.executive_summary.resources_evaluated, 2);
        assert_eq!(bundle.executive_summary.stale_resources, 0);
        assert!(bundle
            .executive_summary
            .affected_resources
            .contains(&"asg-fixed-report".to_string()));
        assert!(bundle
            .executive_summary
            .top_reason_codes
            .contains(&REASON_COST_FIXED_SIZE.to_string()));
        assert_eq!(
            bundle.engineering_backlog.report_id,
            "autoscaling-cost-engineering-backlog"
        );
        assert_eq!(bundle.engineering_backlog.page, 0);
        assert_eq!(bundle.engineering_backlog.page_size, 50);
        assert_eq!(bundle.engineering_backlog.total, report.findings.len());
        assert_eq!(
            bundle.incident_review.report_id,
            "autoscaling-cost-incident-review"
        );
        assert!(bundle.incident_review.rows.iter().any(|row| {
            row.resource_id == "asg-fixed-report"
                && row.reason_code == REASON_COST_FIXED_SIZE
                && row.suppression_supported
                && row.recovery_note.contains("Review scaling policy")
        }));
    }

    #[test]
    fn asg_cost_reporting_bundle_blocks_delivery_for_stale_inventory() {
        let stale = fixture(
            "asg-stale-report",
            json!({"owner": "sre"}),
            healthy_data(),
            now() - chrono::Duration::hours(30),
        );

        let report = evaluate_autoscaling_fleet(&[stale], Pillar::Cost, now());
        let bundle = asg_cost_reporting_bundle(&report);

        assert_eq!(
            bundle.scheduled_delivery_state,
            "blocked_until_fresh_inventory"
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
    fn resilience_flags_missing_replacement_telemetry() {
        let mut data = healthy_data();
        data.as_object_mut()
            .expect("object")
            .remove("health_check_type");
        let r = fixture(
            "asg-no-health-telemetry",
            json!({"team": "core"}),
            data,
            now(),
        );

        let report = evaluate_autoscaling_fleet(&[r], Pillar::Resilience, now());
        assert!(
            codes(&report).contains(&"ASG_RES_MISSING_REPLACEMENT_TELEMETRY"),
            "expected replacement telemetry gap: {:?}",
            report.findings
        );
    }
}

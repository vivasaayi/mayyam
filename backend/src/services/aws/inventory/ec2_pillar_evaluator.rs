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

// Deterministic EC2 inventory and telemetry evaluators for the cost, security,
// resilience, performance, scalability, and disaster-recovery pillars (roadmap
// rows 01-AWS-CLOUD-00001/00010/00037 plus
// 01-AWS-CLOUD-00002/00011/00020/00029/00038/00047/00056).
//
// Pure domain logic: takes already-collected `aws_resources` rows plus an
// explicit `now`, returns reason-coded findings with the raw evidence that
// triggered each finding. No AWS calls, no database access, no LLM.

use chrono::{DateTime, Utc};
use serde::Serialize;
use serde_json::{json, Value};
use std::collections::BTreeSet;

use crate::models::aws_resource::Model as AwsResourceModel;
use crate::services::aws::inventory::types::{
    check_stale, data_str, has_any_tag, score_pillar, InventoryFinding, Pillar, PillarReport,
    Severity, COST_ALLOCATION_TAG_KEYS, OWNER_TAG_KEYS,
};

// Reason codes are the stable contract for findings; never reuse or rename.
pub const REASON_COST_MISSING_ALLOCATION_TAGS: &str = "EC2_COST_MISSING_ALLOCATION_TAGS";
pub const REASON_COST_STOPPED_INSTANCE: &str = "EC2_COST_STOPPED_INSTANCE_ACCRUING_STORAGE";
pub const REASON_SEC_PUBLIC_IP: &str = "EC2_SEC_PUBLIC_IP_ASSIGNED";
pub const REASON_SEC_MISSING_OWNER_TAG: &str = "EC2_SEC_MISSING_OWNER_TAG";
pub const REASON_RES_MISSING_AZ: &str = "EC2_RES_MISSING_AVAILABILITY_ZONE";
pub const REASON_RES_SINGLE_AZ_CONCENTRATION: &str = "EC2_RES_SINGLE_AZ_CONCENTRATION";
pub const REASON_COST_MISSING_UTILIZATION_TELEMETRY: &str =
    "EC2_COST_MISSING_UTILIZATION_TELEMETRY";
pub const REASON_COST_LOW_UTILIZATION_TELEMETRY: &str = "EC2_COST_LOW_UTILIZATION_TELEMETRY";
pub const REASON_RES_MISSING_STATUS_TELEMETRY: &str = "EC2_RES_MISSING_STATUS_TELEMETRY";
pub const REASON_RES_STATUS_CHECK_FAILURE_TELEMETRY: &str =
    "EC2_RES_STATUS_CHECK_FAILURE_TELEMETRY";
pub const REASON_PERF_MISSING_CORE_TELEMETRY: &str = "EC2_PERF_MISSING_CORE_TELEMETRY";
pub const REASON_PERF_HIGH_CPU_TELEMETRY: &str = "EC2_PERF_HIGH_CPU_TELEMETRY";
pub const REASON_SCALE_MISSING_DEMAND_TELEMETRY: &str = "EC2_SCALE_MISSING_DEMAND_TELEMETRY";
pub const REASON_SCALE_HIGH_CPU_PRESSURE_TELEMETRY: &str = "EC2_SCALE_HIGH_CPU_PRESSURE_TELEMETRY";
pub const REASON_SEC_MISSING_PACKET_TELEMETRY: &str = "EC2_SEC_MISSING_PACKET_TELEMETRY";
pub const REASON_SEC_PUBLIC_PACKET_TRAFFIC_TELEMETRY: &str =
    "EC2_SEC_PUBLIC_PACKET_TRAFFIC_TELEMETRY";
pub const REASON_DR_MISSING_RECOVERY_POINT_TELEMETRY: &str =
    "EC2_DR_MISSING_RECOVERY_POINT_TELEMETRY";
pub const REASON_DR_STALE_RECOVERY_POINT_TELEMETRY: &str = "EC2_DR_STALE_RECOVERY_POINT_TELEMETRY";
pub const REASON_OE_MISSING_TELEMETRY_COLLECTION_METADATA: &str =
    "EC2_OE_MISSING_TELEMETRY_COLLECTION_METADATA";
pub const REASON_OE_TELEMETRY_COLLECTION_ERRORS: &str = "EC2_OE_TELEMETRY_COLLECTION_ERRORS";
pub const REASON_OE_BASIC_MONITORING: &str = "EC2_OE_BASIC_MONITORING";
pub const REASON_INV_STALE_DATA: &str = "EC2_INV_STALE_DATA";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Ec2PostureStatus {
    Pass,
    Fail,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Ec2PostureRule {
    pub rule_id: &'static str,
    pub status: Ec2PostureStatus,
    pub reason_codes: Vec<&'static str>,
    pub affected_resources: Vec<String>,
    pub suppression_supported: bool,
    pub assignment_supported: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Ec2PostureSummary {
    pub status: Ec2PostureStatus,
    pub rules_evaluated: usize,
    pub rules_failed: usize,
    pub affected_resources: Vec<String>,
    pub rules: Vec<Ec2PostureRule>,
}

pub type Ec2CostPostureSummary = Ec2PostureSummary;
pub type Ec2ResiliencePostureSummary = Ec2PostureSummary;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Ec2EvidenceCitation {
    pub reason_code: String,
    pub resource_id: String,
    pub severity: Severity,
    pub evidence: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Ec2CostTriageContext {
    pub pillar: Pillar,
    pub facts: Vec<String>,
    pub hypotheses: Vec<String>,
    pub missing_data_questions: Vec<String>,
    pub evidence_citations: Vec<Ec2EvidenceCitation>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Ec2InvestigationStepKind {
    Inspect,
    Compare,
    Diagnose,
    ProposeMutationPlan,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Ec2InvestigationToolMode {
    ReadOnly,
    ApprovalRequired,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Ec2InvestigationStep {
    pub step_id: String,
    pub kind: Ec2InvestigationStepKind,
    pub tool_name: &'static str,
    pub tool_mode: Ec2InvestigationToolMode,
    pub target_resource_id: String,
    pub reason_code: String,
    pub stop_condition: String,
    pub evidence: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Ec2MutationApprovalGate {
    pub gate_id: String,
    pub target_resource_id: String,
    pub required_approval: &'static str,
    pub blast_radius: String,
    pub rollback_note_required: bool,
    pub evidence_reason_codes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Ec2CostAgenticInvestigationPlan {
    pub workflow_id: &'static str,
    pub default_tool_mode: Ec2InvestigationToolMode,
    pub max_tool_calls: usize,
    pub max_evidence_citations: usize,
    pub replay_required: bool,
    pub steps: Vec<Ec2InvestigationStep>,
    pub approval_gates: Vec<Ec2MutationApprovalGate>,
    pub evidence_citations: Vec<Ec2EvidenceCitation>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Ec2RemediationActionKind {
    AssignCostTags,
    ReviewStoppedInstanceArtifacts,
    RightSizeInstance,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Ec2RemediationStatus {
    DryRunPendingApproval,
    BlockedMissingEvidence,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Ec2CostRemediationAction {
    pub action_id: String,
    pub kind: Ec2RemediationActionKind,
    pub status: Ec2RemediationStatus,
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
pub struct Ec2CostRemediationWorkflow {
    pub workflow_id: &'static str,
    pub read_only_mode: bool,
    pub rbac_permission: &'static str,
    pub audit_stream: &'static str,
    pub stale_data_blocks_execution: bool,
    pub actions: Vec<Ec2CostRemediationAction>,
    pub approval_gates: Vec<Ec2MutationApprovalGate>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Ec2CostObjectiveStatus {
    OnTrack,
    AtRisk,
    Breached,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Ec2CostTrendDirection {
    Stable,
    Degrading,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Ec2CostPolicyObjective {
    pub objective_id: &'static str,
    pub status: Ec2CostObjectiveStatus,
    pub target_score_min: u8,
    pub current_score: u8,
    pub trend_direction: Ec2CostTrendDirection,
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
pub struct Ec2CostSloPolicySnapshot {
    pub workflow_id: &'static str,
    pub read_only_mode: bool,
    pub freshness_required: bool,
    pub objective: Ec2CostPolicyObjective,
    pub evidence_reason_codes: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Ec2CostForecastRisk {
    Low,
    Moderate,
    High,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Ec2CostForecastBand {
    pub horizon_days: u16,
    pub lower_monthly_cost_index: u16,
    pub expected_monthly_cost_index: u16,
    pub upper_monthly_cost_index: u16,
    pub confidence_level: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Ec2CostForecastRiskDriver {
    pub reason_code: String,
    pub affected_resources: Vec<String>,
    pub monthly_cost_index_delta: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Ec2CostForecastSnapshot {
    pub workflow_id: &'static str,
    pub read_only_mode: bool,
    pub baseline_window_days: u16,
    pub forecast_horizon_days: u16,
    pub confidence_level: u8,
    pub forecast_band: Ec2CostForecastBand,
    pub risk_level: Ec2CostForecastRisk,
    pub capacity_risk: &'static str,
    pub backtesting_fixture_status: &'static str,
    pub threshold_controls: Vec<&'static str>,
    pub what_if_inputs: Vec<&'static str>,
    pub blocked_by_stale_data: bool,
    pub missing_data_reason_codes: Vec<String>,
    pub risk_drivers: Vec<Ec2CostForecastRiskDriver>,
    pub evidence_reason_codes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Ec2CostExecutiveSummary {
    pub report_id: &'static str,
    pub score: u8,
    pub resources_evaluated: usize,
    pub stale_resources: usize,
    pub rules_failed: usize,
    pub affected_resources: Vec<String>,
    pub top_reason_codes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Ec2CostReportRow {
    pub resource_id: String,
    pub severity: Severity,
    pub reason_code: String,
    pub message: String,
    pub evidence: Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Ec2CostEngineeringBacklog {
    pub report_id: &'static str,
    pub page: u16,
    pub page_size: u16,
    pub total: usize,
    pub rows: Vec<Ec2CostReportRow>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Ec2CostReportingBundle {
    pub workflow_id: &'static str,
    pub read_only_mode: bool,
    pub scheduled_delivery_state: &'static str,
    pub executive_summary: Ec2CostExecutiveSummary,
    pub engineering_backlog: Ec2CostEngineeringBacklog,
    pub evidence_reason_codes: Vec<String>,
}

/// Evaluate every EC2 instance in the fleet for one pillar.
pub fn evaluate_ec2_fleet(
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
            Pillar::DisasterRecovery => evaluate_disaster_recovery(resource, now, &mut findings),
            Pillar::OperationalExcellence => {
                evaluate_operational_excellence(resource, &mut findings)
            }
        }
    }

    if pillar == Pillar::Resilience {
        if let Some(finding) = check_az_concentration(resources) {
            findings.push(finding);
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

pub fn ec2_cost_posture_summary(report: &PillarReport) -> Ec2CostPostureSummary {
    let rules = vec![
        ec2_cost_posture_rule(
            report,
            "ec2-cost-allocation-tags",
            &[REASON_COST_MISSING_ALLOCATION_TAGS],
        ),
        ec2_cost_posture_rule(
            report,
            "ec2-stopped-capacity-review",
            &[REASON_COST_STOPPED_INSTANCE],
        ),
        ec2_cost_posture_rule(
            report,
            "ec2-utilization-telemetry-coverage",
            &[REASON_COST_MISSING_UTILIZATION_TELEMETRY],
        ),
        ec2_cost_posture_rule(
            report,
            "ec2-low-utilization-rightsizing",
            &[REASON_COST_LOW_UTILIZATION_TELEMETRY],
        ),
    ];
    let affected_resources = sorted_unique_resources(
        rules
            .iter()
            .flat_map(|rule| rule.affected_resources.iter().cloned()),
    );
    let rules_failed = rules
        .iter()
        .filter(|rule| rule.status == Ec2PostureStatus::Fail)
        .count();

    Ec2PostureSummary {
        status: if rules_failed == 0 {
            Ec2PostureStatus::Pass
        } else {
            Ec2PostureStatus::Fail
        },
        rules_evaluated: rules.len(),
        rules_failed,
        affected_resources,
        rules,
    }
}

pub fn ec2_resilience_posture_summary(report: &PillarReport) -> Ec2ResiliencePostureSummary {
    let rules = vec![
        ec2_resilience_posture_rule(
            report,
            "ec2-resilience-availability-zone-recorded",
            &[REASON_RES_MISSING_AZ],
        ),
        ec2_resilience_posture_rule(
            report,
            "ec2-resilience-multi-az-placement",
            &[REASON_RES_SINGLE_AZ_CONCENTRATION],
        ),
        ec2_resilience_posture_rule(
            report,
            "ec2-resilience-status-check-telemetry",
            &[REASON_RES_MISSING_STATUS_TELEMETRY],
        ),
        ec2_resilience_posture_rule(
            report,
            "ec2-resilience-status-check-health",
            &[REASON_RES_STATUS_CHECK_FAILURE_TELEMETRY],
        ),
    ];
    let affected_resources = sorted_unique_resources(
        rules
            .iter()
            .flat_map(|rule| rule.affected_resources.iter().cloned()),
    );
    let rules_failed = rules
        .iter()
        .filter(|rule| rule.status == Ec2PostureStatus::Fail)
        .count();

    Ec2PostureSummary {
        status: if rules_failed == 0 {
            Ec2PostureStatus::Pass
        } else {
            Ec2PostureStatus::Fail
        },
        rules_evaluated: rules.len(),
        rules_failed,
        affected_resources,
        rules,
    }
}

pub fn ec2_cost_triage_context(report: &PillarReport) -> Ec2CostTriageContext {
    let mut facts = Vec::new();
    let mut hypotheses = Vec::new();
    let mut missing_data_questions = Vec::new();
    let mut evidence_citations = Vec::new();

    for finding in &report.findings {
        facts.push(format!(
            "{} affects {} with {:?} severity",
            finding.reason_code, finding.resource_id, finding.severity
        ));
        evidence_citations.push(Ec2EvidenceCitation {
            reason_code: finding.reason_code.clone(),
            resource_id: finding.resource_id.clone(),
            severity: finding.severity,
            evidence: finding.evidence.clone(),
        });

        match finding.reason_code.as_str() {
            REASON_COST_LOW_UTILIZATION_TELEMETRY => hypotheses.push(format!(
                "{} may be oversized or eligible for scheduling; validate business hours, recent deployments, and reservation coverage before rightsizing",
                finding.resource_id
            )),
            REASON_COST_STOPPED_INSTANCE => hypotheses.push(format!(
                "{} may be abandoned capacity; verify attached EBS volumes, elastic IPs, and recovery expectations before cleanup",
                finding.resource_id
            )),
            REASON_COST_MISSING_UTILIZATION_TELEMETRY => missing_data_questions.push(format!(
                "Collect CPUUtilization telemetry for {} before distinguishing idle capacity from active workload demand",
                finding.resource_id
            )),
            REASON_COST_MISSING_ALLOCATION_TAGS => missing_data_questions.push(format!(
                "Assign owner, team, project, or cost-center metadata for {} so savings can be routed",
                finding.resource_id
            )),
            _ => {}
        }
    }

    Ec2CostTriageContext {
        pillar: report.pillar,
        facts,
        hypotheses,
        missing_data_questions,
        evidence_citations,
    }
}

pub fn ec2_cost_agentic_investigation_plan(
    report: &PillarReport,
) -> Ec2CostAgenticInvestigationPlan {
    let triage = ec2_cost_triage_context(report);
    let mut steps = Vec::new();
    let mut approval_gates = Vec::new();

    for citation in &triage.evidence_citations {
        if citation.resource_id == "fleet" {
            continue;
        }

        match citation.reason_code.as_str() {
            REASON_COST_MISSING_UTILIZATION_TELEMETRY => {
                steps.push(investigation_step(
                    &steps,
                    Ec2InvestigationStepKind::Inspect,
                    "ec2.cloudwatch.get_metric_data",
                    Ec2InvestigationToolMode::ReadOnly,
                    citation,
                    "stop when CPUUtilization is found for the lookback window or the metric is confirmed absent",
                ));
            }
            REASON_COST_LOW_UTILIZATION_TELEMETRY => {
                steps.push(investigation_step(
                    &steps,
                    Ec2InvestigationStepKind::Compare,
                    "ec2.compute_optimizer.get_instance_recommendations",
                    Ec2InvestigationToolMode::ReadOnly,
                    citation,
                    "stop when rightsizing evidence, reservation coverage, or a conflicting utilization signal is found",
                ));
                approval_gates.push(mutation_gate(
                    &approval_gates,
                    citation,
                    "Approve any resize, stop schedule, or purchase-plan change after owner review",
                ));
            }
            REASON_COST_STOPPED_INSTANCE => {
                steps.push(investigation_step(
                    &steps,
                    Ec2InvestigationStepKind::Diagnose,
                    "ec2.describe_attached_cost_artifacts",
                    Ec2InvestigationToolMode::ReadOnly,
                    citation,
                    "stop when attached EBS volumes, elastic IPs, and recovery expectations are recorded",
                ));
                approval_gates.push(mutation_gate(
                    &approval_gates,
                    citation,
                    "Approve terminate, snapshot, detach, or release actions after rollback notes are captured",
                ));
            }
            REASON_COST_MISSING_ALLOCATION_TAGS => {
                steps.push(investigation_step(
                    &steps,
                    Ec2InvestigationStepKind::Diagnose,
                    "ec2.resource_groups.get_tagging_context",
                    Ec2InvestigationToolMode::ReadOnly,
                    citation,
                    "stop when owner, team, project, or cost-center can be inferred or the gap is assigned",
                ));
                approval_gates.push(mutation_gate(
                    &approval_gates,
                    citation,
                    "Approve tag writes after ownership is verified",
                ));
            }
            _ => {}
        }
    }

    steps.push(Ec2InvestigationStep {
        step_id: format!("ec2-cost-step-{:02}", steps.len() + 1),
        kind: Ec2InvestigationStepKind::ProposeMutationPlan,
        tool_name: "ec2.cost.prepare_approval_plan",
        tool_mode: Ec2InvestigationToolMode::ApprovalRequired,
        target_resource_id: "investigation".to_string(),
        reason_code: "EC2_COST_APPROVAL_PLAN_REQUIRED".to_string(),
        stop_condition:
            "stop before mutation; require explicit operator approval, blast-radius summary, and rollback note"
                .to_string(),
        evidence: json!({
            "approval_gate_count": approval_gates.len(),
            "read_only_step_count": steps.len(),
        }),
    });

    Ec2CostAgenticInvestigationPlan {
        workflow_id: "ec2_cost_agentic_investigation",
        default_tool_mode: Ec2InvestigationToolMode::ReadOnly,
        max_tool_calls: steps.len().min(12),
        max_evidence_citations: triage.evidence_citations.len(),
        replay_required: true,
        steps,
        approval_gates,
        evidence_citations: triage.evidence_citations,
    }
}

pub fn ec2_cost_slo_policy_snapshot(report: &PillarReport) -> Ec2CostSloPolicySnapshot {
    let posture = ec2_cost_posture_summary(report);
    let failed_rule_count = posture.rules_failed;
    let affected_resource_count = posture.affected_resources.len();
    let status = cost_objective_status(report.score, failed_rule_count, report.stale_resources);
    let owner_filters = sorted_unique_evidence_values(report, &["owner", "team"]);
    let environment_filters = sorted_unique_evidence_values(report, &["environment", "env"]);
    let application_filters = sorted_unique_evidence_values(report, &["application", "app"]);
    let notification_targets = notification_targets(&owner_filters, &environment_filters);

    Ec2CostSloPolicySnapshot {
        workflow_id: "ec2_cost_slo_policy",
        read_only_mode: true,
        freshness_required: true,
        objective: Ec2CostPolicyObjective {
            objective_id: "ec2-cost-score-min-90",
            status,
            target_score_min: 90,
            current_score: report.score,
            trend_direction: cost_trend_direction(status, failed_rule_count),
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

pub fn ec2_cost_forecast_snapshot(report: &PillarReport) -> Ec2CostForecastSnapshot {
    const BASELINE_WINDOW_DAYS: u16 = 30;
    const FORECAST_HORIZON_DAYS: u16 = 30;
    const CONFIDENCE_LEVEL: u8 = 80;

    let stale_data = report.stale_resources > 0
        || report
            .findings
            .iter()
            .any(|finding| finding.reason_code == REASON_INV_STALE_DATA);
    let low_utilization_count = count_reason(report, REASON_COST_LOW_UTILIZATION_TELEMETRY);
    let stopped_count = count_reason(report, REASON_COST_STOPPED_INSTANCE);
    let missing_telemetry_count = count_reason(report, REASON_COST_MISSING_UTILIZATION_TELEMETRY);
    let missing_tag_count = count_reason(report, REASON_COST_MISSING_ALLOCATION_TAGS);

    let expected_monthly_cost_index = 100u16
        + (low_utilization_count as u16 * 18)
        + (stopped_count as u16 * 10)
        + (missing_telemetry_count as u16 * 15)
        + (missing_tag_count as u16 * 4)
        + (report.stale_resources as u16 * 25);
    let uncertainty = 8u16
        + (missing_telemetry_count as u16 * 6)
        + (report.stale_resources as u16 * 10)
        + (report.resources_evaluated == 0) as u16 * 20;
    let lower_monthly_cost_index = expected_monthly_cost_index.saturating_sub(uncertainty);
    let upper_monthly_cost_index = expected_monthly_cost_index + uncertainty;
    let risk_level = if stale_data {
        Ec2CostForecastRisk::Blocked
    } else if upper_monthly_cost_index >= 145 {
        Ec2CostForecastRisk::High
    } else if expected_monthly_cost_index > 100 {
        Ec2CostForecastRisk::Moderate
    } else {
        Ec2CostForecastRisk::Low
    };

    Ec2CostForecastSnapshot {
        workflow_id: "ec2_cost_forecasting",
        read_only_mode: true,
        baseline_window_days: BASELINE_WINDOW_DAYS,
        forecast_horizon_days: FORECAST_HORIZON_DAYS,
        confidence_level: CONFIDENCE_LEVEL,
        forecast_band: Ec2CostForecastBand {
            horizon_days: FORECAST_HORIZON_DAYS,
            lower_monthly_cost_index,
            expected_monthly_cost_index,
            upper_monthly_cost_index,
            confidence_level: CONFIDENCE_LEVEL,
        },
        risk_level,
        capacity_risk: ec2_cost_capacity_risk(
            stale_data,
            low_utilization_count,
            stopped_count,
            missing_telemetry_count,
        ),
        backtesting_fixture_status: if report.findings.is_empty() {
            "ready_clean_baseline"
        } else if stale_data || missing_telemetry_count > 0 {
            "needs_fresh_telemetry_fixture"
        } else {
            "ready_findings_baseline"
        },
        threshold_controls: vec![
            "monthly_cost_index_warning_threshold",
            "monthly_cost_index_critical_threshold",
        ],
        what_if_inputs: vec![
            "rightsize_low_utilization_instances",
            "release_stopped_instance_artifacts",
            "restore_missing_utilization_telemetry",
        ],
        blocked_by_stale_data: stale_data,
        missing_data_reason_codes: missing_data_reason_codes(report),
        risk_drivers: ec2_cost_forecast_risk_drivers(report),
        evidence_reason_codes: sorted_unique_reason_codes(report),
    }
}

pub fn ec2_cost_reporting_bundle(report: &PillarReport) -> Ec2CostReportingBundle {
    let posture = ec2_cost_posture_summary(report);
    let reason_codes = sorted_unique_reason_codes(report);
    let rows = ec2_cost_report_rows(report);

    Ec2CostReportingBundle {
        workflow_id: "ec2_cost_reporting",
        read_only_mode: true,
        scheduled_delivery_state: if report.stale_resources > 0 {
            "blocked_until_fresh_inventory"
        } else {
            "ready_for_schedule"
        },
        executive_summary: Ec2CostExecutiveSummary {
            report_id: "ec2-cost-executive-summary",
            score: report.score,
            resources_evaluated: report.resources_evaluated,
            stale_resources: report.stale_resources,
            rules_failed: posture.rules_failed,
            affected_resources: posture.affected_resources,
            top_reason_codes: reason_codes.clone(),
        },
        engineering_backlog: Ec2CostEngineeringBacklog {
            report_id: "ec2-cost-engineering-backlog",
            page: 0,
            page_size: 50,
            total: rows.len(),
            rows,
        },
        evidence_reason_codes: reason_codes,
    }
}

fn ec2_cost_report_rows(report: &PillarReport) -> Vec<Ec2CostReportRow> {
    report
        .findings
        .iter()
        .map(|finding| Ec2CostReportRow {
            resource_id: finding.resource_id.clone(),
            severity: finding.severity,
            reason_code: finding.reason_code.clone(),
            message: finding.message.clone(),
            evidence: finding.evidence.clone(),
        })
        .collect()
}

fn count_reason(report: &PillarReport, reason_code: &str) -> usize {
    report
        .findings
        .iter()
        .filter(|finding| finding.reason_code == reason_code)
        .count()
}

fn ec2_cost_capacity_risk(
    stale_data: bool,
    low_utilization_count: usize,
    stopped_count: usize,
    missing_telemetry_count: usize,
) -> &'static str {
    if stale_data {
        "blocked_until_inventory_refresh"
    } else if missing_telemetry_count > 0 {
        "unknown_due_to_missing_utilization"
    } else if low_utilization_count > 0 {
        "overprovisioned_compute_capacity"
    } else if stopped_count > 0 {
        "stopped_capacity_storage_artifacts"
    } else {
        "within_observed_cost_baseline"
    }
}

fn missing_data_reason_codes(report: &PillarReport) -> Vec<String> {
    report
        .findings
        .iter()
        .filter(|finding| {
            matches!(
                finding.reason_code.as_str(),
                REASON_INV_STALE_DATA | REASON_COST_MISSING_UTILIZATION_TELEMETRY
            )
        })
        .map(|finding| finding.reason_code.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn ec2_cost_forecast_risk_drivers(report: &PillarReport) -> Vec<Ec2CostForecastRiskDriver> {
    [
        (REASON_INV_STALE_DATA, 25u16),
        (REASON_COST_LOW_UTILIZATION_TELEMETRY, 18u16),
        (REASON_COST_MISSING_UTILIZATION_TELEMETRY, 15u16),
        (REASON_COST_STOPPED_INSTANCE, 10u16),
        (REASON_COST_MISSING_ALLOCATION_TAGS, 4u16),
    ]
    .into_iter()
    .filter_map(|(reason_code, delta)| {
        let affected_resources = resources_for_reason(report, reason_code);
        if affected_resources.is_empty() {
            None
        } else {
            Some(Ec2CostForecastRiskDriver {
                reason_code: reason_code.to_string(),
                monthly_cost_index_delta: delta * affected_resources.len() as u16,
                affected_resources,
            })
        }
    })
    .collect()
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

fn cost_objective_status(
    score: u8,
    failed_rule_count: usize,
    stale_resources: usize,
) -> Ec2CostObjectiveStatus {
    if stale_resources > 0 || score < 70 {
        Ec2CostObjectiveStatus::Breached
    } else if failed_rule_count > 0 || score < 90 {
        Ec2CostObjectiveStatus::AtRisk
    } else {
        Ec2CostObjectiveStatus::OnTrack
    }
}

fn cost_trend_direction(
    status: Ec2CostObjectiveStatus,
    failed_rule_count: usize,
) -> Ec2CostTrendDirection {
    match status {
        Ec2CostObjectiveStatus::OnTrack => Ec2CostTrendDirection::Stable,
        Ec2CostObjectiveStatus::AtRisk if failed_rule_count <= 1 => Ec2CostTrendDirection::Stable,
        Ec2CostObjectiveStatus::AtRisk | Ec2CostObjectiveStatus::Breached => {
            Ec2CostTrendDirection::Degrading
        }
    }
}

fn sorted_unique_evidence_values(report: &PillarReport, keys: &[&str]) -> Vec<String> {
    report
        .findings
        .iter()
        .flat_map(|finding| {
            keys.iter()
                .filter_map(|key| evidence_string(&finding.evidence, key))
        })
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn evidence_string(evidence: &Value, key: &str) -> Option<String> {
    evidence
        .get("tags")
        .and_then(|tags| tags.get(key))
        .or_else(|| evidence.get(key))
        .and_then(Value::as_str)
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

fn sorted_unique_reason_codes(report: &PillarReport) -> Vec<String> {
    report
        .findings
        .iter()
        .map(|finding| finding.reason_code.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

pub fn ec2_cost_remediation_workflow(report: &PillarReport) -> Ec2CostRemediationWorkflow {
    let investigation = ec2_cost_agentic_investigation_plan(report);
    let has_stale_data = report
        .findings
        .iter()
        .any(|finding| finding.reason_code == REASON_INV_STALE_DATA);
    let mut actions = Vec::new();

    for gate in &investigation.approval_gates {
        for reason_code in &gate.evidence_reason_codes {
            if let Some(kind) = remediation_action_kind(reason_code) {
                actions.push(remediation_action(
                    &actions,
                    kind,
                    gate,
                    if has_stale_data {
                        Ec2RemediationStatus::BlockedMissingEvidence
                    } else {
                        Ec2RemediationStatus::DryRunPendingApproval
                    },
                ));
            }
        }
    }

    Ec2CostRemediationWorkflow {
        workflow_id: "ec2_cost_safe_remediation",
        read_only_mode: true,
        rbac_permission: "aws.ec2.cost.remediation.approve",
        audit_stream: "ec2_cost_remediation_audit",
        stale_data_blocks_execution: has_stale_data,
        actions,
        approval_gates: investigation.approval_gates,
    }
}

fn remediation_action_kind(reason_code: &str) -> Option<Ec2RemediationActionKind> {
    match reason_code {
        REASON_COST_MISSING_ALLOCATION_TAGS => Some(Ec2RemediationActionKind::AssignCostTags),
        REASON_COST_STOPPED_INSTANCE => {
            Some(Ec2RemediationActionKind::ReviewStoppedInstanceArtifacts)
        }
        REASON_COST_LOW_UTILIZATION_TELEMETRY => Some(Ec2RemediationActionKind::RightSizeInstance),
        _ => None,
    }
}

fn remediation_action(
    existing_actions: &[Ec2CostRemediationAction],
    kind: Ec2RemediationActionKind,
    gate: &Ec2MutationApprovalGate,
    status: Ec2RemediationStatus,
) -> Ec2CostRemediationAction {
    let action_number = existing_actions.len() + 1;
    let action_slug = match kind {
        Ec2RemediationActionKind::AssignCostTags => "assign-cost-tags",
        Ec2RemediationActionKind::ReviewStoppedInstanceArtifacts => {
            "review-stopped-instance-artifacts"
        }
        Ec2RemediationActionKind::RightSizeInstance => "rightsize-instance",
    };

    Ec2CostRemediationAction {
        action_id: format!("ec2-cost-remediation-{:02}", action_number),
        kind,
        status,
        target_resource_id: gate.target_resource_id.clone(),
        dry_run: true,
        requires_approval: true,
        approval_gate_id: Some(gate.gate_id.clone()),
        audit_event_type: "ec2.cost.remediation.dry_run_planned",
        idempotency_key: format!("{}:{}", gate.target_resource_id, action_slug),
        blast_radius: gate.blast_radius.clone(),
        rollback_note: format!(
            "Before approval, record rollback or recovery notes for {} on {}.",
            action_slug, gate.target_resource_id
        ),
        validation_steps: vec![
            "refresh EC2 inventory and cost telemetry",
            "verify resource ownership and suppression policy",
            "capture operator approval and audit id before execution",
        ],
        evidence_reason_codes: gate.evidence_reason_codes.clone(),
    }
}

fn investigation_step(
    existing_steps: &[Ec2InvestigationStep],
    kind: Ec2InvestigationStepKind,
    tool_name: &'static str,
    tool_mode: Ec2InvestigationToolMode,
    citation: &Ec2EvidenceCitation,
    stop_condition: &str,
) -> Ec2InvestigationStep {
    Ec2InvestigationStep {
        step_id: format!("ec2-cost-step-{:02}", existing_steps.len() + 1),
        kind,
        tool_name,
        tool_mode,
        target_resource_id: citation.resource_id.clone(),
        reason_code: citation.reason_code.clone(),
        stop_condition: stop_condition.to_string(),
        evidence: citation.evidence.clone(),
    }
}

fn mutation_gate(
    existing_gates: &[Ec2MutationApprovalGate],
    citation: &Ec2EvidenceCitation,
    required_approval: &'static str,
) -> Ec2MutationApprovalGate {
    Ec2MutationApprovalGate {
        gate_id: format!("ec2-cost-approval-{:02}", existing_gates.len() + 1),
        target_resource_id: citation.resource_id.clone(),
        required_approval,
        blast_radius: format!(
            "single EC2 instance {}; no mutation is executable from the investigation plan",
            citation.resource_id
        ),
        rollback_note_required: true,
        evidence_reason_codes: vec![citation.reason_code.clone()],
    }
}

fn ec2_cost_posture_rule(
    report: &PillarReport,
    rule_id: &'static str,
    reason_codes: &[&'static str],
) -> Ec2PostureRule {
    ec2_posture_rule(report, rule_id, reason_codes)
}

fn ec2_resilience_posture_rule(
    report: &PillarReport,
    rule_id: &'static str,
    reason_codes: &[&'static str],
) -> Ec2PostureRule {
    ec2_posture_rule(report, rule_id, reason_codes)
}

fn ec2_posture_rule(
    report: &PillarReport,
    rule_id: &'static str,
    reason_codes: &[&'static str],
) -> Ec2PostureRule {
    let affected_resources = sorted_unique_resources(
        report
            .findings
            .iter()
            .filter(|finding| reason_codes.contains(&finding.reason_code.as_str()))
            .map(|finding| finding.resource_id.clone()),
    );

    Ec2PostureRule {
        rule_id,
        status: if affected_resources.is_empty() {
            Ec2PostureStatus::Pass
        } else {
            Ec2PostureStatus::Fail
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

fn evaluate_cost(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    if !has_any_tag(&resource.tags, COST_ALLOCATION_TAG_KEYS) {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Cost,
            reason_code: REASON_COST_MISSING_ALLOCATION_TAGS.to_string(),
            severity: Severity::Medium,
            message: format!(
                "Instance {} has no cost allocation tag (expected one of: {})",
                resource.resource_id,
                COST_ALLOCATION_TAG_KEYS.join(", ")
            ),
            evidence: json!({ "tags": resource.tags }),
        });
    }

    let state = data_str(&resource.resource_data, "state");
    if state.as_deref() == Some("stopped") {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Cost,
            reason_code: REASON_COST_STOPPED_INSTANCE.to_string(),
            severity: Severity::Low,
            message: format!(
                "Instance {} is stopped but still accrues EBS and IP charges; review for termination or snapshot",
                resource.resource_id
            ),
            evidence: json!({
                "state": state,
                "tags": resource.tags,
            }),
        });
    }

    match metric_max(resource, "CPUUtilization") {
        None => findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Cost,
            reason_code: REASON_COST_MISSING_UTILIZATION_TELEMETRY.to_string(),
            severity: Severity::Medium,
            message: format!(
                "Instance {} has no CPUUtilization telemetry; cost posture cannot distinguish idle from active capacity",
                resource.resource_id
            ),
            evidence: json!({
                "required_metric": "CPUUtilization",
                "tags": resource.tags,
                "resource_data_keys": resource_data_keys(resource),
            }),
        }),
        Some(max_cpu)
            if state.as_deref() == Some("running") && max_cpu <= LOW_CPU_UTILIZATION_MAX =>
        {
            findings.push(InventoryFinding {
                resource_id: resource.resource_id.clone(),
                arn: resource.arn.clone(),
                pillar: Pillar::Cost,
                reason_code: REASON_COST_LOW_UTILIZATION_TELEMETRY.to_string(),
                severity: Severity::Low,
                message: format!(
                    "Instance {} has low CPUUtilization telemetry; review for rightsizing or scheduling",
                    resource.resource_id
                ),
                evidence: json!({
                    "metric_name": "CPUUtilization",
                    "max": max_cpu,
                    "low_utilization_max": LOW_CPU_UTILIZATION_MAX,
                    "tags": resource.tags,
                }),
            });
        }
        _ => {}
    }
}

fn evaluate_security(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    let public_ip = data_str(&resource.resource_data, "public_ip").filter(|ip| !ip.is_empty());
    if let Some(public_ip) = data_str(&resource.resource_data, "public_ip") {
        if !public_ip.is_empty() {
            findings.push(InventoryFinding {
                resource_id: resource.resource_id.clone(),
                arn: resource.arn.clone(),
                pillar: Pillar::Security,
                reason_code: REASON_SEC_PUBLIC_IP.to_string(),
                severity: Severity::High,
                message: format!(
                    "Instance {} has a public IP address assigned; verify it is intentionally internet-facing",
                    resource.resource_id
                ),
                evidence: json!({ "public_ip": public_ip }),
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
                "Instance {} has no owner/team tag; security findings cannot be routed to an owner",
                resource.resource_id
            ),
            evidence: json!({ "tags": resource.tags }),
        });
    }

    let packet_metrics = ["NetworkPacketsIn", "NetworkPacketsOut"];
    let missing_packet_metrics = missing_metrics(resource, &packet_metrics);
    if !missing_packet_metrics.is_empty() {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Security,
            reason_code: REASON_SEC_MISSING_PACKET_TELEMETRY.to_string(),
            severity: Severity::Medium,
            message: format!(
                "Instance {} is missing EC2 packet telemetry; network exposure cannot be verified from evidence",
                resource.resource_id
            ),
            evidence: json!({
                "required_metrics": packet_metrics,
                "missing_metrics": missing_packet_metrics,
            }),
        });
    }

    if let (Some(public_ip), Some(max_packets_in)) =
        (public_ip, metric_max(resource, "NetworkPacketsIn"))
    {
        if max_packets_in > 0.0 {
            findings.push(InventoryFinding {
                resource_id: resource.resource_id.clone(),
                arn: resource.arn.clone(),
                pillar: Pillar::Security,
                reason_code: REASON_SEC_PUBLIC_PACKET_TRAFFIC_TELEMETRY.to_string(),
                severity: Severity::High,
                message: format!(
                    "Instance {} has a public IP and inbound packet telemetry",
                    resource.resource_id
                ),
                evidence: json!({
                    "public_ip": public_ip,
                    "metric_name": "NetworkPacketsIn",
                    "max": max_packets_in,
                }),
            });
        }
    }
}

fn evaluate_resilience(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    if data_str(&resource.resource_data, "availability_zone")
        .map(|az| az.is_empty())
        .unwrap_or(true)
    {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Resilience,
            reason_code: REASON_RES_MISSING_AZ.to_string(),
            severity: Severity::Medium,
            message: format!(
                "Instance {} has no availability zone recorded; placement resilience cannot be assessed",
                resource.resource_id
            ),
            evidence: json!({ "resource_data": resource.resource_data }),
        });
    }

    let status_metrics = [
        "StatusCheckFailed",
        "StatusCheckFailed_Instance",
        "StatusCheckFailed_System",
    ];
    let observed: Vec<(&str, f64)> = status_metrics
        .iter()
        .filter_map(|metric| metric_max(resource, metric).map(|value| (*metric, value)))
        .collect();

    if observed.is_empty() {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Resilience,
            reason_code: REASON_RES_MISSING_STATUS_TELEMETRY.to_string(),
            severity: Severity::Medium,
            message: format!(
                "Instance {} has no EC2 status check telemetry; reachability resilience cannot be verified",
                resource.resource_id
            ),
            evidence: json!({
                "required_metrics": status_metrics,
                "resource_data_keys": resource_data_keys(resource),
            }),
        });
    } else if let Some((metric_name, max_value)) = observed
        .iter()
        .copied()
        .find(|(_, max_value)| *max_value > 0.0)
    {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Resilience,
            reason_code: REASON_RES_STATUS_CHECK_FAILURE_TELEMETRY.to_string(),
            severity: Severity::High,
            message: format!(
                "Instance {} has non-zero EC2 status check failure telemetry",
                resource.resource_id
            ),
            evidence: json!({
                "metric_name": metric_name,
                "max": max_value,
            }),
        });
    }
}

fn evaluate_performance(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    let required_metrics = [
        "CPUUtilization",
        "NetworkIn",
        "NetworkOut",
        "DiskReadOps",
        "DiskWriteOps",
    ];
    let missing_metrics = missing_metrics(resource, &required_metrics);
    if !missing_metrics.is_empty() {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Performance,
            reason_code: REASON_PERF_MISSING_CORE_TELEMETRY.to_string(),
            severity: Severity::Medium,
            message: format!(
                "Instance {} is missing core EC2 performance telemetry",
                resource.resource_id
            ),
            evidence: json!({
                "required_metrics": required_metrics,
                "missing_metrics": missing_metrics,
            }),
        });
    }

    if let Some(max_cpu) = metric_max(resource, "CPUUtilization") {
        if max_cpu >= HIGH_CPU_UTILIZATION_MIN {
            findings.push(InventoryFinding {
                resource_id: resource.resource_id.clone(),
                arn: resource.arn.clone(),
                pillar: Pillar::Performance,
                reason_code: REASON_PERF_HIGH_CPU_TELEMETRY.to_string(),
                severity: Severity::High,
                message: format!(
                    "Instance {} has high CPUUtilization telemetry",
                    resource.resource_id
                ),
                evidence: json!({
                    "metric_name": "CPUUtilization",
                    "max": max_cpu,
                    "high_utilization_min": HIGH_CPU_UTILIZATION_MIN,
                }),
            });
        }
    }
}

fn evaluate_scalability(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    let required_metrics = ["CPUUtilization", "NetworkIn", "NetworkOut"];
    let missing_metrics = missing_metrics(resource, &required_metrics);
    if !missing_metrics.is_empty() {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Scalability,
            reason_code: REASON_SCALE_MISSING_DEMAND_TELEMETRY.to_string(),
            severity: Severity::Medium,
            message: format!(
                "Instance {} is missing EC2 demand telemetry needed to assess scaling pressure",
                resource.resource_id
            ),
            evidence: json!({
                "required_metrics": required_metrics,
                "missing_metrics": missing_metrics,
            }),
        });
    }

    if data_str(&resource.resource_data, "state").as_deref() == Some("running") {
        if let Some(max_cpu) = metric_max(resource, "CPUUtilization") {
            if max_cpu >= SCALING_CPU_PRESSURE_MIN {
                findings.push(InventoryFinding {
                    resource_id: resource.resource_id.clone(),
                    arn: resource.arn.clone(),
                    pillar: Pillar::Scalability,
                    reason_code: REASON_SCALE_HIGH_CPU_PRESSURE_TELEMETRY.to_string(),
                    severity: Severity::High,
                    message: format!(
                        "Instance {} has high CPUUtilization telemetry; scale-out or rightsizing pressure is likely",
                        resource.resource_id
                    ),
                    evidence: json!({
                        "metric_name": "CPUUtilization",
                        "max": max_cpu,
                        "scaling_cpu_pressure_min": SCALING_CPU_PRESSURE_MIN,
                    }),
                });
            }
        }
    }
}

fn evaluate_disaster_recovery(
    resource: &AwsResourceModel,
    now: DateTime<Utc>,
    findings: &mut Vec<InventoryFinding>,
) {
    match recovery_point_age_hours(resource, now) {
        None => findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::DisasterRecovery,
            reason_code: REASON_DR_MISSING_RECOVERY_POINT_TELEMETRY.to_string(),
            severity: Severity::Medium,
            message: format!(
                "Instance {} has no recovery point telemetry; EC2 restore posture cannot be verified",
                resource.resource_id
            ),
            evidence: json!({
                "expected_fields": [
                    "latest_recovery_point_age_hours",
                    "recovery_point_age_hours",
                    "latest_recovery_point_at"
                ],
                "resource_data_keys": resource_data_keys(resource),
            }),
        }),
        Some(age_hours) if age_hours > RECOVERY_POINT_STALE_AFTER_HOURS => {
            findings.push(InventoryFinding {
                resource_id: resource.resource_id.clone(),
                arn: resource.arn.clone(),
                pillar: Pillar::DisasterRecovery,
                reason_code: REASON_DR_STALE_RECOVERY_POINT_TELEMETRY.to_string(),
                severity: Severity::High,
                message: format!(
                    "Instance {} has stale recovery point telemetry",
                    resource.resource_id
                ),
                evidence: json!({
                    "latest_recovery_point_age_hours": age_hours,
                    "stale_after_hours": RECOVERY_POINT_STALE_AFTER_HOURS,
                }),
            });
        }
        _ => {}
    }
}

fn evaluate_operational_excellence(
    resource: &AwsResourceModel,
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
    let missing_fields: Vec<&str> = required_fields
        .iter()
        .copied()
        .filter(|field| resource.resource_data.get(*field).is_none())
        .collect();
    if !missing_fields.is_empty() {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::OperationalExcellence,
            reason_code: REASON_OE_MISSING_TELEMETRY_COLLECTION_METADATA.to_string(),
            severity: Severity::Medium,
            message: format!(
                "Instance {} is missing EC2 telemetry collection metadata needed for operational runbooks",
                resource.resource_id
            ),
            evidence: json!({
                "required_fields": required_fields,
                "missing_fields": missing_fields,
                "resource_data_keys": resource_data_keys(resource),
            }),
        });
    }

    let error_count = resource
        .resource_data
        .get("telemetry_collection_error_count")
        .and_then(|value| {
            value
                .as_u64()
                .or_else(|| value.as_i64().map(|n| n.max(0) as u64))
        })
        .unwrap_or(0);
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
            pillar: Pillar::OperationalExcellence,
            reason_code: REASON_OE_TELEMETRY_COLLECTION_ERRORS.to_string(),
            severity: Severity::High,
            message: format!(
                "Instance {} has EC2 telemetry collection errors; operators cannot rely on complete evidence",
                resource.resource_id
            ),
            evidence: json!({
                "telemetry_collection_error_count": error_count,
                "telemetry_collection_errors": errors,
            }),
        });
    }

    if data_str(&resource.resource_data, "monitoring_state").as_deref() == Some("disabled") {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::OperationalExcellence,
            reason_code: REASON_OE_BASIC_MONITORING.to_string(),
            severity: Severity::Low,
            message: format!(
                "Instance {} uses basic EC2 monitoring; operational diagnosis has lower-resolution telemetry",
                resource.resource_id
            ),
            evidence: json!({ "monitoring_state": "disabled" }),
        });
    }
}

/// Fleet-level check: two or more running instances all placed in one AZ.
fn check_az_concentration(resources: &[AwsResourceModel]) -> Option<InventoryFinding> {
    let placements: Vec<(&AwsResourceModel, String)> = resources
        .iter()
        .filter(|r| data_str(&r.resource_data, "state").as_deref() == Some("running"))
        .filter_map(|r| {
            data_str(&r.resource_data, "availability_zone")
                .filter(|az| !az.is_empty())
                .map(|az| (r, az))
        })
        .collect();

    if placements.len() < 2 {
        return None;
    }
    let first_az = placements[0].1.clone();
    if !placements.iter().all(|(_, az)| *az == first_az) {
        return None;
    }
    let instance_ids: Vec<&str> = placements
        .iter()
        .map(|(r, _)| r.resource_id.as_str())
        .collect();
    Some(InventoryFinding {
        resource_id: "fleet".to_string(),
        arn: String::new(),
        pillar: Pillar::Resilience,
        reason_code: REASON_RES_SINGLE_AZ_CONCENTRATION.to_string(),
        severity: Severity::Medium,
        message: format!(
            "All {} running instances are placed in availability zone {}; an AZ outage takes down the whole fleet",
            instance_ids.len(),
            first_az
        ),
        evidence: json!({
            "availability_zone": first_az,
            "instance_ids": instance_ids,
        }),
    })
}

const LOW_CPU_UTILIZATION_MAX: f64 = 5.0;
const HIGH_CPU_UTILIZATION_MIN: f64 = 90.0;
const SCALING_CPU_PRESSURE_MIN: f64 = 80.0;
const RECOVERY_POINT_STALE_AFTER_HOURS: f64 = 24.0;

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

fn recovery_point_age_hours(resource: &AwsResourceModel, now: DateTime<Utc>) -> Option<f64> {
    for key in [
        "latest_recovery_point_age_hours",
        "recovery_point_age_hours",
        "backup_age_hours",
    ] {
        if let Some(value) = resource
            .resource_data
            .get(key)
            .and_then(|value| value.as_f64().or_else(|| value.as_i64().map(|n| n as f64)))
        {
            return Some(value);
        }
    }

    if let Some(value) = resource
        .resource_data
        .pointer("/disaster_recovery/latest_recovery_point_age_hours")
        .and_then(|value| value.as_f64().or_else(|| value.as_i64().map(|n| n as f64)))
    {
        return Some(value);
    }

    for key in ["latest_recovery_point_at", "recovery_point_at"] {
        if let Some(value) = resource
            .resource_data
            .get(key)
            .and_then(|value| value.as_str())
            .and_then(|value| DateTime::parse_from_rfc3339(value).ok())
        {
            return Some((now - value.with_timezone(&Utc)).num_hours() as f64);
        }
    }

    resource
        .resource_data
        .pointer("/disaster_recovery/latest_recovery_point_at")
        .and_then(|value| value.as_str())
        .and_then(|value| DateTime::parse_from_rfc3339(value).ok())
        .map(|value| (now - value.with_timezone(&Utc)).num_hours() as f64)
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

fn resource_data_keys(resource: &AwsResourceModel) -> Vec<String> {
    resource
        .resource_data
        .as_object()
        .map(|object| object.keys().cloned().collect())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::aws::inventory::types::DEFAULT_STALE_AFTER_HOURS;
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
            resource_type: "EC2Instance".to_string(),
            resource_id: resource_id.to_string(),
            arn: format!(
                "arn:aws:ec2:us-east-1:123456789012:instance/{}",
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

    fn reason_codes(report: &PillarReport) -> Vec<&str> {
        report
            .findings
            .iter()
            .map(|f| f.reason_code.as_str())
            .collect()
    }

    fn metric(metric_name: &str, values: &[f64]) -> Value {
        json!({
            "metric_name": metric_name,
            "datapoints": values.iter().map(|value| json!({ "value": value })).collect::<Vec<_>>()
        })
    }

    #[test]
    fn cost_flags_missing_allocation_tags_and_stopped_instance() {
        let r = fixture(
            "i-untagged",
            json!({}),
            json!({"state": "stopped", "availability_zone": "us-east-1a"}),
            1,
            now(),
        );
        let report = evaluate_ec2_fleet(&[r], Pillar::Cost, now());
        let codes = reason_codes(&report);
        assert!(codes.contains(&REASON_COST_MISSING_ALLOCATION_TAGS));
        assert!(codes.contains(&REASON_COST_STOPPED_INSTANCE));
        // Evidence preserved: raw tags object for the tag finding.
        let tag_finding = report
            .findings
            .iter()
            .find(|f| f.reason_code == REASON_COST_MISSING_ALLOCATION_TAGS)
            .unwrap();
        assert_eq!(tag_finding.evidence["tags"], json!({}));
        assert!(report.score < 100);
    }

    #[test]
    fn cost_passes_for_tagged_running_instance() {
        let r = fixture(
            "i-good",
            json!({"Team": "payments", "cost-center": "cc-42"}),
            json!({
                "state": "running",
                "availability_zone": "us-east-1a",
                "cloudwatch_metrics": {
                    "metrics": [
                        metric("CPUUtilization", &[35.0, 42.0])
                    ]
                }
            }),
            1,
            now(),
        );
        let report = evaluate_ec2_fleet(&[r], Pillar::Cost, now());
        assert!(
            report.findings.is_empty(),
            "unexpected: {:?}",
            report.findings
        );
        assert_eq!(report.score, 100);
        assert_eq!(report.resources_evaluated, 1);
        assert_eq!(report.stale_resources, 0);
    }

    #[test]
    fn security_flags_public_ip_as_high_and_missing_owner_as_low() {
        let r = fixture(
            "i-exposed",
            json!({}),
            json!({"state": "running", "public_ip": "54.0.0.1", "availability_zone": "us-east-1a"}),
            1,
            now(),
        );
        let report = evaluate_ec2_fleet(&[r], Pillar::Security, now());
        let public = report
            .findings
            .iter()
            .find(|f| f.reason_code == REASON_SEC_PUBLIC_IP)
            .expect("public ip finding");
        assert_eq!(public.severity, Severity::High);
        assert_eq!(public.evidence["public_ip"], json!("54.0.0.1"));
        let owner = report
            .findings
            .iter()
            .find(|f| f.reason_code == REASON_SEC_MISSING_OWNER_TAG)
            .expect("owner tag finding");
        assert_eq!(owner.severity, Severity::Low);
    }

    #[test]
    fn security_passes_for_private_owned_instance() {
        let r = fixture(
            "i-private",
            json!([{"Key": "Owner", "Value": "sre"}]),
            json!({
                "state": "running",
                "private_ip": "10.0.0.5",
                "availability_zone": "us-east-1a",
                "cloudwatch_metrics": {
                    "metrics": [
                        metric("NetworkPacketsIn", &[0.0]),
                        metric("NetworkPacketsOut", &[0.0])
                    ]
                }
            }),
            1,
            now(),
        );
        let report = evaluate_ec2_fleet(&[r], Pillar::Security, now());
        assert!(
            report.findings.is_empty(),
            "unexpected: {:?}",
            report.findings
        );
    }

    #[test]
    fn resilience_flags_missing_availability_zone() {
        let r = fixture(
            "i-noaz",
            json!({"owner": "sre"}),
            json!({
                "state": "running",
                "cloudwatch_metrics": {
                    "metrics": [
                        metric("StatusCheckFailed", &[0.0]),
                        metric("StatusCheckFailed_Instance", &[0.0]),
                        metric("StatusCheckFailed_System", &[0.0])
                    ]
                }
            }),
            1,
            now(),
        );
        let report = evaluate_ec2_fleet(&[r], Pillar::Resilience, now());
        assert_eq!(reason_codes(&report), vec![REASON_RES_MISSING_AZ]);
    }

    #[test]
    fn resilience_flags_single_az_concentration_for_running_fleet() {
        let a = fixture(
            "i-a",
            json!({"owner": "sre"}),
            json!({
                "state": "running",
                "availability_zone": "us-east-1a",
                "cloudwatch_metrics": {
                    "metrics": [
                        metric("StatusCheckFailed", &[0.0]),
                        metric("StatusCheckFailed_Instance", &[0.0]),
                        metric("StatusCheckFailed_System", &[0.0])
                    ]
                }
            }),
            1,
            now(),
        );
        let b = fixture(
            "i-b",
            json!({"owner": "sre"}),
            json!({
                "state": "running",
                "availability_zone": "us-east-1a",
                "cloudwatch_metrics": {
                    "metrics": [
                        metric("StatusCheckFailed", &[0.0]),
                        metric("StatusCheckFailed_Instance", &[0.0]),
                        metric("StatusCheckFailed_System", &[0.0])
                    ]
                }
            }),
            1,
            now(),
        );
        let report = evaluate_ec2_fleet(&[a, b], Pillar::Resilience, now());
        let fleet = report
            .findings
            .iter()
            .find(|f| f.reason_code == REASON_RES_SINGLE_AZ_CONCENTRATION)
            .expect("fleet concentration finding");
        assert_eq!(fleet.evidence["availability_zone"], json!("us-east-1a"));
        assert_eq!(fleet.evidence["instance_ids"], json!(["i-a", "i-b"]));
    }

    #[test]
    fn resilience_accepts_multi_az_fleet() {
        let a = fixture(
            "i-a",
            json!({"owner": "sre"}),
            json!({
                "state": "running",
                "availability_zone": "us-east-1a",
                "cloudwatch_metrics": {
                    "metrics": [
                        metric("StatusCheckFailed", &[0.0]),
                        metric("StatusCheckFailed_Instance", &[0.0]),
                        metric("StatusCheckFailed_System", &[0.0])
                    ]
                }
            }),
            1,
            now(),
        );
        let b = fixture(
            "i-b",
            json!({"owner": "sre"}),
            json!({
                "state": "running",
                "availability_zone": "us-east-1b",
                "cloudwatch_metrics": {
                    "metrics": [
                        metric("StatusCheckFailed", &[0.0]),
                        metric("StatusCheckFailed_Instance", &[0.0]),
                        metric("StatusCheckFailed_System", &[0.0])
                    ]
                }
            }),
            1,
            now(),
        );
        let report = evaluate_ec2_fleet(&[a, b], Pillar::Resilience, now());
        assert!(
            report.findings.is_empty(),
            "unexpected: {:?}",
            report.findings
        );
        assert_eq!(report.score, 100);
    }

    #[test]
    fn ec2_resilience_posture_summary_maps_rules_to_affected_resources() {
        let missing_az = fixture(
            "i-noaz",
            json!({"owner": "sre"}),
            json!({
                "state": "running",
                "cloudwatch_metrics": {
                    "metrics": [
                        metric("StatusCheckFailed", &[0.0]),
                        metric("StatusCheckFailed_Instance", &[0.0]),
                        metric("StatusCheckFailed_System", &[0.0])
                    ]
                }
            }),
            1,
            now(),
        );
        let status_failed = fixture(
            "i-status-failed",
            json!({"owner": "sre"}),
            json!({
                "state": "running",
                "availability_zone": "us-east-1a",
                "cloudwatch_metrics": {
                    "metrics": [
                        metric("StatusCheckFailed", &[0.0, 1.0]),
                        metric("StatusCheckFailed_Instance", &[0.0]),
                        metric("StatusCheckFailed_System", &[0.0])
                    ]
                }
            }),
            1,
            now(),
        );

        let report = evaluate_ec2_fleet(&[missing_az, status_failed], Pillar::Resilience, now());
        let posture = ec2_resilience_posture_summary(&report);

        assert_eq!(posture.status, Ec2PostureStatus::Fail);
        assert_eq!(posture.rules_evaluated, 4);
        assert_eq!(posture.rules_failed, 2);
        assert_eq!(
            posture
                .rules
                .iter()
                .map(|rule| rule.rule_id)
                .collect::<Vec<_>>(),
            vec![
                "ec2-resilience-availability-zone-recorded",
                "ec2-resilience-multi-az-placement",
                "ec2-resilience-status-check-telemetry",
                "ec2-resilience-status-check-health",
            ]
        );
        assert_eq!(
            posture
                .rules
                .iter()
                .map(|rule| rule.reason_codes.clone())
                .collect::<Vec<_>>(),
            vec![
                vec![REASON_RES_MISSING_AZ],
                vec![REASON_RES_SINGLE_AZ_CONCENTRATION],
                vec![REASON_RES_MISSING_STATUS_TELEMETRY],
                vec![REASON_RES_STATUS_CHECK_FAILURE_TELEMETRY],
            ]
        );
        assert!(posture.affected_resources.contains(&"i-noaz".to_string()));
        assert!(posture
            .affected_resources
            .contains(&"i-status-failed".to_string()));
        let az_rule = posture
            .rules
            .iter()
            .find(|rule| rule.rule_id == "ec2-resilience-availability-zone-recorded")
            .expect("availability zone rule");
        assert_eq!(az_rule.status, Ec2PostureStatus::Fail);
        assert_eq!(az_rule.reason_codes, vec![REASON_RES_MISSING_AZ]);
        assert!(az_rule.assignment_supported);
        assert!(az_rule.suppression_supported);
    }

    #[test]
    fn ec2_resilience_posture_summary_passes_for_multi_az_healthy_fleet() {
        let a = fixture(
            "i-a",
            json!({"owner": "sre"}),
            json!({
                "state": "running",
                "availability_zone": "us-east-1a",
                "cloudwatch_metrics": {
                    "metrics": [
                        metric("StatusCheckFailed", &[0.0]),
                        metric("StatusCheckFailed_Instance", &[0.0]),
                        metric("StatusCheckFailed_System", &[0.0])
                    ]
                }
            }),
            1,
            now(),
        );
        let b = fixture(
            "i-b",
            json!({"owner": "sre"}),
            json!({
                "state": "running",
                "availability_zone": "us-east-1b",
                "cloudwatch_metrics": {
                    "metrics": [
                        metric("StatusCheckFailed", &[0.0]),
                        metric("StatusCheckFailed_Instance", &[0.0]),
                        metric("StatusCheckFailed_System", &[0.0])
                    ]
                }
            }),
            1,
            now(),
        );

        let report = evaluate_ec2_fleet(&[a, b], Pillar::Resilience, now());
        let posture = ec2_resilience_posture_summary(&report);

        assert_eq!(posture.status, Ec2PostureStatus::Pass);
        assert_eq!(posture.rules_failed, 0);
        assert!(posture.affected_resources.is_empty());
    }

    #[test]
    fn stale_inventory_is_reported_as_failure_path_for_every_pillar() {
        let r = fixture(
            "i-stale",
            json!({"owner": "sre", "project": "mayyam"}),
            json!({"state": "running", "availability_zone": "us-east-1a", "private_ip": "10.0.0.9"}),
            48,
            now(),
        );
        for pillar in [Pillar::Cost, Pillar::Security, Pillar::Resilience] {
            let report = evaluate_ec2_fleet(std::slice::from_ref(&r), pillar, now());
            assert_eq!(report.stale_resources, 1, "pillar {:?}", pillar);
            let stale = report
                .findings
                .iter()
                .find(|f| f.reason_code == REASON_INV_STALE_DATA)
                .unwrap_or_else(|| panic!("stale finding missing for {:?}", pillar));
            assert_eq!(stale.evidence["age_hours"], json!(48));
            assert_eq!(
                stale.evidence["stale_after_hours"],
                json!(DEFAULT_STALE_AFTER_HOURS)
            );
        }
    }

    #[test]
    fn ec2_telemetry_cost_flags_low_cpu_utilization() {
        let r = fixture(
            "i-idle",
            json!({"cost-center": "cc-42", "owner": "sre"}),
            json!({
                "state": "running",
                "availability_zone": "us-east-1a",
                "cloudwatch_metrics": {
                    "metrics": [
                        metric("CPUUtilization", &[1.2, 2.4, 3.0])
                    ]
                }
            }),
            1,
            now(),
        );

        let report = evaluate_ec2_fleet(&[r], Pillar::Cost, now());
        let idle = report
            .findings
            .iter()
            .find(|f| f.reason_code == REASON_COST_LOW_UTILIZATION_TELEMETRY)
            .expect("low utilization telemetry finding");
        assert_eq!(idle.severity, Severity::Low);
        assert_eq!(idle.evidence["metric_name"], json!("CPUUtilization"));
        assert_eq!(idle.evidence["max"], json!(3.0));
    }

    #[test]
    fn ec2_cost_posture_summary_maps_rules_to_affected_resources() {
        let untagged = fixture(
            "i-untagged",
            json!({}),
            json!({
                "state": "stopped",
                "availability_zone": "us-east-1a",
                "cloudwatch_metrics": {
                    "metrics": [
                        metric("CPUUtilization", &[12.0])
                    ]
                }
            }),
            1,
            now(),
        );
        let missing_telemetry = fixture(
            "i-missing-telemetry",
            json!({"cost-center": "cc-42", "owner": "sre"}),
            json!({"state": "running", "availability_zone": "us-east-1a"}),
            1,
            now(),
        );

        let report = evaluate_ec2_fleet(&[untagged, missing_telemetry], Pillar::Cost, now());
        let posture = ec2_cost_posture_summary(&report);

        assert_eq!(posture.status, Ec2PostureStatus::Fail);
        assert_eq!(posture.rules_evaluated, 4);
        assert_eq!(posture.rules_failed, 3);
        assert_eq!(
            posture.affected_resources,
            vec!["i-missing-telemetry".to_string(), "i-untagged".to_string()]
        );
        let allocation = posture
            .rules
            .iter()
            .find(|rule| rule.rule_id == "ec2-cost-allocation-tags")
            .expect("allocation tag rule");
        assert_eq!(allocation.status, Ec2PostureStatus::Fail);
        assert_eq!(
            allocation.reason_codes,
            vec![REASON_COST_MISSING_ALLOCATION_TAGS]
        );
        assert!(allocation.assignment_supported);
        assert!(allocation.suppression_supported);
    }

    #[test]
    fn ec2_cost_triage_context_separates_facts_hypotheses_and_questions() {
        let idle = fixture(
            "i-idle",
            json!({"cost-center": "cc-42", "owner": "sre"}),
            json!({
                "state": "running",
                "availability_zone": "us-east-1a",
                "cloudwatch_metrics": {
                    "metrics": [
                        metric("CPUUtilization", &[1.2, 2.4, 3.0])
                    ]
                }
            }),
            1,
            now(),
        );
        let missing_telemetry = fixture(
            "i-missing-telemetry",
            json!({"cost-center": "cc-42", "owner": "sre"}),
            json!({"state": "running", "availability_zone": "us-east-1a"}),
            1,
            now(),
        );

        let report = evaluate_ec2_fleet(&[idle, missing_telemetry], Pillar::Cost, now());
        let triage = ec2_cost_triage_context(&report);

        assert_eq!(triage.pillar, Pillar::Cost);
        assert!(triage
            .facts
            .iter()
            .any(|fact| fact.contains(REASON_COST_LOW_UTILIZATION_TELEMETRY)));
        assert!(triage
            .hypotheses
            .iter()
            .any(|hypothesis| hypothesis.contains("rightsizing")));
        assert!(triage
            .missing_data_questions
            .iter()
            .any(|question| question.contains("CPUUtilization")));
        assert!(triage
            .evidence_citations
            .iter()
            .any(|citation| citation.reason_code == REASON_COST_MISSING_UTILIZATION_TELEMETRY));
    }

    #[test]
    fn ec2_cost_agentic_investigation_plan_is_read_only_until_approval() {
        let idle = fixture(
            "i-idle",
            json!({"cost-center": "cc-42", "owner": "sre"}),
            json!({
                "state": "running",
                "availability_zone": "us-east-1a",
                "cloudwatch_metrics": {
                    "metrics": [
                        metric("CPUUtilization", &[1.2, 2.4, 3.0])
                    ]
                }
            }),
            1,
            now(),
        );
        let stopped = fixture(
            "i-stopped",
            json!({}),
            json!({
                "state": "stopped",
                "availability_zone": "us-east-1a",
                "cloudwatch_metrics": {
                    "metrics": [
                        metric("CPUUtilization", &[12.0])
                    ]
                }
            }),
            1,
            now(),
        );

        let report = evaluate_ec2_fleet(&[idle, stopped], Pillar::Cost, now());
        let plan = ec2_cost_agentic_investigation_plan(&report);

        assert_eq!(plan.workflow_id, "ec2_cost_agentic_investigation");
        assert_eq!(plan.default_tool_mode, Ec2InvestigationToolMode::ReadOnly);
        assert!(plan.replay_required);
        assert!(plan.steps.iter().any(|step| {
            step.tool_name == "ec2.compute_optimizer.get_instance_recommendations"
                && step.tool_mode == Ec2InvestigationToolMode::ReadOnly
                && step.target_resource_id == "i-idle"
        }));
        assert!(plan.steps.iter().any(|step| {
            step.tool_name == "ec2.describe_attached_cost_artifacts"
                && step.tool_mode == Ec2InvestigationToolMode::ReadOnly
                && step.target_resource_id == "i-stopped"
        }));
        assert_eq!(
            plan.steps.last().map(|step| step.tool_mode),
            Some(Ec2InvestigationToolMode::ApprovalRequired)
        );
        assert_eq!(plan.approval_gates.len(), 3);
        assert!(plan
            .approval_gates
            .iter()
            .all(|gate| gate.rollback_note_required));
    }

    #[test]
    fn ec2_cost_remediation_workflow_plans_dry_run_actions_until_approved() {
        let idle = fixture(
            "i-idle",
            json!({"cost-center": "cc-42", "owner": "sre"}),
            json!({
                "state": "running",
                "availability_zone": "us-east-1a",
                "cloudwatch_metrics": {
                    "metrics": [
                        metric("CPUUtilization", &[1.2, 2.4, 3.0])
                    ]
                }
            }),
            1,
            now(),
        );
        let stopped = fixture(
            "i-stopped",
            json!({}),
            json!({
                "state": "stopped",
                "availability_zone": "us-east-1a",
                "cloudwatch_metrics": {
                    "metrics": [
                        metric("CPUUtilization", &[12.0])
                    ]
                }
            }),
            1,
            now(),
        );

        let report = evaluate_ec2_fleet(&[idle, stopped], Pillar::Cost, now());
        let workflow = ec2_cost_remediation_workflow(&report);

        assert_eq!(workflow.workflow_id, "ec2_cost_safe_remediation");
        assert!(workflow.read_only_mode);
        assert_eq!(workflow.rbac_permission, "aws.ec2.cost.remediation.approve");
        assert_eq!(workflow.audit_stream, "ec2_cost_remediation_audit");
        assert!(!workflow.stale_data_blocks_execution);
        assert_eq!(workflow.actions.len(), 3);
        assert!(workflow.actions.iter().all(|action| {
            action.dry_run
                && action.requires_approval
                && action.approval_gate_id.is_some()
                && action.status == Ec2RemediationStatus::DryRunPendingApproval
                && action.audit_event_type == "ec2.cost.remediation.dry_run_planned"
                && action.rollback_note.contains("rollback")
        }));
        assert!(workflow.actions.iter().any(|action| {
            action.kind == Ec2RemediationActionKind::RightSizeInstance
                && action.target_resource_id == "i-idle"
                && action
                    .evidence_reason_codes
                    .contains(&REASON_COST_LOW_UTILIZATION_TELEMETRY.to_string())
        }));
        assert!(workflow.actions.iter().any(|action| {
            action.kind == Ec2RemediationActionKind::ReviewStoppedInstanceArtifacts
                && action.target_resource_id == "i-stopped"
                && action
                    .evidence_reason_codes
                    .contains(&REASON_COST_STOPPED_INSTANCE.to_string())
        }));
        assert!(workflow.actions.iter().any(|action| {
            action.kind == Ec2RemediationActionKind::AssignCostTags
                && action.target_resource_id == "i-stopped"
                && action
                    .evidence_reason_codes
                    .contains(&REASON_COST_MISSING_ALLOCATION_TAGS.to_string())
        }));
    }

    #[test]
    fn ec2_cost_remediation_workflow_blocks_execution_when_cost_data_is_stale() {
        let stale = fixture(
            "i-stale",
            json!({"cost-center": "cc-42", "owner": "sre"}),
            json!({
                "state": "running",
                "availability_zone": "us-east-1a",
                "cloudwatch_metrics": {
                    "metrics": [
                        metric("CPUUtilization", &[1.2, 2.4, 3.0])
                    ]
                }
            }),
            30,
            now(),
        );

        let report = evaluate_ec2_fleet(&[stale], Pillar::Cost, now());
        let workflow = ec2_cost_remediation_workflow(&report);

        assert!(workflow.stale_data_blocks_execution);
        assert!(workflow.actions.iter().all(|action| {
            action.dry_run && action.status == Ec2RemediationStatus::BlockedMissingEvidence
        }));
    }

    #[test]
    fn ec2_cost_slo_policy_snapshot_tracks_owner_policy_and_notifications() {
        let tagged = fixture(
            "i-tagged",
            json!({
                "cost-center": "cc-42",
                "owner": "sre",
                "environment": "prod",
                "application": "payments"
            }),
            json!({
                "state": "running",
                "availability_zone": "us-east-1a",
                "cloudwatch_metrics": {
                    "metrics": [
                        metric("CPUUtilization", &[1.2, 2.4, 3.0])
                    ]
                }
            }),
            1,
            now(),
        );
        let unowned = fixture(
            "i-unowned",
            json!({}),
            json!({
                "state": "running",
                "availability_zone": "us-east-1a",
                "cloudwatch_metrics": {
                    "metrics": [
                        metric("CPUUtilization", &[44.0])
                    ]
                }
            }),
            1,
            now(),
        );

        let report = evaluate_ec2_fleet(&[tagged, unowned], Pillar::Cost, now());
        let snapshot = ec2_cost_slo_policy_snapshot(&report);

        assert_eq!(snapshot.workflow_id, "ec2_cost_slo_policy");
        assert!(snapshot.read_only_mode);
        assert!(snapshot.freshness_required);
        assert_eq!(snapshot.objective.objective_id, "ec2-cost-score-min-90");
        assert_eq!(snapshot.objective.status, Ec2CostObjectiveStatus::AtRisk);
        assert_eq!(snapshot.objective.target_score_min, 90);
        assert_eq!(snapshot.objective.failed_rule_count, 2);
        assert!(snapshot.objective.affected_resource_count >= 2);
        assert_eq!(snapshot.objective.owner_filters, vec!["sre"]);
        assert_eq!(snapshot.objective.environment_filters, vec!["prod"]);
        assert_eq!(snapshot.objective.application_filters, vec!["payments"]);
        assert_eq!(
            snapshot.objective.notification_targets,
            vec!["environment:prod", "owner:sre"]
        );
        assert_eq!(snapshot.objective.policy_state, "active_with_findings");
        assert!(snapshot
            .evidence_reason_codes
            .contains(&REASON_COST_LOW_UTILIZATION_TELEMETRY.to_string()));
    }

    #[test]
    fn ec2_cost_slo_policy_snapshot_marks_stale_data_as_breached() {
        let stale = fixture(
            "i-stale",
            json!({"cost-center": "cc-42", "owner": "sre"}),
            json!({
                "state": "running",
                "availability_zone": "us-east-1a",
                "cloudwatch_metrics": {
                    "metrics": [
                        metric("CPUUtilization", &[44.0])
                    ]
                }
            }),
            30,
            now(),
        );

        let report = evaluate_ec2_fleet(&[stale], Pillar::Cost, now());
        let snapshot = ec2_cost_slo_policy_snapshot(&report);

        assert_eq!(snapshot.objective.status, Ec2CostObjectiveStatus::Breached);
        assert_eq!(
            snapshot.objective.trend_direction,
            Ec2CostTrendDirection::Degrading
        );
        assert_eq!(snapshot.objective.policy_state, "blocked_stale_data");
        assert!(snapshot
            .evidence_reason_codes
            .contains(&REASON_INV_STALE_DATA.to_string()));
    }

    #[test]
    fn ec2_cost_forecast_snapshot_builds_read_only_cost_band_from_evidence() {
        let idle = fixture(
            "i-idle",
            json!({"cost-center": "cc-42", "owner": "sre"}),
            json!({
                "state": "running",
                "availability_zone": "us-east-1a",
                "cloudwatch_metrics": {
                    "metrics": [
                        metric("CPUUtilization", &[1.2, 2.4, 3.0])
                    ]
                }
            }),
            1,
            now(),
        );
        let stopped = fixture(
            "i-stopped",
            json!({}),
            json!({
                "state": "stopped",
                "availability_zone": "us-east-1a",
                "cloudwatch_metrics": {
                    "metrics": [
                        metric("CPUUtilization", &[12.0])
                    ]
                }
            }),
            1,
            now(),
        );

        let report = evaluate_ec2_fleet(&[idle, stopped], Pillar::Cost, now());
        let forecast = ec2_cost_forecast_snapshot(&report);

        assert_eq!(forecast.workflow_id, "ec2_cost_forecasting");
        assert!(forecast.read_only_mode);
        assert_eq!(forecast.baseline_window_days, 30);
        assert_eq!(forecast.forecast_horizon_days, 30);
        assert_eq!(forecast.confidence_level, 80);
        assert_eq!(forecast.risk_level, Ec2CostForecastRisk::Moderate);
        assert_eq!(forecast.capacity_risk, "overprovisioned_compute_capacity");
        assert_eq!(forecast.forecast_band.expected_monthly_cost_index, 132);
        assert!(forecast.forecast_band.upper_monthly_cost_index > 132);
        assert_eq!(
            forecast.backtesting_fixture_status,
            "ready_findings_baseline"
        );
        assert!(forecast
            .what_if_inputs
            .contains(&"rightsize_low_utilization_instances"));
        assert!(forecast.risk_drivers.iter().any(|driver| {
            driver.reason_code == REASON_COST_LOW_UTILIZATION_TELEMETRY
                && driver.affected_resources == vec!["i-idle".to_string()]
                && driver.monthly_cost_index_delta == 18
        }));
        assert!(forecast.risk_drivers.iter().any(|driver| {
            driver.reason_code == REASON_COST_STOPPED_INSTANCE
                && driver.affected_resources == vec!["i-stopped".to_string()]
        }));
    }

    #[test]
    fn ec2_cost_forecast_snapshot_blocks_on_stale_or_missing_telemetry() {
        let stale = fixture(
            "i-stale",
            json!({"cost-center": "cc-42", "owner": "sre"}),
            json!({"state": "running", "availability_zone": "us-east-1a"}),
            30,
            now(),
        );

        let report = evaluate_ec2_fleet(&[stale], Pillar::Cost, now());
        let forecast = ec2_cost_forecast_snapshot(&report);

        assert_eq!(forecast.risk_level, Ec2CostForecastRisk::Blocked);
        assert!(forecast.blocked_by_stale_data);
        assert_eq!(forecast.capacity_risk, "blocked_until_inventory_refresh");
        assert_eq!(
            forecast.backtesting_fixture_status,
            "needs_fresh_telemetry_fixture"
        );
        assert!(forecast
            .missing_data_reason_codes
            .contains(&REASON_INV_STALE_DATA.to_string()));
        assert!(forecast
            .missing_data_reason_codes
            .contains(&REASON_COST_MISSING_UTILIZATION_TELEMETRY.to_string()));
        assert!(forecast.forecast_band.upper_monthly_cost_index > 100);
    }

    #[test]
    fn ec2_cost_reporting_bundle_materializes_executive_and_engineering_views() {
        let idle = fixture(
            "i-idle",
            json!({
                "cost-center": "cc-42",
                "owner": "sre",
                "environment": "prod"
            }),
            json!({
                "state": "running",
                "availability_zone": "us-east-1a",
                "cloudwatch_metrics": {
                    "metrics": [
                        metric("CPUUtilization", &[1.2, 2.4, 3.0])
                    ]
                }
            }),
            1,
            now(),
        );
        let stopped = fixture(
            "i-stopped",
            json!({}),
            json!({
                "state": "stopped",
                "availability_zone": "us-east-1a",
                "cloudwatch_metrics": {
                    "metrics": [
                        metric("CPUUtilization", &[12.0])
                    ]
                }
            }),
            1,
            now(),
        );

        let report = evaluate_ec2_fleet(&[idle, stopped], Pillar::Cost, now());
        let bundle = ec2_cost_reporting_bundle(&report);

        assert_eq!(bundle.workflow_id, "ec2_cost_reporting");
        assert!(bundle.read_only_mode);
        assert_eq!(bundle.scheduled_delivery_state, "ready_for_schedule");
        assert_eq!(
            bundle.executive_summary.report_id,
            "ec2-cost-executive-summary"
        );
        assert_eq!(bundle.executive_summary.score, report.score);
        assert_eq!(bundle.executive_summary.resources_evaluated, 2);
        assert_eq!(bundle.executive_summary.stale_resources, 0);
        assert!(bundle
            .executive_summary
            .affected_resources
            .contains(&"i-idle".to_string()));
        assert!(bundle
            .executive_summary
            .top_reason_codes
            .contains(&REASON_COST_LOW_UTILIZATION_TELEMETRY.to_string()));
        assert_eq!(
            bundle.engineering_backlog.report_id,
            "ec2-cost-engineering-backlog"
        );
        assert_eq!(bundle.engineering_backlog.page, 0);
        assert_eq!(bundle.engineering_backlog.page_size, 50);
        assert_eq!(bundle.engineering_backlog.total, report.findings.len());
        assert!(bundle.engineering_backlog.rows.iter().any(|row| {
            row.resource_id == "i-idle"
                && row.reason_code == REASON_COST_LOW_UTILIZATION_TELEMETRY
                && row.severity == Severity::Low
                && row.evidence["metric_name"] == json!("CPUUtilization")
        }));
    }

    #[test]
    fn ec2_cost_reporting_bundle_blocks_scheduled_delivery_for_stale_inventory() {
        let stale = fixture(
            "i-stale",
            json!({"cost-center": "cc-42", "owner": "sre"}),
            json!({
                "state": "running",
                "availability_zone": "us-east-1a",
                "cloudwatch_metrics": {
                    "metrics": [
                        metric("CPUUtilization", &[44.0])
                    ]
                }
            }),
            30,
            now(),
        );

        let report = evaluate_ec2_fleet(&[stale], Pillar::Cost, now());
        let bundle = ec2_cost_reporting_bundle(&report);

        assert_eq!(
            bundle.scheduled_delivery_state,
            "blocked_until_fresh_inventory"
        );
        assert!(bundle
            .evidence_reason_codes
            .contains(&REASON_INV_STALE_DATA.to_string()));
        assert_eq!(bundle.executive_summary.stale_resources, 1);
        assert!(bundle
            .executive_summary
            .top_reason_codes
            .contains(&REASON_INV_STALE_DATA.to_string()));
        assert!(bundle
            .engineering_backlog
            .rows
            .iter()
            .any(|row| row.reason_code == REASON_INV_STALE_DATA));
    }

    #[test]
    fn ec2_telemetry_resilience_flags_status_check_failures() {
        let r = fixture(
            "i-status-failed",
            json!({"owner": "sre"}),
            json!({
                "state": "running",
                "availability_zone": "us-east-1a",
                "cloudwatch_metrics": {
                    "metrics": [
                        metric("StatusCheckFailed", &[0.0, 1.0]),
                        metric("StatusCheckFailed_Instance", &[0.0]),
                        metric("StatusCheckFailed_System", &[0.0])
                    ]
                }
            }),
            1,
            now(),
        );

        let report = evaluate_ec2_fleet(&[r], Pillar::Resilience, now());
        let status = report
            .findings
            .iter()
            .find(|f| f.reason_code == REASON_RES_STATUS_CHECK_FAILURE_TELEMETRY)
            .expect("status check telemetry finding");
        assert_eq!(status.severity, Severity::High);
        assert_eq!(status.evidence["metric_name"], json!("StatusCheckFailed"));
        assert_eq!(status.evidence["max"], json!(1.0));
    }

    #[test]
    fn ec2_telemetry_performance_requires_core_metrics_and_flags_high_cpu() {
        let missing = fixture(
            "i-missing-telemetry",
            json!({"owner": "sre"}),
            json!({
                "state": "running",
                "availability_zone": "us-east-1a",
                "cloudwatch_metrics": {
                    "metrics": [
                        metric("CPUUtilization", &[35.0])
                    ]
                }
            }),
            1,
            now(),
        );
        let hot = fixture(
            "i-hot",
            json!({"owner": "sre"}),
            json!({
                "state": "running",
                "availability_zone": "us-east-1a",
                "cloudwatch_metrics": {
                    "metrics": [
                        metric("CPUUtilization", &[91.0, 94.0]),
                        metric("NetworkIn", &[1024.0]),
                        metric("NetworkOut", &[2048.0]),
                        metric("DiskReadOps", &[10.0]),
                        metric("DiskWriteOps", &[12.0])
                    ]
                }
            }),
            1,
            now(),
        );

        let report = evaluate_ec2_fleet(&[missing, hot], Pillar::Performance, now());
        let codes = reason_codes(&report);
        assert!(codes.contains(&REASON_PERF_MISSING_CORE_TELEMETRY));
        assert!(codes.contains(&REASON_PERF_HIGH_CPU_TELEMETRY));
    }

    #[test]
    fn ec2_telemetry_scalability_requires_demand_metrics_and_flags_cpu_pressure() {
        let missing = fixture(
            "i-scale-gap",
            json!({"owner": "sre"}),
            json!({
                "state": "running",
                "availability_zone": "us-east-1a",
                "cloudwatch_metrics": {
                    "metrics": [
                        metric("CPUUtilization", &[42.0])
                    ]
                }
            }),
            1,
            now(),
        );
        let pressured = fixture(
            "i-scale-hot",
            json!({"owner": "sre"}),
            json!({
                "state": "running",
                "availability_zone": "us-east-1a",
                "cloudwatch_metrics": {
                    "metrics": [
                        metric("CPUUtilization", &[83.0, 91.0]),
                        metric("NetworkIn", &[2048.0]),
                        metric("NetworkOut", &[1024.0])
                    ]
                }
            }),
            1,
            now(),
        );

        let report = evaluate_ec2_fleet(&[missing, pressured], Pillar::Scalability, now());
        let codes = reason_codes(&report);
        assert!(codes.contains(&REASON_SCALE_MISSING_DEMAND_TELEMETRY));
        assert!(codes.contains(&REASON_SCALE_HIGH_CPU_PRESSURE_TELEMETRY));
    }

    #[test]
    fn ec2_telemetry_security_requires_packet_metrics_and_flags_public_traffic() {
        let missing = fixture(
            "i-packet-gap",
            json!({"owner": "sre"}),
            json!({
                "state": "running",
                "private_ip": "10.0.0.5",
                "availability_zone": "us-east-1a",
                "cloudwatch_metrics": {
                    "metrics": [
                        metric("CPUUtilization", &[22.0])
                    ]
                }
            }),
            1,
            now(),
        );
        let exposed = fixture(
            "i-public-packets",
            json!({"owner": "sre"}),
            json!({
                "state": "running",
                "public_ip": "54.0.0.1",
                "availability_zone": "us-east-1a",
                "cloudwatch_metrics": {
                    "metrics": [
                        metric("NetworkPacketsIn", &[0.0, 1250.0]),
                        metric("NetworkPacketsOut", &[400.0])
                    ]
                }
            }),
            1,
            now(),
        );

        let report = evaluate_ec2_fleet(&[missing, exposed], Pillar::Security, now());
        let codes = reason_codes(&report);
        assert!(codes.contains(&REASON_SEC_MISSING_PACKET_TELEMETRY));
        assert!(codes.contains(&REASON_SEC_PUBLIC_PACKET_TRAFFIC_TELEMETRY));
    }

    #[test]
    fn ec2_telemetry_disaster_recovery_requires_recent_recovery_point_evidence() {
        let missing = fixture(
            "i-no-recovery-evidence",
            json!({"owner": "sre"}),
            json!({
                "state": "running",
                "availability_zone": "us-east-1a",
                "cloudwatch_metrics": {
                    "metrics": [
                        metric("StatusCheckFailed", &[0.0])
                    ]
                }
            }),
            1,
            now(),
        );
        let stale = fixture(
            "i-stale-recovery",
            json!({"owner": "sre"}),
            json!({
                "state": "running",
                "availability_zone": "us-east-1b",
                "latest_recovery_point_age_hours": 72
            }),
            1,
            now(),
        );

        let report = evaluate_ec2_fleet(&[missing, stale], Pillar::DisasterRecovery, now());
        let codes = reason_codes(&report);
        assert!(codes.contains(&REASON_DR_MISSING_RECOVERY_POINT_TELEMETRY));
        assert!(codes.contains(&REASON_DR_STALE_RECOVERY_POINT_TELEMETRY));
    }

    #[test]
    fn ec2_telemetry_operational_excellence_flags_collection_errors_and_basic_monitoring() {
        let missing_metadata = fixture(
            "i-no-collection-metadata",
            json!({"owner": "sre"}),
            json!({
                "state": "running",
                "availability_zone": "us-east-1a",
                "monitoring_state": "enabled",
                "cloudwatch_metrics": {
                    "metrics": [
                        metric("CPUUtilization", &[24.0])
                    ]
                }
            }),
            1,
            now(),
        );
        let collection_error = fixture(
            "i-collection-error",
            json!({"owner": "sre"}),
            json!({
                "state": "running",
                "availability_zone": "us-east-1a",
                "monitoring_state": "disabled",
                "telemetry_collection_started_at": "2026-06-10T00:00:00Z",
                "telemetry_collection_completed_at": "2026-06-10T00:00:03Z",
                "telemetry_collection_duration_ms": 3000,
                "telemetry_collection_error_count": 1,
                "telemetry_collection_errors": [
                    {
                        "source": "cloudwatch",
                        "operation": "GetMetricData",
                        "error": "throttled"
                    }
                ]
            }),
            1,
            now(),
        );

        let report = evaluate_ec2_fleet(
            &[missing_metadata, collection_error],
            Pillar::OperationalExcellence,
            now(),
        );
        let codes = reason_codes(&report);
        assert!(codes.contains(&REASON_OE_MISSING_TELEMETRY_COLLECTION_METADATA));
        assert!(codes.contains(&REASON_OE_TELEMETRY_COLLECTION_ERRORS));
        assert!(codes.contains(&REASON_OE_BASIC_MONITORING));
    }

    #[test]
    fn ec2_telemetry_operational_excellence_passes_for_successful_collection() {
        let resource = fixture(
            "i-collection-good",
            json!({"owner": "sre"}),
            json!({
                "state": "running",
                "availability_zone": "us-east-1a",
                "monitoring_state": "enabled",
                "telemetry_collection_started_at": "2026-06-10T00:00:00Z",
                "telemetry_collection_completed_at": "2026-06-10T00:00:03Z",
                "telemetry_collection_duration_ms": 3000,
                "telemetry_collection_success_count": 1,
                "telemetry_collection_failure_count": 0,
                "telemetry_collection_error_count": 0,
                "telemetry_collection_errors": [],
                "cloudwatch_metric_count": 3,
                "cloudwatch_metrics": {
                    "metrics": [
                        metric("CPUUtilization", &[24.0]),
                        metric("NetworkIn", &[2048.0]),
                        metric("NetworkOut", &[1024.0])
                    ]
                }
            }),
            1,
            now(),
        );

        let report = evaluate_ec2_fleet(&[resource], Pillar::OperationalExcellence, now());
        assert!(
            report.findings.is_empty(),
            "unexpected: {:?}",
            report.findings
        );
    }
}

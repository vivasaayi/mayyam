import React, { act } from "react";
import { createRoot } from "react-dom/client";

import PillarScorecard from "./PillarScorecard";

globalThis.IS_REACT_ACT_ENVIRONMENT = true;

const render = async (ui) => {
  const container = document.createElement("div");
  document.body.appendChild(container);
  const root = createRoot(container);
  await act(async () => {
    root.render(ui);
  });
  return {
    container,
    unmount: async () => {
      await act(async () => {
        root.unmount();
      });
      container.remove();
    },
  };
};

describe("PillarScorecard", () => {
  it("renders Auto Scaling cost posture rules", async () => {
    const data = {
      evaluated_at: "2026-06-18T05:00:00Z",
      stale_after_hours: 24,
      reports: [
        {
          pillar: "cost",
          score: 64,
          resources_evaluated: 2,
          stale_resources: 0,
          findings: [],
          assessment_scope: "autoscaling_cost_capacity_tags_and_group_metrics",
          posture: {
            status: "fail",
            rules_evaluated: 7,
            rules_failed: 3,
            affected_resources: ["asg-missing-tags", "asg-fixed"],
            rules: [
              {
                rule_id: "asg-cost-inventory-freshness",
                status: "pass",
                reason_codes: ["ASG_INV_STALE_DATA"],
                affected_resources: [],
                suppression_supported: true,
                assignment_supported: true,
              },
              {
                rule_id: "asg-cost-telemetry-collection-metadata-present",
                status: "pass",
                reason_codes: ["ASG_TEL_MISSING_COLLECTION_METADATA"],
                affected_resources: [],
                suppression_supported: true,
                assignment_supported: true,
              },
              {
                rule_id: "asg-cost-telemetry-collection-errors-clear",
                status: "pass",
                reason_codes: ["ASG_TEL_COLLECTION_ERRORS"],
                affected_resources: [],
                suppression_supported: true,
                assignment_supported: true,
              },
              {
                rule_id: "asg-cost-capacity-telemetry-present",
                status: "fail",
                reason_codes: ["ASG_COST_MISSING_CAPACITY_TELEMETRY"],
                affected_resources: ["asg-missing-tags"],
                suppression_supported: true,
                assignment_supported: true,
              },
              {
                rule_id: "asg-cost-group-metrics-telemetry-present",
                status: "pass",
                reason_codes: ["ASG_COST_MISSING_GROUP_METRICS_TELEMETRY"],
                affected_resources: [],
                suppression_supported: true,
                assignment_supported: true,
              },
              {
                rule_id: "asg-cost-allocation-tags-present",
                status: "fail",
                reason_codes: ["ASG_COST_NO_TAGS"],
                affected_resources: ["asg-missing-tags"],
                suppression_supported: true,
                assignment_supported: true,
              },
              {
                rule_id: "asg-cost-scale-in-capable",
                status: "fail",
                reason_codes: ["ASG_COST_FIXED_SIZE"],
                affected_resources: ["asg-fixed"],
                suppression_supported: true,
                assignment_supported: true,
              },
            ],
          },
          triage_context: {
            workflow_id: "autoscaling_cost_triage_context",
            pillar: "cost",
            context_builder_id: "autoscaling-cost-deterministic-context-v1",
            prompt_template_id: "autoscaling-cost-ai-triage-v1",
            generation_mode: "deterministic_no_llm",
            max_prompt_tokens: 1200,
            provider_routing: ["primary_ops_llm", "fallback_ops_llm"],
            audit_event_type: "autoscaling_ai_triage_context_built",
            guardrails: {
              read_only_mode: true,
              evidence_required: true,
              separate_facts_from_hypotheses: true,
              ask_for_missing_data: true,
              no_llm_invocation: true,
              no_mutation_planning: true,
            },
            facts: [
              "ASG_COST_NO_TAGS affects asg-missing-tags with Medium severity",
            ],
            hypotheses: [
              "asg-fixed may be paying for fixed capacity because scale-in is disabled by min == max",
            ],
            missing_data_questions: [
              "Collect capacity telemetry for asg-missing-tags before quantifying ASG cost posture",
            ],
            evidence_citations: [
              {
                reason_code: "ASG_COST_NO_TAGS",
                resource_id: "asg-missing-tags",
                severity: "medium",
                evidence: { tags: {} },
              },
            ],
          },
          agentic_investigation: {
            workflow_id: "autoscaling_cost_agentic_investigation",
            default_tool_mode: "read_only",
            max_tool_calls: 4,
            max_evidence_citations: 2,
            replay_required: true,
            steps: [
              {
                step_id: "autoscaling-cost-step-01",
                kind: "inspect",
                tool_name: "autoscaling.describe_group_capacity",
                tool_mode: "read_only",
                target_resource_id: "asg-missing-tags",
                reason_code: "ASG_COST_MISSING_CAPACITY_TELEMETRY",
                stop_condition:
                  "stop when min, max, desired, and instance counts are recorded",
                evidence: { missing_fields: ["desired_capacity"] },
              },
              {
                step_id: "autoscaling-cost-step-02",
                kind: "compare",
                tool_name: "autoscaling.compare_scaling_policy_capacity",
                tool_mode: "read_only",
                target_resource_id: "asg-fixed",
                reason_code: "ASG_COST_FIXED_SIZE",
                stop_condition:
                  "stop when fixed capacity is compared with demand and scaling policy evidence",
                evidence: { min_size: 3, max_size: 3 },
              },
              {
                step_id: "autoscaling-cost-step-03",
                kind: "propose_mutation_plan",
                tool_name: "autoscaling.cost.prepare_approval_plan",
                tool_mode: "approval_required",
                target_resource_id: "investigation",
                reason_code: "ASG_COST_APPROVAL_PLAN_REQUIRED",
                stop_condition:
                  "stop before mutation; require operator approval and rollback note",
                evidence: { approval_gate_count: 1 },
              },
            ],
            approval_gates: [
              {
                gate_id: "autoscaling-cost-approval-01",
                target_resource_id: "asg-fixed",
                required_approval:
                  "Approve scaling policy or capacity changes after owner review",
                blast_radius:
                  "single Auto Scaling group asg-fixed; no mutation is executable from the investigation plan",
                rollback_note_required: true,
                evidence_reason_codes: ["ASG_COST_FIXED_SIZE"],
              },
            ],
            evidence_citations: [
              {
                reason_code: "ASG_COST_FIXED_SIZE",
                resource_id: "asg-fixed",
                severity: "low",
                evidence: { min_size: 3, max_size: 3 },
              },
            ],
          },
        },
      ],
    };

    const view = await render(<PillarScorecard data={data} />);
    const text = view.container.textContent;

    expect(text).toContain("Cost Posture");
    expect(text).toContain("3 failed of 7");
    expect(text).toContain("asg-cost-capacity-telemetry-present");
    expect(text).toContain("asg-cost-allocation-tags-present");
    expect(text).toContain("asg-cost-scale-in-capable");
    expect(text).toContain("ASG_COST_MISSING_CAPACITY_TELEMETRY");
    expect(text).toContain("ASG_COST_NO_TAGS");
    expect(text).toContain("ASG_COST_FIXED_SIZE");
    expect(text).toContain("asg-missing-tags");
    expect(text).toContain("asg-fixed");
    expect(text).toContain("Cost Triage Context");
    expect(text).toContain("autoscaling-cost-deterministic-context-v1");
    expect(text).toContain("autoscaling-cost-ai-triage-v1");
    expect(text).toContain("Provider Routing (not invoked)");
    expect(text).toContain("primary_ops_llm");
    expect(text).toContain("1200 token budget");
    expect(text).toContain("Read only");
    expect(text).toContain("Deterministic context only");
    expect(text).toContain("scale-in is disabled");
    expect(text).toContain("Collect capacity telemetry");
    expect(text).toContain("Cost Agentic Investigation");
    expect(text).toContain("autoscaling_cost_agentic_investigation");
    expect(text).toContain("Replay required");
    expect(text).toContain("4 max tool call");
    expect(text).toContain("2 evidence citation");
    expect(text).toContain("autoscaling.describe_group_capacity");
    expect(text).toContain("autoscaling.compare_scaling_policy_capacity");
    expect(text).toContain("autoscaling.cost.prepare_approval_plan");
    expect(text).toContain("Approval Required");
    expect(text).toContain("ASG_COST_APPROVAL_PLAN_REQUIRED");
    expect(text).toContain("autoscaling-cost-approval-01");
    expect(text).toContain("no mutation is executable from the investigation plan");
    expect(text).toContain("Rollback note required");

    await view.unmount();
  });

  it("renders EC2 resilience reporting status, gaps, and recovery notes", async () => {
    const data = {
      evaluated_at: "2026-06-18T05:00:00Z",
      stale_after_hours: 24,
      reports: [
        {
          pillar: "resilience",
          score: 72,
          resources_evaluated: 2,
          stale_resources: 1,
          findings: [],
          reporting: {
            workflow_id: "ec2_resilience_reporting",
            read_only_mode: true,
            scheduled_delivery_state: "blocked_until_fresh_resilience_evidence",
            portfolio_summary_ready: false,
            workload_summary_ready: false,
            stale_data_blocks_delivery: true,
            executive_summary: {
              report_id: "ec2-resilience-executive-summary",
              rules_failed: 2,
              affected_resources: ["i-a"],
              blast_radius_summary:
                "single_az_placement_can_turn_one_az_event_into_fleet_outage",
              top_reason_codes: ["EC2_RES_SINGLE_AZ_CONCENTRATION"],
            },
            incident_review: {
              report_id: "ec2-resilience-incident-review",
              rows: [
                {
                  reason_code: "EC2_RES_SINGLE_AZ_CONCENTRATION",
                  resource_id: "fleet",
                  recovery_note:
                    "Review multi-AZ placement plan before any approved recovery change.",
                  suppression_supported: true,
                },
              ],
            },
            missing_data_reason_codes: ["EC2_INV_STALE_DATA"],
            evidence_reason_codes: ["EC2_RES_SINGLE_AZ_CONCENTRATION"],
          },
        },
      ],
    };

    const view = await render(<PillarScorecard data={data} />);
    const text = view.container.textContent;

    expect(text).toContain("Resilience Reporting");
    expect(text).toContain("Blocked Until Fresh Resilience Evidence");
    expect(text).toContain("Single Az Placement Can Turn One Az Event Into Fleet Outage");
    expect(text).toContain("1 missing signal(s)");
    expect(text).toContain("Fresh resilience evidence required");
    expect(text).toContain("Review multi-AZ placement plan");
    expect(text).toContain("Supported");

    await view.unmount();
  });

  it("renders EC2 disaster-recovery posture and triage context", async () => {
    const data = {
      evaluated_at: "2026-06-18T05:00:00Z",
      stale_after_hours: 24,
      reports: [
        {
          pillar: "disaster-recovery",
          score: 68,
          resources_evaluated: 2,
          stale_resources: 0,
          findings: [],
          posture: {
            status: "fail",
            rules_evaluated: 3,
            rules_failed: 2,
            affected_resources: ["i-no-recovery-evidence", "i-stale-recovery"],
            rules: [
              {
                rule_id: "ec2-disaster-recovery-inventory-freshness",
                status: "pass",
                reason_codes: ["EC2_INV_STALE_DATA"],
                affected_resources: [],
              },
              {
                rule_id:
                  "ec2-disaster-recovery-recovery-point-telemetry-present",
                status: "fail",
                reason_codes: ["EC2_DR_MISSING_RECOVERY_POINT_TELEMETRY"],
                affected_resources: ["i-no-recovery-evidence"],
              },
              {
                rule_id: "ec2-disaster-recovery-recovery-point-freshness",
                status: "fail",
                reason_codes: ["EC2_DR_STALE_RECOVERY_POINT_TELEMETRY"],
                affected_resources: ["i-stale-recovery"],
              },
            ],
          },
          triage_context: {
            workflow_id: "ec2_disaster_recovery_triage_context",
            context_builder_id:
              "ec2-disaster-recovery-deterministic-context-v1",
            prompt_template_id: "ec2-disaster-recovery-ai-triage-v1",
            generation_mode: "deterministic_no_llm",
            max_prompt_tokens: 1200,
            provider_routing: ["primary_ops_llm", "fallback_ops_llm"],
            guardrails: {
              read_only_mode: true,
              evidence_required: true,
              separate_facts_from_hypotheses: true,
              ask_for_missing_data: true,
              no_llm_invocation: true,
              no_mutation_planning: true,
            },
            facts: [
              "EC2_DR_MISSING_RECOVERY_POINT_TELEMETRY affects i-no-recovery-evidence with Medium severity",
              "EC2_DR_STALE_RECOVERY_POINT_TELEMETRY affects i-stale-recovery with High severity",
            ],
            hypotheses: [
              "i-stale-recovery has stale recovery point telemetry; verify backup policy before workflow changes",
            ],
            missing_data_questions: [
              "Collect latest recovery point age or timestamp evidence for i-no-recovery-evidence before judging EC2 disaster-recovery posture",
            ],
            evidence_citations: [
              {
                reason_code: "EC2_DR_STALE_RECOVERY_POINT_TELEMETRY",
                resource_id: "i-stale-recovery",
                severity: "high",
                evidence: { latest_recovery_point_age_hours: 72 },
              },
            ],
          },
        },
      ],
    };

    const view = await render(<PillarScorecard data={data} />);
    const text = view.container.textContent;

    expect(text).toContain("Disaster-Recovery Posture");
    expect(text).toContain(
      "ec2-disaster-recovery-recovery-point-telemetry-present",
    );
    expect(text).toContain("EC2_DR_MISSING_RECOVERY_POINT_TELEMETRY");
    expect(text).toContain("Disaster-Recovery Triage Context");
    expect(text).toContain(
      "ec2-disaster-recovery-deterministic-context-v1",
    );
    expect(text).toContain("ec2-disaster-recovery-ai-triage-v1");
    expect(text).toContain("backup policy");
    expect(text).toContain("latest recovery point age");
    expect(text).toContain("i-stale-recovery");

    await view.unmount();
  });

  it("renders EC2 operational-excellence posture", async () => {
    const data = {
      evaluated_at: "2026-06-18T05:00:00Z",
      stale_after_hours: 24,
      reports: [
        {
          pillar: "operational-excellence",
          score: 71,
          resources_evaluated: 2,
          stale_resources: 0,
          findings: [],
          posture: {
            status: "fail",
            rules_evaluated: 4,
            rules_failed: 3,
            affected_resources: [
              "i-no-collection-metadata",
              "i-collection-error",
            ],
            rules: [
              {
                rule_id: "ec2-operational-excellence-inventory-freshness",
                status: "pass",
                reason_codes: ["EC2_INV_STALE_DATA"],
                affected_resources: [],
              },
              {
                rule_id:
                  "ec2-operational-excellence-collection-metadata-present",
                status: "fail",
                reason_codes: [
                  "EC2_OE_MISSING_TELEMETRY_COLLECTION_METADATA",
                ],
                affected_resources: ["i-no-collection-metadata"],
              },
              {
                rule_id: "ec2-operational-excellence-collection-errors-clear",
                status: "fail",
                reason_codes: ["EC2_OE_TELEMETRY_COLLECTION_ERRORS"],
                affected_resources: ["i-collection-error"],
              },
              {
                rule_id:
                  "ec2-operational-excellence-detailed-monitoring-enabled",
                status: "fail",
                reason_codes: ["EC2_OE_BASIC_MONITORING"],
                affected_resources: ["i-collection-error"],
              },
            ],
          },
          triage_context: {
            workflow_id: "ec2_operational_excellence_triage_context",
            context_builder_id:
              "ec2-operational-excellence-deterministic-context-v1",
            prompt_template_id: "ec2-operational-excellence-ai-triage-v1",
            generation_mode: "deterministic_no_llm",
            max_prompt_tokens: 1200,
            provider_routing: ["primary_ops_llm", "fallback_ops_llm"],
            guardrails: {
              read_only_mode: true,
              evidence_required: true,
              separate_facts_from_hypotheses: true,
              ask_for_missing_data: true,
              no_llm_invocation: true,
              no_mutation_planning: true,
            },
            facts: [
              "EC2_OE_MISSING_TELEMETRY_COLLECTION_METADATA affects i-no-collection-metadata with Medium severity",
              "EC2_OE_TELEMETRY_COLLECTION_ERRORS affects i-collection-error with High severity",
            ],
            hypotheses: [
              "i-collection-error has telemetry collection errors; inspect collector logs before changing runbook workflow",
              "i-collection-error uses basic EC2 monitoring; operational diagnosis may rely on lower-resolution telemetry",
            ],
            missing_data_questions: [
              "Collect telemetry collection metadata for i-no-collection-metadata before generating operational runbook triage",
            ],
            evidence_citations: [
              {
                reason_code: "EC2_OE_TELEMETRY_COLLECTION_ERRORS",
                resource_id: "i-collection-error",
                severity: "high",
                evidence: { telemetry_collection_error_count: 1 },
              },
            ],
          },
        },
      ],
    };

    const view = await render(<PillarScorecard data={data} />);
    const text = view.container.textContent;

    expect(text).toContain("Operational-Excellence Posture");
    expect(text).toContain(
      "ec2-operational-excellence-collection-metadata-present",
    );
    expect(text).toContain(
      "EC2_OE_MISSING_TELEMETRY_COLLECTION_METADATA",
    );
    expect(text).toContain("ec2-operational-excellence-collection-errors-clear");
    expect(text).toContain("EC2_OE_TELEMETRY_COLLECTION_ERRORS");
    expect(text).toContain("Operational-Excellence Triage Context");
    expect(text).toContain(
      "ec2-operational-excellence-deterministic-context-v1",
    );
    expect(text).toContain("ec2-operational-excellence-ai-triage-v1");
    expect(text).toContain("collector logs");
    expect(text).toContain("telemetry collection metadata");
    expect(text).toContain("i-collection-error");

    await view.unmount();
  });

  it("renders EC2 performance posture findings for operator review", async () => {
    const data = {
      evaluated_at: "2026-06-18T05:00:00Z",
      stale_after_hours: 24,
      reports: [
        {
          pillar: "performance",
          score: 74,
          resources_evaluated: 2,
          stale_resources: 0,
          posture: {
            status: "fail",
            rules_evaluated: 3,
            rules_failed: 2,
            affected_resources: ["i-perf-gap", "i-perf-hot"],
            rules: [
              {
                rule_id: "ec2-performance-inventory-freshness",
                status: "pass",
                reason_codes: ["EC2_INV_STALE_DATA"],
                affected_resources: [],
              },
              {
                rule_id: "ec2-performance-core-telemetry-present",
                status: "fail",
                reason_codes: ["EC2_PERF_MISSING_CORE_TELEMETRY"],
                affected_resources: ["i-perf-gap"],
              },
              {
                rule_id: "ec2-performance-cpu-headroom",
                status: "fail",
                reason_codes: ["EC2_PERF_HIGH_CPU_TELEMETRY"],
                affected_resources: ["i-perf-hot"],
              },
            ],
          },
          triage_context: {
            workflow_id: "ec2_performance_triage_context",
            context_builder_id: "ec2-performance-deterministic-context-v1",
            prompt_template_id: "ec2-performance-ai-triage-v1",
            generation_mode: "deterministic_no_llm",
            max_prompt_tokens: 1200,
            provider_routing: ["primary_ops_llm", "fallback_ops_llm"],
            guardrails: {
              read_only_mode: true,
              evidence_required: true,
              separate_facts_from_hypotheses: true,
              ask_for_missing_data: true,
              no_llm_invocation: true,
              no_mutation_planning: true,
            },
            facts: [
              "EC2_PERF_MISSING_CORE_TELEMETRY affects i-perf-gap with Medium severity",
              "EC2_PERF_HIGH_CPU_TELEMETRY affects i-perf-hot with High severity",
            ],
            hypotheses: [
              "i-perf-hot may be CPU constrained; compare instance type before resizing",
            ],
            missing_data_questions: [
              "Collect CPUUtilization, NetworkIn, NetworkOut, DiskReadOps, and DiskWriteOps telemetry for i-perf-gap before diagnosing EC2 performance bottlenecks",
            ],
            evidence_citations: [
              {
                reason_code: "EC2_PERF_HIGH_CPU_TELEMETRY",
                resource_id: "i-perf-hot",
                severity: "high",
                evidence: { metric_name: "CPUUtilization", max: 94 },
              },
            ],
          },
          forecasting: {
            workflow_id: "ec2_performance_forecasting",
            read_only_mode: true,
            baseline_window_days: 30,
            forecast_horizon_days: 30,
            confidence_level: 75,
            forecast_band: {
              horizon_days: 30,
              lower_performance_pressure_index: 128,
              expected_performance_pressure_index: 158,
              upper_performance_pressure_index: 188,
              confidence_level: 75,
            },
            risk_level: "high",
            performance_capacity_risk: "cpu_constrained_compute_capacity",
            backtesting_fixture_status: "ready_performance_findings_baseline",
            threshold_controls: [
              "performance_pressure_index_warning_threshold",
              "performance_pressure_index_critical_threshold",
            ],
            what_if_inputs: [
              "restore_core_performance_telemetry",
              "compare_cpu_pressure_to_workload_demand",
            ],
            blocked_by_stale_data: false,
            blast_radius_summary:
              "instances_with_high_cpu_can_expand_latency_or_throttle_risk",
            missing_data_reason_codes: ["EC2_PERF_MISSING_CORE_TELEMETRY"],
            risk_drivers: [
              {
                reason_code: "EC2_PERF_HIGH_CPU_TELEMETRY",
                affected_resources: ["i-perf-hot"],
                performance_pressure_index_delta: 38,
              },
            ],
            evidence_reason_codes: [
              "EC2_PERF_MISSING_CORE_TELEMETRY",
              "EC2_PERF_HIGH_CPU_TELEMETRY",
            ],
          },
          findings: [
            {
              severity: "medium",
              reason_code: "EC2_PERF_MISSING_CORE_TELEMETRY",
              resource_id: "i-perf-gap",
              message: "Instance i-perf-gap is missing core EC2 performance telemetry",
              evidence: { missing_metrics: ["NetworkIn"] },
            },
            {
              severity: "high",
              reason_code: "EC2_PERF_HIGH_CPU_TELEMETRY",
              resource_id: "i-perf-hot",
              message: "Instance i-perf-hot has high CPUUtilization telemetry",
              evidence: { metric_name: "CPUUtilization", max: 94 },
            },
          ],
        },
      ],
    };

    const view = await render(<PillarScorecard data={data} />);
    const text = view.container.textContent;

    expect(text).toContain("performance");
    expect(text).toContain("Performance Posture");
    expect(text).toContain("2 failed of 3");
    expect(text).toContain("ec2-performance-inventory-freshness");
    expect(text).toContain("ec2-performance-core-telemetry-present");
    expect(text).toContain("ec2-performance-cpu-headroom");
    expect(text).toContain("EC2_INV_STALE_DATA");
    expect(text).toContain("Performance Triage Context");
    expect(text).toContain("Deterministic No Llm");
    expect(text).toContain("ec2-performance-deterministic-context-v1");
    expect(text).toContain("ec2-performance-ai-triage-v1");
    expect(text).toContain("Provider Routing (not invoked)");
    expect(text).toContain("primary_ops_llm");
    expect(text).toContain("1200 token budget");
    expect(text).toContain("Read only");
    expect(text).toContain("Deterministic context only");
    expect(text).toContain("CPU constrained");
    expect(text).toContain("Performance Forecast");
    expect(text).toContain("ec2_performance_forecasting");
    expect(text).toContain("expected 158");
    expect(text).toContain("128-188");
    expect(text).toContain("Cpu Constrained Compute Capacity");
    expect(text).toContain("Ready Performance Findings Baseline");
    expect(text).toContain("instances_with_high_cpu_can_expand_latency_or_throttle_risk");
    expect(text).toContain("Performance Pressure Index Warning Threshold");
    expect(text).toContain("Collect CPUUtilization");
    expect(text).toContain("EC2_PERF_MISSING_CORE_TELEMETRY");
    expect(text).toContain("EC2_PERF_HIGH_CPU_TELEMETRY");
    expect(text).toContain("i-perf-gap");
    expect(text).toContain("i-perf-hot");
    expect(text).toContain("high CPUUtilization telemetry");

    await view.unmount();
  });

  it("renders EC2 scalability posture rule gaps for operator review", async () => {
    const data = {
      evaluated_at: "2026-06-18T05:45:00Z",
      stale_after_hours: 24,
      reports: [
        {
          pillar: "scalability",
          score: 68,
          resources_evaluated: 2,
          stale_resources: 0,
          posture: {
            status: "fail",
            rules_evaluated: 3,
            rules_failed: 2,
            affected_resources: ["i-scale-gap", "i-scale-hot"],
            rules: [
              {
                rule_id: "ec2-scalability-inventory-freshness",
                status: "pass",
                reason_codes: ["EC2_INV_STALE_DATA"],
                affected_resources: [],
              },
              {
                rule_id: "ec2-scalability-demand-telemetry-present",
                status: "fail",
                reason_codes: ["EC2_SCALE_MISSING_DEMAND_TELEMETRY"],
                affected_resources: ["i-scale-gap"],
              },
              {
                rule_id: "ec2-scalability-cpu-pressure",
                status: "fail",
                reason_codes: ["EC2_SCALE_HIGH_CPU_PRESSURE_TELEMETRY"],
                affected_resources: ["i-scale-hot"],
              },
            ],
          },
          triage_context: {
            workflow_id: "ec2_scalability_triage_context",
            context_builder_id: "ec2-scalability-deterministic-context-v1",
            prompt_template_id: "ec2-scalability-ai-triage-v1",
            generation_mode: "deterministic_no_llm",
            max_prompt_tokens: 1200,
            provider_routing: ["primary_ops_llm", "fallback_ops_llm"],
            guardrails: {
              read_only_mode: true,
              no_llm_invocation: true,
            },
            facts: [
              "EC2_SCALE_MISSING_DEMAND_TELEMETRY affects i-scale-gap with Medium severity",
              "EC2_SCALE_HIGH_CPU_PRESSURE_TELEMETRY affects i-scale-hot with High severity",
            ],
            hypotheses: [
              "i-scale-hot may need scale-out, workload distribution, or rightsizing",
            ],
            missing_data_questions: [
              "Collect CPUUtilization, NetworkIn, and NetworkOut telemetry for i-scale-gap before diagnosing EC2 scaling pressure",
            ],
            evidence_citations: [
              {
                reason_code: "EC2_SCALE_MISSING_DEMAND_TELEMETRY",
                resource_id: "i-scale-gap",
                severity: "medium",
                evidence: { missing_metrics: ["NetworkIn"] },
              },
              {
                reason_code: "EC2_SCALE_HIGH_CPU_PRESSURE_TELEMETRY",
                resource_id: "i-scale-hot",
                severity: "high",
                evidence: { metric_name: "CPUUtilization", max: 92 },
              },
            ],
          },
          findings: [
            {
              severity: "medium",
              reason_code: "EC2_SCALE_MISSING_DEMAND_TELEMETRY",
              resource_id: "i-scale-gap",
              message:
                "Instance i-scale-gap is missing EC2 demand telemetry needed to assess scaling pressure",
              evidence: { missing_metrics: ["NetworkIn"] },
            },
            {
              severity: "high",
              reason_code: "EC2_SCALE_HIGH_CPU_PRESSURE_TELEMETRY",
              resource_id: "i-scale-hot",
              message:
                "Instance i-scale-hot has high CPUUtilization telemetry; scale-out or rightsizing pressure is likely",
              evidence: { metric_name: "CPUUtilization", max: 92 },
            },
          ],
        },
      ],
    };

    const view = await render(<PillarScorecard data={data} />);
    const text = view.container.textContent;

    expect(text).toContain("scalability");
    expect(text).toContain("Scalability Posture");
    expect(text).toContain("2 failed of 3");
    expect(text).toContain("ec2-scalability-inventory-freshness");
    expect(text).toContain("ec2-scalability-demand-telemetry-present");
    expect(text).toContain("ec2-scalability-cpu-pressure");
    expect(text).toContain("EC2_INV_STALE_DATA");
    expect(text).toContain("EC2_SCALE_MISSING_DEMAND_TELEMETRY");
    expect(text).toContain("EC2_SCALE_HIGH_CPU_PRESSURE_TELEMETRY");
    expect(text).toContain("i-scale-gap");
    expect(text).toContain("i-scale-hot");
    expect(text).toContain("scale-out or rightsizing pressure");
    expect(text).toContain("Scalability Triage Context");
    expect(text).toContain("ec2-scalability-deterministic-context-v1");
    expect(text).toContain("ec2-scalability-ai-triage-v1");
    expect(text).toContain("Deterministic No Llm");
    expect(text).toContain("Provider Routing (not invoked)");
    expect(text).toContain("Read only");
    expect(text).toContain("Deterministic context only");
    expect(text).toContain("workload distribution");
    expect(text).toContain("before diagnosing EC2 scaling pressure");
    expect(text).toContain("Evidence Citations");
    expect(text).toContain("EC2_SCALE_HIGH_CPU_PRESSURE_TELEMETRY");
    expect(text).toContain('"metric_name":"CPUUtilization"');
    expect(text).toContain('"missing_metrics":["NetworkIn"]');

    await view.unmount();
  });

  it("renders EC2 security posture rule gaps for operator review", async () => {
    const data = {
      evaluated_at: "2026-06-18T06:10:00Z",
      stale_after_hours: 24,
      reports: [
        {
          pillar: "security",
          score: 62,
          resources_evaluated: 2,
          stale_resources: 0,
          posture: {
            status: "fail",
            rules_evaluated: 5,
            rules_failed: 4,
            affected_resources: ["i-sec-exposed", "i-sec-gap"],
            rules: [
              {
                rule_id: "ec2-security-inventory-freshness",
                status: "pass",
                reason_codes: ["EC2_INV_STALE_DATA"],
                affected_resources: [],
              },
              {
                rule_id: "ec2-security-public-ip-exposure",
                status: "fail",
                reason_codes: ["EC2_SEC_PUBLIC_IP_ASSIGNED"],
                affected_resources: ["i-sec-exposed"],
              },
              {
                rule_id: "ec2-security-owner-routing-present",
                status: "fail",
                reason_codes: ["EC2_SEC_MISSING_OWNER_TAG"],
                affected_resources: ["i-sec-exposed"],
              },
              {
                rule_id: "ec2-security-packet-telemetry-present",
                status: "fail",
                reason_codes: ["EC2_SEC_MISSING_PACKET_TELEMETRY"],
                affected_resources: ["i-sec-gap"],
              },
              {
                rule_id: "ec2-security-public-packet-traffic",
                status: "fail",
                reason_codes: ["EC2_SEC_PUBLIC_PACKET_TRAFFIC_TELEMETRY"],
                affected_resources: ["i-sec-exposed"],
              },
            ],
          },
          triage_context: {
            workflow_id: "ec2_security_triage_context",
            context_builder_id: "ec2-security-deterministic-context-v1",
            prompt_template_id: "ec2-security-ai-triage-v1",
            generation_mode: "deterministic_no_llm",
            max_prompt_tokens: 1200,
            provider_routing: ["primary_ops_llm", "fallback_ops_llm"],
            guardrails: {
              read_only_mode: true,
              evidence_required: true,
              separate_facts_from_hypotheses: true,
              ask_for_missing_data: true,
              no_llm_invocation: true,
              no_mutation_planning: true,
            },
            facts: [
              "EC2_SEC_PUBLIC_IP_ASSIGNED affects i-sec-exposed with High severity",
              "EC2_SEC_MISSING_PACKET_TELEMETRY affects i-sec-gap with Medium severity",
            ],
            hypotheses: [
              "i-sec-exposed has a public IP assignment in collected EC2 evidence; verify security groups, network ACLs, route tables, and business intent before treating it as internet-reachable exposure",
              "i-sec-exposed has packet telemetry alongside public IP evidence; compare flow logs, security groups, and allowed ingress before recommending any access change",
            ],
            missing_data_questions: [
              "Assign owner, team, or service metadata for i-sec-exposed before routing security posture follow-up",
              "Collect NetworkPacketsIn and NetworkPacketsOut telemetry for i-sec-gap before judging observed packet exposure",
            ],
            evidence_citations: [
              {
                reason_code: "EC2_SEC_PUBLIC_IP_ASSIGNED",
                resource_id: "i-sec-exposed",
                severity: "high",
                evidence: { public_ip: "54.0.0.1" },
              },
              {
                reason_code: "EC2_SEC_MISSING_PACKET_TELEMETRY",
                resource_id: "i-sec-gap",
                severity: "medium",
                evidence: { missing_metrics: ["NetworkPacketsIn"] },
              },
            ],
          },
          agentic_investigation: {
            workflow_id: "ec2_security_agentic_investigation",
            default_tool_mode: "read_only",
            max_tool_calls: 5,
            max_evidence_citations: 2,
            replay_required: true,
            steps: [
              {
                step_id: "ec2-security-step-01",
                kind: "inspect",
                tool_name: "ec2.describe_instance_networking",
                tool_mode: "read_only",
                target_resource_id: "i-sec-exposed",
                reason_code: "EC2_SEC_PUBLIC_IP_ASSIGNED",
                stop_condition:
                  "stop when public IP, security group, subnet route table, and internet gateway evidence are recorded or confirmed absent",
              },
              {
                step_id: "ec2-security-step-02",
                kind: "compare",
                tool_name: "ec2.compare_security_group_ingress",
                tool_mode: "read_only",
                target_resource_id: "i-sec-exposed",
                reason_code: "EC2_SEC_PUBLIC_IP_ASSIGNED",
                stop_condition:
                  "stop when allowed ingress is compared against owner intent and known exposure exceptions",
              },
              {
                step_id: "ec2-security-step-03",
                kind: "propose_mutation_plan",
                tool_name: "ec2.security.prepare_approval_plan",
                tool_mode: "approval_required",
                target_resource_id: "investigation",
                reason_code: "EC2_SECURITY_APPROVAL_PLAN_REQUIRED",
                stop_condition:
                  "stop before mutation; require explicit operator approval, blast-radius summary, rollback note, and replayable evidence",
              },
            ],
            approval_gates: [
              {
                gate_id: "ec2-security-approval-01",
                target_resource_id: "i-sec-exposed",
                required_approval:
                  "Approve any security group, route, public IP, or exposure suppression change only after owner review, blast-radius summary, and rollback note",
                blast_radius:
                  "single EC2 instance i-sec-exposed; no mutation is executable from the investigation plan",
                rollback_note_required: true,
                evidence_reason_codes: ["EC2_SEC_PUBLIC_IP_ASSIGNED"],
              },
            ],
            evidence_citations: [
              {
                reason_code: "EC2_SEC_PUBLIC_IP_ASSIGNED",
                resource_id: "i-sec-exposed",
                severity: "high",
                evidence: { public_ip: "54.0.0.1" },
              },
              {
                reason_code: "EC2_SEC_MISSING_PACKET_TELEMETRY",
                resource_id: "i-sec-gap",
                severity: "medium",
                evidence: { missing_metrics: ["NetworkPacketsIn"] },
              },
            ],
          },
          remediation_workflow: {
            workflow_id: "ec2_security_safe_remediation",
            read_only_mode: true,
            rbac_permission: "aws.ec2.security.remediation.approve",
            audit_stream: "ec2_security_remediation_audit",
            stale_data_blocks_execution: false,
            actions: [
              {
                action_id: "ec2-security-remediation-01",
                kind: "review_security_exposure",
                status: "dry_run_pending_approval",
                target_resource_id: "i-sec-exposed",
                dry_run: true,
                requires_approval: true,
                approval_gate_id: "ec2-security-approval-01",
                audit_event_type: "ec2.security.remediation.dry_run_planned",
                rollback_note:
                  "Before approval, record rollback or recovery notes for review-security-exposure on i-sec-exposed.",
                evidence_reason_codes: ["EC2_SEC_PUBLIC_IP_ASSIGNED"],
              },
              {
                action_id: "ec2-security-remediation-02",
                kind: "review_security_owner_metadata",
                status: "dry_run_pending_approval",
                target_resource_id: "i-sec-exposed",
                dry_run: true,
                requires_approval: true,
                approval_gate_id: "ec2-security-approval-02",
                audit_event_type: "ec2.security.remediation.dry_run_planned",
                rollback_note:
                  "Before approval, record rollback or recovery notes for review-security-owner-metadata on i-sec-exposed.",
                evidence_reason_codes: ["EC2_SEC_MISSING_OWNER_TAG"],
              },
            ],
            approval_gates: [
              {
                gate_id: "ec2-security-approval-01",
                target_resource_id: "i-sec-exposed",
                required_approval:
                  "Approve any security group, route, public IP, or exposure suppression change only after owner review, blast-radius summary, and rollback note",
                blast_radius:
                  "single EC2 instance i-sec-exposed; no mutation is executable from the investigation plan",
                rollback_note_required: true,
                evidence_reason_codes: ["EC2_SEC_PUBLIC_IP_ASSIGNED"],
              },
              {
                gate_id: "ec2-security-approval-02",
                target_resource_id: "i-sec-exposed",
                required_approval:
                  "Approve tag writes or assignment changes after ownership is verified",
                blast_radius:
                  "single EC2 instance i-sec-exposed; no mutation is executable from the investigation plan",
                rollback_note_required: true,
                evidence_reason_codes: ["EC2_SEC_MISSING_OWNER_TAG"],
              },
            ],
          },
          slo_policy_tracking: {
            workflow_id: "ec2_security_slo_policy",
            read_only_mode: true,
            freshness_required: true,
            objective: {
              objective_id: "ec2-security-score-min-95",
              status: "breached",
              target_score_min: 95,
              current_score: 55,
              trend_direction: "degrading",
              failed_rule_count: 4,
              affected_resource_count: 2,
              owner_filters: ["security"],
              environment_filters: ["prod"],
              application_filters: ["payments"],
              notification_targets: ["environment:prod", "owner:security"],
              policy_state: "active_with_findings",
              status_history: [
                "snapshot_collected",
                "security_policy_evaluated",
                "notification_targets_resolved",
              ],
            },
            evidence_reason_codes: [
              "EC2_SEC_PUBLIC_IP_ASSIGNED",
              "EC2_SEC_PUBLIC_PACKET_TRAFFIC_TELEMETRY",
            ],
          },
          forecasting: {
            workflow_id: "ec2_security_forecasting",
            read_only_mode: true,
            baseline_window_days: 30,
            forecast_horizon_days: 30,
            confidence_level: 75,
            forecast_band: {
              horizon_days: 30,
              lower_security_exposure_index: 140,
              expected_security_exposure_index: 168,
              upper_security_exposure_index: 196,
              confidence_level: 75,
            },
            risk_level: "high",
            exposure_capacity_risk: "public_exposure_with_observed_packet_traffic",
            backtesting_fixture_status: "ready_security_findings_baseline",
            threshold_controls: [
              "security_exposure_index_warning_threshold",
              "security_exposure_index_critical_threshold",
            ],
            what_if_inputs: [
              "verify_public_ip_business_intent",
              "restore_packet_telemetry",
            ],
            blocked_by_stale_data: false,
            blast_radius_summary:
              "public_ip_instances_with_packet_traffic_need_sg_nacl_route_verification",
            missing_data_reason_codes: [],
            risk_drivers: [
              {
                reason_code: "EC2_SEC_PUBLIC_PACKET_TRAFFIC_TELEMETRY",
                affected_resources: ["i-sec-exposed"],
                security_exposure_index_delta: 40,
              },
            ],
            evidence_reason_codes: [
              "EC2_SEC_PUBLIC_IP_ASSIGNED",
              "EC2_SEC_PUBLIC_PACKET_TRAFFIC_TELEMETRY",
            ],
          },
          findings: [
            {
              severity: "high",
              reason_code: "EC2_SEC_PUBLIC_IP_ASSIGNED",
              resource_id: "i-sec-exposed",
              message:
                "Instance i-sec-exposed has a public IP address assigned; verify it is intentionally internet-facing",
              evidence: { public_ip: "54.0.0.1" },
            },
            {
              severity: "medium",
              reason_code: "EC2_SEC_MISSING_PACKET_TELEMETRY",
              resource_id: "i-sec-gap",
              message:
                "Instance i-sec-gap is missing EC2 packet telemetry; network exposure cannot be verified from evidence",
              evidence: { missing_metrics: ["NetworkPacketsIn"] },
            },
          ],
        },
      ],
    };

    const view = await render(<PillarScorecard data={data} />);
    const text = view.container.textContent;

    expect(text).toContain("security");
    expect(text).toContain("Security Posture");
    expect(text).toContain("4 failed of 5");
    expect(text).toContain("ec2-security-public-ip-exposure");
    expect(text).toContain("ec2-security-owner-routing-present");
    expect(text).toContain("ec2-security-packet-telemetry-present");
    expect(text).toContain("ec2-security-public-packet-traffic");
    expect(text).toContain("EC2_SEC_PUBLIC_IP_ASSIGNED");
    expect(text).toContain("EC2_SEC_MISSING_OWNER_TAG");
    expect(text).toContain("EC2_SEC_MISSING_PACKET_TELEMETRY");
    expect(text).toContain("EC2_SEC_PUBLIC_PACKET_TRAFFIC_TELEMETRY");
    expect(text).toContain("Security SLO Policy");
    expect(text).toContain("Breached");
    expect(text).toContain("ec2-security-score-min-95");
    expect(text).toContain("score 55 / target 95");
    expect(text).toContain("Active With Findings");
    expect(text).toContain("Degrading trend");
    expect(text).toContain("security");
    expect(text).toContain("prod");
    expect(text).toContain("environment:prod, owner:security");
    expect(text).toContain("payments");
    expect(text).toContain("Security Policy Evaluated");
    expect(text).toContain("Security Forecast");
    expect(text).toContain("High");
    expect(text).toContain("ec2_security_forecasting");
    expect(text).toContain("30d baseline");
    expect(text).toContain("30d horizon");
    expect(text).toContain("expected 168");
    expect(text).toContain("140-196");
    expect(text).toContain("75% confidence");
    expect(text).toContain("Public Exposure With Observed Packet Traffic");
    expect(text).toContain("Fresh enough");
    expect(text).toContain("Ready Security Findings Baseline");
    expect(text).toContain("1 risk driver");
    expect(text).toContain(
      "public_ip_instances_with_packet_traffic_need_sg_nacl_route_verification"
    );
    expect(text).toContain("Security Exposure Index Warning Threshold");
    expect(text).toContain("Security Triage Context");
    expect(text).toContain("ec2-security-deterministic-context-v1");
    expect(text).toContain("ec2-security-ai-triage-v1");
    expect(text).toContain("Provider Routing (not invoked)");
    expect(text).toContain("primary_ops_llm");
    expect(text).toContain("1200 token budget");
    expect(text).toContain("Read only");
    expect(text).toContain("Deterministic context only");
    expect(text).toContain("public IP assignment");
    expect(text).toContain("security groups");
    expect(text).toContain("NetworkPacketsIn and NetworkPacketsOut");
    expect(text).toContain("Security Agentic Investigation");
    expect(text).toContain("ec2_security_agentic_investigation");
    expect(text).toContain("Replay required");
    expect(text).toContain("5 max tool call");
    expect(text).toContain("2 evidence citation");
    expect(text).toContain("Mutation planning requires approval");
    expect(text).toContain("ec2.describe_instance_networking");
    expect(text).toContain("ec2.compare_security_group_ingress");
    expect(text).toContain("ec2.security.prepare_approval_plan");
    expect(text).toContain("Approval Required");
    expect(text).toContain("EC2_SECURITY_APPROVAL_PLAN_REQUIRED");
    expect(text).toContain("ec2-security-approval-01");
    expect(text).toContain("no mutation is executable from the investigation plan");
    expect(text).toContain("Rollback note required");
    expect(text).toContain("Security Remediation");
    expect(text).toContain("Dry Run");
    expect(text).toContain("ec2_security_safe_remediation");
    expect(text).toContain("ec2_security_remediation_audit");
    expect(text).toContain("aws.ec2.security.remediation.approve");
    expect(text).toContain("Pending approval");
    expect(text).toContain("2 dry-run action");
    expect(text).toContain("ec2-security-remediation-01");
    expect(text).toContain("Review Security Exposure");
    expect(text).toContain("Review Security Owner Metadata");
    expect(text).toContain("Dry Run Pending Approval");
    expect(text).toContain("ec2.security.remediation.dry_run_planned");
    expect(text).toContain("i-sec-exposed");
    expect(text).toContain("i-sec-gap");
    expect(text).toContain("intentionally internet-facing");

    await view.unmount();
  });
});

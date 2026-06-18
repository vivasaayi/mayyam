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
    expect(text).toContain("i-sec-exposed");
    expect(text).toContain("i-sec-gap");
    expect(text).toContain("intentionally internet-facing");

    await view.unmount();
  });
});

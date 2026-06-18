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
    expect(text).toContain("EC2_PERF_MISSING_CORE_TELEMETRY");
    expect(text).toContain("EC2_PERF_HIGH_CPU_TELEMETRY");
    expect(text).toContain("i-perf-gap");
    expect(text).toContain("i-perf-hot");
    expect(text).toContain("high CPUUtilization telemetry");

    await view.unmount();
  });
});

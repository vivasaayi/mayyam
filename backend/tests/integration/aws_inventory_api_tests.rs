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

// API contract tests for /api/aws/inventory/ec2/pillars.

use reqwest::Client;
use serde_json::Value;

use crate::integration::helpers::server::base_url;

fn aws_tests_enabled() -> bool {
    std::env::var("ENABLE_AWS_TESTS").unwrap_or_else(|_| "false".to_string()) == "true"
}

#[tokio::test]
async fn ec2_pillar_reports_contract() {
    if !aws_tests_enabled() {
        println!("Skipping ec2_pillar_reports_contract because ENABLE_AWS_TESTS is not true");
        return;
    }

    let base = base_url().await;
    let client = Client::new();
    let account_id = "123456789012";

    // Happy path: all supported EC2 pillar reports with freshness metadata.
    let resp = client
        .get(format!(
            "{}/api/aws/inventory/ec2/pillars?account_id={}",
            base, account_id
        ))
        .send()
        .await
        .expect("pillar report request failed");
    assert_eq!(resp.status(), 200);
    let body: Value = resp.json().await.expect("invalid JSON body");
    assert_eq!(body["account_id"], account_id);
    assert_eq!(body["resource_type"], "EC2Instance");
    assert!(body["evaluated_at"].is_string());
    assert!(body["stale_after_hours"].is_number());
    assert!(body["resources_evaluated"].is_number());
    let reports = body["reports"].as_array().expect("reports array");
    assert_eq!(reports.len(), 7);
    for report in reports {
        assert!(report["pillar"].is_string());
        assert!(report["score"].is_number());
        assert!(report["findings"].is_array());
    }

    // Single pillar selection.
    let resp = client
        .get(format!(
            "{}/api/aws/inventory/ec2/pillars?account_id={}&pillar=cost",
            base, account_id
        ))
        .send()
        .await
        .expect("cost pillar request failed");
    assert_eq!(resp.status(), 200);
    let body: Value = resp.json().await.expect("invalid JSON body");
    let reports = body["reports"].as_array().expect("reports array");
    assert_eq!(reports.len(), 1);
    assert_eq!(reports[0]["pillar"], "cost");
    assert_eq!(reports[0]["posture"]["rules_evaluated"], 4);
    assert!(reports[0]["posture"]["rules"].is_array());
    assert!(reports[0]["triage_context"]["facts"].is_array());
    assert!(reports[0]["triage_context"]["hypotheses"].is_array());
    assert!(reports[0]["triage_context"]["missing_data_questions"].is_array());
    assert_eq!(
        reports[0]["remediation_workflow"]["workflow_id"],
        "ec2_cost_safe_remediation"
    );
    assert_eq!(reports[0]["remediation_workflow"]["read_only_mode"], true);
    assert_eq!(
        reports[0]["remediation_workflow"]["rbac_permission"],
        "aws.ec2.cost.remediation.approve"
    );
    assert!(reports[0]["remediation_workflow"]["actions"].is_array());
    assert!(reports[0]["remediation_workflow"]["approval_gates"].is_array());
    assert_eq!(
        reports[0]["slo_policy_tracking"]["workflow_id"],
        "ec2_cost_slo_policy"
    );
    assert_eq!(reports[0]["slo_policy_tracking"]["read_only_mode"], true);
    assert_eq!(
        reports[0]["slo_policy_tracking"]["objective"]["objective_id"],
        "ec2-cost-score-min-90"
    );
    assert!(reports[0]["slo_policy_tracking"]["objective"]["status_history"].is_array());
    assert!(reports[0]["slo_policy_tracking"]["evidence_reason_codes"].is_array());
    assert_eq!(
        reports[0]["forecasting"]["workflow_id"],
        "ec2_cost_forecasting"
    );
    assert_eq!(reports[0]["forecasting"]["read_only_mode"], true);
    assert_eq!(reports[0]["forecasting"]["baseline_window_days"], 30);
    assert_eq!(reports[0]["forecasting"]["forecast_horizon_days"], 30);
    assert!(reports[0]["forecasting"]["forecast_band"]["expected_monthly_cost_index"].is_number());
    assert!(reports[0]["forecasting"]["risk_drivers"].is_array());
    assert!(reports[0]["forecasting"]["missing_data_reason_codes"].is_array());
    assert_eq!(reports[0]["reporting"]["workflow_id"], "ec2_cost_reporting");
    assert_eq!(reports[0]["reporting"]["read_only_mode"], true);
    assert_eq!(
        reports[0]["reporting"]["executive_summary"]["report_id"],
        "ec2-cost-executive-summary"
    );
    assert!(reports[0]["reporting"]["executive_summary"]["top_reason_codes"].is_array());
    assert_eq!(
        reports[0]["reporting"]["engineering_backlog"]["report_id"],
        "ec2-cost-engineering-backlog"
    );
    assert_eq!(reports[0]["reporting"]["engineering_backlog"]["page"], 0);
    assert_eq!(
        reports[0]["reporting"]["engineering_backlog"]["page_size"],
        50
    );
    assert!(reports[0]["reporting"]["engineering_backlog"]["rows"].is_array());
    assert!(reports[0]["reporting"]["evidence_reason_codes"].is_array());

    let resp = client
        .get(format!(
            "{}/api/aws/inventory/ec2/pillars?account_id={}&pillar=security",
            base, account_id
        ))
        .send()
        .await
        .expect("security pillar request failed");
    assert_eq!(resp.status(), 200);
    let body: Value = resp.json().await.expect("invalid JSON body");
    let reports = body["reports"].as_array().expect("reports array");
    assert_eq!(reports.len(), 1);
    assert_eq!(reports[0]["pillar"], "security");
    assert_eq!(
        reports[0]["assessment_scope"],
        "ec2_public_exposure_owner_routing_and_packet_telemetry"
    );
    assert_eq!(reports[0]["posture"]["rules_evaluated"], 5);
    assert!(matches!(
        reports[0]["posture"]["status"].as_str(),
        Some("pass" | "fail")
    ));
    let security_rules = reports[0]["posture"]["rules"]
        .as_array()
        .expect("security posture rules");
    assert_eq!(security_rules.len(), 5);
    let expected_rules = [
        ("ec2-security-inventory-freshness", "EC2_INV_STALE_DATA"),
        (
            "ec2-security-public-ip-exposure",
            "EC2_SEC_PUBLIC_IP_ASSIGNED",
        ),
        (
            "ec2-security-owner-routing-present",
            "EC2_SEC_MISSING_OWNER_TAG",
        ),
        (
            "ec2-security-packet-telemetry-present",
            "EC2_SEC_MISSING_PACKET_TELEMETRY",
        ),
        (
            "ec2-security-public-packet-traffic",
            "EC2_SEC_PUBLIC_PACKET_TRAFFIC_TELEMETRY",
        ),
    ];
    for (rule, (expected_rule_id, expected_reason_code)) in
        security_rules.iter().zip(expected_rules)
    {
        assert_eq!(rule["rule_id"], expected_rule_id);
        assert_eq!(rule["reason_codes"][0], expected_reason_code);
        assert!(rule["suppression_supported"].is_boolean());
        assert!(rule["assignment_supported"].is_boolean());
        assert!(rule["affected_resources"].is_array());
    }
    assert!(reports[0]["posture"]["affected_resources"].is_array());
    let triage = &reports[0]["triage_context"];
    assert_eq!(triage["workflow_id"], "ec2_security_triage_context");
    assert_eq!(
        triage["context_builder_id"],
        "ec2-security-deterministic-context-v1"
    );
    assert_eq!(triage["prompt_template_id"], "ec2-security-ai-triage-v1");
    assert_eq!(triage["generation_mode"], "deterministic_no_llm");
    assert_eq!(triage["max_prompt_tokens"], 1200);
    assert_eq!(triage["provider_routing"][0], "primary_ops_llm");
    assert_eq!(triage["audit_event_type"], "ec2_ai_triage_context_built");
    assert_eq!(triage["guardrails"]["read_only_mode"], true);
    assert_eq!(triage["guardrails"]["evidence_required"], true);
    assert_eq!(triage["guardrails"]["separate_facts_from_hypotheses"], true);
    assert_eq!(triage["guardrails"]["ask_for_missing_data"], true);
    assert_eq!(triage["guardrails"]["no_llm_invocation"], true);
    assert_eq!(triage["guardrails"]["no_mutation_planning"], true);
    assert!(triage["facts"].is_array());
    assert!(triage["hypotheses"].is_array());
    assert!(triage["missing_data_questions"].is_array());
    assert!(triage["evidence_citations"].is_array());
    assert_eq!(
        reports[0]["agentic_investigation"]["workflow_id"],
        "ec2_security_agentic_investigation"
    );
    assert_eq!(
        reports[0]["agentic_investigation"]["default_tool_mode"],
        "read_only"
    );
    assert_eq!(reports[0]["agentic_investigation"]["replay_required"], true);
    assert!(reports[0]["agentic_investigation"]["steps"].is_array());
    assert!(reports[0]["agentic_investigation"]["approval_gates"].is_array());
    assert!(reports[0]["agentic_investigation"]["evidence_citations"].is_array());
    let security_steps = reports[0]["agentic_investigation"]["steps"]
        .as_array()
        .expect("security investigation steps should be an array");
    assert!(security_steps.iter().all(|step| {
        let tool_name = step["tool_name"].as_str().unwrap_or_default();
        tool_name.starts_with("ec2.")
            && !tool_name.contains("execute")
            && !tool_name.contains("run_instances")
            && !tool_name.contains("terminate")
    }));
    assert!(security_steps.iter().all(|step| {
        step["tool_mode"] == "read_only"
            || (step["tool_mode"] == "approval_required"
                && step["tool_name"] == "ec2.security.prepare_approval_plan")
    }));
    if let Some(final_step) = security_steps.last() {
        if final_step["tool_mode"] == "approval_required" {
            assert_eq!(
                final_step["reason_code"],
                "EC2_SECURITY_APPROVAL_PLAN_REQUIRED"
            );
            assert_eq!(
                final_step["evidence"]["assessment_scope"],
                "ec2_public_exposure_owner_routing_and_packet_telemetry"
            );
        }
    }
    assert_eq!(
        reports[0]["remediation_workflow"]["workflow_id"],
        "ec2_security_safe_remediation"
    );
    assert_eq!(reports[0]["remediation_workflow"]["read_only_mode"], true);
    assert_eq!(
        reports[0]["remediation_workflow"]["rbac_permission"],
        "aws.ec2.security.remediation.approve"
    );
    assert_eq!(
        reports[0]["remediation_workflow"]["audit_stream"],
        "ec2_security_remediation_audit"
    );
    assert!(reports[0]["remediation_workflow"]["actions"].is_array());
    assert!(reports[0]["remediation_workflow"]["approval_gates"].is_array());
    let security_actions = reports[0]["remediation_workflow"]["actions"]
        .as_array()
        .expect("security remediation actions should be an array");
    assert!(security_actions.iter().all(|action| {
        action["dry_run"] == true
            && action["requires_approval"] == true
            && !action["approval_gate_id"].is_null()
            && action["audit_event_type"] == "ec2.security.remediation.dry_run_planned"
            && (action["status"] == "dry_run_pending_approval"
                || action["status"] == "blocked_missing_evidence")
            && action["rollback_note"]
                .as_str()
                .unwrap_or_default()
                .contains("rollback")
    }));
    assert_eq!(
        reports[0]["slo_policy_tracking"]["workflow_id"],
        "ec2_security_slo_policy"
    );
    assert_eq!(reports[0]["slo_policy_tracking"]["read_only_mode"], true);
    assert_eq!(
        reports[0]["slo_policy_tracking"]["freshness_required"],
        true
    );
    assert_eq!(
        reports[0]["slo_policy_tracking"]["objective"]["objective_id"],
        "ec2-security-score-min-95"
    );
    assert_eq!(
        reports[0]["slo_policy_tracking"]["objective"]["target_score_min"],
        95
    );
    assert!(reports[0]["slo_policy_tracking"]["objective"]["status"]
        .as_str()
        .is_some());
    assert!(
        reports[0]["slo_policy_tracking"]["objective"]["trend_direction"]
            .as_str()
            .is_some()
    );
    assert!(
        reports[0]["slo_policy_tracking"]["objective"]["policy_state"]
            .as_str()
            .is_some()
    );
    assert!(reports[0]["slo_policy_tracking"]["objective"]["owner_filters"].is_array());
    assert!(reports[0]["slo_policy_tracking"]["objective"]["environment_filters"].is_array());
    assert!(reports[0]["slo_policy_tracking"]["objective"]["application_filters"].is_array());
    assert!(reports[0]["slo_policy_tracking"]["objective"]["notification_targets"].is_array());
    assert!(reports[0]["slo_policy_tracking"]["objective"]["status_history"].is_array());
    assert!(reports[0]["slo_policy_tracking"]["evidence_reason_codes"].is_array());
    assert_eq!(
        reports[0]["forecasting"]["workflow_id"],
        "ec2_security_forecasting"
    );
    assert_eq!(reports[0]["forecasting"]["read_only_mode"], true);
    assert_eq!(reports[0]["forecasting"]["baseline_window_days"], 30);
    assert_eq!(reports[0]["forecasting"]["forecast_horizon_days"], 30);
    assert_eq!(reports[0]["forecasting"]["confidence_level"], 75);
    assert_eq!(
        reports[0]["forecasting"]["forecast_band"]["horizon_days"],
        30
    );
    assert!(
        reports[0]["forecasting"]["forecast_band"]["expected_security_exposure_index"].is_number()
    );
    assert!(reports[0]["forecasting"]["risk_level"].as_str().is_some());
    assert!(reports[0]["forecasting"]["exposure_capacity_risk"].is_string());
    assert!(reports[0]["forecasting"]["backtesting_fixture_status"].is_string());
    assert!(reports[0]["forecasting"]["threshold_controls"].is_array());
    assert!(reports[0]["forecasting"]["what_if_inputs"].is_array());
    assert!(reports[0]["forecasting"]["blocked_by_stale_data"].is_boolean());
    assert!(reports[0]["forecasting"]["blast_radius_summary"].is_string());
    assert!(reports[0]["forecasting"]["missing_data_reason_codes"].is_array());
    assert!(reports[0]["forecasting"]["risk_drivers"].is_array());
    assert!(reports[0]["forecasting"]["evidence_reason_codes"].is_array());

    let resp = client
        .get(format!(
            "{}/api/aws/inventory/ec2/pillars?account_id={}&pillar=resilience",
            base, account_id
        ))
        .send()
        .await
        .expect("resilience pillar request failed");
    assert_eq!(resp.status(), 200);
    let body: Value = resp.json().await.expect("invalid JSON body");
    let reports = body["reports"].as_array().expect("reports array");
    assert_eq!(reports.len(), 1);
    assert_eq!(reports[0]["pillar"], "resilience");
    assert_eq!(
        reports[0]["assessment_scope"],
        "ec2_instance_placement_and_status_checks"
    );
    assert_eq!(reports[0]["posture"]["rules_evaluated"], 4);
    let resilience_rules = reports[0]["posture"]["rules"]
        .as_array()
        .expect("resilience posture rules");
    assert_eq!(resilience_rules.len(), 4);
    let expected_rules = [
        (
            "ec2-resilience-availability-zone-recorded",
            "EC2_RES_MISSING_AVAILABILITY_ZONE",
        ),
        (
            "ec2-resilience-multi-az-placement",
            "EC2_RES_SINGLE_AZ_CONCENTRATION",
        ),
        (
            "ec2-resilience-status-check-telemetry",
            "EC2_RES_MISSING_STATUS_TELEMETRY",
        ),
        (
            "ec2-resilience-status-check-health",
            "EC2_RES_STATUS_CHECK_FAILURE_TELEMETRY",
        ),
    ];
    for (rule, (expected_rule_id, expected_reason_code)) in
        resilience_rules.iter().zip(expected_rules)
    {
        assert_eq!(rule["rule_id"], expected_rule_id);
        assert_eq!(rule["reason_codes"][0], expected_reason_code);
    }
    assert!(reports[0]["posture"]["affected_resources"].is_array());
    assert_eq!(
        reports[0]["triage_context"]["workflow_id"],
        "ec2_resilience_triage_context"
    );
    assert_eq!(
        reports[0]["triage_context"]["context_builder_id"],
        "ec2-resilience-deterministic-context-v1"
    );
    assert_eq!(
        reports[0]["triage_context"]["generation_mode"],
        "deterministic_no_llm"
    );
    assert_eq!(
        reports[0]["triage_context"]["guardrails"]["read_only_mode"],
        true
    );
    assert_eq!(
        reports[0]["triage_context"]["guardrails"]["evidence_required"],
        true
    );
    assert_eq!(
        reports[0]["triage_context"]["guardrails"]["no_llm_invocation"],
        true
    );
    assert_eq!(
        reports[0]["triage_context"]["guardrails"]["no_mutation_planning"],
        true
    );
    assert!(reports[0]["triage_context"]["facts"].is_array());
    assert!(reports[0]["triage_context"]["hypotheses"].is_array());
    assert!(reports[0]["triage_context"]["missing_data_questions"].is_array());
    assert!(reports[0]["triage_context"]["evidence_citations"].is_array());
    assert_eq!(
        reports[0]["agentic_investigation"]["workflow_id"],
        "ec2_resilience_agentic_investigation"
    );
    assert_eq!(
        reports[0]["agentic_investigation"]["default_tool_mode"],
        "read_only"
    );
    assert_eq!(reports[0]["agentic_investigation"]["replay_required"], true);
    assert!(reports[0]["agentic_investigation"]["steps"].is_array());
    assert!(reports[0]["agentic_investigation"]["approval_gates"].is_array());
    assert!(reports[0]["agentic_investigation"]["evidence_citations"].is_array());
    let investigation_steps = reports[0]["agentic_investigation"]["steps"]
        .as_array()
        .expect("resilience investigation steps should be an array");
    assert!(investigation_steps.iter().all(|step| {
        let tool_name = step["tool_name"].as_str().unwrap_or_default();
        tool_name.starts_with("ec2.")
            && !tool_name.contains("execute")
            && !tool_name.contains("run_instances")
            && !tool_name.contains("terminate")
    }));
    assert!(investigation_steps.iter().all(|step| {
        step["tool_mode"] == "read_only"
            || (step["tool_mode"] == "approval_required"
                && step["tool_name"] == "ec2.resilience.prepare_approval_plan")
    }));
    if let Some(final_step) = investigation_steps.last() {
        if final_step["tool_mode"] == "approval_required" {
            assert_eq!(
                final_step["reason_code"],
                "EC2_RESILIENCE_APPROVAL_PLAN_REQUIRED"
            );
            assert_eq!(
                final_step["evidence"]["assessment_scope"],
                "ec2_instance_placement_and_status_checks"
            );
        }
    }
    assert_eq!(
        reports[0]["remediation_workflow"]["workflow_id"],
        "ec2_resilience_safe_remediation"
    );
    assert_eq!(reports[0]["remediation_workflow"]["read_only_mode"], true);
    assert_eq!(
        reports[0]["remediation_workflow"]["rbac_permission"],
        "aws.ec2.resilience.remediation.approve"
    );
    assert_eq!(
        reports[0]["remediation_workflow"]["audit_stream"],
        "ec2_resilience_remediation_audit"
    );
    assert!(reports[0]["remediation_workflow"]["actions"].is_array());
    assert!(reports[0]["remediation_workflow"]["approval_gates"].is_array());
    let remediation_actions = reports[0]["remediation_workflow"]["actions"]
        .as_array()
        .expect("resilience remediation actions should be an array");
    assert!(remediation_actions.iter().all(|action| {
        action["dry_run"] == true
            && action["requires_approval"] == true
            && !action["approval_gate_id"].is_null()
            && action["audit_event_type"] == "ec2.resilience.remediation.dry_run_planned"
            && (action["status"] == "dry_run_pending_approval"
                || action["status"] == "blocked_missing_evidence")
            && action["rollback_note"]
                .as_str()
                .map(|note| note.contains("rollback"))
                .unwrap_or(false)
    }));
    assert_eq!(
        reports[0]["slo_policy_tracking"]["workflow_id"],
        "ec2_resilience_slo_policy"
    );
    assert_eq!(reports[0]["slo_policy_tracking"]["read_only_mode"], true);
    assert_eq!(
        reports[0]["slo_policy_tracking"]["objective"]["objective_id"],
        "ec2-resilience-score-min-95"
    );
    assert_eq!(
        reports[0]["slo_policy_tracking"]["objective"]["target_score_min"],
        95
    );
    assert_eq!(
        reports[0]["slo_policy_tracking"]["freshness_required"],
        true
    );
    assert!(matches!(
        reports[0]["slo_policy_tracking"]["objective"]["status"].as_str(),
        Some("on_track" | "at_risk" | "breached")
    ));
    assert!(matches!(
        reports[0]["slo_policy_tracking"]["objective"]["trend_direction"].as_str(),
        Some("stable" | "degrading")
    ));
    assert!(matches!(
        reports[0]["slo_policy_tracking"]["objective"]["policy_state"].as_str(),
        Some("active" | "active_with_findings" | "blocked_stale_data")
    ));
    assert!(reports[0]["slo_policy_tracking"]["objective"]["notification_targets"].is_array());
    assert!(reports[0]["slo_policy_tracking"]["objective"]["status_history"].is_array());
    assert!(reports[0]["slo_policy_tracking"]["evidence_reason_codes"].is_array());
    assert_eq!(
        reports[0]["forecasting"]["workflow_id"],
        "ec2_resilience_forecasting"
    );
    assert_eq!(reports[0]["forecasting"]["read_only_mode"], true);
    assert_eq!(reports[0]["forecasting"]["baseline_window_days"], 30);
    assert_eq!(reports[0]["forecasting"]["forecast_horizon_days"], 30);
    assert_eq!(reports[0]["forecasting"]["confidence_level"], 75);
    assert_eq!(
        reports[0]["forecasting"]["forecast_band"]["horizon_days"],
        30
    );
    assert!(
        reports[0]["forecasting"]["forecast_band"]["expected_recovery_exposure_index"].is_number()
    );
    assert!(matches!(
        reports[0]["forecasting"]["risk_level"].as_str(),
        Some("low" | "moderate" | "high" | "blocked")
    ));
    assert!(reports[0]["forecasting"]["recovery_capacity_risk"].is_string());
    assert!(reports[0]["forecasting"]["backtesting_fixture_status"].is_string());
    assert!(reports[0]["forecasting"]["threshold_controls"].is_array());
    assert!(reports[0]["forecasting"]["what_if_inputs"].is_array());
    assert!(reports[0]["forecasting"]["blocked_by_stale_data"].is_boolean());
    assert!(reports[0]["forecasting"]["blast_radius_summary"].is_string());
    assert!(reports[0]["forecasting"]["recovery_note"]
        .as_str()
        .map(|note| note.contains("read-only") && note.contains("approval"))
        .unwrap_or(false));
    assert!(reports[0]["forecasting"]["missing_data_reason_codes"].is_array());
    assert!(reports[0]["forecasting"]["risk_drivers"].is_array());
    assert!(reports[0]["forecasting"]["evidence_reason_codes"].is_array());
    assert_eq!(
        reports[0]["reporting"]["workflow_id"],
        "ec2_resilience_reporting"
    );
    assert_eq!(reports[0]["reporting"]["read_only_mode"], true);
    assert!(matches!(
        reports[0]["reporting"]["scheduled_delivery_state"].as_str(),
        Some(
            "ready_for_schedule"
                | "ready_with_resilience_evidence_gaps"
                | "blocked_until_fresh_resilience_evidence"
        )
    ));
    assert!(reports[0]["reporting"]["portfolio_summary_ready"].is_boolean());
    assert!(reports[0]["reporting"]["workload_summary_ready"].is_boolean());
    assert!(reports[0]["reporting"]["stale_data_blocks_delivery"].is_boolean());
    assert_eq!(
        reports[0]["reporting"]["executive_summary"]["report_id"],
        "ec2-resilience-executive-summary"
    );
    assert!(reports[0]["reporting"]["executive_summary"]["top_reason_codes"].is_array());
    assert!(reports[0]["reporting"]["executive_summary"]["blast_radius_summary"].is_string());
    assert_eq!(
        reports[0]["reporting"]["incident_review"]["report_id"],
        "ec2-resilience-incident-review"
    );
    assert_eq!(reports[0]["reporting"]["incident_review"]["page"], 0);
    assert_eq!(reports[0]["reporting"]["incident_review"]["page_size"], 50);
    assert!(reports[0]["reporting"]["incident_review"]["rows"].is_array());
    assert!(reports[0]["reporting"]["missing_data_reason_codes"].is_array());
    assert!(reports[0]["reporting"]["evidence_reason_codes"].is_array());

    let resp = client
        .get(format!(
            "{}/api/aws/inventory/ec2/pillars?account_id={}&pillar=performance",
            base, account_id
        ))
        .send()
        .await
        .expect("performance pillar request failed");
    assert_eq!(resp.status(), 200);
    let body: Value = resp.json().await.expect("invalid JSON body");
    let reports = body["reports"].as_array().expect("reports array");
    assert_eq!(reports.len(), 1);
    assert_eq!(reports[0]["pillar"], "performance");
    assert_eq!(
        reports[0]["assessment_scope"],
        "ec2_core_performance_telemetry_and_cpu_headroom"
    );
    assert_eq!(reports[0]["posture"]["rules_evaluated"], 3);
    assert!(matches!(
        reports[0]["posture"]["status"].as_str(),
        Some("pass" | "fail")
    ));
    let performance_rules = reports[0]["posture"]["rules"]
        .as_array()
        .expect("performance posture rules");
    assert_eq!(performance_rules.len(), 3);
    let expected_rules = [
        ("ec2-performance-inventory-freshness", "EC2_INV_STALE_DATA"),
        (
            "ec2-performance-core-telemetry-present",
            "EC2_PERF_MISSING_CORE_TELEMETRY",
        ),
        (
            "ec2-performance-cpu-headroom",
            "EC2_PERF_HIGH_CPU_TELEMETRY",
        ),
    ];
    for (rule, (expected_rule_id, expected_reason_code)) in
        performance_rules.iter().zip(expected_rules)
    {
        assert_eq!(rule["rule_id"], expected_rule_id);
        assert_eq!(rule["reason_codes"][0], expected_reason_code);
        assert!(rule["suppression_supported"].is_boolean());
        assert!(rule["assignment_supported"].is_boolean());
        assert!(rule["affected_resources"].is_array());
    }
    let triage = &reports[0]["triage_context"];
    assert_eq!(triage["workflow_id"], "ec2_performance_triage_context");
    assert_eq!(
        triage["context_builder_id"],
        "ec2-performance-deterministic-context-v1"
    );
    assert_eq!(triage["prompt_template_id"], "ec2-performance-ai-triage-v1");
    assert_eq!(triage["generation_mode"], "deterministic_no_llm");
    assert_eq!(triage["max_prompt_tokens"], 1200);
    assert_eq!(triage["provider_routing"][0], "primary_ops_llm");
    assert_eq!(triage["audit_event_type"], "ec2_ai_triage_context_built");
    assert_eq!(triage["guardrails"]["read_only_mode"], true);
    assert_eq!(triage["guardrails"]["evidence_required"], true);
    assert_eq!(triage["guardrails"]["no_llm_invocation"], true);
    assert_eq!(triage["guardrails"]["no_mutation_planning"], true);
    assert!(triage["facts"].is_array());
    assert!(triage["hypotheses"].is_array());
    assert!(triage["missing_data_questions"].is_array());
    assert!(triage["evidence_citations"].is_array());
    assert_eq!(
        reports[0]["forecasting"]["workflow_id"],
        "ec2_performance_forecasting"
    );
    assert_eq!(reports[0]["forecasting"]["read_only_mode"], true);
    assert_eq!(reports[0]["forecasting"]["baseline_window_days"], 30);
    assert_eq!(reports[0]["forecasting"]["forecast_horizon_days"], 30);
    assert_eq!(reports[0]["forecasting"]["confidence_level"], 75);
    assert_eq!(
        reports[0]["forecasting"]["forecast_band"]["horizon_days"],
        30
    );
    assert!(
        reports[0]["forecasting"]["forecast_band"]["expected_performance_pressure_index"]
            .is_number()
    );
    assert!(reports[0]["forecasting"]["risk_level"].as_str().is_some());
    assert!(reports[0]["forecasting"]["performance_capacity_risk"].is_string());
    assert!(reports[0]["forecasting"]["backtesting_fixture_status"].is_string());
    assert!(reports[0]["forecasting"]["threshold_controls"].is_array());
    assert!(reports[0]["forecasting"]["what_if_inputs"].is_array());
    assert!(reports[0]["forecasting"]["blocked_by_stale_data"].is_boolean());
    assert!(reports[0]["forecasting"]["blast_radius_summary"].is_string());
    assert!(reports[0]["forecasting"]["missing_data_reason_codes"].is_array());
    assert!(reports[0]["forecasting"]["risk_drivers"].is_array());
    assert!(reports[0]["forecasting"]["evidence_reason_codes"].is_array());

    let resp = client
        .get(format!(
            "{}/api/aws/inventory/ec2/pillars?account_id={}&pillar=scalability",
            base, account_id
        ))
        .send()
        .await
        .expect("scalability pillar request failed");
    assert_eq!(resp.status(), 200);
    let body: Value = resp.json().await.expect("invalid JSON body");
    let reports = body["reports"].as_array().expect("reports array");
    assert_eq!(reports.len(), 1);
    assert_eq!(reports[0]["pillar"], "scalability");
    assert_eq!(
        reports[0]["assessment_scope"],
        "ec2_demand_telemetry_and_cpu_scaling_pressure"
    );
    assert_eq!(reports[0]["posture"]["rules_evaluated"], 3);
    assert!(matches!(
        reports[0]["posture"]["status"].as_str(),
        Some("pass" | "fail")
    ));
    let scalability_rules = reports[0]["posture"]["rules"]
        .as_array()
        .expect("scalability posture rules");
    assert_eq!(scalability_rules.len(), 3);
    let expected_rules = [
        ("ec2-scalability-inventory-freshness", "EC2_INV_STALE_DATA"),
        (
            "ec2-scalability-demand-telemetry-present",
            "EC2_SCALE_MISSING_DEMAND_TELEMETRY",
        ),
        (
            "ec2-scalability-cpu-pressure",
            "EC2_SCALE_HIGH_CPU_PRESSURE_TELEMETRY",
        ),
    ];
    for (rule, (expected_rule_id, expected_reason_code)) in
        scalability_rules.iter().zip(expected_rules)
    {
        assert_eq!(rule["rule_id"], expected_rule_id);
        assert_eq!(rule["reason_codes"][0], expected_reason_code);
        assert!(rule["suppression_supported"].is_boolean());
        assert!(rule["assignment_supported"].is_boolean());
        assert!(rule["affected_resources"].is_array());
    }
    let triage = &reports[0]["triage_context"];
    assert_eq!(triage["workflow_id"], "ec2_scalability_triage_context");
    assert_eq!(
        triage["context_builder_id"],
        "ec2-scalability-deterministic-context-v1"
    );
    assert_eq!(triage["prompt_template_id"], "ec2-scalability-ai-triage-v1");
    assert_eq!(triage["generation_mode"], "deterministic_no_llm");
    assert_eq!(triage["max_prompt_tokens"], 1200);
    assert_eq!(triage["provider_routing"][0], "primary_ops_llm");
    assert_eq!(triage["audit_event_type"], "ec2_ai_triage_context_built");
    assert_eq!(triage["guardrails"]["read_only_mode"], true);
    assert_eq!(triage["guardrails"]["evidence_required"], true);
    assert_eq!(triage["guardrails"]["separate_facts_from_hypotheses"], true);
    assert_eq!(triage["guardrails"]["ask_for_missing_data"], true);
    assert_eq!(triage["guardrails"]["no_llm_invocation"], true);
    assert_eq!(triage["guardrails"]["no_mutation_planning"], true);
    assert!(triage["facts"].is_array());
    assert!(triage["hypotheses"].is_array());
    assert!(triage["missing_data_questions"].is_array());
    assert!(triage["evidence_citations"].is_array());

    let resp = client
        .get(format!(
            "{}/api/aws/inventory/ec2/pillars?account_id={}&pillar=disaster-recovery",
            base, account_id
        ))
        .send()
        .await
        .expect("disaster-recovery pillar request failed");
    assert_eq!(resp.status(), 200);
    let body: Value = resp.json().await.expect("invalid JSON body");
    let reports = body["reports"].as_array().expect("reports array");
    assert_eq!(reports.len(), 1);
    assert_eq!(reports[0]["pillar"], "disaster-recovery");
    assert_eq!(
        reports[0]["assessment_scope"],
        "ec2_recovery_point_freshness_and_restore_evidence"
    );
    assert_eq!(reports[0]["posture"]["rules_evaluated"], 3);
    assert!(matches!(
        reports[0]["posture"]["status"].as_str(),
        Some("pass" | "fail")
    ));
    let dr_rules = reports[0]["posture"]["rules"]
        .as_array()
        .expect("disaster-recovery posture rules");
    assert_eq!(dr_rules.len(), 3);
    let expected_rules = [
        (
            "ec2-disaster-recovery-inventory-freshness",
            "EC2_INV_STALE_DATA",
        ),
        (
            "ec2-disaster-recovery-recovery-point-telemetry-present",
            "EC2_DR_MISSING_RECOVERY_POINT_TELEMETRY",
        ),
        (
            "ec2-disaster-recovery-recovery-point-freshness",
            "EC2_DR_STALE_RECOVERY_POINT_TELEMETRY",
        ),
    ];
    for (rule, (expected_rule_id, expected_reason_code)) in dr_rules.iter().zip(expected_rules) {
        assert_eq!(rule["rule_id"], expected_rule_id);
        assert_eq!(rule["reason_codes"][0], expected_reason_code);
        assert!(rule["suppression_supported"].is_boolean());
        assert!(rule["assignment_supported"].is_boolean());
        assert!(rule["affected_resources"].is_array());
    }
    let triage = &reports[0]["triage_context"];
    assert_eq!(
        triage["workflow_id"],
        "ec2_disaster_recovery_triage_context"
    );
    assert_eq!(
        triage["context_builder_id"],
        "ec2-disaster-recovery-deterministic-context-v1"
    );
    assert_eq!(
        triage["prompt_template_id"],
        "ec2-disaster-recovery-ai-triage-v1"
    );
    assert_eq!(triage["generation_mode"], "deterministic_no_llm");
    assert_eq!(triage["max_prompt_tokens"], 1200);
    assert_eq!(triage["provider_routing"][0], "primary_ops_llm");
    assert_eq!(triage["audit_event_type"], "ec2_ai_triage_context_built");
    assert_eq!(triage["guardrails"]["read_only_mode"], true);
    assert_eq!(triage["guardrails"]["evidence_required"], true);
    assert_eq!(triage["guardrails"]["separate_facts_from_hypotheses"], true);
    assert_eq!(triage["guardrails"]["ask_for_missing_data"], true);
    assert_eq!(triage["guardrails"]["no_llm_invocation"], true);
    assert_eq!(triage["guardrails"]["no_mutation_planning"], true);
    assert!(triage["facts"].is_array());
    assert!(triage["hypotheses"].is_array());
    assert!(triage["missing_data_questions"].is_array());
    assert!(triage["evidence_citations"].is_array());

    let resp = client
        .get(format!(
            "{}/api/aws/inventory/ec2/pillars?account_id={}&pillar=operational-excellence",
            base, account_id
        ))
        .send()
        .await
        .expect("operational-excellence pillar request failed");
    assert_eq!(resp.status(), 200);
    let body: Value = resp.json().await.expect("invalid JSON body");
    let reports = body["reports"].as_array().expect("reports array");
    assert_eq!(reports.len(), 1);
    assert_eq!(reports[0]["pillar"], "operational-excellence");
    assert_eq!(
        reports[0]["assessment_scope"],
        "ec2_telemetry_collection_runbook_readiness"
    );
    assert_eq!(reports[0]["posture"]["rules_evaluated"], 4);
    assert!(matches!(
        reports[0]["posture"]["status"].as_str(),
        Some("pass" | "fail")
    ));
    let oe_rules = reports[0]["posture"]["rules"]
        .as_array()
        .expect("operational-excellence posture rules");
    assert_eq!(oe_rules.len(), 4);
    let expected_rules = [
        (
            "ec2-operational-excellence-inventory-freshness",
            "EC2_INV_STALE_DATA",
        ),
        (
            "ec2-operational-excellence-collection-metadata-present",
            "EC2_OE_MISSING_TELEMETRY_COLLECTION_METADATA",
        ),
        (
            "ec2-operational-excellence-collection-errors-clear",
            "EC2_OE_TELEMETRY_COLLECTION_ERRORS",
        ),
        (
            "ec2-operational-excellence-detailed-monitoring-enabled",
            "EC2_OE_BASIC_MONITORING",
        ),
    ];
    for (rule, (expected_rule_id, expected_reason_code)) in oe_rules.iter().zip(expected_rules) {
        assert_eq!(rule["rule_id"], expected_rule_id);
        assert_eq!(rule["reason_codes"][0], expected_reason_code);
        assert!(rule["suppression_supported"].is_boolean());
        assert!(rule["assignment_supported"].is_boolean());
        assert!(rule["affected_resources"].is_array());
    }
    let triage = &reports[0]["triage_context"];
    assert_eq!(
        triage["workflow_id"],
        "ec2_operational_excellence_triage_context"
    );
    assert_eq!(
        triage["context_builder_id"],
        "ec2-operational-excellence-deterministic-context-v1"
    );
    assert_eq!(
        triage["prompt_template_id"],
        "ec2-operational-excellence-ai-triage-v1"
    );
    assert_eq!(triage["generation_mode"], "deterministic_no_llm");
    assert_eq!(triage["max_prompt_tokens"], 1200);
    assert_eq!(triage["provider_routing"][0], "primary_ops_llm");
    assert_eq!(triage["audit_event_type"], "ec2_ai_triage_context_built");
    assert_eq!(triage["guardrails"]["read_only_mode"], true);
    assert_eq!(triage["guardrails"]["evidence_required"], true);
    assert_eq!(triage["guardrails"]["separate_facts_from_hypotheses"], true);
    assert_eq!(triage["guardrails"]["ask_for_missing_data"], true);
    assert_eq!(triage["guardrails"]["no_llm_invocation"], true);
    assert_eq!(triage["guardrails"]["no_mutation_planning"], true);
    assert!(triage["facts"].is_array());
    assert!(triage["hypotheses"].is_array());
    assert!(triage["missing_data_questions"].is_array());
    assert!(triage["evidence_citations"].is_array());
}

#[tokio::test]
async fn lambda_pillar_reports_contract() {
    if !aws_tests_enabled() {
        println!("Skipping lambda_pillar_reports_contract because ENABLE_AWS_TESTS is not true");
        return;
    }

    let base = base_url().await;
    let client = Client::new();
    let resp = client
        .get(format!(
            "{}/api/aws/inventory/lambda/pillars?account_id=123456789012",
            base
        ))
        .send()
        .await
        .expect("lambda pillar report request failed");
    assert_eq!(resp.status(), 200);
    let body: Value = resp.json().await.expect("invalid JSON body");
    assert_eq!(body["resource_type"], "LambdaFunction");
    let reports = body["reports"].as_array().expect("reports array");
    assert_eq!(reports.len(), 5);
    let cost = reports
        .iter()
        .find(|report| report["pillar"] == "cost")
        .expect("cost report");
    let resilience = reports
        .iter()
        .find(|report| report["pillar"] == "resilience")
        .expect("resilience report");
    let security = reports
        .iter()
        .find(|report| report["pillar"] == "security")
        .expect("security report");
    let performance = reports
        .iter()
        .find(|report| report["pillar"] == "performance")
        .expect("performance report");
    let scalability = reports
        .iter()
        .find(|report| report["pillar"] == "scalability")
        .expect("scalability report");
    assert_eq!(
        cost["assessment_scope"],
        "lambda_cost_invocation_duration_error_and_throttle_telemetry"
    );
    assert_eq!(cost["posture"]["workflow_id"], "lambda_cost_posture");
    assert_eq!(
        cost["posture"]["rule_pack_id"],
        "lambda-cost-posture-rules-v1"
    );
    assert_eq!(
        cost["posture"]["audit_event_type"],
        "lambda_cost_posture_evaluated"
    );
    assert_eq!(cost["posture"]["read_only_mode"], true);
    assert_eq!(cost["posture"]["rules_evaluated"], 7);
    assert_eq!(cost["posture"]["suppression_policy"]["supported"], true);
    assert_eq!(cost["posture"]["assignment_policy"]["supported"], true);
    assert!(cost["posture"]["recommendations"].is_array());
    assert_eq!(
        cost["triage_context"]["workflow_id"],
        "lambda_cost_triage_context"
    );
    assert_eq!(
        cost["triage_context"]["api_path"],
        "/api/aws/inventory/lambda/pillars"
    );
    assert_eq!(
        cost["triage_context"]["context_builder_id"],
        "lambda-cost-deterministic-context-v1"
    );
    assert_eq!(
        cost["triage_context"]["prompt_template_id"],
        "lambda-cost-ai-triage-v1"
    );
    assert_eq!(
        cost["triage_context"]["generation_mode"],
        "deterministic_no_llm"
    );
    assert_eq!(
        cost["triage_context"]["audit_id_prefix"],
        "lambda-cost-ai-triage"
    );
    assert_eq!(cost["triage_context"]["pagination"]["default_limit"], 50);
    assert_eq!(cost["triage_context"]["pagination"]["max_limit"], 200);
    assert_eq!(
        cost["triage_context"]["pagination"]["evidence_cursor"],
        "evidence_citations"
    );
    assert!(cost["triage_context"]["freshness"]["stale_data_blocks_ai_summary"].is_boolean());
    assert_eq!(
        cost["triage_context"]["freshness"]["freshness_source"],
        "lambda_inventory_last_synced_at"
    );
    assert!(cost["triage_context"]["export_formats"].is_array());
    assert!(cost["triage_context"]["error_codes"].is_array());
    assert_eq!(cost["triage_context"]["guardrails"]["read_only_mode"], true);
    assert_eq!(
        cost["triage_context"]["guardrails"]["evidence_required"],
        true
    );
    assert!(cost["triage_context"]["facts"].is_array());
    assert!(cost["triage_context"]["hypotheses"].is_array());
    assert!(cost["triage_context"]["missing_data_questions"].is_array());
    assert!(cost["triage_context"]["follow_up_questions"].is_array());
    assert!(cost["triage_context"]["runbook_copy_markdown"].is_string());
    assert_eq!(
        cost["triage_context"]["feedback_capture"]["supported"],
        true
    );
    assert!(cost["triage_context"]["evidence_citations"].is_array());
    let investigation = &cost["agentic_investigation"];
    assert_eq!(
        investigation["workflow_id"],
        "lambda_cost_agentic_investigation"
    );
    assert_eq!(investigation["default_tool_mode"], "read_only");
    assert_eq!(investigation["replay_required"], true);
    assert!(investigation["max_tool_calls"].is_number());
    assert!(investigation["steps"].is_array());
    assert!(investigation["approval_gates"].is_array());
    assert!(investigation["evidence_citations"].is_array());
    let investigation_steps = investigation["steps"]
        .as_array()
        .expect("lambda cost investigation steps should be an array");
    assert!(investigation_steps.iter().all(|step| {
        let tool_name = step["tool_name"].as_str().unwrap_or_default();
        tool_name.starts_with("lambda.")
            && !tool_name.contains("execute")
            && !tool_name.contains("delete")
            && !tool_name.contains("update_function")
    }));
    assert!(investigation_steps.iter().all(|step| {
        step["tool_mode"] == "read_only"
            || (step["tool_mode"] == "approval_required"
                && step["tool_name"] == "lambda.cost.prepare_approval_plan")
    }));
    let remediation = &cost["remediation_workflow"];
    assert_eq!(remediation["workflow_id"], "lambda_cost_safe_remediation");
    assert_eq!(remediation["read_only_mode"], true);
    assert_eq!(
        remediation["rbac_permission"],
        "aws.lambda.cost.remediation.approve"
    );
    assert_eq!(remediation["audit_stream"], "lambda_cost_remediation_audit");
    assert!(remediation["stale_data_blocks_execution"].is_boolean());
    assert!(remediation["actions"].is_array());
    assert!(remediation["approval_gates"].is_array());
    let remediation_actions = remediation["actions"]
        .as_array()
        .expect("lambda cost remediation actions should be an array");
    assert!(remediation_actions.iter().all(|action| {
        action["dry_run"] == true
            && action["requires_approval"] == true
            && action["audit_event_type"] == "lambda.cost.remediation.dry_run_planned"
            && action["idempotency_key"]
                .as_str()
                .unwrap_or_default()
                .starts_with("lambda-cost-")
            && action["validation_steps"].is_array()
            && action["evidence_reason_codes"].is_array()
    }));
    let slo_policy = &cost["slo_policy_tracking"];
    assert_eq!(slo_policy["workflow_id"], "lambda_cost_slo_policy");
    assert_eq!(slo_policy["read_only_mode"], true);
    assert_eq!(slo_policy["freshness_required"], true);
    assert_eq!(
        slo_policy["objective"]["objective_id"],
        "lambda-cost-score-min-90"
    );
    assert!(slo_policy["objective"]["status"].is_string());
    assert_eq!(slo_policy["objective"]["target_score_min"], 90);
    assert!(slo_policy["objective"]["current_score"].is_number());
    assert!(slo_policy["objective"]["trend_direction"].is_string());
    assert!(slo_policy["objective"]["failed_rule_count"].is_number());
    assert!(slo_policy["objective"]["affected_resource_count"].is_number());
    assert!(slo_policy["objective"]["owner_filters"].is_array());
    assert!(slo_policy["objective"]["environment_filters"].is_array());
    assert!(slo_policy["objective"]["application_filters"].is_array());
    assert!(slo_policy["objective"]["notification_targets"].is_array());
    assert!(slo_policy["objective"]["policy_state"].is_string());
    assert!(slo_policy["objective"]["status_history"].is_array());
    assert!(slo_policy["evidence_reason_codes"].is_array());
    let forecasting = &cost["forecasting"];
    assert_eq!(forecasting["workflow_id"], "lambda_cost_forecasting");
    assert_eq!(forecasting["read_only_mode"], true);
    assert_eq!(forecasting["baseline_window_days"], 30);
    assert_eq!(forecasting["forecast_horizon_days"], 30);
    assert_eq!(forecasting["confidence_level"], 80);
    assert!(forecasting["forecast_band"]["expected_monthly_cost_index"].is_number());
    assert!(forecasting["risk_level"].is_string());
    assert!(forecasting["capacity_risk"].is_string());
    assert!(forecasting["backtesting_fixture_status"].is_string());
    assert!(forecasting["threshold_controls"].is_array());
    assert!(forecasting["what_if_inputs"].is_array());
    assert!(forecasting["blocked_by_stale_data"].is_boolean());
    assert!(forecasting["blast_radius_summary"].is_string());
    assert!(forecasting["missing_data_reason_codes"].is_array());
    assert!(forecasting["risk_drivers"].is_array());
    assert!(forecasting["evidence_reason_codes"].is_array());
    let reporting = &cost["reporting"];
    assert_eq!(reporting["workflow_id"], "lambda_cost_reporting");
    assert_eq!(reporting["read_only_mode"], true);
    assert!(reporting["scheduled_delivery_state"].is_string());
    assert!(reporting["stale_data_blocks_delivery"].is_boolean());
    assert!(reporting["portfolio_summary_ready"].is_boolean());
    assert!(reporting["workload_summary_ready"].is_boolean());
    assert!(reporting["export_formats"].is_array());
    assert_eq!(reporting["saved_view_id"], "lambda-cost-posture-report");
    assert_eq!(
        reporting["executive_summary"]["report_id"],
        "lambda-cost-executive-summary"
    );
    assert!(reporting["executive_summary"]["rules_failed"].is_number());
    assert!(reporting["executive_summary"]["affected_resources"].is_array());
    assert!(reporting["executive_summary"]["blast_radius_summary"].is_string());
    assert_eq!(
        reporting["engineering_backlog"]["report_id"],
        "lambda-cost-engineering-backlog"
    );
    assert_eq!(reporting["engineering_backlog"]["page"], 0);
    assert_eq!(reporting["engineering_backlog"]["page_size"], 50);
    assert!(reporting["engineering_backlog"]["rows"].is_array());
    assert_eq!(
        reporting["incident_review"]["report_id"],
        "lambda-cost-incident-review"
    );
    assert!(reporting["incident_review"]["rows"].is_array());
    assert!(reporting["missing_data_reason_codes"].is_array());
    assert!(reporting["evidence_reason_codes"].is_array());
    assert_eq!(cost["telemetry"]["workflow_id"], "lambda_cost_telemetry");
    assert_eq!(cost["telemetry"]["cloudwatch_namespace"], "AWS/Lambda");
    assert_eq!(cost["telemetry"]["cloudwatch_dimension"], "FunctionName");
    assert_eq!(cost["telemetry"]["read_only_mode"], true);
    assert_eq!(
        security["assessment_scope"],
        "lambda_security_metrics_logs_events_and_code_signing_telemetry"
    );
    assert_eq!(
        security["posture"]["workflow_id"],
        "lambda_security_posture"
    );
    assert_eq!(
        security["posture"]["rule_pack_id"],
        "lambda-security-posture-rules-v1"
    );
    assert_eq!(
        security["posture"]["audit_event_type"],
        "lambda_security_posture_evaluated"
    );
    assert_eq!(security["posture"]["rules_evaluated"], 6);
    assert_eq!(
        security["triage_context"]["workflow_id"],
        "lambda_security_triage_context"
    );
    assert_eq!(
        security["triage_context"]["context_builder_id"],
        "lambda-security-deterministic-context-v1"
    );
    assert_eq!(
        security["triage_context"]["prompt_template_id"],
        "lambda-security-ai-triage-v1"
    );
    assert_eq!(
        security["triage_context"]["audit_event_type"],
        "lambda_security_ai_triage_context_built"
    );
    assert_eq!(
        security["agentic_investigation"]["workflow_id"],
        "lambda_security_agentic_investigation"
    );
    assert_eq!(
        security["agentic_investigation"]["default_tool_mode"],
        "read_only"
    );
    assert_eq!(security["agentic_investigation"]["replay_required"], true);
    assert!(security["agentic_investigation"]["steps"].is_array());
    assert!(security["agentic_investigation"]["approval_gates"].is_array());
    assert_eq!(
        security["remediation_workflow"]["workflow_id"],
        "lambda_security_safe_remediation"
    );
    assert_eq!(
        security["remediation_workflow"]["rbac_permission"],
        "aws.lambda.security.remediation.approve"
    );
    assert_eq!(
        security["remediation_workflow"]["audit_stream"],
        "lambda_security_remediation_audit"
    );
    assert_eq!(
        security["slo_policy_tracking"]["workflow_id"],
        "lambda_security_slo_policy"
    );
    assert_eq!(
        security["slo_policy_tracking"]["objective"]["objective_id"],
        "lambda-security-score-min-95"
    );
    assert_eq!(
        security["forecasting"]["workflow_id"],
        "lambda_security_forecasting"
    );
    assert!(security["forecasting"]["forecast_band"]["expected_monthly_cost_index"].is_number());
    assert!(security["forecasting"]["capacity_risk"].is_string());
    assert_eq!(
        security["reporting"]["workflow_id"],
        "lambda_security_reporting"
    );
    assert_eq!(
        security["reporting"]["saved_view_id"],
        "lambda-security-posture-report"
    );
    assert_eq!(
        security["reporting"]["executive_summary"]["report_id"],
        "lambda-security-executive-summary"
    );
    assert_eq!(
        security["reporting"]["engineering_backlog"]["report_id"],
        "lambda-security-engineering-backlog"
    );
    assert_eq!(
        security["reporting"]["incident_review"]["report_id"],
        "lambda-security-incident-review"
    );
    assert_eq!(
        security["telemetry"]["workflow_id"],
        "lambda_security_telemetry"
    );
    assert_eq!(security["telemetry"]["cloudwatch_namespace"], "AWS/Lambda");
    assert!(security["telemetry"]["required_metrics"]
        .as_array()
        .expect("lambda security required metrics")
        .iter()
        .any(|metric| metric == "Errors"));
    assert_eq!(
        resilience["assessment_scope"],
        "lambda_resilience_metrics_logs_events_quotas_limits_and_health_signals"
    );
    assert_eq!(
        resilience["posture"]["workflow_id"],
        "lambda_resilience_posture"
    );
    assert_eq!(
        resilience["posture"]["rule_pack_id"],
        "lambda-resilience-posture-rules-v1"
    );
    assert_eq!(
        resilience["triage_context"]["workflow_id"],
        "lambda_resilience_triage_context"
    );
    assert_eq!(
        resilience["triage_context"]["context_builder_id"],
        "lambda-resilience-deterministic-context-v1"
    );
    assert_eq!(
        resilience["agentic_investigation"]["workflow_id"],
        "lambda_resilience_agentic_investigation"
    );
    assert!(resilience["agentic_investigation"]["steps"].is_array());
    assert!(resilience["agentic_investigation"]["approval_gates"].is_array());
    assert_eq!(
        resilience["remediation_workflow"]["workflow_id"],
        "lambda_resilience_safe_remediation"
    );
    assert_eq!(
        resilience["remediation_workflow"]["rbac_permission"],
        "aws.lambda.resilience.remediation.approve"
    );
    assert_eq!(
        resilience["slo_policy_tracking"]["workflow_id"],
        "lambda_resilience_slo_policy"
    );
    assert_eq!(
        resilience["slo_policy_tracking"]["objective"]["objective_id"],
        "lambda-resilience-score-min-95"
    );
    assert_eq!(
        resilience["forecasting"]["workflow_id"],
        "lambda_resilience_forecasting"
    );
    assert!(
        resilience["forecasting"]["forecast_band"]["expected_recovery_exposure_index"].is_number()
    );
    assert!(resilience["forecasting"]["recovery_capacity_risk"].is_string());
    assert_eq!(
        resilience["reporting"]["workflow_id"],
        "lambda_resilience_reporting"
    );
    assert_eq!(
        resilience["reporting"]["saved_view_id"],
        "lambda-resilience-posture-report"
    );
    assert_eq!(
        resilience["reporting"]["executive_summary"]["report_id"],
        "lambda-resilience-executive-summary"
    );
    assert_eq!(
        resilience["reporting"]["engineering_backlog"]["report_id"],
        "lambda-resilience-engineering-backlog"
    );
    assert_eq!(
        resilience["reporting"]["incident_review"]["report_id"],
        "lambda-resilience-incident-review"
    );
    assert!(resilience["reporting"]["missing_data_reason_codes"].is_array());
    assert!(resilience["reporting"]["evidence_reason_codes"].is_array());
    assert_eq!(
        resilience["telemetry"]["workflow_id"],
        "lambda_resilience_telemetry"
    );
    assert_eq!(
        resilience["telemetry"]["cloudwatch_namespace"],
        "AWS/Lambda"
    );
    assert_eq!(
        resilience["telemetry"]["cloudwatch_dimension"],
        "FunctionName"
    );
    assert!(resilience["telemetry"]["required_metrics"]
        .as_array()
        .expect("lambda resilience required metrics")
        .iter()
        .any(|metric| metric == "Errors"));
    assert!(resilience["telemetry"]["stale_data_blocks_delivery"].is_boolean());
    assert_eq!(
        performance["assessment_scope"],
        "lambda_performance_duration_error_throttle_and_concurrency_telemetry"
    );
    assert_eq!(
        performance["posture"]["workflow_id"],
        "lambda_performance_posture"
    );
    assert_eq!(
        performance["posture"]["rule_pack_id"],
        "lambda-performance-posture-rules-v1"
    );
    assert_eq!(
        performance["posture"]["audit_event_type"],
        "lambda_performance_posture_evaluated"
    );
    assert_eq!(performance["posture"]["rules_evaluated"], 5);
    assert_eq!(
        performance["triage_context"]["workflow_id"],
        "lambda_performance_triage_context"
    );
    assert_eq!(
        performance["triage_context"]["context_builder_id"],
        "lambda-performance-deterministic-context-v1"
    );
    assert_eq!(
        performance["triage_context"]["prompt_template_id"],
        "lambda-performance-ai-triage-v1"
    );
    assert_eq!(
        performance["triage_context"]["audit_event_type"],
        "lambda_performance_ai_triage_context_built"
    );
    assert_eq!(
        performance["triage_context"]["freshness"]["freshness_source"],
        "lambda_inventory_last_synced_at"
    );
    assert_eq!(
        performance["telemetry"]["workflow_id"],
        "lambda_performance_telemetry"
    );
    assert_eq!(
        performance["telemetry"]["cloudwatch_namespace"],
        "AWS/Lambda"
    );
    assert_eq!(
        performance["telemetry"]["cloudwatch_dimension"],
        "FunctionName"
    );
    assert!(performance["telemetry"]["required_metrics"]
        .as_array()
        .expect("lambda performance required metrics")
        .iter()
        .any(|metric| metric == "ConcurrentExecutions"));
    assert_eq!(
        scalability["assessment_scope"],
        "lambda_scalability_invocation_throttle_and_concurrency_telemetry"
    );
    assert_eq!(
        scalability["posture"]["workflow_id"],
        "lambda_scalability_posture"
    );
    assert_eq!(
        scalability["posture"]["rule_pack_id"],
        "lambda-scalability-posture-rules-v1"
    );
    assert_eq!(
        scalability["posture"]["audit_event_type"],
        "lambda_scalability_posture_evaluated"
    );
    assert_eq!(scalability["posture"]["rules_evaluated"], 5);
    assert_eq!(
        scalability["triage_context"]["workflow_id"],
        "lambda_scalability_triage_context"
    );
    assert_eq!(
        scalability["triage_context"]["context_builder_id"],
        "lambda-scalability-deterministic-context-v1"
    );
    assert_eq!(
        scalability["triage_context"]["prompt_template_id"],
        "lambda-scalability-ai-triage-v1"
    );
    assert_eq!(
        scalability["triage_context"]["audit_event_type"],
        "lambda_scalability_ai_triage_context_built"
    );
    assert_eq!(
        scalability["telemetry"]["workflow_id"],
        "lambda_scalability_telemetry"
    );
    assert!(scalability["telemetry"]["required_metrics"]
        .as_array()
        .expect("lambda scalability required metrics")
        .iter()
        .any(|metric| metric == "ConcurrentExecutions"));
}

#[tokio::test]
async fn s3_pillar_reports_contract() {
    if !aws_tests_enabled() {
        println!("Skipping s3_pillar_reports_contract because ENABLE_AWS_TESTS is not true");
        return;
    }

    let base = base_url().await;
    let client = Client::new();
    let resp = client
        .get(format!(
            "{}/api/aws/inventory/s3/pillars?account_id=123456789012",
            base
        ))
        .send()
        .await
        .expect("s3 pillar report request failed");
    assert_eq!(resp.status(), 200);
    let body: Value = resp.json().await.expect("invalid JSON body");
    assert_eq!(body["resource_type"], "S3Bucket");
    let reports = body["reports"].as_array().expect("reports array");
    assert_eq!(reports.len(), 3);
}

#[tokio::test]
async fn storage_and_database_pillar_reports_contract() {
    if !aws_tests_enabled() {
        println!(
            "Skipping storage_and_database_pillar_reports_contract because ENABLE_AWS_TESTS is not true"
        );
        return;
    }

    let base = base_url().await;
    let client = Client::new();
    for (path, resource_type, expected_reports) in [
        ("rds", "RdsInstance", 3),
        ("ebs", "EbsVolume", 3),
        ("efs", "EfsFileSystem", 3),
        ("ecs", "EcsClusterAndService", 3),
        ("eks", "EksCluster", 3),
        ("dynamodb", "DynamoDbTable", 3),
        ("sqs", "SqsQueue", 3),
        ("sns", "SnsTopic", 3),
        ("kinesis", "KinesisStream", 3),
        ("elasticache", "ElasticacheCluster", 3),
        ("opensearch", "OpenSearchDomain", 3),
        ("vpc", "Vpc", 3),
        ("iam", "IamUserRolePolicyAndGroup", 3),
        ("cloudfront", "CloudFrontDistribution", 3),
        ("elb", "AlbNlbAndElb", 3),
        ("apigateway", "ApiGatewayRestApiStageAndMethod", 3),
        ("cloudwatch", "CloudWatchAlarmAndDashboard", 3),
        ("appsync", "AppSyncApi", 3),
        ("glacier", "GlacierArchive", 3),
        ("storagegateway", "StorageGateway", 3),
        ("kinesisanalytics", "KinesisAnalyticsApp", 3),
        ("subnet", "Subnet", 3),
        ("securitygroup", "SecurityGroup", 3),
        ("natgateway", "NatGateway", 3),
        ("internetgateway", "InternetGateway", 3),
        ("routetable", "RouteTable", 3),
        ("networkacl", "NetworkAcl", 3),
        ("fargate", "FargateProfile", 3),
        ("kms", "KmsKey", 3),
        ("acm", "AcmCertificate", 3),
        ("cloudtrail", "CloudTrailTrail", 3),
        ("config", "ConfigRule", 7),
        ("eventbridge", "EventBridgeRule", 7),
        ("stepfunctions", "StepFunction", 7),
        ("apprunner", "AppRunnerService", 3),
        ("athena", "AthenaWorkgroup", 3),
        ("ssm", "SsmDocument", 3),
        ("backup", "BackupVault", 3),
        ("batch", "BatchComputeEnv", 3),
        ("emr", "EmrCluster", 3),
        ("globalaccelerator", "GlobalAccelerator", 3),
        ("glue", "GlueDatabase", 3),
        ("redshift", "RedshiftCluster", 3),
        ("waf", "WafWebAcl", 3),
        ("autoscaling", "AutoScalingGroup", 3),
        ("cloudwatchmetrics", "CloudWatchMetric", 3),
        ("cloudwatchlogs", "CloudWatchLogGroup", 3),
        ("route53", "Route53HostedZone", 3),
        ("transitgateway", "TransitGateway", 3),
        ("secretsmanager", "SecretsManagerSecret", 3),
        ("aurora", "AuroraCluster", 3),
        ("msk", "MskCluster", 3),
        ("guardduty", "GuardDutyDetector", 3),
        ("securityhub", "SecurityHubHub", 3),
        ("inspector", "InspectorAccountCoverage", 3),
        ("macie", "MacieAccount", 3),
        ("organizations", "OrganizationsOrganization", 3),
        ("controltower", "ControlTowerLandingZone", 3),
        ("servicecatalog", "ServiceCatalogPortfolio", 3),
        ("trustedadvisor", "TrustedAdvisorAccount", 3),
        ("computeoptimizer", "ComputeOptimizerAccount", 3),
        ("health", "HealthAccount", 3),
        ("resiliencehub", "ResilienceHubAccount", 3),
        ("documentdb", "DocumentDbCluster", 3),
        ("neptune", "NeptuneCluster", 3),
        ("memorydb", "MemoryDbCluster", 3),
        ("elasticbeanstalk", "ElasticBeanstalkEnvironment", 3),
        ("datasync", "DataSyncTask", 3),
        ("fsx", "FsxFileSystem", 3),
        ("timestream", "TimestreamTable", 3),
        ("firehose", "FirehoseDeliveryStream", 3),
        ("lakeformation", "LakeFormationDataLake", 3),
        ("lightsail", "LightsailResource", 3),
        ("quicksight", "QuickSightAsset", 3),
        ("dms", "DmsResource", 3),
        ("mgn", "MgnResource", 3),
        ("drs", "DrsResource", 3),
        ("bedrock", "BedrockResource", 3),
        ("sagemaker", "SageMakerResource", 3),
        ("textract", "TextractResource", 3),
        ("comprehend", "ComprehendResource", 3),
        ("amazonmq", "AmazonMqBroker", 3),
        ("privatelink", "VpcEndpoint", 3),
        ("shield", "ShieldProtection", 3),
    ] {
        let resp = client
            .get(format!(
                "{}/api/aws/inventory/{}/pillars?account_id=123456789012",
                base, path
            ))
            .send()
            .await
            .unwrap_or_else(|e| panic!("{} pillar report request failed: {}", path, e));
        assert_eq!(resp.status(), 200, "endpoint {}", path);
        let body: Value = resp.json().await.expect("invalid JSON body");
        assert_eq!(body["resource_type"], resource_type);
        assert_eq!(
            body["reports"].as_array().expect("reports").len(),
            expected_reports,
            "endpoint {}",
            path
        );
    }
}

#[tokio::test]
async fn autoscaling_cost_pillar_reports_posture_contract() {
    if !aws_tests_enabled() {
        println!(
            "Skipping autoscaling_cost_pillar_reports_posture_contract because ENABLE_AWS_TESTS is not true"
        );
        return;
    }

    let base = base_url().await;
    let client = Client::new();
    let resp = client
        .get(format!(
            "{}/api/aws/inventory/autoscaling/pillars?account_id=123456789012&pillar=cost",
            base
        ))
        .send()
        .await
        .expect("autoscaling cost pillar request failed");
    assert_eq!(resp.status(), 200);
    let body: Value = resp.json().await.expect("invalid JSON body");
    assert_eq!(body["resource_type"], "AutoScalingGroup");
    let reports = body["reports"].as_array().expect("reports array");
    assert_eq!(reports.len(), 1);
    assert_eq!(reports[0]["pillar"], "cost");
    assert_eq!(
        reports[0]["assessment_scope"],
        "autoscaling_cost_capacity_tags_and_group_metrics"
    );
    assert!(matches!(
        reports[0]["posture"]["status"].as_str(),
        Some("pass" | "fail")
    ));
    assert_eq!(reports[0]["posture"]["rules_evaluated"], 7);
    assert!(reports[0]["posture"]["rules_failed"].is_number());
    assert!(reports[0]["posture"]["affected_resources"].is_array());

    let rules = reports[0]["posture"]["rules"]
        .as_array()
        .expect("autoscaling cost posture rules");
    assert_eq!(rules.len(), 7);
    let expected_rules = [
        ("asg-cost-inventory-freshness", "ASG_INV_STALE_DATA"),
        (
            "asg-cost-telemetry-collection-metadata-present",
            "ASG_TEL_MISSING_COLLECTION_METADATA",
        ),
        (
            "asg-cost-telemetry-collection-errors-clear",
            "ASG_TEL_COLLECTION_ERRORS",
        ),
        (
            "asg-cost-capacity-telemetry-present",
            "ASG_COST_MISSING_CAPACITY_TELEMETRY",
        ),
        (
            "asg-cost-group-metrics-telemetry-present",
            "ASG_COST_MISSING_GROUP_METRICS_TELEMETRY",
        ),
        ("asg-cost-allocation-tags-present", "ASG_COST_NO_TAGS"),
        ("asg-cost-scale-in-capable", "ASG_COST_FIXED_SIZE"),
    ];
    for (rule, (expected_rule_id, expected_reason_code)) in rules.iter().zip(expected_rules) {
        assert_eq!(rule["rule_id"], expected_rule_id);
        assert_eq!(rule["reason_codes"][0], expected_reason_code);
        assert!(rule["suppression_supported"].is_boolean());
        assert!(rule["assignment_supported"].is_boolean());
        assert!(rule["affected_resources"].is_array());
    }
    let triage = &reports[0]["triage_context"];
    assert_eq!(triage["workflow_id"], "autoscaling_cost_triage_context");
    assert_eq!(
        triage["context_builder_id"],
        "autoscaling-cost-deterministic-context-v1"
    );
    assert_eq!(
        triage["prompt_template_id"],
        "autoscaling-cost-ai-triage-v1"
    );
    assert_eq!(triage["generation_mode"], "deterministic_no_llm");
    assert_eq!(triage["max_prompt_tokens"], 1200);
    assert_eq!(triage["provider_routing"][0], "primary_ops_llm");
    assert_eq!(
        triage["audit_event_type"],
        "autoscaling_ai_triage_context_built"
    );
    assert_eq!(triage["guardrails"]["read_only_mode"], true);
    assert_eq!(triage["guardrails"]["evidence_required"], true);
    assert_eq!(triage["guardrails"]["separate_facts_from_hypotheses"], true);
    assert_eq!(triage["guardrails"]["ask_for_missing_data"], true);
    assert_eq!(triage["guardrails"]["no_llm_invocation"], true);
    assert_eq!(triage["guardrails"]["no_mutation_planning"], true);
    assert!(triage["facts"].is_array());
    assert!(triage["hypotheses"].is_array());
    assert!(triage["missing_data_questions"].is_array());
    assert!(triage["evidence_citations"].is_array());
    assert_eq!(
        reports[0]["agentic_investigation"]["workflow_id"],
        "autoscaling_cost_agentic_investigation"
    );
    assert_eq!(
        reports[0]["agentic_investigation"]["default_tool_mode"],
        "read_only"
    );
    assert_eq!(reports[0]["agentic_investigation"]["replay_required"], true);
    assert!(reports[0]["agentic_investigation"]["steps"].is_array());
    assert!(reports[0]["agentic_investigation"]["approval_gates"].is_array());
    assert!(reports[0]["agentic_investigation"]["evidence_citations"].is_array());
    let investigation_steps = reports[0]["agentic_investigation"]["steps"]
        .as_array()
        .expect("autoscaling investigation steps should be an array");
    assert!(investigation_steps.iter().all(|step| {
        let tool_name = step["tool_name"].as_str().unwrap_or_default();
        tool_name.starts_with("autoscaling.")
            && !tool_name.contains("execute")
            && !tool_name.contains("delete")
    }));
    assert!(investigation_steps.iter().all(|step| {
        step["tool_mode"] == "read_only"
            || (step["tool_mode"] == "approval_required"
                && step["tool_name"] == "autoscaling.cost.prepare_approval_plan")
    }));
    assert_eq!(
        reports[0]["remediation_workflow"]["workflow_id"],
        "autoscaling_cost_safe_remediation"
    );
    assert_eq!(reports[0]["remediation_workflow"]["read_only_mode"], true);
    assert_eq!(
        reports[0]["remediation_workflow"]["rbac_permission"],
        "aws.autoscaling.cost.remediation.approve"
    );
    assert_eq!(
        reports[0]["remediation_workflow"]["audit_stream"],
        "autoscaling_cost_remediation_audit"
    );
    assert!(reports[0]["remediation_workflow"]["actions"].is_array());
    assert!(reports[0]["remediation_workflow"]["approval_gates"].is_array());
    let remediation_actions = reports[0]["remediation_workflow"]["actions"]
        .as_array()
        .expect("autoscaling remediation actions should be an array");
    assert!(remediation_actions.iter().all(|action| {
        action["dry_run"] == true
            && action["requires_approval"] == true
            && action["audit_event_type"] == "autoscaling.cost.remediation.dry_run_planned"
            && action["idempotency_key"]
                .as_str()
                .unwrap_or_default()
                .starts_with("autoscaling-cost-")
    }));
    assert_eq!(
        reports[0]["slo_policy_tracking"]["workflow_id"],
        "autoscaling_cost_slo_policy"
    );
    assert_eq!(reports[0]["slo_policy_tracking"]["read_only_mode"], true);
    assert_eq!(
        reports[0]["slo_policy_tracking"]["objective"]["objective_id"],
        "autoscaling-cost-score-min-90"
    );
    assert_eq!(
        reports[0]["slo_policy_tracking"]["objective"]["target_score_min"],
        90
    );
    assert!(reports[0]["slo_policy_tracking"]["objective"]["status"]
        .as_str()
        .is_some());
    assert!(reports[0]["slo_policy_tracking"]["objective"]["owner_filters"].is_array());
    assert!(reports[0]["slo_policy_tracking"]["objective"]["environment_filters"].is_array());
    assert!(reports[0]["slo_policy_tracking"]["objective"]["application_filters"].is_array());
    assert!(reports[0]["slo_policy_tracking"]["objective"]["notification_targets"].is_array());
    assert!(reports[0]["slo_policy_tracking"]["objective"]["status_history"].is_array());
    assert!(reports[0]["slo_policy_tracking"]["evidence_reason_codes"].is_array());
    assert_eq!(
        reports[0]["forecasting"]["workflow_id"],
        "autoscaling_cost_forecasting"
    );
    assert_eq!(reports[0]["forecasting"]["read_only_mode"], true);
    assert_eq!(reports[0]["forecasting"]["baseline_window_days"], 30);
    assert_eq!(reports[0]["forecasting"]["forecast_horizon_days"], 30);
    assert_eq!(reports[0]["forecasting"]["confidence_level"], 80);
    assert!(reports[0]["forecasting"]["forecast_band"]["expected_monthly_cost_index"].is_number());
    assert!(reports[0]["forecasting"]["risk_level"].as_str().is_some());
    assert!(reports[0]["forecasting"]["capacity_risk"].is_string());
    assert!(reports[0]["forecasting"]["backtesting_fixture_status"].is_string());
    assert!(reports[0]["forecasting"]["threshold_controls"].is_array());
    assert!(reports[0]["forecasting"]["what_if_inputs"].is_array());
    assert!(reports[0]["forecasting"]["blocked_by_stale_data"].is_boolean());
    assert!(reports[0]["forecasting"]["blast_radius_summary"].is_string());
    assert!(reports[0]["forecasting"]["missing_data_reason_codes"].is_array());
    assert!(reports[0]["forecasting"]["risk_drivers"].is_array());
    assert!(reports[0]["forecasting"]["evidence_reason_codes"].is_array());
    assert_eq!(
        reports[0]["reporting"]["workflow_id"],
        "autoscaling_cost_reporting"
    );
    assert_eq!(reports[0]["reporting"]["read_only_mode"], true);
    assert!(reports[0]["reporting"]["scheduled_delivery_state"]
        .as_str()
        .is_some());
    assert!(reports[0]["reporting"]["portfolio_summary_ready"].is_boolean());
    assert!(reports[0]["reporting"]["workload_summary_ready"].is_boolean());
    assert!(reports[0]["reporting"]["stale_data_blocks_delivery"].is_boolean());
    assert!(reports[0]["reporting"]["export_formats"].is_array());
    assert_eq!(
        reports[0]["reporting"]["executive_summary"]["report_id"],
        "autoscaling-cost-executive-summary"
    );
    assert!(reports[0]["reporting"]["executive_summary"]["top_reason_codes"].is_array());
    assert!(reports[0]["reporting"]["executive_summary"]["blast_radius_summary"].is_string());
    assert_eq!(
        reports[0]["reporting"]["engineering_backlog"]["report_id"],
        "autoscaling-cost-engineering-backlog"
    );
    assert!(reports[0]["reporting"]["engineering_backlog"]["rows"].is_array());
    assert_eq!(
        reports[0]["reporting"]["incident_review"]["report_id"],
        "autoscaling-cost-incident-review"
    );
    assert!(reports[0]["reporting"]["incident_review"]["rows"].is_array());
    assert!(reports[0]["reporting"]["missing_data_reason_codes"].is_array());
    assert!(reports[0]["reporting"]["evidence_reason_codes"].is_array());
}

#[tokio::test]
async fn autoscaling_security_pillar_reports_posture_contract() {
    if !aws_tests_enabled() {
        println!(
            "Skipping autoscaling_security_pillar_reports_posture_contract because ENABLE_AWS_TESTS is not true"
        );
        return;
    }

    let base = base_url().await;
    let client = Client::new();
    let resp = client
        .get(format!(
            "{}/api/aws/inventory/autoscaling/pillars?account_id=123456789012&pillar=security",
            base
        ))
        .send()
        .await
        .expect("autoscaling security pillar request failed");
    assert_eq!(resp.status(), 200);
    let body: Value = resp.json().await.expect("invalid JSON body");
    assert_eq!(body["resource_type"], "AutoScalingGroup");
    let reports = body["reports"].as_array().expect("reports array");
    assert_eq!(reports.len(), 1);
    assert_eq!(reports[0]["pillar"], "security");
    assert_eq!(
        reports[0]["assessment_scope"],
        "autoscaling_security_launch_source_and_instance_telemetry"
    );
    assert!(matches!(
        reports[0]["posture"]["status"].as_str(),
        Some("pass" | "fail")
    ));
    assert_eq!(reports[0]["posture"]["rules_evaluated"], 6);
    assert!(reports[0]["posture"]["rules_failed"].is_number());
    assert!(reports[0]["posture"]["affected_resources"].is_array());
    let triage = &reports[0]["triage_context"];
    assert_eq!(triage["workflow_id"], "autoscaling_security_triage_context");
    assert_eq!(
        triage["context_builder_id"],
        "autoscaling-security-deterministic-context-v1"
    );
    assert_eq!(
        triage["prompt_template_id"],
        "autoscaling-security-ai-triage-v1"
    );
    assert_eq!(triage["generation_mode"], "deterministic_no_llm");
    assert_eq!(triage["max_prompt_tokens"], 1200);
    assert_eq!(triage["provider_routing"][0], "primary_ops_llm");
    assert_eq!(
        triage["audit_event_type"],
        "autoscaling_security_ai_triage_context_built"
    );
    assert_eq!(triage["guardrails"]["read_only_mode"], true);
    assert_eq!(triage["guardrails"]["evidence_required"], true);
    assert_eq!(triage["guardrails"]["separate_facts_from_hypotheses"], true);
    assert_eq!(triage["guardrails"]["ask_for_missing_data"], true);
    assert_eq!(triage["guardrails"]["no_llm_invocation"], true);
    assert_eq!(triage["guardrails"]["no_mutation_planning"], true);
    assert!(triage["facts"].is_array());
    assert!(triage["hypotheses"].is_array());
    assert!(triage["missing_data_questions"].is_array());
    assert!(triage["evidence_citations"].is_array());
    let investigation = &reports[0]["agentic_investigation"];
    assert_eq!(
        investigation["workflow_id"],
        "autoscaling_security_agentic_investigation"
    );
    assert_eq!(investigation["default_tool_mode"], "read_only");
    assert_eq!(investigation["replay_required"], true);
    assert!(investigation["max_tool_calls"].is_number());
    assert!(investigation["max_evidence_citations"].is_number());
    assert!(investigation["steps"].is_array());
    assert!(investigation["approval_gates"].is_array());
    assert!(investigation["evidence_citations"].is_array());
    let remediation = &reports[0]["remediation_workflow"];
    assert_eq!(
        remediation["workflow_id"],
        "autoscaling_security_safe_remediation"
    );
    assert_eq!(remediation["read_only_mode"], true);
    assert_eq!(
        remediation["rbac_permission"],
        "aws.autoscaling.security.remediation.approve"
    );
    assert_eq!(
        remediation["audit_stream"],
        "autoscaling_security_remediation_audit"
    );
    assert!(remediation["actions"].is_array());
    assert!(remediation["approval_gates"].is_array());
    let slo = &reports[0]["slo_policy_tracking"];
    assert_eq!(slo["workflow_id"], "autoscaling_security_slo_policy");
    assert_eq!(slo["read_only_mode"], true);
    assert_eq!(slo["freshness_required"], true);
    assert_eq!(
        slo["objective"]["objective_id"],
        "autoscaling-security-score-min-95"
    );
    assert_eq!(slo["objective"]["target_score_min"], 95);
    assert!(slo["objective"]["status"].as_str().is_some());
    assert!(slo["objective"]["trend_direction"].as_str().is_some());
    assert!(slo["objective"]["policy_state"].as_str().is_some());
    assert!(slo["objective"]["owner_filters"].is_array());
    assert!(slo["objective"]["environment_filters"].is_array());
    assert!(slo["objective"]["application_filters"].is_array());
    assert!(slo["objective"]["notification_targets"].is_array());
    assert!(slo["objective"]["status_history"].is_array());
    assert!(slo["evidence_reason_codes"].is_array());
    let forecast = &reports[0]["forecasting"];
    assert_eq!(forecast["workflow_id"], "autoscaling_security_forecasting");
    assert_eq!(forecast["read_only_mode"], true);
    assert_eq!(forecast["baseline_window_days"], 30);
    assert_eq!(forecast["forecast_horizon_days"], 30);
    assert_eq!(forecast["confidence_level"], 75);
    assert_eq!(forecast["forecast_band"]["horizon_days"], 30);
    assert!(forecast["forecast_band"]["expected_security_exposure_index"].is_number());
    assert!(forecast["risk_level"].as_str().is_some());
    assert!(forecast["exposure_capacity_risk"].is_string());
    assert!(forecast["backtesting_fixture_status"].is_string());
    assert!(forecast["threshold_controls"].is_array());
    assert!(forecast["what_if_inputs"].is_array());
    assert!(forecast["blocked_by_stale_data"].is_boolean());
    assert!(forecast["blast_radius_summary"].is_string());
    assert!(forecast["missing_data_reason_codes"].is_array());
    assert!(forecast["risk_drivers"].is_array());
    assert!(forecast["evidence_reason_codes"].is_array());
    let reporting = &reports[0]["reporting"];
    assert_eq!(reporting["workflow_id"], "autoscaling_security_reporting");
    assert_eq!(reporting["read_only_mode"], true);
    assert!(reporting["scheduled_delivery_state"].as_str().is_some());
    assert!(reporting["stale_data_blocks_delivery"].is_boolean());
    assert!(reporting["portfolio_summary_ready"].is_boolean());
    assert!(reporting["workload_summary_ready"].is_boolean());
    assert!(reporting["export_formats"].is_array());
    assert_eq!(
        reporting["saved_view_id"],
        "autoscaling-security-posture-report"
    );
    assert_eq!(
        reporting["executive_summary"]["report_id"],
        "autoscaling-security-executive-summary"
    );
    assert!(reporting["engineering_backlog"]["rows"].is_array());
    assert!(reporting["incident_review"]["rows"].is_array());
    assert!(reporting["missing_data_reason_codes"].is_array());
    assert!(reporting["evidence_reason_codes"].is_array());

    let rules = reports[0]["posture"]["rules"]
        .as_array()
        .expect("autoscaling security posture rules");
    assert_eq!(rules.len(), 6);
    let expected_rules = [
        ("asg-security-inventory-freshness", "ASG_INV_STALE_DATA"),
        (
            "asg-security-telemetry-collection-metadata-present",
            "ASG_TEL_MISSING_COLLECTION_METADATA",
        ),
        (
            "asg-security-telemetry-collection-errors-clear",
            "ASG_TEL_COLLECTION_ERRORS",
        ),
        (
            "asg-security-instance-telemetry-present",
            "ASG_SEC_MISSING_INSTANCE_TELEMETRY",
        ),
        (
            "asg-security-launch-source-modern",
            "ASG_SEC_LEGACY_LAUNCH_CONFIGURATION",
        ),
        (
            "asg-security-launch-source-collected",
            "ASG_SEC_LAUNCH_SOURCE_DATA_NOT_COLLECTED",
        ),
    ];
    for (rule, (expected_rule_id, expected_reason_code)) in rules.iter().zip(expected_rules) {
        assert_eq!(rule["rule_id"], expected_rule_id);
        assert_eq!(rule["reason_codes"][0], expected_reason_code);
        assert!(rule["suppression_supported"].is_boolean());
        assert!(rule["assignment_supported"].is_boolean());
        assert!(rule["affected_resources"].is_array());
    }
}

#[tokio::test]
async fn autoscaling_resilience_pillar_reports_posture_contract() {
    if !aws_tests_enabled() {
        println!(
            "Skipping autoscaling_resilience_pillar_reports_posture_contract because ENABLE_AWS_TESTS is not true"
        );
        return;
    }

    let base = base_url().await;
    let client = Client::new();
    let resp = client
        .get(format!(
            "{}/api/aws/inventory/autoscaling/pillars?account_id=123456789012&pillar=resilience",
            base
        ))
        .send()
        .await
        .expect("autoscaling resilience pillar request failed");
    assert_eq!(resp.status(), 200);
    let body: Value = resp.json().await.expect("invalid JSON body");
    assert_eq!(body["resource_type"], "AutoScalingGroup");
    let reports = body["reports"].as_array().expect("reports array");
    assert_eq!(reports.len(), 1);
    assert_eq!(reports[0]["pillar"], "resilience");
    assert_eq!(
        reports[0]["assessment_scope"],
        "autoscaling_resilience_replacement_health_and_multi_az"
    );
    assert!(matches!(
        reports[0]["posture"]["status"].as_str(),
        Some("pass" | "fail")
    ));
    assert_eq!(reports[0]["posture"]["rules_evaluated"], 10);
    assert!(reports[0]["posture"]["rules_failed"].is_number());
    assert!(reports[0]["posture"]["affected_resources"].is_array());

    let rules = reports[0]["posture"]["rules"]
        .as_array()
        .expect("autoscaling resilience posture rules");
    assert_eq!(rules.len(), 10);
    let expected_rules = [
        ("asg-resilience-inventory-freshness", "ASG_INV_STALE_DATA"),
        (
            "asg-resilience-telemetry-collection-metadata-present",
            "ASG_TEL_MISSING_COLLECTION_METADATA",
        ),
        (
            "asg-resilience-telemetry-collection-errors-clear",
            "ASG_TEL_COLLECTION_ERRORS",
        ),
        (
            "asg-resilience-replacement-telemetry-present",
            "ASG_RES_MISSING_REPLACEMENT_TELEMETRY",
        ),
        (
            "asg-resilience-instance-health-telemetry-present",
            "ASG_RES_MISSING_INSTANCE_HEALTH_TELEMETRY",
        ),
        (
            "asg-resilience-instance-health-clean",
            "ASG_RES_UNHEALTHY_INSTANCE_TELEMETRY",
        ),
        ("asg-resilience-multi-az-placement", "ASG_RES_SINGLE_AZ"),
        (
            "asg-resilience-elb-health-checks",
            "ASG_RES_ELB_HEALTH_CHECK_EC2_ONLY",
        ),
        (
            "asg-resilience-scaling-processes-active",
            "ASG_RES_SUSPENDED_PROCESSES",
        ),
        (
            "asg-resilience-desired-capacity-at-or-above-min",
            "ASG_RES_DESIRED_BELOW_MIN",
        ),
    ];
    for (rule, (expected_rule_id, expected_reason_code)) in rules.iter().zip(expected_rules) {
        assert_eq!(rule["rule_id"], expected_rule_id);
        assert_eq!(rule["reason_codes"][0], expected_reason_code);
        assert!(rule["suppression_supported"].is_boolean());
        assert!(rule["assignment_supported"].is_boolean());
        assert!(rule["affected_resources"].is_array());
    }
    let triage = &reports[0]["triage_context"];
    assert_eq!(
        triage["workflow_id"],
        "autoscaling_resilience_triage_context"
    );
    assert_eq!(
        triage["context_builder_id"],
        "autoscaling-resilience-deterministic-context-v1"
    );
    assert_eq!(
        triage["prompt_template_id"],
        "autoscaling-resilience-ai-triage-v1"
    );
    assert_eq!(triage["generation_mode"], "deterministic_no_llm");
    assert_eq!(triage["max_prompt_tokens"], 1200);
    assert_eq!(triage["provider_routing"][0], "primary_ops_llm");
    assert_eq!(
        triage["audit_event_type"],
        "autoscaling_resilience_ai_triage_context_built"
    );
    assert_eq!(triage["guardrails"]["read_only_mode"], true);
    assert_eq!(triage["guardrails"]["evidence_required"], true);
    assert_eq!(triage["guardrails"]["separate_facts_from_hypotheses"], true);
    assert_eq!(triage["guardrails"]["ask_for_missing_data"], true);
    assert_eq!(triage["guardrails"]["no_llm_invocation"], true);
    assert_eq!(triage["guardrails"]["no_mutation_planning"], true);
    assert!(triage["facts"].is_array());
    assert!(triage["hypotheses"].is_array());
    assert!(triage["missing_data_questions"].is_array());
    assert!(triage["evidence_citations"].is_array());

    let investigation = &reports[0]["agentic_investigation"];
    assert_eq!(
        investigation["workflow_id"],
        "autoscaling_resilience_agentic_investigation"
    );
    assert_eq!(investigation["default_tool_mode"], "read_only");
    assert_eq!(investigation["replay_required"], true);
    assert!(investigation["max_tool_calls"].as_u64().unwrap() > 0);
    assert!(investigation["max_evidence_citations"].as_u64().unwrap() > 0);
    assert!(investigation["steps"].is_array());
    assert!(investigation["approval_gates"].is_array());
    assert!(investigation["evidence_citations"].is_array());
    assert!(investigation["steps"]
        .as_array()
        .unwrap()
        .iter()
        .any(
            |step| step["tool_name"] == "autoscaling.resilience.prepare_approval_plan"
                && step["tool_mode"] == "approval_required"
        ));

    let remediation = &reports[0]["remediation_workflow"];
    assert_eq!(
        remediation["workflow_id"],
        "autoscaling_resilience_safe_remediation"
    );
    assert_eq!(remediation["read_only_mode"], true);
    assert_eq!(
        remediation["rbac_permission"],
        "aws.autoscaling.resilience.remediation.approve"
    );
    assert_eq!(
        remediation["audit_stream"],
        "autoscaling_resilience_remediation_audit"
    );
    assert!(remediation["actions"].is_array());
    assert!(remediation["approval_gates"].is_array());
    let remediation_actions = remediation["actions"]
        .as_array()
        .expect("resilience remediation actions should be an array");
    assert!(remediation_actions.iter().all(|action| {
        action["dry_run"] == true
            && action["requires_approval"] == true
            && !action["approval_gate_id"].is_null()
            && action["audit_event_type"] == "autoscaling.resilience.remediation.dry_run_planned"
            && (action["status"] == "dry_run_pending_approval"
                || action["status"] == "blocked_missing_evidence")
            && action["rollback_note"]
                .as_str()
                .map(|note| note.contains("rollback"))
                .unwrap_or(false)
    }));

    let slo = &reports[0]["slo_policy_tracking"];
    assert_eq!(slo["workflow_id"], "autoscaling_resilience_slo_policy");
    assert_eq!(slo["read_only_mode"], true);
    assert_eq!(slo["freshness_required"], true);
    assert_eq!(
        slo["objective"]["objective_id"],
        "autoscaling-resilience-score-min-95"
    );
    assert_eq!(slo["objective"]["target_score_min"], 95);
    assert!(matches!(
        slo["objective"]["status"].as_str(),
        Some("on_track" | "at_risk" | "breached")
    ));
    assert!(matches!(
        slo["objective"]["trend_direction"].as_str(),
        Some("stable" | "degrading")
    ));
    assert!(matches!(
        slo["objective"]["policy_state"].as_str(),
        Some("active" | "active_with_findings" | "blocked_stale_data")
    ));
    assert!(slo["objective"]["owner_filters"].is_array());
    assert!(slo["objective"]["environment_filters"].is_array());
    assert!(slo["objective"]["application_filters"].is_array());
    assert!(slo["objective"]["notification_targets"].is_array());
    assert!(slo["objective"]["status_history"].is_array());
    assert!(slo["evidence_reason_codes"].is_array());

    let forecast = &reports[0]["forecasting"];
    assert_eq!(
        forecast["workflow_id"],
        "autoscaling_resilience_forecasting"
    );
    assert_eq!(forecast["read_only_mode"], true);
    assert_eq!(forecast["baseline_window_days"], 30);
    assert_eq!(forecast["forecast_horizon_days"], 30);
    assert_eq!(forecast["confidence_level"], 75);
    assert_eq!(forecast["forecast_band"]["horizon_days"], 30);
    assert!(forecast["forecast_band"]["expected_recovery_exposure_index"].is_number());
    assert!(forecast["risk_level"].as_str().is_some());
    assert!(forecast["recovery_capacity_risk"].is_string());
    assert!(forecast["backtesting_fixture_status"].is_string());
    assert!(forecast["threshold_controls"].is_array());
    assert!(forecast["what_if_inputs"].is_array());
    assert!(forecast["blocked_by_stale_data"].is_boolean());
    assert!(forecast["blast_radius_summary"].is_string());
    assert!(forecast["recovery_note"]
        .as_str()
        .map(|note| note.contains("read-only"))
        .unwrap_or(false));
    assert!(forecast["missing_data_reason_codes"].is_array());
    assert!(forecast["risk_drivers"].is_array());
    assert!(forecast["evidence_reason_codes"].is_array());

    let reporting = &reports[0]["reporting"];
    assert_eq!(reporting["workflow_id"], "autoscaling_resilience_reporting");
    assert_eq!(reporting["read_only_mode"], true);
    assert!(reporting["scheduled_delivery_state"].as_str().is_some());
    assert!(reporting["portfolio_summary_ready"].is_boolean());
    assert!(reporting["workload_summary_ready"].is_boolean());
    assert!(reporting["stale_data_blocks_delivery"].is_boolean());
    assert!(reporting["export_formats"].is_array());
    assert_eq!(
        reporting["executive_summary"]["report_id"],
        "autoscaling-resilience-executive-summary"
    );
    assert!(reporting["executive_summary"]["top_reason_codes"].is_array());
    assert!(reporting["executive_summary"]["blast_radius_summary"].is_string());
    assert_eq!(
        reporting["engineering_backlog"]["report_id"],
        "autoscaling-resilience-engineering-backlog"
    );
    assert!(reporting["engineering_backlog"]["rows"].is_array());
    assert_eq!(
        reporting["incident_review"]["report_id"],
        "autoscaling-resilience-incident-review"
    );
    assert!(reporting["incident_review"]["rows"].is_array());
    assert!(reporting["missing_data_reason_codes"].is_array());
    assert!(reporting["evidence_reason_codes"].is_array());
}

#[tokio::test]
async fn ec2_pillar_reports_rejects_unknown_pillar() {
    if !aws_tests_enabled() {
        println!(
            "Skipping ec2_pillar_reports_rejects_unknown_pillar because ENABLE_AWS_TESTS is not true"
        );
        return;
    }

    let base = base_url().await;
    let client = Client::new();
    let resp = client
        .get(format!(
            "{}/api/aws/inventory/ec2/pillars?account_id=123456789012&pillar=bogus",
            base
        ))
        .send()
        .await
        .expect("bad pillar request failed");
    assert_eq!(resp.status(), 400);
}

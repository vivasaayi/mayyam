use super::*;
use crate::services::analytics::ai_llm_analytics::tool_call_trace_replay_workflow::{
    tool_call_trace_replay_workflow, ToolCallTraceReplayStatus,
};
use chrono::Duration;

fn item() -> ToolCallTraceInventoryItem {
    ToolCallTraceInventoryItem {
        trace_id: "trace-1".to_string(),
        provider_id: "provider-1".to_string(),
        model_name: "deepseek-agent".to_string(),
        enabled: true,
        owner: Some("sre-ai".to_string()),
        labels: vec!["cost-center=ai-platform".to_string()],
        trace_count: Some(1000),
        failed_tool_call_count: Some(3),
        estimated_monthly_trace_cost_usd: Some(50.0),
        monthly_trace_budget_usd: Some(100.0),
        sampling_policy: Some("sample all failed calls and 10 percent success".to_string()),
        replay_enabled: true,
        approved_tool_registry: Some("ops-readonly-tools".to_string()),
        audit_enabled: true,
        redaction_policy: Some("redact prompt, response, and tool args".to_string()),
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
fn healthy_tool_call_trace_inventory_passes_claimed_pillars() {
    let now = Utc::now();
    for pillar in [Pillar::Cost, Pillar::Resilience, Pillar::Security] {
        let report = evaluate_tool_call_trace_inventory(&[item()], pillar, now);
        assert_eq!(report.resources_evaluated, 1);
        assert!(report.findings.is_empty());
    }
}

#[test]
fn cost_flags_missing_owner_budget_and_overage() {
    let mut item = item();
    item.owner = None;
    item.labels.clear();
    item.estimated_monthly_trace_cost_usd = Some(150.0);

    let report = evaluate_tool_call_trace_inventory(&[item], Pillar::Cost, Utc::now());
    let codes = codes(&report);

    assert!(codes.contains(&REASON_COST_OWNER_NOT_RECORDED.to_string()));
    assert!(codes.contains(&REASON_COST_ESTIMATE_OVER_BUDGET.to_string()));
}

#[test]
fn resilience_flags_missing_sample_sampling_and_replay() {
    let mut item = item();
    item.trace_count = Some(0);
    item.sampling_policy = None;
    item.replay_enabled = false;

    let report = evaluate_tool_call_trace_inventory(&[item], Pillar::Resilience, Utc::now());
    let codes = codes(&report);

    assert!(codes.contains(&REASON_RES_TRACE_SAMPLE_MISSING.to_string()));
    assert!(codes.contains(&REASON_RES_SAMPLING_POLICY_MISSING.to_string()));
    assert!(codes.contains(&REASON_RES_REPLAY_MISSING.to_string()));
}

#[test]
fn security_flags_missing_registry_audit_and_redaction() {
    let mut item = item();
    item.approved_tool_registry = None;
    item.audit_enabled = false;
    item.redaction_policy = None;

    let report = evaluate_tool_call_trace_inventory(&[item], Pillar::Security, Utc::now());
    let codes = codes(&report);

    assert!(codes.contains(&REASON_SEC_REGISTRY_MISSING.to_string()));
    assert!(codes.contains(&REASON_SEC_AUDIT_MISSING.to_string()));
    assert!(codes.contains(&REASON_SEC_REDACTION_MISSING.to_string()));
}

#[test]
fn stale_tool_call_trace_inventory_is_counted_for_any_pillar() {
    let mut item = item();
    item.updated_at = Utc::now() - Duration::hours(DEFAULT_STALE_AFTER_HOURS + 1);

    let report = evaluate_tool_call_trace_inventory(&[item], Pillar::Cost, Utc::now());

    assert_eq!(report.stale_resources, 1);
    assert!(codes(&report).contains(&REASON_INV_STALE_DATA.to_string()));
}

#[test]
fn replay_workflow_blocks_missing_replay_evidence() {
    let mut ready = item();
    ready.trace_id = "trace-ready".to_string();
    ready.model_name = "ready-agent".to_string();

    let mut blocked = item();
    blocked.trace_id = "trace-blocked".to_string();
    blocked.model_name = "blocked-agent".to_string();
    blocked.trace_count = Some(0);
    blocked.sampling_policy = None;
    blocked.replay_enabled = false;
    blocked.approved_tool_registry = None;
    blocked.audit_enabled = false;
    blocked.redaction_policy = None;

    let items = vec![ready, blocked];
    let now = Utc::now();
    let reports = [Pillar::Cost, Pillar::Resilience, Pillar::Security]
        .into_iter()
        .map(|pillar| evaluate_tool_call_trace_inventory(&items, pillar, now))
        .collect::<Vec<_>>();

    let workflow = tool_call_trace_replay_workflow(&items, &reports);

    assert_eq!(
        workflow.workflow_id,
        "ai_llm_tool_call_trace_replay_workflow"
    );
    assert!(workflow.read_only_mode);
    assert!(workflow.replay_supported);
    assert_eq!(workflow.actions.len(), 2);
    assert_eq!(
        workflow.actions[0].status,
        ToolCallTraceReplayStatus::ReadyForReplay
    );
    assert_eq!(
        workflow.actions[1].status,
        ToolCallTraceReplayStatus::BlockedMissingEvidence
    );
    assert!(workflow.actions[1]
        .required_evidence
        .contains(&"trace_sample".to_string()));
    assert!(workflow.actions[1]
        .required_evidence
        .contains(&"replay_enabled".to_string()));
    assert!(workflow.actions[1]
        .required_evidence
        .contains(&"approved_tool_registry".to_string()));
    assert!(workflow.actions.iter().all(|action| action.read_only));
}

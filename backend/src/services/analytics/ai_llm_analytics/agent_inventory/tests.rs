use super::*;
use chrono::Duration;

fn item() -> AgentInventoryItem {
    AgentInventoryItem {
        agent_id: "agent-1".to_string(),
        provider_id: "provider-1".to_string(),
        model_name: "deepseek-agent".to_string(),
        enabled: true,
        owner: Some("sre-ai".to_string()),
        labels: vec!["cost-center=ai-platform".to_string()],
        prompt_count: 1,
        tool_count: 2,
        monthly_budget_usd: Some(1000.0),
        max_tokens_per_run: Some(8000),
        timeout_ms: Some(30000),
        max_iterations: Some(8),
        stop_condition: Some("stop after evidence-backed diagnosis".to_string()),
        provider_failover_policy: Some(
            "fail over to approved standby provider after dry-run health check".to_string(),
        ),
        tool_policy: Some("read-only diagnostics by default".to_string()),
        approval_policy: Some("approval required for mutations".to_string()),
        model_routing_policy: Some(
            "route regulated workloads to approved provider set".to_string(),
        ),
        audit_enabled: true,
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
fn healthy_agent_inventory_passes_claimed_pillars() {
    let now = Utc::now();
    for pillar in [Pillar::Cost, Pillar::Resilience, Pillar::Security] {
        let report = evaluate_agent_inventory(&[item()], pillar, now);
        assert_eq!(report.resources_evaluated, 1);
        assert!(report.findings.is_empty());
    }
}

#[test]
fn cost_flags_missing_owner_budget_and_token_limit() {
    let mut item = item();
    item.owner = None;
    item.labels.clear();
    item.monthly_budget_usd = None;
    item.max_tokens_per_run = None;

    let report = evaluate_agent_inventory(&[item], Pillar::Cost, Utc::now());
    let codes = codes(&report);

    assert!(codes.contains(&REASON_COST_OWNER_NOT_RECORDED.to_string()));
    assert!(codes.contains(&REASON_COST_BUDGET_MISSING.to_string()));
    assert!(codes.contains(&REASON_COST_TOKEN_LIMIT_MISSING.to_string()));
}

#[test]
fn resilience_flags_disabled_route_without_prompt_or_stop_condition() {
    let mut item = item();
    item.enabled = false;
    item.prompt_count = 0;
    item.timeout_ms = None;
    item.max_iterations = None;
    item.stop_condition = None;
    item.provider_failover_policy = None;

    let report = evaluate_agent_inventory(&[item], Pillar::Resilience, Utc::now());
    let codes = codes(&report);

    assert!(codes.contains(&REASON_RES_DISABLED.to_string()));
    assert!(codes.contains(&REASON_RES_PROMPT_MISSING.to_string()));
    assert!(codes.contains(&REASON_RES_TIMEOUT_MISSING.to_string()));
    assert!(codes.contains(&REASON_RES_PROVIDER_FAILOVER_POLICY_MISSING.to_string()));
    assert!(codes.contains(&REASON_RES_STOP_CONDITION_MISSING.to_string()));
}

#[test]
fn security_flags_missing_tool_policy_approval_and_audit() {
    let mut item = item();
    item.tool_count = 0;
    item.tool_policy = None;
    item.approval_policy = None;
    item.model_routing_policy = None;
    item.audit_enabled = false;

    let report = evaluate_agent_inventory(&[item], Pillar::Security, Utc::now());
    let codes = codes(&report);

    assert!(codes.contains(&REASON_SEC_TOOL_POLICY_MISSING.to_string()));
    assert!(codes.contains(&REASON_SEC_APPROVAL_MISSING.to_string()));
    assert!(codes.contains(&REASON_SEC_MODEL_ROUTING_POLICY_MISSING.to_string()));
    assert!(codes.contains(&REASON_SEC_AUDIT_MISSING.to_string()));
}

#[test]
fn stale_agent_inventory_is_counted_for_any_pillar() {
    let mut item = item();
    item.updated_at = Utc::now() - Duration::hours(DEFAULT_STALE_AFTER_HOURS + 1);

    let report = evaluate_agent_inventory(&[item], Pillar::Cost, Utc::now());

    assert_eq!(report.stale_resources, 1);
    assert!(codes(&report).contains(&REASON_INV_STALE_DATA.to_string()));
}

#[test]
fn agent_control_workflow_blocks_missing_budget_and_stop_evidence() {
    let mut complete = item();
    complete.agent_id = "agent-ready".to_string();
    complete.model_name = "ready-agent".to_string();

    let mut incomplete = item();
    incomplete.agent_id = "agent-blocked".to_string();
    incomplete.model_name = "blocked-agent".to_string();
    incomplete.monthly_budget_usd = None;
    incomplete.max_tokens_per_run = None;
    incomplete.max_iterations = None;
    incomplete.stop_condition = None;
    incomplete.provider_failover_policy = None;
    incomplete.tool_policy = None;
    incomplete.approval_policy = None;
    incomplete.model_routing_policy = None;
    incomplete.audit_enabled = false;

    let items = vec![complete, incomplete];
    let now = Utc::now();
    let reports = [Pillar::Cost, Pillar::Resilience, Pillar::Security]
        .into_iter()
        .map(|pillar| evaluate_agent_inventory(&items, pillar, now))
        .collect::<Vec<_>>();

    let workflow = agent_control_workflow(&items, &reports);

    assert_eq!(
        workflow.workflow_id,
        "ai_llm_agent_budget_stop_control_workflow"
    );
    assert!(workflow.read_only_mode);
    assert!(workflow.dry_run_only);
    assert_eq!(workflow.actions.len(), 2);
    assert_eq!(
        workflow.actions[0].status,
        AgentControlStatus::ReadyForApproval
    );
    assert_eq!(
        workflow.actions[1].status,
        AgentControlStatus::BlockedMissingEvidence
    );
    assert!(workflow.actions[1]
        .required_evidence
        .contains(&"monthly_budget_usd".to_string()));
    assert!(workflow.actions[1]
        .required_evidence
        .contains(&"max_iterations_or_stop_condition".to_string()));
    assert!(workflow.actions.iter().all(|action| action.dry_run));
}

#[test]
fn agent_governance_workflow_blocks_missing_approval_and_routing_evidence() {
    let mut ready = item();
    ready.agent_id = "agent-ready".to_string();
    ready.model_name = "ready-agent".to_string();

    let mut blocked = item();
    blocked.agent_id = "agent-blocked".to_string();
    blocked.model_name = "blocked-agent".to_string();
    blocked.approval_policy = None;
    blocked.model_routing_policy = None;
    blocked.provider_failover_policy = None;
    blocked.audit_enabled = false;

    let items = vec![ready, blocked];
    let now = Utc::now();
    let reports = [Pillar::Cost, Pillar::Resilience, Pillar::Security]
        .into_iter()
        .map(|pillar| evaluate_agent_inventory(&items, pillar, now))
        .collect::<Vec<_>>();

    let workflow = agent_governance_workflow(&items, &reports);

    assert_eq!(workflow.workflow_id, "ai_llm_agent_governance_workflow");
    assert!(workflow.read_only_mode);
    assert_eq!(
        workflow.approval_gate_permission,
        "ai.llm.agent.governance.approve"
    );
    assert_eq!(workflow.actions.len(), 2);
    assert_eq!(
        workflow.actions[0].status,
        AgentGovernanceStatus::ReadyForGovernanceReview
    );
    assert_eq!(
        workflow.actions[1].status,
        AgentGovernanceStatus::BlockedMissingEvidence
    );
    assert!(workflow.actions[1]
        .required_evidence
        .contains(&"approval_policy".to_string()));
    assert!(workflow.actions[1]
        .required_evidence
        .contains(&"model_routing_policy".to_string()));
    assert!(workflow.actions[1]
        .required_evidence
        .contains(&"provider_failover_policy".to_string()));
    assert!(workflow.actions.iter().all(|action| action.read_only));
}

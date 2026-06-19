use serde::{Deserialize, Serialize};

use crate::services::analytics::ai_llm_analytics::agent_inventory::{
    AgentInventoryItem, PillarReport, REASON_COST_BUDGET_MISSING, REASON_COST_TOKEN_LIMIT_MISSING,
    REASON_INV_STALE_DATA, REASON_RES_STOP_CONDITION_MISSING, REASON_SEC_APPROVAL_MISSING,
    REASON_SEC_AUDIT_MISSING, REASON_SEC_MODEL_ROUTING_POLICY_MISSING,
    REASON_SEC_TOOL_POLICY_MISSING,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentControlStatus {
    ReadyForApproval,
    BlockedMissingEvidence,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentControlAction {
    pub agent_id: String,
    pub model_name: String,
    pub title: String,
    pub status: AgentControlStatus,
    pub dry_run: bool,
    pub event_name: String,
    pub required_evidence: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentControlWorkflow {
    pub workflow_id: String,
    pub read_only_mode: bool,
    pub dry_run_only: bool,
    pub approval_gate_permission: String,
    pub audit_stream: String,
    pub actions: Vec<AgentControlAction>,
}

pub fn agent_control_workflow(
    items: &[AgentInventoryItem],
    reports: &[PillarReport],
) -> AgentControlWorkflow {
    AgentControlWorkflow {
        workflow_id: "ai_llm_agent_budget_stop_control_workflow".to_string(),
        read_only_mode: true,
        dry_run_only: true,
        approval_gate_permission: "ai.llm.agent.controls.approve".to_string(),
        audit_stream: "ai.llm.agent.controls".to_string(),
        actions: items
            .iter()
            .map(|item| {
                let required_evidence = agent_control_required_evidence(item, reports);
                let status = if required_evidence.is_empty() {
                    AgentControlStatus::ReadyForApproval
                } else {
                    AgentControlStatus::BlockedMissingEvidence
                };

                AgentControlAction {
                    agent_id: item.agent_id.clone(),
                    model_name: item.model_name.clone(),
                    title: format!(
                        "Dry-run agent budget and stop controls for {}",
                        item.model_name
                    ),
                    status,
                    dry_run: true,
                    event_name: "ai_llm.agent.controls.dry_run_planned".to_string(),
                    required_evidence,
                }
            })
            .collect(),
    }
}

fn agent_control_required_evidence(
    item: &AgentInventoryItem,
    reports: &[PillarReport],
) -> Vec<String> {
    let mut evidence = Vec::new();
    for report in reports {
        for finding in report
            .findings
            .iter()
            .filter(|finding| finding.resource_id == item.agent_id)
        {
            let label = match finding.reason_code.as_str() {
                REASON_COST_BUDGET_MISSING => Some("monthly_budget_usd"),
                REASON_COST_TOKEN_LIMIT_MISSING => Some("max_tokens_per_run"),
                REASON_RES_STOP_CONDITION_MISSING => Some("max_iterations_or_stop_condition"),
                REASON_SEC_TOOL_POLICY_MISSING => Some("registered_read_only_tool_policy"),
                REASON_SEC_APPROVAL_MISSING => Some("approval_policy"),
                REASON_SEC_MODEL_ROUTING_POLICY_MISSING => Some("model_routing_policy"),
                REASON_SEC_AUDIT_MISSING => Some("replayable_audit_trace"),
                REASON_INV_STALE_DATA => Some("fresh_inventory_snapshot"),
                _ => None,
            };

            if let Some(label) = label {
                if !evidence.iter().any(|existing| existing == label) {
                    evidence.push(label.to_string());
                }
            }
        }
    }
    evidence
}

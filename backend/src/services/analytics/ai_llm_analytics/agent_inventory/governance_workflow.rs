use serde::{Deserialize, Serialize};

use crate::services::analytics::ai_llm_analytics::agent_inventory::{
    AgentInventoryItem, PillarReport, REASON_INV_STALE_DATA,
    REASON_RES_PROVIDER_FAILOVER_POLICY_MISSING, REASON_SEC_APPROVAL_MISSING,
    REASON_SEC_AUDIT_MISSING, REASON_SEC_MODEL_ROUTING_POLICY_MISSING,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentGovernanceStatus {
    ReadyForGovernanceReview,
    BlockedMissingEvidence,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentGovernanceAction {
    pub agent_id: String,
    pub model_name: String,
    pub title: String,
    pub status: AgentGovernanceStatus,
    pub read_only: bool,
    pub event_name: String,
    pub required_evidence: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentGovernanceWorkflow {
    pub workflow_id: String,
    pub read_only_mode: bool,
    pub approval_gate_permission: String,
    pub audit_stream: String,
    pub actions: Vec<AgentGovernanceAction>,
}

pub fn agent_governance_workflow(
    items: &[AgentInventoryItem],
    reports: &[PillarReport],
) -> AgentGovernanceWorkflow {
    AgentGovernanceWorkflow {
        workflow_id: "ai_llm_agent_governance_workflow".to_string(),
        read_only_mode: true,
        approval_gate_permission: "ai.llm.agent.governance.approve".to_string(),
        audit_stream: "ai.llm.agent.governance".to_string(),
        actions: items
            .iter()
            .map(|item| {
                let required_evidence = agent_governance_required_evidence(item, reports);
                let status = if required_evidence.is_empty() {
                    AgentGovernanceStatus::ReadyForGovernanceReview
                } else {
                    AgentGovernanceStatus::BlockedMissingEvidence
                };

                AgentGovernanceAction {
                    agent_id: item.agent_id.clone(),
                    model_name: item.model_name.clone(),
                    title: format!("Review governance controls for {}", item.model_name),
                    status,
                    read_only: true,
                    event_name: "ai_llm.agent.governance.review_planned".to_string(),
                    required_evidence,
                }
            })
            .collect(),
    }
}

fn agent_governance_required_evidence(
    item: &AgentInventoryItem,
    reports: &[PillarReport],
) -> Vec<String> {
    let mut evidence = Vec::new();

    if item
        .model_routing_policy
        .as_deref()
        .unwrap_or_default()
        .trim()
        .is_empty()
    {
        evidence.push("model_routing_policy".to_string());
    }

    if item
        .provider_failover_policy
        .as_deref()
        .unwrap_or_default()
        .trim()
        .is_empty()
    {
        evidence.push("provider_failover_policy".to_string());
    }

    for report in reports {
        for finding in report
            .findings
            .iter()
            .filter(|finding| finding.resource_id == item.agent_id)
        {
            let label = match finding.reason_code.as_str() {
                REASON_SEC_APPROVAL_MISSING => Some("approval_policy"),
                REASON_SEC_MODEL_ROUTING_POLICY_MISSING => Some("model_routing_policy"),
                REASON_RES_PROVIDER_FAILOVER_POLICY_MISSING => Some("provider_failover_policy"),
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

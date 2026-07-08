use serde::{Deserialize, Serialize};

use crate::services::analytics::ai_llm_analytics::tool_call_trace_inventory::{
    ToolCallTraceInventoryItem, REASON_INV_STALE_DATA, REASON_RES_REPLAY_MISSING,
    REASON_RES_SAMPLING_POLICY_MISSING, REASON_RES_TRACE_SAMPLE_MISSING, REASON_SEC_AUDIT_MISSING,
    REASON_SEC_REDACTION_MISSING, REASON_SEC_REGISTRY_MISSING,
};
use crate::services::aws::inventory::types::PillarReport;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolCallTraceReplayStatus {
    ReadyForReplay,
    BlockedMissingEvidence,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallTraceReplayAction {
    pub trace_id: String,
    pub model_name: String,
    pub title: String,
    pub status: ToolCallTraceReplayStatus,
    pub read_only: bool,
    pub event_name: String,
    pub required_evidence: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallTraceReplayWorkflow {
    pub workflow_id: String,
    pub read_only_mode: bool,
    pub replay_supported: bool,
    pub audit_stream: String,
    pub actions: Vec<ToolCallTraceReplayAction>,
}

pub fn tool_call_trace_replay_workflow(
    items: &[ToolCallTraceInventoryItem],
    reports: &[PillarReport],
) -> ToolCallTraceReplayWorkflow {
    ToolCallTraceReplayWorkflow {
        workflow_id: "ai_llm_tool_call_trace_replay_workflow".to_string(),
        read_only_mode: true,
        replay_supported: true,
        audit_stream: "ai.llm.tool_call_trace.replay".to_string(),
        actions: items
            .iter()
            .map(|item| {
                let required_evidence = tool_call_trace_required_evidence(item, reports);
                let status = if required_evidence.is_empty() {
                    ToolCallTraceReplayStatus::ReadyForReplay
                } else {
                    ToolCallTraceReplayStatus::BlockedMissingEvidence
                };

                ToolCallTraceReplayAction {
                    trace_id: item.trace_id.clone(),
                    model_name: item.model_name.clone(),
                    title: format!("Replay tool-call trace evidence for {}", item.model_name),
                    status,
                    read_only: true,
                    event_name: "ai_llm.tool_call_trace.replay_planned".to_string(),
                    required_evidence,
                }
            })
            .collect(),
    }
}

fn tool_call_trace_required_evidence(
    item: &ToolCallTraceInventoryItem,
    reports: &[PillarReport],
) -> Vec<String> {
    let mut evidence = Vec::new();
    for report in reports {
        for finding in report
            .findings
            .iter()
            .filter(|finding| finding.resource_id == item.trace_id)
        {
            let label = match finding.reason_code.as_str() {
                REASON_RES_TRACE_SAMPLE_MISSING => Some("trace_sample"),
                REASON_RES_SAMPLING_POLICY_MISSING => Some("sampling_policy"),
                REASON_RES_REPLAY_MISSING => Some("replay_enabled"),
                REASON_SEC_REGISTRY_MISSING => Some("approved_tool_registry"),
                REASON_SEC_AUDIT_MISSING => Some("replayable_audit_trace"),
                REASON_SEC_REDACTION_MISSING => Some("redaction_policy"),
                REASON_INV_STALE_DATA => Some("fresh_trace_snapshot"),
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

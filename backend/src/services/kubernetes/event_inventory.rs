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

// Deterministic Kubernetes Event inventory evaluator for roadmap rows
// 02-KUBERNETES-DASHBOARD-01667/01674/01695.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::services::aws::inventory::types::{
    score_pillar, InventoryFinding, Pillar, PillarReport, Severity, DEFAULT_STALE_AFTER_HOURS,
};

pub const RESOURCE_TYPE: &str = "KubernetesEvent";
pub const REASON_COST_HIGH_REPEAT_COUNT: &str = "K8S_EVENT_HIGH_REPEAT_COUNT";
pub const REASON_RES_WARNING_EVENT: &str = "K8S_EVENT_WARNING";
pub const REASON_SEC_SENSITIVE_MESSAGE: &str = "K8S_EVENT_SENSITIVE_MESSAGE";
pub const REASON_INV_STALE_DATA: &str = "K8S_EVENT_INV_STALE_DATA";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventInventoryItem {
    pub cluster_id: String,
    pub namespace: String,
    pub name: String,
    pub event_type: Option<String>,
    pub reason: Option<String>,
    pub message: Option<String>,
    pub count: i32,
    pub action: Option<String>,
    pub reporting_component: Option<String>,
    pub reporting_instance: Option<String>,
    pub involved_object_api_version: Option<String>,
    pub involved_object_kind: Option<String>,
    pub involved_object_namespace: Option<String>,
    pub involved_object_name: Option<String>,
    pub related_object_kind: Option<String>,
    pub related_object_name: Option<String>,
    pub first_timestamp: Option<DateTime<Utc>>,
    pub last_timestamp: Option<DateTime<Utc>>,
    pub event_time: Option<DateTime<Utc>>,
    pub created_at: Option<DateTime<Utc>>,
    pub collected_at: DateTime<Utc>,
}

pub fn evaluate_kubernetes_event_inventory(
    events: &[EventInventoryItem],
    pillar: Pillar,
    now: DateTime<Utc>,
) -> PillarReport {
    let mut stale_resources = 0;
    let mut findings = Vec::new();

    for event in events {
        if let Some(finding) = stale_finding(event, pillar, now) {
            stale_resources += 1;
            findings.push(finding);
        }

        match pillar {
            Pillar::Cost => evaluate_cost(event, pillar, &mut findings),
            Pillar::Resilience => evaluate_resilience(event, pillar, &mut findings),
            Pillar::Security => evaluate_security(event, pillar, &mut findings),
            _ => {}
        }
    }

    PillarReport {
        pillar,
        resources_evaluated: events.len(),
        stale_resources,
        score: score_pillar(&findings),
        findings,
    }
}

fn evaluate_cost(event: &EventInventoryItem, pillar: Pillar, findings: &mut Vec<InventoryFinding>) {
    if event.count < 25 {
        return;
    }

    findings.push(finding(
        event,
        pillar,
        REASON_COST_HIGH_REPEAT_COUNT,
        Severity::Medium,
        format!(
            "Kubernetes Event {}/{} repeated {} times; investigate noisy workload or controller churn",
            event.namespace, event.name, event.count
        ),
        json!({
            "cluster_id": event.cluster_id,
            "namespace": event.namespace,
            "event": event.name,
            "count": event.count,
            "event_type": event.event_type,
            "reason": event.reason,
            "involved_object": involved_object_evidence(event),
            "last_timestamp": event.last_timestamp,
            "recommendation": "Reduce repeated failure loops or controller event spam before relying on event-driven triage and cost posture signals",
        }),
    ));
}

fn evaluate_resilience(
    event: &EventInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    let is_warning = event
        .event_type
        .as_deref()
        .map(str::trim)
        .is_some_and(|event_type| event_type.eq_ignore_ascii_case("Warning"));
    if !is_warning {
        return;
    }

    findings.push(finding(
        event,
        pillar,
        REASON_RES_WARNING_EVENT,
        Severity::High,
        format!(
            "Kubernetes Event {}/{} is Warning{}",
            event.namespace,
            event.name,
            event
                .reason
                .as_deref()
                .map(|reason| format!(" ({})", reason))
                .unwrap_or_default()
        ),
        json!({
            "cluster_id": event.cluster_id,
            "namespace": event.namespace,
            "event": event.name,
            "event_type": event.event_type,
            "reason": event.reason,
            "action": event.action,
            "reporting_component": event.reporting_component,
            "reporting_instance": event.reporting_instance,
            "involved_object": involved_object_evidence(event),
            "related_object": related_object_evidence(event),
            "count": event.count,
            "first_timestamp": event.first_timestamp,
            "last_timestamp": event.last_timestamp,
            "recommendation": "Use the warning Event as evidence for workload or control-plane remediation before marking the resource healthy",
        }),
    ));
}

fn evaluate_security(
    event: &EventInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    let Some(message) = event.message.as_deref() else {
        return;
    };
    let Some(matched_keyword) = sensitive_keyword(message) else {
        return;
    };

    findings.push(finding(
        event,
        pillar,
        REASON_SEC_SENSITIVE_MESSAGE,
        Severity::High,
        format!(
            "Kubernetes Event {}/{} message contains sensitive-looking text",
            event.namespace, event.name
        ),
        json!({
            "cluster_id": event.cluster_id,
            "namespace": event.namespace,
            "event": event.name,
            "event_type": event.event_type,
            "reason": event.reason,
            "matched_keyword": matched_keyword,
            "message_preview": redacted_message_preview(message),
            "involved_object": involved_object_evidence(event),
            "recommendation": "Remove credentials from workload probes, controller errors, and event-producing messages; reference Kubernetes Secrets or an external secret manager instead",
        }),
    ));
}

fn stale_finding(
    event: &EventInventoryItem,
    pillar: Pillar,
    now: DateTime<Utc>,
) -> Option<InventoryFinding> {
    let age_hours = (now - event.collected_at).num_hours();
    if age_hours <= DEFAULT_STALE_AFTER_HOURS {
        return None;
    }

    Some(finding(
        event,
        pillar,
        REASON_INV_STALE_DATA,
        Severity::Medium,
        format!(
            "Inventory data for Kubernetes Event {}/{} is {} hours old (threshold {} hours)",
            event.namespace, event.name, age_hours, DEFAULT_STALE_AFTER_HOURS
        ),
        json!({
            "cluster_id": event.cluster_id,
            "namespace": event.namespace,
            "event": event.name,
            "collected_at": event.collected_at,
            "age_hours": age_hours,
            "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
        }),
    ))
}

fn finding(
    event: &EventInventoryItem,
    pillar: Pillar,
    reason_code: &str,
    severity: Severity,
    message: String,
    evidence: Value,
) -> InventoryFinding {
    InventoryFinding {
        resource_id: format!(
            "{}/{}/Event/{}",
            event.cluster_id, event.namespace, event.name
        ),
        arn: format!(
            "kubernetes://events/{}/{}/{}",
            event.cluster_id, event.namespace, event.name
        ),
        pillar,
        reason_code: reason_code.to_string(),
        severity,
        message,
        evidence,
    }
}

fn involved_object_evidence(event: &EventInventoryItem) -> Value {
    json!({
        "api_version": event.involved_object_api_version,
        "kind": event.involved_object_kind,
        "namespace": event.involved_object_namespace,
        "name": event.involved_object_name,
    })
}

fn related_object_evidence(event: &EventInventoryItem) -> Value {
    json!({
        "kind": event.related_object_kind,
        "name": event.related_object_name,
    })
}

fn sensitive_keyword(message: &str) -> Option<&'static str> {
    let lower = message.to_ascii_lowercase();
    [
        "private key",
        "secret key",
        "access key",
        "api key",
        "apikey",
        "credential",
        "password",
        "secret",
        "token",
    ]
    .into_iter()
    .find(|keyword| lower.contains(keyword))
}

fn redacted_message_preview(message: &str) -> String {
    let mut preview = message.chars().take(160).collect::<String>();
    for token in ["=", ":", " "] {
        for keyword in [
            "password",
            "token",
            "secret",
            "credential",
            "apikey",
            "api key",
            "access key",
            "secret key",
            "private key",
        ] {
            let needle = format!("{}{}", keyword, token);
            let lower = preview.to_ascii_lowercase();
            if let Some(start) = lower.find(&needle) {
                let value_start = start + needle.len();
                let value_end = preview[value_start..]
                    .find(char::is_whitespace)
                    .map(|offset| value_start + offset)
                    .unwrap_or(preview.len());
                preview.replace_range(value_start..value_end, "[REDACTED]");
            }
        }
    }
    preview
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::aws::inventory::types::{Pillar, DEFAULT_STALE_AFTER_HOURS};
    use chrono::Duration;

    fn now() -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2026-06-10T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc)
    }

    fn healthy_event() -> EventInventoryItem {
        EventInventoryItem {
            cluster_id: "cluster-a".to_string(),
            namespace: "apps".to_string(),
            name: "pod-a.started".to_string(),
            event_type: Some("Normal".to_string()),
            reason: Some("Started".to_string()),
            message: Some("Started container app".to_string()),
            count: 1,
            action: Some("Started".to_string()),
            reporting_component: Some("kubelet".to_string()),
            reporting_instance: Some("node-a".to_string()),
            involved_object_api_version: Some("v1".to_string()),
            involved_object_kind: Some("Pod".to_string()),
            involved_object_namespace: Some("apps".to_string()),
            involved_object_name: Some("pod-a".to_string()),
            related_object_kind: None,
            related_object_name: None,
            first_timestamp: Some(now() - Duration::minutes(5)),
            last_timestamp: Some(now() - Duration::minutes(4)),
            event_time: Some(now() - Duration::minutes(5)),
            created_at: Some(now() - Duration::minutes(5)),
            collected_at: now(),
        }
    }

    #[test]
    fn cost_flags_high_repeat_event_count() {
        let mut repeated = healthy_event();
        repeated.count = 50;

        let report = evaluate_kubernetes_event_inventory(&[repeated], Pillar::Cost, now());

        assert_eq!(report.resources_evaluated, 1);
        assert_eq!(report.findings.len(), 1);
        assert_eq!(
            report.findings[0].reason_code,
            REASON_COST_HIGH_REPEAT_COUNT
        );
        assert_eq!(report.findings[0].pillar, Pillar::Cost);
    }

    #[test]
    fn resilience_flags_warning_events() {
        let mut warning = healthy_event();
        warning.event_type = Some("Warning".to_string());
        warning.reason = Some("FailedScheduling".to_string());

        let report = evaluate_kubernetes_event_inventory(&[warning], Pillar::Resilience, now());

        assert_eq!(report.findings.len(), 1);
        assert_eq!(report.findings[0].reason_code, REASON_RES_WARNING_EVENT);
    }

    #[test]
    fn security_flags_sensitive_event_messages() {
        let mut sensitive = healthy_event();
        sensitive.message = Some("probe failed with token=plain-text".to_string());

        let report = evaluate_kubernetes_event_inventory(&[sensitive], Pillar::Security, now());

        assert_eq!(report.findings.len(), 1);
        assert_eq!(report.findings[0].reason_code, REASON_SEC_SENSITIVE_MESSAGE);
    }

    #[test]
    fn stale_inventory_is_counted_for_any_pillar() {
        let mut stale = healthy_event();
        stale.collected_at = now() - Duration::hours(DEFAULT_STALE_AFTER_HOURS + 2);

        let report = evaluate_kubernetes_event_inventory(&[stale], Pillar::Cost, now());

        assert_eq!(report.stale_resources, 1);
        assert_eq!(report.findings[0].reason_code, REASON_INV_STALE_DATA);
    }

    #[test]
    fn healthy_events_pass_claimed_pillars() {
        for pillar in [Pillar::Cost, Pillar::Resilience, Pillar::Security] {
            let report = evaluate_kubernetes_event_inventory(&[healthy_event()], pillar, now());

            assert_eq!(report.resources_evaluated, 1);
            assert_eq!(report.stale_resources, 0);
            assert!(report.findings.is_empty());
        }
    }
}

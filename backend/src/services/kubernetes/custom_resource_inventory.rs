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

// Deterministic Kubernetes CustomResource inventory evaluator for roadmap rows
// 02-KUBERNETES-DASHBOARD-01618/01625/01646.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::services::aws::inventory::types::{
    score_pillar, InventoryFinding, Pillar, PillarReport, Severity, COST_ALLOCATION_TAG_KEYS,
    DEFAULT_STALE_AFTER_HOURS,
};

pub const RESOURCE_TYPE: &str = "KubernetesCustomResource";
pub const REASON_COST_OWNER_NOT_RECORDED: &str = "K8S_CUSTOM_RESOURCE_COST_OWNER_NOT_RECORDED";
pub const REASON_RES_NOT_READY: &str = "K8S_CUSTOM_RESOURCE_NOT_READY";
pub const REASON_RES_STUCK_DELETION: &str = "K8S_CUSTOM_RESOURCE_STUCK_DELETION";
pub const REASON_SEC_SENSITIVE_FIELD_EXPOSED: &str = "K8S_CUSTOM_RESOURCE_SENSITIVE_FIELD_EXPOSED";
pub const REASON_INV_STALE_DATA: &str = "K8S_CUSTOM_RESOURCE_INV_STALE_DATA";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomResourceInventoryItem {
    pub cluster_id: String,
    pub namespace: Option<String>,
    pub name: String,
    pub api_version: String,
    pub kind: String,
    pub group: String,
    pub version: String,
    pub plural: String,
    pub scope: String,
    pub labels: BTreeMap<String, String>,
    pub annotations: BTreeMap<String, String>,
    pub owner_references_count: usize,
    pub finalizers: Vec<String>,
    pub has_status: bool,
    pub ready_condition_status: Option<String>,
    pub deletion_timestamp: Option<DateTime<Utc>>,
    pub spec_keys: Vec<String>,
    pub status_keys: Vec<String>,
    pub sensitive_field_paths: Vec<String>,
    pub created_at: Option<DateTime<Utc>>,
    pub collected_at: DateTime<Utc>,
}

pub fn evaluate_kubernetes_custom_resource_inventory(
    resources: &[CustomResourceInventoryItem],
    pillar: Pillar,
    now: DateTime<Utc>,
) -> PillarReport {
    let mut stale_resources = 0;
    let mut findings = Vec::new();

    for resource in resources {
        if let Some(finding) = stale_finding(resource, pillar, now) {
            stale_resources += 1;
            findings.push(finding);
        }

        match pillar {
            Pillar::Cost => evaluate_cost(resource, pillar, &mut findings),
            Pillar::Resilience => evaluate_resilience(resource, pillar, &mut findings),
            Pillar::Security => evaluate_security(resource, pillar, &mut findings),
            _ => {}
        }
    }

    PillarReport {
        pillar,
        resources_evaluated: resources.len(),
        stale_resources,
        score: score_pillar(&findings),
        findings,
    }
}

fn evaluate_cost(
    resource: &CustomResourceInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if has_any_metadata_key(&resource.labels, COST_ALLOCATION_TAG_KEYS)
        || has_any_metadata_key(&resource.annotations, COST_ALLOCATION_TAG_KEYS)
    {
        return;
    }

    findings.push(finding(
        resource,
        pillar,
        REASON_COST_OWNER_NOT_RECORDED,
        Severity::Medium,
        format!(
            "Kubernetes CustomResource {} has no owner, team, project, or cost-center label or annotation",
            resource_identity(resource)
        ),
        json!({
            "cluster_id": resource.cluster_id,
            "namespace": resource.namespace,
            "api_version": resource.api_version,
            "kind": resource.kind,
            "name": resource.name,
            "checked_keys": COST_ALLOCATION_TAG_KEYS,
            "checked_locations": ["labels", "annotations"],
        }),
    ));
}

fn evaluate_resilience(
    resource: &CustomResourceInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if resource.deletion_timestamp.is_some() && !resource.finalizers.is_empty() {
        findings.push(finding(
            resource,
            pillar,
            REASON_RES_STUCK_DELETION,
            Severity::High,
            format!(
                "Kubernetes CustomResource {} is deleting but still has finalizers",
                resource_identity(resource)
            ),
            json!({
                "cluster_id": resource.cluster_id,
                "namespace": resource.namespace,
                "api_version": resource.api_version,
                "kind": resource.kind,
                "name": resource.name,
                "deletion_timestamp": resource.deletion_timestamp,
                "finalizers": resource.finalizers,
                "recommendation": "Inspect the responsible controller and remove or complete the finalizer only after the cleanup contract is satisfied",
            }),
        ));
    }

    let Some(status) = resource.ready_condition_status.as_deref() else {
        return;
    };
    if status.eq_ignore_ascii_case("true") {
        return;
    }

    findings.push(finding(
        resource,
        pillar,
        REASON_RES_NOT_READY,
        Severity::High,
        format!(
            "Kubernetes CustomResource {} reports readiness condition {}",
            resource_identity(resource),
            status
        ),
        json!({
            "cluster_id": resource.cluster_id,
            "namespace": resource.namespace,
            "api_version": resource.api_version,
            "kind": resource.kind,
            "name": resource.name,
            "ready_condition_status": resource.ready_condition_status,
            "status_keys": resource.status_keys,
            "recommendation": "Inspect the custom controller status conditions and reconcile errors before depending on this resource",
        }),
    ));
}

fn evaluate_security(
    resource: &CustomResourceInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if resource.sensitive_field_paths.is_empty() {
        return;
    }

    findings.push(finding(
        resource,
        pillar,
        REASON_SEC_SENSITIVE_FIELD_EXPOSED,
        Severity::High,
        format!(
            "Kubernetes CustomResource {} exposes sensitive-looking fields in spec or status",
            resource_identity(resource)
        ),
        json!({
            "cluster_id": resource.cluster_id,
            "namespace": resource.namespace,
            "api_version": resource.api_version,
            "kind": resource.kind,
            "name": resource.name,
            "sensitive_field_paths": resource.sensitive_field_paths,
            "recommendation": "Move secret material to Kubernetes Secrets or an external secret manager and reference it from the custom resource",
        }),
    ));
}

fn stale_finding(
    resource: &CustomResourceInventoryItem,
    pillar: Pillar,
    now: DateTime<Utc>,
) -> Option<InventoryFinding> {
    let age_hours = (now - resource.collected_at).num_hours();
    if age_hours <= DEFAULT_STALE_AFTER_HOURS {
        return None;
    }

    Some(finding(
        resource,
        pillar,
        REASON_INV_STALE_DATA,
        Severity::Medium,
        format!(
            "Inventory data for Kubernetes CustomResource {} is {} hours old (threshold {} hours)",
            resource_identity(resource),
            age_hours,
            DEFAULT_STALE_AFTER_HOURS
        ),
        json!({
            "cluster_id": resource.cluster_id,
            "namespace": resource.namespace,
            "api_version": resource.api_version,
            "kind": resource.kind,
            "name": resource.name,
            "collected_at": resource.collected_at,
            "age_hours": age_hours,
            "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
        }),
    ))
}

fn finding(
    resource: &CustomResourceInventoryItem,
    pillar: Pillar,
    reason_code: &str,
    severity: Severity,
    message: String,
    evidence: Value,
) -> InventoryFinding {
    InventoryFinding {
        resource_id: resource_id(resource),
        arn: resource_arn(resource),
        pillar,
        reason_code: reason_code.to_string(),
        severity,
        message,
        evidence,
    }
}

fn resource_id(resource: &CustomResourceInventoryItem) -> String {
    match resource.namespace.as_deref() {
        Some(namespace) if !namespace.is_empty() => format!(
            "{}/{}/CustomResource/{}/{}/{}",
            resource.cluster_id, namespace, resource.api_version, resource.plural, resource.name
        ),
        _ => format!(
            "{}/CustomResource/{}/{}/{}",
            resource.cluster_id, resource.api_version, resource.plural, resource.name
        ),
    }
}

fn resource_arn(resource: &CustomResourceInventoryItem) -> String {
    let namespace = resource.namespace.as_deref().unwrap_or("_cluster");
    format!(
        "kubernetes://customresources/{}/{}/{}/{}/{}",
        resource.cluster_id, namespace, resource.api_version, resource.plural, resource.name
    )
}

fn resource_identity(resource: &CustomResourceInventoryItem) -> String {
    match resource.namespace.as_deref() {
        Some(namespace) if !namespace.is_empty() => format!(
            "{}/{}/{}/{}",
            namespace, resource.api_version, resource.plural, resource.name
        ),
        _ => format!(
            "{}/{}/{}",
            resource.api_version, resource.plural, resource.name
        ),
    }
}

fn has_any_metadata_key(metadata: &BTreeMap<String, String>, wanted_keys: &[&str]) -> bool {
    wanted_keys
        .iter()
        .any(|wanted| metadata_value(metadata, wanted).is_some())
}

fn metadata_value(metadata: &BTreeMap<String, String>, wanted_key: &str) -> Option<String> {
    metadata
        .iter()
        .find(|(key, value)| key.eq_ignore_ascii_case(wanted_key) && !value.trim().is_empty())
        .map(|(_, value)| value.clone())
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

    fn map(values: &[(&str, &str)]) -> BTreeMap<String, String> {
        values
            .iter()
            .map(|(key, value)| ((*key).to_string(), (*value).to_string()))
            .collect()
    }

    fn healthy_resource() -> CustomResourceInventoryItem {
        CustomResourceInventoryItem {
            cluster_id: "cluster-a".to_string(),
            namespace: Some("apps".to_string()),
            name: "widget-a".to_string(),
            api_version: "example.com/v1".to_string(),
            kind: "Widget".to_string(),
            group: "example.com".to_string(),
            version: "v1".to_string(),
            plural: "widgets".to_string(),
            scope: "Namespaced".to_string(),
            labels: map(&[("team", "platform")]),
            annotations: BTreeMap::new(),
            owner_references_count: 1,
            finalizers: Vec::new(),
            has_status: true,
            ready_condition_status: Some("True".to_string()),
            deletion_timestamp: None,
            spec_keys: vec!["replicas".to_string()],
            status_keys: vec!["conditions".to_string()],
            sensitive_field_paths: Vec::new(),
            created_at: Some(now() - Duration::days(1)),
            collected_at: now(),
        }
    }

    #[test]
    fn cost_flags_missing_owner_metadata() {
        let mut unowned = healthy_resource();
        unowned.labels.clear();

        let report = evaluate_kubernetes_custom_resource_inventory(&[unowned], Pillar::Cost, now());

        assert_eq!(report.resources_evaluated, 1);
        assert_eq!(report.findings.len(), 1);
        assert_eq!(
            report.findings[0].reason_code,
            REASON_COST_OWNER_NOT_RECORDED
        );
        assert_eq!(report.findings[0].pillar, Pillar::Cost);
    }

    #[test]
    fn resilience_flags_unready_custom_resources() {
        let mut unready = healthy_resource();
        unready.ready_condition_status = Some("False".to_string());

        let report =
            evaluate_kubernetes_custom_resource_inventory(&[unready], Pillar::Resilience, now());

        assert_eq!(report.findings.len(), 1);
        assert_eq!(report.findings[0].reason_code, REASON_RES_NOT_READY);
    }

    #[test]
    fn resilience_flags_stuck_deletion_with_finalizers() {
        let mut deleting = healthy_resource();
        deleting.deletion_timestamp = Some(now() - Duration::hours(2));
        deleting.finalizers = vec!["cleanup.example.com/finalizer".to_string()];

        let report =
            evaluate_kubernetes_custom_resource_inventory(&[deleting], Pillar::Resilience, now());

        assert_eq!(report.findings.len(), 1);
        assert_eq!(report.findings[0].reason_code, REASON_RES_STUCK_DELETION);
    }

    #[test]
    fn security_flags_sensitive_spec_or_status_fields() {
        let mut exposed = healthy_resource();
        exposed.sensitive_field_paths = vec!["spec.credentials.token".to_string()];

        let report =
            evaluate_kubernetes_custom_resource_inventory(&[exposed], Pillar::Security, now());

        assert_eq!(report.findings.len(), 1);
        assert_eq!(
            report.findings[0].reason_code,
            REASON_SEC_SENSITIVE_FIELD_EXPOSED
        );
    }

    #[test]
    fn stale_inventory_is_counted_for_any_pillar() {
        let mut stale = healthy_resource();
        stale.collected_at = now() - Duration::hours(DEFAULT_STALE_AFTER_HOURS + 2);

        let report = evaluate_kubernetes_custom_resource_inventory(&[stale], Pillar::Cost, now());

        assert_eq!(report.stale_resources, 1);
        assert_eq!(report.findings[0].reason_code, REASON_INV_STALE_DATA);
    }

    #[test]
    fn healthy_custom_resources_pass_claimed_pillars() {
        for pillar in [Pillar::Cost, Pillar::Resilience, Pillar::Security] {
            let report =
                evaluate_kubernetes_custom_resource_inventory(&[healthy_resource()], pillar, now());

            assert_eq!(report.resources_evaluated, 1);
            assert_eq!(report.stale_resources, 0);
            assert!(report.findings.is_empty());
        }
    }
}

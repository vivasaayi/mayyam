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

// Deterministic Kubernetes VerticalPodAutoscaler inventory evaluator for roadmap rows
// 02-KUBERNETES-DASHBOARD-01177/01184/01205.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::services::aws::inventory::types::{
    score_pillar, InventoryFinding, Pillar, PillarReport, Severity, COST_ALLOCATION_TAG_KEYS,
    DEFAULT_STALE_AFTER_HOURS,
};

pub const RESOURCE_TYPE: &str = "KubernetesVerticalPodAutoscaler";
pub const REASON_COST_OWNER_NOT_RECORDED: &str = "K8S_VPA_COST_OWNER_NOT_RECORDED";
pub const REASON_RES_TARGET_REF_NOT_SET: &str = "K8S_VPA_RES_TARGET_REF_NOT_SET";
pub const REASON_SEC_AUTOMATED_UPDATES_WITHOUT_BOUNDS: &str =
    "K8S_VPA_SEC_AUTOMATED_UPDATES_WITHOUT_BOUNDS";
pub const REASON_INV_STALE_DATA: &str = "K8S_VPA_INV_STALE_DATA";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VpaContainerPolicyInventoryItem {
    pub container_name: Option<String>,
    pub mode: Option<String>,
    pub controlled_resources: Vec<String>,
    pub min_allowed: BTreeMap<String, String>,
    pub max_allowed: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VpaConditionInventoryItem {
    pub type_: String,
    pub status: String,
    pub reason: Option<String>,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VpaInventoryItem {
    pub cluster_id: String,
    pub namespace: String,
    pub name: String,
    pub api_version: String,
    pub labels: BTreeMap<String, String>,
    pub annotations: BTreeMap<String, String>,
    pub target_api_version: Option<String>,
    pub target_kind: Option<String>,
    pub target_name: Option<String>,
    pub update_mode: Option<String>,
    pub recommendation_container_count: usize,
    pub container_policies: Vec<VpaContainerPolicyInventoryItem>,
    pub conditions: Vec<VpaConditionInventoryItem>,
    pub spec: Value,
    pub status: Value,
    pub created_at: Option<DateTime<Utc>>,
    pub collected_at: DateTime<Utc>,
}

pub fn evaluate_kubernetes_vpa_inventory(
    vpas: &[VpaInventoryItem],
    pillar: Pillar,
    now: DateTime<Utc>,
) -> PillarReport {
    let mut stale_resources = 0;
    let mut findings = Vec::new();

    for vpa in vpas {
        if let Some(finding) = stale_finding(vpa, pillar, now) {
            stale_resources += 1;
            findings.push(finding);
        }

        match pillar {
            Pillar::Cost => evaluate_cost(vpa, pillar, &mut findings),
            Pillar::Resilience => evaluate_resilience(vpa, pillar, &mut findings),
            Pillar::Security => evaluate_security(vpa, pillar, &mut findings),
            _ => {}
        }
    }

    PillarReport {
        pillar,
        resources_evaluated: vpas.len(),
        stale_resources,
        score: score_pillar(&findings),
        findings,
    }
}

fn evaluate_cost(vpa: &VpaInventoryItem, pillar: Pillar, findings: &mut Vec<InventoryFinding>) {
    if has_any_metadata_key(&vpa.labels, COST_ALLOCATION_TAG_KEYS)
        || has_any_metadata_key(&vpa.annotations, COST_ALLOCATION_TAG_KEYS)
    {
        return;
    }

    findings.push(finding(
        vpa,
        pillar,
        REASON_COST_OWNER_NOT_RECORDED,
        Severity::Medium,
        format!(
            "Kubernetes VerticalPodAutoscaler {}/{} has no owner, team, project, or cost-center label or annotation",
            vpa.namespace, vpa.name
        ),
        json!({
            "cluster_id": vpa.cluster_id,
            "namespace": vpa.namespace,
            "name": vpa.name,
            "target_kind": vpa.target_kind,
            "target_name": vpa.target_name,
            "checked_keys": COST_ALLOCATION_TAG_KEYS,
            "checked_locations": ["labels", "annotations"],
        }),
    ));
}

fn evaluate_resilience(
    vpa: &VpaInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    let target_kind_missing = vpa
        .target_kind
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .is_none();
    let target_name_missing = vpa
        .target_name
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .is_none();

    if target_kind_missing || target_name_missing {
        findings.push(finding(
            vpa,
            pillar,
            REASON_RES_TARGET_REF_NOT_SET,
            Severity::High,
            format!(
                "Kubernetes VerticalPodAutoscaler {}/{} has no complete targetRef",
                vpa.namespace, vpa.name
            ),
            json!({
                "cluster_id": vpa.cluster_id,
                "namespace": vpa.namespace,
                "name": vpa.name,
                "target_api_version": vpa.target_api_version,
                "target_kind": vpa.target_kind,
                "target_name": vpa.target_name,
                "recommendation": "Set spec.targetRef so the VPA recommendations are tied to a concrete workload",
            }),
        ));
    }
}

fn evaluate_security(vpa: &VpaInventoryItem, pillar: Pillar, findings: &mut Vec<InventoryFinding>) {
    if !is_automated_update_mode(vpa.update_mode.as_deref()) || has_bounded_resource_policy(vpa) {
        return;
    }

    findings.push(finding(
        vpa,
        pillar,
        REASON_SEC_AUTOMATED_UPDATES_WITHOUT_BOUNDS,
        Severity::High,
        format!(
            "Kubernetes VerticalPodAutoscaler {}/{} can apply automated updates without explicit resource bounds",
            vpa.namespace, vpa.name
        ),
        json!({
            "cluster_id": vpa.cluster_id,
            "namespace": vpa.namespace,
            "name": vpa.name,
            "update_mode": vpa.update_mode,
            "container_policies": vpa.container_policies,
            "recommendation": "Set resourcePolicy.containerPolicies with minAllowed and maxAllowed for controlled resources before enabling Auto or Recreate updates",
        }),
    ));
}

fn stale_finding(
    vpa: &VpaInventoryItem,
    pillar: Pillar,
    now: DateTime<Utc>,
) -> Option<InventoryFinding> {
    let age_hours = (now - vpa.collected_at).num_hours();
    if age_hours <= DEFAULT_STALE_AFTER_HOURS {
        return None;
    }

    Some(finding(
        vpa,
        pillar,
        REASON_INV_STALE_DATA,
        Severity::Medium,
        format!(
            "Inventory data for Kubernetes VerticalPodAutoscaler {}/{} is {} hours old (threshold {} hours)",
            vpa.namespace, vpa.name, age_hours, DEFAULT_STALE_AFTER_HOURS
        ),
        json!({
            "cluster_id": vpa.cluster_id,
            "namespace": vpa.namespace,
            "name": vpa.name,
            "collected_at": vpa.collected_at,
            "age_hours": age_hours,
            "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
        }),
    ))
}

fn finding(
    vpa: &VpaInventoryItem,
    pillar: Pillar,
    reason_code: &str,
    severity: Severity,
    message: String,
    evidence: Value,
) -> InventoryFinding {
    InventoryFinding {
        resource_id: format!(
            "{}/{}/VerticalPodAutoscaler/{}",
            vpa.cluster_id, vpa.namespace, vpa.name
        ),
        arn: format!(
            "kubernetes://verticalpodautoscalers/{}/{}/{}",
            vpa.cluster_id, vpa.namespace, vpa.name
        ),
        pillar,
        reason_code: reason_code.to_string(),
        severity,
        message,
        evidence,
    }
}

fn is_automated_update_mode(update_mode: Option<&str>) -> bool {
    update_mode
        .map(|mode| {
            let mode = mode.trim();
            mode.eq_ignore_ascii_case("Auto") || mode.eq_ignore_ascii_case("Recreate")
        })
        .unwrap_or(true)
}

fn has_bounded_resource_policy(vpa: &VpaInventoryItem) -> bool {
    vpa.container_policies
        .iter()
        .any(policy_has_resource_bounds)
}

fn policy_has_resource_bounds(policy: &VpaContainerPolicyInventoryItem) -> bool {
    let required_resources = if policy.controlled_resources.is_empty() {
        vec!["cpu", "memory"]
    } else {
        policy
            .controlled_resources
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>()
    };

    required_resources.iter().all(|resource| {
        metadata_value(&policy.min_allowed, resource).is_some()
            && metadata_value(&policy.max_allowed, resource).is_some()
    })
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
    use serde_json::json;

    fn now() -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2026-06-10T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc)
    }

    fn labels(values: &[(&str, &str)]) -> BTreeMap<String, String> {
        values
            .iter()
            .map(|(key, value)| ((*key).to_string(), (*value).to_string()))
            .collect()
    }

    fn bounded_policy() -> VpaContainerPolicyInventoryItem {
        VpaContainerPolicyInventoryItem {
            container_name: Some("*".to_string()),
            mode: Some("Auto".to_string()),
            controlled_resources: vec!["cpu".to_string(), "memory".to_string()],
            min_allowed: labels(&[("cpu", "100m"), ("memory", "128Mi")]),
            max_allowed: labels(&[("cpu", "2"), ("memory", "2Gi")]),
        }
    }

    fn vpa(name: &str, metadata_labels: BTreeMap<String, String>) -> VpaInventoryItem {
        VpaInventoryItem {
            cluster_id: "cluster-a".to_string(),
            namespace: "apps".to_string(),
            name: name.to_string(),
            api_version: "autoscaling.k8s.io/v1".to_string(),
            labels: metadata_labels,
            annotations: BTreeMap::new(),
            target_api_version: Some("apps/v1".to_string()),
            target_kind: Some("Deployment".to_string()),
            target_name: Some("checkout".to_string()),
            update_mode: Some("Auto".to_string()),
            recommendation_container_count: 1,
            container_policies: vec![bounded_policy()],
            conditions: vec![VpaConditionInventoryItem {
                type_: "RecommendationProvided".to_string(),
                status: "True".to_string(),
                reason: None,
                message: None,
            }],
            spec: json!({}),
            status: json!({}),
            created_at: Some(now() - Duration::days(3)),
            collected_at: now(),
        }
    }

    fn healthy_vpa() -> VpaInventoryItem {
        vpa("checkout-vpa", labels(&[("team", "payments")]))
    }

    #[test]
    fn cost_flags_missing_owner_and_cost_allocation_labels() {
        let report = evaluate_kubernetes_vpa_inventory(
            &[vpa("untagged", BTreeMap::new())],
            Pillar::Cost,
            now(),
        );

        assert_eq!(report.resources_evaluated, 1);
        assert_eq!(report.findings.len(), 1);
        assert_eq!(
            report.findings[0].reason_code,
            REASON_COST_OWNER_NOT_RECORDED
        );
        assert_eq!(report.findings[0].pillar, Pillar::Cost);
    }

    #[test]
    fn resilience_flags_missing_target_reference() {
        let mut missing_target = healthy_vpa();
        missing_target.target_api_version = None;
        missing_target.target_kind = None;
        missing_target.target_name = None;

        let report =
            evaluate_kubernetes_vpa_inventory(&[missing_target], Pillar::Resilience, now());

        assert_eq!(report.findings.len(), 1);
        assert_eq!(
            report.findings[0].reason_code,
            REASON_RES_TARGET_REF_NOT_SET
        );
    }

    #[test]
    fn security_flags_auto_updates_without_resource_bounds() {
        let mut unbounded = healthy_vpa();
        unbounded.container_policies.clear();

        let report = evaluate_kubernetes_vpa_inventory(&[unbounded], Pillar::Security, now());

        assert_eq!(report.findings.len(), 1);
        assert_eq!(
            report.findings[0].reason_code,
            REASON_SEC_AUTOMATED_UPDATES_WITHOUT_BOUNDS
        );
    }

    #[test]
    fn stale_inventory_is_counted_for_any_pillar() {
        let mut stale = healthy_vpa();
        stale.collected_at = now() - Duration::hours(DEFAULT_STALE_AFTER_HOURS + 2);

        let report = evaluate_kubernetes_vpa_inventory(&[stale], Pillar::Cost, now());

        assert_eq!(report.stale_resources, 1);
        assert_eq!(report.findings[0].reason_code, REASON_INV_STALE_DATA);
    }

    #[test]
    fn healthy_vpas_pass_claimed_pillars() {
        for pillar in [Pillar::Cost, Pillar::Resilience, Pillar::Security] {
            let report = evaluate_kubernetes_vpa_inventory(&[healthy_vpa()], pillar, now());

            assert_eq!(report.resources_evaluated, 1);
            assert_eq!(report.stale_resources, 0);
            assert!(report.findings.is_empty());
        }
    }
}

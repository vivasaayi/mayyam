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

// Deterministic Kubernetes NetworkPolicy inventory evaluator for roadmap rows
// 02-KUBERNETES-DASHBOARD-01079/01086/01107.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::services::aws::inventory::types::{
    score_pillar, InventoryFinding, Pillar, PillarReport, Severity, COST_ALLOCATION_TAG_KEYS,
    DEFAULT_STALE_AFTER_HOURS,
};

pub const RESOURCE_TYPE: &str = "KubernetesNetworkPolicy";
pub const REASON_COST_OWNER_NOT_RECORDED: &str = "K8S_NETWORK_POLICY_COST_OWNER_NOT_RECORDED";
pub const REASON_RES_NO_POLICY_RULES: &str = "K8S_NETWORK_POLICY_RES_NO_POLICY_RULES";
pub const REASON_SEC_NAMESPACE_WIDE_SELECTOR: &str =
    "K8S_NETWORK_POLICY_SEC_NAMESPACE_WIDE_SELECTOR";
pub const REASON_SEC_ALLOW_ALL_TRAFFIC: &str = "K8S_NETWORK_POLICY_SEC_ALLOW_ALL_TRAFFIC";
pub const REASON_INV_STALE_DATA: &str = "K8S_NETWORK_POLICY_INV_STALE_DATA";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkPolicyOwnerReferenceInventoryItem {
    pub kind: Option<String>,
    pub name: String,
    pub controller: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkPolicySelectorInventoryItem {
    pub match_labels: BTreeMap<String, String>,
    pub match_expression_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkPolicyPortInventoryItem {
    pub protocol: Option<String>,
    pub port: Option<String>,
    pub end_port: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkPolicyPeerInventoryItem {
    pub ip_block_cidr: Option<String>,
    pub ip_block_except: Vec<String>,
    pub namespace_selector: Option<NetworkPolicySelectorInventoryItem>,
    pub pod_selector: Option<NetworkPolicySelectorInventoryItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkPolicyRuleInventoryItem {
    pub direction: String,
    pub peers: Vec<NetworkPolicyPeerInventoryItem>,
    pub ports: Vec<NetworkPolicyPortInventoryItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkPolicyInventoryItem {
    pub cluster_id: String,
    pub namespace: String,
    pub name: String,
    pub labels: BTreeMap<String, String>,
    pub annotations: BTreeMap<String, String>,
    pub pod_selector: NetworkPolicySelectorInventoryItem,
    pub policy_types: Vec<String>,
    pub ingress_rules: Vec<NetworkPolicyRuleInventoryItem>,
    pub egress_rules: Vec<NetworkPolicyRuleInventoryItem>,
    pub owner_references: Vec<NetworkPolicyOwnerReferenceInventoryItem>,
    pub created_at: Option<DateTime<Utc>>,
    pub collected_at: DateTime<Utc>,
}

pub fn evaluate_kubernetes_network_policy_inventory(
    network_policies: &[NetworkPolicyInventoryItem],
    pillar: Pillar,
    now: DateTime<Utc>,
) -> PillarReport {
    let mut stale_resources = 0;
    let mut findings = Vec::new();

    for network_policy in network_policies {
        if let Some(finding) = stale_finding(network_policy, pillar, now) {
            stale_resources += 1;
            findings.push(finding);
        }

        match pillar {
            Pillar::Cost => evaluate_cost(network_policy, pillar, &mut findings),
            Pillar::Resilience => evaluate_resilience(network_policy, pillar, &mut findings),
            Pillar::Security => evaluate_security(network_policy, pillar, &mut findings),
            _ => {}
        }
    }

    PillarReport {
        pillar,
        resources_evaluated: network_policies.len(),
        stale_resources,
        score: score_pillar(&findings),
        findings,
    }
}

fn evaluate_cost(
    network_policy: &NetworkPolicyInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if has_any_metadata_key(&network_policy.labels, COST_ALLOCATION_TAG_KEYS)
        || has_any_metadata_key(&network_policy.annotations, COST_ALLOCATION_TAG_KEYS)
    {
        return;
    }

    findings.push(finding(
        network_policy,
        pillar,
        REASON_COST_OWNER_NOT_RECORDED,
        Severity::Medium,
        format!(
            "Kubernetes NetworkPolicy {}/{} has no owner, team, project, or cost-center label or annotation",
            network_policy.namespace, network_policy.name
        ),
        json!({
            "cluster_id": network_policy.cluster_id,
            "namespace": network_policy.namespace,
            "name": network_policy.name,
            "policy_types": network_policy.policy_types,
            "ingress_rule_count": network_policy.ingress_rules.len(),
            "egress_rule_count": network_policy.egress_rules.len(),
            "checked_keys": COST_ALLOCATION_TAG_KEYS,
            "checked_locations": ["labels", "annotations"],
        }),
    ));
}

fn evaluate_resilience(
    network_policy: &NetworkPolicyInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if network_policy.ingress_rules.is_empty() && network_policy.egress_rules.is_empty() {
        findings.push(finding(
            network_policy,
            pillar,
            REASON_RES_NO_POLICY_RULES,
            Severity::Medium,
            format!(
                "Kubernetes NetworkPolicy {}/{} has no ingress or egress allow rules",
                network_policy.namespace, network_policy.name
            ),
            json!({
                "cluster_id": network_policy.cluster_id,
                "namespace": network_policy.namespace,
                "name": network_policy.name,
                "pod_selector": network_policy.pod_selector,
                "policy_types": network_policy.policy_types,
                "owner_references": network_policy.owner_references,
                "recommendation": "Confirm the selected workloads are intentionally isolated, or add explicit allow rules for required service dependencies",
            }),
        ));
    }
}

fn evaluate_security(
    network_policy: &NetworkPolicyInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if selector_matches_everything(&network_policy.pod_selector) {
        findings.push(finding(
            network_policy,
            pillar,
            REASON_SEC_NAMESPACE_WIDE_SELECTOR,
            Severity::Medium,
            format!(
                "Kubernetes NetworkPolicy {}/{} selects every pod in the namespace",
                network_policy.namespace, network_policy.name
            ),
            json!({
                "cluster_id": network_policy.cluster_id,
                "namespace": network_policy.namespace,
                "name": network_policy.name,
                "pod_selector": network_policy.pod_selector,
                "policy_types": network_policy.policy_types,
                "recommendation": "Use explicit podSelector labels so policy scope matches the intended workload blast radius",
            }),
        ));
    }

    let allow_all_rules = network_policy
        .ingress_rules
        .iter()
        .chain(network_policy.egress_rules.iter())
        .filter(|rule| rule_allows_all_traffic(rule))
        .cloned()
        .collect::<Vec<_>>();
    if !allow_all_rules.is_empty() {
        findings.push(finding(
            network_policy,
            pillar,
            REASON_SEC_ALLOW_ALL_TRAFFIC,
            Severity::High,
            format!(
                "Kubernetes NetworkPolicy {}/{} contains rules that allow all peers and ports",
                network_policy.namespace, network_policy.name
            ),
            json!({
                "cluster_id": network_policy.cluster_id,
                "namespace": network_policy.namespace,
                "name": network_policy.name,
                "matching_rules": allow_all_rules,
                "recommendation": "Constrain NetworkPolicy peers and ports to the minimum required sources, destinations, protocols, and ports",
            }),
        ));
    }
}

fn stale_finding(
    network_policy: &NetworkPolicyInventoryItem,
    pillar: Pillar,
    now: DateTime<Utc>,
) -> Option<InventoryFinding> {
    let age_hours = (now - network_policy.collected_at).num_hours();
    if age_hours <= DEFAULT_STALE_AFTER_HOURS {
        return None;
    }

    Some(finding(
        network_policy,
        pillar,
        REASON_INV_STALE_DATA,
        Severity::Medium,
        format!(
            "Inventory data for Kubernetes NetworkPolicy {}/{} is {} hours old (threshold {} hours)",
            network_policy.namespace, network_policy.name, age_hours, DEFAULT_STALE_AFTER_HOURS
        ),
        json!({
            "cluster_id": network_policy.cluster_id,
            "namespace": network_policy.namespace,
            "name": network_policy.name,
            "collected_at": network_policy.collected_at,
            "age_hours": age_hours,
            "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
        }),
    ))
}

fn finding(
    network_policy: &NetworkPolicyInventoryItem,
    pillar: Pillar,
    reason_code: &str,
    severity: Severity,
    message: String,
    evidence: Value,
) -> InventoryFinding {
    InventoryFinding {
        resource_id: format!(
            "{}/{}/NetworkPolicy/{}",
            network_policy.cluster_id, network_policy.namespace, network_policy.name
        ),
        arn: format!(
            "kubernetes://networkpolicies/{}/{}/{}",
            network_policy.cluster_id, network_policy.namespace, network_policy.name
        ),
        pillar,
        reason_code: reason_code.to_string(),
        severity,
        message,
        evidence,
    }
}

fn selector_matches_everything(selector: &NetworkPolicySelectorInventoryItem) -> bool {
    selector.match_labels.is_empty() && selector.match_expression_count == 0
}

fn rule_allows_all_traffic(rule: &NetworkPolicyRuleInventoryItem) -> bool {
    rule.peers.is_empty() && rule.ports.is_empty()
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

    fn labels(values: &[(&str, &str)]) -> BTreeMap<String, String> {
        values
            .iter()
            .map(|(key, value)| ((*key).to_string(), (*value).to_string()))
            .collect()
    }

    fn selector(values: &[(&str, &str)]) -> NetworkPolicySelectorInventoryItem {
        NetworkPolicySelectorInventoryItem {
            match_labels: labels(values),
            match_expression_count: 0,
        }
    }

    fn limited_ingress_rule() -> NetworkPolicyRuleInventoryItem {
        NetworkPolicyRuleInventoryItem {
            direction: "ingress".to_string(),
            peers: vec![NetworkPolicyPeerInventoryItem {
                ip_block_cidr: None,
                ip_block_except: Vec::new(),
                namespace_selector: None,
                pod_selector: Some(selector(&[("app", "frontend")])),
            }],
            ports: vec![NetworkPolicyPortInventoryItem {
                protocol: Some("TCP".to_string()),
                port: Some("8443".to_string()),
                end_port: None,
            }],
        }
    }

    fn allow_all_egress_rule() -> NetworkPolicyRuleInventoryItem {
        NetworkPolicyRuleInventoryItem {
            direction: "egress".to_string(),
            peers: Vec::new(),
            ports: Vec::new(),
        }
    }

    fn network_policy(
        name: &str,
        metadata_labels: BTreeMap<String, String>,
    ) -> NetworkPolicyInventoryItem {
        NetworkPolicyInventoryItem {
            cluster_id: "cluster-a".to_string(),
            namespace: "apps".to_string(),
            name: name.to_string(),
            labels: metadata_labels,
            annotations: BTreeMap::new(),
            pod_selector: selector(&[("app", "checkout")]),
            policy_types: vec!["Ingress".to_string()],
            ingress_rules: vec![limited_ingress_rule()],
            egress_rules: Vec::new(),
            owner_references: Vec::new(),
            created_at: Some(now() - Duration::days(3)),
            collected_at: now(),
        }
    }

    fn healthy_network_policy() -> NetworkPolicyInventoryItem {
        network_policy("checkout-ingress", labels(&[("team", "payments")]))
    }

    #[test]
    fn cost_flags_missing_owner_and_cost_allocation_labels() {
        let report = evaluate_kubernetes_network_policy_inventory(
            &[network_policy("untagged", BTreeMap::new())],
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
    fn resilience_flags_policies_with_no_allow_rules() {
        let mut isolated = healthy_network_policy();
        isolated.ingress_rules = Vec::new();
        isolated.egress_rules = Vec::new();

        let report =
            evaluate_kubernetes_network_policy_inventory(&[isolated], Pillar::Resilience, now());

        assert_eq!(report.findings.len(), 1);
        assert_eq!(report.findings[0].reason_code, REASON_RES_NO_POLICY_RULES);
    }

    #[test]
    fn security_flags_namespace_wide_selectors_and_allow_all_rules() {
        let mut risky = healthy_network_policy();
        risky.pod_selector = selector(&[]);
        risky.egress_rules = vec![allow_all_egress_rule()];

        let report =
            evaluate_kubernetes_network_policy_inventory(&[risky], Pillar::Security, now());
        let reason_codes = report
            .findings
            .iter()
            .map(|finding| finding.reason_code.as_str())
            .collect::<Vec<_>>();

        assert!(reason_codes.contains(&REASON_SEC_NAMESPACE_WIDE_SELECTOR));
        assert!(reason_codes.contains(&REASON_SEC_ALLOW_ALL_TRAFFIC));
    }

    #[test]
    fn stale_inventory_is_counted_for_any_pillar() {
        let mut stale = healthy_network_policy();
        stale.collected_at = now() - Duration::hours(DEFAULT_STALE_AFTER_HOURS + 2);

        let report = evaluate_kubernetes_network_policy_inventory(&[stale], Pillar::Cost, now());

        assert_eq!(report.stale_resources, 1);
        assert_eq!(report.findings[0].reason_code, REASON_INV_STALE_DATA);
    }

    #[test]
    fn healthy_network_policies_pass_claimed_pillars() {
        for pillar in [Pillar::Cost, Pillar::Resilience, Pillar::Security] {
            let report = evaluate_kubernetes_network_policy_inventory(
                &[healthy_network_policy()],
                pillar,
                now(),
            );

            assert_eq!(report.resources_evaluated, 1);
            assert_eq!(report.stale_resources, 0);
            assert!(report.findings.is_empty());
        }
    }
}

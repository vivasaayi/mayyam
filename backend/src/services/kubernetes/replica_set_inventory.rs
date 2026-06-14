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

// Deterministic Kubernetes ReplicaSet inventory evaluator for roadmap rows
// 02-KUBERNETES-DASHBOARD-00246/00253/00274.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::services::aws::inventory::types::{
    score_pillar, InventoryFinding, Pillar, PillarReport, Severity, COST_ALLOCATION_TAG_KEYS,
    DEFAULT_STALE_AFTER_HOURS,
};

pub const RESOURCE_TYPE: &str = "KubernetesReplicaSet";
pub const REASON_COST_OWNER_NOT_RECORDED: &str = "K8S_REPLICASET_COST_OWNER_NOT_RECORDED";
pub const REASON_RES_REPLICAS_UNAVAILABLE: &str = "K8S_REPLICASET_RES_REPLICAS_UNAVAILABLE";
pub const REASON_RES_READY_REPLICAS_LAG: &str = "K8S_REPLICASET_RES_READY_REPLICAS_LAG";
pub const REASON_RES_FULLY_LABELED_REPLICAS_LAG: &str =
    "K8S_REPLICASET_RES_FULLY_LABELED_REPLICAS_LAG";
pub const REASON_RES_GENERATION_NOT_OBSERVED: &str = "K8S_REPLICASET_RES_GENERATION_NOT_OBSERVED";
pub const REASON_SEC_PRIVILEGED_CONTAINER: &str = "K8S_REPLICASET_SEC_PRIVILEGED_CONTAINER";
pub const REASON_SEC_HOST_NETWORK: &str = "K8S_REPLICASET_SEC_HOST_NETWORK";
pub const REASON_INV_STALE_DATA: &str = "K8S_REPLICASET_INV_STALE_DATA";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplicaSetContainerInventoryItem {
    pub name: String,
    pub image: Option<String>,
    pub privileged: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplicaSetOwnerReferenceInventoryItem {
    pub api_version: String,
    pub kind: String,
    pub name: String,
    pub controller: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplicaSetInventoryItem {
    pub cluster_id: String,
    pub namespace: String,
    pub name: String,
    pub desired_replicas: i32,
    pub current_replicas: i32,
    pub available_replicas: i32,
    pub ready_replicas: i32,
    pub fully_labeled_replicas: i32,
    pub generation: Option<i64>,
    pub observed_generation: Option<i64>,
    pub labels: BTreeMap<String, String>,
    pub annotations: BTreeMap<String, String>,
    pub selector: BTreeMap<String, String>,
    pub pod_template_labels: BTreeMap<String, String>,
    pub containers: Vec<ReplicaSetContainerInventoryItem>,
    pub owner_references: Vec<ReplicaSetOwnerReferenceInventoryItem>,
    pub service_account_name: Option<String>,
    pub host_network: bool,
    pub created_at: Option<DateTime<Utc>>,
    pub collected_at: DateTime<Utc>,
}

pub fn evaluate_kubernetes_replicaset_inventory(
    replicasets: &[ReplicaSetInventoryItem],
    pillar: Pillar,
    now: DateTime<Utc>,
) -> PillarReport {
    let mut stale_resources = 0;
    let mut findings = Vec::new();

    for replicaset in replicasets {
        if let Some(finding) = stale_finding(replicaset, pillar, now) {
            stale_resources += 1;
            findings.push(finding);
        }

        match pillar {
            Pillar::Cost => evaluate_cost(replicaset, pillar, &mut findings),
            Pillar::Resilience => evaluate_resilience(replicaset, pillar, &mut findings),
            Pillar::Security => evaluate_security(replicaset, pillar, &mut findings),
            _ => {}
        }
    }

    PillarReport {
        pillar,
        resources_evaluated: replicasets.len(),
        stale_resources,
        score: score_pillar(&findings),
        findings,
    }
}

fn evaluate_cost(
    replicaset: &ReplicaSetInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if has_any_metadata_key(&replicaset.labels, COST_ALLOCATION_TAG_KEYS)
        || has_any_metadata_key(&replicaset.annotations, COST_ALLOCATION_TAG_KEYS)
        || has_any_metadata_key(&replicaset.pod_template_labels, COST_ALLOCATION_TAG_KEYS)
    {
        return;
    }

    findings.push(finding(
        replicaset,
        pillar,
        REASON_COST_OWNER_NOT_RECORDED,
        Severity::Medium,
        format!(
            "Kubernetes ReplicaSet {}/{} has no owner, team, project, or cost-center label or annotation",
            replicaset.namespace, replicaset.name
        ),
        json!({
            "cluster_id": replicaset.cluster_id,
            "namespace": replicaset.namespace,
            "replicaset": replicaset.name,
            "desired_replicas": replicaset.desired_replicas,
            "owner_references": replicaset.owner_references,
            "checked_keys": COST_ALLOCATION_TAG_KEYS,
            "checked_locations": ["labels", "annotations", "pod_template_labels"],
        }),
    ));
}

fn evaluate_resilience(
    replicaset: &ReplicaSetInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if replicaset.desired_replicas > 0
        && replicaset.available_replicas < replicaset.desired_replicas
    {
        findings.push(finding(
            replicaset,
            pillar,
            REASON_RES_REPLICAS_UNAVAILABLE,
            Severity::High,
            format!(
                "Kubernetes ReplicaSet {}/{} has {}/{} available replicas",
                replicaset.namespace,
                replicaset.name,
                replicaset.available_replicas,
                replicaset.desired_replicas
            ),
            json!({
                "cluster_id": replicaset.cluster_id,
                "namespace": replicaset.namespace,
                "replicaset": replicaset.name,
                "desired_replicas": replicaset.desired_replicas,
                "current_replicas": replicaset.current_replicas,
                "available_replicas": replicaset.available_replicas,
                "ready_replicas": replicaset.ready_replicas,
            }),
        ));
    }

    if replicaset.desired_replicas > 0 && replicaset.ready_replicas < replicaset.desired_replicas {
        findings.push(finding(
            replicaset,
            pillar,
            REASON_RES_READY_REPLICAS_LAG,
            Severity::High,
            format!(
                "Kubernetes ReplicaSet {}/{} has {}/{} ready replicas",
                replicaset.namespace,
                replicaset.name,
                replicaset.ready_replicas,
                replicaset.desired_replicas
            ),
            json!({
                "cluster_id": replicaset.cluster_id,
                "namespace": replicaset.namespace,
                "replicaset": replicaset.name,
                "desired_replicas": replicaset.desired_replicas,
                "ready_replicas": replicaset.ready_replicas,
                "available_replicas": replicaset.available_replicas,
            }),
        ));
    }

    if replicaset.desired_replicas > 0
        && replicaset.fully_labeled_replicas < replicaset.desired_replicas
    {
        findings.push(finding(
            replicaset,
            pillar,
            REASON_RES_FULLY_LABELED_REPLICAS_LAG,
            Severity::Medium,
            format!(
                "Kubernetes ReplicaSet {}/{} has {}/{} fully labeled replicas",
                replicaset.namespace,
                replicaset.name,
                replicaset.fully_labeled_replicas,
                replicaset.desired_replicas
            ),
            json!({
                "cluster_id": replicaset.cluster_id,
                "namespace": replicaset.namespace,
                "replicaset": replicaset.name,
                "desired_replicas": replicaset.desired_replicas,
                "fully_labeled_replicas": replicaset.fully_labeled_replicas,
                "selector": replicaset.selector,
                "pod_template_labels": replicaset.pod_template_labels,
            }),
        ));
    }

    if let (Some(generation), Some(observed_generation)) =
        (replicaset.generation, replicaset.observed_generation)
    {
        if observed_generation < generation {
            findings.push(finding(
                replicaset,
                pillar,
                REASON_RES_GENERATION_NOT_OBSERVED,
                Severity::Medium,
                format!(
                    "Kubernetes ReplicaSet {}/{} controller has not observed generation {}",
                    replicaset.namespace, replicaset.name, generation
                ),
                json!({
                    "cluster_id": replicaset.cluster_id,
                    "namespace": replicaset.namespace,
                    "replicaset": replicaset.name,
                    "generation": generation,
                    "observed_generation": observed_generation,
                }),
            ));
        }
    }
}

fn evaluate_security(
    replicaset: &ReplicaSetInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    let privileged_containers = replicaset
        .containers
        .iter()
        .filter(|container| container.privileged == Some(true))
        .map(|container| container.name.clone())
        .collect::<Vec<_>>();
    if !privileged_containers.is_empty() {
        findings.push(finding(
            replicaset,
            pillar,
            REASON_SEC_PRIVILEGED_CONTAINER,
            Severity::High,
            format!(
                "Kubernetes ReplicaSet {}/{} template has privileged containers",
                replicaset.namespace, replicaset.name
            ),
            json!({
                "cluster_id": replicaset.cluster_id,
                "namespace": replicaset.namespace,
                "replicaset": replicaset.name,
                "privileged_containers": privileged_containers,
                "service_account_name": replicaset.service_account_name,
            }),
        ));
    }

    if replicaset.host_network {
        findings.push(finding(
            replicaset,
            pillar,
            REASON_SEC_HOST_NETWORK,
            Severity::High,
            format!(
                "Kubernetes ReplicaSet {}/{} template runs with hostNetwork enabled",
                replicaset.namespace, replicaset.name
            ),
            json!({
                "cluster_id": replicaset.cluster_id,
                "namespace": replicaset.namespace,
                "replicaset": replicaset.name,
                "host_network": replicaset.host_network,
            }),
        ));
    }
}

fn stale_finding(
    replicaset: &ReplicaSetInventoryItem,
    pillar: Pillar,
    now: DateTime<Utc>,
) -> Option<InventoryFinding> {
    let age_hours = (now - replicaset.collected_at).num_hours();
    if age_hours <= DEFAULT_STALE_AFTER_HOURS {
        return None;
    }

    Some(finding(
        replicaset,
        pillar,
        REASON_INV_STALE_DATA,
        Severity::Medium,
        format!(
            "Inventory data for Kubernetes ReplicaSet {}/{} is {} hours old (threshold {} hours)",
            replicaset.namespace, replicaset.name, age_hours, DEFAULT_STALE_AFTER_HOURS
        ),
        json!({
            "cluster_id": replicaset.cluster_id,
            "namespace": replicaset.namespace,
            "replicaset": replicaset.name,
            "collected_at": replicaset.collected_at,
            "age_hours": age_hours,
            "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
        }),
    ))
}

fn finding(
    replicaset: &ReplicaSetInventoryItem,
    pillar: Pillar,
    reason_code: &str,
    severity: Severity,
    message: String,
    evidence: Value,
) -> InventoryFinding {
    InventoryFinding {
        resource_id: format!(
            "{}/{}/{}",
            replicaset.cluster_id, replicaset.namespace, replicaset.name
        ),
        arn: format!(
            "kubernetes://replicaset/{}/{}/{}",
            replicaset.cluster_id, replicaset.namespace, replicaset.name
        ),
        pillar,
        reason_code: reason_code.to_string(),
        severity,
        message,
        evidence,
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

    fn labels(values: &[(&str, &str)]) -> BTreeMap<String, String> {
        values
            .iter()
            .map(|(key, value)| ((*key).to_string(), (*value).to_string()))
            .collect()
    }

    fn container(name: &str, privileged: Option<bool>) -> ReplicaSetContainerInventoryItem {
        ReplicaSetContainerInventoryItem {
            name: name.to_string(),
            image: Some("registry.local/app:1.0.0".to_string()),
            privileged,
        }
    }

    fn owner(
        kind: &str,
        name: &str,
        controller: Option<bool>,
    ) -> ReplicaSetOwnerReferenceInventoryItem {
        ReplicaSetOwnerReferenceInventoryItem {
            api_version: "apps/v1".to_string(),
            kind: kind.to_string(),
            name: name.to_string(),
            controller,
        }
    }

    fn replicaset(
        name: &str,
        metadata_labels: BTreeMap<String, String>,
        desired_replicas: i32,
        available_replicas: i32,
        ready_replicas: i32,
        fully_labeled_replicas: i32,
    ) -> ReplicaSetInventoryItem {
        ReplicaSetInventoryItem {
            cluster_id: "cluster-a".to_string(),
            namespace: "payments".to_string(),
            name: name.to_string(),
            desired_replicas,
            current_replicas: available_replicas,
            available_replicas,
            ready_replicas,
            fully_labeled_replicas,
            generation: Some(7),
            observed_generation: Some(7),
            labels: metadata_labels,
            annotations: BTreeMap::new(),
            selector: labels(&[("app", "checkout")]),
            pod_template_labels: labels(&[("app", "checkout")]),
            containers: vec![container("api", Some(false))],
            owner_references: vec![owner("Deployment", "checkout", Some(true))],
            service_account_name: Some("checkout".to_string()),
            host_network: false,
            created_at: Some(now() - Duration::hours(4)),
            collected_at: now(),
        }
    }

    fn healthy_replicaset() -> ReplicaSetInventoryItem {
        replicaset(
            "checkout-7f845",
            labels(&[("team", "payments")]),
            3,
            3,
            3,
            3,
        )
    }

    #[test]
    fn cost_flags_missing_owner_and_cost_allocation_labels() {
        let report = evaluate_kubernetes_replicaset_inventory(
            &[replicaset("untagged", BTreeMap::new(), 2, 2, 2, 2)],
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
    fn resilience_flags_replica_and_generation_gaps() {
        let mut lagging = replicaset(
            "checkout-7f845",
            labels(&[("team", "payments")]),
            4,
            2,
            1,
            3,
        );
        lagging.generation = Some(9);
        lagging.observed_generation = Some(8);

        let report =
            evaluate_kubernetes_replicaset_inventory(&[lagging], Pillar::Resilience, now());
        let reason_codes = report
            .findings
            .iter()
            .map(|finding| finding.reason_code.as_str())
            .collect::<Vec<_>>();

        assert!(reason_codes.contains(&REASON_RES_REPLICAS_UNAVAILABLE));
        assert!(reason_codes.contains(&REASON_RES_READY_REPLICAS_LAG));
        assert!(reason_codes.contains(&REASON_RES_FULLY_LABELED_REPLICAS_LAG));
        assert!(reason_codes.contains(&REASON_RES_GENERATION_NOT_OBSERVED));
    }

    #[test]
    fn security_flags_privileged_template_and_host_network() {
        let mut exposed = healthy_replicaset();
        exposed.host_network = true;
        exposed.containers = vec![
            container("api", Some(false)),
            container("sidecar", Some(true)),
        ];

        let report = evaluate_kubernetes_replicaset_inventory(&[exposed], Pillar::Security, now());
        let reason_codes = report
            .findings
            .iter()
            .map(|finding| finding.reason_code.as_str())
            .collect::<Vec<_>>();

        assert!(reason_codes.contains(&REASON_SEC_PRIVILEGED_CONTAINER));
        assert!(reason_codes.contains(&REASON_SEC_HOST_NETWORK));
    }

    #[test]
    fn stale_inventory_is_counted_for_any_pillar() {
        let mut stale = healthy_replicaset();
        stale.collected_at = now() - Duration::hours(DEFAULT_STALE_AFTER_HOURS + 2);

        let report = evaluate_kubernetes_replicaset_inventory(&[stale], Pillar::Cost, now());

        assert_eq!(report.stale_resources, 1);
        assert_eq!(report.findings[0].reason_code, REASON_INV_STALE_DATA);
    }

    #[test]
    fn healthy_replicaset_passes_claimed_pillars() {
        for pillar in [Pillar::Cost, Pillar::Resilience, Pillar::Security] {
            let report =
                evaluate_kubernetes_replicaset_inventory(&[healthy_replicaset()], pillar, now());

            assert_eq!(report.resources_evaluated, 1);
            assert_eq!(report.stale_resources, 0);
            assert!(report.findings.is_empty());
        }
    }
}

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

// Deterministic Kubernetes DaemonSet inventory evaluator for roadmap rows
// 02-KUBERNETES-DASHBOARD-00344/00351/00372.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::services::aws::inventory::types::{
    score_pillar, InventoryFinding, Pillar, PillarReport, Severity, COST_ALLOCATION_TAG_KEYS,
    DEFAULT_STALE_AFTER_HOURS,
};

pub const RESOURCE_TYPE: &str = "KubernetesDaemonSet";
pub const REASON_COST_OWNER_NOT_RECORDED: &str = "K8S_DAEMONSET_COST_OWNER_NOT_RECORDED";
pub const REASON_RES_NODES_UNAVAILABLE: &str = "K8S_DAEMONSET_RES_NODES_UNAVAILABLE";
pub const REASON_RES_READY_NODES_LAG: &str = "K8S_DAEMONSET_RES_READY_NODES_LAG";
pub const REASON_RES_UPDATED_NODES_LAG: &str = "K8S_DAEMONSET_RES_UPDATED_NODES_LAG";
pub const REASON_RES_MISSCHEDULED_PODS: &str = "K8S_DAEMONSET_RES_MISSCHEDULED_PODS";
pub const REASON_RES_GENERATION_NOT_OBSERVED: &str = "K8S_DAEMONSET_RES_GENERATION_NOT_OBSERVED";
pub const REASON_SEC_PRIVILEGED_CONTAINER: &str = "K8S_DAEMONSET_SEC_PRIVILEGED_CONTAINER";
pub const REASON_SEC_HOST_NETWORK: &str = "K8S_DAEMONSET_SEC_HOST_NETWORK";
pub const REASON_INV_STALE_DATA: &str = "K8S_DAEMONSET_INV_STALE_DATA";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DaemonSetContainerInventoryItem {
    pub name: String,
    pub image: Option<String>,
    pub privileged: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DaemonSetInventoryItem {
    pub cluster_id: String,
    pub namespace: String,
    pub name: String,
    pub desired_number_scheduled: i32,
    pub current_number_scheduled: i32,
    pub number_ready: i32,
    pub number_available: i32,
    pub number_unavailable: i32,
    pub number_misscheduled: i32,
    pub updated_number_scheduled: i32,
    pub generation: Option<i64>,
    pub observed_generation: Option<i64>,
    pub labels: BTreeMap<String, String>,
    pub annotations: BTreeMap<String, String>,
    pub selector: BTreeMap<String, String>,
    pub pod_template_labels: BTreeMap<String, String>,
    pub containers: Vec<DaemonSetContainerInventoryItem>,
    pub update_strategy_type: Option<String>,
    pub service_account_name: Option<String>,
    pub host_network: bool,
    pub created_at: Option<DateTime<Utc>>,
    pub collected_at: DateTime<Utc>,
}

pub fn evaluate_kubernetes_daemonset_inventory(
    daemonsets: &[DaemonSetInventoryItem],
    pillar: Pillar,
    now: DateTime<Utc>,
) -> PillarReport {
    let mut stale_resources = 0;
    let mut findings = Vec::new();

    for daemonset in daemonsets {
        if let Some(finding) = stale_finding(daemonset, pillar, now) {
            stale_resources += 1;
            findings.push(finding);
        }

        match pillar {
            Pillar::Cost => evaluate_cost(daemonset, pillar, &mut findings),
            Pillar::Resilience => evaluate_resilience(daemonset, pillar, &mut findings),
            Pillar::Security => evaluate_security(daemonset, pillar, &mut findings),
            _ => {}
        }
    }

    PillarReport {
        pillar,
        resources_evaluated: daemonsets.len(),
        stale_resources,
        score: score_pillar(&findings),
        findings,
    }
}

fn evaluate_cost(
    daemonset: &DaemonSetInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if has_any_metadata_key(&daemonset.labels, COST_ALLOCATION_TAG_KEYS)
        || has_any_metadata_key(&daemonset.annotations, COST_ALLOCATION_TAG_KEYS)
        || has_any_metadata_key(&daemonset.pod_template_labels, COST_ALLOCATION_TAG_KEYS)
    {
        return;
    }

    findings.push(finding(
        daemonset,
        pillar,
        REASON_COST_OWNER_NOT_RECORDED,
        Severity::Medium,
        format!(
            "Kubernetes DaemonSet {}/{} has no owner, team, project, or cost-center label or annotation",
            daemonset.namespace, daemonset.name
        ),
        json!({
            "cluster_id": daemonset.cluster_id,
            "namespace": daemonset.namespace,
            "daemonset": daemonset.name,
            "desired_number_scheduled": daemonset.desired_number_scheduled,
            "current_number_scheduled": daemonset.current_number_scheduled,
            "checked_keys": COST_ALLOCATION_TAG_KEYS,
            "checked_locations": ["labels", "annotations", "pod_template_labels"],
        }),
    ));
}

fn evaluate_resilience(
    daemonset: &DaemonSetInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if daemonset.desired_number_scheduled > 0
        && daemonset.number_available < daemonset.desired_number_scheduled
    {
        findings.push(finding(
            daemonset,
            pillar,
            REASON_RES_NODES_UNAVAILABLE,
            Severity::High,
            format!(
                "Kubernetes DaemonSet {}/{} has {}/{} available scheduled nodes",
                daemonset.namespace,
                daemonset.name,
                daemonset.number_available,
                daemonset.desired_number_scheduled
            ),
            json!({
                "cluster_id": daemonset.cluster_id,
                "namespace": daemonset.namespace,
                "daemonset": daemonset.name,
                "desired_number_scheduled": daemonset.desired_number_scheduled,
                "current_number_scheduled": daemonset.current_number_scheduled,
                "number_available": daemonset.number_available,
                "number_unavailable": daemonset.number_unavailable,
            }),
        ));
    }

    if daemonset.desired_number_scheduled > 0
        && daemonset.number_ready < daemonset.desired_number_scheduled
    {
        findings.push(finding(
            daemonset,
            pillar,
            REASON_RES_READY_NODES_LAG,
            Severity::High,
            format!(
                "Kubernetes DaemonSet {}/{} has {}/{} ready scheduled nodes",
                daemonset.namespace,
                daemonset.name,
                daemonset.number_ready,
                daemonset.desired_number_scheduled
            ),
            json!({
                "cluster_id": daemonset.cluster_id,
                "namespace": daemonset.namespace,
                "daemonset": daemonset.name,
                "desired_number_scheduled": daemonset.desired_number_scheduled,
                "number_ready": daemonset.number_ready,
                "number_available": daemonset.number_available,
            }),
        ));
    }

    if daemonset.desired_number_scheduled > 0
        && daemonset.updated_number_scheduled < daemonset.desired_number_scheduled
    {
        findings.push(finding(
            daemonset,
            pillar,
            REASON_RES_UPDATED_NODES_LAG,
            Severity::Medium,
            format!(
                "Kubernetes DaemonSet {}/{} has {}/{} updated scheduled nodes",
                daemonset.namespace,
                daemonset.name,
                daemonset.updated_number_scheduled,
                daemonset.desired_number_scheduled
            ),
            json!({
                "cluster_id": daemonset.cluster_id,
                "namespace": daemonset.namespace,
                "daemonset": daemonset.name,
                "desired_number_scheduled": daemonset.desired_number_scheduled,
                "updated_number_scheduled": daemonset.updated_number_scheduled,
                "update_strategy_type": daemonset.update_strategy_type,
            }),
        ));
    }

    if daemonset.number_misscheduled > 0 {
        findings.push(finding(
            daemonset,
            pillar,
            REASON_RES_MISSCHEDULED_PODS,
            Severity::Medium,
            format!(
                "Kubernetes DaemonSet {}/{} has {} misscheduled pods",
                daemonset.namespace, daemonset.name, daemonset.number_misscheduled
            ),
            json!({
                "cluster_id": daemonset.cluster_id,
                "namespace": daemonset.namespace,
                "daemonset": daemonset.name,
                "number_misscheduled": daemonset.number_misscheduled,
                "selector": daemonset.selector,
            }),
        ));
    }

    if let (Some(generation), Some(observed_generation)) =
        (daemonset.generation, daemonset.observed_generation)
    {
        if observed_generation < generation {
            findings.push(finding(
                daemonset,
                pillar,
                REASON_RES_GENERATION_NOT_OBSERVED,
                Severity::Medium,
                format!(
                    "Kubernetes DaemonSet {}/{} controller has not observed generation {}",
                    daemonset.namespace, daemonset.name, generation
                ),
                json!({
                    "cluster_id": daemonset.cluster_id,
                    "namespace": daemonset.namespace,
                    "daemonset": daemonset.name,
                    "generation": generation,
                    "observed_generation": observed_generation,
                }),
            ));
        }
    }
}

fn evaluate_security(
    daemonset: &DaemonSetInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    let privileged_containers = daemonset
        .containers
        .iter()
        .filter(|container| container.privileged == Some(true))
        .map(|container| container.name.clone())
        .collect::<Vec<_>>();
    if !privileged_containers.is_empty() {
        findings.push(finding(
            daemonset,
            pillar,
            REASON_SEC_PRIVILEGED_CONTAINER,
            Severity::High,
            format!(
                "Kubernetes DaemonSet {}/{} template has privileged containers",
                daemonset.namespace, daemonset.name
            ),
            json!({
                "cluster_id": daemonset.cluster_id,
                "namespace": daemonset.namespace,
                "daemonset": daemonset.name,
                "privileged_containers": privileged_containers,
                "service_account_name": daemonset.service_account_name,
            }),
        ));
    }

    if daemonset.host_network {
        findings.push(finding(
            daemonset,
            pillar,
            REASON_SEC_HOST_NETWORK,
            Severity::High,
            format!(
                "Kubernetes DaemonSet {}/{} template runs with hostNetwork enabled",
                daemonset.namespace, daemonset.name
            ),
            json!({
                "cluster_id": daemonset.cluster_id,
                "namespace": daemonset.namespace,
                "daemonset": daemonset.name,
                "host_network": daemonset.host_network,
            }),
        ));
    }
}

fn stale_finding(
    daemonset: &DaemonSetInventoryItem,
    pillar: Pillar,
    now: DateTime<Utc>,
) -> Option<InventoryFinding> {
    let age_hours = (now - daemonset.collected_at).num_hours();
    if age_hours <= DEFAULT_STALE_AFTER_HOURS {
        return None;
    }

    Some(finding(
        daemonset,
        pillar,
        REASON_INV_STALE_DATA,
        Severity::Medium,
        format!(
            "Inventory data for Kubernetes DaemonSet {}/{} is {} hours old (threshold {} hours)",
            daemonset.namespace, daemonset.name, age_hours, DEFAULT_STALE_AFTER_HOURS
        ),
        json!({
            "cluster_id": daemonset.cluster_id,
            "namespace": daemonset.namespace,
            "daemonset": daemonset.name,
            "collected_at": daemonset.collected_at,
            "age_hours": age_hours,
            "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
        }),
    ))
}

fn finding(
    daemonset: &DaemonSetInventoryItem,
    pillar: Pillar,
    reason_code: &str,
    severity: Severity,
    message: String,
    evidence: Value,
) -> InventoryFinding {
    InventoryFinding {
        resource_id: format!(
            "{}/{}/{}",
            daemonset.cluster_id, daemonset.namespace, daemonset.name
        ),
        arn: format!(
            "kubernetes://daemonset/{}/{}/{}",
            daemonset.cluster_id, daemonset.namespace, daemonset.name
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

    fn container(name: &str, privileged: Option<bool>) -> DaemonSetContainerInventoryItem {
        DaemonSetContainerInventoryItem {
            name: name.to_string(),
            image: Some("registry.local/agent:1.0".to_string()),
            privileged,
        }
    }

    fn daemonset(
        name: &str,
        metadata_labels: BTreeMap<String, String>,
        desired: i32,
        available: i32,
        ready: i32,
        updated: i32,
    ) -> DaemonSetInventoryItem {
        DaemonSetInventoryItem {
            cluster_id: "cluster-a".to_string(),
            namespace: "observability".to_string(),
            name: name.to_string(),
            desired_number_scheduled: desired,
            current_number_scheduled: desired,
            number_ready: ready,
            number_available: available,
            number_unavailable: desired.saturating_sub(available),
            number_misscheduled: 0,
            updated_number_scheduled: updated,
            generation: Some(4),
            observed_generation: Some(4),
            labels: metadata_labels,
            annotations: BTreeMap::new(),
            selector: labels(&[("app", "agent")]),
            pod_template_labels: labels(&[("app", "agent")]),
            containers: vec![container("agent", Some(false))],
            update_strategy_type: Some("RollingUpdate".to_string()),
            service_account_name: Some("agent".to_string()),
            host_network: false,
            created_at: Some(now() - Duration::hours(6)),
            collected_at: now(),
        }
    }

    fn healthy_daemonset() -> DaemonSetInventoryItem {
        daemonset("node-agent", labels(&[("team", "platform")]), 5, 5, 5, 5)
    }

    #[test]
    fn cost_flags_missing_owner_and_cost_allocation_labels() {
        let report = evaluate_kubernetes_daemonset_inventory(
            &[daemonset("untagged", BTreeMap::new(), 5, 5, 5, 5)],
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
    fn resilience_flags_unavailable_ready_update_misscheduled_and_generation_gaps() {
        let mut lagging = daemonset("node-agent", labels(&[("team", "platform")]), 6, 4, 3, 2);
        lagging.number_misscheduled = 1;
        lagging.generation = Some(9);
        lagging.observed_generation = Some(8);

        let report = evaluate_kubernetes_daemonset_inventory(&[lagging], Pillar::Resilience, now());
        let reason_codes = report
            .findings
            .iter()
            .map(|finding| finding.reason_code.as_str())
            .collect::<Vec<_>>();

        assert!(reason_codes.contains(&REASON_RES_NODES_UNAVAILABLE));
        assert!(reason_codes.contains(&REASON_RES_READY_NODES_LAG));
        assert!(reason_codes.contains(&REASON_RES_UPDATED_NODES_LAG));
        assert!(reason_codes.contains(&REASON_RES_MISSCHEDULED_PODS));
        assert!(reason_codes.contains(&REASON_RES_GENERATION_NOT_OBSERVED));
    }

    #[test]
    fn security_flags_privileged_template_and_host_network() {
        let mut exposed = healthy_daemonset();
        exposed.host_network = true;
        exposed.containers = vec![
            container("agent", Some(false)),
            container("collector", Some(true)),
        ];

        let report = evaluate_kubernetes_daemonset_inventory(&[exposed], Pillar::Security, now());
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
        let mut stale = healthy_daemonset();
        stale.collected_at = now() - Duration::hours(DEFAULT_STALE_AFTER_HOURS + 2);

        let report = evaluate_kubernetes_daemonset_inventory(&[stale], Pillar::Cost, now());

        assert_eq!(report.stale_resources, 1);
        assert_eq!(report.findings[0].reason_code, REASON_INV_STALE_DATA);
    }

    #[test]
    fn healthy_daemonset_passes_claimed_pillars() {
        for pillar in [Pillar::Cost, Pillar::Resilience, Pillar::Security] {
            let report =
                evaluate_kubernetes_daemonset_inventory(&[healthy_daemonset()], pillar, now());

            assert_eq!(report.resources_evaluated, 1);
            assert_eq!(report.stale_resources, 0);
            assert!(report.findings.is_empty());
        }
    }
}

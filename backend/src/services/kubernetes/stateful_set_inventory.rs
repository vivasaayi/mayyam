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

// Deterministic Kubernetes StatefulSet inventory evaluator for roadmap rows
// 02-KUBERNETES-DASHBOARD-00295/00302/00323.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::services::aws::inventory::types::{
    score_pillar, InventoryFinding, Pillar, PillarReport, Severity, COST_ALLOCATION_TAG_KEYS,
    DEFAULT_STALE_AFTER_HOURS,
};

pub const RESOURCE_TYPE: &str = "KubernetesStatefulSet";
pub const REASON_COST_OWNER_NOT_RECORDED: &str = "K8S_STATEFULSET_COST_OWNER_NOT_RECORDED";
pub const REASON_RES_REPLICAS_NOT_READY: &str = "K8S_STATEFULSET_RES_REPLICAS_NOT_READY";
pub const REASON_RES_UPDATED_REPLICAS_LAG: &str = "K8S_STATEFULSET_RES_UPDATED_REPLICAS_LAG";
pub const REASON_RES_GENERATION_NOT_OBSERVED: &str = "K8S_STATEFULSET_RES_GENERATION_NOT_OBSERVED";
pub const REASON_SEC_PRIVILEGED_CONTAINER: &str = "K8S_STATEFULSET_SEC_PRIVILEGED_CONTAINER";
pub const REASON_SEC_HOST_NETWORK: &str = "K8S_STATEFULSET_SEC_HOST_NETWORK";
pub const REASON_INV_STALE_DATA: &str = "K8S_STATEFULSET_INV_STALE_DATA";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatefulSetContainerInventoryItem {
    pub name: String,
    pub image: Option<String>,
    pub privileged: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatefulSetInventoryItem {
    pub cluster_id: String,
    pub namespace: String,
    pub name: String,
    pub service_name: Option<String>,
    pub desired_replicas: i32,
    pub current_replicas: i32,
    pub ready_replicas: i32,
    pub available_replicas: i32,
    pub updated_replicas: i32,
    pub generation: Option<i64>,
    pub observed_generation: Option<i64>,
    pub labels: BTreeMap<String, String>,
    pub annotations: BTreeMap<String, String>,
    pub selector: BTreeMap<String, String>,
    pub pod_template_labels: BTreeMap<String, String>,
    pub containers: Vec<StatefulSetContainerInventoryItem>,
    pub update_strategy_type: Option<String>,
    pub pod_management_policy: Option<String>,
    pub current_revision: Option<String>,
    pub update_revision: Option<String>,
    pub volume_claim_templates: Vec<String>,
    pub service_account_name: Option<String>,
    pub host_network: bool,
    pub created_at: Option<DateTime<Utc>>,
    pub collected_at: DateTime<Utc>,
}

pub fn evaluate_kubernetes_statefulset_inventory(
    statefulsets: &[StatefulSetInventoryItem],
    pillar: Pillar,
    now: DateTime<Utc>,
) -> PillarReport {
    let mut stale_resources = 0;
    let mut findings = Vec::new();

    for statefulset in statefulsets {
        if let Some(finding) = stale_finding(statefulset, pillar, now) {
            stale_resources += 1;
            findings.push(finding);
        }

        match pillar {
            Pillar::Cost => evaluate_cost(statefulset, pillar, &mut findings),
            Pillar::Resilience => evaluate_resilience(statefulset, pillar, &mut findings),
            Pillar::Security => evaluate_security(statefulset, pillar, &mut findings),
            _ => {}
        }
    }

    PillarReport {
        pillar,
        resources_evaluated: statefulsets.len(),
        stale_resources,
        score: score_pillar(&findings),
        findings,
    }
}

fn evaluate_cost(
    statefulset: &StatefulSetInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if has_any_metadata_key(&statefulset.labels, COST_ALLOCATION_TAG_KEYS)
        || has_any_metadata_key(&statefulset.annotations, COST_ALLOCATION_TAG_KEYS)
        || has_any_metadata_key(&statefulset.pod_template_labels, COST_ALLOCATION_TAG_KEYS)
    {
        return;
    }

    findings.push(finding(
        statefulset,
        pillar,
        REASON_COST_OWNER_NOT_RECORDED,
        Severity::Medium,
        format!(
            "Kubernetes StatefulSet {}/{} has no owner, team, project, or cost-center label or annotation",
            statefulset.namespace, statefulset.name
        ),
        json!({
            "cluster_id": statefulset.cluster_id,
            "namespace": statefulset.namespace,
            "statefulset": statefulset.name,
            "desired_replicas": statefulset.desired_replicas,
            "service_name": statefulset.service_name,
            "volume_claim_templates": statefulset.volume_claim_templates,
            "checked_keys": COST_ALLOCATION_TAG_KEYS,
            "checked_locations": ["labels", "annotations", "pod_template_labels"],
        }),
    ));
}

fn evaluate_resilience(
    statefulset: &StatefulSetInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if statefulset.desired_replicas > 0 && statefulset.ready_replicas < statefulset.desired_replicas
    {
        findings.push(finding(
            statefulset,
            pillar,
            REASON_RES_REPLICAS_NOT_READY,
            Severity::High,
            format!(
                "Kubernetes StatefulSet {}/{} has {}/{} ready replicas",
                statefulset.namespace,
                statefulset.name,
                statefulset.ready_replicas,
                statefulset.desired_replicas
            ),
            json!({
                "cluster_id": statefulset.cluster_id,
                "namespace": statefulset.namespace,
                "statefulset": statefulset.name,
                "desired_replicas": statefulset.desired_replicas,
                "current_replicas": statefulset.current_replicas,
                "ready_replicas": statefulset.ready_replicas,
                "available_replicas": statefulset.available_replicas,
                "pod_management_policy": statefulset.pod_management_policy,
            }),
        ));
    }

    if statefulset.desired_replicas > 0
        && statefulset.updated_replicas < statefulset.desired_replicas
    {
        findings.push(finding(
            statefulset,
            pillar,
            REASON_RES_UPDATED_REPLICAS_LAG,
            Severity::Medium,
            format!(
                "Kubernetes StatefulSet {}/{} has {}/{} updated replicas",
                statefulset.namespace,
                statefulset.name,
                statefulset.updated_replicas,
                statefulset.desired_replicas
            ),
            json!({
                "cluster_id": statefulset.cluster_id,
                "namespace": statefulset.namespace,
                "statefulset": statefulset.name,
                "desired_replicas": statefulset.desired_replicas,
                "updated_replicas": statefulset.updated_replicas,
                "current_revision": statefulset.current_revision,
                "update_revision": statefulset.update_revision,
                "update_strategy_type": statefulset.update_strategy_type,
            }),
        ));
    }

    if let (Some(generation), Some(observed_generation)) =
        (statefulset.generation, statefulset.observed_generation)
    {
        if observed_generation < generation {
            findings.push(finding(
                statefulset,
                pillar,
                REASON_RES_GENERATION_NOT_OBSERVED,
                Severity::Medium,
                format!(
                    "Kubernetes StatefulSet {}/{} controller has not observed generation {}",
                    statefulset.namespace, statefulset.name, generation
                ),
                json!({
                    "cluster_id": statefulset.cluster_id,
                    "namespace": statefulset.namespace,
                    "statefulset": statefulset.name,
                    "generation": generation,
                    "observed_generation": observed_generation,
                }),
            ));
        }
    }
}

fn evaluate_security(
    statefulset: &StatefulSetInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    let privileged_containers = statefulset
        .containers
        .iter()
        .filter(|container| container.privileged == Some(true))
        .map(|container| container.name.clone())
        .collect::<Vec<_>>();
    if !privileged_containers.is_empty() {
        findings.push(finding(
            statefulset,
            pillar,
            REASON_SEC_PRIVILEGED_CONTAINER,
            Severity::High,
            format!(
                "Kubernetes StatefulSet {}/{} template has privileged containers",
                statefulset.namespace, statefulset.name
            ),
            json!({
                "cluster_id": statefulset.cluster_id,
                "namespace": statefulset.namespace,
                "statefulset": statefulset.name,
                "privileged_containers": privileged_containers,
                "service_account_name": statefulset.service_account_name,
            }),
        ));
    }

    if statefulset.host_network {
        findings.push(finding(
            statefulset,
            pillar,
            REASON_SEC_HOST_NETWORK,
            Severity::High,
            format!(
                "Kubernetes StatefulSet {}/{} template runs with hostNetwork enabled",
                statefulset.namespace, statefulset.name
            ),
            json!({
                "cluster_id": statefulset.cluster_id,
                "namespace": statefulset.namespace,
                "statefulset": statefulset.name,
                "host_network": statefulset.host_network,
            }),
        ));
    }
}

fn stale_finding(
    statefulset: &StatefulSetInventoryItem,
    pillar: Pillar,
    now: DateTime<Utc>,
) -> Option<InventoryFinding> {
    let age_hours = (now - statefulset.collected_at).num_hours();
    if age_hours <= DEFAULT_STALE_AFTER_HOURS {
        return None;
    }

    Some(finding(
        statefulset,
        pillar,
        REASON_INV_STALE_DATA,
        Severity::Medium,
        format!(
            "Inventory data for Kubernetes StatefulSet {}/{} is {} hours old (threshold {} hours)",
            statefulset.namespace, statefulset.name, age_hours, DEFAULT_STALE_AFTER_HOURS
        ),
        json!({
            "cluster_id": statefulset.cluster_id,
            "namespace": statefulset.namespace,
            "statefulset": statefulset.name,
            "collected_at": statefulset.collected_at,
            "age_hours": age_hours,
            "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
        }),
    ))
}

fn finding(
    statefulset: &StatefulSetInventoryItem,
    pillar: Pillar,
    reason_code: &str,
    severity: Severity,
    message: String,
    evidence: Value,
) -> InventoryFinding {
    InventoryFinding {
        resource_id: format!(
            "{}/{}/{}",
            statefulset.cluster_id, statefulset.namespace, statefulset.name
        ),
        arn: format!(
            "kubernetes://statefulset/{}/{}/{}",
            statefulset.cluster_id, statefulset.namespace, statefulset.name
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

    fn container(name: &str, privileged: Option<bool>) -> StatefulSetContainerInventoryItem {
        StatefulSetContainerInventoryItem {
            name: name.to_string(),
            image: Some("registry.local/mysql:8.0".to_string()),
            privileged,
        }
    }

    fn statefulset(
        name: &str,
        metadata_labels: BTreeMap<String, String>,
        desired_replicas: i32,
        ready_replicas: i32,
        updated_replicas: i32,
    ) -> StatefulSetInventoryItem {
        StatefulSetInventoryItem {
            cluster_id: "cluster-a".to_string(),
            namespace: "database".to_string(),
            name: name.to_string(),
            service_name: Some("mysql".to_string()),
            desired_replicas,
            current_replicas: ready_replicas,
            ready_replicas,
            available_replicas: ready_replicas,
            updated_replicas,
            generation: Some(4),
            observed_generation: Some(4),
            labels: metadata_labels,
            annotations: BTreeMap::new(),
            selector: labels(&[("app", "mysql")]),
            pod_template_labels: labels(&[("app", "mysql")]),
            containers: vec![container("mysql", Some(false))],
            update_strategy_type: Some("RollingUpdate".to_string()),
            pod_management_policy: Some("OrderedReady".to_string()),
            current_revision: Some("mysql-1".to_string()),
            update_revision: Some("mysql-1".to_string()),
            volume_claim_templates: vec!["data".to_string()],
            service_account_name: Some("mysql".to_string()),
            host_network: false,
            created_at: Some(now() - Duration::hours(6)),
            collected_at: now(),
        }
    }

    fn healthy_statefulset() -> StatefulSetInventoryItem {
        statefulset("mysql", labels(&[("team", "database")]), 3, 3, 3)
    }

    #[test]
    fn cost_flags_missing_owner_and_cost_allocation_labels() {
        let report = evaluate_kubernetes_statefulset_inventory(
            &[statefulset("untagged", BTreeMap::new(), 2, 2, 2)],
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
    fn resilience_flags_ready_update_and_generation_gaps() {
        let mut lagging = statefulset("mysql", labels(&[("team", "database")]), 4, 2, 1);
        lagging.generation = Some(9);
        lagging.observed_generation = Some(8);
        lagging.current_revision = Some("mysql-1".to_string());
        lagging.update_revision = Some("mysql-2".to_string());

        let report =
            evaluate_kubernetes_statefulset_inventory(&[lagging], Pillar::Resilience, now());
        let reason_codes = report
            .findings
            .iter()
            .map(|finding| finding.reason_code.as_str())
            .collect::<Vec<_>>();

        assert!(reason_codes.contains(&REASON_RES_REPLICAS_NOT_READY));
        assert!(reason_codes.contains(&REASON_RES_UPDATED_REPLICAS_LAG));
        assert!(reason_codes.contains(&REASON_RES_GENERATION_NOT_OBSERVED));
    }

    #[test]
    fn security_flags_privileged_template_and_host_network() {
        let mut exposed = healthy_statefulset();
        exposed.host_network = true;
        exposed.containers = vec![
            container("mysql", Some(false)),
            container("backup", Some(true)),
        ];

        let report = evaluate_kubernetes_statefulset_inventory(&[exposed], Pillar::Security, now());
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
        let mut stale = healthy_statefulset();
        stale.collected_at = now() - Duration::hours(DEFAULT_STALE_AFTER_HOURS + 2);

        let report = evaluate_kubernetes_statefulset_inventory(&[stale], Pillar::Cost, now());

        assert_eq!(report.stale_resources, 1);
        assert_eq!(report.findings[0].reason_code, REASON_INV_STALE_DATA);
    }

    #[test]
    fn healthy_statefulset_passes_claimed_pillars() {
        for pillar in [Pillar::Cost, Pillar::Resilience, Pillar::Security] {
            let report =
                evaluate_kubernetes_statefulset_inventory(&[healthy_statefulset()], pillar, now());

            assert_eq!(report.resources_evaluated, 1);
            assert_eq!(report.stale_resources, 0);
            assert!(report.findings.is_empty());
        }
    }
}

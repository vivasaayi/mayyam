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

// Deterministic Kubernetes deployment inventory evaluator for roadmap rows
// 02-KUBERNETES-DASHBOARD-00197/00204/00225.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::services::aws::inventory::types::{
    score_pillar, InventoryFinding, Pillar, PillarReport, Severity, COST_ALLOCATION_TAG_KEYS,
    DEFAULT_STALE_AFTER_HOURS,
};

pub const RESOURCE_TYPE: &str = "KubernetesDeployment";
pub const REASON_COST_OWNER_NOT_RECORDED: &str = "K8S_DEPLOYMENT_COST_OWNER_NOT_RECORDED";
pub const REASON_RES_REPLICAS_UNAVAILABLE: &str = "K8S_DEPLOYMENT_RES_REPLICAS_UNAVAILABLE";
pub const REASON_RES_UPDATED_REPLICAS_LAG: &str = "K8S_DEPLOYMENT_RES_UPDATED_REPLICAS_LAG";
pub const REASON_SEC_PRIVILEGED_CONTAINER: &str = "K8S_DEPLOYMENT_SEC_PRIVILEGED_CONTAINER";
pub const REASON_SEC_HOST_NETWORK: &str = "K8S_DEPLOYMENT_SEC_HOST_NETWORK";
pub const REASON_INV_STALE_DATA: &str = "K8S_DEPLOYMENT_INV_STALE_DATA";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeploymentContainerInventoryItem {
    pub name: String,
    pub image: Option<String>,
    pub privileged: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeploymentInventoryItem {
    pub cluster_id: String,
    pub namespace: String,
    pub name: String,
    pub desired_replicas: i32,
    pub available_replicas: i32,
    pub updated_replicas: i32,
    pub ready_replicas: i32,
    pub unavailable_replicas: i32,
    pub labels: BTreeMap<String, String>,
    pub annotations: BTreeMap<String, String>,
    pub selector: BTreeMap<String, String>,
    pub pod_template_labels: BTreeMap<String, String>,
    pub containers: Vec<DeploymentContainerInventoryItem>,
    pub strategy_type: Option<String>,
    pub service_account_name: Option<String>,
    pub host_network: bool,
    pub paused: bool,
    pub created_at: Option<DateTime<Utc>>,
    pub collected_at: DateTime<Utc>,
}

pub fn evaluate_kubernetes_deployment_inventory(
    deployments: &[DeploymentInventoryItem],
    pillar: Pillar,
    now: DateTime<Utc>,
) -> PillarReport {
    let mut stale_resources = 0;
    let mut findings = Vec::new();

    for deployment in deployments {
        if let Some(finding) = stale_finding(deployment, pillar, now) {
            stale_resources += 1;
            findings.push(finding);
        }

        match pillar {
            Pillar::Cost => evaluate_cost(deployment, pillar, &mut findings),
            Pillar::Resilience => evaluate_resilience(deployment, pillar, &mut findings),
            Pillar::Security => evaluate_security(deployment, pillar, &mut findings),
            _ => {}
        }
    }

    PillarReport {
        pillar,
        resources_evaluated: deployments.len(),
        stale_resources,
        score: score_pillar(&findings),
        findings,
    }
}

fn evaluate_cost(
    deployment: &DeploymentInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if has_any_metadata_key(&deployment.labels, COST_ALLOCATION_TAG_KEYS)
        || has_any_metadata_key(&deployment.annotations, COST_ALLOCATION_TAG_KEYS)
        || has_any_metadata_key(&deployment.pod_template_labels, COST_ALLOCATION_TAG_KEYS)
    {
        return;
    }

    findings.push(finding(
        deployment,
        pillar,
        REASON_COST_OWNER_NOT_RECORDED,
        Severity::Medium,
        format!(
            "Kubernetes deployment {}/{} has no owner, team, project, or cost-center label or annotation",
            deployment.namespace, deployment.name
        ),
        json!({
            "cluster_id": deployment.cluster_id,
            "namespace": deployment.namespace,
            "deployment": deployment.name,
            "desired_replicas": deployment.desired_replicas,
            "checked_keys": COST_ALLOCATION_TAG_KEYS,
            "checked_locations": ["labels", "annotations", "pod_template_labels"],
        }),
    ));
}

fn evaluate_resilience(
    deployment: &DeploymentInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if deployment.desired_replicas > 0
        && deployment.available_replicas < deployment.desired_replicas
    {
        findings.push(finding(
            deployment,
            pillar,
            REASON_RES_REPLICAS_UNAVAILABLE,
            Severity::High,
            format!(
                "Kubernetes deployment {}/{} has {}/{} available replicas",
                deployment.namespace,
                deployment.name,
                deployment.available_replicas,
                deployment.desired_replicas
            ),
            json!({
                "cluster_id": deployment.cluster_id,
                "namespace": deployment.namespace,
                "deployment": deployment.name,
                "desired_replicas": deployment.desired_replicas,
                "available_replicas": deployment.available_replicas,
                "ready_replicas": deployment.ready_replicas,
                "unavailable_replicas": deployment.unavailable_replicas,
                "paused": deployment.paused,
            }),
        ));
    }

    if deployment.desired_replicas > 0 && deployment.updated_replicas < deployment.desired_replicas
    {
        findings.push(finding(
            deployment,
            pillar,
            REASON_RES_UPDATED_REPLICAS_LAG,
            Severity::Medium,
            format!(
                "Kubernetes deployment {}/{} has {}/{} updated replicas",
                deployment.namespace,
                deployment.name,
                deployment.updated_replicas,
                deployment.desired_replicas
            ),
            json!({
                "cluster_id": deployment.cluster_id,
                "namespace": deployment.namespace,
                "deployment": deployment.name,
                "desired_replicas": deployment.desired_replicas,
                "updated_replicas": deployment.updated_replicas,
                "strategy_type": deployment.strategy_type,
                "selector": deployment.selector,
            }),
        ));
    }
}

fn evaluate_security(
    deployment: &DeploymentInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    let privileged_containers = deployment
        .containers
        .iter()
        .filter(|container| container.privileged == Some(true))
        .map(|container| container.name.clone())
        .collect::<Vec<_>>();
    if !privileged_containers.is_empty() {
        findings.push(finding(
            deployment,
            pillar,
            REASON_SEC_PRIVILEGED_CONTAINER,
            Severity::High,
            format!(
                "Kubernetes deployment {}/{} template has privileged containers",
                deployment.namespace, deployment.name
            ),
            json!({
                "cluster_id": deployment.cluster_id,
                "namespace": deployment.namespace,
                "deployment": deployment.name,
                "privileged_containers": privileged_containers,
                "service_account_name": deployment.service_account_name,
            }),
        ));
    }

    if deployment.host_network {
        findings.push(finding(
            deployment,
            pillar,
            REASON_SEC_HOST_NETWORK,
            Severity::High,
            format!(
                "Kubernetes deployment {}/{} template runs with hostNetwork enabled",
                deployment.namespace, deployment.name
            ),
            json!({
                "cluster_id": deployment.cluster_id,
                "namespace": deployment.namespace,
                "deployment": deployment.name,
                "host_network": deployment.host_network,
            }),
        ));
    }
}

fn stale_finding(
    deployment: &DeploymentInventoryItem,
    pillar: Pillar,
    now: DateTime<Utc>,
) -> Option<InventoryFinding> {
    let age_hours = (now - deployment.collected_at).num_hours();
    if age_hours <= DEFAULT_STALE_AFTER_HOURS {
        return None;
    }

    Some(finding(
        deployment,
        pillar,
        REASON_INV_STALE_DATA,
        Severity::Medium,
        format!(
            "Inventory data for Kubernetes deployment {}/{} is {} hours old (threshold {} hours)",
            deployment.namespace, deployment.name, age_hours, DEFAULT_STALE_AFTER_HOURS
        ),
        json!({
            "cluster_id": deployment.cluster_id,
            "namespace": deployment.namespace,
            "deployment": deployment.name,
            "collected_at": deployment.collected_at,
            "age_hours": age_hours,
            "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
        }),
    ))
}

fn finding(
    deployment: &DeploymentInventoryItem,
    pillar: Pillar,
    reason_code: &str,
    severity: Severity,
    message: String,
    evidence: Value,
) -> InventoryFinding {
    InventoryFinding {
        resource_id: format!(
            "{}/{}/{}",
            deployment.cluster_id, deployment.namespace, deployment.name
        ),
        arn: format!(
            "kubernetes://deployment/{}/{}/{}",
            deployment.cluster_id, deployment.namespace, deployment.name
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

    fn container(name: &str, privileged: Option<bool>) -> DeploymentContainerInventoryItem {
        DeploymentContainerInventoryItem {
            name: name.to_string(),
            image: Some("registry.local/app:1.0.0".to_string()),
            privileged,
        }
    }

    fn deployment(
        name: &str,
        metadata_labels: BTreeMap<String, String>,
        desired_replicas: i32,
        available_replicas: i32,
        updated_replicas: i32,
        containers: Vec<DeploymentContainerInventoryItem>,
        host_network: bool,
        collected_at: DateTime<Utc>,
    ) -> DeploymentInventoryItem {
        DeploymentInventoryItem {
            cluster_id: "cluster-1".to_string(),
            namespace: "payments".to_string(),
            name: name.to_string(),
            desired_replicas,
            available_replicas,
            updated_replicas,
            ready_replicas: available_replicas,
            unavailable_replicas: desired_replicas.saturating_sub(available_replicas),
            labels: metadata_labels,
            annotations: BTreeMap::new(),
            selector: labels(&[("app", name)]),
            pod_template_labels: labels(&[("app", name)]),
            containers,
            strategy_type: Some("RollingUpdate".to_string()),
            service_account_name: Some("payments".to_string()),
            host_network,
            paused: false,
            created_at: Some(now() - Duration::days(3)),
            collected_at,
        }
    }

    fn healthy_deployment() -> DeploymentInventoryItem {
        deployment(
            "payments",
            labels(&[("owner", "platform"), ("cost-center", "cc-42")]),
            3,
            3,
            3,
            vec![container("app", Some(false))],
            false,
            now(),
        )
    }

    fn reason_codes(report: &PillarReport) -> Vec<&str> {
        report
            .findings
            .iter()
            .map(|finding| finding.reason_code.as_str())
            .collect()
    }

    #[test]
    fn cost_flags_missing_owner_and_cost_allocation_labels() {
        let report = evaluate_kubernetes_deployment_inventory(
            &[deployment(
                "unowned",
                BTreeMap::new(),
                2,
                2,
                2,
                vec![container("app", Some(false))],
                false,
                now(),
            )],
            Pillar::Cost,
            now(),
        );

        assert_eq!(report.resources_evaluated, 1);
        assert!(reason_codes(&report).contains(&REASON_COST_OWNER_NOT_RECORDED));
    }

    #[test]
    fn resilience_flags_unavailable_and_outdated_replicas() {
        let report = evaluate_kubernetes_deployment_inventory(
            &[deployment(
                "lagging",
                labels(&[("owner", "platform")]),
                4,
                2,
                1,
                vec![container("app", Some(false))],
                false,
                now(),
            )],
            Pillar::Resilience,
            now(),
        );
        let codes = reason_codes(&report);

        assert!(codes.contains(&REASON_RES_REPLICAS_UNAVAILABLE));
        assert!(codes.contains(&REASON_RES_UPDATED_REPLICAS_LAG));
    }

    #[test]
    fn security_flags_privileged_template_and_host_network() {
        let exposed = deployment(
            "privileged",
            labels(&[("owner", "platform")]),
            2,
            2,
            2,
            vec![container("app", Some(true))],
            true,
            now(),
        );

        let report = evaluate_kubernetes_deployment_inventory(&[exposed], Pillar::Security, now());
        let codes = reason_codes(&report);

        assert!(codes.contains(&REASON_SEC_PRIVILEGED_CONTAINER));
        assert!(codes.contains(&REASON_SEC_HOST_NETWORK));
    }

    #[test]
    fn stale_inventory_is_counted_for_any_pillar() {
        let stale = deployment(
            "stale",
            labels(&[("owner", "platform"), ("cost-center", "cc-42")]),
            1,
            1,
            1,
            vec![container("app", Some(false))],
            false,
            now() - Duration::hours(DEFAULT_STALE_AFTER_HOURS + 2),
        );

        let report = evaluate_kubernetes_deployment_inventory(&[stale], Pillar::Cost, now());
        assert_eq!(report.stale_resources, 1);
        assert!(reason_codes(&report).contains(&REASON_INV_STALE_DATA));
    }

    #[test]
    fn healthy_deployment_passes_claimed_pillars() {
        let deployment = healthy_deployment();

        for pillar in [Pillar::Cost, Pillar::Resilience, Pillar::Security] {
            let report = evaluate_kubernetes_deployment_inventory(
                std::slice::from_ref(&deployment),
                pillar,
                now(),
            );
            assert!(
                report.findings.is_empty(),
                "unexpected for {:?}: {:?}",
                pillar,
                report.findings
            );
            assert_eq!(report.score, 100);
        }
    }
}

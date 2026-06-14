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

// Deterministic Kubernetes Ingress inventory evaluator for roadmap rows
// 02-KUBERNETES-DASHBOARD-00540/00547/00568.

use std::collections::{BTreeMap, BTreeSet};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::services::aws::inventory::types::{
    score_pillar, InventoryFinding, Pillar, PillarReport, Severity, COST_ALLOCATION_TAG_KEYS,
    DEFAULT_STALE_AFTER_HOURS,
};

pub const RESOURCE_TYPE: &str = "KubernetesIngress";
pub const REASON_COST_OWNER_NOT_RECORDED: &str = "K8S_INGRESS_COST_OWNER_NOT_RECORDED";
pub const REASON_RES_CLASS_NOT_SET: &str = "K8S_INGRESS_RES_CLASS_NOT_SET";
pub const REASON_RES_LOAD_BALANCER_PENDING: &str = "K8S_INGRESS_RES_LOAD_BALANCER_PENDING";
pub const REASON_SEC_TLS_NOT_CONFIGURED: &str = "K8S_INGRESS_SEC_TLS_NOT_CONFIGURED";
pub const REASON_SEC_WILDCARD_HOST: &str = "K8S_INGRESS_SEC_WILDCARD_HOST";
pub const REASON_INV_STALE_DATA: &str = "K8S_INGRESS_INV_STALE_DATA";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IngressBackendInventoryItem {
    pub service_name: Option<String>,
    pub service_port: Option<String>,
    pub resource_api_group: Option<String>,
    pub resource_kind: Option<String>,
    pub resource_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IngressPathInventoryItem {
    pub host: Option<String>,
    pub path: Option<String>,
    pub path_type: String,
    pub backend: IngressBackendInventoryItem,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IngressTlsInventoryItem {
    pub hosts: Vec<String>,
    pub secret_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IngressLoadBalancerInventoryItem {
    pub ip: Option<String>,
    pub hostname: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IngressInventoryItem {
    pub cluster_id: String,
    pub namespace: String,
    pub name: String,
    pub ingress_class_name: Option<String>,
    pub legacy_class_annotation: Option<String>,
    pub labels: BTreeMap<String, String>,
    pub annotations: BTreeMap<String, String>,
    pub hosts: Vec<String>,
    pub paths: Vec<IngressPathInventoryItem>,
    pub tls: Vec<IngressTlsInventoryItem>,
    pub default_backend: Option<IngressBackendInventoryItem>,
    pub load_balancer_ingress: Vec<IngressLoadBalancerInventoryItem>,
    pub created_at: Option<DateTime<Utc>>,
    pub collected_at: DateTime<Utc>,
}

pub fn evaluate_kubernetes_ingress_inventory(
    ingresses: &[IngressInventoryItem],
    pillar: Pillar,
    now: DateTime<Utc>,
) -> PillarReport {
    let mut stale_resources = 0;
    let mut findings = Vec::new();

    for ingress in ingresses {
        if let Some(finding) = stale_finding(ingress, pillar, now) {
            stale_resources += 1;
            findings.push(finding);
        }

        match pillar {
            Pillar::Cost => evaluate_cost(ingress, pillar, &mut findings),
            Pillar::Resilience => evaluate_resilience(ingress, pillar, &mut findings),
            Pillar::Security => evaluate_security(ingress, pillar, &mut findings),
            _ => {}
        }
    }

    PillarReport {
        pillar,
        resources_evaluated: ingresses.len(),
        stale_resources,
        score: score_pillar(&findings),
        findings,
    }
}

fn evaluate_cost(
    ingress: &IngressInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if has_any_metadata_key(&ingress.labels, COST_ALLOCATION_TAG_KEYS)
        || has_any_metadata_key(&ingress.annotations, COST_ALLOCATION_TAG_KEYS)
    {
        return;
    }

    findings.push(finding(
        ingress,
        pillar,
        REASON_COST_OWNER_NOT_RECORDED,
        Severity::Medium,
        format!(
            "Kubernetes Ingress {}/{} has no owner, team, project, or cost-center label or annotation",
            ingress.namespace, ingress.name
        ),
        json!({
            "cluster_id": ingress.cluster_id,
            "namespace": ingress.namespace,
            "ingress": ingress.name,
            "ingress_class_name": ingress.ingress_class_name,
            "legacy_class_annotation": ingress.legacy_class_annotation,
            "hosts": ingress.hosts,
            "checked_keys": COST_ALLOCATION_TAG_KEYS,
            "checked_locations": ["labels", "annotations"],
        }),
    ));
}

fn evaluate_resilience(
    ingress: &IngressInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if ingress
        .ingress_class_name
        .as_deref()
        .map(str::trim)
        .unwrap_or("")
        .is_empty()
        && ingress
            .legacy_class_annotation
            .as_deref()
            .map(str::trim)
            .unwrap_or("")
            .is_empty()
    {
        findings.push(finding(
            ingress,
            pillar,
            REASON_RES_CLASS_NOT_SET,
            Severity::Medium,
            format!(
                "Kubernetes Ingress {}/{} has no ingressClassName or legacy class annotation",
                ingress.namespace, ingress.name
            ),
            json!({
                "cluster_id": ingress.cluster_id,
                "namespace": ingress.namespace,
                "ingress": ingress.name,
                "ingress_class_name": ingress.ingress_class_name,
                "legacy_class_annotation": ingress.legacy_class_annotation,
                "hosts": ingress.hosts,
            }),
        ));
    }

    if ingress.load_balancer_ingress.is_empty() {
        findings.push(finding(
            ingress,
            pillar,
            REASON_RES_LOAD_BALANCER_PENDING,
            Severity::High,
            format!(
                "Kubernetes Ingress {}/{} has no assigned load balancer ingress",
                ingress.namespace, ingress.name
            ),
            json!({
                "cluster_id": ingress.cluster_id,
                "namespace": ingress.namespace,
                "ingress": ingress.name,
                "ingress_class_name": ingress.ingress_class_name,
                "hosts": ingress.hosts,
                "load_balancer_ingress": ingress.load_balancer_ingress,
            }),
        ));
    }
}

fn evaluate_security(
    ingress: &IngressInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    let host_set = ingress_hosts(ingress);
    let tls_hosts = ingress
        .tls
        .iter()
        .flat_map(|tls| tls.hosts.iter())
        .filter(|host| !host.trim().is_empty())
        .cloned()
        .collect::<BTreeSet<_>>();
    let hosts_without_tls = host_set
        .iter()
        .filter(|host| !host_is_tls_covered(host, &tls_hosts))
        .cloned()
        .collect::<Vec<_>>();
    if !hosts_without_tls.is_empty() {
        findings.push(finding(
            ingress,
            pillar,
            REASON_SEC_TLS_NOT_CONFIGURED,
            Severity::High,
            format!(
                "Kubernetes Ingress {}/{} has hosts without TLS coverage",
                ingress.namespace, ingress.name
            ),
            json!({
                "cluster_id": ingress.cluster_id,
                "namespace": ingress.namespace,
                "ingress": ingress.name,
                "hosts_without_tls": hosts_without_tls,
                "tls_hosts": tls_hosts,
                "tls": ingress.tls,
            }),
        ));
    }

    let wildcard_hosts = host_set
        .iter()
        .filter(|host| host.trim_start().starts_with("*."))
        .cloned()
        .collect::<Vec<_>>();
    if !wildcard_hosts.is_empty() {
        findings.push(finding(
            ingress,
            pillar,
            REASON_SEC_WILDCARD_HOST,
            Severity::Medium,
            format!(
                "Kubernetes Ingress {}/{} accepts wildcard hosts",
                ingress.namespace, ingress.name
            ),
            json!({
                "cluster_id": ingress.cluster_id,
                "namespace": ingress.namespace,
                "ingress": ingress.name,
                "wildcard_hosts": wildcard_hosts,
                "tls_hosts": tls_hosts,
            }),
        ));
    }
}

fn stale_finding(
    ingress: &IngressInventoryItem,
    pillar: Pillar,
    now: DateTime<Utc>,
) -> Option<InventoryFinding> {
    let age_hours = (now - ingress.collected_at).num_hours();
    if age_hours <= DEFAULT_STALE_AFTER_HOURS {
        return None;
    }

    Some(finding(
        ingress,
        pillar,
        REASON_INV_STALE_DATA,
        Severity::Medium,
        format!(
            "Inventory data for Kubernetes Ingress {}/{} is {} hours old (threshold {} hours)",
            ingress.namespace, ingress.name, age_hours, DEFAULT_STALE_AFTER_HOURS
        ),
        json!({
            "cluster_id": ingress.cluster_id,
            "namespace": ingress.namespace,
            "ingress": ingress.name,
            "collected_at": ingress.collected_at,
            "age_hours": age_hours,
            "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
        }),
    ))
}

fn finding(
    ingress: &IngressInventoryItem,
    pillar: Pillar,
    reason_code: &str,
    severity: Severity,
    message: String,
    evidence: Value,
) -> InventoryFinding {
    InventoryFinding {
        resource_id: format!(
            "{}/{}/{}",
            ingress.cluster_id, ingress.namespace, ingress.name
        ),
        arn: format!(
            "kubernetes://ingress/{}/{}/{}",
            ingress.cluster_id, ingress.namespace, ingress.name
        ),
        pillar,
        reason_code: reason_code.to_string(),
        severity,
        message,
        evidence,
    }
}

fn ingress_hosts(ingress: &IngressInventoryItem) -> BTreeSet<String> {
    ingress
        .hosts
        .iter()
        .chain(ingress.paths.iter().filter_map(|path| path.host.as_ref()))
        .filter_map(|host| {
            let host = host.trim();
            if host.is_empty() {
                None
            } else {
                Some(host.to_string())
            }
        })
        .collect()
}

fn host_is_tls_covered(host: &str, tls_hosts: &BTreeSet<String>) -> bool {
    tls_hosts.contains(host)
        || tls_hosts.iter().any(|tls_host| {
            tls_host
                .strip_prefix("*.")
                .map(|suffix| host.ends_with(suffix))
                .unwrap_or(false)
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

    fn backend(service_name: &str) -> IngressBackendInventoryItem {
        IngressBackendInventoryItem {
            service_name: Some(service_name.to_string()),
            service_port: Some("443".to_string()),
            resource_api_group: None,
            resource_kind: None,
            resource_name: None,
        }
    }

    fn path(host: &str, service_name: &str) -> IngressPathInventoryItem {
        IngressPathInventoryItem {
            host: Some(host.to_string()),
            path: Some("/".to_string()),
            path_type: "Prefix".to_string(),
            backend: backend(service_name),
        }
    }

    fn tls(host: &str) -> IngressTlsInventoryItem {
        IngressTlsInventoryItem {
            hosts: vec![host.to_string()],
            secret_name: Some(format!("{host}-tls")),
        }
    }

    fn ingress(name: &str, metadata_labels: BTreeMap<String, String>) -> IngressInventoryItem {
        IngressInventoryItem {
            cluster_id: "cluster-a".to_string(),
            namespace: "apps".to_string(),
            name: name.to_string(),
            ingress_class_name: Some("nginx".to_string()),
            legacy_class_annotation: None,
            labels: metadata_labels,
            annotations: BTreeMap::new(),
            hosts: vec!["checkout.example.com".to_string()],
            paths: vec![path("checkout.example.com", "checkout")],
            tls: vec![tls("checkout.example.com")],
            default_backend: None,
            load_balancer_ingress: vec![IngressLoadBalancerInventoryItem {
                ip: Some("203.0.113.20".to_string()),
                hostname: None,
            }],
            created_at: Some(now() - Duration::days(7)),
            collected_at: now(),
        }
    }

    fn healthy_ingress() -> IngressInventoryItem {
        ingress("checkout", labels(&[("team", "payments")]))
    }

    #[test]
    fn cost_flags_missing_owner_and_cost_allocation_labels() {
        let report = evaluate_kubernetes_ingress_inventory(
            &[ingress("untagged", BTreeMap::new())],
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
    fn resilience_flags_missing_class_and_pending_load_balancer() {
        let mut pending = ingress("edge-api", labels(&[("team", "edge")]));
        pending.ingress_class_name = None;
        pending.legacy_class_annotation = None;
        pending.load_balancer_ingress = Vec::new();

        let report = evaluate_kubernetes_ingress_inventory(&[pending], Pillar::Resilience, now());
        let reason_codes = report
            .findings
            .iter()
            .map(|finding| finding.reason_code.as_str())
            .collect::<Vec<_>>();

        assert!(reason_codes.contains(&REASON_RES_CLASS_NOT_SET));
        assert!(reason_codes.contains(&REASON_RES_LOAD_BALANCER_PENDING));
    }

    #[test]
    fn security_flags_wildcard_hosts_without_tls() {
        let mut exposed = healthy_ingress();
        exposed.hosts = vec!["*.example.com".to_string()];
        exposed.paths = vec![path("*.example.com", "edge-api")];
        exposed.tls = Vec::new();

        let report = evaluate_kubernetes_ingress_inventory(&[exposed], Pillar::Security, now());
        let reason_codes = report
            .findings
            .iter()
            .map(|finding| finding.reason_code.as_str())
            .collect::<Vec<_>>();

        assert!(reason_codes.contains(&REASON_SEC_TLS_NOT_CONFIGURED));
        assert!(reason_codes.contains(&REASON_SEC_WILDCARD_HOST));
    }

    #[test]
    fn stale_inventory_is_counted_for_any_pillar() {
        let mut stale = healthy_ingress();
        stale.collected_at = now() - Duration::hours(DEFAULT_STALE_AFTER_HOURS + 2);

        let report = evaluate_kubernetes_ingress_inventory(&[stale], Pillar::Cost, now());

        assert_eq!(report.stale_resources, 1);
        assert_eq!(report.findings[0].reason_code, REASON_INV_STALE_DATA);
    }

    #[test]
    fn healthy_ingress_passes_claimed_pillars() {
        for pillar in [Pillar::Cost, Pillar::Resilience, Pillar::Security] {
            let report = evaluate_kubernetes_ingress_inventory(&[healthy_ingress()], pillar, now());

            assert_eq!(report.resources_evaluated, 1);
            assert_eq!(report.stale_resources, 0);
            assert!(report.findings.is_empty());
        }
    }
}

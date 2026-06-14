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

// Deterministic Kubernetes Service inventory evaluator for roadmap rows
// 02-KUBERNETES-DASHBOARD-00491/00498/00519.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::services::aws::inventory::types::{
    score_pillar, InventoryFinding, Pillar, PillarReport, Severity, COST_ALLOCATION_TAG_KEYS,
    DEFAULT_STALE_AFTER_HOURS,
};

pub const RESOURCE_TYPE: &str = "KubernetesService";
pub const REASON_COST_OWNER_NOT_RECORDED: &str = "K8S_SERVICE_COST_OWNER_NOT_RECORDED";
pub const REASON_RES_SELECTOR_NOT_SET: &str = "K8S_SERVICE_RES_SELECTOR_NOT_SET";
pub const REASON_RES_LOAD_BALANCER_PENDING: &str = "K8S_SERVICE_RES_LOAD_BALANCER_PENDING";
pub const REASON_SEC_PUBLIC_LOAD_BALANCER: &str = "K8S_SERVICE_SEC_PUBLIC_LOAD_BALANCER";
pub const REASON_SEC_NODE_PORT_EXPOSED: &str = "K8S_SERVICE_SEC_NODE_PORT_EXPOSED";
pub const REASON_INV_STALE_DATA: &str = "K8S_SERVICE_INV_STALE_DATA";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServicePortInventoryItem {
    pub name: Option<String>,
    pub port: i32,
    pub target_port: Option<String>,
    pub protocol: Option<String>,
    pub node_port: Option<i32>,
    pub app_protocol: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceLoadBalancerIngressInventoryItem {
    pub ip: Option<String>,
    pub hostname: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceInventoryItem {
    pub cluster_id: String,
    pub namespace: String,
    pub name: String,
    pub service_type: String,
    pub cluster_ip: Option<String>,
    pub cluster_ips: Vec<String>,
    pub external_ips: Vec<String>,
    pub load_balancer_ip: Option<String>,
    pub load_balancer_ingress: Vec<ServiceLoadBalancerIngressInventoryItem>,
    pub external_name: Option<String>,
    pub selector: BTreeMap<String, String>,
    pub labels: BTreeMap<String, String>,
    pub annotations: BTreeMap<String, String>,
    pub ports: Vec<ServicePortInventoryItem>,
    pub session_affinity: Option<String>,
    pub ip_families: Vec<String>,
    pub ip_family_policy: Option<String>,
    pub internal_traffic_policy: Option<String>,
    pub external_traffic_policy: Option<String>,
    pub allocate_load_balancer_node_ports: Option<bool>,
    pub publish_not_ready_addresses: bool,
    pub created_at: Option<DateTime<Utc>>,
    pub collected_at: DateTime<Utc>,
}

pub fn evaluate_kubernetes_service_inventory(
    services: &[ServiceInventoryItem],
    pillar: Pillar,
    now: DateTime<Utc>,
) -> PillarReport {
    let mut stale_resources = 0;
    let mut findings = Vec::new();

    for service in services {
        if let Some(finding) = stale_finding(service, pillar, now) {
            stale_resources += 1;
            findings.push(finding);
        }

        match pillar {
            Pillar::Cost => evaluate_cost(service, pillar, &mut findings),
            Pillar::Resilience => evaluate_resilience(service, pillar, &mut findings),
            Pillar::Security => evaluate_security(service, pillar, &mut findings),
            _ => {}
        }
    }

    PillarReport {
        pillar,
        resources_evaluated: services.len(),
        stale_resources,
        score: score_pillar(&findings),
        findings,
    }
}

fn evaluate_cost(
    service: &ServiceInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if has_any_metadata_key(&service.labels, COST_ALLOCATION_TAG_KEYS)
        || has_any_metadata_key(&service.annotations, COST_ALLOCATION_TAG_KEYS)
    {
        return;
    }

    findings.push(finding(
        service,
        pillar,
        REASON_COST_OWNER_NOT_RECORDED,
        Severity::Medium,
        format!(
            "Kubernetes Service {}/{} has no owner, team, project, or cost-center label or annotation",
            service.namespace, service.name
        ),
        json!({
            "cluster_id": service.cluster_id,
            "namespace": service.namespace,
            "service": service.name,
            "service_type": service.service_type,
            "ports": service.ports,
            "checked_keys": COST_ALLOCATION_TAG_KEYS,
            "checked_locations": ["labels", "annotations"],
        }),
    ));
}

fn evaluate_resilience(
    service: &ServiceInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if !service_type_is(service, "ExternalName") && service.selector.is_empty() {
        findings.push(finding(
            service,
            pillar,
            REASON_RES_SELECTOR_NOT_SET,
            Severity::Medium,
            format!(
                "Kubernetes Service {}/{} has no selector for Kubernetes-managed endpoints",
                service.namespace, service.name
            ),
            json!({
                "cluster_id": service.cluster_id,
                "namespace": service.namespace,
                "service": service.name,
                "service_type": service.service_type,
                "cluster_ip": service.cluster_ip,
                "ports": service.ports,
            }),
        ));
    }

    if service_type_is(service, "LoadBalancer")
        && service.load_balancer_ingress.is_empty()
        && service.load_balancer_ip.is_none()
    {
        findings.push(finding(
            service,
            pillar,
            REASON_RES_LOAD_BALANCER_PENDING,
            Severity::High,
            format!(
                "Kubernetes Service {}/{} is a LoadBalancer without assigned ingress",
                service.namespace, service.name
            ),
            json!({
                "cluster_id": service.cluster_id,
                "namespace": service.namespace,
                "service": service.name,
                "service_type": service.service_type,
                "load_balancer_ip": service.load_balancer_ip,
                "load_balancer_ingress": service.load_balancer_ingress,
                "external_traffic_policy": service.external_traffic_policy,
            }),
        ));
    }
}

fn evaluate_security(
    service: &ServiceInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if service_type_is(service, "LoadBalancer")
        && (!service.load_balancer_ingress.is_empty()
            || service.load_balancer_ip.is_some()
            || !service.external_ips.is_empty())
    {
        findings.push(finding(
            service,
            pillar,
            REASON_SEC_PUBLIC_LOAD_BALANCER,
            Severity::High,
            format!(
                "Kubernetes Service {}/{} exposes a public LoadBalancer entrypoint",
                service.namespace, service.name
            ),
            json!({
                "cluster_id": service.cluster_id,
                "namespace": service.namespace,
                "service": service.name,
                "service_type": service.service_type,
                "load_balancer_ip": service.load_balancer_ip,
                "load_balancer_ingress": service.load_balancer_ingress,
                "external_ips": service.external_ips,
                "load_balancer_source_ranges": service.annotations.get("service.beta.kubernetes.io/load-balancer-source-ranges"),
            }),
        ));
    }

    let node_ports = service
        .ports
        .iter()
        .filter_map(|port| port.node_port.map(|node_port| (port.port, node_port)))
        .collect::<Vec<_>>();
    if !node_ports.is_empty() || service_type_is(service, "NodePort") {
        findings.push(finding(
            service,
            pillar,
            REASON_SEC_NODE_PORT_EXPOSED,
            Severity::Medium,
            format!(
                "Kubernetes Service {}/{} exposes node ports",
                service.namespace, service.name
            ),
            json!({
                "cluster_id": service.cluster_id,
                "namespace": service.namespace,
                "service": service.name,
                "service_type": service.service_type,
                "node_ports": node_ports,
                "external_traffic_policy": service.external_traffic_policy,
            }),
        ));
    }
}

fn stale_finding(
    service: &ServiceInventoryItem,
    pillar: Pillar,
    now: DateTime<Utc>,
) -> Option<InventoryFinding> {
    let age_hours = (now - service.collected_at).num_hours();
    if age_hours <= DEFAULT_STALE_AFTER_HOURS {
        return None;
    }

    Some(finding(
        service,
        pillar,
        REASON_INV_STALE_DATA,
        Severity::Medium,
        format!(
            "Inventory data for Kubernetes Service {}/{} is {} hours old (threshold {} hours)",
            service.namespace, service.name, age_hours, DEFAULT_STALE_AFTER_HOURS
        ),
        json!({
            "cluster_id": service.cluster_id,
            "namespace": service.namespace,
            "service": service.name,
            "collected_at": service.collected_at,
            "age_hours": age_hours,
            "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
        }),
    ))
}

fn finding(
    service: &ServiceInventoryItem,
    pillar: Pillar,
    reason_code: &str,
    severity: Severity,
    message: String,
    evidence: Value,
) -> InventoryFinding {
    InventoryFinding {
        resource_id: format!(
            "{}/{}/{}",
            service.cluster_id, service.namespace, service.name
        ),
        arn: format!(
            "kubernetes://service/{}/{}/{}",
            service.cluster_id, service.namespace, service.name
        ),
        pillar,
        reason_code: reason_code.to_string(),
        severity,
        message,
        evidence,
    }
}

fn service_type_is(service: &ServiceInventoryItem, expected: &str) -> bool {
    service.service_type.eq_ignore_ascii_case(expected)
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

    fn port(name: &str, port: i32, node_port: Option<i32>) -> ServicePortInventoryItem {
        ServicePortInventoryItem {
            name: Some(name.to_string()),
            port,
            target_port: Some(port.to_string()),
            protocol: Some("TCP".to_string()),
            node_port,
            app_protocol: None,
        }
    }

    fn ingress(ip: &str) -> ServiceLoadBalancerIngressInventoryItem {
        ServiceLoadBalancerIngressInventoryItem {
            ip: Some(ip.to_string()),
            hostname: None,
        }
    }

    fn service(
        name: &str,
        service_type: &str,
        metadata_labels: BTreeMap<String, String>,
    ) -> ServiceInventoryItem {
        ServiceInventoryItem {
            cluster_id: "cluster-a".to_string(),
            namespace: "apps".to_string(),
            name: name.to_string(),
            service_type: service_type.to_string(),
            cluster_ip: Some("10.96.1.10".to_string()),
            cluster_ips: vec!["10.96.1.10".to_string()],
            external_ips: Vec::new(),
            load_balancer_ip: None,
            load_balancer_ingress: Vec::new(),
            external_name: None,
            selector: labels(&[("app", name)]),
            labels: metadata_labels,
            annotations: BTreeMap::new(),
            ports: vec![port("https", 443, None)],
            session_affinity: Some("None".to_string()),
            ip_families: vec!["IPv4".to_string()],
            ip_family_policy: Some("SingleStack".to_string()),
            internal_traffic_policy: Some("Cluster".to_string()),
            external_traffic_policy: None,
            allocate_load_balancer_node_ports: None,
            publish_not_ready_addresses: false,
            created_at: Some(now() - Duration::days(7)),
            collected_at: now(),
        }
    }

    fn healthy_service() -> ServiceInventoryItem {
        service("checkout", "ClusterIP", labels(&[("team", "payments")]))
    }

    #[test]
    fn cost_flags_missing_owner_and_cost_allocation_labels() {
        let report = evaluate_kubernetes_service_inventory(
            &[service("untagged", "ClusterIP", BTreeMap::new())],
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
    fn resilience_flags_selectorless_and_pending_load_balancer_services() {
        let mut pending = service("edge-api", "LoadBalancer", labels(&[("team", "edge")]));
        pending.selector = BTreeMap::new();

        let report = evaluate_kubernetes_service_inventory(&[pending], Pillar::Resilience, now());
        let reason_codes = report
            .findings
            .iter()
            .map(|finding| finding.reason_code.as_str())
            .collect::<Vec<_>>();

        assert!(reason_codes.contains(&REASON_RES_SELECTOR_NOT_SET));
        assert!(reason_codes.contains(&REASON_RES_LOAD_BALANCER_PENDING));
    }

    #[test]
    fn security_flags_public_load_balancer_and_node_port_services() {
        let mut exposed = healthy_service();
        exposed.service_type = "LoadBalancer".to_string();
        exposed.external_ips = vec!["203.0.113.10".to_string()];
        exposed.load_balancer_ingress = vec![ingress("203.0.113.11")];
        exposed.ports = vec![port("https", 443, Some(32000))];

        let report = evaluate_kubernetes_service_inventory(&[exposed], Pillar::Security, now());
        let reason_codes = report
            .findings
            .iter()
            .map(|finding| finding.reason_code.as_str())
            .collect::<Vec<_>>();

        assert!(reason_codes.contains(&REASON_SEC_PUBLIC_LOAD_BALANCER));
        assert!(reason_codes.contains(&REASON_SEC_NODE_PORT_EXPOSED));
    }

    #[test]
    fn stale_inventory_is_counted_for_any_pillar() {
        let mut stale = healthy_service();
        stale.collected_at = now() - Duration::hours(DEFAULT_STALE_AFTER_HOURS + 2);

        let report = evaluate_kubernetes_service_inventory(&[stale], Pillar::Cost, now());

        assert_eq!(report.stale_resources, 1);
        assert_eq!(report.findings[0].reason_code, REASON_INV_STALE_DATA);
    }

    #[test]
    fn healthy_service_passes_claimed_pillars() {
        for pillar in [Pillar::Cost, Pillar::Resilience, Pillar::Security] {
            let report = evaluate_kubernetes_service_inventory(&[healthy_service()], pillar, now());

            assert_eq!(report.resources_evaluated, 1);
            assert_eq!(report.stale_resources, 0);
            assert!(report.findings.is_empty());
        }
    }
}

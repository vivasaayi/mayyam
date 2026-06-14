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

// Deterministic Kubernetes Endpoints inventory evaluator for roadmap rows
// 02-KUBERNETES-DASHBOARD-00638/00645/00666.

use std::collections::BTreeMap;
use std::net::IpAddr;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::services::aws::inventory::types::{
    score_pillar, InventoryFinding, Pillar, PillarReport, Severity, COST_ALLOCATION_TAG_KEYS,
    DEFAULT_STALE_AFTER_HOURS,
};

pub const RESOURCE_TYPE: &str = "KubernetesEndpoints";
pub const REASON_COST_OWNER_NOT_RECORDED: &str = "K8S_ENDPOINTS_COST_OWNER_NOT_RECORDED";
pub const REASON_RES_NO_READY_ADDRESSES: &str = "K8S_ENDPOINTS_RES_NO_READY_ADDRESSES";
pub const REASON_RES_PORTS_NOT_DEFINED: &str = "K8S_ENDPOINTS_RES_PORTS_NOT_DEFINED";
pub const REASON_SEC_UNMANAGED_BACKEND: &str = "K8S_ENDPOINTS_SEC_UNMANAGED_BACKEND";
pub const REASON_SEC_FQDN_ADDRESS: &str = "K8S_ENDPOINTS_SEC_FQDN_ADDRESS";
pub const REASON_INV_STALE_DATA: &str = "K8S_ENDPOINTS_INV_STALE_DATA";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EndpointPortInventoryItem {
    pub name: Option<String>,
    pub port: Option<i32>,
    pub protocol: Option<String>,
    pub app_protocol: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EndpointAddressInventoryItem {
    pub address: String,
    pub hostname: Option<String>,
    pub node_name: Option<String>,
    pub target_kind: Option<String>,
    pub target_namespace: Option<String>,
    pub target_name: Option<String>,
    pub ready: Option<bool>,
    pub serving: Option<bool>,
    pub terminating: Option<bool>,
    pub zone: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EndpointsInventoryItem {
    pub cluster_id: String,
    pub namespace: String,
    pub name: String,
    pub source: String,
    pub service_name: Option<String>,
    pub address_type: Option<String>,
    pub labels: BTreeMap<String, String>,
    pub annotations: BTreeMap<String, String>,
    pub ports: Vec<EndpointPortInventoryItem>,
    pub ready_addresses: Vec<EndpointAddressInventoryItem>,
    pub not_ready_addresses: Vec<EndpointAddressInventoryItem>,
    pub created_at: Option<DateTime<Utc>>,
    pub collected_at: DateTime<Utc>,
}

pub fn evaluate_kubernetes_endpoints_inventory(
    endpoints: &[EndpointsInventoryItem],
    pillar: Pillar,
    now: DateTime<Utc>,
) -> PillarReport {
    let mut stale_resources = 0;
    let mut findings = Vec::new();

    for endpoint in endpoints {
        if let Some(finding) = stale_finding(endpoint, pillar, now) {
            stale_resources += 1;
            findings.push(finding);
        }

        match pillar {
            Pillar::Cost => evaluate_cost(endpoint, pillar, &mut findings),
            Pillar::Resilience => evaluate_resilience(endpoint, pillar, &mut findings),
            Pillar::Security => evaluate_security(endpoint, pillar, &mut findings),
            _ => {}
        }
    }

    PillarReport {
        pillar,
        resources_evaluated: endpoints.len(),
        stale_resources,
        score: score_pillar(&findings),
        findings,
    }
}

fn evaluate_cost(
    endpoint: &EndpointsInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if has_any_metadata_key(&endpoint.labels, COST_ALLOCATION_TAG_KEYS)
        || has_any_metadata_key(&endpoint.annotations, COST_ALLOCATION_TAG_KEYS)
    {
        return;
    }

    findings.push(finding(
        endpoint,
        pillar,
        REASON_COST_OWNER_NOT_RECORDED,
        Severity::Medium,
        format!(
            "Kubernetes Endpoints {}/{} has no owner, team, project, or cost-center label or annotation",
            endpoint.namespace, endpoint.name
        ),
        json!({
            "cluster_id": endpoint.cluster_id,
            "namespace": endpoint.namespace,
            "name": endpoint.name,
            "source": endpoint.source,
            "service_name": endpoint.service_name,
            "ready_address_count": endpoint.ready_addresses.len(),
            "not_ready_address_count": endpoint.not_ready_addresses.len(),
            "ports": endpoint.ports,
            "checked_keys": COST_ALLOCATION_TAG_KEYS,
            "checked_locations": ["labels", "annotations"],
        }),
    ));
}

fn evaluate_resilience(
    endpoint: &EndpointsInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if endpoint.ready_addresses.is_empty() {
        findings.push(finding(
            endpoint,
            pillar,
            REASON_RES_NO_READY_ADDRESSES,
            Severity::High,
            format!(
                "Kubernetes Endpoints {}/{} has no ready backend addresses",
                endpoint.namespace, endpoint.name
            ),
            json!({
                "cluster_id": endpoint.cluster_id,
                "namespace": endpoint.namespace,
                "name": endpoint.name,
                "source": endpoint.source,
                "service_name": endpoint.service_name,
                "not_ready_addresses": endpoint.not_ready_addresses,
                "address_type": endpoint.address_type,
            }),
        ));
    }

    if endpoint.ports.is_empty() || endpoint.ports.iter().all(|port| port.port.is_none()) {
        findings.push(finding(
            endpoint,
            pillar,
            REASON_RES_PORTS_NOT_DEFINED,
            Severity::Medium,
            format!(
                "Kubernetes Endpoints {}/{} has no concrete endpoint ports",
                endpoint.namespace, endpoint.name
            ),
            json!({
                "cluster_id": endpoint.cluster_id,
                "namespace": endpoint.namespace,
                "name": endpoint.name,
                "source": endpoint.source,
                "service_name": endpoint.service_name,
                "ports": endpoint.ports,
            }),
        ));
    }
}

fn evaluate_security(
    endpoint: &EndpointsInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    let managed_ready_count = endpoint
        .ready_addresses
        .iter()
        .filter(|address| {
            address
                .target_kind
                .as_deref()
                .map(|kind| kind.eq_ignore_ascii_case("Pod"))
                .unwrap_or(false)
                && address
                    .target_name
                    .as_deref()
                    .map(|name| !name.trim().is_empty())
                    .unwrap_or(false)
        })
        .count();
    let unmanaged_addresses = endpoint
        .ready_addresses
        .iter()
        .filter(|address| {
            !address
                .target_kind
                .as_deref()
                .map(|kind| kind.eq_ignore_ascii_case("Pod"))
                .unwrap_or(false)
                || address
                    .target_name
                    .as_deref()
                    .map(|name| name.trim().is_empty())
                    .unwrap_or(true)
        })
        .cloned()
        .collect::<Vec<_>>();

    if !unmanaged_addresses.is_empty() {
        findings.push(finding(
            endpoint,
            pillar,
            REASON_SEC_UNMANAGED_BACKEND,
            Severity::Medium,
            format!(
                "Kubernetes Endpoints {}/{} includes ready backends without Pod target references",
                endpoint.namespace, endpoint.name
            ),
            json!({
                "cluster_id": endpoint.cluster_id,
                "namespace": endpoint.namespace,
                "name": endpoint.name,
                "source": endpoint.source,
                "service_name": endpoint.service_name,
                "managed_ready_count": managed_ready_count,
                "unmanaged_addresses": unmanaged_addresses,
            }),
        ));
    }

    let fqdn_addresses = all_addresses(endpoint)
        .into_iter()
        .filter(|address| {
            endpoint
                .address_type
                .as_deref()
                .map(|address_type| address_type.eq_ignore_ascii_case("FQDN"))
                .unwrap_or(false)
                || address.address.parse::<IpAddr>().is_err()
        })
        .collect::<Vec<_>>();
    if !fqdn_addresses.is_empty() {
        findings.push(finding(
            endpoint,
            pillar,
            REASON_SEC_FQDN_ADDRESS,
            Severity::Medium,
            format!(
                "Kubernetes Endpoints {}/{} routes to FQDN-style backend addresses",
                endpoint.namespace, endpoint.name
            ),
            json!({
                "cluster_id": endpoint.cluster_id,
                "namespace": endpoint.namespace,
                "name": endpoint.name,
                "source": endpoint.source,
                "service_name": endpoint.service_name,
                "address_type": endpoint.address_type,
                "fqdn_addresses": fqdn_addresses,
            }),
        ));
    }
}

fn stale_finding(
    endpoint: &EndpointsInventoryItem,
    pillar: Pillar,
    now: DateTime<Utc>,
) -> Option<InventoryFinding> {
    let age_hours = (now - endpoint.collected_at).num_hours();
    if age_hours <= DEFAULT_STALE_AFTER_HOURS {
        return None;
    }

    Some(finding(
        endpoint,
        pillar,
        REASON_INV_STALE_DATA,
        Severity::Medium,
        format!(
            "Inventory data for Kubernetes Endpoints {}/{} is {} hours old (threshold {} hours)",
            endpoint.namespace, endpoint.name, age_hours, DEFAULT_STALE_AFTER_HOURS
        ),
        json!({
            "cluster_id": endpoint.cluster_id,
            "namespace": endpoint.namespace,
            "name": endpoint.name,
            "source": endpoint.source,
            "service_name": endpoint.service_name,
            "collected_at": endpoint.collected_at,
            "age_hours": age_hours,
            "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
        }),
    ))
}

fn finding(
    endpoint: &EndpointsInventoryItem,
    pillar: Pillar,
    reason_code: &str,
    severity: Severity,
    message: String,
    evidence: Value,
) -> InventoryFinding {
    InventoryFinding {
        resource_id: format!(
            "{}/{}/{}/{}",
            endpoint.cluster_id, endpoint.namespace, endpoint.source, endpoint.name
        ),
        arn: format!(
            "kubernetes://endpoints/{}/{}/{}/{}",
            endpoint.cluster_id, endpoint.namespace, endpoint.source, endpoint.name
        ),
        pillar,
        reason_code: reason_code.to_string(),
        severity,
        message,
        evidence,
    }
}

fn all_addresses(endpoint: &EndpointsInventoryItem) -> Vec<EndpointAddressInventoryItem> {
    endpoint
        .ready_addresses
        .iter()
        .chain(endpoint.not_ready_addresses.iter())
        .cloned()
        .collect()
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

    fn port(name: &str, port: i32) -> EndpointPortInventoryItem {
        EndpointPortInventoryItem {
            name: Some(name.to_string()),
            port: Some(port),
            protocol: Some("TCP".to_string()),
            app_protocol: None,
        }
    }

    fn address(address: &str, target_kind: Option<&str>) -> EndpointAddressInventoryItem {
        EndpointAddressInventoryItem {
            address: address.to_string(),
            hostname: None,
            node_name: Some("node-a".to_string()),
            target_kind: target_kind.map(str::to_string),
            target_namespace: Some("apps".to_string()),
            target_name: target_kind.map(|_| "checkout-abc123".to_string()),
            ready: Some(true),
            serving: Some(true),
            terminating: Some(false),
            zone: Some("us-east-1a".to_string()),
        }
    }

    fn endpoints(name: &str, metadata_labels: BTreeMap<String, String>) -> EndpointsInventoryItem {
        EndpointsInventoryItem {
            cluster_id: "cluster-a".to_string(),
            namespace: "apps".to_string(),
            name: name.to_string(),
            source: "EndpointSlice".to_string(),
            service_name: Some(name.to_string()),
            address_type: Some("IPv4".to_string()),
            labels: metadata_labels,
            annotations: BTreeMap::new(),
            ports: vec![port("https", 443)],
            ready_addresses: vec![address("10.42.0.12", Some("Pod"))],
            not_ready_addresses: Vec::new(),
            created_at: Some(now() - Duration::days(3)),
            collected_at: now(),
        }
    }

    fn healthy_endpoints() -> EndpointsInventoryItem {
        endpoints("checkout", labels(&[("team", "payments")]))
    }

    #[test]
    fn cost_flags_missing_owner_and_cost_allocation_labels() {
        let report = evaluate_kubernetes_endpoints_inventory(
            &[endpoints("untagged", BTreeMap::new())],
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
    fn resilience_flags_empty_ready_addresses_and_missing_ports() {
        let mut empty = healthy_endpoints();
        empty.ready_addresses = Vec::new();
        empty.ports = Vec::new();

        let report = evaluate_kubernetes_endpoints_inventory(&[empty], Pillar::Resilience, now());
        let reason_codes = report
            .findings
            .iter()
            .map(|finding| finding.reason_code.as_str())
            .collect::<Vec<_>>();

        assert!(reason_codes.contains(&REASON_RES_NO_READY_ADDRESSES));
        assert!(reason_codes.contains(&REASON_RES_PORTS_NOT_DEFINED));
    }

    #[test]
    fn security_flags_unmanaged_and_fqdn_backends() {
        let mut exposed = healthy_endpoints();
        exposed.address_type = Some("FQDN".to_string());
        exposed.ready_addresses = vec![address("db.internal.example.com", None)];

        let report = evaluate_kubernetes_endpoints_inventory(&[exposed], Pillar::Security, now());
        let reason_codes = report
            .findings
            .iter()
            .map(|finding| finding.reason_code.as_str())
            .collect::<Vec<_>>();

        assert!(reason_codes.contains(&REASON_SEC_UNMANAGED_BACKEND));
        assert!(reason_codes.contains(&REASON_SEC_FQDN_ADDRESS));
    }

    #[test]
    fn stale_inventory_is_counted_for_any_pillar() {
        let mut stale = healthy_endpoints();
        stale.collected_at = now() - Duration::hours(DEFAULT_STALE_AFTER_HOURS + 2);

        let report = evaluate_kubernetes_endpoints_inventory(&[stale], Pillar::Cost, now());

        assert_eq!(report.stale_resources, 1);
        assert_eq!(report.findings[0].reason_code, REASON_INV_STALE_DATA);
    }

    #[test]
    fn healthy_endpoints_pass_claimed_pillars() {
        for pillar in [Pillar::Cost, Pillar::Resilience, Pillar::Security] {
            let report =
                evaluate_kubernetes_endpoints_inventory(&[healthy_endpoints()], pillar, now());

            assert_eq!(report.resources_evaluated, 1);
            assert_eq!(report.stale_resources, 0);
            assert!(report.findings.is_empty());
        }
    }
}

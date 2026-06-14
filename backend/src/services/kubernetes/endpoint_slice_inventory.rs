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

// Deterministic Kubernetes EndpointSlice inventory evaluator for roadmap rows
// 02-KUBERNETES-DASHBOARD-00687/00694/00715.

use std::collections::BTreeMap;
use std::net::IpAddr;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::services::aws::inventory::types::{
    score_pillar, InventoryFinding, Pillar, PillarReport, Severity, COST_ALLOCATION_TAG_KEYS,
    DEFAULT_STALE_AFTER_HOURS,
};
use crate::services::kubernetes::endpoints_inventory::{
    EndpointAddressInventoryItem, EndpointPortInventoryItem,
};

pub const RESOURCE_TYPE: &str = "KubernetesEndpointSlice";
pub const REASON_COST_OWNER_NOT_RECORDED: &str = "K8S_ENDPOINT_SLICE_COST_OWNER_NOT_RECORDED";
pub const REASON_RES_NO_READY_ADDRESSES: &str = "K8S_ENDPOINT_SLICE_RES_NO_READY_ADDRESSES";
pub const REASON_RES_PORTS_NOT_DEFINED: &str = "K8S_ENDPOINT_SLICE_RES_PORTS_NOT_DEFINED";
pub const REASON_SEC_UNMANAGED_BACKEND: &str = "K8S_ENDPOINT_SLICE_SEC_UNMANAGED_BACKEND";
pub const REASON_SEC_FQDN_ADDRESS: &str = "K8S_ENDPOINT_SLICE_SEC_FQDN_ADDRESS";
pub const REASON_INV_STALE_DATA: &str = "K8S_ENDPOINT_SLICE_INV_STALE_DATA";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EndpointSliceInventoryItem {
    pub cluster_id: String,
    pub namespace: String,
    pub name: String,
    pub service_name: Option<String>,
    pub address_type: String,
    pub labels: BTreeMap<String, String>,
    pub annotations: BTreeMap<String, String>,
    pub ports: Vec<EndpointPortInventoryItem>,
    pub ready_addresses: Vec<EndpointAddressInventoryItem>,
    pub not_ready_addresses: Vec<EndpointAddressInventoryItem>,
    pub created_at: Option<DateTime<Utc>>,
    pub collected_at: DateTime<Utc>,
}

pub fn evaluate_kubernetes_endpoint_slice_inventory(
    endpoint_slices: &[EndpointSliceInventoryItem],
    pillar: Pillar,
    now: DateTime<Utc>,
) -> PillarReport {
    let mut stale_resources = 0;
    let mut findings = Vec::new();

    for endpoint_slice in endpoint_slices {
        if let Some(finding) = stale_finding(endpoint_slice, pillar, now) {
            stale_resources += 1;
            findings.push(finding);
        }

        match pillar {
            Pillar::Cost => evaluate_cost(endpoint_slice, pillar, &mut findings),
            Pillar::Resilience => evaluate_resilience(endpoint_slice, pillar, &mut findings),
            Pillar::Security => evaluate_security(endpoint_slice, pillar, &mut findings),
            _ => {}
        }
    }

    PillarReport {
        pillar,
        resources_evaluated: endpoint_slices.len(),
        stale_resources,
        score: score_pillar(&findings),
        findings,
    }
}

fn evaluate_cost(
    endpoint_slice: &EndpointSliceInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if has_any_metadata_key(&endpoint_slice.labels, COST_ALLOCATION_TAG_KEYS)
        || has_any_metadata_key(&endpoint_slice.annotations, COST_ALLOCATION_TAG_KEYS)
    {
        return;
    }

    findings.push(finding(
        endpoint_slice,
        pillar,
        REASON_COST_OWNER_NOT_RECORDED,
        Severity::Medium,
        format!(
            "Kubernetes EndpointSlice {}/{} has no owner, team, project, or cost-center label or annotation",
            endpoint_slice.namespace, endpoint_slice.name
        ),
        json!({
            "cluster_id": endpoint_slice.cluster_id,
            "namespace": endpoint_slice.namespace,
            "name": endpoint_slice.name,
            "service_name": endpoint_slice.service_name,
            "address_type": endpoint_slice.address_type,
            "ready_address_count": endpoint_slice.ready_addresses.len(),
            "not_ready_address_count": endpoint_slice.not_ready_addresses.len(),
            "ports": endpoint_slice.ports,
            "checked_keys": COST_ALLOCATION_TAG_KEYS,
            "checked_locations": ["labels", "annotations"],
        }),
    ));
}

fn evaluate_resilience(
    endpoint_slice: &EndpointSliceInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if endpoint_slice.ready_addresses.is_empty() {
        findings.push(finding(
            endpoint_slice,
            pillar,
            REASON_RES_NO_READY_ADDRESSES,
            Severity::High,
            format!(
                "Kubernetes EndpointSlice {}/{} has no ready backend addresses",
                endpoint_slice.namespace, endpoint_slice.name
            ),
            json!({
                "cluster_id": endpoint_slice.cluster_id,
                "namespace": endpoint_slice.namespace,
                "name": endpoint_slice.name,
                "service_name": endpoint_slice.service_name,
                "address_type": endpoint_slice.address_type,
                "not_ready_addresses": endpoint_slice.not_ready_addresses,
            }),
        ));
    }

    if endpoint_slice.ports.is_empty()
        || endpoint_slice.ports.iter().all(|port| port.port.is_none())
    {
        findings.push(finding(
            endpoint_slice,
            pillar,
            REASON_RES_PORTS_NOT_DEFINED,
            Severity::Medium,
            format!(
                "Kubernetes EndpointSlice {}/{} has no concrete endpoint ports",
                endpoint_slice.namespace, endpoint_slice.name
            ),
            json!({
                "cluster_id": endpoint_slice.cluster_id,
                "namespace": endpoint_slice.namespace,
                "name": endpoint_slice.name,
                "service_name": endpoint_slice.service_name,
                "address_type": endpoint_slice.address_type,
                "ports": endpoint_slice.ports,
            }),
        ));
    }
}

fn evaluate_security(
    endpoint_slice: &EndpointSliceInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    let managed_ready_count = endpoint_slice
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
    let unmanaged_addresses = endpoint_slice
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
            endpoint_slice,
            pillar,
            REASON_SEC_UNMANAGED_BACKEND,
            Severity::Medium,
            format!(
                "Kubernetes EndpointSlice {}/{} includes ready backends without Pod target references",
                endpoint_slice.namespace, endpoint_slice.name
            ),
            json!({
                "cluster_id": endpoint_slice.cluster_id,
                "namespace": endpoint_slice.namespace,
                "name": endpoint_slice.name,
                "service_name": endpoint_slice.service_name,
                "address_type": endpoint_slice.address_type,
                "managed_ready_count": managed_ready_count,
                "unmanaged_addresses": unmanaged_addresses,
            }),
        ));
    }

    let fqdn_addresses = all_addresses(endpoint_slice)
        .into_iter()
        .filter(|address| {
            endpoint_slice.address_type.eq_ignore_ascii_case("FQDN")
                || address.address.parse::<IpAddr>().is_err()
        })
        .collect::<Vec<_>>();
    if !fqdn_addresses.is_empty() {
        findings.push(finding(
            endpoint_slice,
            pillar,
            REASON_SEC_FQDN_ADDRESS,
            Severity::Medium,
            format!(
                "Kubernetes EndpointSlice {}/{} routes to FQDN-style backend addresses",
                endpoint_slice.namespace, endpoint_slice.name
            ),
            json!({
                "cluster_id": endpoint_slice.cluster_id,
                "namespace": endpoint_slice.namespace,
                "name": endpoint_slice.name,
                "service_name": endpoint_slice.service_name,
                "address_type": endpoint_slice.address_type,
                "fqdn_addresses": fqdn_addresses,
            }),
        ));
    }
}

fn stale_finding(
    endpoint_slice: &EndpointSliceInventoryItem,
    pillar: Pillar,
    now: DateTime<Utc>,
) -> Option<InventoryFinding> {
    let age_hours = (now - endpoint_slice.collected_at).num_hours();
    if age_hours <= DEFAULT_STALE_AFTER_HOURS {
        return None;
    }

    Some(finding(
        endpoint_slice,
        pillar,
        REASON_INV_STALE_DATA,
        Severity::Medium,
        format!(
            "Inventory data for Kubernetes EndpointSlice {}/{} is {} hours old (threshold {} hours)",
            endpoint_slice.namespace, endpoint_slice.name, age_hours, DEFAULT_STALE_AFTER_HOURS
        ),
        json!({
            "cluster_id": endpoint_slice.cluster_id,
            "namespace": endpoint_slice.namespace,
            "name": endpoint_slice.name,
            "service_name": endpoint_slice.service_name,
            "address_type": endpoint_slice.address_type,
            "collected_at": endpoint_slice.collected_at,
            "age_hours": age_hours,
            "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
        }),
    ))
}

fn finding(
    endpoint_slice: &EndpointSliceInventoryItem,
    pillar: Pillar,
    reason_code: &str,
    severity: Severity,
    message: String,
    evidence: Value,
) -> InventoryFinding {
    InventoryFinding {
        resource_id: format!(
            "{}/{}/EndpointSlice/{}",
            endpoint_slice.cluster_id, endpoint_slice.namespace, endpoint_slice.name
        ),
        arn: format!(
            "kubernetes://endpointslices/{}/{}/{}",
            endpoint_slice.cluster_id, endpoint_slice.namespace, endpoint_slice.name
        ),
        pillar,
        reason_code: reason_code.to_string(),
        severity,
        message,
        evidence,
    }
}

fn all_addresses(endpoint_slice: &EndpointSliceInventoryItem) -> Vec<EndpointAddressInventoryItem> {
    endpoint_slice
        .ready_addresses
        .iter()
        .chain(endpoint_slice.not_ready_addresses.iter())
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

    fn endpoint_slice(
        name: &str,
        metadata_labels: BTreeMap<String, String>,
    ) -> EndpointSliceInventoryItem {
        EndpointSliceInventoryItem {
            cluster_id: "cluster-a".to_string(),
            namespace: "apps".to_string(),
            name: name.to_string(),
            service_name: Some(name.to_string()),
            address_type: "IPv4".to_string(),
            labels: metadata_labels,
            annotations: BTreeMap::new(),
            ports: vec![port("https", 443)],
            ready_addresses: vec![address("10.42.0.12", Some("Pod"))],
            not_ready_addresses: Vec::new(),
            created_at: Some(now() - Duration::days(3)),
            collected_at: now(),
        }
    }

    fn healthy_endpoint_slice() -> EndpointSliceInventoryItem {
        endpoint_slice("checkout", labels(&[("team", "payments")]))
    }

    #[test]
    fn cost_flags_missing_owner_and_cost_allocation_labels() {
        let report = evaluate_kubernetes_endpoint_slice_inventory(
            &[endpoint_slice("untagged", BTreeMap::new())],
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
        let mut empty = healthy_endpoint_slice();
        empty.ready_addresses = Vec::new();
        empty.ports = Vec::new();

        let report =
            evaluate_kubernetes_endpoint_slice_inventory(&[empty], Pillar::Resilience, now());
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
        let mut exposed = healthy_endpoint_slice();
        exposed.address_type = "FQDN".to_string();
        exposed.ready_addresses = vec![address("db.internal.example.com", None)];

        let report =
            evaluate_kubernetes_endpoint_slice_inventory(&[exposed], Pillar::Security, now());
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
        let mut stale = healthy_endpoint_slice();
        stale.collected_at = now() - Duration::hours(DEFAULT_STALE_AFTER_HOURS + 2);

        let report = evaluate_kubernetes_endpoint_slice_inventory(&[stale], Pillar::Cost, now());

        assert_eq!(report.stale_resources, 1);
        assert_eq!(report.findings[0].reason_code, REASON_INV_STALE_DATA);
    }

    #[test]
    fn healthy_endpoint_slices_pass_claimed_pillars() {
        for pillar in [Pillar::Cost, Pillar::Resilience, Pillar::Security] {
            let report = evaluate_kubernetes_endpoint_slice_inventory(
                &[healthy_endpoint_slice()],
                pillar,
                now(),
            );

            assert_eq!(report.resources_evaluated, 1);
            assert_eq!(report.stale_resources, 0);
            assert!(report.findings.is_empty());
        }
    }
}

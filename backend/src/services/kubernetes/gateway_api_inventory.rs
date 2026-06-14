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

// Deterministic Kubernetes Gateway API inventory evaluator for roadmap rows
// 02-KUBERNETES-DASHBOARD-00589/00596/00617.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::services::aws::inventory::types::{
    score_pillar, InventoryFinding, Pillar, PillarReport, Severity, COST_ALLOCATION_TAG_KEYS,
    DEFAULT_STALE_AFTER_HOURS,
};

pub const RESOURCE_TYPE: &str = "KubernetesGatewayApi";
pub const REASON_COST_OWNER_NOT_RECORDED: &str = "K8S_GATEWAY_API_COST_OWNER_NOT_RECORDED";
pub const REASON_RES_GATEWAY_NOT_PROGRAMMED: &str = "K8S_GATEWAY_API_RES_GATEWAY_NOT_PROGRAMMED";
pub const REASON_RES_ROUTE_PARENT_REF_NOT_SET: &str =
    "K8S_GATEWAY_API_RES_ROUTE_PARENT_REF_NOT_SET";
pub const REASON_SEC_CLEAR_TEXT_LISTENER: &str = "K8S_GATEWAY_API_SEC_CLEAR_TEXT_LISTENER";
pub const REASON_SEC_TLS_CERTIFICATE_NOT_CONFIGURED: &str =
    "K8S_GATEWAY_API_SEC_TLS_CERTIFICATE_NOT_CONFIGURED";
pub const REASON_SEC_WILDCARD_HOST: &str = "K8S_GATEWAY_API_SEC_WILDCARD_HOST";
pub const REASON_INV_STALE_DATA: &str = "K8S_GATEWAY_API_INV_STALE_DATA";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatewayApiListenerInventoryItem {
    pub name: Option<String>,
    pub protocol: Option<String>,
    pub hostname: Option<String>,
    pub tls_mode: Option<String>,
    pub certificate_ref_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatewayApiParentRefInventoryItem {
    pub group: Option<String>,
    pub kind: Option<String>,
    pub namespace: Option<String>,
    pub name: String,
    pub section_name: Option<String>,
    pub port: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatewayApiConditionInventoryItem {
    pub type_: String,
    pub status: String,
    pub reason: Option<String>,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatewayApiInventoryItem {
    pub cluster_id: String,
    pub api_version: String,
    pub kind: String,
    pub namespace: Option<String>,
    pub name: String,
    pub gateway_class_name: Option<String>,
    pub labels: BTreeMap<String, String>,
    pub annotations: BTreeMap<String, String>,
    pub listeners: Vec<GatewayApiListenerInventoryItem>,
    pub parent_refs: Vec<GatewayApiParentRefInventoryItem>,
    pub address_count: usize,
    pub conditions: Vec<GatewayApiConditionInventoryItem>,
    pub spec: Value,
    pub status: Value,
    pub created_at: Option<DateTime<Utc>>,
    pub collected_at: DateTime<Utc>,
}

pub fn evaluate_kubernetes_gateway_api_inventory(
    resources: &[GatewayApiInventoryItem],
    pillar: Pillar,
    now: DateTime<Utc>,
) -> PillarReport {
    let mut stale_resources = 0;
    let mut findings = Vec::new();

    for resource in resources {
        if let Some(finding) = stale_finding(resource, pillar, now) {
            stale_resources += 1;
            findings.push(finding);
        }

        match pillar {
            Pillar::Cost => evaluate_cost(resource, pillar, &mut findings),
            Pillar::Resilience => evaluate_resilience(resource, pillar, &mut findings),
            Pillar::Security => evaluate_security(resource, pillar, &mut findings),
            _ => {}
        }
    }

    PillarReport {
        pillar,
        resources_evaluated: resources.len(),
        stale_resources,
        score: score_pillar(&findings),
        findings,
    }
}

fn evaluate_cost(
    resource: &GatewayApiInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if has_any_metadata_key(&resource.labels, COST_ALLOCATION_TAG_KEYS)
        || has_any_metadata_key(&resource.annotations, COST_ALLOCATION_TAG_KEYS)
    {
        return;
    }

    findings.push(finding(
        resource,
        pillar,
        REASON_COST_OWNER_NOT_RECORDED,
        Severity::Medium,
        format!(
            "Kubernetes Gateway API {} {} has no owner, team, project, or cost-center label or annotation",
            resource.kind,
            namespaced_name(resource)
        ),
        json!({
            "cluster_id": resource.cluster_id,
            "api_version": resource.api_version,
            "kind": resource.kind,
            "namespace": resource.namespace,
            "name": resource.name,
            "gateway_class_name": resource.gateway_class_name,
            "checked_keys": COST_ALLOCATION_TAG_KEYS,
            "checked_locations": ["labels", "annotations"],
        }),
    ));
}

fn evaluate_resilience(
    resource: &GatewayApiInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if resource.kind.eq_ignore_ascii_case("Gateway")
        && (!has_condition_status(resource, "Programmed", "True") || resource.address_count == 0)
    {
        findings.push(finding(
            resource,
            pillar,
            REASON_RES_GATEWAY_NOT_PROGRAMMED,
            Severity::High,
            format!(
                "Kubernetes Gateway API Gateway {} is not programmed with an assigned address",
                namespaced_name(resource)
            ),
            json!({
                "cluster_id": resource.cluster_id,
                "namespace": resource.namespace,
                "name": resource.name,
                "gateway_class_name": resource.gateway_class_name,
                "address_count": resource.address_count,
                "conditions": resource.conditions,
            }),
        ));
    }

    if is_route(resource) && resource.parent_refs.is_empty() {
        findings.push(finding(
            resource,
            pillar,
            REASON_RES_ROUTE_PARENT_REF_NOT_SET,
            Severity::Medium,
            format!(
                "Kubernetes Gateway API route {} has no parentRefs",
                namespaced_name(resource)
            ),
            json!({
                "cluster_id": resource.cluster_id,
                "api_version": resource.api_version,
                "kind": resource.kind,
                "namespace": resource.namespace,
                "name": resource.name,
                "conditions": resource.conditions,
            }),
        ));
    }
}

fn evaluate_security(
    resource: &GatewayApiInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    let clear_text_listeners = resource
        .listeners
        .iter()
        .filter(|listener| {
            listener
                .protocol
                .as_deref()
                .map(|protocol| protocol.eq_ignore_ascii_case("HTTP"))
                .unwrap_or(false)
        })
        .cloned()
        .collect::<Vec<_>>();
    if !clear_text_listeners.is_empty() {
        findings.push(finding(
            resource,
            pillar,
            REASON_SEC_CLEAR_TEXT_LISTENER,
            Severity::High,
            format!(
                "Kubernetes Gateway API Gateway {} exposes clear-text HTTP listeners",
                namespaced_name(resource)
            ),
            json!({
                "cluster_id": resource.cluster_id,
                "namespace": resource.namespace,
                "name": resource.name,
                "listeners": clear_text_listeners,
            }),
        ));
    }

    let tls_without_certificates = resource
        .listeners
        .iter()
        .filter(|listener| listener_is_tls(listener) && listener.certificate_ref_count == 0)
        .cloned()
        .collect::<Vec<_>>();
    if !tls_without_certificates.is_empty() {
        findings.push(finding(
            resource,
            pillar,
            REASON_SEC_TLS_CERTIFICATE_NOT_CONFIGURED,
            Severity::High,
            format!(
                "Kubernetes Gateway API Gateway {} has TLS listeners without certificateRefs",
                namespaced_name(resource)
            ),
            json!({
                "cluster_id": resource.cluster_id,
                "namespace": resource.namespace,
                "name": resource.name,
                "listeners": tls_without_certificates,
            }),
        ));
    }

    let wildcard_hosts = resource
        .listeners
        .iter()
        .filter_map(|listener| listener.hostname.as_ref())
        .filter(|hostname| hostname.trim_start().starts_with("*."))
        .cloned()
        .collect::<Vec<_>>();
    if !wildcard_hosts.is_empty() {
        findings.push(finding(
            resource,
            pillar,
            REASON_SEC_WILDCARD_HOST,
            Severity::Medium,
            format!(
                "Kubernetes Gateway API Gateway {} accepts wildcard hostnames",
                namespaced_name(resource)
            ),
            json!({
                "cluster_id": resource.cluster_id,
                "namespace": resource.namespace,
                "name": resource.name,
                "wildcard_hosts": wildcard_hosts,
            }),
        ));
    }
}

fn stale_finding(
    resource: &GatewayApiInventoryItem,
    pillar: Pillar,
    now: DateTime<Utc>,
) -> Option<InventoryFinding> {
    let age_hours = (now - resource.collected_at).num_hours();
    if age_hours <= DEFAULT_STALE_AFTER_HOURS {
        return None;
    }

    Some(finding(
        resource,
        pillar,
        REASON_INV_STALE_DATA,
        Severity::Medium,
        format!(
            "Inventory data for Kubernetes Gateway API {} {} is {} hours old (threshold {} hours)",
            resource.kind,
            namespaced_name(resource),
            age_hours,
            DEFAULT_STALE_AFTER_HOURS
        ),
        json!({
            "cluster_id": resource.cluster_id,
            "api_version": resource.api_version,
            "kind": resource.kind,
            "namespace": resource.namespace,
            "name": resource.name,
            "collected_at": resource.collected_at,
            "age_hours": age_hours,
            "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
        }),
    ))
}

fn finding(
    resource: &GatewayApiInventoryItem,
    pillar: Pillar,
    reason_code: &str,
    severity: Severity,
    message: String,
    evidence: Value,
) -> InventoryFinding {
    InventoryFinding {
        resource_id: resource_id(resource),
        arn: format!(
            "kubernetes://gateway-api/{}/{}/{}",
            resource.cluster_id,
            resource.kind.to_ascii_lowercase(),
            namespaced_name(resource)
        ),
        pillar,
        reason_code: reason_code.to_string(),
        severity,
        message,
        evidence,
    }
}

fn resource_id(resource: &GatewayApiInventoryItem) -> String {
    format!(
        "{}/{}/{}",
        resource.cluster_id,
        resource.kind,
        namespaced_name(resource)
    )
}

fn namespaced_name(resource: &GatewayApiInventoryItem) -> String {
    resource
        .namespace
        .as_ref()
        .filter(|namespace| !namespace.trim().is_empty())
        .map(|namespace| format!("{}/{}", namespace, resource.name))
        .unwrap_or_else(|| resource.name.clone())
}

fn has_condition_status(resource: &GatewayApiInventoryItem, type_: &str, status: &str) -> bool {
    resource.conditions.iter().any(|condition| {
        condition.type_.eq_ignore_ascii_case(type_) && condition.status.eq_ignore_ascii_case(status)
    })
}

fn is_route(resource: &GatewayApiInventoryItem) -> bool {
    resource.kind.ends_with("Route")
}

fn listener_is_tls(listener: &GatewayApiListenerInventoryItem) -> bool {
    listener
        .protocol
        .as_deref()
        .map(|protocol| {
            protocol.eq_ignore_ascii_case("HTTPS")
                || protocol.eq_ignore_ascii_case("TLS")
                || protocol.eq_ignore_ascii_case("HTTP2")
        })
        .unwrap_or(false)
        || listener
            .tls_mode
            .as_deref()
            .map(|mode| !mode.trim().is_empty())
            .unwrap_or(false)
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

    fn listener(
        protocol: &str,
        hostname: Option<&str>,
        certificate_ref_count: usize,
    ) -> GatewayApiListenerInventoryItem {
        GatewayApiListenerInventoryItem {
            name: Some("https".to_string()),
            protocol: Some(protocol.to_string()),
            hostname: hostname.map(str::to_string),
            tls_mode: Some("Terminate".to_string()),
            certificate_ref_count,
        }
    }

    fn condition(type_: &str, status: &str) -> GatewayApiConditionInventoryItem {
        GatewayApiConditionInventoryItem {
            type_: type_.to_string(),
            status: status.to_string(),
            reason: None,
            message: None,
        }
    }

    fn parent_ref(name: &str) -> GatewayApiParentRefInventoryItem {
        GatewayApiParentRefInventoryItem {
            group: Some("gateway.networking.k8s.io".to_string()),
            kind: Some("Gateway".to_string()),
            namespace: Some("apps".to_string()),
            name: name.to_string(),
            section_name: Some("https".to_string()),
            port: Some(443),
        }
    }

    fn gateway(name: &str, metadata_labels: BTreeMap<String, String>) -> GatewayApiInventoryItem {
        GatewayApiInventoryItem {
            cluster_id: "cluster-a".to_string(),
            api_version: "gateway.networking.k8s.io/v1".to_string(),
            kind: "Gateway".to_string(),
            namespace: Some("apps".to_string()),
            name: name.to_string(),
            gateway_class_name: Some("istio".to_string()),
            labels: metadata_labels,
            annotations: BTreeMap::new(),
            listeners: vec![listener("HTTPS", Some("api.example.com"), 1)],
            parent_refs: Vec::new(),
            address_count: 1,
            conditions: vec![condition("Programmed", "True")],
            spec: json!({}),
            status: json!({}),
            created_at: Some(now() - Duration::days(7)),
            collected_at: now(),
        }
    }

    fn route(
        name: &str,
        parent_refs: Vec<GatewayApiParentRefInventoryItem>,
    ) -> GatewayApiInventoryItem {
        GatewayApiInventoryItem {
            cluster_id: "cluster-a".to_string(),
            api_version: "gateway.networking.k8s.io/v1".to_string(),
            kind: "HTTPRoute".to_string(),
            namespace: Some("apps".to_string()),
            name: name.to_string(),
            gateway_class_name: None,
            labels: labels(&[("team", "edge")]),
            annotations: BTreeMap::new(),
            listeners: Vec::new(),
            parent_refs,
            address_count: 0,
            conditions: vec![condition("Accepted", "True")],
            spec: json!({}),
            status: json!({}),
            created_at: Some(now() - Duration::days(5)),
            collected_at: now(),
        }
    }

    fn healthy_gateway() -> GatewayApiInventoryItem {
        gateway("edge", labels(&[("team", "platform")]))
    }

    #[test]
    fn cost_flags_missing_owner_and_cost_allocation_labels() {
        let report = evaluate_kubernetes_gateway_api_inventory(
            &[gateway("untagged", BTreeMap::new())],
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
    fn resilience_flags_unprogrammed_gateways_and_orphaned_routes() {
        let mut unprogrammed = gateway("edge", labels(&[("team", "edge")]));
        unprogrammed.address_count = 0;
        unprogrammed.conditions = vec![condition("Programmed", "False")];
        let orphaned_route = route("checkout", Vec::new());

        let report = evaluate_kubernetes_gateway_api_inventory(
            &[unprogrammed, orphaned_route],
            Pillar::Resilience,
            now(),
        );
        let reason_codes = report
            .findings
            .iter()
            .map(|finding| finding.reason_code.as_str())
            .collect::<Vec<_>>();

        assert!(reason_codes.contains(&REASON_RES_GATEWAY_NOT_PROGRAMMED));
        assert!(reason_codes.contains(&REASON_RES_ROUTE_PARENT_REF_NOT_SET));
    }

    #[test]
    fn security_flags_cleartext_missing_certificates_and_wildcards() {
        let mut exposed = healthy_gateway();
        exposed.listeners = vec![
            listener("HTTP", Some("api.example.com"), 0),
            listener("HTTPS", Some("*.example.com"), 0),
        ];

        let report = evaluate_kubernetes_gateway_api_inventory(&[exposed], Pillar::Security, now());
        let reason_codes = report
            .findings
            .iter()
            .map(|finding| finding.reason_code.as_str())
            .collect::<Vec<_>>();

        assert!(reason_codes.contains(&REASON_SEC_CLEAR_TEXT_LISTENER));
        assert!(reason_codes.contains(&REASON_SEC_TLS_CERTIFICATE_NOT_CONFIGURED));
        assert!(reason_codes.contains(&REASON_SEC_WILDCARD_HOST));
    }

    #[test]
    fn stale_inventory_is_counted_for_any_pillar() {
        let mut stale = healthy_gateway();
        stale.collected_at = now() - Duration::hours(DEFAULT_STALE_AFTER_HOURS + 2);

        let report = evaluate_kubernetes_gateway_api_inventory(&[stale], Pillar::Cost, now());

        assert_eq!(report.stale_resources, 1);
        assert_eq!(report.findings[0].reason_code, REASON_INV_STALE_DATA);
    }

    #[test]
    fn healthy_gateway_api_resources_pass_claimed_pillars() {
        let route = route("checkout", vec![parent_ref("edge")]);
        for pillar in [Pillar::Cost, Pillar::Resilience, Pillar::Security] {
            let report = evaluate_kubernetes_gateway_api_inventory(
                &[healthy_gateway(), route.clone()],
                pillar,
                now(),
            );

            assert_eq!(report.resources_evaluated, 2);
            assert_eq!(report.stale_resources, 0);
            assert!(report.findings.is_empty());
        }
    }
}

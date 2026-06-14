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

use crate::errors::AppError;
use crate::models::cluster::KubernetesClusterConfig;
use crate::services::kubernetes::client::ClientFactory;
use crate::services::kubernetes::network_policy_inventory::{
    NetworkPolicyInventoryItem, NetworkPolicyOwnerReferenceInventoryItem,
    NetworkPolicyPeerInventoryItem, NetworkPolicyPortInventoryItem, NetworkPolicyRuleInventoryItem,
    NetworkPolicySelectorInventoryItem,
};
use chrono::{DateTime, Utc};
use k8s_openapi::api::networking::v1::{
    NetworkPolicy, NetworkPolicyEgressRule, NetworkPolicyIngressRule, NetworkPolicyPeer,
    NetworkPolicyPort,
};
use k8s_openapi::apimachinery::pkg::apis::meta::v1::LabelSelector;
use k8s_openapi::apimachinery::pkg::util::intstr::IntOrString;
use kube::api::{DeleteParams, ListParams, Patch, PatchParams};
use kube::{Api, ResourceExt};

pub struct NetworkPoliciesService;

fn convert_int_or_string(value: &IntOrString) -> String {
    match value {
        IntOrString::Int(value) => value.to_string(),
        IntOrString::String(value) => value.clone(),
    }
}

fn convert_label_selector(selector: &LabelSelector) -> NetworkPolicySelectorInventoryItem {
    NetworkPolicySelectorInventoryItem {
        match_labels: selector.match_labels.clone().unwrap_or_default(),
        match_expression_count: selector
            .match_expressions
            .as_ref()
            .map(|expressions| expressions.len())
            .unwrap_or(0),
    }
}

fn convert_network_policy_port(port: &NetworkPolicyPort) -> NetworkPolicyPortInventoryItem {
    NetworkPolicyPortInventoryItem {
        protocol: port.protocol.clone(),
        port: port.port.as_ref().map(convert_int_or_string),
        end_port: port.end_port,
    }
}

fn convert_network_policy_peer(peer: &NetworkPolicyPeer) -> NetworkPolicyPeerInventoryItem {
    let mut ip_block_except = peer
        .ip_block
        .as_ref()
        .and_then(|ip_block| ip_block.except.clone())
        .unwrap_or_default();
    ip_block_except.sort();

    NetworkPolicyPeerInventoryItem {
        ip_block_cidr: peer.ip_block.as_ref().map(|ip_block| ip_block.cidr.clone()),
        ip_block_except,
        namespace_selector: peer.namespace_selector.as_ref().map(convert_label_selector),
        pod_selector: peer.pod_selector.as_ref().map(convert_label_selector),
    }
}

fn convert_ingress_rule(rule: &NetworkPolicyIngressRule) -> NetworkPolicyRuleInventoryItem {
    let peers = rule
        .from
        .as_ref()
        .map(|peers| peers.iter().map(convert_network_policy_peer).collect())
        .unwrap_or_default();
    let ports = rule
        .ports
        .as_ref()
        .map(|ports| ports.iter().map(convert_network_policy_port).collect())
        .unwrap_or_default();

    NetworkPolicyRuleInventoryItem {
        direction: "ingress".to_string(),
        peers,
        ports,
    }
}

fn convert_egress_rule(rule: &NetworkPolicyEgressRule) -> NetworkPolicyRuleInventoryItem {
    let peers = rule
        .to
        .as_ref()
        .map(|peers| peers.iter().map(convert_network_policy_peer).collect())
        .unwrap_or_default();
    let ports = rule
        .ports
        .as_ref()
        .map(|ports| ports.iter().map(convert_network_policy_port).collect())
        .unwrap_or_default();

    NetworkPolicyRuleInventoryItem {
        direction: "egress".to_string(),
        peers,
        ports,
    }
}

fn convert_kube_network_policy_to_inventory(
    network_policy: &NetworkPolicy,
    cluster_id: &str,
    current_namespace: &str,
    collected_at: DateTime<Utc>,
) -> NetworkPolicyInventoryItem {
    let namespace = network_policy
        .namespace()
        .unwrap_or_else(|| current_namespace.to_string());
    let spec = network_policy.spec.as_ref();
    let mut policy_types = spec
        .and_then(|spec| spec.policy_types.clone())
        .unwrap_or_default();
    policy_types.sort();

    let pod_selector = spec
        .map(|spec| convert_label_selector(&spec.pod_selector))
        .unwrap_or_else(|| NetworkPolicySelectorInventoryItem {
            match_labels: Default::default(),
            match_expression_count: 0,
        });
    let ingress_rules = spec
        .and_then(|spec| spec.ingress.as_ref())
        .map(|rules| rules.iter().map(convert_ingress_rule).collect())
        .unwrap_or_default();
    let egress_rules = spec
        .and_then(|spec| spec.egress.as_ref())
        .map(|rules| rules.iter().map(convert_egress_rule).collect())
        .unwrap_or_default();
    let owner_references = network_policy
        .metadata
        .owner_references
        .as_ref()
        .map(|owners| {
            owners
                .iter()
                .map(|owner| NetworkPolicyOwnerReferenceInventoryItem {
                    kind: Some(owner.kind.clone()),
                    name: owner.name.clone(),
                    controller: owner.controller,
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    NetworkPolicyInventoryItem {
        cluster_id: cluster_id.to_string(),
        namespace,
        name: network_policy.name_any(),
        labels: network_policy.metadata.labels.clone().unwrap_or_default(),
        annotations: network_policy
            .metadata
            .annotations
            .clone()
            .unwrap_or_default(),
        pod_selector,
        policy_types,
        ingress_rules,
        egress_rules,
        owner_references,
        created_at: network_policy
            .metadata
            .creation_timestamp
            .as_ref()
            .map(|timestamp| timestamp.0),
        collected_at,
    }
}

impl NetworkPoliciesService {
    pub fn new() -> Self {
        Self
    }

    async fn api(
        cluster: &KubernetesClusterConfig,
        namespace: &str,
    ) -> Result<Api<NetworkPolicy>, AppError> {
        let client = ClientFactory::get_client(cluster).await?;
        Ok(if namespace.is_empty() || namespace == "all" {
            Api::all(client)
        } else {
            Api::namespaced(client, namespace)
        })
    }

    pub async fn list(
        &self,
        cluster: &KubernetesClusterConfig,
        namespace: &str,
    ) -> Result<Vec<NetworkPolicy>, AppError> {
        let api = Self::api(cluster, namespace).await?;
        let list = api
            .list(&ListParams::default())
            .await
            .map_err(|e| AppError::Kubernetes(e.to_string()))?;
        Ok(list.items)
    }

    pub async fn list_inventory(
        &self,
        cluster: &KubernetesClusterConfig,
        cluster_id: &str,
        namespace: Option<&str>,
    ) -> Result<Vec<NetworkPolicyInventoryItem>, AppError> {
        let namespace = namespace
            .map(str::trim)
            .filter(|namespace| !namespace.is_empty());
        let namespace_arg = namespace.unwrap_or("");
        let fallback_namespace = namespace
            .filter(|namespace| *namespace != "all")
            .unwrap_or("");
        let collected_at = Utc::now();

        let api = Self::api(cluster, namespace_arg).await?;
        let list = api
            .list(&ListParams::default())
            .await
            .map_err(|e| AppError::Kubernetes(e.to_string()))?;
        let mut inventory = list
            .items
            .iter()
            .map(|network_policy| {
                convert_kube_network_policy_to_inventory(
                    network_policy,
                    cluster_id,
                    fallback_namespace,
                    collected_at,
                )
            })
            .collect::<Vec<_>>();
        inventory.sort_by(|left, right| {
            (left.namespace.as_str(), left.name.as_str())
                .cmp(&(right.namespace.as_str(), right.name.as_str()))
        });
        Ok(inventory)
    }

    pub async fn get(
        &self,
        cluster: &KubernetesClusterConfig,
        namespace: &str,
        name: &str,
    ) -> Result<NetworkPolicy, AppError> {
        let api: Api<NetworkPolicy> =
            Api::namespaced(ClientFactory::get_client(cluster).await?, namespace);
        api.get(name)
            .await
            .map_err(|e| AppError::Kubernetes(e.to_string()))
    }

    pub async fn upsert(
        &self,
        cluster: &KubernetesClusterConfig,
        namespace: &str,
        item: &NetworkPolicy,
    ) -> Result<NetworkPolicy, AppError> {
        let api: Api<NetworkPolicy> =
            Api::namespaced(ClientFactory::get_client(cluster).await?, namespace);
        let params = PatchParams::apply("mayyam").force();
        api.patch(
            item.metadata.name.as_ref().ok_or_else(|| {
                AppError::BadRequest("NetworkPolicy.metadata.name required".into())
            })?,
            &params,
            &Patch::Apply(item),
        )
        .await
        .map_err(|e| AppError::Kubernetes(e.to_string()))
    }

    pub async fn delete(
        &self,
        cluster: &KubernetesClusterConfig,
        namespace: &str,
        name: &str,
    ) -> Result<(), AppError> {
        let api: Api<NetworkPolicy> =
            Api::namespaced(ClientFactory::get_client(cluster).await?, namespace);
        api.delete(name, &DeleteParams::default())
            .await
            .map_err(|e| AppError::Kubernetes(e.to_string()))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use k8s_openapi::api::networking::v1::{
        IPBlock, NetworkPolicyEgressRule, NetworkPolicyIngressRule, NetworkPolicySpec,
    };
    use k8s_openapi::apimachinery::pkg::apis::meta::v1::{ObjectMeta, Time};
    use std::collections::BTreeMap;

    fn map(values: &[(&str, &str)]) -> BTreeMap<String, String> {
        values
            .iter()
            .map(|(key, value)| ((*key).to_string(), (*value).to_string()))
            .collect()
    }

    fn selector(values: &[(&str, &str)]) -> LabelSelector {
        LabelSelector {
            match_labels: Some(map(values)),
            match_expressions: None,
        }
    }

    #[test]
    fn network_policy_inventory_conversion_preserves_metadata_and_rules() {
        let created_at = Utc.with_ymd_and_hms(2026, 6, 1, 12, 0, 0).unwrap();
        let collected_at = Utc.with_ymd_and_hms(2026, 6, 10, 0, 0, 0).unwrap();
        let network_policy = NetworkPolicy {
            metadata: ObjectMeta {
                name: Some("checkout-traffic".to_string()),
                namespace: Some("apps".to_string()),
                labels: Some(map(&[("team", "payments")])),
                annotations: Some(map(&[("cost-center", "cc-73")])),
                creation_timestamp: Some(Time(created_at)),
                ..Default::default()
            },
            spec: Some(NetworkPolicySpec {
                pod_selector: selector(&[("app", "checkout")]),
                policy_types: Some(vec!["Egress".to_string(), "Ingress".to_string()]),
                ingress: Some(vec![NetworkPolicyIngressRule {
                    from: Some(vec![NetworkPolicyPeer {
                        ip_block: Some(IPBlock {
                            cidr: "10.0.0.0/8".to_string(),
                            except: Some(vec![
                                "10.2.0.0/16".to_string(),
                                "10.1.0.0/16".to_string(),
                            ]),
                        }),
                        namespace_selector: None,
                        pod_selector: None,
                    }]),
                    ports: Some(vec![NetworkPolicyPort {
                        protocol: Some("TCP".to_string()),
                        port: Some(IntOrString::Int(8443)),
                        end_port: Some(8444),
                    }]),
                }]),
                egress: Some(vec![NetworkPolicyEgressRule {
                    to: None,
                    ports: None,
                }]),
            }),
            status: None,
        };

        let item = convert_kube_network_policy_to_inventory(
            &network_policy,
            "cluster-a",
            "fallback",
            collected_at,
        );

        assert_eq!(item.cluster_id, "cluster-a");
        assert_eq!(item.namespace, "apps");
        assert_eq!(item.name, "checkout-traffic");
        assert_eq!(item.labels["team"], "payments");
        assert_eq!(item.annotations["cost-center"], "cc-73");
        assert_eq!(item.created_at, Some(created_at));
        assert_eq!(item.collected_at, collected_at);
        assert_eq!(item.pod_selector.match_labels["app"], "checkout");
        assert_eq!(item.policy_types, vec!["Egress", "Ingress"]);
        assert_eq!(item.ingress_rules.len(), 1);
        assert_eq!(item.ingress_rules[0].direction, "ingress");
        assert_eq!(
            item.ingress_rules[0].peers[0].ip_block_cidr,
            Some("10.0.0.0/8".to_string())
        );
        assert_eq!(
            item.ingress_rules[0].peers[0].ip_block_except,
            vec!["10.1.0.0/16", "10.2.0.0/16"]
        );
        assert_eq!(
            item.ingress_rules[0].ports[0].protocol,
            Some("TCP".to_string())
        );
        assert_eq!(
            item.ingress_rules[0].ports[0].port,
            Some("8443".to_string())
        );
        assert_eq!(item.ingress_rules[0].ports[0].end_port, Some(8444));
        assert_eq!(item.egress_rules.len(), 1);
        assert!(item.egress_rules[0].peers.is_empty());
        assert!(item.egress_rules[0].ports.is_empty());
    }

    #[test]
    fn network_policy_inventory_conversion_uses_fallback_namespace_when_missing() {
        let collected_at = Utc.with_ymd_and_hms(2026, 6, 10, 0, 0, 0).unwrap();
        let network_policy = NetworkPolicy {
            metadata: ObjectMeta {
                name: Some("fallback-policy".to_string()),
                ..Default::default()
            },
            spec: None,
            status: None,
        };

        let item = convert_kube_network_policy_to_inventory(
            &network_policy,
            "cluster-a",
            "requested-namespace",
            collected_at,
        );

        assert_eq!(item.namespace, "requested-namespace");
        assert!(item.policy_types.is_empty());
        assert!(item.ingress_rules.is_empty());
        assert!(item.egress_rules.is_empty());
    }
}

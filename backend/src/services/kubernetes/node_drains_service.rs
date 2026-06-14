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

use chrono::{DateTime, Utc};
use k8s_openapi::api::core::v1::{Node, Taint};
use kube::{api::ListParams, Api, Client, ResourceExt};

use crate::errors::AppError;
use crate::models::cluster::KubernetesClusterConfig;
use crate::services::kubernetes::client::ClientFactory;
use crate::services::kubernetes::node_drains_inventory::NodeDrainInventoryItem;

pub struct NodeDrainsService;

impl NodeDrainsService {
    pub fn new() -> Self {
        NodeDrainsService {}
    }

    async fn get_kube_client(cluster_config: &KubernetesClusterConfig) -> Result<Client, AppError> {
        ClientFactory::get_client(cluster_config).await
    }

    pub async fn list_inventory(
        &self,
        cluster_config: &KubernetesClusterConfig,
        cluster_id: &str,
    ) -> Result<Vec<NodeDrainInventoryItem>, AppError> {
        let client = Self::get_kube_client(cluster_config).await?;
        let api: Api<Node> = Api::all(client);
        let lp = ListParams::default();
        let collected_at = Utc::now();
        let nodes = api.list(&lp).await.map_err(|e| {
            AppError::ExternalService(format!("Failed to list nodes for drains inventory: {}", e))
        })?;

        let mut items = nodes
            .iter()
            .map(|node| convert_node_to_drain_inventory(node, cluster_id, collected_at))
            .collect::<Vec<_>>();
        items.sort_by(|left, right| left.node_name.cmp(&right.node_name));
        Ok(items)
    }
}

fn convert_node_to_drain_inventory(
    node: &Node,
    cluster_id: &str,
    collected_at: DateTime<Utc>,
) -> NodeDrainInventoryItem {
    let taints = node
        .spec
        .as_ref()
        .and_then(|spec| spec.taints.as_ref())
        .cloned()
        .unwrap_or_default();
    let labels = node.metadata.labels.clone().unwrap_or_default();
    let annotations = node.metadata.annotations.clone().unwrap_or_default();

    NodeDrainInventoryItem {
        cluster_id: cluster_id.to_string(),
        node_name: node.name_any(),
        node_ready_status: node_ready_status(node),
        node_unschedulable: node
            .spec
            .as_ref()
            .and_then(|spec| spec.unschedulable)
            .unwrap_or(false),
        no_schedule_taints: count_taint_effects(&taints, "NoSchedule"),
        no_execute_taints: count_taint_effects(&taints, "NoExecute"),
        taint_keys: taint_keys(&taints),
        roles: node_roles(node),
        labels,
        annotations,
        created_at: node
            .metadata
            .creation_timestamp
            .as_ref()
            .map(|timestamp| timestamp.0),
        collected_at,
    }
}

fn count_taint_effects(taints: &[Taint], effect: &str) -> usize {
    taints
        .iter()
        .filter(|taint| taint.effect.eq_ignore_ascii_case(effect))
        .count()
}

fn taint_keys(taints: &[Taint]) -> Vec<String> {
    let mut keys = taints
        .iter()
        .map(|taint| match taint.value.as_deref() {
            Some(value) if !value.trim().is_empty() => {
                format!("{}={}:{}", taint.key, value, taint.effect)
            }
            _ => format!("{}:{}", taint.key, taint.effect),
        })
        .collect::<Vec<_>>();
    keys.sort();
    keys
}

fn node_ready_status(node: &Node) -> Option<String> {
    let conditions = node.status.as_ref()?.conditions.as_ref()?;
    let ready = conditions
        .iter()
        .find(|condition| condition.type_ == "Ready")?;
    if ready.status.eq_ignore_ascii_case("True") {
        Some("Ready".to_string())
    } else {
        Some(format!(
            "NotReady ({})",
            ready.reason.as_deref().unwrap_or("Unknown")
        ))
    }
}

fn node_roles(node: &Node) -> Vec<String> {
    let mut roles = Vec::new();
    if let Some(labels) = &node.metadata.labels {
        for (key, value) in labels {
            if key.starts_with("node-role.kubernetes.io/") && value == "true" {
                roles.push(
                    key.trim_start_matches("node-role.kubernetes.io/")
                        .to_string(),
                );
            }
            if key == "kubernetes.io/role" && (value == "master" || value == "control-plane") {
                roles.push(value.clone());
            }
        }
    }
    if roles.is_empty() {
        roles.push("<none>".to_string());
    }
    roles.sort();
    roles.dedup();
    roles
}

#[cfg(test)]
mod tests {
    use super::*;
    use k8s_openapi::api::core::v1::{NodeCondition, NodeSpec, NodeStatus};
    use k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta;
    use std::collections::BTreeMap;

    fn labels(values: &[(&str, &str)]) -> BTreeMap<String, String> {
        values
            .iter()
            .map(|(key, value)| ((*key).to_string(), (*value).to_string()))
            .collect()
    }

    #[test]
    fn node_conversion_records_drain_state_and_roles() {
        let collected_at = Utc::now();
        let node = Node {
            metadata: ObjectMeta {
                name: Some("ip-10-0-0-10".to_string()),
                labels: Some(labels(&[
                    ("owner", "platform"),
                    ("node-role.kubernetes.io/control-plane", "true"),
                ])),
                ..Default::default()
            },
            spec: Some(NodeSpec {
                taints: Some(vec![
                    Taint {
                        key: "node.kubernetes.io/unschedulable".to_string(),
                        value: None,
                        effect: "NoSchedule".to_string(),
                        time_added: None,
                    },
                    Taint {
                        key: "node.kubernetes.io/unreachable".to_string(),
                        value: None,
                        effect: "NoExecute".to_string(),
                        time_added: None,
                    },
                ]),
                unschedulable: Some(true),
                ..Default::default()
            }),
            status: Some(NodeStatus {
                conditions: Some(vec![NodeCondition {
                    type_: "Ready".to_string(),
                    status: "False".to_string(),
                    reason: Some("KubeletNotReady".to_string()),
                    ..Default::default()
                }]),
                ..Default::default()
            }),
        };

        let item = convert_node_to_drain_inventory(&node, "cluster-1", collected_at);

        assert_eq!(item.cluster_id, "cluster-1");
        assert_eq!(item.node_name, "ip-10-0-0-10");
        assert_eq!(
            item.node_ready_status.as_deref(),
            Some("NotReady (KubeletNotReady)")
        );
        assert!(item.node_unschedulable);
        assert_eq!(item.no_schedule_taints, 1);
        assert_eq!(item.no_execute_taints, 1);
        assert_eq!(
            item.taint_keys,
            vec![
                "node.kubernetes.io/unreachable:NoExecute".to_string(),
                "node.kubernetes.io/unschedulable:NoSchedule".to_string(),
            ]
        );
        assert_eq!(item.roles, vec!["control-plane".to_string()]);
        assert_eq!(
            item.labels.get("owner").map(String::as_str),
            Some("platform")
        );
        assert_eq!(item.collected_at, collected_at);
    }

    #[test]
    fn node_conversion_tolerates_missing_spec_and_status() {
        let node = Node {
            metadata: ObjectMeta {
                name: Some("empty-node".to_string()),
                ..Default::default()
            },
            ..Default::default()
        };

        let item = convert_node_to_drain_inventory(&node, "cluster-1", Utc::now());

        assert_eq!(item.node_name, "empty-node");
        assert!(item.node_ready_status.is_none());
        assert!(!item.node_unschedulable);
        assert_eq!(item.no_schedule_taints, 0);
        assert_eq!(item.no_execute_taints, 0);
        assert!(item.taint_keys.is_empty());
        assert_eq!(item.roles, vec!["<none>".to_string()]);
    }
}

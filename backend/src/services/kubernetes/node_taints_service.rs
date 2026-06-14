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
use crate::services::kubernetes::node_taints_inventory::NodeTaintInventoryItem;

pub struct NodeTaintsService;

impl NodeTaintsService {
    pub fn new() -> Self {
        NodeTaintsService {}
    }

    async fn get_kube_client(cluster_config: &KubernetesClusterConfig) -> Result<Client, AppError> {
        ClientFactory::get_client(cluster_config).await
    }

    pub async fn list_inventory(
        &self,
        cluster_config: &KubernetesClusterConfig,
        cluster_id: &str,
    ) -> Result<Vec<NodeTaintInventoryItem>, AppError> {
        let client = Self::get_kube_client(cluster_config).await?;
        let api: Api<Node> = Api::all(client);
        let lp = ListParams::default();
        let collected_at = Utc::now();
        let nodes = api.list(&lp).await.map_err(|e| {
            AppError::ExternalService(format!("Failed to list nodes for taints inventory: {}", e))
        })?;

        let mut items = nodes
            .iter()
            .flat_map(|node| convert_node_to_taint_inventory(node, cluster_id, collected_at))
            .collect::<Vec<_>>();
        items.sort_by(|left, right| {
            left.node_name
                .cmp(&right.node_name)
                .then(left.taint_key.cmp(&right.taint_key))
                .then(left.effect.cmp(&right.effect))
                .then(left.taint_value.cmp(&right.taint_value))
        });
        Ok(items)
    }
}

fn convert_node_to_taint_inventory(
    node: &Node,
    cluster_id: &str,
    collected_at: DateTime<Utc>,
) -> Vec<NodeTaintInventoryItem> {
    let taints = node
        .spec
        .as_ref()
        .and_then(|spec| spec.taints.as_ref())
        .cloned()
        .unwrap_or_default();
    let labels = node.metadata.labels.clone().unwrap_or_default();
    let annotations = node.metadata.annotations.clone().unwrap_or_default();
    let roles = node_roles(node);
    let ready_status = node_ready_status(node);
    let node_unschedulable = node
        .spec
        .as_ref()
        .and_then(|spec| spec.unschedulable)
        .unwrap_or(false);
    let created_at = node
        .metadata
        .creation_timestamp
        .as_ref()
        .map(|timestamp| timestamp.0);
    let node_name = node.name_any();

    taints
        .iter()
        .map(|taint| {
            convert_taint_to_inventory(
                taint,
                cluster_id,
                &node_name,
                ready_status.as_deref(),
                node_unschedulable,
                &roles,
                &labels,
                &annotations,
                created_at,
                collected_at,
            )
        })
        .collect()
}

#[allow(clippy::too_many_arguments)]
fn convert_taint_to_inventory(
    taint: &Taint,
    cluster_id: &str,
    node_name: &str,
    ready_status: Option<&str>,
    node_unschedulable: bool,
    roles: &[String],
    labels: &std::collections::BTreeMap<String, String>,
    annotations: &std::collections::BTreeMap<String, String>,
    created_at: Option<DateTime<Utc>>,
    collected_at: DateTime<Utc>,
) -> NodeTaintInventoryItem {
    NodeTaintInventoryItem {
        cluster_id: cluster_id.to_string(),
        node_name: node_name.to_string(),
        taint_key: taint.key.clone(),
        taint_value: taint.value.clone(),
        effect: taint.effect.clone(),
        time_added: taint.time_added.as_ref().map(|timestamp| timestamp.0),
        node_ready_status: ready_status.map(str::to_string),
        node_unschedulable,
        roles: roles.to_vec(),
        labels: labels.clone(),
        annotations: annotations.clone(),
        created_at,
        collected_at,
    }
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
    use k8s_openapi::apimachinery::pkg::apis::meta::v1::{ObjectMeta, Time};
    use std::collections::BTreeMap;

    fn labels(values: &[(&str, &str)]) -> BTreeMap<String, String> {
        values
            .iter()
            .map(|(key, value)| ((*key).to_string(), (*value).to_string()))
            .collect()
    }

    #[test]
    fn node_conversion_expands_each_taint() {
        let collected_at = Utc::now();
        let node = Node {
            metadata: ObjectMeta {
                name: Some("ip-10-0-0-10".to_string()),
                labels: Some(labels(&[
                    ("owner", "platform"),
                    ("node-role.kubernetes.io/worker", "true"),
                ])),
                ..Default::default()
            },
            spec: Some(NodeSpec {
                taints: Some(vec![
                    Taint {
                        key: "dedicated".to_string(),
                        value: Some("gpu".to_string()),
                        effect: "NoSchedule".to_string(),
                        time_added: Some(Time(collected_at)),
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
                    status: "True".to_string(),
                    ..Default::default()
                }]),
                ..Default::default()
            }),
        };

        let items = convert_node_to_taint_inventory(&node, "cluster-1", collected_at);

        assert_eq!(items.len(), 2);
        assert_eq!(items[0].node_name, "ip-10-0-0-10");
        assert_eq!(items[0].taint_key, "dedicated");
        assert_eq!(items[0].taint_value.as_deref(), Some("gpu"));
        assert_eq!(items[0].effect, "NoSchedule");
        assert_eq!(items[0].node_ready_status.as_deref(), Some("Ready"));
        assert!(items[0].node_unschedulable);
        assert_eq!(items[0].roles, vec!["worker".to_string()]);
        assert_eq!(items[0].collected_at, collected_at);
        assert_eq!(items[1].taint_key, "node.kubernetes.io/unreachable");
        assert_eq!(items[1].effect, "NoExecute");
    }

    #[test]
    fn node_conversion_ignores_nodes_without_taints() {
        let node = Node {
            metadata: ObjectMeta {
                name: Some("untainted".to_string()),
                ..Default::default()
            },
            ..Default::default()
        };

        let items = convert_node_to_taint_inventory(&node, "cluster-1", Utc::now());

        assert!(items.is_empty());
    }
}

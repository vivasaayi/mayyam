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
use crate::services::kubernetes::replica_set_inventory::{
    ReplicaSetContainerInventoryItem, ReplicaSetInventoryItem,
    ReplicaSetOwnerReferenceInventoryItem,
};
use chrono::Utc;
use k8s_openapi::api::apps::v1::ReplicaSet;
use kube::{api::ListParams, Api, ResourceExt};
use serde_json::Value;

pub struct ReplicaSetsService;

fn convert_kube_replicaset_to_replicaset_inventory(
    replicaset: &ReplicaSet,
    cluster_id: &str,
    current_namespace: &str,
    collected_at: chrono::DateTime<Utc>,
) -> ReplicaSetInventoryItem {
    let namespace = replicaset
        .namespace()
        .unwrap_or_else(|| current_namespace.to_string());
    let spec = replicaset.spec.as_ref();
    let status = replicaset.status.as_ref();
    let pod_template = spec.and_then(|spec| spec.template.as_ref());
    let pod_spec = pod_template.and_then(|template| template.spec.as_ref());
    let containers = pod_spec
        .map(|pod_spec| {
            pod_spec
                .containers
                .iter()
                .map(|container| ReplicaSetContainerInventoryItem {
                    name: container.name.clone(),
                    image: container.image.clone(),
                    privileged: container
                        .security_context
                        .as_ref()
                        .and_then(|security_context| security_context.privileged),
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let owner_references = replicaset
        .metadata
        .owner_references
        .clone()
        .unwrap_or_default()
        .into_iter()
        .map(|owner| ReplicaSetOwnerReferenceInventoryItem {
            api_version: owner.api_version,
            kind: owner.kind,
            name: owner.name,
            controller: owner.controller,
        })
        .collect::<Vec<_>>();

    ReplicaSetInventoryItem {
        cluster_id: cluster_id.to_string(),
        namespace,
        name: replicaset.name_any(),
        desired_replicas: spec.and_then(|spec| spec.replicas).unwrap_or(0),
        current_replicas: status.map(|status| status.replicas).unwrap_or(0),
        available_replicas: status
            .and_then(|status| status.available_replicas)
            .unwrap_or(0),
        ready_replicas: status.and_then(|status| status.ready_replicas).unwrap_or(0),
        fully_labeled_replicas: status
            .and_then(|status| status.fully_labeled_replicas)
            .unwrap_or(0),
        generation: replicaset.metadata.generation,
        observed_generation: status.and_then(|status| status.observed_generation),
        labels: replicaset.metadata.labels.clone().unwrap_or_default(),
        annotations: replicaset.metadata.annotations.clone().unwrap_or_default(),
        selector: spec
            .and_then(|spec| spec.selector.match_labels.clone())
            .unwrap_or_default(),
        pod_template_labels: pod_template
            .and_then(|template| {
                template
                    .metadata
                    .as_ref()
                    .and_then(|metadata| metadata.labels.clone())
            })
            .unwrap_or_default(),
        containers,
        owner_references,
        service_account_name: pod_spec.and_then(|pod_spec| pod_spec.service_account_name.clone()),
        host_network: pod_spec
            .and_then(|pod_spec| pod_spec.host_network)
            .unwrap_or(false),
        created_at: replicaset
            .metadata
            .creation_timestamp
            .as_ref()
            .map(|timestamp| timestamp.0),
        collected_at,
    }
}

impl ReplicaSetsService {
    pub async fn list_replica_sets(
        &self,
        cluster_config: &KubernetesClusterConfig,
        namespace_name: &str,
    ) -> Result<Vec<Value>, AppError> {
        let client = ClientFactory::get_client(cluster_config).await?;
        let replica_sets: Api<ReplicaSet> = if namespace_name.is_empty() {
            Api::all(client)
        } else {
            Api::namespaced(client, namespace_name)
        };

        let rs_list = replica_sets
            .list(&ListParams::default())
            .await
            .map_err(|e| AppError::ExternalService(format!("Failed to list ReplicaSets: {}", e)))?;

        let mut formatted_rs = Vec::new();
        for rs in rs_list {
            if let Ok(value) = serde_json::to_value(&rs) {
                formatted_rs.push(value);
            }
        }

        Ok(formatted_rs)
    }

    pub async fn list_replicaset_inventory(
        &self,
        cluster_config: &KubernetesClusterConfig,
        cluster_id: &str,
        namespace: Option<&str>,
    ) -> Result<Vec<ReplicaSetInventoryItem>, AppError> {
        let namespace = namespace
            .map(str::trim)
            .filter(|namespace| !namespace.is_empty());
        let client = ClientFactory::get_client(cluster_config).await?;
        let replica_sets: Api<ReplicaSet> = match namespace {
            Some(namespace) if namespace != "all" => Api::namespaced(client, namespace),
            _ => Api::all(client),
        };
        let collected_at = Utc::now();
        let rs_list = replica_sets
            .list(&ListParams::default())
            .await
            .map_err(|e| {
                AppError::ExternalService(format!(
                    "Failed to list ReplicaSet inventory in namespace '{}': {}",
                    namespace.unwrap_or("all"),
                    e
                ))
            })?;
        let fallback_namespace = namespace
            .filter(|namespace| *namespace != "all")
            .unwrap_or("");

        Ok(rs_list
            .iter()
            .map(|replicaset| {
                convert_kube_replicaset_to_replicaset_inventory(
                    replicaset,
                    cluster_id,
                    fallback_namespace,
                    collected_at,
                )
            })
            .collect())
    }

    pub async fn get_replica_set_details(
        &self,
        cluster_config: &KubernetesClusterConfig,
        namespace_name: &str,
        replica_set_name: &str,
    ) -> Result<Value, AppError> {
        let client = ClientFactory::get_client(cluster_config).await?;
        let replica_sets: Api<ReplicaSet> = Api::namespaced(client, namespace_name);

        let rs = replica_sets.get(replica_set_name).await.map_err(|e| {
            AppError::ExternalService(format!("Failed to get ReplicaSet details: {}", e))
        })?;

        serde_json::to_value(&rs).map_err(|e| {
            AppError::Internal(format!("Failed to serialize ReplicaSet details: {}", e))
        })
    }
}

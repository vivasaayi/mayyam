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
use k8s_openapi::api::apps::v1::ReplicaSet;
use kube::{api::ListParams, Api};
use serde_json::Value;

pub struct ReplicaSetsService;

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

        serde_json::to_value(&rs)
            .map_err(|e| AppError::Internal(format!("Failed to serialize ReplicaSet details: {}", e)))
    }
}

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
use crate::services::kubernetes::service_account_inventory::{
    ServiceAccountInventoryItem, ServiceAccountOwnerReferenceInventoryItem,
};
use chrono::{DateTime, Utc};
use k8s_openapi::api::core::v1::ServiceAccount;
use kube::api::{DeleteParams, ListParams, Patch, PatchParams};
use kube::{Api, ResourceExt};

pub struct ServiceAccountsService;

fn convert_kube_service_account_to_inventory(
    item: &ServiceAccount,
    cluster_id: &str,
    current_namespace: &str,
    collected_at: DateTime<Utc>,
) -> ServiceAccountInventoryItem {
    let namespace = item
        .namespace()
        .unwrap_or_else(|| current_namespace.to_string());
    let mut image_pull_secret_names = item
        .image_pull_secrets
        .as_ref()
        .map(|secrets| {
            secrets
                .iter()
                .filter_map(|secret| secret.name.clone())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let mut secret_names = item
        .secrets
        .as_ref()
        .map(|secrets| {
            secrets
                .iter()
                .filter_map(|secret| secret.name.clone())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    image_pull_secret_names.sort();
    secret_names.sort();
    let owner_references = item
        .metadata
        .owner_references
        .as_ref()
        .map(|owners| {
            owners
                .iter()
                .map(|owner| ServiceAccountOwnerReferenceInventoryItem {
                    kind: Some(owner.kind.clone()),
                    name: owner.name.clone(),
                    controller: owner.controller,
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    ServiceAccountInventoryItem {
        cluster_id: cluster_id.to_string(),
        namespace,
        name: item.name_any(),
        labels: item.metadata.labels.clone().unwrap_or_default(),
        annotations: item.metadata.annotations.clone().unwrap_or_default(),
        automount_service_account_token: item.automount_service_account_token,
        image_pull_secret_names,
        secret_names,
        owner_references,
        created_at: item
            .metadata
            .creation_timestamp
            .as_ref()
            .map(|timestamp| timestamp.0),
        collected_at,
    }
}

impl ServiceAccountsService {
    pub fn new() -> Self {
        Self
    }

    async fn api(
        cluster: &KubernetesClusterConfig,
        namespace: &str,
    ) -> Result<Api<ServiceAccount>, AppError> {
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
    ) -> Result<Vec<ServiceAccount>, AppError> {
        let api = Self::api(cluster, namespace).await?;
        let list = api
            .list(&ListParams::default())
            .await
            .map_err(|e| AppError::Kubernetes(e.to_string()))?;
        Ok(list.items)
    }

    pub async fn list_service_account_inventory(
        &self,
        cluster: &KubernetesClusterConfig,
        cluster_id: &str,
        namespace: Option<&str>,
    ) -> Result<Vec<ServiceAccountInventoryItem>, AppError> {
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
            .map(|item| {
                convert_kube_service_account_to_inventory(
                    item,
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
    ) -> Result<ServiceAccount, AppError> {
        let api: Api<ServiceAccount> =
            Api::namespaced(ClientFactory::get_client(cluster).await?, namespace);
        api.get(name)
            .await
            .map_err(|e| AppError::Kubernetes(e.to_string()))
    }

    pub async fn upsert(
        &self,
        cluster: &KubernetesClusterConfig,
        namespace: &str,
        item: &ServiceAccount,
    ) -> Result<ServiceAccount, AppError> {
        let api: Api<ServiceAccount> =
            Api::namespaced(ClientFactory::get_client(cluster).await?, namespace);
        let params = PatchParams::apply("mayyam").force();
        api.patch(
            item.metadata.name.as_ref().ok_or_else(|| {
                AppError::BadRequest("ServiceAccount.metadata.name required".into())
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
        let api: Api<ServiceAccount> =
            Api::namespaced(ClientFactory::get_client(cluster).await?, namespace);
        api.delete(name, &DeleteParams::default())
            .await
            .map_err(|e| AppError::Kubernetes(e.to_string()))?;
        Ok(())
    }
}

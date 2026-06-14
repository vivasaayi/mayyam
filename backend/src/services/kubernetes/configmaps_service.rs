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
use crate::services::kubernetes::configmap_inventory::{
    ConfigMapInventoryItem, ConfigMapOwnerReferenceInventoryItem,
};
use k8s_openapi::api::core::v1::ConfigMap;
use kube::{
    api::{Api, DeleteParams, ListParams, Patch, PatchParams},
    ResourceExt,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Serialize, Deserialize)]
pub struct ConfigMapInfo {
    pub name: String,
    pub namespace: String,
    pub data_keys: Vec<String>,
    pub labels: Option<BTreeMap<String, String>>,
    pub annotations: Option<BTreeMap<String, String>>,
}

pub struct ConfigMapsService;

fn convert_kube_configmap_to_inventory(
    configmap: &ConfigMap,
    cluster_id: &str,
    current_namespace: &str,
    collected_at: chrono::DateTime<chrono::Utc>,
) -> ConfigMapInventoryItem {
    let namespace = configmap
        .namespace()
        .unwrap_or_else(|| current_namespace.to_string());
    let data = configmap.data.as_ref();
    let binary_data = configmap.binary_data.as_ref();
    let mut data_keys = data
        .map(|data| data.keys().cloned().collect::<Vec<_>>())
        .unwrap_or_default();
    let mut binary_data_keys = binary_data
        .map(|data| data.keys().cloned().collect::<Vec<_>>())
        .unwrap_or_default();
    data_keys.sort();
    binary_data_keys.sort();

    let text_bytes = data
        .map(|data| data.values().map(|value| value.len()).sum::<usize>())
        .unwrap_or_default();
    let binary_bytes = binary_data
        .map(|data| data.values().map(|value| value.0.len()).sum::<usize>())
        .unwrap_or_default();
    let owner_references = configmap
        .metadata
        .owner_references
        .as_ref()
        .map(|owners| {
            owners
                .iter()
                .map(|owner| ConfigMapOwnerReferenceInventoryItem {
                    kind: Some(owner.kind.clone()),
                    name: owner.name.clone(),
                    controller: owner.controller,
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    ConfigMapInventoryItem {
        cluster_id: cluster_id.to_string(),
        namespace,
        name: configmap.name_any(),
        labels: configmap.metadata.labels.clone().unwrap_or_default(),
        annotations: configmap.metadata.annotations.clone().unwrap_or_default(),
        data_keys,
        binary_data_keys,
        total_data_bytes: text_bytes + binary_bytes,
        immutable: configmap.immutable.unwrap_or(false),
        owner_references,
        created_at: configmap
            .metadata
            .creation_timestamp
            .as_ref()
            .map(|timestamp| timestamp.0),
        collected_at,
    }
}

impl ConfigMapsService {
    pub fn new() -> Self {
        Self
    }

    async fn api(
        cluster_config: &KubernetesClusterConfig,
        namespace: &str,
    ) -> Result<Api<ConfigMap>, AppError> {
        let client = ClientFactory::get_client(cluster_config).await?;
        let api = if namespace.is_empty() || namespace == "all" {
            Api::all(client)
        } else {
            Api::namespaced(client, namespace)
        };
        Ok(api)
    }

    pub async fn list(
        &self,
        cluster_config: &KubernetesClusterConfig,
        namespace: &str,
        label_selector: Option<String>,
        field_selector: Option<String>,
        limit: Option<u32>,
        continue_token: Option<String>,
    ) -> Result<Vec<ConfigMapInfo>, AppError> {
        let api = Self::api(cluster_config, namespace).await?;
        let mut lp = ListParams::default();
        if let Some(ls) = label_selector {
            lp = lp.labels(&ls);
        }
        if let Some(fs) = field_selector {
            lp = lp.fields(&fs);
        }
        if let Some(l) = limit {
            lp = lp.limit(l);
        }
        if let Some(ct) = continue_token {
            lp = lp.continue_token(&ct);
        }
        let cms = api
            .list(&lp)
            .await
            .map_err(|e| AppError::Kubernetes(e.to_string()))?;
        let mut out = Vec::new();
        for cm in cms {
            out.push(ConfigMapInfo {
                name: cm.name_any(),
                namespace: cm.namespace().unwrap_or_else(|| {
                    if namespace == "all" {
                        String::new()
                    } else {
                        namespace.to_string()
                    }
                }),
                data_keys: cm.data.unwrap_or_default().keys().cloned().collect(),
                labels: cm.metadata.labels.clone(),
                annotations: cm.metadata.annotations.clone(),
            });
        }
        Ok(out)
    }

    pub async fn list_configmap_inventory(
        &self,
        cluster_config: &KubernetesClusterConfig,
        cluster_id: &str,
        namespace: Option<&str>,
    ) -> Result<Vec<ConfigMapInventoryItem>, AppError> {
        let namespace = namespace
            .map(str::trim)
            .filter(|namespace| !namespace.is_empty());
        let namespace_arg = namespace.unwrap_or("");
        let fallback_namespace = namespace
            .filter(|namespace| *namespace != "all")
            .unwrap_or("");
        let collected_at = chrono::Utc::now();

        let api = Self::api(cluster_config, namespace_arg).await?;
        let configmaps = api
            .list(&ListParams::default())
            .await
            .map_err(|e| AppError::Kubernetes(e.to_string()))?;
        let mut inventory = configmaps
            .items
            .iter()
            .map(|configmap| {
                convert_kube_configmap_to_inventory(
                    configmap,
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
        cluster_config: &KubernetesClusterConfig,
        namespace: &str,
        name: &str,
    ) -> Result<ConfigMap, AppError> {
        let api = Self::api(cluster_config, namespace).await?;
        api.get(name)
            .await
            .map_err(|e| AppError::Kubernetes(e.to_string()))
    }

    pub async fn upsert(
        &self,
        cluster_config: &KubernetesClusterConfig,
        namespace: &str,
        name: &str,
        data: BTreeMap<String, String>,
        labels: Option<BTreeMap<String, String>>,
        annotations: Option<BTreeMap<String, String>>,
    ) -> Result<ConfigMap, AppError> {
        let api = Self::api(cluster_config, namespace).await?;

        // Try server-side apply merge patch
        let patch = serde_json::json!({
            "apiVersion": "v1",
            "kind": "ConfigMap",
            "metadata": { "name": name, "labels": labels, "annotations": annotations },
            "data": data,
        });
        let params = PatchParams::apply("mayyam").force();
        let res = api
            .patch(name, &params, &Patch::Apply(&patch))
            .await
            .map_err(|e| AppError::Kubernetes(e.to_string()))?;
        Ok(res)
    }

    pub async fn delete(
        &self,
        cluster_config: &KubernetesClusterConfig,
        namespace: &str,
        name: &str,
    ) -> Result<(), AppError> {
        let api = Self::api(cluster_config, namespace).await?;
        api.delete(name, &DeleteParams::default())
            .await
            .map_err(|e| AppError::Kubernetes(e.to_string()))?;
        Ok(())
    }
}

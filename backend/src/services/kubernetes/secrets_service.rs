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
use crate::services::kubernetes::secret_inventory::{
    SecretInventoryItem, SecretOwnerReferenceInventoryItem,
};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use k8s_openapi::api::core::v1::Secret;
use kube::{
    api::{Api, DeleteParams, ListParams, Patch, PatchParams},
    ResourceExt,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Serialize, Deserialize)]
pub struct SecretInfo {
    pub name: String,
    pub namespace: String,
    pub type_field: Option<String>,
    pub data_keys: Vec<String>,
    pub labels: Option<BTreeMap<String, String>>,
    pub annotations: Option<BTreeMap<String, String>>,
}

pub struct SecretsService;

fn convert_kube_secret_to_inventory(
    secret: &Secret,
    cluster_id: &str,
    current_namespace: &str,
    collected_at: chrono::DateTime<chrono::Utc>,
) -> SecretInventoryItem {
    let namespace = secret
        .namespace()
        .unwrap_or_else(|| current_namespace.to_string());
    let data = secret.data.as_ref();
    let string_data = secret.string_data.as_ref();
    let mut data_keys = data
        .map(|data| data.keys().cloned().collect::<Vec<_>>())
        .unwrap_or_default();
    let mut string_data_keys = string_data
        .map(|data| data.keys().cloned().collect::<Vec<_>>())
        .unwrap_or_default();
    data_keys.sort();
    string_data_keys.sort();

    let data_bytes = data
        .map(|data| data.values().map(|value| value.0.len()).sum::<usize>())
        .unwrap_or_default();
    let string_data_bytes = string_data
        .map(|data| data.values().map(|value| value.len()).sum::<usize>())
        .unwrap_or_default();
    let owner_references = secret
        .metadata
        .owner_references
        .as_ref()
        .map(|owners| {
            owners
                .iter()
                .map(|owner| SecretOwnerReferenceInventoryItem {
                    kind: Some(owner.kind.clone()),
                    name: owner.name.clone(),
                    controller: owner.controller,
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    SecretInventoryItem {
        cluster_id: cluster_id.to_string(),
        namespace,
        name: secret.name_any(),
        secret_type: secret.type_.clone(),
        labels: secret.metadata.labels.clone().unwrap_or_default(),
        annotations: secret.metadata.annotations.clone().unwrap_or_default(),
        data_keys,
        string_data_keys,
        total_data_bytes: data_bytes + string_data_bytes,
        immutable: secret.immutable.unwrap_or(false),
        owner_references,
        created_at: secret
            .metadata
            .creation_timestamp
            .as_ref()
            .map(|timestamp| timestamp.0),
        collected_at,
    }
}

impl SecretsService {
    pub fn new() -> Self {
        Self
    }

    async fn api(
        cluster_config: &KubernetesClusterConfig,
        namespace: &str,
    ) -> Result<Api<Secret>, AppError> {
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
    ) -> Result<Vec<SecretInfo>, AppError> {
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
        let items = api
            .list(&lp)
            .await
            .map_err(|e| AppError::Kubernetes(e.to_string()))?;

        Ok(items
            .into_iter()
            .map(|s| SecretInfo {
                name: s.name_any(),
                namespace: s.namespace().unwrap_or_else(|| {
                    if namespace == "all" {
                        String::new()
                    } else {
                        namespace.to_string()
                    }
                }),
                type_field: s.type_.clone(),
                data_keys: s.data.unwrap_or_default().keys().cloned().collect(),
                labels: s.metadata.labels.clone(),
                annotations: s.metadata.annotations.clone(),
            })
            .collect())
    }

    pub async fn list_secret_inventory(
        &self,
        cluster_config: &KubernetesClusterConfig,
        cluster_id: &str,
        namespace: Option<&str>,
    ) -> Result<Vec<SecretInventoryItem>, AppError> {
        let namespace = namespace
            .map(str::trim)
            .filter(|namespace| !namespace.is_empty());
        let namespace_arg = namespace.unwrap_or("");
        let fallback_namespace = namespace
            .filter(|namespace| *namespace != "all")
            .unwrap_or("");
        let collected_at = chrono::Utc::now();

        let api = Self::api(cluster_config, namespace_arg).await?;
        let secrets = api
            .list(&ListParams::default())
            .await
            .map_err(|e| AppError::Kubernetes(e.to_string()))?;
        let mut inventory = secrets
            .items
            .iter()
            .map(|secret| {
                convert_kube_secret_to_inventory(
                    secret,
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

    pub async fn get_redacted(
        &self,
        cluster_config: &KubernetesClusterConfig,
        namespace: &str,
        name: &str,
    ) -> Result<Secret, AppError> {
        let api = Self::api(cluster_config, namespace).await?;
        let mut s = api
            .get(name)
            .await
            .map_err(|e| AppError::Kubernetes(e.to_string()))?;
        // Redact values
        if let Some(ref mut data) = s.data {
            for (_k, v) in data.iter_mut() {
                // Overwrite with redaction marker bytes; API serializes as base64 automatically
                *v = k8s_openapi::ByteString(b"***".to_vec());
            }
        }
        Ok(s)
    }

    pub async fn upsert_plaintext(
        &self,
        cluster_config: &KubernetesClusterConfig,
        namespace: &str,
        name: &str,
        type_field: Option<String>,
        data: BTreeMap<String, String>, // plaintext values; will be b64 encoded
        labels: Option<BTreeMap<String, String>>,
        annotations: Option<BTreeMap<String, String>>,
    ) -> Result<Secret, AppError> {
        let api = Self::api(cluster_config, namespace).await?;
        let encoded: BTreeMap<String, String> = data
            .into_iter()
            .map(|(k, v)| (k, BASE64.encode(v.as_bytes())))
            .collect();

        let patch = serde_json::json!({
            "apiVersion": "v1",
            "kind": "Secret",
            "metadata": { "name": name, "labels": labels, "annotations": annotations },
            "type": type_field,
            "data": encoded,
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

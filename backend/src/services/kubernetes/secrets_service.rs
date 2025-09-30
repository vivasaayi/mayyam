use crate::errors::AppError;
use crate::models::cluster::KubernetesClusterConfig;
use crate::services::kubernetes::client::ClientFactory;
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

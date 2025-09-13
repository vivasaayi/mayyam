use crate::errors::AppError;
use crate::models::cluster::KubernetesClusterConfig;
use crate::services::kubernetes::client::ClientFactory;
use k8s_openapi::api::core::v1::ConfigMap;
use kube::{api::{Api, ListParams, PatchParams, Patch, DeleteParams}, ResourceExt};
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

impl ConfigMapsService {
    pub fn new() -> Self { Self }

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
        if let Some(ls) = label_selector { lp = lp.labels(&ls); }
        if let Some(fs) = field_selector { lp = lp.fields(&fs); }
        if let Some(l) = limit { lp = lp.limit(l); }
        if let Some(ct) = continue_token { lp = lp.continue_token(&ct); }
        let cms = api
            .list(&lp)
            .await
            .map_err(|e| AppError::Kubernetes(e.to_string()))?;
        let mut out = Vec::new();
        for cm in cms {
            out.push(ConfigMapInfo {
                name: cm.name_any(),
                namespace: cm.namespace().unwrap_or_else(|| if namespace == "all" { String::new() } else { namespace.to_string() }),
                data_keys: cm.data.unwrap_or_default().keys().cloned().collect(),
                labels: cm.metadata.labels.clone(),
                annotations: cm.metadata.annotations.clone(),
            });
        }
        Ok(out)
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
        api
            .delete(name, &DeleteParams::default())
            .await
            .map_err(|e| AppError::Kubernetes(e.to_string()))?;
        Ok(())
    }
}

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
use k8s_openapi::apiextensions_apiserver::pkg::apis::apiextensions::v1::CustomResourceDefinition;
use kube::{
    api::{Api, DynamicObject, GroupVersionKind, ListParams},
    discovery::{ApiGroup, ApiResource, Discovery, Scope},
};
use serde_json::Value;

pub struct CrdsService;

impl CrdsService {
    pub async fn list_crds(
        &self,
        cluster_config: &KubernetesClusterConfig,
    ) -> Result<Vec<Value>, AppError> {
        let client = ClientFactory::get_client(cluster_config).await?;
        let crds: Api<CustomResourceDefinition> = Api::all(client);

        let crd_list = crds
            .list(&ListParams::default())
            .await
            .map_err(|e| AppError::ExternalService(format!("Failed to list CRDs: {}", e)))?;

        let mut formatted_crds = Vec::new();
        for crd in crd_list {
            if let Ok(value) = serde_json::to_value(&crd) {
                formatted_crds.push(value);
            }
        }

        Ok(formatted_crds)
    }

    pub async fn get_crd_details(
        &self,
        cluster_config: &KubernetesClusterConfig,
        crd_name: &str,
    ) -> Result<Value, AppError> {
        let client = ClientFactory::get_client(cluster_config).await?;
        let crds: Api<CustomResourceDefinition> = Api::all(client);

        let crd = crds.get(crd_name).await.map_err(|e| {
            AppError::ExternalService(format!("Failed to get CRD details: {}", e))
        })?;

        serde_json::to_value(&crd)
            .map_err(|e| AppError::Internal(format!("Failed to serialize CRD details: {}", e)))
    }

    /// Generic fallback for dynamically dealing with custom resources based on their GroupVersionKind
    pub async fn list_custom_resources(
        &self,
        cluster_config: &KubernetesClusterConfig,
        group: &str,
        version: &str,
        plural: &str,
        namespace: Option<&str>,
    ) -> Result<Vec<Value>, AppError> {
        let client = ClientFactory::get_client(cluster_config).await?;
        let discovery = Discovery::new(client.clone())
            .run()
            .await
            .map_err(|e| AppError::ExternalService(format!("Discovery failed: {}", e)))?;

        let gvk = GroupVersionKind::gvk(group, version, "");
        
        // Use discovery to find the exact APIResource matching the requested group/version/plural
        let _api_group = discovery
            .resolve_gvk(&gvk)
            .ok_or_else(|| AppError::NotFound(format!("ApiGroup {}/{} not found", group, version)))?;

        // Fallback or explicit check for resource by plural if gvk resolution doesn't match perfectly.
        // We really want the resource by plural name since that maps to the REST endpoint.
        let mut target_ar: Option<(ApiResource, kube::discovery::ApiCapabilities)> = None;
        if let Some(group_info) = discovery.get(group) {
            for (ar, caps) in group_info.recommended_resources() {
                if ar.plural == plural && ar.version == version && ar.group == group {
                    target_ar = Some((ar, caps));
                    break;
                }
            }
        }

        let (ar, caps) = target_ar.ok_or_else(|| {
            AppError::NotFound(format!("Resource {} not found in {}/{}", plural, group, version))
        })?;


        let api: Api<DynamicObject> = match namespace {
            Some(ns) if caps.scope == Scope::Namespaced => Api::namespaced_with(client.clone(), ns, &ar),
            _ => Api::all_with(client.clone(), &ar),
        };

        let list = api.list(&ListParams::default()).await.map_err(|e| {
            AppError::ExternalService(format!("Failed to list CustomResources: {}", e))
        })?;

        let mut items = Vec::new();
        for item in list {
            if let Ok(value) = serde_json::to_value(&item) {
                items.push(value);
            }
        }

        Ok(items)
    }
}

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
use crate::services::kubernetes::ingress_inventory::{
    IngressBackendInventoryItem, IngressInventoryItem, IngressLoadBalancerInventoryItem,
    IngressPathInventoryItem, IngressTlsInventoryItem,
};
use chrono::Utc;
use k8s_openapi::api::networking::v1::Ingress;
use kube::api::{DeleteParams, ListParams, Patch, PatchParams};
use kube::{Api, ResourceExt};

pub struct IngressService;

fn backend_to_inventory(
    backend: &k8s_openapi::api::networking::v1::IngressBackend,
) -> IngressBackendInventoryItem {
    let service_port = backend.service.as_ref().and_then(|service| {
        service.port.as_ref().and_then(|port| {
            port.name
                .clone()
                .or_else(|| port.number.map(|number| number.to_string()))
        })
    });

    IngressBackendInventoryItem {
        service_name: backend.service.as_ref().map(|service| service.name.clone()),
        service_port,
        resource_api_group: backend
            .resource
            .as_ref()
            .and_then(|resource| resource.api_group.clone()),
        resource_kind: backend
            .resource
            .as_ref()
            .map(|resource| resource.kind.clone()),
        resource_name: backend
            .resource
            .as_ref()
            .map(|resource| resource.name.clone()),
    }
}

fn convert_kube_ingress_to_ingress_inventory(
    ingress: &Ingress,
    cluster_id: &str,
    current_namespace: &str,
    collected_at: chrono::DateTime<Utc>,
) -> IngressInventoryItem {
    let namespace = ingress
        .namespace()
        .unwrap_or_else(|| current_namespace.to_string());
    let spec = ingress.spec.as_ref();
    let status = ingress.status.as_ref();
    let annotations = ingress.metadata.annotations.clone().unwrap_or_default();
    let legacy_class_annotation = annotations.get("kubernetes.io/ingress.class").cloned();
    let hosts = spec
        .and_then(|spec| spec.rules.as_ref())
        .map(|rules| {
            rules
                .iter()
                .filter_map(|rule| rule.host.clone())
                .filter(|host| !host.trim().is_empty())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let paths = spec
        .and_then(|spec| spec.rules.as_ref())
        .map(|rules| {
            rules
                .iter()
                .flat_map(|rule| {
                    let host = rule.host.clone();
                    rule.http
                        .as_ref()
                        .map(|http| {
                            http.paths
                                .iter()
                                .map(move |path| IngressPathInventoryItem {
                                    host: host.clone(),
                                    path: path.path.clone(),
                                    path_type: path.path_type.clone(),
                                    backend: backend_to_inventory(&path.backend),
                                })
                                .collect::<Vec<_>>()
                        })
                        .unwrap_or_default()
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let tls = spec
        .and_then(|spec| spec.tls.as_ref())
        .map(|tls_entries| {
            tls_entries
                .iter()
                .map(|tls| IngressTlsInventoryItem {
                    hosts: tls.hosts.clone().unwrap_or_default(),
                    secret_name: tls.secret_name.clone(),
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let default_backend = spec
        .and_then(|spec| spec.default_backend.as_ref())
        .map(backend_to_inventory);
    let load_balancer_ingress = status
        .and_then(|status| status.load_balancer.as_ref())
        .and_then(|load_balancer| load_balancer.ingress.as_ref())
        .map(|ingress| {
            ingress
                .iter()
                .map(|entry| IngressLoadBalancerInventoryItem {
                    ip: entry.ip.clone(),
                    hostname: entry.hostname.clone(),
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    IngressInventoryItem {
        cluster_id: cluster_id.to_string(),
        namespace,
        name: ingress.name_any(),
        ingress_class_name: spec.and_then(|spec| spec.ingress_class_name.clone()),
        legacy_class_annotation,
        labels: ingress.metadata.labels.clone().unwrap_or_default(),
        annotations,
        hosts,
        paths,
        tls,
        default_backend,
        load_balancer_ingress,
        created_at: ingress
            .metadata
            .creation_timestamp
            .as_ref()
            .map(|timestamp| timestamp.0),
        collected_at,
    }
}

impl IngressService {
    pub fn new() -> Self {
        Self
    }

    async fn api(
        cluster: &KubernetesClusterConfig,
        namespace: &str,
    ) -> Result<Api<Ingress>, AppError> {
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
    ) -> Result<Vec<Ingress>, AppError> {
        let api = Self::api(cluster, namespace).await?;
        let list = api
            .list(&ListParams::default())
            .await
            .map_err(|e| AppError::Kubernetes(e.to_string()))?;
        Ok(list.items)
    }

    pub async fn list_ingress_inventory(
        &self,
        cluster: &KubernetesClusterConfig,
        cluster_id: &str,
        namespace: Option<&str>,
    ) -> Result<Vec<IngressInventoryItem>, AppError> {
        let namespace = namespace
            .map(str::trim)
            .filter(|namespace| !namespace.is_empty());
        let namespace_arg = namespace.unwrap_or("");
        let api = Self::api(cluster, namespace_arg).await?;
        let collected_at = Utc::now();
        let list = api
            .list(&ListParams::default())
            .await
            .map_err(|e| AppError::Kubernetes(e.to_string()))?;
        let fallback_namespace = namespace
            .filter(|namespace| *namespace != "all")
            .unwrap_or("");

        Ok(list
            .items
            .iter()
            .map(|ingress| {
                convert_kube_ingress_to_ingress_inventory(
                    ingress,
                    cluster_id,
                    fallback_namespace,
                    collected_at,
                )
            })
            .collect())
    }

    pub async fn get(
        &self,
        cluster: &KubernetesClusterConfig,
        namespace: &str,
        name: &str,
    ) -> Result<Ingress, AppError> {
        let api: Api<Ingress> =
            Api::namespaced(ClientFactory::get_client(cluster).await?, namespace);
        api.get(name)
            .await
            .map_err(|e| AppError::Kubernetes(e.to_string()))
    }

    pub async fn upsert(
        &self,
        cluster: &KubernetesClusterConfig,
        namespace: &str,
        item: &Ingress,
    ) -> Result<Ingress, AppError> {
        let api: Api<Ingress> =
            Api::namespaced(ClientFactory::get_client(cluster).await?, namespace);
        let params = PatchParams::apply("mayyam").force();
        api.patch(
            item.metadata
                .name
                .as_ref()
                .ok_or_else(|| AppError::BadRequest("Ingress.metadata.name required".into()))?,
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
        let api: Api<Ingress> =
            Api::namespaced(ClientFactory::get_client(cluster).await?, namespace);
        api.delete(name, &DeleteParams::default())
            .await
            .map_err(|e| AppError::Kubernetes(e.to_string()))?;
        Ok(())
    }
}

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
use crate::services::kubernetes::endpoint_slice_inventory::EndpointSliceInventoryItem;
use crate::services::kubernetes::endpoints_inventory::{
    EndpointAddressInventoryItem, EndpointPortInventoryItem, EndpointsInventoryItem,
};
use k8s_openapi::api::core::v1::Endpoints;
use k8s_openapi::api::discovery::v1::EndpointSlice;
use kube::api::{DeleteParams, ListParams, Patch, PatchParams};
use kube::{Api, ResourceExt};

pub struct EndpointsService;

fn convert_target_ref(
    target_ref: Option<&k8s_openapi::api::core::v1::ObjectReference>,
    fallback_namespace: &str,
) -> (Option<String>, Option<String>, Option<String>) {
    target_ref
        .map(|target_ref| {
            (
                target_ref.kind.clone(),
                target_ref
                    .namespace
                    .clone()
                    .or_else(|| Some(fallback_namespace.to_string())),
                target_ref.name.clone(),
            )
        })
        .unwrap_or((None, None, None))
}

fn convert_core_endpoint_address(
    address: &k8s_openapi::api::core::v1::EndpointAddress,
    namespace: &str,
    ready: bool,
) -> EndpointAddressInventoryItem {
    let (target_kind, target_namespace, target_name) =
        convert_target_ref(address.target_ref.as_ref(), namespace);

    EndpointAddressInventoryItem {
        address: address.ip.clone(),
        hostname: address.hostname.clone(),
        node_name: address.node_name.clone(),
        target_kind,
        target_namespace,
        target_name,
        ready: Some(ready),
        serving: Some(ready),
        terminating: Some(false),
        zone: None,
    }
}

fn convert_slice_endpoint_address(
    endpoint: &k8s_openapi::api::discovery::v1::Endpoint,
    address: &str,
    namespace: &str,
) -> EndpointAddressInventoryItem {
    let conditions = endpoint.conditions.as_ref();
    let (target_kind, target_namespace, target_name) =
        convert_target_ref(endpoint.target_ref.as_ref(), namespace);

    EndpointAddressInventoryItem {
        address: address.to_string(),
        hostname: endpoint.hostname.clone(),
        node_name: endpoint.node_name.clone(),
        target_kind,
        target_namespace,
        target_name,
        ready: conditions.and_then(|conditions| conditions.ready),
        serving: conditions.and_then(|conditions| conditions.serving),
        terminating: conditions.and_then(|conditions| conditions.terminating),
        zone: endpoint.zone.clone(),
    }
}

fn convert_kube_endpoints_to_inventory(
    endpoints: &Endpoints,
    cluster_id: &str,
    current_namespace: &str,
    collected_at: chrono::DateTime<chrono::Utc>,
) -> EndpointsInventoryItem {
    let namespace = endpoints
        .namespace()
        .unwrap_or_else(|| current_namespace.to_string());
    let mut ports = Vec::new();
    let mut ready_addresses = Vec::new();
    let mut not_ready_addresses = Vec::new();

    for subset in endpoints.subsets.as_ref().into_iter().flatten() {
        if let Some(subset_ports) = subset.ports.as_ref() {
            ports.extend(subset_ports.iter().map(|port| EndpointPortInventoryItem {
                name: port.name.clone(),
                port: Some(port.port),
                protocol: port.protocol.clone(),
                app_protocol: port.app_protocol.clone(),
            }));
        }
        if let Some(addresses) = subset.addresses.as_ref() {
            ready_addresses.extend(
                addresses
                    .iter()
                    .map(|address| convert_core_endpoint_address(address, &namespace, true)),
            );
        }
        if let Some(addresses) = subset.not_ready_addresses.as_ref() {
            not_ready_addresses.extend(
                addresses
                    .iter()
                    .map(|address| convert_core_endpoint_address(address, &namespace, false)),
            );
        }
    }

    EndpointsInventoryItem {
        cluster_id: cluster_id.to_string(),
        namespace,
        name: endpoints.name_any(),
        source: "Endpoints".to_string(),
        service_name: endpoints.metadata.name.clone(),
        address_type: None,
        labels: endpoints.metadata.labels.clone().unwrap_or_default(),
        annotations: endpoints.metadata.annotations.clone().unwrap_or_default(),
        ports,
        ready_addresses,
        not_ready_addresses,
        created_at: endpoints
            .metadata
            .creation_timestamp
            .as_ref()
            .map(|timestamp| timestamp.0),
        collected_at,
    }
}

fn convert_kube_endpoint_slice_to_inventory(
    slice: &EndpointSlice,
    cluster_id: &str,
    current_namespace: &str,
    collected_at: chrono::DateTime<chrono::Utc>,
) -> EndpointsInventoryItem {
    let namespace = slice
        .namespace()
        .unwrap_or_else(|| current_namespace.to_string());
    let labels = slice.metadata.labels.clone().unwrap_or_default();
    let service_name = labels.get("kubernetes.io/service-name").cloned();
    let ports = slice
        .ports
        .as_ref()
        .map(|ports| {
            ports
                .iter()
                .map(|port| EndpointPortInventoryItem {
                    name: port.name.clone(),
                    port: port.port,
                    protocol: port.protocol.clone(),
                    app_protocol: port.app_protocol.clone(),
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let mut ready_addresses = Vec::new();
    let mut not_ready_addresses = Vec::new();

    for endpoint in &slice.endpoints {
        let conditions = endpoint.conditions.as_ref();
        let ready = conditions.and_then(|conditions| conditions.ready) != Some(false);
        let terminating = conditions
            .and_then(|conditions| conditions.terminating)
            .unwrap_or(false);
        let converted = endpoint
            .addresses
            .iter()
            .map(|address| convert_slice_endpoint_address(endpoint, address, &namespace));
        if ready && !terminating {
            ready_addresses.extend(converted);
        } else {
            not_ready_addresses.extend(converted);
        }
    }

    EndpointsInventoryItem {
        cluster_id: cluster_id.to_string(),
        namespace,
        name: slice.name_any(),
        source: "EndpointSlice".to_string(),
        service_name,
        address_type: Some(slice.address_type.clone()),
        labels,
        annotations: slice.metadata.annotations.clone().unwrap_or_default(),
        ports,
        ready_addresses,
        not_ready_addresses,
        created_at: slice
            .metadata
            .creation_timestamp
            .as_ref()
            .map(|timestamp| timestamp.0),
        collected_at,
    }
}

fn convert_kube_endpoint_slice_to_endpoint_slice_inventory(
    slice: &EndpointSlice,
    cluster_id: &str,
    current_namespace: &str,
    collected_at: chrono::DateTime<chrono::Utc>,
) -> EndpointSliceInventoryItem {
    let item = convert_kube_endpoint_slice_to_inventory(
        slice,
        cluster_id,
        current_namespace,
        collected_at,
    );

    EndpointSliceInventoryItem {
        cluster_id: item.cluster_id,
        namespace: item.namespace,
        name: item.name,
        service_name: item.service_name,
        address_type: item.address_type.unwrap_or_else(|| "Unknown".to_string()),
        labels: item.labels,
        annotations: item.annotations,
        ports: item.ports,
        ready_addresses: item.ready_addresses,
        not_ready_addresses: item.not_ready_addresses,
        created_at: item.created_at,
        collected_at: item.collected_at,
    }
}

impl EndpointsService {
    pub fn new() -> Self {
        Self
    }

    async fn endpoints_api(
        cluster: &KubernetesClusterConfig,
        namespace: &str,
    ) -> Result<Api<Endpoints>, AppError> {
        let client = ClientFactory::get_client(cluster).await?;
        Ok(if namespace.is_empty() || namespace == "all" {
            Api::all(client)
        } else {
            Api::namespaced(client, namespace)
        })
    }

    async fn endpoint_slice_api(
        cluster: &KubernetesClusterConfig,
        namespace: &str,
    ) -> Result<Api<EndpointSlice>, AppError> {
        let client = ClientFactory::get_client(cluster).await?;
        Ok(if namespace.is_empty() || namespace == "all" {
            Api::all(client)
        } else {
            Api::namespaced(client, namespace)
        })
    }

    pub async fn list_endpoints(
        &self,
        cluster: &KubernetesClusterConfig,
        namespace: &str,
    ) -> Result<Vec<Endpoints>, AppError> {
        let api = Self::endpoints_api(cluster, namespace).await?;
        let list = api
            .list(&ListParams::default())
            .await
            .map_err(|e| AppError::Kubernetes(e.to_string()))?;
        Ok(list.items)
    }

    pub async fn list_endpoint_slices(
        &self,
        cluster: &KubernetesClusterConfig,
        namespace: &str,
    ) -> Result<Vec<EndpointSlice>, AppError> {
        let api = Self::endpoint_slice_api(cluster, namespace).await?;
        let list = api
            .list(&ListParams::default())
            .await
            .map_err(|e| AppError::Kubernetes(e.to_string()))?;
        Ok(list.items)
    }

    pub async fn list_endpoints_inventory(
        &self,
        cluster: &KubernetesClusterConfig,
        cluster_id: &str,
        namespace: Option<&str>,
    ) -> Result<Vec<EndpointsInventoryItem>, AppError> {
        let namespace = namespace
            .map(str::trim)
            .filter(|namespace| !namespace.is_empty());
        let namespace_arg = namespace.unwrap_or("");
        let collected_at = chrono::Utc::now();
        let fallback_namespace = namespace
            .filter(|namespace| *namespace != "all")
            .unwrap_or("");

        let endpoints_api = Self::endpoints_api(cluster, namespace_arg).await?;
        let endpoints = endpoints_api
            .list(&ListParams::default())
            .await
            .map_err(|e| AppError::Kubernetes(e.to_string()))?;
        let endpoint_slice_api = Self::endpoint_slice_api(cluster, namespace_arg).await?;
        let endpoint_slices = endpoint_slice_api
            .list(&ListParams::default())
            .await
            .map_err(|e| AppError::Kubernetes(e.to_string()))?;

        let mut inventory = endpoints
            .items
            .iter()
            .map(|endpoints| {
                convert_kube_endpoints_to_inventory(
                    endpoints,
                    cluster_id,
                    fallback_namespace,
                    collected_at,
                )
            })
            .collect::<Vec<_>>();
        inventory.extend(endpoint_slices.items.iter().map(|slice| {
            convert_kube_endpoint_slice_to_inventory(
                slice,
                cluster_id,
                fallback_namespace,
                collected_at,
            )
        }));
        inventory.sort_by(|left, right| {
            (
                left.namespace.as_str(),
                left.service_name.as_deref().unwrap_or(""),
                left.source.as_str(),
                left.name.as_str(),
            )
                .cmp(&(
                    right.namespace.as_str(),
                    right.service_name.as_deref().unwrap_or(""),
                    right.source.as_str(),
                    right.name.as_str(),
                ))
        });
        Ok(inventory)
    }

    pub async fn list_endpoint_slice_inventory(
        &self,
        cluster: &KubernetesClusterConfig,
        cluster_id: &str,
        namespace: Option<&str>,
    ) -> Result<Vec<EndpointSliceInventoryItem>, AppError> {
        let namespace = namespace
            .map(str::trim)
            .filter(|namespace| !namespace.is_empty());
        let namespace_arg = namespace.unwrap_or("");
        let collected_at = chrono::Utc::now();
        let fallback_namespace = namespace
            .filter(|namespace| *namespace != "all")
            .unwrap_or("");

        let endpoint_slice_api = Self::endpoint_slice_api(cluster, namespace_arg).await?;
        let endpoint_slices = endpoint_slice_api
            .list(&ListParams::default())
            .await
            .map_err(|e| AppError::Kubernetes(e.to_string()))?;

        let mut inventory = endpoint_slices
            .items
            .iter()
            .map(|slice| {
                convert_kube_endpoint_slice_to_endpoint_slice_inventory(
                    slice,
                    cluster_id,
                    fallback_namespace,
                    collected_at,
                )
            })
            .collect::<Vec<_>>();
        inventory.sort_by(|left, right| {
            (
                left.namespace.as_str(),
                left.service_name.as_deref().unwrap_or(""),
                left.name.as_str(),
            )
                .cmp(&(
                    right.namespace.as_str(),
                    right.service_name.as_deref().unwrap_or(""),
                    right.name.as_str(),
                ))
        });
        Ok(inventory)
    }

    pub async fn get_endpoints(
        &self,
        cluster: &KubernetesClusterConfig,
        namespace: &str,
        name: &str,
    ) -> Result<Endpoints, AppError> {
        let api: Api<Endpoints> =
            Api::namespaced(ClientFactory::get_client(cluster).await?, namespace);
        api.get(name)
            .await
            .map_err(|e| AppError::Kubernetes(e.to_string()))
    }

    pub async fn upsert_endpoints(
        &self,
        cluster: &KubernetesClusterConfig,
        namespace: &str,
        item: &Endpoints,
    ) -> Result<Endpoints, AppError> {
        let api: Api<Endpoints> =
            Api::namespaced(ClientFactory::get_client(cluster).await?, namespace);
        let params = PatchParams::apply("mayyam").force();
        api.patch(
            item.metadata
                .name
                .as_ref()
                .ok_or_else(|| AppError::BadRequest("Endpoints.metadata.name required".into()))?,
            &params,
            &Patch::Apply(item),
        )
        .await
        .map_err(|e| AppError::Kubernetes(e.to_string()))
    }

    pub async fn delete_endpoints(
        &self,
        cluster: &KubernetesClusterConfig,
        namespace: &str,
        name: &str,
    ) -> Result<(), AppError> {
        let api: Api<Endpoints> =
            Api::namespaced(ClientFactory::get_client(cluster).await?, namespace);
        api.delete(name, &DeleteParams::default())
            .await
            .map_err(|e| AppError::Kubernetes(e.to_string()))?;
        Ok(())
    }
}

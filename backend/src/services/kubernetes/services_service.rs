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

use chrono::Utc;
use k8s_openapi::api::core::v1::Service;
use k8s_openapi::apimachinery::pkg::util::intstr::IntOrString;
use kube::api::ListParams;
use kube::config::{Config as KubeConfig, KubeConfigOptions, Kubeconfig};
use kube::{Api, Client, ResourceExt};
use serde::{Deserialize, Serialize};

use crate::errors::AppError;
use crate::models::cluster::KubernetesClusterConfig;
use crate::services::kubernetes::service_inventory::{
    ServiceInventoryItem, ServiceLoadBalancerIngressInventoryItem, ServicePortInventoryItem,
};

#[derive(Debug, Serialize, Deserialize)]
pub struct ServicePortInfo {
    pub name: Option<String>,
    pub port: i32,
    pub target_port: Option<String>, // k8s_openapi::apimachinery::pkg::util::intstr::IntOrString
    pub protocol: Option<String>,
    pub node_port: Option<i32>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ServiceInfo {
    pub name: String,
    pub namespace: String,
    pub service_type: String,
    pub cluster_ip: Option<String>,
    pub external_ips: Vec<String>,
    pub ports: Vec<ServicePortInfo>,
    pub age: String,
    // pub selector: Option<std::collections::BTreeMap<String, String>>, // Too detailed for list view
}

pub struct ServicesService;

fn convert_int_or_string(value: &IntOrString) -> String {
    match value {
        IntOrString::Int(value) => value.to_string(),
        IntOrString::String(value) => value.clone(),
    }
}

fn convert_kube_service_to_service_inventory(
    service: &Service,
    cluster_id: &str,
    current_namespace: &str,
    collected_at: chrono::DateTime<Utc>,
) -> ServiceInventoryItem {
    let namespace = service
        .namespace()
        .unwrap_or_else(|| current_namespace.to_string());
    let spec = service.spec.as_ref();
    let status = service.status.as_ref();
    let ports = spec
        .and_then(|spec| spec.ports.as_ref())
        .map(|ports| {
            ports
                .iter()
                .map(|port| ServicePortInventoryItem {
                    name: port.name.clone(),
                    port: port.port,
                    target_port: port.target_port.as_ref().map(convert_int_or_string),
                    protocol: port.protocol.clone(),
                    node_port: port.node_port,
                    app_protocol: port.app_protocol.clone(),
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let load_balancer_ingress = status
        .and_then(|status| status.load_balancer.as_ref())
        .and_then(|load_balancer| load_balancer.ingress.as_ref())
        .map(|ingress| {
            ingress
                .iter()
                .map(|ingress| ServiceLoadBalancerIngressInventoryItem {
                    ip: ingress.ip.clone(),
                    hostname: ingress.hostname.clone(),
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    ServiceInventoryItem {
        cluster_id: cluster_id.to_string(),
        namespace,
        name: service.name_any(),
        service_type: spec
            .and_then(|spec| spec.type_.clone())
            .unwrap_or_else(|| "ClusterIP".to_string()),
        cluster_ip: spec
            .and_then(|spec| spec.cluster_ip.clone())
            .filter(|cluster_ip| !cluster_ip.is_empty() && cluster_ip != "None"),
        cluster_ips: spec
            .and_then(|spec| spec.cluster_ips.clone())
            .unwrap_or_default()
            .into_iter()
            .filter(|cluster_ip| !cluster_ip.is_empty() && cluster_ip != "None")
            .collect(),
        external_ips: spec
            .and_then(|spec| spec.external_ips.clone())
            .unwrap_or_default(),
        load_balancer_ip: spec.and_then(|spec| spec.load_balancer_ip.clone()),
        load_balancer_ingress,
        external_name: spec.and_then(|spec| spec.external_name.clone()),
        selector: spec
            .and_then(|spec| spec.selector.clone())
            .unwrap_or_default(),
        labels: service.metadata.labels.clone().unwrap_or_default(),
        annotations: service.metadata.annotations.clone().unwrap_or_default(),
        ports,
        session_affinity: spec.and_then(|spec| spec.session_affinity.clone()),
        ip_families: spec
            .and_then(|spec| spec.ip_families.clone())
            .unwrap_or_default(),
        ip_family_policy: spec.and_then(|spec| spec.ip_family_policy.clone()),
        internal_traffic_policy: spec.and_then(|spec| spec.internal_traffic_policy.clone()),
        external_traffic_policy: spec.and_then(|spec| spec.external_traffic_policy.clone()),
        allocate_load_balancer_node_ports: spec
            .and_then(|spec| spec.allocate_load_balancer_node_ports),
        publish_not_ready_addresses: spec
            .and_then(|spec| spec.publish_not_ready_addresses)
            .unwrap_or(false),
        created_at: service
            .metadata
            .creation_timestamp
            .as_ref()
            .map(|timestamp| timestamp.0),
        collected_at,
    }
}

impl ServicesService {
    pub fn new() -> Self {
        ServicesService {}
    }

    async fn get_kube_client(cluster_config: &KubernetesClusterConfig) -> Result<Client, AppError> {
        let kubeconfig = if let Some(path) = &cluster_config.kube_config_path {
            Kubeconfig::read_from(path).map_err(|e| {
                AppError::ExternalService(format!("Failed to read kubeconfig from path: {}", e))
            })?
        } else {
            let infer_config = kube::Config::infer().await.map_err(|e| {
                AppError::ExternalService(format!("Failed to infer Kubernetes config: {}", e))
            })?;
            return Client::try_from(infer_config).map_err(|e| {
                AppError::ExternalService(format!(
                    "Failed to create Kubernetes client from inferred config: {}",
                    e
                ))
            });
        };

        let client_config = KubeConfig::from_custom_kubeconfig(
            kubeconfig,
            &KubeConfigOptions {
                context: cluster_config.kube_context.clone(),
                cluster: None,
                user: None,
            },
        )
        .await
        .map_err(|e| {
            AppError::ExternalService(format!("Failed to create Kubernetes client config: {}", e))
        })?;

        Client::try_from(client_config).map_err(|e| {
            AppError::ExternalService(format!("Failed to create Kubernetes client: {}", e))
        })
    }

    pub async fn list_services(
        &self,
        cluster_config: &KubernetesClusterConfig,
        namespace: &str,
    ) -> Result<Vec<ServiceInfo>, AppError> {
        let client = Self::get_kube_client(cluster_config).await?;
        let api: Api<Service> = Api::namespaced(client, namespace);
        let lp = ListParams::default();
        let service_list = api.list(&lp).await.map_err(|e| {
            AppError::ExternalService(format!(
                "Failed to list services in namespace '{}': {}",
                namespace, e
            ))
        })?;

        let mut infos = Vec::new();
        for s in service_list {
            let name = s.name_any();
            let spec = s.spec.as_ref();

            let age = s.metadata.creation_timestamp.as_ref().map_or_else(
                || "Unknown".to_string(),
                |ts| {
                    let creation_time = ts.0;
                    let duration = Utc::now().signed_duration_since(creation_time);
                    if duration.num_days() > 0 {
                        format!("{}d", duration.num_days())
                    } else if duration.num_hours() > 0 {
                        format!("{}h", duration.num_hours())
                    } else if duration.num_minutes() > 0 {
                        format!("{}m", duration.num_minutes())
                    } else {
                        format!("{}s", duration.num_seconds())
                    }
                },
            );

            let ports_info = spec.and_then(|s_spec| s_spec.ports.as_ref()).map_or_else(Vec::new, |k8s_ports| {
                k8s_ports.iter().map(|p| ServicePortInfo {
                    name: p.name.clone(),
                    port: p.port,
                    target_port: p.target_port.as_ref().map(|tp| match tp {
                        k8s_openapi::apimachinery::pkg::util::intstr::IntOrString::Int(i) => i.to_string(),
                        k8s_openapi::apimachinery::pkg::util::intstr::IntOrString::String(s) => s.clone(),
                    }),
                    protocol: p.protocol.clone(),
                    node_port: p.node_port,
                }).collect()
            });

            let external_ips = spec
                .and_then(|s_spec| s_spec.external_ips.clone())
                .unwrap_or_default();

            infos.push(ServiceInfo {
                name,
                namespace: namespace.to_string(),
                service_type: spec
                    .and_then(|s_spec| s_spec.type_.clone())
                    .unwrap_or_else(|| "Unknown".to_string()),
                cluster_ip: spec
                    .and_then(|s_spec| s_spec.cluster_ip.clone())
                    .filter(|ip| ip != "None"),
                external_ips,
                ports: ports_info,
                age,
            });
        }
        Ok(infos)
    }

    pub async fn list_service_inventory(
        &self,
        cluster_config: &KubernetesClusterConfig,
        cluster_id: &str,
        namespace: Option<&str>,
    ) -> Result<Vec<ServiceInventoryItem>, AppError> {
        let namespace = namespace
            .map(str::trim)
            .filter(|namespace| !namespace.is_empty());
        let namespace_arg = namespace.unwrap_or("");
        let client = Self::get_kube_client(cluster_config).await?;
        let api: Api<Service> = if namespace_arg.is_empty() || namespace_arg == "all" {
            Api::all(client)
        } else {
            Api::namespaced(client, namespace_arg)
        };
        let collected_at = Utc::now();
        let list = api.list(&ListParams::default()).await.map_err(|e| {
            AppError::ExternalService(format!("Failed to list Service inventory: {}", e))
        })?;
        let fallback_namespace = namespace
            .filter(|namespace| *namespace != "all")
            .unwrap_or("");

        Ok(list
            .items
            .iter()
            .map(|service| {
                convert_kube_service_to_service_inventory(
                    service,
                    cluster_id,
                    fallback_namespace,
                    collected_at,
                )
            })
            .collect())
    }

    pub async fn get_service_details(
        &self,
        cluster_config: &KubernetesClusterConfig,
        namespace: &str,
        name: &str,
    ) -> Result<Service, AppError> {
        let client = Self::get_kube_client(cluster_config).await?;
        let api: Api<Service> = Api::namespaced(client, namespace);
        api.get(name).await.map_err(|e| {
            AppError::ExternalService(format!(
                "Failed to get service '{}' in namespace '{}': {}",
                name, namespace, e
            ))
        })
    }
}

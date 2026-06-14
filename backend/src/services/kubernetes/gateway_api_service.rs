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
use kube::{
    api::{Api, DynamicObject, ListParams},
    discovery::{verbs, ApiCapabilities, ApiGroup, ApiResource, Discovery, Scope},
    Client,
};
use serde_json::{json, Value};

use crate::errors::AppError;
use crate::models::cluster::KubernetesClusterConfig;
use crate::services::kubernetes::client::ClientFactory;
use crate::services::kubernetes::gateway_api_inventory::{
    GatewayApiConditionInventoryItem, GatewayApiInventoryItem, GatewayApiListenerInventoryItem,
    GatewayApiParentRefInventoryItem,
};

const GATEWAY_API_GROUP: &str = "gateway.networking.k8s.io";
const GATEWAY_API_VERSION: &str = "v1";

#[derive(Clone, Copy)]
struct GatewayApiResourceSpec {
    kind: &'static str,
    plural: &'static str,
}

const GATEWAY_API_RESOURCES: &[GatewayApiResourceSpec] = &[
    GatewayApiResourceSpec {
        kind: "GatewayClass",
        plural: "gatewayclasses",
    },
    GatewayApiResourceSpec {
        kind: "Gateway",
        plural: "gateways",
    },
    GatewayApiResourceSpec {
        kind: "HTTPRoute",
        plural: "httproutes",
    },
    GatewayApiResourceSpec {
        kind: "GRPCRoute",
        plural: "grpcroutes",
    },
];

pub struct GatewayApiService;

impl GatewayApiService {
    pub fn new() -> Self {
        Self
    }

    pub async fn list_gateway_api_inventory(
        &self,
        cluster: &KubernetesClusterConfig,
        cluster_id: &str,
        namespace: Option<&str>,
    ) -> Result<Vec<GatewayApiInventoryItem>, AppError> {
        let client = ClientFactory::get_client(cluster).await?;
        let discovery = Discovery::new(client.clone())
            .filter(&[GATEWAY_API_GROUP])
            .run()
            .await
            .map_err(|e| {
                AppError::ExternalService(format!("Gateway API discovery failed: {}", e))
            })?;

        let Some(group) = discovery.get(GATEWAY_API_GROUP) else {
            return Ok(Vec::new());
        };

        let namespace = namespace
            .map(str::trim)
            .filter(|namespace| !namespace.is_empty());
        let collected_at = Utc::now();
        let mut inventory = Vec::new();

        for spec in GATEWAY_API_RESOURCES {
            let Some((resource, capabilities)) = resolve_gateway_api_resource(group, *spec) else {
                continue;
            };
            if !capabilities.supports_operation(verbs::LIST) {
                continue;
            }

            let items = list_dynamic_resource(
                &client,
                &resource,
                &capabilities,
                cluster_id,
                namespace,
                collected_at,
            )
            .await?;
            inventory.extend(items);
        }

        inventory.sort_by(|left, right| {
            (
                left.kind.as_str(),
                left.namespace.as_deref().unwrap_or(""),
                left.name.as_str(),
            )
                .cmp(&(
                    right.kind.as_str(),
                    right.namespace.as_deref().unwrap_or(""),
                    right.name.as_str(),
                ))
        });
        Ok(inventory)
    }
}

fn resolve_gateway_api_resource(
    group: &ApiGroup,
    spec: GatewayApiResourceSpec,
) -> Option<(ApiResource, ApiCapabilities)> {
    group
        .recommended_resources()
        .into_iter()
        .find(|(resource, _)| {
            resource.group == GATEWAY_API_GROUP
                && resource.version == GATEWAY_API_VERSION
                && resource.kind == spec.kind
                && resource.plural == spec.plural
        })
}

async fn list_dynamic_resource(
    client: &Client,
    resource: &ApiResource,
    capabilities: &ApiCapabilities,
    cluster_id: &str,
    namespace: Option<&str>,
    collected_at: chrono::DateTime<Utc>,
) -> Result<Vec<GatewayApiInventoryItem>, AppError> {
    let api: Api<DynamicObject> = match namespace {
        Some(namespace) if namespace != "all" && capabilities.scope == Scope::Namespaced => {
            Api::namespaced_with(client.clone(), namespace, resource)
        }
        _ => Api::all_with(client.clone(), resource),
    };

    let list = api.list(&ListParams::default()).await.map_err(|e| {
        AppError::ExternalService(format!(
            "Failed to list Gateway API {}: {}",
            resource.kind, e
        ))
    })?;

    Ok(list
        .items
        .into_iter()
        .map(|item| convert_dynamic_object_to_inventory(item, cluster_id, resource, collected_at))
        .collect())
}

fn convert_dynamic_object_to_inventory(
    item: DynamicObject,
    cluster_id: &str,
    resource: &ApiResource,
    collected_at: chrono::DateTime<Utc>,
) -> GatewayApiInventoryItem {
    let spec = object_field(&item.data, "spec");
    let status = object_field(&item.data, "status");
    let api_version = item
        .types
        .as_ref()
        .map(|type_meta| type_meta.api_version.clone())
        .unwrap_or_else(|| resource.api_version.clone());
    let kind = item
        .types
        .as_ref()
        .map(|type_meta| type_meta.kind.clone())
        .unwrap_or_else(|| resource.kind.clone());

    GatewayApiInventoryItem {
        cluster_id: cluster_id.to_string(),
        api_version,
        kind,
        namespace: item.metadata.namespace.clone(),
        name: item.metadata.name.clone().unwrap_or_default(),
        gateway_class_name: string_field(&spec, "gatewayClassName"),
        labels: item.metadata.labels.clone().unwrap_or_default(),
        annotations: item.metadata.annotations.clone().unwrap_or_default(),
        listeners: listeners_from_spec(&spec),
        parent_refs: parent_refs_from_spec(&spec),
        address_count: status
            .get("addresses")
            .and_then(Value::as_array)
            .map(Vec::len)
            .unwrap_or(0),
        conditions: conditions_from_status(&status),
        spec,
        status,
        created_at: item
            .metadata
            .creation_timestamp
            .as_ref()
            .map(|timestamp| timestamp.0),
        collected_at,
    }
}

fn object_field(value: &Value, key: &str) -> Value {
    value.get(key).cloned().unwrap_or_else(|| json!({}))
}

fn string_field(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn int_field(value: &Value, key: &str) -> Option<i64> {
    value.get(key).and_then(Value::as_i64)
}

fn listeners_from_spec(spec: &Value) -> Vec<GatewayApiListenerInventoryItem> {
    spec.get("listeners")
        .and_then(Value::as_array)
        .map(|listeners| {
            listeners
                .iter()
                .filter_map(listener_from_value)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

fn listener_from_value(listener: &Value) -> Option<GatewayApiListenerInventoryItem> {
    if !listener.is_object() {
        return None;
    }

    let tls = listener.get("tls").unwrap_or(&Value::Null);
    Some(GatewayApiListenerInventoryItem {
        name: string_field(listener, "name"),
        protocol: string_field(listener, "protocol"),
        hostname: string_field(listener, "hostname"),
        tls_mode: string_field(tls, "mode"),
        certificate_ref_count: tls
            .get("certificateRefs")
            .and_then(Value::as_array)
            .map(Vec::len)
            .unwrap_or(0),
    })
}

fn parent_refs_from_spec(spec: &Value) -> Vec<GatewayApiParentRefInventoryItem> {
    spec.get("parentRefs")
        .and_then(Value::as_array)
        .map(|parent_refs| {
            parent_refs
                .iter()
                .filter_map(parent_ref_from_value)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

fn parent_ref_from_value(parent_ref: &Value) -> Option<GatewayApiParentRefInventoryItem> {
    if !parent_ref.is_object() {
        return None;
    }

    Some(GatewayApiParentRefInventoryItem {
        group: string_field(parent_ref, "group"),
        kind: string_field(parent_ref, "kind"),
        namespace: string_field(parent_ref, "namespace"),
        name: string_field(parent_ref, "name").unwrap_or_default(),
        section_name: string_field(parent_ref, "sectionName"),
        port: int_field(parent_ref, "port"),
    })
}

fn conditions_from_status(status: &Value) -> Vec<GatewayApiConditionInventoryItem> {
    let mut conditions = Vec::new();
    extend_conditions(status.get("conditions"), &mut conditions);

    if let Some(parents) = status.get("parents").and_then(Value::as_array) {
        for parent in parents {
            extend_conditions(parent.get("conditions"), &mut conditions);
        }
    }

    conditions
}

fn extend_conditions(
    value: Option<&Value>,
    conditions: &mut Vec<GatewayApiConditionInventoryItem>,
) {
    if let Some(items) = value.and_then(Value::as_array) {
        conditions.extend(items.iter().filter_map(condition_from_value));
    }
}

fn condition_from_value(condition: &Value) -> Option<GatewayApiConditionInventoryItem> {
    Some(GatewayApiConditionInventoryItem {
        type_: string_field(condition, "type")?,
        status: string_field(condition, "status")?,
        reason: string_field(condition, "reason"),
        message: string_field(condition, "message"),
    })
}

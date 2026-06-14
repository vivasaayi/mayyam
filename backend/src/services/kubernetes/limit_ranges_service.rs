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
use crate::services::kubernetes::limit_range_inventory::{
    LimitRangeInventoryItem, LimitRangeItemInventoryItem,
};
use chrono::{DateTime, Utc};
use k8s_openapi::api::core::v1::LimitRange;
use k8s_openapi::apimachinery::pkg::api::resource::Quantity;
use kube::api::{DeleteParams, ListParams, Patch, PatchParams};
use kube::{Api, ResourceExt};
use std::collections::BTreeMap;

pub struct LimitRangesService;

fn quantity_map_to_strings(map: &BTreeMap<String, Quantity>) -> BTreeMap<String, String> {
    map.iter()
        .map(|(key, value)| (key.clone(), value.0.clone()))
        .collect()
}

fn convert_kube_limit_range_to_inventory(
    limit_range: &LimitRange,
    cluster_id: &str,
    current_namespace: &str,
    collected_at: DateTime<Utc>,
) -> LimitRangeInventoryItem {
    let namespace = limit_range
        .namespace()
        .unwrap_or_else(|| current_namespace.to_string());
    let limits = limit_range
        .spec
        .as_ref()
        .map(|spec| {
            spec.limits
                .iter()
                .map(|item| LimitRangeItemInventoryItem {
                    item_type: item.type_.clone(),
                    default: item
                        .default
                        .as_ref()
                        .map(quantity_map_to_strings)
                        .unwrap_or_default(),
                    default_request: item
                        .default_request
                        .as_ref()
                        .map(quantity_map_to_strings)
                        .unwrap_or_default(),
                    max: item
                        .max
                        .as_ref()
                        .map(quantity_map_to_strings)
                        .unwrap_or_default(),
                    max_limit_request_ratio: item
                        .max_limit_request_ratio
                        .as_ref()
                        .map(quantity_map_to_strings)
                        .unwrap_or_default(),
                    min: item
                        .min
                        .as_ref()
                        .map(quantity_map_to_strings)
                        .unwrap_or_default(),
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    LimitRangeInventoryItem {
        cluster_id: cluster_id.to_string(),
        namespace,
        name: limit_range.name_any(),
        labels: limit_range.metadata.labels.clone().unwrap_or_default(),
        annotations: limit_range.metadata.annotations.clone().unwrap_or_default(),
        limits,
        created_at: limit_range
            .metadata
            .creation_timestamp
            .as_ref()
            .map(|timestamp| timestamp.0),
        collected_at,
    }
}

impl LimitRangesService {
    pub fn new() -> Self {
        Self
    }

    async fn api(
        cluster: &KubernetesClusterConfig,
        namespace: &str,
    ) -> Result<Api<LimitRange>, AppError> {
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
    ) -> Result<Vec<LimitRange>, AppError> {
        let api = Self::api(cluster, namespace).await?;
        let list = api
            .list(&ListParams::default())
            .await
            .map_err(|e| AppError::Kubernetes(e.to_string()))?;
        Ok(list.items)
    }

    pub async fn list_inventory(
        &self,
        cluster: &KubernetesClusterConfig,
        cluster_id: &str,
        namespace: Option<&str>,
    ) -> Result<Vec<LimitRangeInventoryItem>, AppError> {
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
            .map(|limit_range| {
                convert_kube_limit_range_to_inventory(
                    limit_range,
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
    ) -> Result<LimitRange, AppError> {
        let api: Api<LimitRange> =
            Api::namespaced(ClientFactory::get_client(cluster).await?, namespace);
        api.get(name)
            .await
            .map_err(|e| AppError::Kubernetes(e.to_string()))
    }

    pub async fn upsert(
        &self,
        cluster: &KubernetesClusterConfig,
        namespace: &str,
        item: &LimitRange,
    ) -> Result<LimitRange, AppError> {
        let api: Api<LimitRange> =
            Api::namespaced(ClientFactory::get_client(cluster).await?, namespace);
        let params = PatchParams::apply("mayyam").force();
        api.patch(
            item.metadata
                .name
                .as_ref()
                .ok_or_else(|| AppError::BadRequest("LimitRange.metadata.name required".into()))?,
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
        let api: Api<LimitRange> =
            Api::namespaced(ClientFactory::get_client(cluster).await?, namespace);
        api.delete(name, &DeleteParams::default())
            .await
            .map_err(|e| AppError::Kubernetes(e.to_string()))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use k8s_openapi::api::core::v1::{LimitRangeItem, LimitRangeSpec};
    use k8s_openapi::apimachinery::pkg::apis::meta::v1::{ObjectMeta, Time};

    fn map(values: &[(&str, &str)]) -> BTreeMap<String, String> {
        values
            .iter()
            .map(|(key, value)| ((*key).to_string(), (*value).to_string()))
            .collect()
    }

    fn quantities(values: &[(&str, &str)]) -> BTreeMap<String, Quantity> {
        values
            .iter()
            .map(|(key, value)| ((*key).to_string(), Quantity((*value).to_string())))
            .collect()
    }

    #[test]
    fn limit_range_inventory_conversion_preserves_metadata_and_limit_items() {
        let created_at = Utc.with_ymd_and_hms(2026, 6, 1, 12, 0, 0).unwrap();
        let collected_at = Utc.with_ymd_and_hms(2026, 6, 10, 0, 0, 0).unwrap();
        let limit_range = LimitRange {
            metadata: ObjectMeta {
                name: Some("apps-limits".to_string()),
                namespace: Some("apps".to_string()),
                labels: Some(map(&[("team", "platform")])),
                annotations: Some(map(&[("cost-center", "cc-42")])),
                creation_timestamp: Some(Time(created_at)),
                ..Default::default()
            },
            spec: Some(LimitRangeSpec {
                limits: vec![LimitRangeItem {
                    default: Some(quantities(&[("cpu", "500m"), ("memory", "512Mi")])),
                    default_request: Some(quantities(&[("cpu", "100m"), ("memory", "128Mi")])),
                    max: Some(quantities(&[("cpu", "2"), ("memory", "2Gi")])),
                    max_limit_request_ratio: Some(quantities(&[("cpu", "4")])),
                    min: Some(quantities(&[("cpu", "50m")])),
                    type_: "Container".to_string(),
                }],
            }),
        };

        let item = convert_kube_limit_range_to_inventory(
            &limit_range,
            "cluster-a",
            "fallback",
            collected_at,
        );

        assert_eq!(item.cluster_id, "cluster-a");
        assert_eq!(item.namespace, "apps");
        assert_eq!(item.name, "apps-limits");
        assert_eq!(item.labels["team"], "platform");
        assert_eq!(item.annotations["cost-center"], "cc-42");
        assert_eq!(item.created_at, Some(created_at));
        assert_eq!(item.collected_at, collected_at);
        assert_eq!(item.limits.len(), 1);
        assert_eq!(item.limits[0].item_type, "Container");
        assert_eq!(item.limits[0].default["cpu"], "500m");
        assert_eq!(item.limits[0].default_request["memory"], "128Mi");
        assert_eq!(item.limits[0].max["memory"], "2Gi");
        assert_eq!(item.limits[0].min["cpu"], "50m");
    }

    #[test]
    fn limit_range_inventory_conversion_uses_fallback_namespace_when_missing() {
        let collected_at = Utc.with_ymd_and_hms(2026, 6, 10, 0, 0, 0).unwrap();
        let limit_range = LimitRange {
            metadata: ObjectMeta {
                name: Some("fallback-limits".to_string()),
                ..Default::default()
            },
            spec: None,
        };

        let item = convert_kube_limit_range_to_inventory(
            &limit_range,
            "cluster-a",
            "requested-namespace",
            collected_at,
        );

        assert_eq!(item.namespace, "requested-namespace");
        assert_eq!(item.name, "fallback-limits");
        assert!(item.limits.is_empty());
    }
}

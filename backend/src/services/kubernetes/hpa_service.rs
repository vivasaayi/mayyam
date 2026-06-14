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
use crate::services::kubernetes::hpa_inventory::{
    HpaInventoryItem, HpaMetricInventoryItem, HpaOwnerReferenceInventoryItem,
};
use chrono::{DateTime, Utc};
use k8s_openapi::api::autoscaling::v2::{HorizontalPodAutoscaler, MetricSpec};
use kube::api::{DeleteParams, ListParams, Patch, PatchParams};
use kube::{Api, ResourceExt};

pub struct HorizontalPodAutoscalerService;

fn convert_metric_to_inventory(metric: &MetricSpec) -> HpaMetricInventoryItem {
    match metric.type_.as_str() {
        "Resource" => metric
            .resource
            .as_ref()
            .map(|source| HpaMetricInventoryItem {
                metric_type: metric.type_.clone(),
                name: Some(source.name.clone()),
                target_type: Some(source.target.type_.clone()),
            }),
        "ContainerResource" => {
            metric
                .container_resource
                .as_ref()
                .map(|source| HpaMetricInventoryItem {
                    metric_type: metric.type_.clone(),
                    name: Some(format!("{}/{}", source.container, source.name)),
                    target_type: Some(source.target.type_.clone()),
                })
        }
        "External" => metric
            .external
            .as_ref()
            .map(|source| HpaMetricInventoryItem {
                metric_type: metric.type_.clone(),
                name: Some(source.metric.name.clone()),
                target_type: Some(source.target.type_.clone()),
            }),
        "Object" => metric.object.as_ref().map(|source| HpaMetricInventoryItem {
            metric_type: metric.type_.clone(),
            name: Some(source.metric.name.clone()),
            target_type: Some(source.target.type_.clone()),
        }),
        "Pods" => metric.pods.as_ref().map(|source| HpaMetricInventoryItem {
            metric_type: metric.type_.clone(),
            name: Some(source.metric.name.clone()),
            target_type: Some(source.target.type_.clone()),
        }),
        _ => None,
    }
    .unwrap_or_else(|| HpaMetricInventoryItem {
        metric_type: metric.type_.clone(),
        name: None,
        target_type: None,
    })
}

fn convert_kube_hpa_to_inventory(
    hpa: &HorizontalPodAutoscaler,
    cluster_id: &str,
    current_namespace: &str,
    collected_at: DateTime<Utc>,
) -> HpaInventoryItem {
    let namespace = hpa
        .namespace()
        .unwrap_or_else(|| current_namespace.to_string());
    let metrics = hpa
        .spec
        .as_ref()
        .and_then(|spec| spec.metrics.as_ref())
        .map(|metrics| metrics.iter().map(convert_metric_to_inventory).collect())
        .unwrap_or_default();
    let (
        target_api_version,
        target_kind,
        target_name,
        min_replicas,
        max_replicas,
        behavior_configured,
    ) = hpa
        .spec
        .as_ref()
        .map(|spec| {
            (
                spec.scale_target_ref
                    .api_version
                    .clone()
                    .unwrap_or_default(),
                spec.scale_target_ref.kind.clone(),
                spec.scale_target_ref.name.clone(),
                spec.min_replicas,
                spec.max_replicas,
                spec.behavior.is_some(),
            )
        })
        .unwrap_or_default();
    let owner_references = hpa
        .metadata
        .owner_references
        .as_ref()
        .map(|owners| {
            owners
                .iter()
                .map(|owner| HpaOwnerReferenceInventoryItem {
                    kind: Some(owner.kind.clone()),
                    name: owner.name.clone(),
                    controller: owner.controller,
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    HpaInventoryItem {
        cluster_id: cluster_id.to_string(),
        namespace,
        name: hpa.name_any(),
        labels: hpa.metadata.labels.clone().unwrap_or_default(),
        annotations: hpa.metadata.annotations.clone().unwrap_or_default(),
        target_api_version,
        target_kind,
        target_name,
        min_replicas,
        max_replicas,
        current_replicas: hpa
            .status
            .as_ref()
            .and_then(|status| status.current_replicas),
        desired_replicas: hpa.status.as_ref().map(|status| status.desired_replicas),
        metrics,
        behavior_configured,
        owner_references,
        created_at: hpa
            .metadata
            .creation_timestamp
            .as_ref()
            .map(|timestamp| timestamp.0),
        collected_at,
    }
}

impl HorizontalPodAutoscalerService {
    pub fn new() -> Self {
        Self
    }

    async fn api(
        cluster: &KubernetesClusterConfig,
        namespace: &str,
    ) -> Result<Api<HorizontalPodAutoscaler>, AppError> {
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
    ) -> Result<Vec<HorizontalPodAutoscaler>, AppError> {
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
    ) -> Result<Vec<HpaInventoryItem>, AppError> {
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
            .map(|hpa| {
                convert_kube_hpa_to_inventory(hpa, cluster_id, fallback_namespace, collected_at)
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
    ) -> Result<HorizontalPodAutoscaler, AppError> {
        let api: Api<HorizontalPodAutoscaler> =
            Api::namespaced(ClientFactory::get_client(cluster).await?, namespace);
        api.get(name)
            .await
            .map_err(|e| AppError::Kubernetes(e.to_string()))
    }

    pub async fn upsert(
        &self,
        cluster: &KubernetesClusterConfig,
        namespace: &str,
        item: &HorizontalPodAutoscaler,
    ) -> Result<HorizontalPodAutoscaler, AppError> {
        let api: Api<HorizontalPodAutoscaler> =
            Api::namespaced(ClientFactory::get_client(cluster).await?, namespace);
        let params = PatchParams::apply("mayyam").force();
        api.patch(
            item.metadata.name.as_ref().ok_or_else(|| {
                AppError::BadRequest("HorizontalPodAutoscaler.metadata.name required".into())
            })?,
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
        let api: Api<HorizontalPodAutoscaler> =
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
    use k8s_openapi::api::autoscaling::v2::{
        CrossVersionObjectReference, HorizontalPodAutoscalerSpec, HorizontalPodAutoscalerStatus,
        MetricSpec, MetricTarget, ResourceMetricSource,
    };
    use k8s_openapi::apimachinery::pkg::apis::meta::v1::{ObjectMeta, Time};
    use std::collections::BTreeMap;

    fn map(values: &[(&str, &str)]) -> BTreeMap<String, String> {
        values
            .iter()
            .map(|(key, value)| ((*key).to_string(), (*value).to_string()))
            .collect()
    }

    fn target() -> CrossVersionObjectReference {
        CrossVersionObjectReference {
            api_version: Some("apps/v1".to_string()),
            kind: "Deployment".to_string(),
            name: "checkout".to_string(),
        }
    }

    fn metric_target(target_type: &str) -> MetricTarget {
        MetricTarget {
            type_: target_type.to_string(),
            average_utilization: Some(70),
            average_value: None,
            value: None,
        }
    }

    #[test]
    fn hpa_inventory_conversion_preserves_metadata_metrics_and_status() {
        let created_at = Utc.with_ymd_and_hms(2026, 6, 1, 12, 0, 0).unwrap();
        let collected_at = Utc.with_ymd_and_hms(2026, 6, 10, 0, 0, 0).unwrap();
        let hpa = HorizontalPodAutoscaler {
            metadata: ObjectMeta {
                name: Some("checkout-autoscaler".to_string()),
                namespace: Some("apps".to_string()),
                labels: Some(map(&[("team", "payments")])),
                annotations: Some(map(&[("cost-center", "cc-12")])),
                creation_timestamp: Some(Time(created_at)),
                ..Default::default()
            },
            spec: Some(HorizontalPodAutoscalerSpec {
                scale_target_ref: target(),
                min_replicas: Some(2),
                max_replicas: 12,
                metrics: Some(vec![MetricSpec {
                    type_: "Resource".to_string(),
                    resource: Some(ResourceMetricSource {
                        name: "cpu".to_string(),
                        target: metric_target("Utilization"),
                    }),
                    container_resource: None,
                    external: None,
                    object: None,
                    pods: None,
                }]),
                behavior: None,
            }),
            status: Some(HorizontalPodAutoscalerStatus {
                current_metrics: None,
                current_replicas: Some(3),
                desired_replicas: 5,
                last_scale_time: None,
                observed_generation: None,
                conditions: None,
            }),
        };

        let item = convert_kube_hpa_to_inventory(&hpa, "cluster-a", "fallback", collected_at);

        assert_eq!(item.cluster_id, "cluster-a");
        assert_eq!(item.namespace, "apps");
        assert_eq!(item.name, "checkout-autoscaler");
        assert_eq!(item.labels["team"], "payments");
        assert_eq!(item.annotations["cost-center"], "cc-12");
        assert_eq!(item.created_at, Some(created_at));
        assert_eq!(item.collected_at, collected_at);
        assert_eq!(item.target_api_version, "apps/v1");
        assert_eq!(item.target_kind, "Deployment");
        assert_eq!(item.target_name, "checkout");
        assert_eq!(item.min_replicas, Some(2));
        assert_eq!(item.max_replicas, 12);
        assert_eq!(item.current_replicas, Some(3));
        assert_eq!(item.desired_replicas, Some(5));
        assert_eq!(item.metrics.len(), 1);
        assert_eq!(item.metrics[0].metric_type, "Resource");
        assert_eq!(item.metrics[0].name, Some("cpu".to_string()));
        assert_eq!(item.metrics[0].target_type, Some("Utilization".to_string()));
    }

    #[test]
    fn hpa_inventory_conversion_uses_fallback_namespace_when_missing() {
        let collected_at = Utc.with_ymd_and_hms(2026, 6, 10, 0, 0, 0).unwrap();
        let hpa = HorizontalPodAutoscaler {
            metadata: ObjectMeta {
                name: Some("fallback-autoscaler".to_string()),
                ..Default::default()
            },
            spec: Some(HorizontalPodAutoscalerSpec {
                scale_target_ref: target(),
                min_replicas: None,
                max_replicas: 4,
                metrics: None,
                behavior: None,
            }),
            status: None,
        };

        let item =
            convert_kube_hpa_to_inventory(&hpa, "cluster-a", "requested-namespace", collected_at);

        assert_eq!(item.namespace, "requested-namespace");
        assert!(item.metrics.is_empty());
        assert_eq!(item.current_replicas, None);
        assert_eq!(item.desired_replicas, None);
    }
}

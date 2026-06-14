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
use crate::services::kubernetes::pdb_inventory::{PdbConditionInventoryItem, PdbInventoryItem};
use chrono::{DateTime, Utc};
use k8s_openapi::api::policy::v1::PodDisruptionBudget;
use k8s_openapi::apimachinery::pkg::util::intstr::IntOrString;
use kube::api::{DeleteParams, ListParams, Patch, PatchParams};
use kube::{Api, ResourceExt};

pub struct PodDisruptionBudgetsService;

fn int_or_string_to_string(value: &Option<IntOrString>) -> Option<String> {
    match value {
        Some(IntOrString::Int(value)) => Some(value.to_string()),
        Some(IntOrString::String(value)) => Some(value.clone()),
        None => None,
    }
}

fn convert_kube_pdb_to_inventory(
    pdb: &PodDisruptionBudget,
    cluster_id: &str,
    current_namespace: &str,
    collected_at: DateTime<Utc>,
) -> PdbInventoryItem {
    let namespace = pdb
        .namespace()
        .unwrap_or_else(|| current_namespace.to_string());
    let selector = pdb.spec.as_ref().and_then(|spec| spec.selector.as_ref());
    let status = pdb.status.as_ref();

    PdbInventoryItem {
        cluster_id: cluster_id.to_string(),
        namespace,
        name: pdb.name_any(),
        labels: pdb.metadata.labels.clone().unwrap_or_default(),
        annotations: pdb.metadata.annotations.clone().unwrap_or_default(),
        min_available: pdb
            .spec
            .as_ref()
            .and_then(|spec| int_or_string_to_string(&spec.min_available)),
        max_unavailable: pdb
            .spec
            .as_ref()
            .and_then(|spec| int_or_string_to_string(&spec.max_unavailable)),
        selector_present: selector.is_some(),
        selector_match_labels: selector
            .and_then(|selector| selector.match_labels.clone())
            .unwrap_or_default(),
        selector_expression_count: selector
            .and_then(|selector| selector.match_expressions.as_ref())
            .map(Vec::len)
            .unwrap_or(0),
        unhealthy_pod_eviction_policy: pdb
            .spec
            .as_ref()
            .and_then(|spec| spec.unhealthy_pod_eviction_policy.clone()),
        current_healthy: status.map(|status| status.current_healthy),
        desired_healthy: status.map(|status| status.desired_healthy),
        disruptions_allowed: status.map(|status| status.disruptions_allowed),
        expected_pods: status.map(|status| status.expected_pods),
        disrupted_pod_count: status
            .and_then(|status| status.disrupted_pods.as_ref())
            .map(|pods| pods.len())
            .unwrap_or(0),
        conditions: status
            .and_then(|status| status.conditions.as_ref())
            .map(|conditions| {
                conditions
                    .iter()
                    .map(|condition| PdbConditionInventoryItem {
                        type_: condition.type_.clone(),
                        status: condition.status.clone(),
                        reason: Some(condition.reason.clone()),
                        message: Some(condition.message.clone()),
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default(),
        created_at: pdb
            .metadata
            .creation_timestamp
            .as_ref()
            .map(|timestamp| timestamp.0),
        collected_at,
    }
}

impl PodDisruptionBudgetsService {
    pub fn new() -> Self {
        Self
    }

    async fn api(
        cluster: &KubernetesClusterConfig,
        namespace: &str,
    ) -> Result<Api<PodDisruptionBudget>, AppError> {
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
    ) -> Result<Vec<PodDisruptionBudget>, AppError> {
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
    ) -> Result<Vec<PdbInventoryItem>, AppError> {
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
            .map(|pdb| {
                convert_kube_pdb_to_inventory(pdb, cluster_id, fallback_namespace, collected_at)
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
    ) -> Result<PodDisruptionBudget, AppError> {
        let api: Api<PodDisruptionBudget> =
            Api::namespaced(ClientFactory::get_client(cluster).await?, namespace);
        api.get(name)
            .await
            .map_err(|e| AppError::Kubernetes(e.to_string()))
    }

    pub async fn upsert(
        &self,
        cluster: &KubernetesClusterConfig,
        namespace: &str,
        item: &PodDisruptionBudget,
    ) -> Result<PodDisruptionBudget, AppError> {
        let api: Api<PodDisruptionBudget> =
            Api::namespaced(ClientFactory::get_client(cluster).await?, namespace);
        let params = PatchParams::apply("mayyam").force();
        api.patch(
            item.metadata.name.as_ref().ok_or_else(|| {
                AppError::BadRequest("PodDisruptionBudget.metadata.name required".into())
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
        let api: Api<PodDisruptionBudget> =
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
    use k8s_openapi::api::policy::v1::{PodDisruptionBudgetSpec, PodDisruptionBudgetStatus};
    use k8s_openapi::apimachinery::pkg::apis::meta::v1::{
        Condition, LabelSelector, ObjectMeta, Time,
    };
    use std::collections::BTreeMap;

    fn map(values: &[(&str, &str)]) -> BTreeMap<String, String> {
        values
            .iter()
            .map(|(key, value)| ((*key).to_string(), (*value).to_string()))
            .collect()
    }

    #[test]
    fn pdb_inventory_conversion_preserves_metadata_spec_selector_and_status() {
        let created_at = Utc.with_ymd_and_hms(2026, 6, 1, 12, 0, 0).unwrap();
        let transition_at = Utc.with_ymd_and_hms(2026, 6, 9, 12, 0, 0).unwrap();
        let collected_at = Utc.with_ymd_and_hms(2026, 6, 10, 0, 0, 0).unwrap();
        let pdb = PodDisruptionBudget {
            metadata: ObjectMeta {
                name: Some("checkout-pdb".to_string()),
                namespace: Some("apps".to_string()),
                labels: Some(map(&[("team", "payments")])),
                annotations: Some(map(&[("cost-center", "cc-12")])),
                creation_timestamp: Some(Time(created_at)),
                ..Default::default()
            },
            spec: Some(PodDisruptionBudgetSpec {
                min_available: Some(IntOrString::Int(2)),
                max_unavailable: None,
                selector: Some(LabelSelector {
                    match_labels: Some(map(&[("app", "checkout")])),
                    match_expressions: None,
                }),
                unhealthy_pod_eviction_policy: Some("IfHealthyBudget".to_string()),
            }),
            status: Some(PodDisruptionBudgetStatus {
                conditions: Some(vec![Condition {
                    last_transition_time: Time(transition_at),
                    message: "two disruptions allowed".to_string(),
                    observed_generation: Some(1),
                    reason: "SufficientPods".to_string(),
                    status: "True".to_string(),
                    type_: "DisruptionAllowed".to_string(),
                }]),
                current_healthy: 4,
                desired_healthy: 2,
                disrupted_pods: None,
                disruptions_allowed: 2,
                expected_pods: 4,
                observed_generation: Some(1),
            }),
        };

        let item = convert_kube_pdb_to_inventory(&pdb, "cluster-a", "fallback", collected_at);

        assert_eq!(item.cluster_id, "cluster-a");
        assert_eq!(item.namespace, "apps");
        assert_eq!(item.name, "checkout-pdb");
        assert_eq!(item.labels["team"], "payments");
        assert_eq!(item.annotations["cost-center"], "cc-12");
        assert_eq!(item.created_at, Some(created_at));
        assert_eq!(item.collected_at, collected_at);
        assert_eq!(item.min_available, Some("2".to_string()));
        assert_eq!(item.max_unavailable, None);
        assert!(item.selector_present);
        assert_eq!(item.selector_match_labels["app"], "checkout");
        assert_eq!(
            item.unhealthy_pod_eviction_policy,
            Some("IfHealthyBudget".to_string())
        );
        assert_eq!(item.current_healthy, Some(4));
        assert_eq!(item.desired_healthy, Some(2));
        assert_eq!(item.disruptions_allowed, Some(2));
        assert_eq!(item.expected_pods, Some(4));
        assert_eq!(item.conditions.len(), 1);
        assert_eq!(item.conditions[0].type_, "DisruptionAllowed");
    }

    #[test]
    fn pdb_inventory_conversion_uses_fallback_namespace_when_missing() {
        let collected_at = Utc.with_ymd_and_hms(2026, 6, 10, 0, 0, 0).unwrap();
        let pdb = PodDisruptionBudget {
            metadata: ObjectMeta {
                name: Some("fallback-pdb".to_string()),
                ..Default::default()
            },
            spec: Some(PodDisruptionBudgetSpec {
                min_available: None,
                max_unavailable: Some(IntOrString::String("25%".to_string())),
                selector: None,
                unhealthy_pod_eviction_policy: None,
            }),
            status: None,
        };

        let item =
            convert_kube_pdb_to_inventory(&pdb, "cluster-a", "requested-namespace", collected_at);

        assert_eq!(item.namespace, "requested-namespace");
        assert_eq!(item.max_unavailable, Some("25%".to_string()));
        assert!(!item.selector_present);
        assert!(item.selector_match_labels.is_empty());
        assert_eq!(item.current_healthy, None);
        assert_eq!(item.disruptions_allowed, None);
    }
}

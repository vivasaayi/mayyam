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

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use k8s_openapi::api::core::v1::Namespace;
use kube::{api::ListParams, Api, Client, ResourceExt};

use crate::errors::AppError;
use crate::models::cluster::KubernetesClusterConfig;
use crate::services::kubernetes::client::ClientFactory;
use crate::services::kubernetes::pod_security_standards_inventory::{
    PodSecurityStandardsInventoryItem, POD_SECURITY_AUDIT_LABEL, POD_SECURITY_AUDIT_VERSION_LABEL,
    POD_SECURITY_ENFORCE_LABEL, POD_SECURITY_ENFORCE_VERSION_LABEL, POD_SECURITY_WARN_LABEL,
    POD_SECURITY_WARN_VERSION_LABEL,
};

pub struct PodSecurityStandardsService;

impl PodSecurityStandardsService {
    pub fn new() -> Self {
        PodSecurityStandardsService {}
    }

    async fn get_kube_client(cluster_config: &KubernetesClusterConfig) -> Result<Client, AppError> {
        ClientFactory::get_client(cluster_config).await
    }

    pub async fn list_inventory(
        &self,
        cluster_config: &KubernetesClusterConfig,
        cluster_id: &str,
    ) -> Result<Vec<PodSecurityStandardsInventoryItem>, AppError> {
        let client = Self::get_kube_client(cluster_config).await?;
        let api: Api<Namespace> = Api::all(client);
        let lp = ListParams::default();
        let collected_at = Utc::now();
        let namespaces = api.list(&lp).await.map_err(|e| {
            AppError::ExternalService(format!(
                "Failed to list namespaces for Pod Security Standards inventory: {}",
                e
            ))
        })?;

        let mut items = namespaces
            .iter()
            .map(|namespace| {
                convert_namespace_to_pod_security_standards_inventory(
                    namespace,
                    cluster_id,
                    collected_at,
                )
            })
            .collect::<Vec<_>>();
        items.sort_by(|left, right| left.namespace.cmp(&right.namespace));
        Ok(items)
    }
}

fn convert_namespace_to_pod_security_standards_inventory(
    namespace: &Namespace,
    cluster_id: &str,
    collected_at: DateTime<Utc>,
) -> PodSecurityStandardsInventoryItem {
    let labels = namespace.metadata.labels.clone().unwrap_or_default();
    let annotations = namespace.metadata.annotations.clone().unwrap_or_default();

    PodSecurityStandardsInventoryItem {
        cluster_id: cluster_id.to_string(),
        namespace: namespace.name_any(),
        status: namespace
            .status
            .as_ref()
            .and_then(|status| status.phase.clone()),
        enforce_level: label_value(&labels, POD_SECURITY_ENFORCE_LABEL),
        enforce_version: label_value(&labels, POD_SECURITY_ENFORCE_VERSION_LABEL),
        audit_level: label_value(&labels, POD_SECURITY_AUDIT_LABEL),
        audit_version: label_value(&labels, POD_SECURITY_AUDIT_VERSION_LABEL),
        warn_level: label_value(&labels, POD_SECURITY_WARN_LABEL),
        warn_version: label_value(&labels, POD_SECURITY_WARN_VERSION_LABEL),
        labels,
        annotations,
        created_at: namespace
            .metadata
            .creation_timestamp
            .as_ref()
            .map(|timestamp| timestamp.0),
        collected_at,
    }
}

fn label_value(labels: &BTreeMap<String, String>, wanted_key: &str) -> Option<String> {
    labels
        .iter()
        .find(|(key, value)| key.eq_ignore_ascii_case(wanted_key) && !value.trim().is_empty())
        .map(|(_, value)| value.clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use k8s_openapi::api::core::v1::NamespaceStatus;
    use k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta;

    fn labels(values: &[(&str, &str)]) -> BTreeMap<String, String> {
        values
            .iter()
            .map(|(key, value)| ((*key).to_string(), (*value).to_string()))
            .collect()
    }

    #[test]
    fn namespace_conversion_extracts_pod_security_labels() {
        let collected_at = Utc::now();
        let namespace = Namespace {
            metadata: ObjectMeta {
                name: Some("payments".to_string()),
                labels: Some(labels(&[
                    ("owner", "platform"),
                    ("pod-security.kubernetes.io/enforce", "restricted"),
                    ("pod-security.kubernetes.io/enforce-version", "v1.30"),
                    ("pod-security.kubernetes.io/audit", "baseline"),
                    ("pod-security.kubernetes.io/audit-version", "v1.30"),
                    ("pod-security.kubernetes.io/warn", "restricted"),
                    ("pod-security.kubernetes.io/warn-version", "v1.30"),
                ])),
                ..Default::default()
            },
            status: Some(NamespaceStatus {
                phase: Some("Active".to_string()),
                ..Default::default()
            }),
            ..Default::default()
        };

        let item = convert_namespace_to_pod_security_standards_inventory(
            &namespace,
            "cluster-1",
            collected_at,
        );

        assert_eq!(item.cluster_id, "cluster-1");
        assert_eq!(item.namespace, "payments");
        assert_eq!(item.status.as_deref(), Some("Active"));
        assert_eq!(item.enforce_level.as_deref(), Some("restricted"));
        assert_eq!(item.enforce_version.as_deref(), Some("v1.30"));
        assert_eq!(item.audit_level.as_deref(), Some("baseline"));
        assert_eq!(item.warn_level.as_deref(), Some("restricted"));
        assert_eq!(item.collected_at, collected_at);
    }

    #[test]
    fn namespace_conversion_tolerates_missing_policy_labels() {
        let namespace = Namespace {
            metadata: ObjectMeta {
                name: Some("default".to_string()),
                labels: Some(BTreeMap::new()),
                ..Default::default()
            },
            ..Default::default()
        };

        let item = convert_namespace_to_pod_security_standards_inventory(
            &namespace,
            "cluster-1",
            Utc::now(),
        );

        assert_eq!(item.namespace, "default");
        assert!(item.enforce_level.is_none());
        assert!(item.audit_level.is_none());
        assert!(item.warn_level.is_none());
        assert!(item.status.is_none());
    }
}

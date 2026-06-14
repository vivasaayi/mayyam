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
use crate::services::kubernetes::cluster_role_binding_inventory::{
    ClusterRoleBindingInventoryItem, ClusterRoleBindingOwnerReferenceInventoryItem,
    ClusterRoleBindingRoleRefInventoryItem, ClusterRoleBindingSubjectInventoryItem,
};
use crate::services::kubernetes::cluster_role_inventory::{
    ClusterRoleInventoryItem, ClusterRoleOwnerReferenceInventoryItem, ClusterRoleRuleInventoryItem,
};
use crate::services::kubernetes::role_binding_inventory::{
    RoleBindingInventoryItem, RoleBindingOwnerReferenceInventoryItem,
    RoleBindingRoleRefInventoryItem, RoleBindingSubjectInventoryItem,
};
use crate::services::kubernetes::role_inventory::{
    RoleInventoryItem, RoleOwnerReferenceInventoryItem, RoleRuleInventoryItem,
};
use chrono::{DateTime, Utc};
use k8s_openapi::api::rbac::v1::{
    ClusterRole, ClusterRoleBinding, PolicyRule, Role, RoleBinding, RoleRef, Subject,
};
use kube::api::{DeleteParams, ListParams, Patch, PatchParams};
use kube::{Api, ResourceExt};

pub struct RbacService;

fn convert_policy_rule_to_inventory(rule: &PolicyRule) -> RoleRuleInventoryItem {
    let mut api_groups = rule.api_groups.clone().unwrap_or_default();
    let mut resources = rule.resources.clone().unwrap_or_default();
    let mut verbs = rule.verbs.clone();
    let mut resource_names = rule.resource_names.clone().unwrap_or_default();
    let mut non_resource_urls = rule.non_resource_urls.clone().unwrap_or_default();
    api_groups.sort();
    resources.sort();
    verbs.sort();
    resource_names.sort();
    non_resource_urls.sort();

    RoleRuleInventoryItem {
        api_groups,
        resources,
        verbs,
        resource_names,
        non_resource_urls,
    }
}

fn convert_kube_role_to_inventory(
    role: &Role,
    cluster_id: &str,
    current_namespace: &str,
    collected_at: DateTime<Utc>,
) -> RoleInventoryItem {
    let namespace = role
        .namespace()
        .unwrap_or_else(|| current_namespace.to_string());
    let rules = role
        .rules
        .as_ref()
        .map(|rules| {
            rules
                .iter()
                .map(convert_policy_rule_to_inventory)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let owner_references = role
        .metadata
        .owner_references
        .as_ref()
        .map(|owners| {
            owners
                .iter()
                .map(|owner| RoleOwnerReferenceInventoryItem {
                    kind: Some(owner.kind.clone()),
                    name: owner.name.clone(),
                    controller: owner.controller,
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    RoleInventoryItem {
        cluster_id: cluster_id.to_string(),
        namespace,
        name: role.name_any(),
        labels: role.metadata.labels.clone().unwrap_or_default(),
        annotations: role.metadata.annotations.clone().unwrap_or_default(),
        rules,
        owner_references,
        created_at: role
            .metadata
            .creation_timestamp
            .as_ref()
            .map(|timestamp| timestamp.0),
        collected_at,
    }
}

fn convert_policy_rule_to_cluster_role_inventory(
    rule: &PolicyRule,
) -> ClusterRoleRuleInventoryItem {
    let mut api_groups = rule.api_groups.clone().unwrap_or_default();
    let mut resources = rule.resources.clone().unwrap_or_default();
    let mut verbs = rule.verbs.clone();
    let mut resource_names = rule.resource_names.clone().unwrap_or_default();
    let mut non_resource_urls = rule.non_resource_urls.clone().unwrap_or_default();
    api_groups.sort();
    resources.sort();
    verbs.sort();
    resource_names.sort();
    non_resource_urls.sort();

    ClusterRoleRuleInventoryItem {
        api_groups,
        resources,
        verbs,
        resource_names,
        non_resource_urls,
    }
}

fn convert_kube_cluster_role_to_inventory(
    cluster_role: &ClusterRole,
    cluster_id: &str,
    collected_at: DateTime<Utc>,
) -> ClusterRoleInventoryItem {
    let rules = cluster_role
        .rules
        .as_ref()
        .map(|rules| {
            rules
                .iter()
                .map(convert_policy_rule_to_cluster_role_inventory)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let owner_references = cluster_role
        .metadata
        .owner_references
        .as_ref()
        .map(|owners| {
            owners
                .iter()
                .map(|owner| ClusterRoleOwnerReferenceInventoryItem {
                    kind: Some(owner.kind.clone()),
                    name: owner.name.clone(),
                    controller: owner.controller,
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    ClusterRoleInventoryItem {
        cluster_id: cluster_id.to_string(),
        name: cluster_role.name_any(),
        labels: cluster_role.metadata.labels.clone().unwrap_or_default(),
        annotations: cluster_role
            .metadata
            .annotations
            .clone()
            .unwrap_or_default(),
        rules,
        owner_references,
        created_at: cluster_role
            .metadata
            .creation_timestamp
            .as_ref()
            .map(|timestamp| timestamp.0),
        collected_at,
    }
}

fn convert_role_ref_to_inventory(role_ref: &RoleRef) -> RoleBindingRoleRefInventoryItem {
    RoleBindingRoleRefInventoryItem {
        api_group: role_ref.api_group.clone(),
        kind: role_ref.kind.clone(),
        name: role_ref.name.clone(),
    }
}

fn convert_subject_to_inventory(subject: &Subject) -> RoleBindingSubjectInventoryItem {
    RoleBindingSubjectInventoryItem {
        api_group: subject.api_group.clone(),
        kind: subject.kind.clone(),
        namespace: subject.namespace.clone(),
        name: subject.name.clone(),
    }
}

fn convert_kube_role_binding_to_inventory(
    role_binding: &RoleBinding,
    cluster_id: &str,
    current_namespace: &str,
    collected_at: DateTime<Utc>,
) -> RoleBindingInventoryItem {
    let namespace = role_binding
        .namespace()
        .unwrap_or_else(|| current_namespace.to_string());
    let mut subjects = role_binding
        .subjects
        .as_ref()
        .map(|subjects| {
            subjects
                .iter()
                .map(convert_subject_to_inventory)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    subjects.sort_by(|left, right| {
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

    let owner_references = role_binding
        .metadata
        .owner_references
        .as_ref()
        .map(|owners| {
            owners
                .iter()
                .map(|owner| RoleBindingOwnerReferenceInventoryItem {
                    kind: Some(owner.kind.clone()),
                    name: owner.name.clone(),
                    controller: owner.controller,
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    RoleBindingInventoryItem {
        cluster_id: cluster_id.to_string(),
        namespace,
        name: role_binding.name_any(),
        labels: role_binding.metadata.labels.clone().unwrap_or_default(),
        annotations: role_binding
            .metadata
            .annotations
            .clone()
            .unwrap_or_default(),
        role_ref: convert_role_ref_to_inventory(&role_binding.role_ref),
        subjects,
        owner_references,
        created_at: role_binding
            .metadata
            .creation_timestamp
            .as_ref()
            .map(|timestamp| timestamp.0),
        collected_at,
    }
}

fn convert_role_ref_to_cluster_role_binding_inventory(
    role_ref: &RoleRef,
) -> ClusterRoleBindingRoleRefInventoryItem {
    ClusterRoleBindingRoleRefInventoryItem {
        api_group: role_ref.api_group.clone(),
        kind: role_ref.kind.clone(),
        name: role_ref.name.clone(),
    }
}

fn convert_subject_to_cluster_role_binding_inventory(
    subject: &Subject,
) -> ClusterRoleBindingSubjectInventoryItem {
    ClusterRoleBindingSubjectInventoryItem {
        api_group: subject.api_group.clone(),
        kind: subject.kind.clone(),
        namespace: subject.namespace.clone(),
        name: subject.name.clone(),
    }
}

fn convert_kube_cluster_role_binding_to_inventory(
    cluster_role_binding: &ClusterRoleBinding,
    cluster_id: &str,
    collected_at: DateTime<Utc>,
) -> ClusterRoleBindingInventoryItem {
    let mut subjects = cluster_role_binding
        .subjects
        .as_ref()
        .map(|subjects| {
            subjects
                .iter()
                .map(convert_subject_to_cluster_role_binding_inventory)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    subjects.sort_by(|left, right| {
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

    let owner_references = cluster_role_binding
        .metadata
        .owner_references
        .as_ref()
        .map(|owners| {
            owners
                .iter()
                .map(|owner| ClusterRoleBindingOwnerReferenceInventoryItem {
                    kind: Some(owner.kind.clone()),
                    name: owner.name.clone(),
                    controller: owner.controller,
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    ClusterRoleBindingInventoryItem {
        cluster_id: cluster_id.to_string(),
        name: cluster_role_binding.name_any(),
        labels: cluster_role_binding
            .metadata
            .labels
            .clone()
            .unwrap_or_default(),
        annotations: cluster_role_binding
            .metadata
            .annotations
            .clone()
            .unwrap_or_default(),
        role_ref: convert_role_ref_to_cluster_role_binding_inventory(
            &cluster_role_binding.role_ref,
        ),
        subjects,
        owner_references,
        created_at: cluster_role_binding
            .metadata
            .creation_timestamp
            .as_ref()
            .map(|timestamp| timestamp.0),
        collected_at,
    }
}

impl RbacService {
    pub fn new() -> Self {
        Self
    }

    // Roles
    async fn roles_api(
        cluster: &KubernetesClusterConfig,
        namespace: &str,
    ) -> Result<Api<Role>, AppError> {
        let client = ClientFactory::get_client(cluster).await?;
        Ok(if namespace.is_empty() || namespace == "all" {
            Api::all(client)
        } else {
            Api::namespaced(client, namespace)
        })
    }
    pub async fn list_roles(
        &self,
        cluster: &KubernetesClusterConfig,
        namespace: &str,
    ) -> Result<Vec<Role>, AppError> {
        let api = Self::roles_api(cluster, namespace).await?;
        Ok(api
            .list(&ListParams::default())
            .await
            .map_err(|e| AppError::Kubernetes(e.to_string()))?
            .items)
    }
    pub async fn list_role_inventory(
        &self,
        cluster: &KubernetesClusterConfig,
        cluster_id: &str,
        namespace: Option<&str>,
    ) -> Result<Vec<RoleInventoryItem>, AppError> {
        let namespace = namespace
            .map(str::trim)
            .filter(|namespace| !namespace.is_empty());
        let namespace_arg = namespace.unwrap_or("");
        let fallback_namespace = namespace
            .filter(|namespace| *namespace != "all")
            .unwrap_or("");
        let collected_at = Utc::now();

        let api = Self::roles_api(cluster, namespace_arg).await?;
        let list = api
            .list(&ListParams::default())
            .await
            .map_err(|e| AppError::Kubernetes(e.to_string()))?;
        let mut inventory = list
            .items
            .iter()
            .map(|role| {
                convert_kube_role_to_inventory(role, cluster_id, fallback_namespace, collected_at)
            })
            .collect::<Vec<_>>();
        inventory.sort_by(|left, right| {
            (left.namespace.as_str(), left.name.as_str())
                .cmp(&(right.namespace.as_str(), right.name.as_str()))
        });
        Ok(inventory)
    }
    pub async fn get_role(
        &self,
        cluster: &KubernetesClusterConfig,
        namespace: &str,
        name: &str,
    ) -> Result<Role, AppError> {
        let api: Api<Role> = Api::namespaced(ClientFactory::get_client(cluster).await?, namespace);
        api.get(name)
            .await
            .map_err(|e| AppError::Kubernetes(e.to_string()))
    }
    pub async fn upsert_role(
        &self,
        cluster: &KubernetesClusterConfig,
        namespace: &str,
        item: &Role,
    ) -> Result<Role, AppError> {
        let api: Api<Role> = Api::namespaced(ClientFactory::get_client(cluster).await?, namespace);
        let pp = PatchParams::apply("mayyam").force();
        api.patch(
            item.metadata
                .name
                .as_ref()
                .ok_or_else(|| AppError::BadRequest("Role.metadata.name required".into()))?,
            &pp,
            &Patch::Apply(item),
        )
        .await
        .map_err(|e| AppError::Kubernetes(e.to_string()))
    }
    pub async fn delete_role(
        &self,
        cluster: &KubernetesClusterConfig,
        namespace: &str,
        name: &str,
    ) -> Result<(), AppError> {
        let api: Api<Role> = Api::namespaced(ClientFactory::get_client(cluster).await?, namespace);
        api.delete(name, &DeleteParams::default())
            .await
            .map_err(|e| AppError::Kubernetes(e.to_string()))?;
        Ok(())
    }

    // RoleBindings
    async fn role_bindings_api(
        cluster: &KubernetesClusterConfig,
        namespace: &str,
    ) -> Result<Api<RoleBinding>, AppError> {
        let client = ClientFactory::get_client(cluster).await?;
        Ok(if namespace.is_empty() || namespace == "all" {
            Api::all(client)
        } else {
            Api::namespaced(client, namespace)
        })
    }
    pub async fn list_role_bindings(
        &self,
        cluster: &KubernetesClusterConfig,
        namespace: &str,
    ) -> Result<Vec<RoleBinding>, AppError> {
        let api = Self::role_bindings_api(cluster, namespace).await?;
        Ok(api
            .list(&ListParams::default())
            .await
            .map_err(|e| AppError::Kubernetes(e.to_string()))?
            .items)
    }
    pub async fn list_role_binding_inventory(
        &self,
        cluster: &KubernetesClusterConfig,
        cluster_id: &str,
        namespace: Option<&str>,
    ) -> Result<Vec<RoleBindingInventoryItem>, AppError> {
        let namespace = namespace
            .map(str::trim)
            .filter(|namespace| !namespace.is_empty());
        let namespace_arg = namespace.unwrap_or("");
        let fallback_namespace = namespace
            .filter(|namespace| *namespace != "all")
            .unwrap_or("");
        let collected_at = Utc::now();

        let api = Self::role_bindings_api(cluster, namespace_arg).await?;
        let list = api
            .list(&ListParams::default())
            .await
            .map_err(|e| AppError::Kubernetes(e.to_string()))?;
        let mut inventory = list
            .items
            .iter()
            .map(|role_binding| {
                convert_kube_role_binding_to_inventory(
                    role_binding,
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
    pub async fn get_role_binding(
        &self,
        cluster: &KubernetesClusterConfig,
        namespace: &str,
        name: &str,
    ) -> Result<RoleBinding, AppError> {
        let api: Api<RoleBinding> =
            Api::namespaced(ClientFactory::get_client(cluster).await?, namespace);
        api.get(name)
            .await
            .map_err(|e| AppError::Kubernetes(e.to_string()))
    }
    pub async fn upsert_role_binding(
        &self,
        cluster: &KubernetesClusterConfig,
        namespace: &str,
        item: &RoleBinding,
    ) -> Result<RoleBinding, AppError> {
        let api: Api<RoleBinding> =
            Api::namespaced(ClientFactory::get_client(cluster).await?, namespace);
        let pp = PatchParams::apply("mayyam").force();
        api.patch(
            item.metadata
                .name
                .as_ref()
                .ok_or_else(|| AppError::BadRequest("RoleBinding.metadata.name required".into()))?,
            &pp,
            &Patch::Apply(item),
        )
        .await
        .map_err(|e| AppError::Kubernetes(e.to_string()))
    }
    pub async fn delete_role_binding(
        &self,
        cluster: &KubernetesClusterConfig,
        namespace: &str,
        name: &str,
    ) -> Result<(), AppError> {
        let api: Api<RoleBinding> =
            Api::namespaced(ClientFactory::get_client(cluster).await?, namespace);
        api.delete(name, &DeleteParams::default())
            .await
            .map_err(|e| AppError::Kubernetes(e.to_string()))?;
        Ok(())
    }

    // ClusterRoles (cluster-scoped)
    async fn cluster_roles_api(
        cluster: &KubernetesClusterConfig,
    ) -> Result<Api<ClusterRole>, AppError> {
        let client = ClientFactory::get_client(cluster).await?;
        Ok(Api::all(client))
    }
    pub async fn list_cluster_roles(
        &self,
        cluster: &KubernetesClusterConfig,
    ) -> Result<Vec<ClusterRole>, AppError> {
        let api: Api<ClusterRole> = Self::cluster_roles_api(cluster).await?;
        Ok(api
            .list(&ListParams::default())
            .await
            .map_err(|e| AppError::Kubernetes(e.to_string()))?
            .items)
    }
    pub async fn list_cluster_role_inventory(
        &self,
        cluster: &KubernetesClusterConfig,
        cluster_id: &str,
    ) -> Result<Vec<ClusterRoleInventoryItem>, AppError> {
        let collected_at = Utc::now();
        let api = Self::cluster_roles_api(cluster).await?;
        let list = api
            .list(&ListParams::default())
            .await
            .map_err(|e| AppError::Kubernetes(e.to_string()))?;
        let mut inventory = list
            .items
            .iter()
            .map(|cluster_role| {
                convert_kube_cluster_role_to_inventory(cluster_role, cluster_id, collected_at)
            })
            .collect::<Vec<_>>();
        inventory.sort_by(|left, right| left.name.cmp(&right.name));
        Ok(inventory)
    }
    pub async fn get_cluster_role(
        &self,
        cluster: &KubernetesClusterConfig,
        name: &str,
    ) -> Result<ClusterRole, AppError> {
        let api: Api<ClusterRole> = Self::cluster_roles_api(cluster).await?;
        api.get(name)
            .await
            .map_err(|e| AppError::Kubernetes(e.to_string()))
    }
    pub async fn upsert_cluster_role(
        &self,
        cluster: &KubernetesClusterConfig,
        item: &ClusterRole,
    ) -> Result<ClusterRole, AppError> {
        let api = Self::cluster_roles_api(cluster).await?;
        let pp = PatchParams::apply("mayyam").force();
        api.patch(
            item.metadata
                .name
                .as_ref()
                .ok_or_else(|| AppError::BadRequest("ClusterRole.metadata.name required".into()))?,
            &pp,
            &Patch::Apply(item),
        )
        .await
        .map_err(|e| AppError::Kubernetes(e.to_string()))
    }
    pub async fn delete_cluster_role(
        &self,
        cluster: &KubernetesClusterConfig,
        name: &str,
    ) -> Result<(), AppError> {
        let api = Self::cluster_roles_api(cluster).await?;
        api.delete(name, &DeleteParams::default())
            .await
            .map_err(|e| AppError::Kubernetes(e.to_string()))?;
        Ok(())
    }

    // ClusterRoleBindings (cluster-scoped)
    async fn cluster_role_bindings_api(
        cluster: &KubernetesClusterConfig,
    ) -> Result<Api<ClusterRoleBinding>, AppError> {
        let client = ClientFactory::get_client(cluster).await?;
        Ok(Api::all(client))
    }
    pub async fn list_cluster_role_bindings(
        &self,
        cluster: &KubernetesClusterConfig,
    ) -> Result<Vec<ClusterRoleBinding>, AppError> {
        let api: Api<ClusterRoleBinding> = Self::cluster_role_bindings_api(cluster).await?;
        Ok(api
            .list(&ListParams::default())
            .await
            .map_err(|e| AppError::Kubernetes(e.to_string()))?
            .items)
    }
    pub async fn list_cluster_role_binding_inventory(
        &self,
        cluster: &KubernetesClusterConfig,
        cluster_id: &str,
    ) -> Result<Vec<ClusterRoleBindingInventoryItem>, AppError> {
        let collected_at = Utc::now();
        let api = Self::cluster_role_bindings_api(cluster).await?;
        let list = api
            .list(&ListParams::default())
            .await
            .map_err(|e| AppError::Kubernetes(e.to_string()))?;
        let mut inventory = list
            .items
            .iter()
            .map(|cluster_role_binding| {
                convert_kube_cluster_role_binding_to_inventory(
                    cluster_role_binding,
                    cluster_id,
                    collected_at,
                )
            })
            .collect::<Vec<_>>();
        inventory.sort_by(|left, right| left.name.cmp(&right.name));
        Ok(inventory)
    }
    pub async fn get_cluster_role_binding(
        &self,
        cluster: &KubernetesClusterConfig,
        name: &str,
    ) -> Result<ClusterRoleBinding, AppError> {
        let api: Api<ClusterRoleBinding> = Self::cluster_role_bindings_api(cluster).await?;
        api.get(name)
            .await
            .map_err(|e| AppError::Kubernetes(e.to_string()))
    }
    pub async fn upsert_cluster_role_binding(
        &self,
        cluster: &KubernetesClusterConfig,
        item: &ClusterRoleBinding,
    ) -> Result<ClusterRoleBinding, AppError> {
        let api = Self::cluster_role_bindings_api(cluster).await?;
        let pp = PatchParams::apply("mayyam").force();
        api.patch(
            item.metadata.name.as_ref().ok_or_else(|| {
                AppError::BadRequest("ClusterRoleBinding.metadata.name required".into())
            })?,
            &pp,
            &Patch::Apply(item),
        )
        .await
        .map_err(|e| AppError::Kubernetes(e.to_string()))
    }
    pub async fn delete_cluster_role_binding(
        &self,
        cluster: &KubernetesClusterConfig,
        name: &str,
    ) -> Result<(), AppError> {
        let api = Self::cluster_role_bindings_api(cluster).await?;
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
    use k8s_openapi::apimachinery::pkg::apis::meta::v1::{ObjectMeta, Time};
    use std::collections::BTreeMap;

    fn map(values: &[(&str, &str)]) -> BTreeMap<String, String> {
        values
            .iter()
            .map(|(key, value)| ((*key).to_string(), (*value).to_string()))
            .collect()
    }

    fn subject(kind: &str, namespace: Option<&str>, name: &str) -> Subject {
        Subject {
            api_group: if kind == "ServiceAccount" {
                None
            } else {
                Some("rbac.authorization.k8s.io".to_string())
            },
            kind: kind.to_string(),
            namespace: namespace.map(str::to_string),
            name: name.to_string(),
        }
    }

    #[test]
    fn cluster_role_inventory_conversion_preserves_metadata_and_sorts_rules() {
        let created_at = Utc.with_ymd_and_hms(2026, 6, 1, 12, 0, 0).unwrap();
        let collected_at = Utc.with_ymd_and_hms(2026, 6, 10, 0, 0, 0).unwrap();
        let cluster_role = ClusterRole {
            metadata: ObjectMeta {
                name: Some("cluster-reader".to_string()),
                labels: Some(map(&[("team", "platform")])),
                annotations: Some(map(&[("cost-center", "cc-21")])),
                creation_timestamp: Some(Time(created_at)),
                ..Default::default()
            },
            rules: Some(vec![PolicyRule {
                api_groups: Some(vec!["apps".to_string(), "".to_string()]),
                resources: Some(vec!["secrets".to_string(), "configmaps".to_string()]),
                verbs: vec!["list".to_string(), "get".to_string()],
                resource_names: Some(vec!["z".to_string(), "a".to_string()]),
                non_resource_urls: Some(vec!["/healthz".to_string(), "/metrics".to_string()]),
            }]),
            aggregation_rule: None,
        };

        let item = convert_kube_cluster_role_to_inventory(&cluster_role, "cluster-a", collected_at);

        assert_eq!(item.cluster_id, "cluster-a");
        assert_eq!(item.name, "cluster-reader");
        assert_eq!(item.labels["team"], "platform");
        assert_eq!(item.annotations["cost-center"], "cc-21");
        assert_eq!(item.created_at, Some(created_at));
        assert_eq!(item.collected_at, collected_at);
        assert_eq!(item.rules.len(), 1);
        assert_eq!(item.rules[0].api_groups, vec!["", "apps"]);
        assert_eq!(item.rules[0].resources, vec!["configmaps", "secrets"]);
        assert_eq!(item.rules[0].verbs, vec!["get", "list"]);
        assert_eq!(item.rules[0].resource_names, vec!["a", "z"]);
        assert_eq!(
            item.rules[0].non_resource_urls,
            vec!["/healthz", "/metrics"]
        );
    }

    #[test]
    fn role_binding_inventory_conversion_preserves_metadata_and_sorts_subjects() {
        let created_at = Utc.with_ymd_and_hms(2026, 6, 1, 12, 0, 0).unwrap();
        let collected_at = Utc.with_ymd_and_hms(2026, 6, 10, 0, 0, 0).unwrap();
        let role_binding = RoleBinding {
            metadata: ObjectMeta {
                name: Some("checkout-admin".to_string()),
                namespace: Some("apps".to_string()),
                labels: Some(map(&[("team", "payments")])),
                annotations: Some(map(&[("cost-center", "cc-42")])),
                creation_timestamp: Some(Time(created_at)),
                ..Default::default()
            },
            role_ref: RoleRef {
                api_group: "rbac.authorization.k8s.io".to_string(),
                kind: "ClusterRole".to_string(),
                name: "edit".to_string(),
            },
            subjects: Some(vec![
                subject("User", None, "alice"),
                subject("ServiceAccount", Some("apps"), "default"),
                subject("Group", None, "ops"),
                subject("ServiceAccount", Some("apps"), "checkout"),
            ]),
        };

        let item = convert_kube_role_binding_to_inventory(
            &role_binding,
            "cluster-a",
            "fallback",
            collected_at,
        );

        assert_eq!(item.cluster_id, "cluster-a");
        assert_eq!(item.namespace, "apps");
        assert_eq!(item.name, "checkout-admin");
        assert_eq!(item.labels["team"], "payments");
        assert_eq!(item.annotations["cost-center"], "cc-42");
        assert_eq!(item.created_at, Some(created_at));
        assert_eq!(item.collected_at, collected_at);
        assert_eq!(item.role_ref.kind, "ClusterRole");
        assert_eq!(item.role_ref.name, "edit");

        let subject_keys = item
            .subjects
            .iter()
            .map(|subject| {
                format!(
                    "{}/{}/{}",
                    subject.kind,
                    subject.namespace.as_deref().unwrap_or(""),
                    subject.name
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(
            subject_keys,
            vec![
                "Group//ops",
                "ServiceAccount/apps/checkout",
                "ServiceAccount/apps/default",
                "User//alice"
            ]
        );
    }

    #[test]
    fn role_binding_inventory_conversion_uses_fallback_namespace_when_missing() {
        let collected_at = Utc.with_ymd_and_hms(2026, 6, 10, 0, 0, 0).unwrap();
        let role_binding = RoleBinding {
            metadata: ObjectMeta {
                name: Some("fallback-reader".to_string()),
                ..Default::default()
            },
            role_ref: RoleRef {
                api_group: "rbac.authorization.k8s.io".to_string(),
                kind: "Role".to_string(),
                name: "reader".to_string(),
            },
            subjects: None,
        };

        let item = convert_kube_role_binding_to_inventory(
            &role_binding,
            "cluster-a",
            "requested-namespace",
            collected_at,
        );

        assert_eq!(item.namespace, "requested-namespace");
        assert!(item.subjects.is_empty());
    }

    #[test]
    fn cluster_role_binding_inventory_conversion_preserves_metadata_and_sorts_subjects() {
        let created_at = Utc.with_ymd_and_hms(2026, 6, 1, 12, 0, 0).unwrap();
        let collected_at = Utc.with_ymd_and_hms(2026, 6, 10, 0, 0, 0).unwrap();
        let cluster_role_binding = ClusterRoleBinding {
            metadata: ObjectMeta {
                name: Some("cluster-admin-binding".to_string()),
                labels: Some(map(&[("team", "platform")])),
                annotations: Some(map(&[("cost-center", "cc-88")])),
                creation_timestamp: Some(Time(created_at)),
                ..Default::default()
            },
            role_ref: RoleRef {
                api_group: "rbac.authorization.k8s.io".to_string(),
                kind: "ClusterRole".to_string(),
                name: "cluster-admin".to_string(),
            },
            subjects: Some(vec![
                subject("User", None, "alice"),
                subject("ServiceAccount", Some("kube-system"), "default"),
                subject("Group", None, "ops"),
                subject("ServiceAccount", Some("apps"), "checkout"),
            ]),
        };

        let item = convert_kube_cluster_role_binding_to_inventory(
            &cluster_role_binding,
            "cluster-a",
            collected_at,
        );

        assert_eq!(item.cluster_id, "cluster-a");
        assert_eq!(item.name, "cluster-admin-binding");
        assert_eq!(item.labels["team"], "platform");
        assert_eq!(item.annotations["cost-center"], "cc-88");
        assert_eq!(item.created_at, Some(created_at));
        assert_eq!(item.collected_at, collected_at);
        assert_eq!(item.role_ref.kind, "ClusterRole");
        assert_eq!(item.role_ref.name, "cluster-admin");

        let subject_keys = item
            .subjects
            .iter()
            .map(|subject| {
                format!(
                    "{}/{}/{}",
                    subject.kind,
                    subject.namespace.as_deref().unwrap_or(""),
                    subject.name
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(
            subject_keys,
            vec![
                "Group//ops",
                "ServiceAccount/apps/checkout",
                "ServiceAccount/kube-system/default",
                "User//alice"
            ]
        );
    }
}

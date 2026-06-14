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
use crate::services::kubernetes::admission_webhook_inventory::AdmissionWebhookInventoryItem;
use crate::services::kubernetes::client::ClientFactory;
use chrono::{DateTime, Utc};
use k8s_openapi::api::admissionregistration::v1::{
    MutatingWebhook, MutatingWebhookConfiguration, RuleWithOperations, ValidatingWebhook,
    ValidatingWebhookConfiguration, WebhookClientConfig,
};
use k8s_openapi::apimachinery::pkg::apis::meta::v1::{LabelSelector, ObjectMeta};
use kube::{api::ListParams, Api};

pub struct AdmissionWebhooksService;

struct AdmissionWebhookParts<'a> {
    name: &'a str,
    webhook_type: &'static str,
    failure_policy: &'a Option<String>,
    match_policy: &'a Option<String>,
    side_effects: &'a str,
    timeout_seconds: Option<i32>,
    admission_review_versions: &'a [String],
    namespace_selector: &'a Option<LabelSelector>,
    object_selector: &'a Option<LabelSelector>,
    rules: Option<&'a Vec<RuleWithOperations>>,
    client_config: &'a WebhookClientConfig,
}

fn convert_validating_webhook_configuration_to_inventory(
    config: &ValidatingWebhookConfiguration,
    cluster_id: &str,
    collected_at: DateTime<Utc>,
) -> Vec<AdmissionWebhookInventoryItem> {
    config
        .webhooks
        .as_ref()
        .map(|webhooks| {
            webhooks
                .iter()
                .map(|webhook| {
                    convert_webhook_to_inventory(
                        &config.metadata,
                        cluster_id,
                        collected_at,
                        AdmissionWebhookParts::from_validating(webhook),
                    )
                })
                .collect()
        })
        .unwrap_or_default()
}

fn convert_mutating_webhook_configuration_to_inventory(
    config: &MutatingWebhookConfiguration,
    cluster_id: &str,
    collected_at: DateTime<Utc>,
) -> Vec<AdmissionWebhookInventoryItem> {
    config
        .webhooks
        .as_ref()
        .map(|webhooks| {
            webhooks
                .iter()
                .map(|webhook| {
                    convert_webhook_to_inventory(
                        &config.metadata,
                        cluster_id,
                        collected_at,
                        AdmissionWebhookParts::from_mutating(webhook),
                    )
                })
                .collect()
        })
        .unwrap_or_default()
}

impl<'a> AdmissionWebhookParts<'a> {
    fn from_validating(webhook: &'a ValidatingWebhook) -> Self {
        Self {
            name: &webhook.name,
            webhook_type: "Validating",
            failure_policy: &webhook.failure_policy,
            match_policy: &webhook.match_policy,
            side_effects: &webhook.side_effects,
            timeout_seconds: webhook.timeout_seconds,
            admission_review_versions: &webhook.admission_review_versions,
            namespace_selector: &webhook.namespace_selector,
            object_selector: &webhook.object_selector,
            rules: webhook.rules.as_ref(),
            client_config: &webhook.client_config,
        }
    }

    fn from_mutating(webhook: &'a MutatingWebhook) -> Self {
        Self {
            name: &webhook.name,
            webhook_type: "Mutating",
            failure_policy: &webhook.failure_policy,
            match_policy: &webhook.match_policy,
            side_effects: &webhook.side_effects,
            timeout_seconds: webhook.timeout_seconds,
            admission_review_versions: &webhook.admission_review_versions,
            namespace_selector: &webhook.namespace_selector,
            object_selector: &webhook.object_selector,
            rules: webhook.rules.as_ref(),
            client_config: &webhook.client_config,
        }
    }
}

fn convert_webhook_to_inventory(
    metadata: &ObjectMeta,
    cluster_id: &str,
    collected_at: DateTime<Utc>,
    parts: AdmissionWebhookParts<'_>,
) -> AdmissionWebhookInventoryItem {
    let rules = parts.rules.cloned().unwrap_or_default();
    let (client_url_scheme, client_url_host) = parts
        .client_config
        .url
        .as_deref()
        .map_or((None, None), parse_url);
    let service = parts.client_config.service.as_ref();

    AdmissionWebhookInventoryItem {
        cluster_id: cluster_id.to_string(),
        configuration_name: metadata.name.clone().unwrap_or_default(),
        webhook_name: parts.name.to_string(),
        webhook_type: parts.webhook_type.to_string(),
        labels: metadata.labels.clone().unwrap_or_default(),
        annotations: metadata.annotations.clone().unwrap_or_default(),
        failure_policy: parts.failure_policy.clone(),
        match_policy: parts.match_policy.clone(),
        side_effects: Some(parts.side_effects.to_string()),
        timeout_seconds: parts.timeout_seconds,
        admission_review_versions: parts.admission_review_versions.to_vec(),
        namespace_selector_present: selector_has_constraints(parts.namespace_selector),
        object_selector_present: selector_has_constraints(parts.object_selector),
        rules_count: rules.len(),
        operations: collect_rule_values(&rules, |rule| rule.operations.as_ref()),
        api_groups: collect_rule_values(&rules, |rule| rule.api_groups.as_ref()),
        api_versions: collect_rule_values(&rules, |rule| rule.api_versions.as_ref()),
        resources: collect_rule_values(&rules, |rule| rule.resources.as_ref()),
        scope: collect_scope(&rules),
        client_service_namespace: service.map(|service| service.namespace.clone()),
        client_service_name: service.map(|service| service.name.clone()),
        client_service_path: service.and_then(|service| service.path.clone()),
        client_service_port: service.and_then(|service| service.port),
        client_url_host,
        client_url_scheme,
        ca_bundle_present: parts
            .client_config
            .ca_bundle
            .as_ref()
            .map(|bundle| !bundle.0.is_empty())
            .unwrap_or(false),
        created_at: metadata
            .creation_timestamp
            .as_ref()
            .map(|timestamp| timestamp.0),
        collected_at,
    }
}

fn selector_has_constraints(selector: &Option<LabelSelector>) -> bool {
    selector
        .as_ref()
        .map(|selector| {
            selector
                .match_labels
                .as_ref()
                .map(|labels| !labels.is_empty())
                .unwrap_or(false)
                || selector
                    .match_expressions
                    .as_ref()
                    .map(|expressions| !expressions.is_empty())
                    .unwrap_or(false)
        })
        .unwrap_or(false)
}

fn collect_rule_values<F>(rules: &[RuleWithOperations], get_values: F) -> Vec<String>
where
    F: Fn(&RuleWithOperations) -> Option<&Vec<String>>,
{
    let mut values = rules
        .iter()
        .flat_map(|rule| get_values(rule).into_iter().flatten().cloned())
        .collect::<Vec<_>>();
    values.sort();
    values.dedup();
    values
}

fn collect_scope(rules: &[RuleWithOperations]) -> Option<String> {
    let mut scopes = rules
        .iter()
        .filter_map(|rule| rule.scope.clone())
        .collect::<Vec<_>>();
    scopes.sort();
    scopes.dedup();
    match scopes.as_slice() {
        [] => None,
        [scope] => Some(scope.clone()),
        _ => Some("*".to_string()),
    }
}

fn parse_url(url: &str) -> (Option<String>, Option<String>) {
    let Some((scheme, rest)) = url.split_once("://") else {
        return (None, None);
    };
    let host = rest
        .split('/')
        .next()
        .unwrap_or_default()
        .split('@')
        .next_back()
        .unwrap_or_default()
        .split(':')
        .next()
        .filter(|host| !host.is_empty())
        .map(str::to_string);
    (Some(scheme.to_string()), host)
}

impl AdmissionWebhooksService {
    pub fn new() -> Self {
        Self
    }

    pub async fn list_inventory(
        &self,
        cluster_config: &KubernetesClusterConfig,
        cluster_id: &str,
    ) -> Result<Vec<AdmissionWebhookInventoryItem>, AppError> {
        let client = ClientFactory::get_client(cluster_config).await?;
        let collected_at = Utc::now();
        let validating_api: Api<ValidatingWebhookConfiguration> = Api::all(client.clone());
        let mutating_api: Api<MutatingWebhookConfiguration> = Api::all(client);

        let validating = validating_api
            .list(&ListParams::default())
            .await
            .map_err(|e| {
                AppError::ExternalService(format!(
                    "Failed to list ValidatingWebhookConfiguration inventory: {}",
                    e
                ))
            })?;
        let mutating = mutating_api
            .list(&ListParams::default())
            .await
            .map_err(|e| {
                AppError::ExternalService(format!(
                    "Failed to list MutatingWebhookConfiguration inventory: {}",
                    e
                ))
            })?;

        let mut inventory = validating
            .items
            .iter()
            .flat_map(|config| {
                convert_validating_webhook_configuration_to_inventory(
                    config,
                    cluster_id,
                    collected_at,
                )
            })
            .chain(mutating.items.iter().flat_map(|config| {
                convert_mutating_webhook_configuration_to_inventory(
                    config,
                    cluster_id,
                    collected_at,
                )
            }))
            .collect::<Vec<_>>();

        inventory.sort_by(|left, right| {
            left.webhook_type
                .cmp(&right.webhook_type)
                .then_with(|| left.configuration_name.cmp(&right.configuration_name))
                .then_with(|| left.webhook_name.cmp(&right.webhook_name))
        });
        Ok(inventory)
    }
}

impl Default for AdmissionWebhooksService {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use k8s_openapi::api::admissionregistration::v1::{ServiceReference, WebhookClientConfig};
    use k8s_openapi::apimachinery::pkg::apis::meta::v1::{LabelSelector, Time};
    use k8s_openapi::ByteString;
    use std::collections::BTreeMap;

    #[test]
    fn validating_webhook_inventory_conversion_preserves_rules_and_service_target() {
        let collected_at = Utc.with_ymd_and_hms(2026, 6, 10, 0, 0, 0).unwrap();
        let created_at = Utc.with_ymd_and_hms(2026, 6, 9, 23, 0, 0).unwrap();
        let config = ValidatingWebhookConfiguration {
            metadata: ObjectMeta {
                name: Some("platform-validators".to_string()),
                labels: Some(BTreeMap::from([
                    ("owner".to_string(), "platform".to_string()),
                    ("cost-center".to_string(), "cc-42".to_string()),
                ])),
                annotations: Some(BTreeMap::from([(
                    "mayyam.io/runbook".to_string(),
                    "admission".to_string(),
                )])),
                creation_timestamp: Some(Time(created_at)),
                ..Default::default()
            },
            webhooks: Some(vec![ValidatingWebhook {
                name: "validate.platform.example.com".to_string(),
                admission_review_versions: vec!["v1".to_string()],
                client_config: WebhookClientConfig {
                    ca_bundle: Some(ByteString(vec![1, 2, 3])),
                    service: Some(ServiceReference {
                        namespace: "platform".to_string(),
                        name: "validator".to_string(),
                        path: Some("/validate".to_string()),
                        port: Some(8443),
                    }),
                    ..Default::default()
                },
                failure_policy: Some("Fail".to_string()),
                match_policy: Some("Equivalent".to_string()),
                namespace_selector: Some(LabelSelector {
                    match_labels: Some(BTreeMap::from([(
                        "environment".to_string(),
                        "prod".to_string(),
                    )])),
                    ..Default::default()
                }),
                rules: Some(vec![RuleWithOperations {
                    operations: Some(vec!["CREATE".to_string(), "UPDATE".to_string()]),
                    api_groups: Some(vec!["apps".to_string()]),
                    api_versions: Some(vec!["v1".to_string()]),
                    resources: Some(vec!["deployments".to_string()]),
                    scope: Some("Namespaced".to_string()),
                }]),
                side_effects: "None".to_string(),
                timeout_seconds: Some(5),
                ..Default::default()
            }]),
        };

        let inventory = convert_validating_webhook_configuration_to_inventory(
            &config,
            "cluster-a",
            collected_at,
        );

        assert_eq!(inventory.len(), 1);
        let webhook = &inventory[0];
        assert_eq!(webhook.cluster_id, "cluster-a");
        assert_eq!(webhook.configuration_name, "platform-validators");
        assert_eq!(webhook.webhook_name, "validate.platform.example.com");
        assert_eq!(webhook.webhook_type, "Validating");
        assert_eq!(webhook.failure_policy.as_deref(), Some("Fail"));
        assert_eq!(webhook.match_policy.as_deref(), Some("Equivalent"));
        assert_eq!(webhook.side_effects.as_deref(), Some("None"));
        assert_eq!(webhook.timeout_seconds, Some(5));
        assert_eq!(webhook.admission_review_versions, vec!["v1"]);
        assert!(webhook.namespace_selector_present);
        assert!(!webhook.object_selector_present);
        assert_eq!(webhook.rules_count, 1);
        assert_eq!(webhook.operations, vec!["CREATE", "UPDATE"]);
        assert_eq!(webhook.api_groups, vec!["apps"]);
        assert_eq!(webhook.api_versions, vec!["v1"]);
        assert_eq!(webhook.resources, vec!["deployments"]);
        assert_eq!(webhook.scope.as_deref(), Some("Namespaced"));
        assert_eq!(
            webhook.client_service_namespace.as_deref(),
            Some("platform")
        );
        assert_eq!(webhook.client_service_name.as_deref(), Some("validator"));
        assert_eq!(webhook.client_service_path.as_deref(), Some("/validate"));
        assert_eq!(webhook.client_service_port, Some(8443));
        assert_eq!(webhook.client_url_scheme, None);
        assert_eq!(webhook.client_url_host, None);
        assert!(webhook.ca_bundle_present);
        assert_eq!(webhook.created_at, Some(created_at));
        assert_eq!(webhook.collected_at, collected_at);
    }

    #[test]
    fn mutating_webhook_inventory_conversion_preserves_url_target_and_wildcards() {
        let collected_at = Utc.with_ymd_and_hms(2026, 6, 10, 0, 0, 0).unwrap();
        let config = MutatingWebhookConfiguration {
            metadata: ObjectMeta {
                name: Some("platform-mutators".to_string()),
                ..Default::default()
            },
            webhooks: Some(vec![MutatingWebhook {
                name: "mutate.platform.example.com".to_string(),
                admission_review_versions: vec!["v1".to_string()],
                client_config: WebhookClientConfig {
                    url: Some("http://webhook.example.com/mutate".to_string()),
                    ..Default::default()
                },
                failure_policy: Some("Ignore".to_string()),
                match_policy: Some("Equivalent".to_string()),
                rules: Some(vec![RuleWithOperations {
                    operations: Some(vec!["*".to_string()]),
                    api_groups: Some(vec!["*".to_string()]),
                    api_versions: Some(vec!["*".to_string()]),
                    resources: Some(vec!["*/*".to_string()]),
                    scope: Some("*".to_string()),
                }]),
                side_effects: "Some".to_string(),
                timeout_seconds: Some(30),
                ..Default::default()
            }]),
        };

        let inventory =
            convert_mutating_webhook_configuration_to_inventory(&config, "cluster-a", collected_at);

        assert_eq!(inventory.len(), 1);
        let webhook = &inventory[0];
        assert_eq!(webhook.configuration_name, "platform-mutators");
        assert_eq!(webhook.webhook_type, "Mutating");
        assert_eq!(webhook.client_url_scheme.as_deref(), Some("http"));
        assert_eq!(
            webhook.client_url_host.as_deref(),
            Some("webhook.example.com")
        );
        assert!(!webhook.ca_bundle_present);
        assert!(!webhook.namespace_selector_present);
        assert_eq!(webhook.operations, vec!["*"]);
        assert_eq!(webhook.api_groups, vec!["*"]);
        assert_eq!(webhook.api_versions, vec!["*"]);
        assert_eq!(webhook.resources, vec!["*/*"]);
        assert_eq!(webhook.scope.as_deref(), Some("*"));
        assert_eq!(webhook.side_effects.as_deref(), Some("Some"));
        assert_eq!(webhook.timeout_seconds, Some(30));
    }
}

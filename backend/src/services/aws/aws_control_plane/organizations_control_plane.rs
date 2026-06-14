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
use crate::models::aws_account::AwsAccountDto;
use crate::models::aws_resource::{AwsResourceDto, AwsResourceType, Model as AwsResourceModel};
use crate::services::aws::client_factory::AwsClientFactory;
use crate::services::AwsService;
use aws_sdk_organizations::types::{Account, OrganizationalUnit, PolicySummary, PolicyType, Root};
use aws_smithy_types::date_time::Format;
use serde_json::{json, Value};
use std::sync::Arc;
use tracing::{debug, error};
use uuid::Uuid;

const COST_ALLOCATION_TAG_KEYS: &[&str] = &[
    "owner",
    "team",
    "cost-center",
    "costcenter",
    "cost_center",
    "project",
];

pub struct OrganizationsControlPlane {
    aws_service: Arc<AwsService>,
}

impl OrganizationsControlPlane {
    pub fn new(aws_service: Arc<AwsService>) -> Self {
        Self { aws_service }
    }

    pub async fn sync_organization(
        &self,
        aws_account_dto: &AwsAccountDto,
        sync_id: Uuid,
    ) -> Result<Vec<AwsResourceModel>, AppError> {
        debug!(
            "Syncing AWS Organizations inventory for account: {} with sync_id: {}",
            &aws_account_dto.account_id, sync_id
        );

        let client = self
            .aws_service
            .create_organizations_client(aws_account_dto)
            .await?;
        let describe = client.describe_organization().send().await.map_err(|e| {
            error!("Failed to describe AWS Organizations organization: {}", e);
            AppError::ExternalService(format!(
                "Failed to describe AWS Organizations organization: {}",
                e
            ))
        })?;

        let organization = describe.organization();
        let organization_id = organization
            .and_then(|org| org.id())
            .map(String::from)
            .unwrap_or_else(|| aws_account_dto.account_id.clone());
        let management_account_id = organization
            .and_then(|org| org.master_account_id())
            .map(String::from);
        let arn = organization
            .and_then(|org| org.arn())
            .map(String::from)
            .unwrap_or_else(|| fallback_organization_arn(aws_account_dto, &organization_id));

        let roots = collect_roots(&client).await;
        let accounts = collect_accounts(&client).await;
        let organizational_units = collect_organizational_units(&client, &roots).await;
        let service_control_policies = collect_service_control_policies(&client).await;
        let tags =
            collect_resource_tags(&client, &organization_id, management_account_id.as_deref())
                .await;

        let account_count = accounts.len();
        let active_account_count = count_account_status(&accounts, "ACTIVE");
        let suspended_account_count = count_account_status(&accounts, "SUSPENDED");
        let cost_allocation_tagged_account_count = accounts
            .iter()
            .filter(|account| {
                account
                    .get("has_cost_allocation_tag")
                    .and_then(|value| value.as_bool())
                    .unwrap_or(false)
            })
            .count();
        let service_control_policy_enabled = roots.iter().any(root_has_enabled_scp);

        let mut resource_data = serde_json::Map::new();
        resource_data.insert("organization_id".to_string(), json!(organization_id));
        resource_data.insert(
            "organization_arn".to_string(),
            organization
                .and_then(|org| org.arn())
                .map(String::from)
                .map(Value::String)
                .unwrap_or(Value::Null),
        );
        resource_data.insert(
            "feature_set".to_string(),
            json!(organization
                .and_then(|org| org.feature_set())
                .map(|value| value.as_str())),
        );
        resource_data.insert(
            "management_account_id".to_string(),
            json!(management_account_id),
        );
        resource_data.insert(
            "management_account_email".to_string(),
            json!(organization.and_then(|org| org.master_account_email())),
        );
        resource_data.insert(
            "management_account_arn".to_string(),
            json!(organization.and_then(|org| org.master_account_arn())),
        );
        resource_data.insert("root_count".to_string(), json!(roots.len()));
        resource_data.insert("roots".to_string(), json!(roots));
        resource_data.insert("account_count".to_string(), json!(account_count));
        resource_data.insert(
            "active_account_count".to_string(),
            json!(active_account_count),
        );
        resource_data.insert(
            "suspended_account_count".to_string(),
            json!(suspended_account_count),
        );
        resource_data.insert(
            "cost_allocation_tagged_account_count".to_string(),
            json!(cost_allocation_tagged_account_count),
        );
        resource_data.insert(
            "untagged_account_count".to_string(),
            json!(account_count.saturating_sub(cost_allocation_tagged_account_count)),
        );
        resource_data.insert("accounts".to_string(), json!(accounts));
        resource_data.insert(
            "organizational_unit_count".to_string(),
            json!(organizational_units.len()),
        );
        resource_data.insert(
            "organizational_units".to_string(),
            json!(organizational_units),
        );
        resource_data.insert(
            "service_control_policy_enabled".to_string(),
            json!(service_control_policy_enabled),
        );
        resource_data.insert(
            "service_control_policy_count".to_string(),
            json!(service_control_policies.len()),
        );
        resource_data.insert(
            "service_control_policies".to_string(),
            json!(service_control_policies),
        );

        let dto = AwsResourceDto {
            id: None,
            sync_id: Some(sync_id),
            account_id: aws_account_dto.account_id.clone(),
            profile: aws_account_dto.profile.clone(),
            region: "aws-global".to_string(),
            resource_type: AwsResourceType::OrganizationsOrganization.to_string(),
            resource_id: format!("organizations:{}", organization_id),
            arn,
            name: Some(organization_id),
            tags,
            resource_data: Value::Object(resource_data),
        };

        debug!(
            "Successfully synced AWS Organizations inventory for account: {} with sync_id: {}",
            &aws_account_dto.account_id, sync_id
        );

        Ok(vec![dto.into()])
    }
}

async fn collect_roots(client: &aws_sdk_organizations::Client) -> Vec<Value> {
    let mut roots = Vec::new();
    let mut next_token: Option<String> = None;

    loop {
        let mut request = client.list_roots();
        if let Some(token) = next_token {
            request = request.next_token(token);
        }

        let response = match request.send().await {
            Ok(response) => response,
            Err(e) => {
                debug!("Failed to list AWS Organizations roots: {}", e);
                return roots;
            }
        };

        roots.extend(response.roots().iter().map(root_to_json));
        next_token = response.next_token().map(String::from);
        if next_token.is_none() {
            break;
        }
    }

    roots
}

async fn collect_accounts(client: &aws_sdk_organizations::Client) -> Vec<Value> {
    let mut accounts = Vec::new();
    let mut next_token: Option<String> = None;

    loop {
        let mut request = client.list_accounts();
        if let Some(token) = next_token {
            request = request.next_token(token);
        }

        let response = match request.send().await {
            Ok(response) => response,
            Err(e) => {
                debug!("Failed to list AWS Organizations accounts: {}", e);
                return accounts;
            }
        };

        for account in response.accounts() {
            let tags = match account.id() {
                Some(id) => list_tags(client, id).await,
                None => json!({}),
            };
            accounts.push(account_to_json(account, tags));
        }

        next_token = response.next_token().map(String::from);
        if next_token.is_none() {
            break;
        }
    }

    accounts
}

async fn collect_organizational_units(
    client: &aws_sdk_organizations::Client,
    roots: &[Value],
) -> Vec<Value> {
    let mut units = Vec::new();

    for root in roots {
        let Some(parent_id) = root.get("id").and_then(|value| value.as_str()) else {
            continue;
        };
        let mut next_token: Option<String> = None;
        loop {
            let mut request = client
                .list_organizational_units_for_parent()
                .parent_id(parent_id);
            if let Some(token) = next_token {
                request = request.next_token(token);
            }

            let response = match request.send().await {
                Ok(response) => response,
                Err(e) => {
                    debug!(
                        "Failed to list AWS Organizations OUs for parent {}: {}",
                        parent_id, e
                    );
                    break;
                }
            };

            units.extend(
                response
                    .organizational_units()
                    .iter()
                    .map(|unit| organizational_unit_to_json(parent_id, unit)),
            );
            next_token = response.next_token().map(String::from);
            if next_token.is_none() {
                break;
            }
        }
    }

    units
}

async fn collect_service_control_policies(client: &aws_sdk_organizations::Client) -> Vec<Value> {
    let mut policies = Vec::new();
    let mut next_token: Option<String> = None;

    loop {
        let mut request = client
            .list_policies()
            .filter(PolicyType::ServiceControlPolicy);
        if let Some(token) = next_token {
            request = request.next_token(token);
        }

        let response = match request.send().await {
            Ok(response) => response,
            Err(e) => {
                debug!(
                    "Failed to list AWS Organizations service control policies: {}",
                    e
                );
                return policies;
            }
        };

        policies.extend(response.policies().iter().map(policy_to_json));
        next_token = response.next_token().map(String::from);
        if next_token.is_none() {
            break;
        }
    }

    policies
}

async fn collect_resource_tags(
    client: &aws_sdk_organizations::Client,
    organization_id: &str,
    management_account_id: Option<&str>,
) -> Value {
    let tags = list_tags(client, organization_id).await;
    if tags.as_object().map(|map| !map.is_empty()).unwrap_or(false) {
        return tags;
    }

    match management_account_id {
        Some(account_id) => list_tags(client, account_id).await,
        None => tags,
    }
}

async fn list_tags(client: &aws_sdk_organizations::Client, resource_id: &str) -> Value {
    let mut tags_map = serde_json::Map::new();
    let mut next_token: Option<String> = None;

    loop {
        let mut request = client.list_tags_for_resource().resource_id(resource_id);
        if let Some(token) = next_token {
            request = request.next_token(token);
        }

        let response = match request.send().await {
            Ok(response) => response,
            Err(e) => {
                debug!(
                    "Failed to list AWS Organizations tags for {}: {}",
                    resource_id, e
                );
                return Value::Object(tags_map);
            }
        };

        for tag in response.tags() {
            tags_map.insert(tag.key().to_string(), json!(tag.value()));
        }

        next_token = response.next_token().map(String::from);
        if next_token.is_none() {
            break;
        }
    }

    Value::Object(tags_map)
}

fn root_to_json(root: &Root) -> Value {
    let policy_types: Vec<Value> = root
        .policy_types()
        .iter()
        .map(|policy_type| {
            json!({
                "type": policy_type.r#type().map(|value| value.as_str()),
                "status": policy_type.status().map(|value| value.as_str()),
            })
        })
        .collect();
    let service_control_policy_enabled = policy_types_have_enabled_scp(&policy_types);

    json!({
        "id": root.id(),
        "arn": root.arn(),
        "name": root.name(),
        "policy_types": policy_types,
        "service_control_policy_enabled": service_control_policy_enabled,
    })
}

fn account_to_json(account: &Account, tags: Value) -> Value {
    json!({
        "id": account.id(),
        "arn": account.arn(),
        "email": account.email(),
        "name": account.name(),
        "status": account.status().map(|value| value.as_str()),
        "joined_method": account.joined_method().map(|value| value.as_str()),
        "joined_timestamp": fmt_date(account.joined_timestamp()),
        "tags": tags,
        "has_cost_allocation_tag": has_cost_allocation_tag(&tags),
    })
}

fn organizational_unit_to_json(parent_id: &str, unit: &OrganizationalUnit) -> Value {
    json!({
        "id": unit.id(),
        "arn": unit.arn(),
        "name": unit.name(),
        "parent_id": parent_id,
    })
}

fn policy_to_json(policy: &PolicySummary) -> Value {
    json!({
        "id": policy.id(),
        "arn": policy.arn(),
        "name": policy.name(),
        "description": policy.description(),
        "type": policy.r#type().map(|value| value.as_str()),
        "aws_managed": policy.aws_managed(),
    })
}

fn root_has_enabled_scp(root: &Value) -> bool {
    root.get("policy_types")
        .and_then(|value| value.as_array())
        .map(|policy_types| policy_types_have_enabled_scp(policy_types))
        .unwrap_or(false)
}

fn policy_types_have_enabled_scp(policy_types: &[Value]) -> bool {
    policy_types.iter().any(|policy_type| {
        policy_type.get("type").and_then(|value| value.as_str()) == Some("SERVICE_CONTROL_POLICY")
            && policy_type.get("status").and_then(|value| value.as_str()) == Some("ENABLED")
    })
}

fn count_account_status(accounts: &[Value], wanted: &str) -> usize {
    accounts
        .iter()
        .filter(|account| {
            account
                .get("status")
                .and_then(|value| value.as_str())
                .map(|status| status.eq_ignore_ascii_case(wanted))
                .unwrap_or(false)
        })
        .count()
}

fn has_cost_allocation_tag(tags: &Value) -> bool {
    let Some(map) = tags.as_object() else {
        return false;
    };

    COST_ALLOCATION_TAG_KEYS
        .iter()
        .any(|wanted| map.keys().any(|key| key.eq_ignore_ascii_case(wanted)))
}

fn fallback_organization_arn(aws_account_dto: &AwsAccountDto, organization_id: &str) -> String {
    format!(
        "arn:aws:organizations::{}:organization/{}",
        aws_account_dto.account_id, organization_id
    )
}

fn fmt_date(value: Option<&aws_smithy_types::DateTime>) -> Option<String> {
    value.and_then(|date| date.fmt(Format::DateTime).ok())
}

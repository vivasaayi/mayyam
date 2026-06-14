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
use aws_sdk_servicecatalog::types::{
    BudgetDetail, ConstraintDetail, DescribePortfolioShareType, LaunchPathSummary, PortfolioDetail,
    PortfolioShareDetail, Principal, ProductViewDetail, ProvisioningArtifactDetail, Tag,
    TagOptionDetail,
};
use aws_smithy_types::date_time::Format;
use serde_json::{json, Value};
use std::sync::Arc;
use tracing::{debug, error};
use uuid::Uuid;

pub struct ServiceCatalogControlPlane {
    aws_service: Arc<AwsService>,
}

impl ServiceCatalogControlPlane {
    pub fn new(aws_service: Arc<AwsService>) -> Self {
        Self { aws_service }
    }

    pub async fn sync_portfolios(
        &self,
        aws_account_dto: &AwsAccountDto,
        sync_id: Uuid,
    ) -> Result<Vec<AwsResourceModel>, AppError> {
        debug!(
            "Syncing AWS Service Catalog portfolios for account: {} with sync_id: {}",
            &aws_account_dto.account_id, sync_id
        );

        let client = self
            .aws_service
            .create_servicecatalog_client(aws_account_dto)
            .await?;
        let portfolios = list_portfolios(&client).await.map_err(|e| {
            error!("Failed to list AWS Service Catalog portfolios: {}", e);
            AppError::ExternalService(format!(
                "Failed to list AWS Service Catalog portfolios: {}",
                e
            ))
        })?;

        if portfolios.is_empty() {
            return Ok(vec![account_level_resource(aws_account_dto, sync_id)]);
        }

        let mut resources = Vec::new();
        for portfolio in portfolios {
            let Some(portfolio_id) = portfolio.id() else {
                continue;
            };
            let described = describe_portfolio(&client, portfolio_id).await;
            let portfolio_detail = described
                .as_ref()
                .and_then(|d| d.portfolio_detail())
                .unwrap_or(&portfolio);
            let tags = described
                .as_ref()
                .map(|d| tags_to_value(d.tags()))
                .unwrap_or_else(|| json!({}));
            let tag_options = described
                .as_ref()
                .map(|d| tag_options_to_value(d.tag_options()))
                .unwrap_or_else(|| json!([]));
            let budgets = described
                .as_ref()
                .map(|d| budgets_to_value(d.budgets()))
                .unwrap_or_else(|| json!([]));

            let products = collect_products_for_portfolio(&client, portfolio_id).await;
            let constraints = collect_constraints(&client, portfolio_id, None).await;
            let principals = collect_principals(&client, portfolio_id).await;
            let shares = collect_portfolio_shares(&client, portfolio_id).await;

            resources.push(portfolio_resource(
                aws_account_dto,
                sync_id,
                portfolio_detail,
                tags,
                tag_options,
                budgets,
                products,
                constraints,
                principals,
                shares,
            ));
        }

        debug!(
            "Successfully synced {} AWS Service Catalog portfolio resources for account: {} with sync_id: {}",
            resources.len(),
            &aws_account_dto.account_id,
            sync_id
        );

        Ok(resources)
    }
}

async fn list_portfolios(
    client: &aws_sdk_servicecatalog::Client,
) -> Result<Vec<PortfolioDetail>, String> {
    let mut portfolios = Vec::new();
    let mut next_token: Option<String> = None;

    loop {
        let mut request = client.list_portfolios().page_size(20);
        if let Some(token) = next_token {
            request = request.page_token(token);
        }

        let response = request.send().await.map_err(|e| e.to_string())?;
        portfolios.extend(response.portfolio_details().iter().cloned());
        next_token = response.next_page_token().map(String::from);
        if next_token.is_none() {
            break;
        }
    }

    Ok(portfolios)
}

async fn describe_portfolio(
    client: &aws_sdk_servicecatalog::Client,
    portfolio_id: &str,
) -> Option<aws_sdk_servicecatalog::operation::describe_portfolio::DescribePortfolioOutput> {
    match client.describe_portfolio().id(portfolio_id).send().await {
        Ok(response) => Some(response),
        Err(e) => {
            debug!(
                "Failed to describe AWS Service Catalog portfolio {}: {}",
                portfolio_id, e
            );
            None
        }
    }
}

async fn collect_products_for_portfolio(
    client: &aws_sdk_servicecatalog::Client,
    portfolio_id: &str,
) -> Value {
    let product_details = match search_products(client, portfolio_id).await {
        Ok(products) => products,
        Err(e) => {
            debug!(
                "Failed to search AWS Service Catalog products for portfolio {}: {}",
                portfolio_id, e
            );
            return json!([]);
        }
    };

    let mut products = Vec::new();
    for product in product_details {
        let Some(summary) = product.product_view_summary() else {
            continue;
        };
        let Some(product_id) = summary.product_id().or_else(|| summary.id()) else {
            continue;
        };

        let described = describe_product(client, product_id).await;
        let detail = described.as_ref().and_then(|d| d.product_view_detail());
        let detail_summary = detail
            .and_then(|d| d.product_view_summary())
            .or_else(|| Some(summary));
        let product_tags = described
            .as_ref()
            .map(|d| tags_to_value(d.tags()))
            .unwrap_or_else(|| json!([]));
        let product_tag_options = described
            .as_ref()
            .map(|d| tag_options_to_value(d.tag_options()))
            .unwrap_or_else(|| json!([]));
        let product_budgets = described
            .as_ref()
            .map(|d| budgets_to_value(d.budgets()))
            .unwrap_or_else(|| json!([]));

        let artifacts = collect_provisioning_artifacts(client, product_id).await;
        let launch_paths = collect_launch_paths(client, product_id).await;
        let product_constraints = collect_constraints(client, portfolio_id, Some(product_id)).await;

        products.push(json!({
            "product_id": product_id,
            "product_view_id": detail_summary.and_then(|s| s.id()),
            "name": detail_summary.and_then(|s| s.name()),
            "owner": detail_summary.and_then(|s| s.owner()),
            "distributor": detail_summary.and_then(|s| s.distributor()),
            "product_type": detail_summary.and_then(|s| s.r#type()).map(|v| v.as_str()),
            "status": detail.and_then(|d| d.status()).map(|v| v.as_str()),
            "product_arn": detail.and_then(|d| d.product_arn()),
            "short_description": detail_summary.and_then(|s| s.short_description()),
            "support_email": detail_summary.and_then(|s| s.support_email()),
            "support_url": detail_summary.and_then(|s| s.support_url()),
            "has_default_path": detail_summary.map(|s| s.has_default_path()).unwrap_or(false),
            "created_time": detail.and_then(|d| fmt_date(d.created_time())),
            "tags": product_tags,
            "tag_options": product_tag_options,
            "budgets": product_budgets,
            "provisioning_artifacts": artifacts,
            "launch_paths": launch_paths,
            "constraints": product_constraints,
        }));
    }

    Value::Array(products)
}

async fn search_products(
    client: &aws_sdk_servicecatalog::Client,
    portfolio_id: &str,
) -> Result<Vec<ProductViewDetail>, String> {
    let mut products = Vec::new();
    let mut next_token: Option<String> = None;

    loop {
        let mut request = client
            .search_products_as_admin()
            .portfolio_id(portfolio_id)
            .page_size(20);
        if let Some(token) = next_token {
            request = request.page_token(token);
        }

        let response = request.send().await.map_err(|e| e.to_string())?;
        products.extend(response.product_view_details().iter().cloned());
        next_token = response.next_page_token().map(String::from);
        if next_token.is_none() {
            break;
        }
    }

    Ok(products)
}

async fn describe_product(
    client: &aws_sdk_servicecatalog::Client,
    product_id: &str,
) -> Option<
    aws_sdk_servicecatalog::operation::describe_product_as_admin::DescribeProductAsAdminOutput,
> {
    match client
        .describe_product_as_admin()
        .id(product_id)
        .send()
        .await
    {
        Ok(response) => Some(response),
        Err(e) => {
            debug!(
                "Failed to describe AWS Service Catalog product {}: {}",
                product_id, e
            );
            None
        }
    }
}

async fn collect_provisioning_artifacts(
    client: &aws_sdk_servicecatalog::Client,
    product_id: &str,
) -> Value {
    match client
        .list_provisioning_artifacts()
        .product_id(product_id)
        .send()
        .await
    {
        Ok(response) => Value::Array(
            response
                .provisioning_artifact_details()
                .iter()
                .map(provisioning_artifact_to_value)
                .collect(),
        ),
        Err(e) => {
            debug!(
                "Failed to list AWS Service Catalog provisioning artifacts for product {}: {}",
                product_id, e
            );
            json!([])
        }
    }
}

async fn collect_launch_paths(client: &aws_sdk_servicecatalog::Client, product_id: &str) -> Value {
    let mut paths = Vec::new();
    let mut next_token: Option<String> = None;

    loop {
        let mut request = client
            .list_launch_paths()
            .product_id(product_id)
            .page_size(20);
        if let Some(token) = next_token {
            request = request.page_token(token);
        }

        let response = match request.send().await {
            Ok(response) => response,
            Err(e) => {
                debug!(
                    "Failed to list AWS Service Catalog launch paths for product {}: {}",
                    product_id, e
                );
                break;
            }
        };

        paths.extend(
            response
                .launch_path_summaries()
                .iter()
                .map(launch_path_to_value),
        );
        next_token = response.next_page_token().map(String::from);
        if next_token.is_none() {
            break;
        }
    }

    Value::Array(paths)
}

async fn collect_constraints(
    client: &aws_sdk_servicecatalog::Client,
    portfolio_id: &str,
    product_id: Option<&str>,
) -> Value {
    let mut constraints = Vec::new();
    let mut next_token: Option<String> = None;

    loop {
        let mut request = client
            .list_constraints_for_portfolio()
            .portfolio_id(portfolio_id)
            .page_size(20);
        if let Some(product_id) = product_id {
            request = request.product_id(product_id);
        }
        if let Some(token) = next_token {
            request = request.page_token(token);
        }

        let response = match request.send().await {
            Ok(response) => response,
            Err(e) => {
                debug!(
                    "Failed to list AWS Service Catalog constraints for portfolio {}: {}",
                    portfolio_id, e
                );
                break;
            }
        };

        constraints.extend(
            response
                .constraint_details()
                .iter()
                .map(constraint_to_value),
        );
        next_token = response.next_page_token().map(String::from);
        if next_token.is_none() {
            break;
        }
    }

    Value::Array(constraints)
}

async fn collect_principals(client: &aws_sdk_servicecatalog::Client, portfolio_id: &str) -> Value {
    let mut principals = Vec::new();
    let mut next_token: Option<String> = None;

    loop {
        let mut request = client
            .list_principals_for_portfolio()
            .portfolio_id(portfolio_id)
            .page_size(20);
        if let Some(token) = next_token {
            request = request.page_token(token);
        }

        let response = match request.send().await {
            Ok(response) => response,
            Err(e) => {
                debug!(
                    "Failed to list AWS Service Catalog principals for portfolio {}: {}",
                    portfolio_id, e
                );
                break;
            }
        };

        principals.extend(response.principals().iter().map(principal_to_value));
        next_token = response.next_page_token().map(String::from);
        if next_token.is_none() {
            break;
        }
    }

    Value::Array(principals)
}

async fn collect_portfolio_shares(
    client: &aws_sdk_servicecatalog::Client,
    portfolio_id: &str,
) -> Value {
    let share_types = [
        DescribePortfolioShareType::Account,
        DescribePortfolioShareType::Organization,
        DescribePortfolioShareType::OrganizationalUnit,
        DescribePortfolioShareType::OrganizationMemberAccount,
    ];
    let mut shares = Vec::new();

    for share_type in share_types {
        let mut next_token: Option<String> = None;
        loop {
            let mut request = client
                .describe_portfolio_shares()
                .portfolio_id(portfolio_id)
                .r#type(share_type.clone())
                .page_size(20);
            if let Some(token) = next_token {
                request = request.page_token(token);
            }

            let response = match request.send().await {
                Ok(response) => response,
                Err(e) => {
                    debug!(
                        "Failed to describe AWS Service Catalog portfolio shares for portfolio {} and type {}: {}",
                        portfolio_id,
                        share_type.as_str(),
                        e
                    );
                    break;
                }
            };

            shares.extend(
                response
                    .portfolio_share_details()
                    .iter()
                    .map(share_to_value),
            );
            next_token = response.next_page_token().map(String::from);
            if next_token.is_none() {
                break;
            }
        }
    }

    Value::Array(shares)
}

fn portfolio_resource(
    aws_account_dto: &AwsAccountDto,
    sync_id: Uuid,
    portfolio: &PortfolioDetail,
    tags: Value,
    tag_options: Value,
    budgets: Value,
    products: Value,
    constraints: Value,
    principals: Value,
    portfolio_shares: Value,
) -> AwsResourceModel {
    let portfolio_id = portfolio.id().unwrap_or("unknown");
    let arn = portfolio
        .arn()
        .map(String::from)
        .unwrap_or_else(|| fallback_portfolio_arn(aws_account_dto, portfolio_id));

    let product_count = products.as_array().map(Vec::len).unwrap_or(0);
    let budget_count = budgets.as_array().map(Vec::len).unwrap_or(0);
    let constraint_count = constraints.as_array().map(Vec::len).unwrap_or(0);
    let principal_count = principals.as_array().map(Vec::len).unwrap_or(0);
    let portfolio_share_count = portfolio_shares.as_array().map(Vec::len).unwrap_or(0);
    let active_cost_tag_option_count = count_active_cost_tag_options(&tag_options);
    let inactive_tag_option_count = count_inactive_tag_options(&tag_options);
    let active_provisioning_artifact_count = count_active_artifacts(&products);
    let deprecated_provisioning_artifact_count = count_deprecated_artifacts(&products);
    let launch_path_count = count_launch_paths(&products);
    let has_default_launch_path = any_product_has_default_path(&products);
    let broad_share_count = count_broad_shares(&portfolio_shares);

    let mut resource_data = serde_json::Map::new();
    resource_data.insert("asset_kind".to_string(), json!("portfolio"));
    resource_data.insert("portfolio_id".to_string(), json!(portfolio_id));
    resource_data.insert("portfolio_arn".to_string(), json!(arn));
    if let Some(display_name) = portfolio.display_name() {
        resource_data.insert("display_name".to_string(), json!(display_name));
    }
    if let Some(description) = portfolio.description() {
        resource_data.insert("description".to_string(), json!(description));
    }
    if let Some(provider_name) = portfolio.provider_name() {
        resource_data.insert("provider_name".to_string(), json!(provider_name));
    }
    if let Some(created_time) = fmt_date(portfolio.created_time()) {
        resource_data.insert("created_time".to_string(), json!(created_time));
    }
    resource_data.insert("product_count".to_string(), json!(product_count));
    resource_data.insert("budget_count".to_string(), json!(budget_count));
    resource_data.insert("constraint_count".to_string(), json!(constraint_count));
    resource_data.insert("principal_count".to_string(), json!(principal_count));
    resource_data.insert(
        "portfolio_share_count".to_string(),
        json!(portfolio_share_count),
    );
    resource_data.insert(
        "active_cost_tag_option_count".to_string(),
        json!(active_cost_tag_option_count),
    );
    resource_data.insert(
        "inactive_tag_option_count".to_string(),
        json!(inactive_tag_option_count),
    );
    resource_data.insert(
        "active_provisioning_artifact_count".to_string(),
        json!(active_provisioning_artifact_count),
    );
    resource_data.insert(
        "deprecated_provisioning_artifact_count".to_string(),
        json!(deprecated_provisioning_artifact_count),
    );
    resource_data.insert("launch_path_count".to_string(), json!(launch_path_count));
    resource_data.insert(
        "has_default_launch_path".to_string(),
        json!(has_default_launch_path),
    );
    resource_data.insert("broad_share_count".to_string(), json!(broad_share_count));
    resource_data.insert("tag_options".to_string(), tag_options);
    resource_data.insert("budgets".to_string(), budgets);
    resource_data.insert("products".to_string(), products);
    resource_data.insert("constraints".to_string(), constraints);
    resource_data.insert("principals".to_string(), principals);
    resource_data.insert("portfolio_shares".to_string(), portfolio_shares);

    let dto = AwsResourceDto {
        id: None,
        sync_id: Some(sync_id),
        account_id: aws_account_dto.account_id.clone(),
        profile: aws_account_dto.profile.clone(),
        region: aws_account_dto.default_region.clone(),
        resource_type: AwsResourceType::ServiceCatalogPortfolio.to_string(),
        resource_id: format!("servicecatalog:{}", portfolio_id),
        arn,
        name: portfolio.display_name().map(String::from),
        tags,
        resource_data: Value::Object(resource_data),
    };

    dto.into()
}

fn account_level_resource(aws_account_dto: &AwsAccountDto, sync_id: Uuid) -> AwsResourceModel {
    let dto = AwsResourceDto {
        id: None,
        sync_id: Some(sync_id),
        account_id: aws_account_dto.account_id.clone(),
        profile: aws_account_dto.profile.clone(),
        region: aws_account_dto.default_region.clone(),
        resource_type: AwsResourceType::ServiceCatalogPortfolio.to_string(),
        resource_id: format!("servicecatalog:{}", aws_account_dto.account_id),
        arn: fallback_portfolio_arn(aws_account_dto, "account"),
        name: Some("servicecatalog".to_string()),
        tags: json!({}),
        resource_data: json!({
            "asset_kind": "account",
            "portfolio_count": 0,
            "product_count": 0,
            "budget_count": 0,
            "constraint_count": 0,
            "principal_count": 0,
            "portfolio_share_count": 0,
            "active_cost_tag_option_count": 0,
            "inactive_tag_option_count": 0,
            "active_provisioning_artifact_count": 0,
            "deprecated_provisioning_artifact_count": 0,
            "launch_path_count": 0,
            "has_default_launch_path": false,
            "broad_share_count": 0,
            "tag_options": [],
            "budgets": [],
            "products": [],
            "constraints": [],
            "principals": [],
            "portfolio_shares": [],
        }),
    };

    dto.into()
}

fn tags_to_value(tags: &[Tag]) -> Value {
    let mut map = serde_json::Map::new();
    for tag in tags {
        map.insert(tag.key().to_string(), json!(tag.value()));
    }
    Value::Object(map)
}

fn tag_options_to_value(tag_options: &[TagOptionDetail]) -> Value {
    Value::Array(
        tag_options
            .iter()
            .map(|tag_option| {
                json!({
                    "id": tag_option.id(),
                    "key": tag_option.key(),
                    "value": tag_option.value(),
                    "active": tag_option.active(),
                    "owner": tag_option.owner(),
                })
            })
            .collect(),
    )
}

fn budgets_to_value(budgets: &[BudgetDetail]) -> Value {
    Value::Array(
        budgets
            .iter()
            .map(|budget| json!({ "budget_name": budget.budget_name() }))
            .collect(),
    )
}

fn provisioning_artifact_to_value(artifact: &ProvisioningArtifactDetail) -> Value {
    json!({
        "id": artifact.id(),
        "name": artifact.name(),
        "description": artifact.description(),
        "type": artifact.r#type().map(|v| v.as_str()),
        "created_time": fmt_date(artifact.created_time()),
        "active": artifact.active(),
        "guidance": artifact.guidance().map(|v| v.as_str()),
        "source_revision": artifact.source_revision(),
    })
}

fn launch_path_to_value(path: &LaunchPathSummary) -> Value {
    json!({
        "id": path.id(),
        "name": path.name(),
        "tags": tags_to_value(path.tags()),
        "constraints": path.constraint_summaries().iter().map(|constraint| {
            json!({
                "type": constraint.r#type(),
                "description": constraint.description(),
            })
        }).collect::<Vec<Value>>(),
    })
}

fn constraint_to_value(constraint: &ConstraintDetail) -> Value {
    json!({
        "constraint_id": constraint.constraint_id(),
        "type": constraint.r#type(),
        "description": constraint.description(),
        "owner": constraint.owner(),
        "product_id": constraint.product_id(),
        "portfolio_id": constraint.portfolio_id(),
    })
}

fn principal_to_value(principal: &Principal) -> Value {
    json!({
        "principal_arn": principal.principal_arn(),
        "principal_type": principal.principal_type().map(|v| v.as_str()),
    })
}

fn share_to_value(share: &PortfolioShareDetail) -> Value {
    json!({
        "principal_id": share.principal_id(),
        "type": share.r#type().map(|v| v.as_str()),
        "accepted": share.accepted(),
        "share_tag_options": share.share_tag_options(),
        "share_principals": share.share_principals(),
    })
}

fn count_active_cost_tag_options(tag_options: &Value) -> usize {
    tag_options
        .as_array()
        .map(|entries| {
            entries
                .iter()
                .filter(|entry| {
                    entry
                        .get("active")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false)
                        && entry
                            .get("key")
                            .and_then(|v| v.as_str())
                            .map(|key| {
                                matches!(
                                    key.trim().to_ascii_lowercase().as_str(),
                                    "owner"
                                        | "team"
                                        | "cost-center"
                                        | "costcenter"
                                        | "cost_center"
                                        | "project"
                                )
                            })
                            .unwrap_or(false)
                })
                .count()
        })
        .unwrap_or(0)
}

fn count_inactive_tag_options(tag_options: &Value) -> usize {
    tag_options
        .as_array()
        .map(|entries| {
            entries
                .iter()
                .filter(|entry| entry.get("active").and_then(|v| v.as_bool()) == Some(false))
                .count()
        })
        .unwrap_or(0)
}

fn count_active_artifacts(products: &Value) -> usize {
    products
        .as_array()
        .map(|products| {
            products
                .iter()
                .filter_map(|product| product.get("provisioning_artifacts")?.as_array())
                .flatten()
                .filter(|artifact| artifact.get("active").and_then(|v| v.as_bool()) == Some(true))
                .count()
        })
        .unwrap_or(0)
}

fn count_deprecated_artifacts(products: &Value) -> usize {
    products
        .as_array()
        .map(|products| {
            products
                .iter()
                .filter_map(|product| product.get("provisioning_artifacts")?.as_array())
                .flatten()
                .filter(|artifact| {
                    artifact
                        .get("guidance")
                        .and_then(|v| v.as_str())
                        .map(|value| value.eq_ignore_ascii_case("DEPRECATED"))
                        .unwrap_or(false)
                })
                .count()
        })
        .unwrap_or(0)
}

fn count_launch_paths(products: &Value) -> usize {
    products
        .as_array()
        .map(|products| {
            products
                .iter()
                .filter_map(|product| product.get("launch_paths")?.as_array())
                .map(Vec::len)
                .sum()
        })
        .unwrap_or(0)
}

fn any_product_has_default_path(products: &Value) -> bool {
    products
        .as_array()
        .map(|products| {
            products.iter().any(|product| {
                product
                    .get("has_default_path")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false)
            })
        })
        .unwrap_or(false)
}

fn count_broad_shares(portfolio_shares: &Value) -> usize {
    portfolio_shares
        .as_array()
        .map(|shares| {
            shares
                .iter()
                .filter(|share| {
                    matches!(
                        share
                            .get("type")
                            .and_then(|v| v.as_str())
                            .map(|value| value.to_ascii_uppercase())
                            .as_deref(),
                        Some("ACCOUNT") | Some("ORGANIZATION") | Some("ORGANIZATIONAL_UNIT")
                    )
                })
                .count()
        })
        .unwrap_or(0)
}

fn fmt_date(date: Option<&aws_smithy_types::DateTime>) -> Option<String> {
    date.map(|d| {
        d.fmt(Format::DateTime)
            .unwrap_or_else(|_| format!("{:?}", d))
    })
}

fn fallback_portfolio_arn(aws_account_dto: &AwsAccountDto, portfolio_id: &str) -> String {
    format!(
        "arn:aws:servicecatalog:{}:{}:portfolio/{}",
        aws_account_dto.default_region, aws_account_dto.account_id, portfolio_id
    )
}

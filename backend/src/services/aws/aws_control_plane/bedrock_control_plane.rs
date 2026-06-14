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
use aws_sdk_bedrock::types::{
    CustomModelSummary, FoundationModelSummary, GuardrailSummary, LoggingConfig,
    ModelCustomizationJobSummary, ModelInvocationJobSummary, ProvisionedModelSummary, Tag,
};
use aws_sdk_bedrock::Client as BedrockClient;
use aws_smithy_types::date_time::Format;
use serde_json::{json, Map, Value};
use std::collections::BTreeMap;
use std::sync::Arc;
use tracing::debug;
use uuid::Uuid;

pub struct BedrockControlPlane {
    aws_service: Arc<AwsService>,
}

#[derive(Debug, Default)]
struct CollectionSummary {
    resources_by_kind: BTreeMap<String, usize>,
    collection_errors: Vec<Value>,
    collection_error_count: usize,
    resource_count: usize,
    untagged_resource_count: usize,
    foundation_model_count: usize,
    legacy_foundation_model_count: usize,
    custom_model_count: usize,
    failed_custom_model_count: usize,
    provisioned_model_count: usize,
    failed_or_updating_provisioned_model_count: usize,
    provisioned_model_unit_count: i64,
    desired_model_unit_count: i64,
    provisioned_units_mismatch_count: usize,
    customization_job_count: usize,
    failed_or_incomplete_customization_job_count: usize,
    invocation_job_count: usize,
    failed_or_incomplete_invocation_job_count: usize,
    batch_job_without_vpc_count: usize,
    guardrail_count: usize,
    non_ready_guardrail_count: usize,
}

#[derive(Debug, Default)]
struct LoggingObservation {
    logging_configured: bool,
    logging_configuration_error: Option<String>,
    cloudwatch_logging_configured: bool,
    cloudwatch_log_group_name: Option<String>,
    cloudwatch_role_arn: Option<String>,
    s3_logging_configured: bool,
    s3_bucket_name: Option<String>,
    s3_key_prefix: Option<String>,
    logging_destination_count: usize,
    text_data_delivery_enabled: bool,
    image_data_delivery_enabled: bool,
    embedding_data_delivery_enabled: bool,
    video_data_delivery_enabled: bool,
    audio_data_delivery_enabled: bool,
    sensitive_logging_delivery_enabled: bool,
}

impl BedrockControlPlane {
    pub fn new(aws_service: Arc<AwsService>) -> Self {
        Self { aws_service }
    }

    pub async fn sync_resources(
        &self,
        aws_account_dto: &AwsAccountDto,
        sync_id: Uuid,
    ) -> Result<Vec<AwsResourceModel>, AppError> {
        debug!(
            "Syncing AWS Bedrock inventory for account: {} with sync_id: {}",
            &aws_account_dto.account_id, sync_id
        );

        let client = self
            .aws_service
            .create_bedrock_client(aws_account_dto)
            .await?;
        let mut resources = Vec::new();
        let mut summary = CollectionSummary::default();

        match list_foundation_models(&client).await {
            Ok(items) => {
                for item in items {
                    resources.push(foundation_model_resource(
                        aws_account_dto,
                        sync_id,
                        &item,
                        &mut summary,
                    ));
                }
            }
            Err(e) => {
                record_collection_error(&mut summary, "foundation_model", "ListFoundationModels", e)
            }
        }

        match list_custom_models(&client).await {
            Ok(items) => {
                for item in items {
                    let (tags, tag_error) = tags_for_resource(&client, item.model_arn()).await;
                    resources.push(custom_model_resource(
                        aws_account_dto,
                        sync_id,
                        &item,
                        tags,
                        tag_error,
                        &mut summary,
                    ));
                }
            }
            Err(e) => record_collection_error(&mut summary, "custom_model", "ListCustomModels", e),
        }

        match list_provisioned_models(&client).await {
            Ok(items) => {
                for item in items {
                    let (tags, tag_error) =
                        tags_for_resource(&client, item.provisioned_model_arn()).await;
                    resources.push(provisioned_model_resource(
                        aws_account_dto,
                        sync_id,
                        &item,
                        tags,
                        tag_error,
                        &mut summary,
                    ));
                }
            }
            Err(e) => record_collection_error(
                &mut summary,
                "provisioned_model",
                "ListProvisionedModelThroughputs",
                e,
            ),
        }

        match list_model_customization_jobs(&client).await {
            Ok(items) => {
                for item in items {
                    resources.push(customization_job_resource(
                        aws_account_dto,
                        sync_id,
                        &item,
                        &mut summary,
                    ));
                }
            }
            Err(e) => record_collection_error(
                &mut summary,
                "customization_job",
                "ListModelCustomizationJobs",
                e,
            ),
        }

        match list_model_invocation_jobs(&client).await {
            Ok(items) => {
                for item in items {
                    resources.push(invocation_job_resource(
                        aws_account_dto,
                        sync_id,
                        &item,
                        &mut summary,
                    ));
                }
            }
            Err(e) => record_collection_error(
                &mut summary,
                "invocation_job",
                "ListModelInvocationJobs",
                e,
            ),
        }

        match list_guardrails(&client).await {
            Ok(items) => {
                for item in items {
                    let (tags, tag_error) = tags_for_resource(&client, item.arn()).await;
                    resources.push(guardrail_resource(
                        aws_account_dto,
                        sync_id,
                        &item,
                        tags,
                        tag_error,
                        &mut summary,
                    ));
                }
            }
            Err(e) => record_collection_error(&mut summary, "guardrail", "ListGuardrails", e),
        }

        let logging = match get_logging_config(&client).await {
            Ok(config) => observe_logging(config, None),
            Err(e) => {
                record_collection_error(
                    &mut summary,
                    "model_invocation_logging_configuration",
                    "GetModelInvocationLoggingConfiguration",
                    e.clone(),
                );
                observe_logging(None, Some(e))
            }
        };

        let summary_resource =
            account_summary_resource(aws_account_dto, sync_id, &summary, &logging);
        resources.insert(0, summary_resource);

        debug!(
            "Successfully synced {} AWS Bedrock inventory resources for account: {} with sync_id: {}",
            resources.len(),
            &aws_account_dto.account_id,
            sync_id
        );

        Ok(resources)
    }
}

async fn list_foundation_models(
    client: &BedrockClient,
) -> Result<Vec<FoundationModelSummary>, String> {
    let response = client
        .list_foundation_models()
        .send()
        .await
        .map_err(|e| e.to_string())?;
    Ok(response.model_summaries().to_vec())
}

async fn list_custom_models(client: &BedrockClient) -> Result<Vec<CustomModelSummary>, String> {
    let mut resources = Vec::new();
    let mut next_token: Option<String> = None;

    loop {
        let mut request = client.list_custom_models().max_results(100);
        if let Some(token) = next_token.take() {
            request = request.next_token(token);
        }
        let response = request.send().await.map_err(|e| e.to_string())?;
        resources.extend(response.model_summaries().iter().cloned());
        next_token = response.next_token().map(String::from);
        if next_token.is_none() {
            break;
        }
    }

    Ok(resources)
}

async fn list_provisioned_models(
    client: &BedrockClient,
) -> Result<Vec<ProvisionedModelSummary>, String> {
    let mut resources = Vec::new();
    let mut next_token: Option<String> = None;

    loop {
        let mut request = client.list_provisioned_model_throughputs().max_results(100);
        if let Some(token) = next_token.take() {
            request = request.next_token(token);
        }
        let response = request.send().await.map_err(|e| e.to_string())?;
        resources.extend(response.provisioned_model_summaries().iter().cloned());
        next_token = response.next_token().map(String::from);
        if next_token.is_none() {
            break;
        }
    }

    Ok(resources)
}

async fn list_model_customization_jobs(
    client: &BedrockClient,
) -> Result<Vec<ModelCustomizationJobSummary>, String> {
    let mut resources = Vec::new();
    let mut next_token: Option<String> = None;

    loop {
        let mut request = client.list_model_customization_jobs().max_results(100);
        if let Some(token) = next_token.take() {
            request = request.next_token(token);
        }
        let response = request.send().await.map_err(|e| e.to_string())?;
        resources.extend(response.model_customization_job_summaries().iter().cloned());
        next_token = response.next_token().map(String::from);
        if next_token.is_none() {
            break;
        }
    }

    Ok(resources)
}

async fn list_model_invocation_jobs(
    client: &BedrockClient,
) -> Result<Vec<ModelInvocationJobSummary>, String> {
    let mut resources = Vec::new();
    let mut next_token: Option<String> = None;

    loop {
        let mut request = client.list_model_invocation_jobs().max_results(100);
        if let Some(token) = next_token.take() {
            request = request.next_token(token);
        }
        let response = request.send().await.map_err(|e| e.to_string())?;
        resources.extend(response.invocation_job_summaries().iter().cloned());
        next_token = response.next_token().map(String::from);
        if next_token.is_none() {
            break;
        }
    }

    Ok(resources)
}

async fn list_guardrails(client: &BedrockClient) -> Result<Vec<GuardrailSummary>, String> {
    let mut resources = Vec::new();
    let mut next_token: Option<String> = None;

    loop {
        let mut request = client.list_guardrails().max_results(100);
        if let Some(token) = next_token.take() {
            request = request.next_token(token);
        }
        let response = request.send().await.map_err(|e| e.to_string())?;
        resources.extend(response.guardrails().iter().cloned());
        next_token = response.next_token().map(String::from);
        if next_token.is_none() {
            break;
        }
    }

    Ok(resources)
}

async fn get_logging_config(client: &BedrockClient) -> Result<Option<LoggingConfig>, String> {
    let response = client
        .get_model_invocation_logging_configuration()
        .send()
        .await
        .map_err(|e| e.to_string())?;
    Ok(response.logging_config().cloned())
}

async fn tags_for_resource(client: &BedrockClient, resource_arn: &str) -> (Value, Option<String>) {
    match client
        .list_tags_for_resource()
        .resource_arn(resource_arn)
        .send()
        .await
    {
        Ok(response) => (tags_to_json(response.tags()), None),
        Err(e) => (json!({}), Some(e.to_string())),
    }
}

fn foundation_model_resource(
    aws_account_dto: &AwsAccountDto,
    sync_id: Uuid,
    item: &FoundationModelSummary,
    summary: &mut CollectionSummary,
) -> AwsResourceModel {
    summary.foundation_model_count += 1;
    let lifecycle_status = item
        .model_lifecycle()
        .map(|lifecycle| lifecycle.status().as_str().to_string());
    if lifecycle_status.as_deref() == Some("LEGACY") {
        summary.legacy_foundation_model_count += 1;
    }

    let mut data = base_resource_data("foundation_model");
    data.insert("model_id".to_string(), json!(item.model_id()));
    data.insert("model_name".to_string(), json!(item.model_name()));
    data.insert("provider_name".to_string(), json!(item.provider_name()));
    data.insert(
        "input_modalities".to_string(),
        json!(enum_values(item.input_modalities())),
    );
    data.insert(
        "output_modalities".to_string(),
        json!(enum_values(item.output_modalities())),
    );
    data.insert(
        "response_streaming_supported".to_string(),
        json!(item.response_streaming_supported()),
    );
    data.insert(
        "customizations_supported".to_string(),
        json!(enum_values(item.customizations_supported())),
    );
    data.insert(
        "inference_types_supported".to_string(),
        json!(enum_values(item.inference_types_supported())),
    );
    data.insert("lifecycle_status".to_string(), json!(lifecycle_status));
    data.insert(
        "legacy_time".to_string(),
        json!(item
            .model_lifecycle()
            .and_then(|lifecycle| fmt_date(lifecycle.legacy_time()))),
    );
    data.insert(
        "public_extended_access_time".to_string(),
        json!(item
            .model_lifecycle()
            .and_then(|lifecycle| fmt_date(lifecycle.public_extended_access_time()))),
    );
    data.insert(
        "end_of_life_time".to_string(),
        json!(item
            .model_lifecycle()
            .and_then(|lifecycle| fmt_date(lifecycle.end_of_life_time()))),
    );

    bedrock_resource(
        aws_account_dto,
        sync_id,
        "foundation_model",
        item.model_id(),
        Some(item.model_arn()),
        item.model_name().map(String::from),
        json!({}),
        data,
        None,
        summary,
    )
}

fn custom_model_resource(
    aws_account_dto: &AwsAccountDto,
    sync_id: Uuid,
    item: &CustomModelSummary,
    tags: Value,
    tag_error: Option<String>,
    summary: &mut CollectionSummary,
) -> AwsResourceModel {
    summary.custom_model_count += 1;
    let model_status = item
        .model_status()
        .map(|status| status.as_str().to_string());
    if model_status.as_deref() == Some("Failed") {
        summary.failed_custom_model_count += 1;
    }

    let mut data = base_resource_data("custom_model");
    data.insert("model_name".to_string(), json!(item.model_name()));
    data.insert(
        "creation_time".to_string(),
        json!(fmt_date(Some(item.creation_time()))),
    );
    data.insert("base_model_arn".to_string(), json!(item.base_model_arn()));
    data.insert("base_model_name".to_string(), json!(item.base_model_name()));
    data.insert(
        "customization_type".to_string(),
        json!(item
            .customization_type()
            .map(|customization_type| customization_type.as_str())),
    );
    data.insert(
        "owner_account_id".to_string(),
        json!(item.owner_account_id()),
    );
    data.insert("model_status".to_string(), json!(model_status));

    bedrock_resource(
        aws_account_dto,
        sync_id,
        "custom_model",
        item.model_name(),
        Some(item.model_arn()),
        Some(item.model_name().to_string()),
        tags,
        data,
        tag_error,
        summary,
    )
}

fn provisioned_model_resource(
    aws_account_dto: &AwsAccountDto,
    sync_id: Uuid,
    item: &ProvisionedModelSummary,
    tags: Value,
    tag_error: Option<String>,
    summary: &mut CollectionSummary,
) -> AwsResourceModel {
    summary.provisioned_model_count += 1;
    summary.provisioned_model_unit_count += item.model_units() as i64;
    summary.desired_model_unit_count += item.desired_model_units() as i64;
    if item.model_units() != item.desired_model_units() {
        summary.provisioned_units_mismatch_count += 1;
    }
    let status = item.status().as_str().to_string();
    if status != "InService" {
        summary.failed_or_updating_provisioned_model_count += 1;
    }

    let mut data = base_resource_data("provisioned_model");
    data.insert(
        "provisioned_model_name".to_string(),
        json!(item.provisioned_model_name()),
    );
    data.insert("model_arn".to_string(), json!(item.model_arn()));
    data.insert(
        "desired_model_arn".to_string(),
        json!(item.desired_model_arn()),
    );
    data.insert(
        "foundation_model_arn".to_string(),
        json!(item.foundation_model_arn()),
    );
    data.insert("model_units".to_string(), json!(item.model_units()));
    data.insert(
        "desired_model_units".to_string(),
        json!(item.desired_model_units()),
    );
    data.insert("status".to_string(), json!(status));
    data.insert(
        "commitment_duration".to_string(),
        json!(item
            .commitment_duration()
            .map(|commitment_duration| commitment_duration.as_str())),
    );
    data.insert(
        "commitment_expiration_time".to_string(),
        json!(fmt_date(item.commitment_expiration_time())),
    );
    data.insert(
        "creation_time".to_string(),
        json!(fmt_date(Some(item.creation_time()))),
    );
    data.insert(
        "last_modified_time".to_string(),
        json!(fmt_date(Some(item.last_modified_time()))),
    );

    bedrock_resource(
        aws_account_dto,
        sync_id,
        "provisioned_model",
        item.provisioned_model_name(),
        Some(item.provisioned_model_arn()),
        Some(item.provisioned_model_name().to_string()),
        tags,
        data,
        tag_error,
        summary,
    )
}

fn customization_job_resource(
    aws_account_dto: &AwsAccountDto,
    sync_id: Uuid,
    item: &ModelCustomizationJobSummary,
    summary: &mut CollectionSummary,
) -> AwsResourceModel {
    summary.customization_job_count += 1;
    let status = item.status().as_str().to_string();
    if is_incomplete_customization_status(&status) {
        summary.failed_or_incomplete_customization_job_count += 1;
    }

    let mut data = base_resource_data("customization_job");
    data.insert("job_name".to_string(), json!(item.job_name()));
    data.insert("base_model_arn".to_string(), json!(item.base_model_arn()));
    data.insert("status".to_string(), json!(status));
    data.insert(
        "status_details".to_string(),
        json!(item
            .status_details()
            .map(|status_details| format!("{:?}", status_details))),
    );
    data.insert(
        "last_modified_time".to_string(),
        json!(fmt_date(item.last_modified_time())),
    );
    data.insert(
        "creation_time".to_string(),
        json!(fmt_date(Some(item.creation_time()))),
    );
    data.insert("end_time".to_string(), json!(fmt_date(item.end_time())));
    data.insert(
        "custom_model_arn".to_string(),
        json!(item.custom_model_arn()),
    );
    data.insert(
        "custom_model_name".to_string(),
        json!(item.custom_model_name()),
    );
    data.insert(
        "customization_type".to_string(),
        json!(item
            .customization_type()
            .map(|customization_type| customization_type.as_str())),
    );

    bedrock_resource(
        aws_account_dto,
        sync_id,
        "customization_job",
        item.job_name(),
        Some(item.job_arn()),
        Some(item.job_name().to_string()),
        json!({}),
        data,
        None,
        summary,
    )
}

fn invocation_job_resource(
    aws_account_dto: &AwsAccountDto,
    sync_id: Uuid,
    item: &ModelInvocationJobSummary,
    summary: &mut CollectionSummary,
) -> AwsResourceModel {
    summary.invocation_job_count += 1;
    let status = item.status().map(|status| status.as_str().to_string());
    if status
        .as_deref()
        .map(is_incomplete_invocation_status)
        .unwrap_or(false)
    {
        summary.failed_or_incomplete_invocation_job_count += 1;
    }
    let vpc_configured = item.vpc_config().is_some();
    if !vpc_configured {
        summary.batch_job_without_vpc_count += 1;
    }

    let mut data = base_resource_data("invocation_job");
    data.insert("job_name".to_string(), json!(item.job_name()));
    data.insert("model_id".to_string(), json!(item.model_id()));
    data.insert("role_arn".to_string(), json!(item.role_arn()));
    data.insert("status".to_string(), json!(status));
    data.insert("message".to_string(), json!(item.message()));
    data.insert(
        "submit_time".to_string(),
        json!(fmt_date(Some(item.submit_time()))),
    );
    data.insert(
        "last_modified_time".to_string(),
        json!(fmt_date(item.last_modified_time())),
    );
    data.insert("end_time".to_string(), json!(fmt_date(item.end_time())));
    data.insert(
        "input_data_configured".to_string(),
        json!(item.input_data_config().is_some()),
    );
    data.insert(
        "output_data_configured".to_string(),
        json!(item.output_data_config().is_some()),
    );
    data.insert("vpc_configured".to_string(), json!(vpc_configured));
    data.insert(
        "timeout_duration_in_hours".to_string(),
        json!(item.timeout_duration_in_hours()),
    );
    data.insert(
        "job_expiration_time".to_string(),
        json!(fmt_date(item.job_expiration_time())),
    );
    data.insert(
        "model_invocation_type".to_string(),
        json!(item
            .model_invocation_type()
            .map(|invocation_type| invocation_type.as_str())),
    );

    bedrock_resource(
        aws_account_dto,
        sync_id,
        "invocation_job",
        item.job_name(),
        Some(item.job_arn()),
        Some(item.job_name().to_string()),
        json!({}),
        data,
        None,
        summary,
    )
}

fn guardrail_resource(
    aws_account_dto: &AwsAccountDto,
    sync_id: Uuid,
    item: &GuardrailSummary,
    tags: Value,
    tag_error: Option<String>,
    summary: &mut CollectionSummary,
) -> AwsResourceModel {
    summary.guardrail_count += 1;
    let status = item.status().as_str().to_string();
    if status != "READY" {
        summary.non_ready_guardrail_count += 1;
    }

    let mut data = base_resource_data("guardrail");
    data.insert("guardrail_id".to_string(), json!(item.id()));
    data.insert("name".to_string(), json!(item.name()));
    data.insert("description".to_string(), json!(item.description()));
    data.insert("status".to_string(), json!(status));
    data.insert("version".to_string(), json!(item.version()));
    data.insert(
        "created_at".to_string(),
        json!(fmt_date(Some(item.created_at()))),
    );
    data.insert(
        "updated_at".to_string(),
        json!(fmt_date(Some(item.updated_at()))),
    );
    data.insert(
        "cross_region_guardrail_configured".to_string(),
        json!(item.cross_region_details().is_some()),
    );

    bedrock_resource(
        aws_account_dto,
        sync_id,
        "guardrail",
        item.id(),
        Some(item.arn()),
        Some(item.name().to_string()),
        tags,
        data,
        tag_error,
        summary,
    )
}

fn account_summary_resource(
    aws_account_dto: &AwsAccountDto,
    sync_id: Uuid,
    summary: &CollectionSummary,
    logging: &LoggingObservation,
) -> AwsResourceModel {
    let dto = AwsResourceDto {
        id: None,
        sync_id: Some(sync_id),
        account_id: aws_account_dto.account_id.clone(),
        profile: aws_account_dto.profile.clone(),
        region: aws_account_dto.default_region.clone(),
        resource_type: AwsResourceType::BedrockResource.to_string(),
        resource_id: format!("bedrock:{}", aws_account_dto.account_id),
        arn: format!(
            "arn:aws:bedrock:{}:{}:account/{}",
            aws_account_dto.default_region, aws_account_dto.account_id, aws_account_dto.account_id
        ),
        name: Some("Bedrock account summary".to_string()),
        tags: json!({}),
        resource_data: json!({
            "resource_kind": "account_summary",
            "resource_count": summary.resource_count,
            "resources_by_kind": summary.resources_by_kind,
            "collection_error_count": summary.collection_error_count,
            "collection_errors": summary.collection_errors,
            "untagged_resource_count": summary.untagged_resource_count,
            "foundation_model_count": summary.foundation_model_count,
            "legacy_foundation_model_count": summary.legacy_foundation_model_count,
            "custom_model_count": summary.custom_model_count,
            "failed_custom_model_count": summary.failed_custom_model_count,
            "provisioned_model_count": summary.provisioned_model_count,
            "failed_or_updating_provisioned_model_count": summary.failed_or_updating_provisioned_model_count,
            "provisioned_model_unit_count": summary.provisioned_model_unit_count,
            "desired_model_unit_count": summary.desired_model_unit_count,
            "provisioned_units_mismatch_count": summary.provisioned_units_mismatch_count,
            "customization_job_count": summary.customization_job_count,
            "failed_or_incomplete_customization_job_count": summary.failed_or_incomplete_customization_job_count,
            "invocation_job_count": summary.invocation_job_count,
            "failed_or_incomplete_invocation_job_count": summary.failed_or_incomplete_invocation_job_count,
            "batch_job_without_vpc_count": summary.batch_job_without_vpc_count,
            "guardrail_count": summary.guardrail_count,
            "non_ready_guardrail_count": summary.non_ready_guardrail_count,
            "logging_configured": logging.logging_configured,
            "logging_configuration_error": logging.logging_configuration_error,
            "cloudwatch_logging_configured": logging.cloudwatch_logging_configured,
            "cloudwatch_log_group_name": logging.cloudwatch_log_group_name,
            "cloudwatch_role_arn": logging.cloudwatch_role_arn,
            "s3_logging_configured": logging.s3_logging_configured,
            "s3_bucket_name": logging.s3_bucket_name,
            "s3_key_prefix": logging.s3_key_prefix,
            "logging_destination_count": logging.logging_destination_count,
            "text_data_delivery_enabled": logging.text_data_delivery_enabled,
            "image_data_delivery_enabled": logging.image_data_delivery_enabled,
            "embedding_data_delivery_enabled": logging.embedding_data_delivery_enabled,
            "video_data_delivery_enabled": logging.video_data_delivery_enabled,
            "audio_data_delivery_enabled": logging.audio_data_delivery_enabled,
            "sensitive_logging_delivery_enabled": logging.sensitive_logging_delivery_enabled,
        }),
    };

    dto.into()
}

fn bedrock_resource(
    aws_account_dto: &AwsAccountDto,
    sync_id: Uuid,
    resource_kind: &str,
    resource_id: &str,
    arn: Option<&str>,
    name: Option<String>,
    tags: Value,
    mut resource_data: Map<String, Value>,
    tag_error: Option<String>,
    summary: &mut CollectionSummary,
) -> AwsResourceModel {
    summary.resource_count += 1;
    *summary
        .resources_by_kind
        .entry(resource_kind.to_string())
        .or_insert(0) += 1;
    if is_cost_taggable(resource_kind)
        && tags.as_object().map(|tags| tags.is_empty()).unwrap_or(true)
    {
        summary.untagged_resource_count += 1;
    }
    if let Some(error) = tag_error {
        resource_data.insert("tag_collection_error".to_string(), json!(error.clone()));
        record_collection_error(summary, resource_kind, "ListTagsForResource", error);
    }

    let dto = AwsResourceDto {
        id: None,
        sync_id: Some(sync_id),
        account_id: aws_account_dto.account_id.clone(),
        profile: aws_account_dto.profile.clone(),
        region: aws_account_dto.default_region.clone(),
        resource_type: AwsResourceType::BedrockResource.to_string(),
        resource_id: format!("{}/{}", resource_kind, resource_id),
        arn: arn.map(String::from).unwrap_or_else(|| {
            format!(
                "arn:aws:bedrock:{}:{}:{}/{}",
                aws_account_dto.default_region,
                aws_account_dto.account_id,
                resource_kind,
                resource_id
            )
        }),
        name,
        tags,
        resource_data: Value::Object(resource_data),
    };

    dto.into()
}

fn record_collection_error(
    summary: &mut CollectionSummary,
    resource_kind: &str,
    operation: &str,
    error: String,
) {
    debug!(
        "Failed to collect Bedrock {} inventory with {}: {}",
        resource_kind, operation, error
    );
    summary.collection_error_count += 1;
    summary.collection_errors.push(json!({
        "resource_kind": resource_kind,
        "operation": operation,
        "error": error,
    }));
}

fn base_resource_data(resource_kind: &str) -> Map<String, Value> {
    let mut data = Map::new();
    data.insert("resource_kind".to_string(), json!(resource_kind));
    data
}

fn tags_to_json(tags: &[Tag]) -> Value {
    let mut map = Map::new();
    for tag in tags {
        map.insert(tag.key().to_string(), json!(tag.value()));
    }
    Value::Object(map)
}

fn observe_logging(
    config: Option<LoggingConfig>,
    logging_configuration_error: Option<String>,
) -> LoggingObservation {
    let mut observation = LoggingObservation {
        logging_configuration_error,
        ..Default::default()
    };

    if let Some(config) = config {
        observation.logging_configured = true;
        observation.text_data_delivery_enabled =
            config.text_data_delivery_enabled().unwrap_or(false);
        observation.image_data_delivery_enabled =
            config.image_data_delivery_enabled().unwrap_or(false);
        observation.embedding_data_delivery_enabled =
            config.embedding_data_delivery_enabled().unwrap_or(false);
        observation.video_data_delivery_enabled =
            config.video_data_delivery_enabled().unwrap_or(false);
        observation.audio_data_delivery_enabled =
            config.audio_data_delivery_enabled().unwrap_or(false);
        observation.sensitive_logging_delivery_enabled = observation.text_data_delivery_enabled
            || observation.image_data_delivery_enabled
            || observation.embedding_data_delivery_enabled
            || observation.video_data_delivery_enabled
            || observation.audio_data_delivery_enabled;

        if let Some(cloudwatch) = config.cloud_watch_config() {
            observation.cloudwatch_logging_configured = true;
            observation.cloudwatch_log_group_name = Some(cloudwatch.log_group_name().to_string());
            observation.cloudwatch_role_arn = Some(cloudwatch.role_arn().to_string());
            observation.logging_destination_count += 1;
        }

        if let Some(s3) = config.s3_config() {
            observation.s3_logging_configured = true;
            observation.s3_bucket_name = Some(s3.bucket_name().to_string());
            observation.s3_key_prefix = s3.key_prefix().map(String::from);
            observation.logging_destination_count += 1;
        }
    }

    observation
}

fn enum_values<T: AsRef<str>>(values: &[T]) -> Vec<String> {
    values
        .iter()
        .map(|value| value.as_ref().to_string())
        .collect()
}

fn fmt_date(date: Option<&aws_smithy_types::DateTime>) -> Option<String> {
    date.and_then(|value| value.fmt(Format::DateTime).ok())
}

fn is_cost_taggable(resource_kind: &str) -> bool {
    matches!(
        resource_kind,
        "custom_model" | "provisioned_model" | "guardrail"
    )
}

fn is_incomplete_customization_status(status: &str) -> bool {
    matches!(status, "Failed" | "Stopped" | "Stopping")
}

fn is_incomplete_invocation_status(status: &str) -> bool {
    matches!(
        status,
        "Expired" | "Failed" | "PartiallyCompleted" | "Stopped" | "Stopping"
    )
}

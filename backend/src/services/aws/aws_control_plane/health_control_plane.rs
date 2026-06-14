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
use aws_sdk_health::types::{AffectedEntity, EntityFilter, Event};
use aws_smithy_types::date_time::Format;
use serde_json::{json, Value};
use std::sync::Arc;
use tracing::{debug, error};
use uuid::Uuid;

pub struct HealthControlPlane {
    aws_service: Arc<AwsService>,
}

#[derive(Debug, Default)]
struct EventAggregates {
    open_event_count: usize,
    upcoming_event_count: usize,
    issue_event_count: usize,
    scheduled_change_event_count: usize,
    account_notification_event_count: usize,
    cost_relevant_event_count: usize,
    security_relevant_event_count: usize,
}

#[derive(Debug, Default)]
struct AffectedEntitySamples {
    samples: Vec<Value>,
    affected_entity_count: usize,
    impaired_entity_count: usize,
    pending_entity_count: usize,
    collection_error_count: usize,
}

impl HealthControlPlane {
    pub fn new(aws_service: Arc<AwsService>) -> Self {
        Self { aws_service }
    }

    pub async fn sync_accounts(
        &self,
        aws_account_dto: &AwsAccountDto,
        sync_id: Uuid,
    ) -> Result<Vec<AwsResourceModel>, AppError> {
        debug!(
            "Syncing AWS Health account inventory for account: {} with sync_id: {}",
            &aws_account_dto.account_id, sync_id
        );

        let client = self
            .aws_service
            .create_health_client(aws_account_dto)
            .await?;
        let events = list_events(&client).await.map_err(|e| {
            error!("Failed to describe AWS Health events: {}", e);
            AppError::ExternalService(format!("Failed to describe AWS Health events: {}", e))
        })?;
        let affected_entities = collect_affected_entity_samples(&client, &events).await;
        let resource = account_resource(aws_account_dto, sync_id, events, affected_entities);

        debug!(
            "Successfully synced AWS Health account inventory for account: {} with sync_id: {}",
            &aws_account_dto.account_id, sync_id
        );

        Ok(vec![resource])
    }
}

async fn list_events(client: &aws_sdk_health::Client) -> Result<Vec<Event>, String> {
    let mut events = Vec::new();
    let mut next_token: Option<String> = None;

    loop {
        let mut request = client.describe_events().max_results(100);
        if let Some(token) = next_token {
            request = request.next_token(token);
        }

        let response = request.send().await.map_err(|e| e.to_string())?;
        events.extend(response.events().iter().cloned());
        next_token = response.next_token().map(String::from);
        if next_token.is_none() {
            break;
        }
    }

    Ok(events)
}

async fn collect_affected_entity_samples(
    client: &aws_sdk_health::Client,
    events: &[Event],
) -> AffectedEntitySamples {
    let mut summary = AffectedEntitySamples::default();
    let event_arns: Vec<String> = events
        .iter()
        .filter_map(|event| event.arn().map(String::from))
        .collect();

    for chunk in event_arns.chunks(10) {
        let filter = match EntityFilter::builder()
            .set_event_arns(Some(chunk.to_vec()))
            .build()
        {
            Ok(filter) => filter,
            Err(e) => {
                debug!("Failed to build AWS Health affected-entity filter: {}", e);
                summary.collection_error_count += 1;
                continue;
            }
        };
        let mut next_token: Option<String> = None;

        loop {
            let mut request = client
                .describe_affected_entities()
                .filter(filter.clone())
                .max_results(100);
            if let Some(token) = next_token {
                request = request.next_token(token);
            }

            let response = match request.send().await {
                Ok(response) => response,
                Err(e) => {
                    debug!("Failed to describe AWS Health affected entities: {}", e);
                    summary.collection_error_count += 1;
                    break;
                }
            };

            for entity in response.entities() {
                record_affected_entity(&mut summary, entity);
            }

            next_token = response.next_token().map(String::from);
            if next_token.is_none() {
                break;
            }
        }
    }

    summary
}

fn record_affected_entity(summary: &mut AffectedEntitySamples, entity: &AffectedEntity) {
    summary.affected_entity_count += 1;
    match entity.status_code().map(|status| status.as_str()) {
        Some("IMPAIRED") | Some("impaired") => summary.impaired_entity_count += 1,
        Some("PENDING") | Some("pending") => summary.pending_entity_count += 1,
        _ => {}
    }

    if summary.samples.len() < 100 {
        summary.samples.push(affected_entity_to_value(entity));
    }
}

fn account_resource(
    aws_account_dto: &AwsAccountDto,
    sync_id: Uuid,
    events: Vec<Event>,
    affected_entities: AffectedEntitySamples,
) -> AwsResourceModel {
    let aggregates = summarize_events(&events);
    let event_samples: Vec<Value> = events.iter().take(100).map(event_to_value).collect();
    let arn = fallback_account_arn(aws_account_dto);

    let mut resource_data = serde_json::Map::new();
    resource_data.insert("asset_kind".to_string(), json!("account"));
    resource_data.insert("account_id".to_string(), json!(&aws_account_dto.account_id));
    resource_data.insert("event_count".to_string(), json!(events.len()));
    resource_data.insert(
        "open_event_count".to_string(),
        json!(aggregates.open_event_count),
    );
    resource_data.insert(
        "upcoming_event_count".to_string(),
        json!(aggregates.upcoming_event_count),
    );
    resource_data.insert(
        "issue_event_count".to_string(),
        json!(aggregates.issue_event_count),
    );
    resource_data.insert(
        "scheduled_change_event_count".to_string(),
        json!(aggregates.scheduled_change_event_count),
    );
    resource_data.insert(
        "account_notification_event_count".to_string(),
        json!(aggregates.account_notification_event_count),
    );
    resource_data.insert(
        "cost_relevant_event_count".to_string(),
        json!(aggregates.cost_relevant_event_count),
    );
    resource_data.insert(
        "security_relevant_event_count".to_string(),
        json!(aggregates.security_relevant_event_count),
    );
    resource_data.insert(
        "affected_entity_count".to_string(),
        json!(affected_entities.affected_entity_count),
    );
    resource_data.insert(
        "impaired_entity_count".to_string(),
        json!(affected_entities.impaired_entity_count),
    );
    resource_data.insert(
        "pending_entity_count".to_string(),
        json!(affected_entities.pending_entity_count),
    );
    resource_data.insert(
        "affected_entity_collection_error_count".to_string(),
        json!(affected_entities.collection_error_count),
    );
    resource_data.insert("event_sample_count".to_string(), json!(event_samples.len()));
    resource_data.insert(
        "affected_entity_sample_count".to_string(),
        json!(affected_entities.samples.len()),
    );
    resource_data.insert("events".to_string(), Value::Array(event_samples));
    resource_data.insert(
        "affected_entities".to_string(),
        Value::Array(affected_entities.samples),
    );

    let dto = AwsResourceDto {
        id: None,
        sync_id: Some(sync_id),
        account_id: aws_account_dto.account_id.clone(),
        profile: aws_account_dto.profile.clone(),
        region: aws_account_dto.default_region.clone(),
        resource_type: AwsResourceType::HealthAccount.to_string(),
        resource_id: format!("health:{}", aws_account_dto.account_id),
        arn,
        name: Some("AWS Health".to_string()),
        tags: json!({}),
        resource_data: Value::Object(resource_data),
    };

    dto.into()
}

fn summarize_events(events: &[Event]) -> EventAggregates {
    let mut aggregates = EventAggregates::default();

    for event in events {
        let status = event.status_code().map(|status| status.as_str());
        let category = event
            .event_type_category()
            .map(|category| category.as_str())
            .unwrap_or_default();
        let service = event.service().unwrap_or_default();
        let event_type_code = event.event_type_code().unwrap_or_default();

        if status_eq(status, "open") {
            aggregates.open_event_count += 1;
        }
        if status_eq(status, "upcoming") {
            aggregates.upcoming_event_count += 1;
        }
        if value_eq(category, "issue") {
            aggregates.issue_event_count += 1;
        }
        if value_eq(category, "scheduledChange") {
            aggregates.scheduled_change_event_count += 1;
        }
        if value_eq(category, "accountNotification") {
            aggregates.account_notification_event_count += 1;
        }
        if is_cost_relevant_event(service, event_type_code) {
            aggregates.cost_relevant_event_count += 1;
        }
        if is_security_relevant_event(service, event_type_code, category) {
            aggregates.security_relevant_event_count += 1;
        }
    }

    aggregates
}

fn status_eq(status: Option<&str>, expected: &str) -> bool {
    status
        .map(|status| status.eq_ignore_ascii_case(expected))
        .unwrap_or(false)
}

fn value_eq(value: &str, expected: &str) -> bool {
    value.eq_ignore_ascii_case(expected)
}

fn is_cost_relevant_event(service: &str, event_type_code: &str) -> bool {
    let service = service.to_ascii_lowercase();
    let code = event_type_code.to_ascii_lowercase();
    matches!(
        service.as_str(),
        "ec2"
            | "ebs"
            | "rds"
            | "elasticloadbalancing"
            | "lambda"
            | "ecs"
            | "eks"
            | "dynamodb"
            | "s3"
            | "elasticache"
            | "opensearch"
    ) || code.contains("cost")
        || code.contains("billing")
        || code.contains("limit")
        || code.contains("capacity")
}

fn is_security_relevant_event(service: &str, event_type_code: &str, category: &str) -> bool {
    let service = service.to_ascii_lowercase();
    let code = event_type_code.to_ascii_lowercase();
    let category = category.to_ascii_lowercase();
    matches!(
        service.as_str(),
        "iam" | "acm" | "guardduty" | "inspector" | "macie" | "securityhub" | "shield" | "waf"
    ) || code.contains("security")
        || code.contains("abuse")
        || code.contains("certificate")
        || code.contains("credential")
        || category.contains("security")
}

fn event_to_value(event: &Event) -> Value {
    json!({
        "arn": event.arn(),
        "service": event.service(),
        "event_type_code": event.event_type_code(),
        "event_type_category": event.event_type_category().map(|category| category.as_str()),
        "region": event.region(),
        "availability_zone": event.availability_zone(),
        "status_code": event.status_code().map(|status| status.as_str()),
        "event_scope_code": event.event_scope_code().map(|scope| scope.as_str()),
        "start_time": fmt_date(event.start_time()),
        "end_time": fmt_date(event.end_time()),
        "last_updated_time": fmt_date(event.last_updated_time()),
    })
}

fn affected_entity_to_value(entity: &AffectedEntity) -> Value {
    json!({
        "entity_arn": entity.entity_arn(),
        "entity_value": entity.entity_value(),
        "event_arn": entity.event_arn(),
        "aws_account_id": entity.aws_account_id(),
        "status_code": entity.status_code().map(|status| status.as_str()),
        "last_updated_time": fmt_date(entity.last_updated_time()),
    })
}

fn fmt_date(date: Option<&aws_smithy_types::DateTime>) -> Option<String> {
    date.and_then(|date| date.fmt(Format::DateTime).ok())
}

fn fallback_account_arn(aws_account_dto: &AwsAccountDto) -> String {
    format!(
        "arn:aws:health:{}:{}:account/{}",
        aws_account_dto.default_region, aws_account_dto.account_id, aws_account_dto.account_id
    )
}

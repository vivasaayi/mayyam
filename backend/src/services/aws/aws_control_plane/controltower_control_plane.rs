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
use aws_sdk_controltower::types::{
    DriftStatusSummary, EnabledControlDetails, EnabledControlSummary, EnablementStatusSummary,
    LandingZoneDetail,
};
use aws_sdk_organizations::types::OrganizationalUnit;
use aws_smithy_types::{Document, Number};
use serde_json::{json, Map, Value};
use std::collections::HashSet;
use std::sync::Arc;
use tracing::{debug, error};
use uuid::Uuid;

#[derive(Debug, Clone)]
struct OrganizationalUnitEvidence {
    id: String,
    arn: String,
    name: Option<String>,
}

pub struct ControlTowerControlPlane {
    aws_service: Arc<AwsService>,
}

impl ControlTowerControlPlane {
    pub fn new(aws_service: Arc<AwsService>) -> Self {
        Self { aws_service }
    }

    pub async fn sync_landing_zones(
        &self,
        aws_account_dto: &AwsAccountDto,
        sync_id: Uuid,
    ) -> Result<Vec<AwsResourceModel>, AppError> {
        debug!(
            "Syncing AWS Control Tower landing zones for account: {} with sync_id: {}",
            &aws_account_dto.account_id, sync_id
        );

        let client = self
            .aws_service
            .create_controltower_client(aws_account_dto)
            .await?;
        let landing_zones = list_landing_zones(&client).await.map_err(|e| {
            error!("Failed to list AWS Control Tower landing zones: {}", e);
            AppError::ExternalService(format!(
                "Failed to list AWS Control Tower landing zones: {}",
                e
            ))
        })?;

        let organizational_units = match self
            .aws_service
            .create_organizations_client(aws_account_dto)
            .await
        {
            Ok(organizations_client) => collect_organizational_units(&organizations_client).await,
            Err(e) => {
                debug!(
                    "Failed to create AWS Organizations client for Control Tower OU evidence: {}",
                    e
                );
                Vec::new()
            }
        };
        let enabled_controls =
            collect_enabled_controls_for_ous(&client, &organizational_units).await;

        if landing_zones.is_empty() {
            return Ok(vec![account_level_resource(
                aws_account_dto,
                sync_id,
                organizational_units,
                enabled_controls,
            )]);
        }

        let mut resources = Vec::new();
        for summary in landing_zones {
            let landing_zone_arn = match summary.arn() {
                Some(arn) => arn.to_string(),
                None => fallback_landing_zone_arn(aws_account_dto, "unknown"),
            };
            let detail = get_landing_zone_detail(&client, &landing_zone_arn).await;
            let tags = list_tags(&client, &landing_zone_arn).await;
            resources.push(landing_zone_resource(
                aws_account_dto,
                sync_id,
                &landing_zone_arn,
                detail.as_ref(),
                tags,
                &organizational_units,
                &enabled_controls,
            ));
        }

        debug!(
            "Successfully synced {} AWS Control Tower landing zone resources for account: {} with sync_id: {}",
            resources.len(),
            &aws_account_dto.account_id,
            sync_id
        );

        Ok(resources)
    }
}

async fn list_landing_zones(
    client: &aws_sdk_controltower::Client,
) -> Result<Vec<aws_sdk_controltower::types::LandingZoneSummary>, String> {
    let mut landing_zones = Vec::new();
    let mut next_token: Option<String> = None;

    loop {
        let mut request = client.list_landing_zones();
        if let Some(token) = next_token {
            request = request.next_token(token);
        }

        let response = request.send().await.map_err(|e| e.to_string())?;
        landing_zones.extend(response.landing_zones().iter().cloned());
        next_token = response.next_token().map(String::from);
        if next_token.is_none() {
            break;
        }
    }

    Ok(landing_zones)
}

async fn get_landing_zone_detail(
    client: &aws_sdk_controltower::Client,
    landing_zone_arn: &str,
) -> Option<LandingZoneDetail> {
    match client
        .get_landing_zone()
        .landing_zone_identifier(landing_zone_arn)
        .send()
        .await
    {
        Ok(response) => response.landing_zone().cloned(),
        Err(e) => {
            debug!(
                "Failed to get AWS Control Tower landing zone detail for {}: {}",
                landing_zone_arn, e
            );
            None
        }
    }
}

async fn collect_organizational_units(
    client: &aws_sdk_organizations::Client,
) -> Vec<OrganizationalUnitEvidence> {
    let mut organizational_units = Vec::new();
    let mut parent_ids = Vec::new();
    let mut next_token: Option<String> = None;

    loop {
        let mut request = client.list_roots();
        if let Some(token) = next_token {
            request = request.next_token(token);
        }

        let response = match request.send().await {
            Ok(response) => response,
            Err(e) => {
                debug!(
                    "Failed to list AWS Organizations roots for Control Tower OU evidence: {}",
                    e
                );
                return organizational_units;
            }
        };

        parent_ids.extend(
            response
                .roots()
                .iter()
                .filter_map(|root| root.id().map(String::from)),
        );
        next_token = response.next_token().map(String::from);
        if next_token.is_none() {
            break;
        }
    }

    while let Some(parent_id) = parent_ids.pop() {
        let mut child_next_token: Option<String> = None;
        loop {
            let mut request = client
                .list_organizational_units_for_parent()
                .parent_id(parent_id.clone());
            if let Some(token) = child_next_token {
                request = request.next_token(token);
            }

            let response = match request.send().await {
                Ok(response) => response,
                Err(e) => {
                    debug!(
                        "Failed to list AWS Organizations child OUs for parent {}: {}",
                        parent_id, e
                    );
                    break;
                }
            };

            for ou in response.organizational_units() {
                if let Some(evidence) = organizational_unit_to_evidence(ou) {
                    parent_ids.push(evidence.id.clone());
                    organizational_units.push(evidence);
                }
            }

            child_next_token = response.next_token().map(String::from);
            if child_next_token.is_none() {
                break;
            }
        }
    }

    organizational_units
}

fn organizational_unit_to_evidence(ou: &OrganizationalUnit) -> Option<OrganizationalUnitEvidence> {
    Some(OrganizationalUnitEvidence {
        id: ou.id()?.to_string(),
        arn: ou.arn()?.to_string(),
        name: ou.name().map(String::from),
    })
}

async fn collect_enabled_controls_for_ous(
    client: &aws_sdk_controltower::Client,
    organizational_units: &[OrganizationalUnitEvidence],
) -> Vec<Value> {
    let mut controls = Vec::new();
    let mut seen = HashSet::new();

    for ou in organizational_units {
        let summaries = list_enabled_controls(client, &ou.arn).await;
        for summary in summaries {
            let key = format!(
                "{}:{}",
                summary
                    .arn()
                    .or_else(|| summary.control_identifier())
                    .unwrap_or("unknown"),
                summary.target_identifier().unwrap_or(ou.arn.as_str())
            );
            if !seen.insert(key) {
                continue;
            }
            let detail = match summary.arn() {
                Some(arn) => get_enabled_control_detail(client, arn).await,
                None => None,
            };
            controls.push(enabled_control_to_json(&summary, detail.as_ref()));
        }
    }

    controls
}

async fn list_enabled_controls(
    client: &aws_sdk_controltower::Client,
    target_identifier: &str,
) -> Vec<EnabledControlSummary> {
    let mut controls = Vec::new();
    let mut next_token: Option<String> = None;

    loop {
        let mut request = client
            .list_enabled_controls()
            .target_identifier(target_identifier)
            .include_children(false);
        if let Some(token) = next_token {
            request = request.next_token(token);
        }

        let response = match request.send().await {
            Ok(response) => response,
            Err(e) => {
                debug!(
                    "Failed to list AWS Control Tower enabled controls for target {}: {}",
                    target_identifier, e
                );
                return controls;
            }
        };

        controls.extend(response.enabled_controls().iter().cloned());
        next_token = response.next_token().map(String::from);
        if next_token.is_none() {
            break;
        }
    }

    controls
}

async fn get_enabled_control_detail(
    client: &aws_sdk_controltower::Client,
    enabled_control_arn: &str,
) -> Option<EnabledControlDetails> {
    match client
        .get_enabled_control()
        .enabled_control_identifier(enabled_control_arn)
        .send()
        .await
    {
        Ok(response) => response.enabled_control_details().cloned(),
        Err(e) => {
            debug!(
                "Failed to get AWS Control Tower enabled control detail for {}: {}",
                enabled_control_arn, e
            );
            None
        }
    }
}

async fn list_tags(client: &aws_sdk_controltower::Client, arn: &str) -> Value {
    match client
        .list_tags_for_resource()
        .resource_arn(arn)
        .send()
        .await
    {
        Ok(response) => json!(response.tags()),
        Err(e) => {
            debug!(
                "Failed to list AWS Control Tower tags for resource {}: {}",
                arn, e
            );
            json!({})
        }
    }
}

fn account_level_resource(
    aws_account_dto: &AwsAccountDto,
    sync_id: Uuid,
    organizational_units: Vec<OrganizationalUnitEvidence>,
    enabled_controls: Vec<Value>,
) -> AwsResourceModel {
    let fallback_arn = fallback_landing_zone_arn(aws_account_dto, "none");
    let resource_data = resource_data(
        0,
        None,
        &organizational_units,
        &enabled_controls,
        Value::Null,
    );

    AwsResourceDto {
        id: None,
        sync_id: Some(sync_id),
        account_id: aws_account_dto.account_id.clone(),
        profile: aws_account_dto.profile.clone(),
        region: aws_account_dto.default_region.clone(),
        resource_type: AwsResourceType::ControlTowerLandingZone.to_string(),
        resource_id: format!("controltower:{}", aws_account_dto.account_id),
        arn: fallback_arn,
        name: Some("controltower".to_string()),
        tags: json!({}),
        resource_data,
    }
    .into()
}

fn landing_zone_resource(
    aws_account_dto: &AwsAccountDto,
    sync_id: Uuid,
    landing_zone_arn: &str,
    detail: Option<&LandingZoneDetail>,
    tags: Value,
    organizational_units: &[OrganizationalUnitEvidence],
    enabled_controls: &[Value],
) -> AwsResourceModel {
    let arn = detail
        .and_then(|landing_zone| landing_zone.arn())
        .unwrap_or(landing_zone_arn)
        .to_string();
    let resource_data = resource_data(
        1,
        detail,
        organizational_units,
        enabled_controls,
        landing_zone_detail_to_json(detail),
    );

    AwsResourceDto {
        id: None,
        sync_id: Some(sync_id),
        account_id: aws_account_dto.account_id.clone(),
        profile: aws_account_dto.profile.clone(),
        region: aws_account_dto.default_region.clone(),
        resource_type: AwsResourceType::ControlTowerLandingZone.to_string(),
        resource_id: format!("controltower:{}", arn),
        arn,
        name: name_from_arn(landing_zone_arn),
        tags,
        resource_data,
    }
    .into()
}

fn resource_data(
    landing_zone_count: usize,
    detail: Option<&LandingZoneDetail>,
    organizational_units: &[OrganizationalUnitEvidence],
    enabled_controls: &[Value],
    landing_zone_detail: Value,
) -> Value {
    let managed_ou_targets: HashSet<String> = enabled_controls
        .iter()
        .filter_map(|control| {
            control
                .get("target_identifier")
                .and_then(|target| target.as_str())
                .map(String::from)
        })
        .collect();
    let organizational_unit_values = organizational_units
        .iter()
        .map(|ou| organization_unit_to_json(ou, managed_ou_targets.contains(&ou.arn)))
        .collect::<Vec<_>>();
    let registered_ou_count = organizational_units
        .iter()
        .filter(|ou| managed_ou_targets.contains(&ou.arn))
        .count();
    let failed_control_count = enabled_controls
        .iter()
        .filter(|control| is_failed_control(control))
        .count();

    json!({
        "landing_zone_count": landing_zone_count,
        "landing_zone_arn": detail.and_then(|landing_zone| landing_zone.arn()),
        "landing_zone_status": detail
            .and_then(|landing_zone| landing_zone.status())
            .map(|status| status.as_str()),
        "landing_zone_drift_status": detail
            .and_then(|landing_zone| landing_zone.drift_status())
            .and_then(|drift| drift.status())
            .map(|status| status.as_str()),
        "landing_zone_version": detail.map(|landing_zone| landing_zone.version()),
        "landing_zone_latest_available_version": detail
            .and_then(|landing_zone| landing_zone.latest_available_version()),
        "landing_zone_detail": landing_zone_detail,
        "organizational_unit_count": organizational_units.len(),
        "registered_ou_count": registered_ou_count,
        "unmanaged_ou_count": organizational_units.len().saturating_sub(registered_ou_count),
        "organizational_units": organizational_unit_values,
        "enabled_control_count": enabled_controls.len(),
        "failed_control_count": failed_control_count,
        "enabled_controls": enabled_controls,
    })
}

fn organization_unit_to_json(
    ou: &OrganizationalUnitEvidence,
    managed_by_control_tower: bool,
) -> Value {
    json!({
        "id": ou.id,
        "arn": ou.arn,
        "name": ou.name,
        "managed_by_control_tower": managed_by_control_tower,
    })
}

fn landing_zone_detail_to_json(detail: Option<&LandingZoneDetail>) -> Value {
    match detail {
        Some(landing_zone) => json!({
            "arn": landing_zone.arn(),
            "status": landing_zone.status().map(|status| status.as_str()),
            "version": landing_zone.version(),
            "latest_available_version": landing_zone.latest_available_version(),
            "drift_status": landing_zone
                .drift_status()
                .and_then(|drift| drift.status())
                .map(|status| status.as_str()),
            "remediation_types": landing_zone
                .remediation_types()
                .iter()
                .map(|remediation| remediation.as_str())
                .collect::<Vec<_>>(),
            "manifest": document_to_json(landing_zone.manifest()),
        }),
        None => Value::Null,
    }
}

fn enabled_control_to_json(
    summary: &EnabledControlSummary,
    detail: Option<&EnabledControlDetails>,
) -> Value {
    let status_summary = detail
        .and_then(|control| control.status_summary())
        .or_else(|| summary.status_summary());
    let drift_summary = detail
        .and_then(|control| control.drift_status_summary())
        .or_else(|| summary.drift_status_summary());

    json!({
        "arn": detail
            .and_then(|control| control.arn())
            .or_else(|| summary.arn()),
        "control_identifier": detail
            .and_then(|control| control.control_identifier())
            .or_else(|| summary.control_identifier()),
        "target_identifier": detail
            .and_then(|control| control.target_identifier())
            .or_else(|| summary.target_identifier()),
        "status": status_summary
            .and_then(|status| status.status())
            .map(|status| status.as_str()),
        "status_summary": enablement_status_summary_to_json(status_summary),
        "drift_status": drift_summary
            .and_then(|drift| drift.drift_status())
            .map(|status| status.as_str()),
        "drift_status_summary": drift_status_summary_to_json(drift_summary),
        "parent_identifier": detail
            .and_then(|control| control.parent_identifier())
            .or_else(|| summary.parent_identifier()),
    })
}

fn enablement_status_summary_to_json(status_summary: Option<&EnablementStatusSummary>) -> Value {
    match status_summary {
        Some(status_summary) => json!({
            "status": status_summary.status().map(|status| status.as_str()),
            "last_operation_identifier": status_summary.last_operation_identifier(),
        }),
        None => Value::Null,
    }
}

fn drift_status_summary_to_json(drift_summary: Option<&DriftStatusSummary>) -> Value {
    match drift_summary {
        Some(drift_summary) => json!({
            "drift_status": drift_summary
                .drift_status()
                .map(|status| status.as_str()),
        }),
        None => Value::Null,
    }
}

fn is_failed_control(control: &Value) -> bool {
    matches!(
        control.get("status").and_then(|status| status.as_str()),
        Some("FAILED")
    ) || matches!(
        control
            .get("drift_status")
            .and_then(|status| status.as_str()),
        Some("DRIFTED")
    )
}

fn document_to_json(document: &Document) -> Value {
    match document {
        Document::Object(object) => {
            let mut value = Map::new();
            for (key, entry) in object {
                value.insert(key.clone(), document_to_json(entry));
            }
            Value::Object(value)
        }
        Document::Array(entries) => {
            Value::Array(entries.iter().map(document_to_json).collect::<Vec<_>>())
        }
        Document::Number(number) => number_to_json(*number),
        Document::String(value) => Value::String(value.clone()),
        Document::Bool(value) => Value::Bool(*value),
        Document::Null => Value::Null,
    }
}

fn number_to_json(number: Number) -> Value {
    match number {
        Number::PosInt(value) => json!(value),
        Number::NegInt(value) => json!(value),
        Number::Float(value) => serde_json::Number::from_f64(value)
            .map(Value::Number)
            .unwrap_or(Value::Null),
    }
}

fn fallback_landing_zone_arn(aws_account_dto: &AwsAccountDto, suffix: &str) -> String {
    format!(
        "arn:aws:controltower:{}:{}:landingzone/{}",
        aws_account_dto.default_region, aws_account_dto.account_id, suffix
    )
}

fn name_from_arn(arn: &str) -> Option<String> {
    arn.rsplit('/')
        .next()
        .filter(|value| !value.is_empty())
        .map(String::from)
}

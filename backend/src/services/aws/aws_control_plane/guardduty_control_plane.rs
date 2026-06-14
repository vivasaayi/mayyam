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
use serde_json::json;
use std::sync::Arc;
use tracing::{debug, error};
use uuid::Uuid;

pub struct GuardDutyControlPlane {
    aws_service: Arc<AwsService>,
}

impl GuardDutyControlPlane {
    pub fn new(aws_service: Arc<AwsService>) -> Self {
        Self { aws_service }
    }

    pub async fn sync_detectors(
        &self,
        aws_account_dto: &AwsAccountDto,
        sync_id: Uuid,
    ) -> Result<Vec<AwsResourceModel>, AppError> {
        debug!(
            "Syncing GuardDuty detectors for account: {} with sync_id: {}",
            &aws_account_dto.account_id, sync_id
        );

        let client = self
            .aws_service
            .create_guardduty_client(aws_account_dto)
            .await?;
        let mut resources: Vec<AwsResourceModel> = Vec::new();
        let mut next_token: Option<String> = None;

        let mut detector_ids: Vec<String> = Vec::new();
        loop {
            let mut request = client.list_detectors();
            if let Some(token) = next_token {
                request = request.next_token(token);
            }

            let response = request.send().await.map_err(|e| {
                error!("Failed to list GuardDuty detectors: {}", e);
                AppError::ExternalService(format!("Failed to list GuardDuty detectors: {}", e))
            })?;

            detector_ids.extend(response.detector_ids().iter().map(String::from));
            next_token = response.next_token().map(String::from);
            if next_token.is_none() {
                break;
            }
        }

        for detector_id in &detector_ids {
            let detail = client
                .get_detector()
                .detector_id(detector_id)
                .send()
                .await
                .map_err(|e| {
                    error!("Failed to get GuardDuty detector {}: {}", detector_id, e);
                    AppError::ExternalService(format!(
                        "Failed to get GuardDuty detector {}: {}",
                        detector_id, e
                    ))
                })?;

            let mut resource_data = serde_json::Map::new();

            if let Some(status) = detail.status() {
                resource_data.insert("status".to_string(), json!(status.as_str()));
            }
            if let Some(created) = detail.created_at() {
                resource_data.insert("created_at".to_string(), json!(created));
            }
            if let Some(updated) = detail.updated_at() {
                resource_data.insert("updated_at".to_string(), json!(updated));
            }
            if let Some(svc_role) = detail.service_role() {
                resource_data.insert("service_role".to_string(), json!(svc_role));
            }
            if let Some(freq) = detail.finding_publishing_frequency() {
                resource_data.insert(
                    "finding_publishing_frequency".to_string(),
                    json!(freq.as_str()),
                );
            }

            // Data sources: each returns Option<&XxxConfigurationResult> with status()
            if let Some(ds) = detail.data_sources() {
                let s3_enabled = ds
                    .s3_logs()
                    .and_then(|s| s.status())
                    .map(|st| st.as_str() == "ENABLED")
                    .unwrap_or(false);
                resource_data.insert("s3_logs_enabled".to_string(), json!(s3_enabled));

                let ct_enabled = ds
                    .cloud_trail()
                    .and_then(|c| c.status())
                    .map(|st| st.as_str() == "ENABLED")
                    .unwrap_or(false);
                resource_data.insert("cloudtrail_enabled".to_string(), json!(ct_enabled));

                let dns_enabled = ds
                    .dns_logs()
                    .and_then(|d| d.status())
                    .map(|st| st.as_str() == "ENABLED")
                    .unwrap_or(false);
                resource_data.insert("dns_logs_enabled".to_string(), json!(dns_enabled));

                let flow_enabled = ds
                    .flow_logs()
                    .and_then(|f| f.status())
                    .map(|st| st.as_str() == "ENABLED")
                    .unwrap_or(false);
                resource_data.insert("flow_logs_enabled".to_string(), json!(flow_enabled));

                let k8s_enabled = ds
                    .kubernetes()
                    .and_then(|k| k.audit_logs())
                    .and_then(|a| a.status())
                    .map(|st| st.as_str() == "ENABLED")
                    .unwrap_or(false);
                resource_data.insert(
                    "kubernetes_audit_logs_enabled".to_string(),
                    json!(k8s_enabled),
                );
            }

            // Features (newer API)
            let mut features_list: Vec<serde_json::Value> = Vec::new();
            for feature in detail.features() {
                if let (Some(name), Some(status)) = (feature.name(), feature.status()) {
                    features_list.push(json!({
                        "name": name.as_str(),
                        "status": status.as_str(),
                    }));
                }
            }
            if !features_list.is_empty() {
                resource_data.insert("features".to_string(), json!(features_list));
            }

            let mut tags_map = serde_json::Map::new();
            if let Some(tags) = detail.tags() {
                for (k, v) in tags {
                    tags_map.insert(k.clone(), json!(v));
                }
            }

            let arn = format!(
                "arn:aws:guardduty:{}:{}:detector/{}",
                aws_account_dto.default_region, aws_account_dto.account_id, detector_id
            );

            let dto = AwsResourceDto {
                id: None,
                sync_id: Some(sync_id),
                account_id: aws_account_dto.account_id.clone(),
                profile: aws_account_dto.profile.clone(),
                region: aws_account_dto.default_region.clone(),
                resource_type: AwsResourceType::GuardDutyDetector.to_string(),
                resource_id: detector_id.clone(),
                arn,
                name: Some(detector_id.clone()),
                tags: serde_json::Value::Object(tags_map),
                resource_data: serde_json::Value::Object(resource_data),
            };
            resources.push(dto.into());
        }

        debug!(
            "Successfully synced {} GuardDuty detectors for account: {} with sync_id: {}",
            resources.len(),
            &aws_account_dto.account_id,
            sync_id
        );

        Ok(resources)
    }
}

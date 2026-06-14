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

pub struct AcmControlPlane {
    aws_service: Arc<AwsService>,
}

impl AcmControlPlane {
    pub fn new(aws_service: Arc<AwsService>) -> Self {
        Self { aws_service }
    }

    pub async fn sync_certificates(
        &self,
        aws_account_dto: &AwsAccountDto,
        sync_id: Uuid,
    ) -> Result<Vec<AwsResourceModel>, AppError> {
        debug!(
            "Syncing ACM certificates for account: {} with sync_id: {}",
            &aws_account_dto.account_id, sync_id
        );

        let client = self.aws_service.create_acm_client(aws_account_dto).await?;
        let mut resources: Vec<AwsResourceModel> = Vec::new();

        let mut next_token: Option<String> = None;

        loop {
            let mut request = client.list_certificates();
            if let Some(token) = next_token {
                request = request.next_token(token);
            }

            let response = request.send().await.map_err(|e| {
                error!("Failed to list ACM certificates: {}", e);
                AppError::ExternalService(format!("Failed to list ACM certificates: {}", e))
            })?;

            for summary in response.certificate_summary_list() {
                let certificate_arn = match summary.certificate_arn() {
                    Some(arn) if !arn.is_empty() => arn.to_string(),
                    _ => {
                        debug!("Skipping ACM certificate summary without an ARN");
                        continue;
                    }
                };

                // Per-certificate describe; log and continue on failure so one
                // bad certificate does not abort the whole sync.
                let describe_response = match client
                    .describe_certificate()
                    .certificate_arn(&certificate_arn)
                    .send()
                    .await
                {
                    Ok(res) => res,
                    Err(e) => {
                        error!(
                            "Failed to describe ACM certificate {}: {}",
                            certificate_arn, e
                        );
                        continue;
                    }
                };

                let detail = match describe_response.certificate() {
                    Some(detail) => detail,
                    None => {
                        debug!(
                            "Describe returned no detail for ACM certificate {}",
                            certificate_arn
                        );
                        continue;
                    }
                };

                let domain_name = detail.domain_name().map(|d| d.to_string());

                let mut resource_data = serde_json::Map::new();
                resource_data.insert("certificate_arn".to_string(), json!(certificate_arn));

                if let Some(domain) = &domain_name {
                    resource_data.insert("domain_name".to_string(), json!(domain));
                }

                if let Some(status) = detail.status() {
                    resource_data.insert("status".to_string(), json!(status.as_str()));
                }

                if let Some(cert_type) = detail.r#type() {
                    resource_data.insert("certificate_type".to_string(), json!(cert_type.as_str()));
                }

                let in_use_by: Vec<String> =
                    detail.in_use_by().iter().map(|s| s.to_string()).collect();
                resource_data.insert("in_use_by".to_string(), json!(in_use_by));

                if let Some(not_after) = detail.not_after() {
                    resource_data.insert(
                        "not_after".to_string(),
                        if let Ok(formatted) =
                            not_after.fmt(aws_smithy_types::date_time::Format::DateTime)
                        {
                            json!(formatted)
                        } else {
                            json!(format!("{:?}", not_after))
                        },
                    );
                }

                if let Some(renewal_eligibility) = detail.renewal_eligibility() {
                    resource_data.insert(
                        "renewal_eligibility".to_string(),
                        json!(renewal_eligibility.as_str()),
                    );
                }

                if let Some(key_algorithm) = detail.key_algorithm() {
                    resource_data
                        .insert("key_algorithm".to_string(), json!(key_algorithm.as_str()));
                }

                let subject_alternative_names: Vec<String> = detail
                    .subject_alternative_names()
                    .iter()
                    .map(|s| s.to_string())
                    .collect();
                resource_data.insert(
                    "subject_alternative_names".to_string(),
                    json!(subject_alternative_names),
                );

                // Real tags via ListTagsForCertificate; fall back to an empty
                // object on failure so the resource is still persisted.
                let tags = match client
                    .list_tags_for_certificate()
                    .certificate_arn(&certificate_arn)
                    .send()
                    .await
                {
                    Ok(tags_response) => {
                        let mut tags_map = serde_json::Map::new();
                        for tag in tags_response.tags() {
                            tags_map
                                .insert(tag.key().to_string(), json!(tag.value().unwrap_or("")));
                        }
                        serde_json::Value::Object(tags_map)
                    }
                    Err(e) => {
                        debug!(
                            "Failed to list tags for ACM certificate {}: {}",
                            certificate_arn, e
                        );
                        json!({})
                    }
                };

                // Certificate ARNs end in /<uuid>; use that suffix as the
                // resource ID, falling back to the full ARN.
                let resource_id = certificate_arn
                    .rsplit('/')
                    .next()
                    .filter(|s| !s.is_empty())
                    .unwrap_or(certificate_arn.as_str())
                    .to_string();

                let dto = AwsResourceDto {
                    id: None,
                    sync_id: Some(sync_id),
                    account_id: aws_account_dto.account_id.clone(),
                    profile: aws_account_dto.profile.clone(),
                    region: aws_account_dto.default_region.clone(),
                    resource_type: AwsResourceType::AcmCertificate.to_string(),
                    resource_id,
                    arn: certificate_arn.clone(),
                    name: domain_name,
                    tags,
                    resource_data: serde_json::Value::Object(resource_data),
                };

                resources.push(dto.into());
            }

            next_token = response.next_token().map(String::from);
            if next_token.is_none() {
                break;
            }
        }

        debug!(
            "Successfully synced {} ACM certificates for account: {} with sync_id: {}",
            resources.len(),
            &aws_account_dto.account_id,
            sync_id
        );

        Ok(resources)
    }
}

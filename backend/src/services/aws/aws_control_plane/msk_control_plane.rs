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

pub struct MskControlPlane {
    aws_service: Arc<AwsService>,
}

impl MskControlPlane {
    pub fn new(aws_service: Arc<AwsService>) -> Self {
        Self { aws_service }
    }

    pub async fn sync_clusters(
        &self,
        aws_account_dto: &AwsAccountDto,
        sync_id: Uuid,
    ) -> Result<Vec<AwsResourceModel>, AppError> {
        debug!(
            "Syncing MSK clusters for account: {} with sync_id: {}",
            &aws_account_dto.account_id, sync_id
        );

        let client = self.aws_service.create_msk_client(aws_account_dto).await?;
        let mut resources: Vec<AwsResourceModel> = Vec::new();
        let mut next_token: Option<String> = None;

        loop {
            let mut request = client.list_clusters_v2();
            if let Some(token) = next_token {
                request = request.next_token(token);
            }

            let response = request.send().await.map_err(|e| {
                error!("Failed to list MSK clusters: {}", e);
                AppError::ExternalService(format!("Failed to list MSK clusters: {}", e))
            })?;

            for cluster in response.cluster_info_list() {
                let cluster_arn = cluster.cluster_arn().unwrap_or("").to_string();
                let cluster_name = cluster.cluster_name().unwrap_or("").to_string();
                if cluster_arn.is_empty() {
                    continue;
                }

                let mut resource_data = serde_json::Map::new();

                if let Some(state) = cluster.state() {
                    resource_data.insert("state".to_string(), json!(state.as_str()));
                }
                if let Some(cluster_type) = cluster.cluster_type() {
                    resource_data.insert("cluster_type".to_string(), json!(cluster_type.as_str()));
                }
                if let Some(created) = cluster.creation_time() {
                    resource_data.insert("creation_time".to_string(), json!(created.to_string()));
                }

                // Provisioned cluster details
                if let Some(provisioned) = cluster.provisioned() {
                    // kafka version via current_broker_software_info
                    if let Some(sw) = provisioned.current_broker_software_info() {
                        if let Some(kafka_version) = sw.kafka_version() {
                            resource_data.insert("kafka_version".to_string(), json!(kafka_version));
                        }
                    }
                    // broker node group info for instance type
                    if let Some(broker_info) = provisioned.broker_node_group_info() {
                        if let Some(instance_type) = broker_info.instance_type() {
                            resource_data.insert("instance_type".to_string(), json!(instance_type));
                        }
                        if let Some(storage) = broker_info
                            .storage_info()
                            .and_then(|s| s.ebs_storage_info())
                        {
                            if let Some(volume_size) = storage.volume_size() {
                                resource_data.insert(
                                    "storage_per_broker_gb".to_string(),
                                    json!(volume_size),
                                );
                            }
                        }
                    }
                    // number_of_broker_nodes is directly on Provisioned
                    if let Some(broker_count) = provisioned.number_of_broker_nodes() {
                        resource_data
                            .insert("number_of_broker_nodes".to_string(), json!(broker_count));
                    }
                    if let Some(enc) = provisioned.encryption_info() {
                        if let Some(enc_transit) = enc.encryption_in_transit() {
                            if let Some(tls) = enc_transit.client_broker() {
                                resource_data.insert(
                                    "encryption_in_transit_client_broker".to_string(),
                                    json!(tls.as_str()),
                                );
                            }
                            resource_data.insert(
                                "encryption_in_transit_in_cluster".to_string(),
                                json!(enc_transit.in_cluster()),
                            );
                        }
                        if let Some(enc_rest) = enc.encryption_at_rest() {
                            if let Some(kms) = enc_rest.data_volume_kms_key_id() {
                                resource_data.insert("kms_key_id".to_string(), json!(kms));
                            }
                        }
                    }
                    if let Some(auth) = provisioned.client_authentication() {
                        let sasl_scram = auth
                            .sasl()
                            .and_then(|s| s.scram())
                            .and_then(|sc| sc.enabled())
                            .unwrap_or(false);
                        resource_data.insert("sasl_scram_enabled".to_string(), json!(sasl_scram));
                        let sasl_iam = auth
                            .sasl()
                            .and_then(|s| s.iam())
                            .and_then(|i| i.enabled())
                            .unwrap_or(false);
                        resource_data.insert("sasl_iam_enabled".to_string(), json!(sasl_iam));
                        let tls = auth.tls().and_then(|t| t.enabled()).unwrap_or(false);
                        resource_data.insert("tls_enabled".to_string(), json!(tls));
                        let unauth = auth
                            .unauthenticated()
                            .and_then(|u| u.enabled())
                            .unwrap_or(false);
                        resource_data.insert("unauthenticated_enabled".to_string(), json!(unauth));
                    }
                    if let Some(monitoring) = provisioned.enhanced_monitoring() {
                        resource_data.insert(
                            "enhanced_monitoring".to_string(),
                            json!(monitoring.as_str()),
                        );
                    }
                    if let Some(logging) = provisioned.logging_info() {
                        // broker_logs() returns Option<&BrokerLogs>
                        if let Some(broker_logs) = logging.broker_logs() {
                            let cw_enabled = broker_logs
                                .cloud_watch_logs()
                                .and_then(|cw| cw.enabled())
                                .unwrap_or(false);
                            resource_data
                                .insert("cloudwatch_logs_enabled".to_string(), json!(cw_enabled));
                            let s3_enabled =
                                broker_logs.s3().and_then(|s| s.enabled()).unwrap_or(false);
                            resource_data.insert("s3_logs_enabled".to_string(), json!(s3_enabled));
                        } else {
                            resource_data
                                .insert("cloudwatch_logs_enabled".to_string(), json!(false));
                            resource_data.insert("s3_logs_enabled".to_string(), json!(false));
                        }
                    }
                    resource_data.insert("serverless".to_string(), json!(false));
                } else if cluster.serverless().is_some() {
                    resource_data.insert("serverless".to_string(), json!(true));
                    if let Some(serverless) = cluster.serverless() {
                        if let Some(auth) = serverless.client_authentication() {
                            let sasl_iam = auth
                                .sasl()
                                .and_then(|s| s.iam())
                                .and_then(|i| i.enabled())
                                .unwrap_or(false);
                            resource_data.insert("sasl_iam_enabled".to_string(), json!(sasl_iam));
                        }
                    }
                } else {
                    resource_data.insert("serverless".to_string(), json!(false));
                }

                let mut tags_map = serde_json::Map::new();
                if let Some(tags) = cluster.tags() {
                    for (k, v) in tags {
                        tags_map.insert(k.clone(), json!(v));
                    }
                }

                let dto = AwsResourceDto {
                    id: None,
                    sync_id: Some(sync_id),
                    account_id: aws_account_dto.account_id.clone(),
                    profile: aws_account_dto.profile.clone(),
                    region: aws_account_dto.default_region.clone(),
                    resource_type: AwsResourceType::MskCluster.to_string(),
                    resource_id: cluster_arn.clone(),
                    arn: cluster_arn,
                    name: Some(cluster_name),
                    tags: serde_json::Value::Object(tags_map),
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
            "Successfully synced {} MSK clusters for account: {} with sync_id: {}",
            resources.len(),
            &aws_account_dto.account_id,
            sync_id
        );

        Ok(resources)
    }
}

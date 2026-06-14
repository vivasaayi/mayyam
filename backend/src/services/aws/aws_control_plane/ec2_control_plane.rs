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

use aws_sdk_ec2::Client as Ec2Client;
use chrono::{Duration, Utc};
use std::sync::Arc;
use tracing::{debug, error, info};
use uuid::Uuid;

use crate::errors::AppError;
use crate::models::aws_account::AwsAccountDto;
use crate::models::aws_auth::AccountAuthInfo;
use crate::models::aws_resource::{AwsResourceDto, Model as AwsResourceModel};
use crate::services::aws::aws_data_plane::cloudwatch::{
    CloudWatchMetrics, CloudWatchMetricsRequest, CloudWatchService,
};
use crate::services::aws::aws_types::ec2::{
    Ec2InstanceInfo, Ec2InstanceVolumeModification, Ec2LaunchInstanceRequest,
    Ec2SecurityGroupRequest, Ec2VolumeRequest, Tag,
};
use crate::services::aws::client_factory::AwsClientFactory;
use crate::services::AwsService;
use base64;
use serde_json::{json, Value};

// Control plane implementation for EC2
pub struct Ec2ControlPlane {
    aws_service: Arc<AwsService>,
}

impl Ec2ControlPlane {
    pub fn new(aws_service: Arc<AwsService>) -> Self {
        Self { aws_service }
    }

    pub async fn sync_instances(
        &self,
        aws_account_dto: &AwsAccountDto,
        sync_id: Uuid,
    ) -> Result<Vec<AwsResourceModel>, AppError> {
        debug!("Syncing EC2 instances with sync_id: {}.", sync_id);
        let client = self.aws_service.create_ec2_client(aws_account_dto).await?;

        // Get instances from AWS
        let response = client.describe_instances().send().await.map_err(|e| {
            error!("Failed to describe EC2 instances: {}", &e);
            error!("Error raw response: {:?}", &e.raw_response());
            error!("Error raw response: {:?}", &e.into_service_error());
            AppError::ExternalService(format!("Failed to describe EC2 instances: {}", 11))
        })?;

        let cloudwatch_service = CloudWatchService::new(self.aws_service.clone());
        let mut instances = Vec::new();

        // Process each reservation and its instances

        debug!("Described instances successfully");
        for reservation in response.reservations() {
            debug!(
                "Processing reservation: {:?}",
                &reservation.reservation_id()
            );

            for ec2_instance in reservation.instances() {
                debug!("Processing EC2 instance: {:?}", &ec2_instance.instance_id());

                let instance_id = ec2_instance.instance_id().unwrap_or_default().to_string();

                let arn = format!(
                    "arn:aws:ec2:{}:{}:instance/{}",
                    aws_account_dto.default_region, aws_account_dto.account_id, instance_id
                );

                // Extract tags
                let mut tags_map = serde_json::Map::new();
                let mut name = None;

                for tag in ec2_instance.tags() {
                    // FIX ME

                    // if let (Some(key), Some(value)) = (tag.key(), tag.value()) {
                    //     if key == "Name" {
                    //         name = Some(value.to_string());
                    //     }
                    //     tags_map.insert(key.to_string(), json!(value));
                    // }
                }

                // Build resource data
                let mut resource_data = serde_json::Map::new();

                resource_data.insert("instance_id".to_string(), json!(instance_id));

                if let Some(instance_type) = ec2_instance.instance_type().map(|t| t.as_str()) {
                    resource_data.insert("instance_type".to_string(), json!(instance_type));
                }

                if let Some(state) = ec2_instance
                    .state()
                    .and_then(|s| s.name())
                    .map(|n| n.as_str())
                {
                    resource_data.insert("state".to_string(), json!(state));
                }

                if let Some(az) = ec2_instance.placement().and_then(|p| p.availability_zone()) {
                    resource_data.insert("availability_zone".to_string(), json!(az));
                }

                if let Some(public_ip) = ec2_instance.public_ip_address() {
                    resource_data.insert("public_ip".to_string(), json!(public_ip));
                }

                if let Some(private_ip) = ec2_instance.private_ip_address() {
                    resource_data.insert("private_ip".to_string(), json!(private_ip));
                }

                if let Some(launch_time) = ec2_instance.launch_time() {
                    if let Ok(formatted_time) =
                        launch_time.fmt(aws_smithy_types::date_time::Format::DateTime)
                    {
                        resource_data.insert("launch_time".to_string(), json!(formatted_time));
                    } else {
                        // Convert DateTime to a standard format we can represent as a string
                        let launch_time_str = launch_time.as_secs_f64().to_string();
                        resource_data.insert("launch_time".to_string(), json!(launch_time_str));
                    }
                }

                if let Some(vpc_id) = ec2_instance.vpc_id() {
                    resource_data.insert("vpc_id".to_string(), json!(vpc_id));
                }

                if let Some(subnet_id) = ec2_instance.subnet_id() {
                    resource_data.insert("subnet_id".to_string(), json!(subnet_id));
                }

                if let Some(monitoring_state) = ec2_instance
                    .monitoring()
                    .and_then(|monitoring| monitoring.state())
                    .map(|state| state.as_str())
                {
                    resource_data.insert("monitoring_state".to_string(), json!(monitoring_state));
                }

                self.attach_cloudwatch_telemetry(
                    &cloudwatch_service,
                    aws_account_dto,
                    &instance_id,
                    &mut resource_data,
                )
                .await;

                // Create resource DTO
                let instance = AwsResourceDto {
                    id: None,
                    sync_id: Some(sync_id),
                    account_id: aws_account_dto.account_id.clone(),
                    profile: aws_account_dto.profile.clone(),
                    region: aws_account_dto.default_region.clone().to_string(),
                    resource_type: "EC2Instance".to_string(),
                    resource_id: instance_id.clone(),
                    arn: arn.clone(),
                    name,
                    tags: serde_json::Value::Object(tags_map),
                    resource_data: serde_json::Value::Object(resource_data),
                };

                instances.push(instance);
            }
        }

        Ok(instances.into_iter().map(|i| i.into()).collect())
    }

    async fn attach_cloudwatch_telemetry(
        &self,
        cloudwatch_service: &CloudWatchService,
        aws_account_dto: &AwsAccountDto,
        instance_id: &str,
        resource_data: &mut serde_json::Map<String, Value>,
    ) {
        let collection_started_at = Utc::now();
        let telemetry_result = self
            .collect_cloudwatch_metric_sample(cloudwatch_service, aws_account_dto, instance_id)
            .await;
        let collection_completed_at = Utc::now();
        let duration_ms = (collection_completed_at - collection_started_at).num_milliseconds();

        resource_data.insert(
            "telemetry_collection_started_at".to_string(),
            json!(collection_started_at.to_rfc3339()),
        );
        resource_data.insert(
            "telemetry_collection_completed_at".to_string(),
            json!(collection_completed_at.to_rfc3339()),
        );
        resource_data.insert(
            "telemetry_collection_duration_ms".to_string(),
            json!(duration_ms.max(0)),
        );

        match telemetry_result {
            Ok(cloudwatch_metrics) => {
                let metric_names: Vec<String> = cloudwatch_metrics
                    .get("metrics")
                    .and_then(|metrics| metrics.as_array())
                    .map(|metrics| {
                        metrics
                            .iter()
                            .filter_map(|metric| {
                                metric
                                    .get("metric_name")
                                    .and_then(|name| name.as_str())
                                    .map(|name| name.to_string())
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                let metric_count = metric_names.len();

                resource_data.insert("cloudwatch_metrics".to_string(), cloudwatch_metrics);
                resource_data.insert("cloudwatch_metric_names".to_string(), json!(metric_names));
                resource_data.insert("cloudwatch_metric_count".to_string(), json!(metric_count));
                resource_data.insert(
                    "cpu_metric_observed".to_string(),
                    json!(has_metric(resource_data, "CPUUtilization")),
                );
                resource_data.insert(
                    "network_metrics_observed".to_string(),
                    json!(
                        has_metric(resource_data, "NetworkIn")
                            && has_metric(resource_data, "NetworkOut")
                    ),
                );
                resource_data.insert(
                    "status_check_metric_observed".to_string(),
                    json!(has_metric(resource_data, "StatusCheckFailed")),
                );
                resource_data.insert(
                    "packet_metrics_observed".to_string(),
                    json!(
                        has_metric(resource_data, "NetworkPacketsIn")
                            && has_metric(resource_data, "NetworkPacketsOut")
                    ),
                );
                resource_data.insert(
                    "recovery_point_telemetry_observed".to_string(),
                    json!(recovery_point_age_hours(resource_data).is_some()),
                );
                resource_data.insert("telemetry_collection_success_count".to_string(), json!(1));
                resource_data.insert("telemetry_collection_failure_count".to_string(), json!(0));
                resource_data.insert("telemetry_collection_error_count".to_string(), json!(0));
                resource_data.insert("telemetry_collection_errors".to_string(), json!([]));
            }
            Err(error) => {
                resource_data.insert("cloudwatch_metrics".to_string(), json!({ "metrics": [] }));
                resource_data.insert("cloudwatch_metric_names".to_string(), json!([]));
                resource_data.insert("cloudwatch_metric_count".to_string(), json!(0));
                resource_data.insert("cpu_metric_observed".to_string(), json!(false));
                resource_data.insert("network_metrics_observed".to_string(), json!(false));
                resource_data.insert("status_check_metric_observed".to_string(), json!(false));
                resource_data.insert("packet_metrics_observed".to_string(), json!(false));
                resource_data.insert(
                    "recovery_point_telemetry_observed".to_string(),
                    json!(recovery_point_age_hours(resource_data).is_some()),
                );
                resource_data.insert("telemetry_collection_success_count".to_string(), json!(0));
                resource_data.insert("telemetry_collection_failure_count".to_string(), json!(1));
                resource_data.insert("telemetry_collection_error_count".to_string(), json!(1));
                resource_data.insert(
                    "telemetry_collection_errors".to_string(),
                    json!([{
                        "source": "cloudwatch",
                        "operation": "GetMetricData",
                        "error": error.to_string(),
                    }]),
                );
            }
        }
    }

    async fn collect_cloudwatch_metric_sample(
        &self,
        cloudwatch_service: &CloudWatchService,
        aws_account_dto: &AwsAccountDto,
        instance_id: &str,
    ) -> Result<Value, AppError> {
        let end_time = Utc::now();
        let start_time = end_time - Duration::hours(3);
        let metrics = ec2_core_metric_names()
            .iter()
            .map(|metric| metric.to_string())
            .collect::<Vec<_>>();
        let request = CloudWatchMetricsRequest {
            resource_type: "EC2Instance".to_string(),
            resource_id: instance_id.to_string(),
            region: aws_account_dto.default_region.clone(),
            metrics: metrics.clone(),
            start_time,
            end_time,
            period: 300,
        };

        let result = cloudwatch_service
            .get_metrics(aws_account_dto, &request)
            .await?;
        let metric_samples = result
            .metrics
            .into_iter()
            .map(|metric| {
                json!({
                    "namespace": metric.namespace,
                    "metric_name": metric.metric_name,
                    "unit": metric.unit,
                    "datapoints": metric
                        .datapoints
                        .into_iter()
                        .map(|datapoint| {
                            json!({
                                "timestamp": datapoint.timestamp.to_rfc3339(),
                                "value": datapoint.value,
                                "unit": datapoint.unit,
                            })
                        })
                        .collect::<Vec<_>>()
                })
            })
            .collect::<Vec<_>>();

        Ok(json!({
            "source": "CloudWatch",
            "namespace": "AWS/EC2",
            "resource_id": instance_id,
            "lookback_hours": 3,
            "period_seconds": 300,
            "requested_metrics": metrics,
            "metrics": metric_samples,
            "collected_at": end_time.to_rfc3339(),
        }))
    }

    pub async fn launch_instances(
        &self,
        aws_account_dto: &AwsAccountDto,
        request: &Ec2LaunchInstanceRequest,
    ) -> Result<Vec<Ec2InstanceInfo>, AppError> {
        let client = self.aws_service.create_ec2_client(aws_account_dto).await?;

        // Prepare run instances request
        let mut run_instances_req = client
            .run_instances()
            .image_id(&request.image_id)
            .instance_type(aws_sdk_ec2::types::InstanceType::from(
                request.instance_type.as_str(),
            ))
            .min_count(request.min_count)
            .max_count(request.max_count);

        // Add optional parameters
        if let Some(subnet_id) = &request.subnet_id {
            run_instances_req = run_instances_req.subnet_id(subnet_id);
        }

        if let Some(sg_ids) = &request.security_group_ids {
            for sg_id in sg_ids {
                run_instances_req = run_instances_req.security_group_ids(sg_id);
            }
        }

        if let Some(key_name) = &request.key_name {
            run_instances_req = run_instances_req.key_name(key_name);
        }

        if let Some(user_data) = &request.user_data {
            run_instances_req = run_instances_req.user_data(user_data.clone());
        }

        // Run the instances
        let response = run_instances_req.send().await.map_err(|e| {
            AppError::ExternalService(format!("Failed to launch EC2 instances: {}", e))
        })?;

        // Process the response
        let mut instances = Vec::new();

        for ec2_instance in response.instances() {
            let instance_info = Ec2InstanceInfo {
                instance_id: ec2_instance.instance_id().unwrap_or_default().to_string(),
                instance_type: ec2_instance
                    .instance_type()
                    .map_or_else(|| "unknown".to_string(), |t| t.as_str().to_string()),
                state: ec2_instance
                    .state()
                    .and_then(|s| s.name())
                    .map_or_else(|| "unknown".to_string(), |s| s.as_str().to_string()),
                availability_zone: ec2_instance
                    .placement()
                    .and_then(|p| p.availability_zone())
                    .unwrap_or_default()
                    .to_string(),
                public_ip: ec2_instance.public_ip_address().map(|s| s.to_string()),
                private_ip: ec2_instance.private_ip_address().map(|s| s.to_string()),
                launch_time: ec2_instance.launch_time().map_or_else(
                    || chrono::Utc::now().to_rfc3339(),
                    |t| {
                        if let Ok(formatted) = t.fmt(aws_smithy_types::date_time::Format::DateTime)
                        {
                            formatted
                        } else {
                            // Fall back to seconds since epoch
                            t.as_secs_f64().to_string()
                        }
                    },
                ),
                vpc_id: ec2_instance.vpc_id().map(|s| s.to_string()),
                subnet_id: ec2_instance.subnet_id().map(|s| s.to_string()),
            };

            instances.push(instance_info);
        }

        Ok(instances)
    }

    pub async fn start_instances(
        &self,
        aws_account_dto: &AwsAccountDto,
        instance_ids: &[String],
    ) -> Result<Vec<(String, String)>, AppError> {
        let client = self.aws_service.create_ec2_client(aws_account_dto).await?;

        let mut request = client.start_instances();

        // Add all instance IDs to the request
        for id in instance_ids {
            request = request.instance_ids(id);
        }

        // Send the request
        let response = request.send().await.map_err(|e| {
            AppError::ExternalService(format!("Failed to start EC2 instances: {}", e))
        })?;

        // Process response to extract instance states
        let mut result = Vec::new();

        for instance in response.starting_instances() {
            let id = instance.instance_id().unwrap_or_default().to_string();
            let state = instance
                .current_state()
                .and_then(|s| s.name())
                .map_or_else(|| "unknown".to_string(), |s| s.as_str().to_string());

            result.push((id, state));
        }

        Ok(result)
    }

    pub async fn stop_instances(
        &self,
        aws_account_dto: &AwsAccountDto,
        instance_ids: &[String],
        force: bool,
    ) -> Result<Vec<(String, String)>, AppError> {
        let client = self.aws_service.create_ec2_client(aws_account_dto).await?;

        let mut stopping_instances = Vec::new();
        for id in instance_ids {
            if force {
                let _ = client.terminate_instances().instance_ids(id).send().await?;
                stopping_instances.push((id.clone(), "terminated".to_string()));
            } else {
                let _ = client.stop_instances().instance_ids(id).send().await?;
                stopping_instances.push((id.clone(), "stopping".to_string()));
            }
        }

        Ok(stopping_instances)
    }

    pub async fn reboot_instances(
        &self,
        aws_account_dto: &AwsAccountDto,
        instance_ids: &[String],
    ) -> Result<Vec<(String, String)>, AppError> {
        let client = self.aws_service.create_ec2_client(aws_account_dto).await?;

        let mut rebooting_instances = Vec::new();
        for id in instance_ids {
            let _ = client.reboot_instances().instance_ids(id).send().await?;
            rebooting_instances.push((id.clone(), "rebooting".to_string()));
        }

        Ok(rebooting_instances)
    }

    pub async fn get_instance_tags(
        &self,
        aws_account_dto: &AwsAccountDto,
        instance_id: &String,
    ) -> Result<Vec<Tag>, AppError> {
        let client = self.aws_service.create_ec2_client(aws_account_dto).await?;

        let sdk_tags = client
            .describe_instances()
            .instance_ids(instance_id)
            .send()
            .await?
            .reservations
            .unwrap_or_default()
            .into_iter()
            .flat_map(|r| r.instances.unwrap_or_default().into_iter())
            .flat_map(|i| i.tags.unwrap_or_default())
            .collect::<Vec<aws_sdk_ec2::types::Tag>>();

        // Convert AWS SDK tags to our custom Tag type
        let tags: Vec<Tag> = sdk_tags
            .into_iter()
            .map(|tag| Tag {
                key: Some(tag.key().unwrap_or_default().to_string()),
                value: Some(tag.value().unwrap_or_default().to_string()),
            })
            .collect();

        Ok(tags)
    }

    pub async fn update_instance_tags(
        &self,
        aws_account_dto: &AwsAccountDto,
        instance_id: &String,
        tags: Vec<Tag>,
    ) -> Result<(), AppError> {
        let client = self.aws_service.create_ec2_client(aws_account_dto).await?;

        // Convert our custom Tag type to AWS SDK Tag type
        let sdk_tags: Vec<aws_sdk_ec2::types::Tag> = tags
            .into_iter()
            .map(|tag| {
                aws_sdk_ec2::types::Tag::builder()
                    .key(tag.key.unwrap_or_default())
                    .value(tag.value.unwrap_or_default())
                    .build()
            })
            .collect();

        let _ = client
            .create_tags()
            .resources(instance_id)
            .set_tags(Some(sdk_tags))
            .send()
            .await?;

        Ok(())
    }

    pub async fn terminate_instances(
        &self,
        aws_account_dto: &AwsAccountDto,
        instance_ids: &[String],
    ) -> Result<Vec<(String, String)>, AppError> {
        let client = self.aws_service.create_ec2_client(aws_account_dto).await?;

        let mut request = client.terminate_instances();

        // Add all instance IDs to the request
        for id in instance_ids {
            request = request.instance_ids(id);
        }

        // Send the request
        let response = request.send().await.map_err(|e| {
            AppError::ExternalService(format!("Failed to terminate EC2 instances: {}", e))
        })?;

        // Process response to extract instance states
        let mut result = Vec::new();

        for instance in response.terminating_instances() {
            let id = instance.instance_id().unwrap_or_default().to_string();
            let state = instance
                .current_state()
                .and_then(|s| s.name())
                .map_or_else(|| "unknown".to_string(), |s| s.as_str().to_string());

            result.push((id, state));
        }

        Ok(result)
    }

    pub async fn describe_instances(
        &self,
        aws_account_dto: &AwsAccountDto,
        instance_ids: Option<&[String]>,
    ) -> Result<Vec<Ec2InstanceInfo>, AppError> {
        let client = self.aws_service.create_ec2_client(aws_account_dto).await?;

        let mut request = client.describe_instances();

        // Add instance IDs to filter if provided
        if let Some(ids) = instance_ids {
            for id in ids {
                request = request.instance_ids(id);
            }
        }

        // Send request
        let response = request.send().await.map_err(|e| {
            AppError::ExternalService(format!("Failed to describe EC2 instances: {}", e))
        })?;

        let mut instances = Vec::new();

        // Process each reservation and its instances

        for reservation in response.reservations() {
            for ec2_instance in reservation.instances() {
                let instance_info = Ec2InstanceInfo {
                    instance_id: ec2_instance.instance_id().unwrap_or_default().to_string(),
                    instance_type: ec2_instance
                        .instance_type()
                        .map_or_else(|| "unknown".to_string(), |t| t.as_str().to_string()),
                    state: ec2_instance
                        .state()
                        .and_then(|s| s.name())
                        .map_or_else(|| "unknown".to_string(), |s| s.as_str().to_string()),
                    availability_zone: ec2_instance
                        .placement()
                        .and_then(|p| p.availability_zone())
                        .unwrap_or_default()
                        .to_string(),
                    public_ip: ec2_instance.public_ip_address().map(|s| s.to_string()),
                    private_ip: ec2_instance.private_ip_address().map(|s| s.to_string()),
                    launch_time: ec2_instance.launch_time().map_or_else(
                        || chrono::Utc::now().to_rfc3339(),
                        |t| {
                            if let Ok(formatted) =
                                t.fmt(aws_smithy_types::date_time::Format::DateTime)
                            {
                                formatted
                            } else {
                                t.as_secs_f64().to_string()
                            }
                        },
                    ),
                    vpc_id: ec2_instance.vpc_id().map(|s| s.to_string()),
                    subnet_id: ec2_instance.subnet_id().map(|s| s.to_string()),
                };

                instances.push(instance_info);
            }
        }

        Ok(instances)
    }

    pub async fn create_security_group(
        &self,
        aws_account_dto: &AwsAccountDto,
        request: &Ec2SecurityGroupRequest,
    ) -> Result<String, AppError> {
        let client = self.aws_service.create_ec2_client(aws_account_dto).await?;

        // Create the security group
        let create_response = client
            .create_security_group()
            .group_name(&request.group_name)
            .description(&request.description)
            .vpc_id(&request.vpc_id)
            .send()
            .await
            .map_err(|e| {
                AppError::ExternalService(format!("Failed to create security group: {}", e))
            })?;

        let group_id = create_response
            .group_id()
            .ok_or_else(|| {
                AppError::ExternalService("No security group ID returned from AWS".to_string())
            })?
            .to_string();

        // Add ingress rules
        for rule in &request.ingress_rules {
            let mut ingress_request = client
                .authorize_security_group_ingress()
                .group_id(&group_id)
                .ip_protocol(&rule.ip_protocol)
                .from_port(rule.from_port)
                .to_port(rule.to_port);

            for cidr in &rule.cidr_blocks {
                ingress_request = ingress_request.cidr_ip(cidr);
            }

            // No built-in description method in the SDK, so we won't set it
            // AWS SDK doesn't support setting description for individual rules directly this way

            ingress_request.send().await.map_err(|e| {
                AppError::ExternalService(format!("Failed to add ingress rules: {}", e))
            })?;
        }

        // Add egress rules
        for rule in &request.egress_rules {
            let mut egress_request = client
                .authorize_security_group_egress()
                .group_id(&group_id)
                .ip_protocol(&rule.ip_protocol)
                .from_port(rule.from_port)
                .to_port(rule.to_port);

            for cidr in &rule.cidr_blocks {
                egress_request = egress_request.cidr_ip(cidr);
            }

            // No built-in description method in the SDK, so we won't set it
            // AWS SDK doesn't support setting description for individual rules directly this way

            egress_request.send().await.map_err(|e| {
                AppError::ExternalService(format!("Failed to add egress rules: {}", e))
            })?;
        }

        // Add tags if provided
        if let Some(tags) = &request.tags {
            if let serde_json::Value::Object(tag_map) = tags {
                let mut tag_list = Vec::new();

                for (key, value) in tag_map {
                    if let Some(val_str) = value.as_str() {
                        let tag = aws_sdk_ec2::types::Tag::builder()
                            .key(key)
                            .value(val_str)
                            .build();
                        tag_list.push(tag);
                    }
                }

                client
                    .create_tags()
                    .resources(&group_id)
                    .set_tags(Some(tag_list))
                    .send()
                    .await
                    .map_err(|e| AppError::ExternalService(format!("Failed to add tags: {}", e)))?;
            }
        }

        Ok(group_id)
    }

    pub async fn create_volume(
        &self,
        aws_account_dto: &AwsAccountDto,
        request: &Ec2VolumeRequest,
    ) -> Result<String, AppError> {
        let client = self.aws_service.create_ec2_client(aws_account_dto).await?;

        let mut create_volume_req = client
            .create_volume()
            .availability_zone(&request.availability_zone)
            .volume_type(aws_sdk_ec2::types::VolumeType::from(
                request.volume_type.as_str(),
            ))
            .size(request.size);

        // Add optional parameters
        if let Some(iops) = request.iops {
            create_volume_req = create_volume_req.iops(iops);
        }

        if let Some(encrypted) = request.encrypted {
            create_volume_req = create_volume_req.encrypted(encrypted);
        }

        // Create the volume
        let response = create_volume_req.send().await.map_err(|e| {
            AppError::ExternalService(format!("Failed to create EC2 volume: {}", e))
        })?;

        let volume_id = response
            .volume_id()
            .ok_or_else(|| AppError::ExternalService("No volume ID returned from AWS".to_string()))?
            .to_string();

        // Add tags if provided
        if let Some(tags) = &request.tags {
            if let serde_json::Value::Object(tag_map) = tags {
                let mut tag_list = Vec::new();

                for (key, value) in tag_map {
                    if let Some(val_str) = value.as_str() {
                        let tag = aws_sdk_ec2::types::Tag::builder()
                            .key(key)
                            .value(val_str)
                            .build();
                        tag_list.push(tag);
                    }
                }

                client
                    .create_tags()
                    .resources(&volume_id)
                    .set_tags(Some(tag_list))
                    .send()
                    .await
                    .map_err(|e| AppError::ExternalService(format!("Failed to add tags: {}", e)))?;
            }
        }

        Ok(volume_id)
    }

    pub async fn attach_volume(
        &self,
        aws_account_dto: &AwsAccountDto,
        modification: &Ec2InstanceVolumeModification,
    ) -> Result<(), AppError> {
        let client = self.aws_service.create_ec2_client(aws_account_dto).await?;

        let mut request = client
            .attach_volume()
            .instance_id(&modification.instance_id)
            .volume_id(&modification.volume_id)
            .device(&modification.device_name);

        // Send the request to AWS
        let response = request
            .send()
            .await
            .map_err(|e| AppError::ExternalService(format!("Failed to attach volume: {}", e)))?;

        // Check response
        if response.volume_id().is_some() {
            Ok(())
        } else {
            Err(AppError::ExternalService(
                "Failed to attach volume: No volume ID returned".to_string(),
            ))
        }
    }

    pub async fn modify_instance_attribute(
        &self,
        aws_account_dto: &AwsAccountDto,
        instance_id: &str,
        attribute: &str,
        value: &str,
    ) -> Result<(), AppError> {
        let client = self.aws_service.create_ec2_client(aws_account_dto).await?;

        // Create the appropriate modify request based on the attribute
        match attribute {
            "instanceType" => {
                client
                    .modify_instance_attribute()
                    .instance_id(instance_id)
                    .instance_type(
                        aws_sdk_ec2::types::AttributeValue::builder()
                            .value(value)
                            .build(),
                    )
                    .send()
                    .await
                    .map_err(|e| {
                        AppError::ExternalService(format!("Failed to modify instance type: {}", e))
                    })?;
            }
            "userData" => {
                // For user data we directly encode the value as base64 and send
                let encoded_value = base64::encode(value.as_bytes());
                // Convert to aws_smithy_types::Blob
                let blob = ::aws_smithy_types::Blob::new(encoded_value.into_bytes());
                client
                    .modify_instance_attribute()
                    .instance_id(instance_id)
                    .user_data(
                        aws_sdk_ec2::types::BlobAttributeValue::builder()
                            .value(blob)
                            .build(),
                    )
                    .send()
                    .await
                    .map_err(|e| {
                        AppError::ExternalService(format!("Failed to modify user data: {}", e))
                    })?;
            }
            "disableApiTermination" => {
                let bool_value = value.parse::<bool>().map_err(|_| {
                    AppError::ExternalService(format!(
                        "Invalid boolean value for disableApiTermination: {}",
                        value
                    ))
                })?;

                client
                    .modify_instance_attribute()
                    .instance_id(instance_id)
                    .disable_api_termination(
                        aws_sdk_ec2::types::AttributeBooleanValue::builder()
                            .value(bool_value)
                            .build(),
                    )
                    .send()
                    .await
                    .map_err(|e| {
                        AppError::ExternalService(format!(
                            "Failed to modify termination protection: {}",
                            e
                        ))
                    })?;
            }
            "instanceInitiatedShutdownBehavior" => {
                client
                    .modify_instance_attribute()
                    .instance_id(instance_id)
                    .instance_initiated_shutdown_behavior(
                        aws_sdk_ec2::types::AttributeValue::builder()
                            .value(value)
                            .build(),
                    )
                    .send()
                    .await
                    .map_err(|e| {
                        AppError::ExternalService(format!(
                            "Failed to modify shutdown behavior: {}",
                            e
                        ))
                    })?;
            }
            _ => {
                return Err(AppError::ExternalService(format!(
                    "Unsupported instance attribute: {}",
                    attribute
                )));
            }
        }

        Ok(())
    }
}

fn ec2_core_metric_names() -> [&'static str; 10] {
    [
        "CPUUtilization",
        "NetworkIn",
        "NetworkOut",
        "NetworkPacketsIn",
        "NetworkPacketsOut",
        "DiskReadOps",
        "DiskWriteOps",
        "StatusCheckFailed",
        "StatusCheckFailed_Instance",
        "StatusCheckFailed_System",
    ]
}

fn has_metric(resource_data: &serde_json::Map<String, Value>, metric_name: &str) -> bool {
    resource_data
        .get("cloudwatch_metrics")
        .and_then(|cloudwatch| cloudwatch.get("metrics"))
        .and_then(|metrics| metrics.as_array())
        .map(|metrics| {
            metrics.iter().any(|metric| {
                metric
                    .get("metric_name")
                    .or_else(|| metric.get("MetricName"))
                    .and_then(|name| name.as_str())
                    .map(|name| name == metric_name)
                    .unwrap_or(false)
                    && metric
                        .get("datapoints")
                        .or_else(|| metric.get("Datapoints"))
                        .and_then(|datapoints| datapoints.as_array())
                        .map(|datapoints| !datapoints.is_empty())
                        .unwrap_or(false)
            })
        })
        .unwrap_or(false)
}

fn recovery_point_age_hours(resource_data: &serde_json::Map<String, Value>) -> Option<f64> {
    for key in [
        "latest_recovery_point_age_hours",
        "recovery_point_age_hours",
        "backup_age_hours",
    ] {
        if let Some(value) = resource_data
            .get(key)
            .and_then(|value| value.as_f64().or_else(|| value.as_i64().map(|n| n as f64)))
        {
            return Some(value);
        }
    }

    resource_data
        .get("disaster_recovery")
        .and_then(|value| value.get("latest_recovery_point_age_hours"))
        .and_then(|value| value.as_f64().or_else(|| value.as_i64().map(|n| n as f64)))
}

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
use aws_sdk_drs::types::{
    Account, Job, LaunchConfigurationTemplate, RecoveryInstance, ReplicationConfigurationTemplate,
    SourceNetwork, SourceServer,
};
use aws_sdk_drs::Client as DrsClient;
use serde_json::{json, Map, Value};
use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;
use tracing::debug;
use uuid::Uuid;

pub struct DrsControlPlane {
    aws_service: Arc<AwsService>,
}

#[derive(Debug, Default)]
struct CollectionSummary {
    resources_by_kind: BTreeMap<String, usize>,
    collection_errors: Vec<Value>,
    collection_error_count: usize,
    resource_count: usize,
    untagged_resource_count: usize,
    active_replication_source_count: usize,
    replication_problem_count: usize,
    source_network_error_count: usize,
    failed_launch_count: usize,
    failed_or_incomplete_job_count: usize,
    public_replication_template_count: usize,
    default_ebs_encryption_template_count: usize,
    staging_account_count: usize,
}

impl DrsControlPlane {
    pub fn new(aws_service: Arc<AwsService>) -> Self {
        Self { aws_service }
    }

    pub async fn sync_resources(
        &self,
        aws_account_dto: &AwsAccountDto,
        sync_id: Uuid,
    ) -> Result<Vec<AwsResourceModel>, AppError> {
        debug!(
            "Syncing AWS Elastic Disaster Recovery inventory for account: {} with sync_id: {}",
            &aws_account_dto.account_id, sync_id
        );

        let client = self.aws_service.create_drs_client(aws_account_dto).await?;
        let mut resources = Vec::new();
        let mut summary = CollectionSummary::default();

        match list_source_servers(&client).await {
            Ok(items) => {
                for item in items {
                    resources.push(source_server_resource(
                        aws_account_dto,
                        sync_id,
                        &item,
                        &mut summary,
                    ));
                }
            }
            Err(e) => {
                record_collection_error(&mut summary, "source_server", "DescribeSourceServers", e)
            }
        }

        match list_recovery_instances(&client).await {
            Ok(items) => {
                for item in items {
                    resources.push(recovery_instance_resource(
                        aws_account_dto,
                        sync_id,
                        &item,
                        &mut summary,
                    ));
                }
            }
            Err(e) => record_collection_error(
                &mut summary,
                "recovery_instance",
                "DescribeRecoveryInstances",
                e,
            ),
        }

        match list_source_networks(&client).await {
            Ok(items) => {
                for item in items {
                    resources.push(source_network_resource(
                        aws_account_dto,
                        sync_id,
                        &item,
                        &mut summary,
                    ));
                }
            }
            Err(e) => {
                record_collection_error(&mut summary, "source_network", "DescribeSourceNetworks", e)
            }
        }

        match list_replication_templates(&client).await {
            Ok(items) => {
                for item in items {
                    resources.push(replication_template_resource(
                        aws_account_dto,
                        sync_id,
                        &item,
                        &mut summary,
                    ));
                }
            }
            Err(e) => record_collection_error(
                &mut summary,
                "replication_configuration_template",
                "DescribeReplicationConfigurationTemplates",
                e,
            ),
        }

        match list_launch_templates(&client).await {
            Ok(items) => {
                for item in items {
                    resources.push(launch_template_resource(
                        aws_account_dto,
                        sync_id,
                        &item,
                        &mut summary,
                    ));
                }
            }
            Err(e) => record_collection_error(
                &mut summary,
                "launch_configuration_template",
                "DescribeLaunchConfigurationTemplates",
                e,
            ),
        }

        match list_jobs(&client).await {
            Ok(items) => {
                for item in items {
                    resources.push(job_resource(aws_account_dto, sync_id, &item, &mut summary));
                }
            }
            Err(e) => record_collection_error(&mut summary, "job", "DescribeJobs", e),
        }

        match list_staging_accounts(&client).await {
            Ok(items) => {
                summary.staging_account_count = items.len();
                for item in items {
                    resources.push(staging_account_resource(
                        aws_account_dto,
                        sync_id,
                        &item,
                        &mut summary,
                    ));
                }
            }
            Err(e) => {
                record_collection_error(&mut summary, "staging_account", "ListStagingAccounts", e)
            }
        }

        let summary_resource = account_summary_resource(aws_account_dto, sync_id, &summary);
        resources.insert(0, summary_resource);

        debug!(
            "Successfully synced {} AWS Elastic Disaster Recovery inventory resources for account: {} with sync_id: {}",
            resources.len(),
            &aws_account_dto.account_id,
            sync_id
        );

        Ok(resources)
    }
}

async fn list_source_servers(client: &DrsClient) -> Result<Vec<SourceServer>, String> {
    let mut resources = Vec::new();
    let mut next_token: Option<String> = None;

    loop {
        let mut request = client.describe_source_servers().max_results(100);
        if let Some(token) = next_token.take() {
            request = request.next_token(token);
        }
        let response = request.send().await.map_err(|e| e.to_string())?;
        resources.extend(response.items().iter().cloned());
        next_token = response.next_token().map(String::from);
        if next_token.is_none() {
            break;
        }
    }

    Ok(resources)
}

async fn list_recovery_instances(client: &DrsClient) -> Result<Vec<RecoveryInstance>, String> {
    let mut resources = Vec::new();
    let mut next_token: Option<String> = None;

    loop {
        let mut request = client.describe_recovery_instances().max_results(100);
        if let Some(token) = next_token.take() {
            request = request.next_token(token);
        }
        let response = request.send().await.map_err(|e| e.to_string())?;
        resources.extend(response.items().iter().cloned());
        next_token = response.next_token().map(String::from);
        if next_token.is_none() {
            break;
        }
    }

    Ok(resources)
}

async fn list_source_networks(client: &DrsClient) -> Result<Vec<SourceNetwork>, String> {
    let mut resources = Vec::new();
    let mut next_token: Option<String> = None;

    loop {
        let mut request = client.describe_source_networks().max_results(100);
        if let Some(token) = next_token.take() {
            request = request.next_token(token);
        }
        let response = request.send().await.map_err(|e| e.to_string())?;
        resources.extend(response.items().iter().cloned());
        next_token = response.next_token().map(String::from);
        if next_token.is_none() {
            break;
        }
    }

    Ok(resources)
}

async fn list_replication_templates(
    client: &DrsClient,
) -> Result<Vec<ReplicationConfigurationTemplate>, String> {
    let mut resources = Vec::new();
    let mut next_token: Option<String> = None;

    loop {
        let mut request = client
            .describe_replication_configuration_templates()
            .max_results(100);
        if let Some(token) = next_token.take() {
            request = request.next_token(token);
        }
        let response = request.send().await.map_err(|e| e.to_string())?;
        resources.extend(response.items().iter().cloned());
        next_token = response.next_token().map(String::from);
        if next_token.is_none() {
            break;
        }
    }

    Ok(resources)
}

async fn list_launch_templates(
    client: &DrsClient,
) -> Result<Vec<LaunchConfigurationTemplate>, String> {
    let mut resources = Vec::new();
    let mut next_token: Option<String> = None;

    loop {
        let mut request = client
            .describe_launch_configuration_templates()
            .max_results(100);
        if let Some(token) = next_token.take() {
            request = request.next_token(token);
        }
        let response = request.send().await.map_err(|e| e.to_string())?;
        resources.extend(response.items().iter().cloned());
        next_token = response.next_token().map(String::from);
        if next_token.is_none() {
            break;
        }
    }

    Ok(resources)
}

async fn list_jobs(client: &DrsClient) -> Result<Vec<Job>, String> {
    let mut resources = Vec::new();
    let mut next_token: Option<String> = None;

    loop {
        let mut request = client.describe_jobs().max_results(100);
        if let Some(token) = next_token.take() {
            request = request.next_token(token);
        }
        let response = request.send().await.map_err(|e| e.to_string())?;
        resources.extend(response.items().iter().cloned());
        next_token = response.next_token().map(String::from);
        if next_token.is_none() {
            break;
        }
    }

    Ok(resources)
}

async fn list_staging_accounts(client: &DrsClient) -> Result<Vec<Account>, String> {
    let mut resources = Vec::new();
    let mut next_token: Option<String> = None;

    loop {
        let mut request = client.list_staging_accounts().max_results(100);
        if let Some(token) = next_token.take() {
            request = request.next_token(token);
        }
        let response = request.send().await.map_err(|e| e.to_string())?;
        resources.extend(response.accounts().iter().cloned());
        next_token = response.next_token().map(String::from);
        if next_token.is_none() {
            break;
        }
    }

    Ok(resources)
}

fn source_server_resource(
    aws_account_dto: &AwsAccountDto,
    sync_id: Uuid,
    server: &SourceServer,
    summary: &mut CollectionSummary,
) -> AwsResourceModel {
    let resource_kind = "source_server";
    let resource_id = server.source_server_id().unwrap_or("unknown").to_string();
    let tags = tags_from_map(server.tags());
    let mut resource_data = base_resource_data(resource_kind);

    insert_str(
        &mut resource_data,
        "source_server_id",
        server.source_server_id(),
    );
    insert_str(
        &mut resource_data,
        "recovery_instance_id",
        server.recovery_instance_id(),
    );
    insert_str(
        &mut resource_data,
        "source_network_id",
        server.source_network_id(),
    );
    insert_str(&mut resource_data, "agent_version", server.agent_version());
    insert_str(
        &mut resource_data,
        "reversed_direction_source_server_arn",
        server.reversed_direction_source_server_arn(),
    );
    if let Some(direction) = server.replication_direction() {
        resource_data.insert(
            "replication_direction".to_string(),
            json!(direction.as_str()),
        );
    }
    if let Some(last_launch_result) = server.last_launch_result() {
        let last_launch_result = last_launch_result.as_str();
        resource_data.insert("last_launch_result".to_string(), json!(last_launch_result));
        if last_launch_result == "FAILED" {
            summary.failed_launch_count += 1;
        }
    }
    if let Some(replication_info) = server.data_replication_info() {
        insert_str(
            &mut resource_data,
            "lag_duration",
            replication_info.lag_duration(),
        );
        insert_str(
            &mut resource_data,
            "eta_date_time",
            replication_info.eta_date_time(),
        );
        insert_str(
            &mut resource_data,
            "staging_availability_zone",
            replication_info.staging_availability_zone(),
        );
        insert_str(
            &mut resource_data,
            "staging_outpost_arn",
            replication_info.staging_outpost_arn(),
        );
        if let Some(state) = replication_info.data_replication_state() {
            let state = state.as_str();
            resource_data.insert("data_replication_state".to_string(), json!(state));
            if is_active_replication_state(state) {
                summary.active_replication_source_count += 1;
            }
            if is_replication_problem_state(state) {
                summary.replication_problem_count += 1;
            }
        }
        if let Some(error) = replication_info.data_replication_error() {
            if let Some(error_code) = error.error() {
                resource_data.insert(
                    "data_replication_error".to_string(),
                    json!(error_code.as_str()),
                );
                summary.replication_problem_count += 1;
            }
            insert_str(
                &mut resource_data,
                "data_replication_raw_error",
                error.raw_error(),
            );
        }
        resource_data.insert(
            "replicated_disk_count".to_string(),
            json!(replication_info.replicated_disks().len()),
        );
    }
    if let Some(life_cycle) = server.life_cycle() {
        insert_str(
            &mut resource_data,
            "added_to_service_date_time",
            life_cycle.added_to_service_date_time(),
        );
        insert_str(
            &mut resource_data,
            "first_byte_date_time",
            life_cycle.first_byte_date_time(),
        );
        insert_str(
            &mut resource_data,
            "last_seen_by_service_date_time",
            life_cycle.last_seen_by_service_date_time(),
        );
        insert_str(
            &mut resource_data,
            "elapsed_replication_duration",
            life_cycle.elapsed_replication_duration(),
        );
    }
    if let Some(properties) = server.source_properties() {
        insert_str(
            &mut resource_data,
            "source_last_updated_date_time",
            properties.last_updated_date_time(),
        );
        insert_str(
            &mut resource_data,
            "recommended_instance_type",
            properties.recommended_instance_type(),
        );
        resource_data.insert(
            "source_disk_count".to_string(),
            json!(properties.disks().len()),
        );
        resource_data.insert(
            "source_cpu_count".to_string(),
            json!(properties.cpus().len()),
        );
        resource_data.insert(
            "source_network_interface_count".to_string(),
            json!(properties.network_interfaces().len()),
        );
        resource_data.insert("ram_bytes".to_string(), json!(properties.ram_bytes()));
    }
    if let Some(staging_area) = server.staging_area() {
        if let Some(status) = staging_area.status() {
            let status = status.as_str();
            resource_data.insert("staging_area_status".to_string(), json!(status));
            if status == "EXTENSION_ERROR" {
                summary.replication_problem_count += 1;
            }
        }
        insert_str(
            &mut resource_data,
            "staging_account_id",
            staging_area.staging_account_id(),
        );
        insert_str(
            &mut resource_data,
            "staging_source_server_arn",
            staging_area.staging_source_server_arn(),
        );
        insert_str(
            &mut resource_data,
            "staging_area_error_message",
            staging_area.error_message(),
        );
    }

    resource_model(
        aws_account_dto,
        sync_id,
        resource_kind,
        &resource_id,
        server.arn(),
        Some(resource_id.clone()),
        tags,
        Value::Object(resource_data),
        summary,
    )
}

fn recovery_instance_resource(
    aws_account_dto: &AwsAccountDto,
    sync_id: Uuid,
    instance: &RecoveryInstance,
    summary: &mut CollectionSummary,
) -> AwsResourceModel {
    let resource_kind = "recovery_instance";
    let resource_id = instance
        .recovery_instance_id()
        .unwrap_or("unknown")
        .to_string();
    let tags = tags_from_map(instance.tags());
    let mut resource_data = base_resource_data(resource_kind);

    insert_str(
        &mut resource_data,
        "recovery_instance_id",
        instance.recovery_instance_id(),
    );
    insert_str(
        &mut resource_data,
        "ec2_instance_id",
        instance.ec2_instance_id(),
    );
    insert_str(&mut resource_data, "job_id", instance.job_id());
    insert_str(
        &mut resource_data,
        "source_server_id",
        instance.source_server_id(),
    );
    insert_bool(&mut resource_data, "is_drill", instance.is_drill());
    insert_str(
        &mut resource_data,
        "point_in_time_snapshot_date_time",
        instance.point_in_time_snapshot_date_time(),
    );
    insert_str(
        &mut resource_data,
        "origin_availability_zone",
        instance.origin_availability_zone(),
    );
    insert_str(
        &mut resource_data,
        "agent_version",
        instance.agent_version(),
    );
    if let Some(state) = instance.ec2_instance_state() {
        resource_data.insert("ec2_instance_state".to_string(), json!(state.as_str()));
    }
    if let Some(origin) = instance.origin_environment() {
        resource_data.insert("origin_environment".to_string(), json!(origin.as_str()));
    }
    if let Some(replication_info) = instance.data_replication_info() {
        insert_str(
            &mut resource_data,
            "lag_duration",
            replication_info.lag_duration(),
        );
        insert_str(
            &mut resource_data,
            "eta_date_time",
            replication_info.eta_date_time(),
        );
        insert_str(
            &mut resource_data,
            "staging_availability_zone",
            replication_info.staging_availability_zone(),
        );
        if let Some(state) = replication_info.data_replication_state() {
            let state = state.as_str();
            resource_data.insert("data_replication_state".to_string(), json!(state));
            if is_replication_problem_state(state) {
                summary.replication_problem_count += 1;
            }
        }
        if let Some(error) = replication_info.data_replication_error() {
            if let Some(error_code) = error.error() {
                resource_data.insert(
                    "data_replication_error".to_string(),
                    json!(error_code.as_str()),
                );
                summary.replication_problem_count += 1;
            }
            insert_str(
                &mut resource_data,
                "data_replication_raw_error",
                error.raw_error(),
            );
        }
        resource_data.insert(
            "replicated_disk_count".to_string(),
            json!(replication_info.replicated_disks().len()),
        );
    }
    if let Some(properties) = instance.recovery_instance_properties() {
        insert_str(
            &mut resource_data,
            "recovery_instance_last_updated_date_time",
            properties.last_updated_date_time(),
        );
        resource_data.insert(
            "recovery_disk_count".to_string(),
            json!(properties.disks().len()),
        );
        resource_data.insert(
            "recovery_cpu_count".to_string(),
            json!(properties.cpus().len()),
        );
        resource_data.insert(
            "recovery_network_interface_count".to_string(),
            json!(properties.network_interfaces().len()),
        );
        resource_data.insert("ram_bytes".to_string(), json!(properties.ram_bytes()));
    }

    resource_model(
        aws_account_dto,
        sync_id,
        resource_kind,
        &resource_id,
        instance.arn(),
        Some(resource_id.clone()),
        tags,
        Value::Object(resource_data),
        summary,
    )
}

fn source_network_resource(
    aws_account_dto: &AwsAccountDto,
    sync_id: Uuid,
    network: &SourceNetwork,
    summary: &mut CollectionSummary,
) -> AwsResourceModel {
    let resource_kind = "source_network";
    let resource_id = network.source_network_id().unwrap_or("unknown").to_string();
    let tags = tags_from_map(network.tags());
    let mut resource_data = base_resource_data(resource_kind);

    insert_str(
        &mut resource_data,
        "source_network_id",
        network.source_network_id(),
    );
    insert_str(&mut resource_data, "source_vpc_id", network.source_vpc_id());
    insert_str(&mut resource_data, "source_region", network.source_region());
    insert_str(
        &mut resource_data,
        "source_account_id",
        network.source_account_id(),
    );
    insert_str(
        &mut resource_data,
        "cfn_stack_name",
        network.cfn_stack_name(),
    );
    insert_str(
        &mut resource_data,
        "launched_vpc_id",
        network.launched_vpc_id(),
    );
    insert_str(
        &mut resource_data,
        "replication_status_details",
        network.replication_status_details(),
    );
    if let Some(status) = network.replication_status() {
        let status = status.as_str();
        resource_data.insert("replication_status".to_string(), json!(status));
        if status == "ERROR" {
            summary.source_network_error_count += 1;
        }
    }

    resource_model(
        aws_account_dto,
        sync_id,
        resource_kind,
        &resource_id,
        network.arn(),
        Some(resource_id.clone()),
        tags,
        Value::Object(resource_data),
        summary,
    )
}

fn replication_template_resource(
    aws_account_dto: &AwsAccountDto,
    sync_id: Uuid,
    template: &ReplicationConfigurationTemplate,
    summary: &mut CollectionSummary,
) -> AwsResourceModel {
    let resource_kind = "replication_configuration_template";
    let resource_id = template.replication_configuration_template_id().to_string();
    let tags = tags_from_map(template.tags());
    let mut resource_data = base_resource_data(resource_kind);

    resource_data.insert(
        "replication_configuration_template_id".to_string(),
        json!(template.replication_configuration_template_id()),
    );
    insert_str(
        &mut resource_data,
        "staging_area_subnet_id",
        template.staging_area_subnet_id(),
    );
    insert_bool(
        &mut resource_data,
        "associate_default_security_group",
        template.associate_default_security_group(),
    );
    insert_str(
        &mut resource_data,
        "replication_server_instance_type",
        template.replication_server_instance_type(),
    );
    insert_bool(
        &mut resource_data,
        "use_dedicated_replication_server",
        template.use_dedicated_replication_server(),
    );
    if let Some(ebs_encryption) = template.ebs_encryption() {
        let ebs_encryption = ebs_encryption.as_str();
        resource_data.insert("ebs_encryption".to_string(), json!(ebs_encryption));
        if ebs_encryption == "DEFAULT" {
            summary.default_ebs_encryption_template_count += 1;
        }
    }
    insert_str(
        &mut resource_data,
        "ebs_encryption_key_arn",
        template.ebs_encryption_key_arn(),
    );
    resource_data.insert(
        "bandwidth_throttling".to_string(),
        json!(template.bandwidth_throttling()),
    );
    if let Some(data_plane_routing) = template.data_plane_routing() {
        let data_plane_routing = data_plane_routing.as_str();
        resource_data.insert("data_plane_routing".to_string(), json!(data_plane_routing));
        if data_plane_routing == "PUBLIC_IP" {
            summary.public_replication_template_count += 1;
        }
    }
    insert_bool(
        &mut resource_data,
        "create_public_ip",
        template.create_public_ip(),
    );
    if template.create_public_ip() == Some(true) {
        summary.public_replication_template_count += 1;
    }
    insert_bool(
        &mut resource_data,
        "auto_replicate_new_disks",
        template.auto_replicate_new_disks(),
    );
    resource_data.insert(
        "replication_server_security_group_count".to_string(),
        json!(template.replication_servers_security_groups_ids().len()),
    );
    resource_data.insert(
        "pit_policy_rule_count".to_string(),
        json!(template.pit_policy().len()),
    );
    resource_data.insert(
        "staging_area_tags".to_string(),
        tags_from_map(template.staging_area_tags()),
    );

    resource_model(
        aws_account_dto,
        sync_id,
        resource_kind,
        &resource_id,
        template.arn(),
        Some(resource_id.clone()),
        tags,
        Value::Object(resource_data),
        summary,
    )
}

fn launch_template_resource(
    aws_account_dto: &AwsAccountDto,
    sync_id: Uuid,
    template: &LaunchConfigurationTemplate,
    summary: &mut CollectionSummary,
) -> AwsResourceModel {
    let resource_kind = "launch_configuration_template";
    let resource_id = template
        .launch_configuration_template_id()
        .unwrap_or("unknown")
        .to_string();
    let tags = tags_from_map(template.tags());
    let mut resource_data = base_resource_data(resource_kind);

    insert_str(
        &mut resource_data,
        "launch_configuration_template_id",
        template.launch_configuration_template_id(),
    );
    if let Some(disposition) = template.launch_disposition() {
        resource_data.insert(
            "launch_disposition".to_string(),
            json!(disposition.as_str()),
        );
    }
    if let Some(method) = template.target_instance_type_right_sizing_method() {
        resource_data.insert(
            "target_instance_type_right_sizing_method".to_string(),
            json!(method.as_str()),
        );
    }
    insert_bool(
        &mut resource_data,
        "copy_private_ip",
        template.copy_private_ip(),
    );
    insert_bool(&mut resource_data, "copy_tags", template.copy_tags());
    insert_str(
        &mut resource_data,
        "export_bucket_arn",
        template.export_bucket_arn(),
    );
    insert_bool(
        &mut resource_data,
        "post_launch_enabled",
        template.post_launch_enabled(),
    );
    insert_bool(
        &mut resource_data,
        "launch_into_source_instance",
        template.launch_into_source_instance(),
    );

    resource_model(
        aws_account_dto,
        sync_id,
        resource_kind,
        &resource_id,
        template.arn(),
        Some(resource_id.clone()),
        tags,
        Value::Object(resource_data),
        summary,
    )
}

fn job_resource(
    aws_account_dto: &AwsAccountDto,
    sync_id: Uuid,
    job: &Job,
    summary: &mut CollectionSummary,
) -> AwsResourceModel {
    let resource_kind = "job";
    let resource_id = job.job_id().to_string();
    let tags = tags_from_map(job.tags());
    let mut resource_data = base_resource_data(resource_kind);

    resource_data.insert("job_id".to_string(), json!(job.job_id()));
    insert_str(
        &mut resource_data,
        "creation_date_time",
        job.creation_date_time(),
    );
    insert_str(&mut resource_data, "end_date_time", job.end_date_time());
    if let Some(job_type) = job.r#type() {
        resource_data.insert("job_type".to_string(), json!(job_type.as_str()));
    }
    if let Some(initiated_by) = job.initiated_by() {
        resource_data.insert("initiated_by".to_string(), json!(initiated_by.as_str()));
    }
    if let Some(status) = job.status() {
        let status = status.as_str();
        resource_data.insert("status".to_string(), json!(status));
        if status != "COMPLETED" {
            summary.failed_or_incomplete_job_count += 1;
        }
    }
    resource_data.insert(
        "participating_server_count".to_string(),
        json!(job.participating_servers().len()),
    );
    resource_data.insert(
        "participating_resource_count".to_string(),
        json!(job.participating_resources().len()),
    );

    resource_model(
        aws_account_dto,
        sync_id,
        resource_kind,
        &resource_id,
        job.arn(),
        Some(resource_id.clone()),
        tags,
        Value::Object(resource_data),
        summary,
    )
}

fn staging_account_resource(
    aws_account_dto: &AwsAccountDto,
    sync_id: Uuid,
    account: &Account,
    summary: &mut CollectionSummary,
) -> AwsResourceModel {
    let resource_kind = "staging_account";
    let resource_id = account.account_id().unwrap_or("unknown").to_string();
    let mut resource_data = base_resource_data(resource_kind);
    insert_str(&mut resource_data, "account_id", account.account_id());

    resource_model(
        aws_account_dto,
        sync_id,
        resource_kind,
        &resource_id,
        None,
        Some(resource_id.clone()),
        json!({}),
        Value::Object(resource_data),
        summary,
    )
}

fn account_summary_resource(
    aws_account_dto: &AwsAccountDto,
    sync_id: Uuid,
    summary: &CollectionSummary,
) -> AwsResourceModel {
    let mut resource_data = base_resource_data("account_summary");
    resource_data.insert("account_id".to_string(), json!(&aws_account_dto.account_id));
    resource_data.insert("resource_count".to_string(), json!(summary.resource_count));
    resource_data.insert(
        "resources_by_kind".to_string(),
        json!(summary.resources_by_kind),
    );
    resource_data.insert(
        "collection_error_count".to_string(),
        json!(summary.collection_error_count),
    );
    resource_data.insert(
        "collection_errors".to_string(),
        Value::Array(summary.collection_errors.clone()),
    );
    resource_data.insert(
        "untagged_resource_count".to_string(),
        json!(summary.untagged_resource_count),
    );
    resource_data.insert(
        "active_replication_source_count".to_string(),
        json!(summary.active_replication_source_count),
    );
    resource_data.insert(
        "replication_problem_count".to_string(),
        json!(summary.replication_problem_count),
    );
    resource_data.insert(
        "source_network_error_count".to_string(),
        json!(summary.source_network_error_count),
    );
    resource_data.insert(
        "failed_launch_count".to_string(),
        json!(summary.failed_launch_count),
    );
    resource_data.insert(
        "failed_or_incomplete_job_count".to_string(),
        json!(summary.failed_or_incomplete_job_count),
    );
    resource_data.insert(
        "public_replication_template_count".to_string(),
        json!(summary.public_replication_template_count),
    );
    resource_data.insert(
        "default_ebs_encryption_template_count".to_string(),
        json!(summary.default_ebs_encryption_template_count),
    );
    resource_data.insert(
        "staging_account_count".to_string(),
        json!(summary.staging_account_count),
    );

    let dto = AwsResourceDto {
        id: None,
        sync_id: Some(sync_id),
        account_id: aws_account_dto.account_id.clone(),
        profile: aws_account_dto.profile.clone(),
        region: aws_account_dto.default_region.clone(),
        resource_type: AwsResourceType::DrsResource.to_string(),
        resource_id: format!("drs:{}", aws_account_dto.account_id),
        arn: format!(
            "arn:aws:drs:{}:{}:account/{}",
            aws_account_dto.default_region, aws_account_dto.account_id, aws_account_dto.account_id
        ),
        name: Some("Elastic Disaster Recovery".to_string()),
        tags: json!({}),
        resource_data: Value::Object(resource_data),
    };

    dto.into()
}

fn resource_model(
    aws_account_dto: &AwsAccountDto,
    sync_id: Uuid,
    resource_kind: &str,
    resource_id: &str,
    arn: Option<&str>,
    name: Option<String>,
    tags: Value,
    resource_data: Value,
    summary: &mut CollectionSummary,
) -> AwsResourceModel {
    summary.resource_count += 1;
    *summary
        .resources_by_kind
        .entry(resource_kind.to_string())
        .or_insert(0) += 1;
    if tags.as_object().map(|tags| tags.is_empty()).unwrap_or(true) {
        summary.untagged_resource_count += 1;
    }

    let dto = AwsResourceDto {
        id: None,
        sync_id: Some(sync_id),
        account_id: aws_account_dto.account_id.clone(),
        profile: aws_account_dto.profile.clone(),
        region: aws_account_dto.default_region.clone(),
        resource_type: AwsResourceType::DrsResource.to_string(),
        resource_id: format!("{}/{}", resource_kind, resource_id),
        arn: arn.map(String::from).unwrap_or_else(|| {
            format!(
                "arn:aws:drs:{}:{}:{}/{}",
                aws_account_dto.default_region,
                aws_account_dto.account_id,
                resource_kind,
                resource_id
            )
        }),
        name,
        tags,
        resource_data,
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
        "Failed to list DRS {} inventory with {}: {}",
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
    let mut resource_data = Map::new();
    resource_data.insert("resource_kind".to_string(), json!(resource_kind));
    resource_data
}

fn tags_from_map(tags: Option<&HashMap<String, String>>) -> Value {
    let mut tag_map = Map::new();
    if let Some(tags) = tags {
        for (key, value) in tags {
            tag_map.insert(key.clone(), json!(value));
        }
    }
    Value::Object(tag_map)
}

fn insert_str(resource_data: &mut Map<String, Value>, key: &str, value: Option<&str>) {
    if let Some(value) = value {
        resource_data.insert(key.to_string(), json!(value));
    }
}

fn insert_bool(resource_data: &mut Map<String, Value>, key: &str, value: Option<bool>) {
    if let Some(value) = value {
        resource_data.insert(key.to_string(), json!(value));
    }
}

fn is_active_replication_state(state: &str) -> bool {
    matches!(
        state,
        "BACKLOG" | "CONTINUOUS" | "CREATING_SNAPSHOT" | "INITIAL_SYNC" | "INITIATING" | "RESCAN"
    )
}

fn is_replication_problem_state(state: &str) -> bool {
    matches!(state, "DISCONNECTED" | "PAUSED" | "STALLED" | "STOPPED")
}

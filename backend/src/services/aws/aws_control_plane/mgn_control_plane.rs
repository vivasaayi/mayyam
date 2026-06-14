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
use aws_sdk_mgn::types::{
    Application, Connector, LaunchConfigurationTemplate, ReplicationConfigurationTemplate,
    SourceServer, Wave,
};
use aws_sdk_mgn::Client as MgnClient;
use serde_json::{json, Map, Value};
use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;
use tracing::debug;
use uuid::Uuid;

pub struct MgnControlPlane {
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
    public_replication_template_count: usize,
    public_launch_template_count: usize,
    default_ebs_encryption_template_count: usize,
    parameters_encryption_disabled_count: usize,
}

impl MgnControlPlane {
    pub fn new(aws_service: Arc<AwsService>) -> Self {
        Self { aws_service }
    }

    pub async fn sync_resources(
        &self,
        aws_account_dto: &AwsAccountDto,
        sync_id: Uuid,
    ) -> Result<Vec<AwsResourceModel>, AppError> {
        debug!(
            "Syncing AWS Application Migration Service inventory for account: {} with sync_id: {}",
            &aws_account_dto.account_id, sync_id
        );

        let client = self.aws_service.create_mgn_client(aws_account_dto).await?;
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

        match list_applications(&client).await {
            Ok(items) => {
                for item in items {
                    resources.push(application_resource(
                        aws_account_dto,
                        sync_id,
                        &item,
                        &mut summary,
                    ));
                }
            }
            Err(e) => record_collection_error(&mut summary, "application", "ListApplications", e),
        }

        match list_waves(&client).await {
            Ok(items) => {
                for item in items {
                    resources.push(wave_resource(aws_account_dto, sync_id, &item, &mut summary));
                }
            }
            Err(e) => record_collection_error(&mut summary, "wave", "ListWaves", e),
        }

        match list_connectors(&client).await {
            Ok(items) => {
                for item in items {
                    resources.push(connector_resource(
                        aws_account_dto,
                        sync_id,
                        &item,
                        &mut summary,
                    ));
                }
            }
            Err(e) => record_collection_error(&mut summary, "connector", "ListConnectors", e),
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

        let summary_resource = account_summary_resource(aws_account_dto, sync_id, &summary);
        resources.insert(0, summary_resource);

        debug!(
            "Successfully synced {} AWS Application Migration Service inventory resources for account: {} with sync_id: {}",
            resources.len(),
            &aws_account_dto.account_id,
            sync_id
        );

        Ok(resources)
    }
}

async fn list_source_servers(client: &MgnClient) -> Result<Vec<SourceServer>, String> {
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

async fn list_applications(client: &MgnClient) -> Result<Vec<Application>, String> {
    let mut resources = Vec::new();
    let mut next_token: Option<String> = None;

    loop {
        let mut request = client.list_applications().max_results(100);
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

async fn list_waves(client: &MgnClient) -> Result<Vec<Wave>, String> {
    let mut resources = Vec::new();
    let mut next_token: Option<String> = None;

    loop {
        let mut request = client.list_waves().max_results(100);
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

async fn list_connectors(client: &MgnClient) -> Result<Vec<Connector>, String> {
    let mut resources = Vec::new();
    let mut next_token: Option<String> = None;

    loop {
        let mut request = client.list_connectors().max_results(100);
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
    client: &MgnClient,
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
    client: &MgnClient,
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

fn source_server_resource(
    aws_account_dto: &AwsAccountDto,
    sync_id: Uuid,
    server: &SourceServer,
    summary: &mut CollectionSummary,
) -> AwsResourceModel {
    let resource_kind = "source_server";
    let resource_id = server.source_server_id().unwrap_or("unknown").to_string();
    let name = server
        .user_provided_id()
        .or_else(|| server.fqdn_for_action_framework())
        .unwrap_or(&resource_id)
        .to_string();
    let tags = tags_from_map(server.tags());
    let mut resource_data = base_resource_data(resource_kind);

    insert_str(
        &mut resource_data,
        "source_server_id",
        server.source_server_id(),
    );
    insert_bool(&mut resource_data, "is_archived", server.is_archived());
    insert_str(
        &mut resource_data,
        "application_id",
        server.application_id(),
    );
    insert_str(
        &mut resource_data,
        "user_provided_id",
        server.user_provided_id(),
    );
    insert_str(
        &mut resource_data,
        "fqdn_for_action_framework",
        server.fqdn_for_action_framework(),
    );
    insert_str(
        &mut resource_data,
        "vcenter_client_id",
        server.vcenter_client_id(),
    );
    if let Some(replication_type) = server.replication_type() {
        resource_data.insert(
            "replication_type".to_string(),
            json!(replication_type.as_str()),
        );
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
            "last_snapshot_date_time",
            replication_info.last_snapshot_date_time(),
        );
        insert_str(
            &mut resource_data,
            "replicator_id",
            replication_info.replicator_id(),
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
            "last_seen_by_service_date_time",
            life_cycle.last_seen_by_service_date_time(),
        );
        insert_str(
            &mut resource_data,
            "elapsed_replication_duration",
            life_cycle.elapsed_replication_duration(),
        );
        if let Some(state) = life_cycle.state() {
            resource_data.insert("life_cycle_state".to_string(), json!(state.as_str()));
        }
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
    if let Some(launched_instance) = server.launched_instance() {
        insert_str(
            &mut resource_data,
            "launched_ec2_instance_id",
            launched_instance.ec2_instance_id(),
        );
        insert_str(
            &mut resource_data,
            "launched_job_id",
            launched_instance.job_id(),
        );
    }

    resource_model(
        aws_account_dto,
        sync_id,
        resource_kind,
        &resource_id,
        server.arn(),
        Some(name),
        tags,
        Value::Object(resource_data),
        summary,
    )
}

fn application_resource(
    aws_account_dto: &AwsAccountDto,
    sync_id: Uuid,
    application: &Application,
    summary: &mut CollectionSummary,
) -> AwsResourceModel {
    let resource_kind = "application";
    let resource_id = application
        .application_id()
        .unwrap_or("unknown")
        .to_string();
    let tags = tags_from_map(application.tags());
    let mut resource_data = base_resource_data(resource_kind);

    insert_str(
        &mut resource_data,
        "application_id",
        application.application_id(),
    );
    insert_str(&mut resource_data, "name", application.name());
    insert_bool(&mut resource_data, "is_archived", application.is_archived());
    insert_str(&mut resource_data, "wave_id", application.wave_id());
    insert_str(
        &mut resource_data,
        "creation_date_time",
        application.creation_date_time(),
    );
    insert_str(
        &mut resource_data,
        "last_modified_date_time",
        application.last_modified_date_time(),
    );
    if let Some(status) = application.application_aggregated_status() {
        insert_str(
            &mut resource_data,
            "aggregated_status_last_update_date_time",
            status.last_update_date_time(),
        );
        if let Some(health) = status.health_status() {
            resource_data.insert("health_status".to_string(), json!(health.as_str()));
        }
        if let Some(progress) = status.progress_status() {
            resource_data.insert("progress_status".to_string(), json!(progress.as_str()));
        }
        resource_data.insert(
            "total_source_servers".to_string(),
            json!(status.total_source_servers()),
        );
    }

    resource_model(
        aws_account_dto,
        sync_id,
        resource_kind,
        &resource_id,
        application.arn(),
        application.name().map(String::from),
        tags,
        Value::Object(resource_data),
        summary,
    )
}

fn wave_resource(
    aws_account_dto: &AwsAccountDto,
    sync_id: Uuid,
    wave: &Wave,
    summary: &mut CollectionSummary,
) -> AwsResourceModel {
    let resource_kind = "wave";
    let resource_id = wave.wave_id().unwrap_or("unknown").to_string();
    let tags = tags_from_map(wave.tags());
    let mut resource_data = base_resource_data(resource_kind);

    insert_str(&mut resource_data, "wave_id", wave.wave_id());
    insert_str(&mut resource_data, "name", wave.name());
    insert_bool(&mut resource_data, "is_archived", wave.is_archived());
    insert_str(
        &mut resource_data,
        "creation_date_time",
        wave.creation_date_time(),
    );
    insert_str(
        &mut resource_data,
        "last_modified_date_time",
        wave.last_modified_date_time(),
    );
    if let Some(status) = wave.wave_aggregated_status() {
        insert_str(
            &mut resource_data,
            "aggregated_status_last_update_date_time",
            status.last_update_date_time(),
        );
        insert_str(
            &mut resource_data,
            "replication_started_date_time",
            status.replication_started_date_time(),
        );
        if let Some(health) = status.health_status() {
            resource_data.insert("health_status".to_string(), json!(health.as_str()));
        }
        if let Some(progress) = status.progress_status() {
            resource_data.insert("progress_status".to_string(), json!(progress.as_str()));
        }
        resource_data.insert(
            "total_applications".to_string(),
            json!(status.total_applications()),
        );
    }

    resource_model(
        aws_account_dto,
        sync_id,
        resource_kind,
        &resource_id,
        wave.arn(),
        wave.name().map(String::from),
        tags,
        Value::Object(resource_data),
        summary,
    )
}

fn connector_resource(
    aws_account_dto: &AwsAccountDto,
    sync_id: Uuid,
    connector: &Connector,
    summary: &mut CollectionSummary,
) -> AwsResourceModel {
    let resource_kind = "connector";
    let resource_id = connector.connector_id().unwrap_or("unknown").to_string();
    let tags = tags_from_map(connector.tags());
    let mut resource_data = base_resource_data(resource_kind);

    insert_str(&mut resource_data, "connector_id", connector.connector_id());
    insert_str(&mut resource_data, "name", connector.name());
    insert_str(
        &mut resource_data,
        "ssm_instance_id",
        connector.ssm_instance_id(),
    );
    resource_data.insert(
        "has_ssm_command_config".to_string(),
        json!(connector.ssm_command_config().is_some()),
    );

    resource_model(
        aws_account_dto,
        sync_id,
        resource_kind,
        &resource_id,
        connector.arn(),
        connector.name().map(String::from),
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
        "use_fips_endpoint",
        template.use_fips_endpoint(),
    );
    if let Some(internet_protocol) = template.internet_protocol() {
        resource_data.insert(
            "internet_protocol".to_string(),
            json!(internet_protocol.as_str()),
        );
    }
    insert_bool(
        &mut resource_data,
        "store_snapshot_on_local_zone",
        template.store_snapshot_on_local_zone(),
    );
    resource_data.insert(
        "replication_server_security_group_count".to_string(),
        json!(template.replication_servers_security_groups_ids().len()),
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
    let resource_id = template.launch_configuration_template_id().to_string();
    let tags = tags_from_map(template.tags());
    let mut resource_data = base_resource_data(resource_kind);

    resource_data.insert(
        "launch_configuration_template_id".to_string(),
        json!(template.launch_configuration_template_id()),
    );
    insert_str(
        &mut resource_data,
        "ec2_launch_template_id",
        template.ec2_launch_template_id(),
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
    insert_bool(
        &mut resource_data,
        "associate_public_ip_address",
        template.associate_public_ip_address(),
    );
    if template.associate_public_ip_address() == Some(true) {
        summary.public_launch_template_count += 1;
    }
    insert_bool(&mut resource_data, "copy_tags", template.copy_tags());
    if let Some(boot_mode) = template.boot_mode() {
        resource_data.insert("boot_mode".to_string(), json!(boot_mode.as_str()));
    }
    resource_data.insert(
        "small_volume_max_size".to_string(),
        json!(template.small_volume_max_size()),
    );
    insert_bool(
        &mut resource_data,
        "enable_parameters_encryption",
        template.enable_parameters_encryption(),
    );
    if template.enable_parameters_encryption() == Some(false) {
        summary.parameters_encryption_disabled_count += 1;
    }
    insert_str(
        &mut resource_data,
        "parameters_encryption_key",
        template.parameters_encryption_key(),
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
        "public_replication_template_count".to_string(),
        json!(summary.public_replication_template_count),
    );
    resource_data.insert(
        "public_launch_template_count".to_string(),
        json!(summary.public_launch_template_count),
    );
    resource_data.insert(
        "default_ebs_encryption_template_count".to_string(),
        json!(summary.default_ebs_encryption_template_count),
    );
    resource_data.insert(
        "parameters_encryption_disabled_count".to_string(),
        json!(summary.parameters_encryption_disabled_count),
    );

    let dto = AwsResourceDto {
        id: None,
        sync_id: Some(sync_id),
        account_id: aws_account_dto.account_id.clone(),
        profile: aws_account_dto.profile.clone(),
        region: aws_account_dto.default_region.clone(),
        resource_type: AwsResourceType::MgnResource.to_string(),
        resource_id: format!("mgn:{}", aws_account_dto.account_id),
        arn: format!(
            "arn:aws:mgn:{}:{}:account/{}",
            aws_account_dto.default_region, aws_account_dto.account_id, aws_account_dto.account_id
        ),
        name: Some("Application Migration Service".to_string()),
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
        resource_type: AwsResourceType::MgnResource.to_string(),
        resource_id: format!("{}/{}", resource_kind, resource_id),
        arn: arn.map(String::from).unwrap_or_else(|| {
            format!(
                "arn:aws:mgn:{}:{}:{}/{}",
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
        "Failed to list MGN {} inventory with {}: {}",
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
        "BACKLOG"
            | "CONTINUOUS"
            | "CREATING_SNAPSHOT"
            | "INITIAL_SYNC"
            | "PENDING_SNAPSHOT_SHIPPING"
            | "RESCAN"
            | "SHIPPING_SNAPSHOT"
    )
}

fn is_replication_problem_state(state: &str) -> bool {
    matches!(state, "DISCONNECTED" | "PAUSED" | "STALLED" | "STOPPED")
}

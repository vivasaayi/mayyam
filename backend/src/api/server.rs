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


use actix_cors::Cors;
use actix_web::{middleware::Logger, web, App, HttpServer};
use std::error::Error;
use std::sync::Arc;
use tracing::info;

use crate::api::routes;
use crate::config::Config;
use crate::controllers::sync_run::SyncRunController;
use crate::controllers::{
    auth::AuthController, aws_analytics::AwsAnalyticsController, data_source::DataSourceController,
    kubernetes_cluster_management::KubernetesClusterManagementController,
    llm_analytics::LlmAnalyticsController, llm_model::LlmModelController,
    llm_provider::LlmProviderController, prompt_template::PromptTemplateController,
};
use crate::middleware::auth::AuthMiddleware;
use crate::models::aws_account::AwsAccountDto;
use crate::repositories::{
    aws_account::AwsAccountRepository, aws_resource::AwsResourceRepository,
    cloud_resource::CloudResourceRepository, cluster::ClusterRepository,
    cost_analytics::CostAnalyticsRepository, data_source::DataSourceRepository,
    database::DatabaseRepository, llm_provider::LlmProviderRepository,
    prompt_template::PromptTemplateRepository, user::UserRepository,
};
use crate::services::analytics::aws_analytics::aws_analytics::AwsAnalyticsService;
use crate::services::aws::aws_control_plane::dynamodb_control_plane::DynamoDbControlPlane;
use crate::services::aws::aws_control_plane::kinesis_control_plane::KinesisControlPlane;
use crate::services::aws::aws_control_plane::s3_control_plane;
use crate::services::aws::aws_control_plane::sqs_control_plane::SqsControlPlane;
use crate::services::aws::aws_data_plane::cloudwatch::CloudWatchService;
use crate::services::aws::aws_data_plane::dynamodb_data_plane::DynamoDBDataPlane;
use crate::services::aws::aws_data_plane::kinesis_data_plane::KinesisDataPlane;
use crate::services::aws::aws_data_plane::s3_data_plane::S3DataPlane;
use crate::services::aws::aws_data_plane::sqs_data_plane::SqsDataPlane;
use crate::services::{
    aws::{AwsControlPlane, AwsCostService, AwsDataPlane, AwsService},
    aws_account::AwsAccountService,
    aws_cost_analytics::AwsCostAnalyticsService,
    data_collection::DataCollectionService,
    kafka::KafkaService,
    llm::{LlmAnalyticsService, LlmIntegrationService},
    llm_provider::LlmProviderService,
    user::UserService,
};
use crate::utils::database;

// Import Kubernetes Services
use crate::services::kubernetes::authz_service::AuthorizationService;
use crate::services::kubernetes::cronjobs_service::CronJobsService;
use crate::services::kubernetes::endpoints_service::EndpointsService;
use crate::services::kubernetes::hpa_service::HorizontalPodAutoscalerService;
use crate::services::kubernetes::ingress_service::IngressService;
use crate::services::kubernetes::jobs_service::JobsService;
use crate::services::kubernetes::limit_ranges_service::LimitRangesService;
use crate::services::kubernetes::metrics_service::MetricsService;
use crate::services::kubernetes::network_policies_service::NetworkPoliciesService;
use crate::services::kubernetes::nodes_ops_service::NodeOpsService;
use crate::services::kubernetes::pdb_service::PodDisruptionBudgetsService;
use crate::services::kubernetes::rbac_service::RbacService;
use crate::services::kubernetes::resource_quotas_service::ResourceQuotasService;
use crate::services::kubernetes::service_accounts_service::ServiceAccountsService;
use crate::services::kubernetes::{
    daemon_sets::DaemonSetsService,
    deployments_service::DeploymentsService,
    namespaces_service::NamespacesService,
    nodes_service::NodesService,
    persistent_volume_claims_service::PersistentVolumeClaimsService,
    persistent_volumes_service::PersistentVolumesService,
    pod::PodService,                                         // Changed from pods
    services_service::ServicesService as K8sServicesService, // Alias to avoid conflict with general 'Service'
    stateful_sets_service::StatefulSetsService,
};

pub async fn run_server(host: String, port: u16, config: Config) -> Result<(), Box<dyn Error>> {
    let addr = format!("{}:{}", host, port);

    info!("Starting Mayyam server on http://{}", addr);

    // Connect to the database
    let db_connection_val = database::connect(&config).await?;
    // Ensure critical tables exist in case migrations weren't applied
    // Parent tables first (referenced by foreign keys)
    if let Err(e) = database::ensure_llm_providers_table(&db_connection_val).await {
        tracing::warn!("Failed to ensure llm_providers table exists: {}", e);
    }
    if let Err(e) = database::ensure_aws_resources_table(&db_connection_val).await {
        tracing::warn!("Failed to ensure aws_resources table exists: {}", e);
    }
    if let Err(e) = database::ensure_aws_accounts_table(&db_connection_val).await {
        tracing::warn!("Failed to ensure aws_accounts table exists: {}", e);
    }
    // Child tables (with foreign key references)
    if let Err(e) = database::ensure_llm_provider_models_table(&db_connection_val).await {
        tracing::warn!("Failed to ensure llm_provider_models table exists: {}", e);
    }
    if let Err(e) = database::ensure_sync_runs_table(&db_connection_val).await {
        tracing::warn!("Failed to ensure sync_runs table exists: {}", e);
    }
    if let Err(e) = database::ensure_aws_resources_table(&db_connection_val).await {
        tracing::warn!("Failed to ensure aws_resources table exists: {}", e);
    }
    let db_connection = Arc::new(db_connection_val);

    // Initialize repositories
    let user_repo = Arc::new(UserRepository::new(db_connection.clone()));
    let aws_account_repo = Arc::new(AwsAccountRepository::new(db_connection.clone()));
    let database_repo = Arc::new(DatabaseRepository::new(
        db_connection.clone(),
        config.clone(),
    ));
    let cluster_repo = Arc::new(ClusterRepository::new(
        db_connection.clone(),
        config.clone(),
    ));
    let aws_resource_repo = Arc::new(AwsResourceRepository::new(
        db_connection.clone(),
        config.clone(),
    ));
    let cloud_resource_repo = Arc::new(CloudResourceRepository::new(db_connection.clone()));
    let sync_run_repo = Arc::new(crate::repositories::sync_run::SyncRunRepository::new(
        db_connection.clone(),
    ));
    let data_source_repo = Arc::new(DataSourceRepository::new(db_connection.clone()));
    let llm_provider_repo = Arc::new(LlmProviderRepository::new(
        db_connection.clone(),
        config.clone(),
    ));
    let prompt_template_repo = Arc::new(PromptTemplateRepository::new((*db_connection).clone()));
    let llm_provider_model_repo = Arc::new(
        crate::repositories::llm_model::LlmProviderModelRepository::new(db_connection.clone()),
    );
    let cost_analytics_repo = Arc::new(CostAnalyticsRepository::new(db_connection.clone()));
    let cost_budget_repo = Arc::new(crate::repositories::cost_budget_repository::CostBudgetRepository::new((*db_connection).clone()));
    let llm_provider_service = Arc::new(LlmProviderService::new(llm_provider_repo.clone()));

    // Initialize services
    let user_service = Arc::new(UserService::new(user_repo.clone()));
    let kafka_service = Arc::new(KafkaService::new(cluster_repo.clone()));

    // AWS services
    let aws_service = Arc::new(AwsService::new(
        aws_resource_repo.clone(),
        cloud_resource_repo.clone(),
        config.clone(),
    ));
    let aws_control_plane = Arc::new(AwsControlPlane::new(aws_service.clone()));
    let aws_data_plane = Arc::new(AwsDataPlane::new(aws_service.clone()));
    let aws_cost_service = Arc::new(AwsCostService::new(aws_service.clone()));
    let cloudwatch_service = Arc::new(CloudWatchService::new(aws_service.clone()));
    let aws_account_service = Arc::new(AwsAccountService::new(
        aws_account_repo.clone(),
        aws_control_plane.clone(),
        sync_run_repo.clone(),
    ));

    // Initialize LLM Integration service first (needed by AWS analytics)
    let llm_integration_service = Arc::new(LlmIntegrationService::new(
        llm_provider_repo.clone(),
        prompt_template_repo.clone(),
    ));

    // AWS Analytics service with real LLM integration
    let aws_analytics_service = Arc::new(AwsAnalyticsService::new(
        config.clone(),
        aws_service.clone(),
        aws_data_plane.clone(),
        aws_resource_repo.clone(),
        llm_integration_service.clone(),
        cloudwatch_service.clone(),
    ));

    // New LLM Analytics platform services
    let data_collection_service = Arc::new(DataCollectionService::new(
        data_source_repo.clone(),
        None, // CloudWatch client - will be initialized when needed
        config.clone(),
    ));
    // Initialize Unified LLM Manager
    let mut llm_manager_init =
        crate::services::llm::UnifiedLlmManager::new(llm_provider_repo.clone());
    llm_manager_init.initialize_common_providers().await?;
    let unified_llm_manager = Arc::new(llm_manager_init);

    let llm_analytics_service = Arc::new(LlmAnalyticsService::new(
        unified_llm_manager.clone(),
        data_collection_service.clone(),
        data_source_repo.clone(),
        llm_provider_repo.clone(),
        prompt_template_repo.clone(),
    ));

    // AWS Cost Analytics service
    let aws_cost_analytics_service = {
        Arc::new(AwsCostAnalyticsService::new(
            cost_analytics_repo.clone(),
            aws_account_repo.clone(),
            aws_resource_repo.clone(),
            aws_service.clone(),
            llm_integration_service.clone(),
            llm_provider_repo.clone(),
            db_connection.clone(),
        ))
    };

    // Initialize Kubernetes Services
    let deployments_service = Arc::new(DeploymentsService::new());
    let stateful_sets_service = Arc::new(StatefulSetsService::new());
    let daemon_sets_service = Arc::new(DaemonSetsService::new());
    let pod_service = Arc::new(PodService::new());
    let k8s_services_service = Arc::new(K8sServicesService::new());
    let nodes_service = Arc::new(NodesService::new());
    let namespaces_service = Arc::new(NamespacesService::new());
    let persistent_volume_claims_service = Arc::new(PersistentVolumeClaimsService::new());
    let persistent_volumes_service = Arc::new(PersistentVolumesService::new());
    let configmaps_service =
        Arc::new(crate::services::kubernetes::configmaps_service::ConfigMapsService::new());
    let secrets_service =
        Arc::new(crate::services::kubernetes::secrets_service::SecretsService::new());
    let metrics_service = Arc::new(MetricsService::new());
    let jobs_service = Arc::new(JobsService::new());
    let cronjobs_service = Arc::new(CronJobsService::new());
    let ingress_service = Arc::new(IngressService::new());
    let endpoints_service = Arc::new(EndpointsService::new());
    let network_policies_service = Arc::new(NetworkPoliciesService::new());
    let hpa_service = Arc::new(HorizontalPodAutoscalerService::new());
    let pdb_service = Arc::new(PodDisruptionBudgetsService::new());
    let resource_quotas_service = Arc::new(ResourceQuotasService::new());
    let limit_ranges_service = Arc::new(LimitRangesService::new());
    let service_accounts_service = Arc::new(ServiceAccountsService::new());
    let rbac_service = Arc::new(RbacService::new());
    let authorization_service = Arc::new(AuthorizationService::new());
    let node_ops_service = Arc::new(NodeOpsService::new());

    // Initialize controllers
    let auth_controller = Arc::new(AuthController::new(user_service.clone(), config.clone()));
    let aws_analytics_controller =
        Arc::new(AwsAnalyticsController::new(aws_analytics_service.clone()));

    let kubernetes_cluster_management_controller = Arc::new(
        KubernetesClusterManagementController::new(cluster_repo.clone()),
    );

    // New LLM Analytics platform controllers
    let data_source_controller = Arc::new(DataSourceController::new(
        data_source_repo.clone(),
        data_collection_service.clone(),
    ));
    let llm_provider_controller =
        Arc::new(LlmProviderController::new(llm_provider_service.clone()));
    let prompt_template_controller =
        Arc::new(PromptTemplateController::new(prompt_template_repo.clone()));
    let llm_analytics_controller =
        Arc::new(LlmAnalyticsController::new(llm_analytics_service.clone()));
    let unified_llm_controller = Arc::new(
        crate::controllers::unified_llm::UnifiedLlmController::new(unified_llm_manager.clone()),
    );
    let sync_run_controller = Arc::new(SyncRunController::new(sync_run_repo.clone()));

    let s3_data_plane = Arc::new(S3DataPlane::new(aws_service.clone()));
    let s3_control_plane = Arc::new(s3_control_plane::S3ControlPlane::new(aws_service.clone()));

    let dynamodb_data_plane = Arc::new(DynamoDBDataPlane::new(aws_service.clone()));
    let dynamodb_control_plane = Arc::new(DynamoDbControlPlane::new(aws_service.clone()));

    let sqs_data_plane = Arc::new(SqsDataPlane::new(aws_service.clone()));
    let sqs_control_plane = Arc::new(SqsControlPlane::new(aws_service.clone()));

    let kinesis_data_plane = Arc::new(KinesisDataPlane::new(aws_service.clone()));
    let kinesis_control_plane = Arc::new(KinesisControlPlane::new(aws_service.clone()));

    // Create and start the HTTP server
    HttpServer::new(move || {
        let cors = Cors::default()
            .allow_any_origin()
            .allow_any_method()
            .allow_any_header()
            .supports_credentials()
            .max_age(3600);

        info!("Configuring CORS with credentials support");

        App::new()
            .wrap(cors)
            .wrap(Logger::default())
            .wrap(AuthMiddleware::new(&config))
            // Global JSON config: limit large payloads (256KB)
            .app_data(web::JsonConfig::default().limit(256 * 1024))
            .app_data(web::Data::new(db_connection.clone())) // Now correctly Data<Arc<DatabaseConnection>>
            .app_data(web::Data::new(config.clone()))
            // Repositories
            .app_data(web::Data::new(user_repo.clone()))
            .app_data(web::Data::new(database_repo.clone()))
            .app_data(web::Data::new(cluster_repo.clone()))
            .app_data(web::Data::new(aws_resource_repo.clone()))
            .app_data(web::Data::new(cloud_resource_repo.clone()))
            .app_data(web::Data::new(aws_account_repo.clone()))
            .app_data(web::Data::new(data_source_repo.clone()))
            .app_data(web::Data::new(llm_provider_repo.clone()))
            .app_data(web::Data::new(prompt_template_repo.clone()))
            .app_data(web::Data::new(cost_analytics_repo.clone()))
            // Services
            .app_data(web::Data::new(user_service.clone()))
            .app_data(web::Data::new(kafka_service.clone()))
            .app_data(web::Data::new(aws_service.clone()))
            .app_data(web::Data::new(aws_control_plane.clone()))
            .app_data(web::Data::new(aws_data_plane.clone()))
            .app_data(web::Data::new(aws_cost_service.clone()))
            .app_data(web::Data::new(cloudwatch_service.clone()))
            .app_data(web::Data::new(aws_account_service.clone()))
            .app_data(web::Data::new(aws_analytics_service.clone()))
            .app_data(web::Data::new(llm_integration_service.clone()))
            .app_data(web::Data::new(llm_provider_service.clone()))
            .app_data(web::Data::new(data_collection_service.clone()))
            .app_data(web::Data::new(llm_analytics_service.clone()))
            .app_data(web::Data::new(unified_llm_manager.clone()))
            .app_data(web::Data::new(aws_cost_analytics_service.clone()))
            // Kubernetes Services
            .app_data(web::Data::new(deployments_service.clone()))
            .app_data(web::Data::new(stateful_sets_service.clone()))
            .app_data(web::Data::new(daemon_sets_service.clone()))
            .app_data(web::Data::new(pod_service.clone()))
            .app_data(web::Data::new(k8s_services_service.clone()))
            .app_data(web::Data::new(nodes_service.clone()))
            .app_data(web::Data::new(namespaces_service.clone()))
            .app_data(web::Data::new(persistent_volume_claims_service.clone()))
            .app_data(web::Data::new(persistent_volumes_service.clone()))
            .app_data(web::Data::new(configmaps_service.clone()))
            .app_data(web::Data::new(secrets_service.clone()))
            .app_data(web::Data::new(metrics_service.clone()))
            .app_data(web::Data::new(jobs_service.clone()))
            .app_data(web::Data::new(cronjobs_service.clone()))
            .app_data(web::Data::new(ingress_service.clone()))
            .app_data(web::Data::new(endpoints_service.clone()))
            .app_data(web::Data::new(network_policies_service.clone()))
            .app_data(web::Data::new(hpa_service.clone()))
            .app_data(web::Data::new(pdb_service.clone()))
            .app_data(web::Data::new(resource_quotas_service.clone()))
            .app_data(web::Data::new(limit_ranges_service.clone()))
            .app_data(web::Data::new(service_accounts_service.clone()))
            .app_data(web::Data::new(rbac_service.clone()))
            .app_data(web::Data::new(authorization_service.clone()))
            .app_data(web::Data::new(node_ops_service.clone()))
            // Controllers
            .app_data(web::Data::new(auth_controller.clone()))
            .app_data(web::Data::new(aws_analytics_controller.clone()))
            .app_data(web::Data::new(
                kubernetes_cluster_management_controller.clone(),
            ))
            .app_data(web::Data::new(data_source_controller.clone()))
            .app_data(web::Data::new(llm_provider_controller.clone()))
            .app_data(web::Data::new(Arc::new(LlmModelController::new(
                llm_provider_model_repo.clone(),
            ))))
            .app_data(web::Data::new(prompt_template_controller.clone()))
            .app_data(web::Data::new(llm_analytics_controller.clone()))
            .app_data(web::Data::new(unified_llm_controller.clone()))
            .app_data(web::Data::new(sync_run_controller.clone()))
            .app_data(web::Data::new(s3_data_plane.clone()))
            .app_data(web::Data::new(s3_control_plane.clone()))
            .app_data(web::Data::new(dynamodb_data_plane.clone()))
            .app_data(web::Data::new(dynamodb_control_plane.clone()))
            .app_data(web::Data::new(sqs_data_plane.clone()))
            .app_data(web::Data::new(sqs_control_plane.clone()))
            .app_data(web::Data::new(kinesis_data_plane.clone()))
            .app_data(web::Data::new(kinesis_control_plane.clone()))
            // Middleware
            // Routes configuration - specify the order: analytics first, then general routes
            .configure(|cfg_param: &mut web::ServiceConfig| {
                // Explicit type for cfg_param
                info!("Registering route handlers in server.rs");

                info!("Registering AWS analytics routes with highest priority");
                routes::aws_analytics::configure(cfg_param, aws_analytics_controller.clone());

                info!("Registering AI routes");
                routes::ai::configure(cfg_param);

                info!("Registering Kubernetes cluster management routes");
                routes::kubernetes_cluster_management::configure(
                    cfg_param,
                    kubernetes_cluster_management_controller.clone(),
                );

                info!("Registering LLM Analytics Platform routes");
                routes::data_source::configure(cfg_param, data_source_controller.clone());
                routes::llm_provider::configure(
                    cfg_param,
                    llm_provider_controller.clone(),
                    Arc::new(LlmModelController::new(llm_provider_model_repo.clone())),
                );
                routes::prompt_template::configure(cfg_param, prompt_template_controller.clone());
                routes::query_template::configure(cfg_param);
                routes::llm_analytics::configure(cfg_param, llm_analytics_controller.clone());
                routes::unified_llm::configure(cfg_param, unified_llm_controller.clone());
                // Sync runs API
                cfg_param.service(crate::api::routes::sync_run::configure(
                    sync_run_controller.clone(),
                ));

                info!("Registering AWS Cost Analytics routes");
                routes::cost_analytics::configure_routes(
                    cfg_param,
                    aws_account_repo.clone(),
                    aws_resource_repo.clone(),
                );

                info!("Registering Budget Management routes");
                routes::budget::configure_routes(
                    cfg_param,
                    cost_budget_repo.clone(),
                );

                info!("Registering other general routes");
                // Pass Arc<DatabaseConnection> to the general routes::configure function
                routes::configure(cfg_param, db_connection.clone());
            })
            .service(web::resource("/health").to(|| async { "Mayyam API is running!" }))
    })
    .bind(addr)?
    .run()
    .await?;

    Ok(())
}

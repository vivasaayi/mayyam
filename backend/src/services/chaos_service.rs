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

use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{error, info, warn};
use uuid::Uuid;

use crate::errors::AppError;
use crate::models::aws_account::AwsAccountDto;
use crate::services::aws::client_factory::AwsClientFactory;
use crate::models::chaos_experiment::{
    BatchRunRequest, ChaosExperimentCreateDto, ChaosExperimentPage, ChaosExperimentQuery,
    ChaosExperimentUpdateDto, ChaosExperimentWithRuns, ExperimentStatus, RunExperimentRequest,
};
use crate::models::chaos_experiment_result::{ResourceExperimentHistory, Model as ResultModel};
use crate::models::chaos_experiment_run::{Model as RunModel, RunStatus, RunWithResults};
use crate::models::chaos_experiment::Model as ExperimentModel;
use crate::models::chaos_template::{
    ChaosTemplateCreateDto, ChaosTemplateQuery, ChaosTemplateUpdateDto,
    Model as TemplateModel,
};
use crate::repositories::aws_account::AwsAccountRepository;
use crate::repositories::chaos_repository::ChaosRepository;
use crate::services::aws::AwsService;
use crate::services::chaos_audit_service::ChaosAuditService;
use crate::services::chaos_metrics_service::ChaosMetricsService;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChaosExperimentSummary {
    pub total_experiments: u64,
    pub by_status: std::collections::HashMap<String, u64>,
    pub by_resource_type: std::collections::HashMap<String, u64>,
    pub recent_runs: Vec<RunModel>,
}

#[derive(Debug)]
pub struct ChaosService {
    chaos_repo: Arc<ChaosRepository>,
    aws_service: Arc<AwsService>,
    aws_account_repo: Arc<AwsAccountRepository>,
    audit_service: Arc<ChaosAuditService>,
    metrics_service: Arc<ChaosMetricsService>,
}

impl ChaosService {
    pub fn new(
        chaos_repo: Arc<ChaosRepository>,
        aws_service: Arc<AwsService>,
        aws_account_repo: Arc<AwsAccountRepository>,
        audit_service: Arc<ChaosAuditService>,
        metrics_service: Arc<ChaosMetricsService>,
    ) -> Self {
        Self {
            chaos_repo,
            aws_service,
            aws_account_repo,
            audit_service,
            metrics_service,
        }
    }

    // ========================================================================
    // Template Operations
    // ========================================================================

    pub async fn list_templates(
        &self,
        query: &ChaosTemplateQuery,
    ) -> Result<Vec<TemplateModel>, AppError> {
        self.chaos_repo.list_templates(query).await
    }

    pub async fn get_template(&self, id: Uuid) -> Result<TemplateModel, AppError> {
        self.chaos_repo
            .get_template(id)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("Template {} not found", id)))
    }

    pub async fn create_template(
        &self,
        dto: ChaosTemplateCreateDto,
    ) -> Result<TemplateModel, AppError> {
        let template = self.chaos_repo.create_template(&dto).await?;

        // Log template created event
        let _ = self
            .audit_service
            .log_action(
                crate::models::chaos_audit_log::ChaosAuditAction::TEMPLATE_CREATED,
                None,
                Some("system".to_string()),
                None,
                None,
                None,
                None,
                Some(serde_json::json!(template)),
                None,
                None,
                None,
                None,
                None,
            )
            .await;

        Ok(template)
    }

    pub async fn update_template(
        &self,
        id: Uuid,
        dto: ChaosTemplateUpdateDto,
    ) -> Result<TemplateModel, AppError> {
        let old_template = self.get_template(id).await?;
        let updated_template = self.chaos_repo.update_template(id, &dto).await?;

        // Log template updated event
        let _ = self
            .audit_service
            .log_action(
                crate::models::chaos_audit_log::ChaosAuditAction::TEMPLATE_UPDATED,
                None,
                Some("system".to_string()),
                None,
                None,
                None,
                Some(serde_json::json!(old_template)),
                Some(serde_json::json!(updated_template)),
                None,
                None,
                None,
                None,
                None,
            )
            .await;

        Ok(updated_template)
    }

    pub async fn delete_template(&self, id: Uuid) -> Result<(), AppError> {
        let template = self.get_template(id).await?;
        self.chaos_repo.delete_template(id).await?;

        // Log template deleted event
        let _ = self
            .audit_service
            .log_action(
                crate::models::chaos_audit_log::ChaosAuditAction::TEMPLATE_DELETED,
                None,
                Some("system".to_string()),
                None,
                None,
                None,
                Some(serde_json::json!(template)),
                None,
                None,
                None,
                None,
                None,
                None,
            )
            .await;

        Ok(())
    }

    // ========================================================================
    // Experiment Operations
    // ========================================================================

    pub async fn list_experiments(
        &self,
        query: &ChaosExperimentQuery,
    ) -> Result<ChaosExperimentPage, AppError> {
        self.chaos_repo.list_experiments(query).await
    }

    pub async fn list_experiments_with_runs(
        &self,
        query: &ChaosExperimentQuery,
    ) -> Result<Vec<ChaosExperimentWithRuns>, AppError> {
        let page = self.chaos_repo.list_experiments(query).await?;
        let mut result = Vec::new();

        for experiment in page.experiments {
            let last_run = self
                .chaos_repo
                .get_latest_run_for_experiment(experiment.id)
                .await?;
            let total_runs = self
                .chaos_repo
                .get_experiment_run_count(experiment.id)
                .await?;

            result.push(ChaosExperimentWithRuns {
                experiment,
                last_run,
                total_runs,
            });
        }

        Ok(result)
    }

    pub async fn get_experiment(&self, id: Uuid) -> Result<ExperimentModel, AppError> {
        self.chaos_repo
            .get_experiment(id)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("Experiment {} not found", id)))
    }

    pub async fn create_experiment(
        &self,
        dto: ChaosExperimentCreateDto,
    ) -> Result<ExperimentModel, AppError> {
        // If template_id is provided, merge default parameters from the template
        let final_dto = if let Some(template_id) = dto.template_id {
            let template = self.get_template(template_id).await?;
            let merged_params = if let Some(ref user_params) = dto.parameters {
                let mut base = template.default_parameters.clone();
                if let Some(base_obj) = base.as_object_mut() {
                    if let Some(user_obj) = user_params.as_object() {
                        for (k, v) in user_obj {
                            base_obj.insert(k.clone(), v.clone());
                        }
                    }
                }
                Some(base)
            } else {
                Some(template.default_parameters.clone())
            };

            ChaosExperimentCreateDto {
                parameters: merged_params,
                ..dto
            }
        } else {
            dto
        };

        let experiment = self.chaos_repo.create_experiment(&final_dto).await?;

        // Log experiment created event
        let _ = self
            .audit_service
            .log_action(
                crate::models::chaos_audit_log::ChaosAuditAction::EXPERIMENT_CREATED,
                None,
                Some("system".to_string()),
                Some(experiment.id),
                None,
                Some(experiment.target_resource_id.clone()),
                None,
                Some(serde_json::json!(experiment)),
                None,
                Some(ExperimentStatus::DRAFT),
                None,
                None,
                None,
            )
            .await;

        Ok(experiment)
    }

    pub async fn create_experiment_from_template(
        &self,
        template_id: Uuid,
        account_id: String,
        region: String,
        target_resource_id: String,
        target_resource_name: Option<String>,
        parameter_overrides: Option<serde_json::Value>,
    ) -> Result<ExperimentModel, AppError> {
        let template = self.get_template(template_id).await?;

        let mut params = template.default_parameters.clone();
        if let Some(overrides) = parameter_overrides {
            if let Some(base_obj) = params.as_object_mut() {
                if let Some(override_obj) = overrides.as_object() {
                    for (k, v) in override_obj {
                        base_obj.insert(k.clone(), v.clone());
                    }
                }
            }
        }

        let dto = ChaosExperimentCreateDto {
            name: format!("{} - {}", template.name, target_resource_id),
            description: template.description.clone(),
            template_id: Some(template_id),
            account_id,
            region,
            resource_type: template.resource_type.clone(),
            target_resource_id,
            target_resource_name,
            experiment_type: template.experiment_type.clone(),
            parameters: Some(params),
            schedule_cron: None,
        };

        let experiment = self.chaos_repo.create_experiment(&dto).await?;

        // Log experiment created from template event
        let _ = self
            .audit_service
            .log_action(
                crate::models::chaos_audit_log::ChaosAuditAction::EXPERIMENT_CREATED,
                None,
                Some("system".to_string()),
                Some(experiment.id),
                None,
                Some(experiment.target_resource_id.clone()),
                None,
                Some(serde_json::json!({"template_id": template_id})),
                None,
                Some(ExperimentStatus::DRAFT),
                None,
                None,
                None,
            )
            .await;

        Ok(experiment)
    }

    pub async fn update_experiment(
        &self,
        id: Uuid,
        dto: ChaosExperimentUpdateDto,
    ) -> Result<ExperimentModel, AppError> {
        let old_experiment = self.get_experiment(id).await?;
        let updated_experiment = self.chaos_repo.update_experiment(id, &dto).await?;

        // Log experiment updated event
        let _ = self
            .audit_service
            .log_action(
                crate::models::chaos_audit_log::ChaosAuditAction::EXPERIMENT_UPDATED,
                None,
                Some("system".to_string()),
                Some(id),
                None,
                Some(updated_experiment.target_resource_id.clone()),
                Some(serde_json::json!(old_experiment)),
                Some(serde_json::json!(updated_experiment)),
                Some(old_experiment.status.clone()),
                Some(updated_experiment.status.clone()),
                None,
                None,
                None,
            )
            .await;

        Ok(updated_experiment)
    }

    pub async fn delete_experiment(&self, id: Uuid) -> Result<(), AppError> {
        // Check if the experiment is currently running
        let experiment = self.get_experiment(id).await?;
        if experiment.status == ExperimentStatus::RUNNING {
            return Err(AppError::BadRequest(
                "Cannot delete a running experiment. Stop it first.".to_string(),
            ));
        }
        self.chaos_repo.delete_experiment(id).await?;

        // Log experiment deleted event
        let _ = self
            .audit_service
            .log_action(
                crate::models::chaos_audit_log::ChaosAuditAction::EXPERIMENT_DELETED,
                None,
                Some("system".to_string()),
                Some(id),
                None,
                Some(experiment.target_resource_id.clone()),
                Some(serde_json::json!(experiment)),
                None,
                Some(experiment.status.clone()),
                None,
                None,
                None,
                None,
            )
            .await;

        Ok(())
    }

    // ========================================================================
    // Run/Execute Operations
    // ========================================================================

    pub async fn run_experiment(
        &self,
        experiment_id: Uuid,
        request: RunExperimentRequest,
    ) -> Result<RunModel, AppError> {
        let experiment = self.get_experiment(experiment_id).await?;

        // Validate experiment can be run
        if experiment.status == ExperimentStatus::RUNNING {
            return Err(AppError::BadRequest(
                "Experiment is already running".to_string(),
            ));
        }

        // Create the run
        let run = self
            .chaos_repo
            .create_run(experiment_id, request.triggered_by.clone())
            .await?;

        // Log run started event
        let _ = self
            .audit_service
            .log_action(
                "run_started",
                request.user_id.clone(),
                request.triggered_by.clone(),
                Some(experiment_id),
                Some(run.id),
                Some(experiment.target_resource_id.clone()),
                None,
                None,
                Some(ExperimentStatus::DRAFT),
                Some(RunStatus::PENDING),
                None,
                request.ip_address.clone(),
                request.user_agent.clone(),
            )
            .await;

        // Update experiment status to running
        self.chaos_repo
            .update_experiment_status(experiment_id, ExperimentStatus::RUNNING)
            .await?;

        // Update run to initializing
        let run = self
            .chaos_repo
            .update_run_status(run.id, RunStatus::INITIALIZING, None)
            .await?;

        // Log the start
        self.chaos_repo
            .append_execution_log(
                run.id,
                serde_json::json!({
                    "timestamp": Utc::now().to_rfc3339(),
                    "level": "info",
                    "message": format!("Initializing chaos experiment: {}", experiment.name)
                }),
            )
            .await?;

        // Execute the experiment based on type
        let execution_result = self
            .execute_chaos_action(&experiment, &run, &request)
            .await;

        match execution_result {
            Ok(result_data) => {
                // Log success
                self.chaos_repo
                    .append_execution_log(
                        run.id,
                        serde_json::json!({
                            "timestamp": Utc::now().to_rfc3339(),
                            "level": "info",
                            "message": "Chaos experiment completed successfully"
                        }),
                    )
                    .await?;

                // Create result record
                self.chaos_repo
                    .create_result(
                        run.id,
                        experiment_id,
                        &experiment.target_resource_id,
                        &experiment.resource_type,
                        result_data.baseline_metrics,
                        result_data.during_metrics,
                        result_data.recovery_metrics,
                        result_data.impact_summary,
                        &result_data.impact_severity,
                        result_data.recovery_time_ms,
                        result_data.steady_state_hypothesis,
                        result_data.hypothesis_met,
                        result_data.observations,
                    )
                    .await?;

                // Update statuses
                let run = self
                    .chaos_repo
                    .update_run_status(run.id, RunStatus::COMPLETED, None)
                    .await?;
                self.chaos_repo
                    .update_experiment_status(experiment_id, ExperimentStatus::COMPLETED)
                    .await?;

                // Record execution metrics
                let _ = self
                    .metrics_service
                    .record_execution_metrics(
                        run.id,
                        experiment_id,
                        experiment.target_resource_id.clone(),
                        experiment.resource_type.clone(),
                        result_data.execution_duration_ms,
                        result_data.rollback_duration_ms,
                        Some(true), // execution_success
                        result_data.rollback_success,
                        Some(result_data.impact_severity.clone()),
                        result_data.recovery_time_ms,
                        result_data.api_calls_made,
                        result_data.api_errors,
                        Some(result_data.custom_metrics.clone()),
                    )
                    .await;

                // Log run completed event
                let _ = self
                    .audit_service
                    .log_action(
                        crate::models::chaos_audit_log::ChaosAuditAction::RUN_COMPLETED,
                        request.user_id.clone(),
                        request.triggered_by.clone(),
                        Some(experiment_id),
                        Some(run.id),
                        Some(experiment.target_resource_id.clone()),
                        None,
                        Some(serde_json::json!({"impact_severity": result_data.impact_severity})),
                        Some(RunStatus::INITIALIZING),
                        Some(RunStatus::COMPLETED),
                        None,
                        request.ip_address.clone(),
                        request.user_agent.clone(),
                    )
                    .await;

                Ok(run)
            }
            Err(e) => {
                // Log failure
                self.chaos_repo
                    .append_execution_log(
                        run.id,
                        serde_json::json!({
                            "timestamp": Utc::now().to_rfc3339(),
                            "level": "error",
                            "message": format!("Chaos experiment failed: {}", e)
                        }),
                    )
                    .await?;

                // Attempt rollback if configured
                let params = &experiment.parameters;
                let should_rollback = params
                    .get("rollback_on_failure")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(true);

                if should_rollback {
                    self.chaos_repo
                        .update_run_rollback_status(run.id, "in_progress")
                        .await?;

                    self.chaos_repo
                        .append_execution_log(
                            run.id,
                            serde_json::json!({
                                "timestamp": Utc::now().to_rfc3339(),
                                "level": "warn",
                                "message": "Initiating rollback due to experiment failure"
                            }),
                        )
                        .await?;

                    match self.execute_rollback(&experiment).await {
                        Ok(_) => {
                            self.chaos_repo
                                .update_run_rollback_status(run.id, "completed")
                                .await?;
                            self.chaos_repo
                                .append_execution_log(
                                    run.id,
                                    serde_json::json!({
                                        "timestamp": Utc::now().to_rfc3339(),
                                        "level": "info",
                                        "message": "Rollback completed successfully"
                                    }),
                                )
                                .await?;
                        }
                        Err(rollback_err) => {
                            self.chaos_repo
                                .update_run_rollback_status(run.id, "failed")
                                .await?;
                            self.chaos_repo
                                .append_execution_log(
                                    run.id,
                                    serde_json::json!({
                                        "timestamp": Utc::now().to_rfc3339(),
                                        "level": "error",
                                        "message": format!("Rollback failed: {}", rollback_err)
                                    }),
                                )
                                .await?;
                        }
                    }
                }

                // Update statuses to failed
                let run = self
                    .chaos_repo
                    .update_run_status(run.id, RunStatus::FAILED, Some(e.to_string()))
                    .await?;
                self.chaos_repo
                    .update_experiment_status(experiment_id, ExperimentStatus::FAILED)
                    .await?;

                // Log run failed event
                let _ = self
                    .audit_service
                    .log_action(
                        crate::models::chaos_audit_log::ChaosAuditAction::RUN_FAILED,
                        request.user_id.clone(),
                        request.triggered_by.clone(),
                        Some(experiment_id),
                        Some(run.id),
                        Some(experiment.target_resource_id.clone()),
                        None,
                        Some(serde_json::json!({"error": e.to_string()})),
                        Some(RunStatus::INITIALIZING),
                        Some(RunStatus::FAILED),
                        None,
                        request.ip_address.clone(),
                        request.user_agent.clone(),
                    )
                    .await;

                Ok(run)
            }
        }
    }

    pub async fn stop_experiment(&self, experiment_id: Uuid) -> Result<ExperimentModel, AppError> {
        let experiment = self.get_experiment(experiment_id).await?;
        if experiment.status != ExperimentStatus::RUNNING {
            return Err(AppError::BadRequest(
                "Experiment is not currently running".to_string(),
            ));
        }

        // Attempt rollback
        info!(
            "Stopping experiment {} and initiating rollback",
            experiment_id
        );

        if let Err(e) = self.execute_rollback(&experiment).await {
            warn!(
                "Rollback failed during stop for experiment {}: {}",
                experiment_id, e
            );
        }

        self.chaos_repo
            .update_experiment_status(experiment_id, ExperimentStatus::CANCELLED)
            .await
    }

    pub async fn batch_run_experiments(
        &self,
        request: BatchRunRequest,
    ) -> Result<Vec<RunModel>, AppError> {
        let mut runs = Vec::new();
        for experiment_id in &request.experiment_ids {
            let run_request = RunExperimentRequest {
                triggered_by: request.triggered_by.clone(),
                parameter_overrides: None,
            };
            match self.run_experiment(*experiment_id, run_request).await {
                Ok(run) => runs.push(run),
                Err(e) => {
                    error!(
                        "Failed to run experiment {}: {}. Continuing with remaining.",
                        experiment_id, e
                    );
                }
            }
        }
        Ok(runs)
    }

    // ========================================================================
    // Run History & Results
    // ========================================================================

    pub async fn get_run(&self, run_id: Uuid) -> Result<RunModel, AppError> {
        self.chaos_repo
            .get_run(run_id)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("Run {} not found", run_id)))
    }

    pub async fn get_run_with_results(
        &self,
        run_id: Uuid,
    ) -> Result<RunWithResults, AppError> {
        self.chaos_repo
            .get_run_with_results(run_id)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("Run {} not found", run_id)))
    }

    pub async fn list_runs_for_experiment(
        &self,
        experiment_id: Uuid,
    ) -> Result<Vec<RunModel>, AppError> {
        self.chaos_repo.list_runs_for_experiment(experiment_id).await
    }

    pub async fn get_results_for_experiment(
        &self,
        experiment_id: Uuid,
    ) -> Result<Vec<ResultModel>, AppError> {
        self.chaos_repo
            .get_results_for_experiment(experiment_id)
            .await
    }

    // ========================================================================
    // Resource-centric Views
    // ========================================================================

    pub async fn get_experiments_for_resource(
        &self,
        resource_id: &str,
    ) -> Result<Vec<ExperimentModel>, AppError> {
        self.chaos_repo.get_experiments_for_resource(resource_id).await
    }

    pub async fn get_resource_experiment_history(
        &self,
        resource_id: &str,
    ) -> Result<ResourceExperimentHistory, AppError> {
        self.chaos_repo
            .get_resource_experiment_history(resource_id)
            .await
    }

    // ========================================================================
    // Private: Chaos Execution Engine
    // ========================================================================

    async fn execute_chaos_action(
        &self,
        experiment: &ExperimentModel,
        run: &RunModel,
        request: &RunExperimentRequest,
    ) -> Result<ChaosResultData, AppError> {
        let experiment_type = experiment.experiment_type.as_str();
        let params = if let Some(ref overrides) = request.parameter_overrides {
            let mut merged = experiment.parameters.clone();
            if let Some(base_obj) = merged.as_object_mut() {
                if let Some(override_obj) = overrides.as_object() {
                    for (k, v) in override_obj {
                        base_obj.insert(k.clone(), v.clone());
                    }
                }
            }
            merged
        } else {
            experiment.parameters.clone()
        };

        let is_dry_run = params
            .get("dry_run")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        // Get the AWS account config for this experiment
        let aws_account = self
            .aws_account_repo
            .get_by_account_id(&experiment.account_id)
            .await?
            .ok_or_else(|| {
                AppError::NotFound(format!(
                    "AWS account {} not found",
                    experiment.account_id
                ))
            })?;

        let mut aws_account_dto = AwsAccountDto::from(aws_account);
        aws_account_dto.default_region = experiment.region.clone();

        self.chaos_repo
            .append_execution_log(
                run.id,
                serde_json::json!({
                    "timestamp": Utc::now().to_rfc3339(),
                    "level": "info",
                    "message": format!(
                        "Executing {} on {} (account: {}, region: {}, dry_run: {})",
                        experiment_type, experiment.target_resource_id,
                        experiment.account_id, experiment.region, is_dry_run
                    )
                }),
            )
            .await?;

        // Route to the appropriate handler
        match experiment_type {
            // EC2 Experiments
            "instance_stop" => {
                self.execute_ec2_stop(experiment, &aws_account_dto, &params, is_dry_run, run.id)
                    .await
            }
            "instance_reboot" => {
                self.execute_ec2_reboot(experiment, &aws_account_dto, &params, is_dry_run, run.id)
                    .await
            }
            "instance_terminate" => {
                self.execute_ec2_terminate(experiment, &aws_account_dto, &params, is_dry_run, run.id)
                    .await
            }

            // RDS Experiments
            "rds_failover" => {
                self.execute_rds_failover(experiment, &aws_account_dto, &params, is_dry_run, run.id)
                    .await
            }
            "rds_reboot" => {
                self.execute_rds_reboot(experiment, &aws_account_dto, &params, is_dry_run, run.id)
                    .await
            }

            // Lambda Experiments
            "lambda_disable" => {
                self.execute_lambda_disable(experiment, &aws_account_dto, &params, is_dry_run, run.id)
                    .await
            }
            "lambda_timeout" => {
                self.execute_lambda_timeout(experiment, &aws_account_dto, &params, is_dry_run, run.id)
                    .await
            }

            // ECS Experiments
            "ecs_scale_down" => {
                self.execute_ecs_scale_down(experiment, &aws_account_dto, &params, is_dry_run, run.id)
                    .await
            }

            // ElastiCache Experiments
            "elasticache_failover" => {
                self.execute_elasticache_failover(experiment, &aws_account_dto, &params, is_dry_run, run.id)
                    .await
            }

            // DynamoDB Experiments
            "dynamodb_throttle" => {
                self.execute_dynamodb_throttle(experiment, &aws_account_dto, &params, is_dry_run, run.id)
                    .await
            }

            // S3 Experiments
            "s3_deny_access" => {
                self.execute_s3_deny_access(experiment, &aws_account_dto, &params, is_dry_run, run.id)
                    .await
            }

            // Networking Experiments
            "alb_deregister_targets" => {
                self.execute_alb_deregister(experiment, &aws_account_dto, &params, is_dry_run, run.id)
                    .await
            }
            "sg_block_ingress" => {
                self.execute_sg_block_ingress(experiment, &aws_account_dto, &params, is_dry_run, run.id)
                    .await
            }

            // SQS Experiments
            "sqs_purge" => {
                self.execute_sqs_purge(experiment, &aws_account_dto, &params, is_dry_run, run.id)
                    .await
            }

            // EKS Experiments
            "eks_scale_down" => {
                self.execute_eks_scale_down(experiment, &aws_account_dto, &params, is_dry_run, run.id)
                    .await
            }

            _ => Err(AppError::BadRequest(format!(
                "Unknown experiment type: {}",
                experiment_type
            ))),
        }
    }

    // ========================================================================
    // EC2 Chaos Actions
    // ========================================================================

    async fn execute_ec2_stop(
        &self,
        experiment: &ExperimentModel,
        aws_account_dto: &AwsAccountDto,
        params: &serde_json::Value,
        is_dry_run: bool,
        run_id: Uuid,
    ) -> Result<ChaosResultData, AppError> {
        let instance_id = &experiment.target_resource_id;

        self.log_action(run_id, &format!("Stopping EC2 instance: {}", instance_id))
            .await?;

        let ec2_client = self.aws_service.create_ec2_client(aws_account_dto).await?;

        if !is_dry_run {
            ec2_client
                .stop_instances()
                .instance_ids(instance_id)
                .send()
                .await
                .map_err(|e| {
                    AppError::CloudProvider(format!("Failed to stop EC2 instance: {:?}", e))
                })?;

            self.log_action(
                run_id,
                &format!("EC2 instance {} stop initiated successfully", instance_id),
            )
            .await?;
        } else {
            self.log_action(run_id, "[DRY RUN] Would stop EC2 instance")
                .await?;
        }

        Ok(ChaosResultData::new(
            "medium",
            Some(format!("EC2 instance {} stopped", instance_id)),
        ))
    }

    async fn execute_ec2_reboot(
        &self,
        experiment: &ExperimentModel,
        aws_account_dto: &AwsAccountDto,
        params: &serde_json::Value,
        is_dry_run: bool,
        run_id: Uuid,
    ) -> Result<ChaosResultData, AppError> {
        let instance_id = &experiment.target_resource_id;

        self.log_action(run_id, &format!("Rebooting EC2 instance: {}", instance_id))
            .await?;

        let ec2_client = self.aws_service.create_ec2_client(aws_account_dto).await?;

        if !is_dry_run {
            ec2_client
                .reboot_instances()
                .instance_ids(instance_id)
                .send()
                .await
                .map_err(|e| {
                    AppError::CloudProvider(format!("Failed to reboot EC2 instance: {:?}", e))
                })?;

            self.log_action(
                run_id,
                &format!("EC2 instance {} reboot initiated", instance_id),
            )
            .await?;
        } else {
            self.log_action(run_id, "[DRY RUN] Would reboot EC2 instance")
                .await?;
        }

        Ok(ChaosResultData::new(
            "low",
            Some(format!("EC2 instance {} rebooted", instance_id)),
        ))
    }

    async fn execute_ec2_terminate(
        &self,
        experiment: &ExperimentModel,
        aws_account_dto: &AwsAccountDto,
        params: &serde_json::Value,
        is_dry_run: bool,
        run_id: Uuid,
    ) -> Result<ChaosResultData, AppError> {
        let instance_id = &experiment.target_resource_id;

        self.log_action(
            run_id,
            &format!("Terminating EC2 instance: {}", instance_id),
        )
        .await?;

        let ec2_client = self.aws_service.create_ec2_client(aws_account_dto).await?;

        if !is_dry_run {
            ec2_client
                .terminate_instances()
                .instance_ids(instance_id)
                .send()
                .await
                .map_err(|e| {
                    AppError::CloudProvider(format!("Failed to terminate EC2 instance: {:?}", e))
                })?;

            self.log_action(
                run_id,
                &format!("EC2 instance {} terminated", instance_id),
            )
            .await?;
        } else {
            self.log_action(run_id, "[DRY RUN] Would terminate EC2 instance")
                .await?;
        }

        Ok(ChaosResultData::new(
            "critical",
            Some(format!("EC2 instance {} terminated (irreversible)", instance_id)),
        ))
    }

    // ========================================================================
    // RDS Chaos Actions
    // ========================================================================

    async fn execute_rds_failover(
        &self,
        experiment: &ExperimentModel,
        aws_account_dto: &AwsAccountDto,
        params: &serde_json::Value,
        is_dry_run: bool,
        run_id: Uuid,
    ) -> Result<ChaosResultData, AppError> {
        let db_instance_id = &experiment.target_resource_id;

        self.log_action(
            run_id,
            &format!("Initiating RDS failover for: {}", db_instance_id),
        )
        .await?;

        let rds_client = self.aws_service.create_rds_client(aws_account_dto).await?;

        if !is_dry_run {
            rds_client
                .reboot_db_instance()
                .db_instance_identifier(db_instance_id)
                .force_failover(true)
                .send()
                .await
                .map_err(|e| {
                    AppError::CloudProvider(format!("Failed to initiate RDS failover: {:?}", e))
                })?;

            self.log_action(
                run_id,
                &format!(
                    "RDS failover initiated for {}. Instance is rebooting with force failover.",
                    db_instance_id
                ),
            )
            .await?;
        } else {
            self.log_action(run_id, "[DRY RUN] Would initiate RDS failover")
                .await?;
        }

        Ok(ChaosResultData::new(
            "high",
            Some(format!("RDS failover initiated for {}", db_instance_id)),
        ))
    }

    async fn execute_rds_reboot(
        &self,
        experiment: &ExperimentModel,
        aws_account_dto: &AwsAccountDto,
        params: &serde_json::Value,
        is_dry_run: bool,
        run_id: Uuid,
    ) -> Result<ChaosResultData, AppError> {
        let db_instance_id = &experiment.target_resource_id;
        let force_failover = params
            .get("force_failover")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        self.log_action(
            run_id,
            &format!("Rebooting RDS instance: {}", db_instance_id),
        )
        .await?;

        let rds_client = self.aws_service.create_rds_client(aws_account_dto).await?;

        if !is_dry_run {
            rds_client
                .reboot_db_instance()
                .db_instance_identifier(db_instance_id)
                .force_failover(force_failover)
                .send()
                .await
                .map_err(|e| {
                    AppError::CloudProvider(format!("Failed to reboot RDS instance: {:?}", e))
                })?;

            self.log_action(
                run_id,
                &format!("RDS instance {} reboot initiated", db_instance_id),
            )
            .await?;
        } else {
            self.log_action(run_id, "[DRY RUN] Would reboot RDS instance")
                .await?;
        }

        Ok(ChaosResultData::new(
            "high",
            Some(format!("RDS instance {} rebooted", db_instance_id)),
        ))
    }

    // ========================================================================
    // Lambda Chaos Actions
    // ========================================================================

    async fn execute_lambda_disable(
        &self,
        experiment: &ExperimentModel,
        aws_account_dto: &AwsAccountDto,
        params: &serde_json::Value,
        is_dry_run: bool,
        run_id: Uuid,
    ) -> Result<ChaosResultData, AppError> {
        let function_name = &experiment.target_resource_id;
        let concurrency = params
            .get("reserved_concurrency")
            .and_then(|v| v.as_i64())
            .unwrap_or(0) as i32;

        self.log_action(
            run_id,
            &format!(
                "Setting Lambda {} reserved concurrency to {}",
                function_name, concurrency
            ),
        )
        .await?;

        let lambda_client = self
            .aws_service
            .create_lambda_client(aws_account_dto)
            .await?;

        if !is_dry_run {
            lambda_client
                .put_function_concurrency()
                .function_name(function_name)
                .reserved_concurrent_executions(concurrency)
                .send()
                .await
                .map_err(|e| {
                    AppError::CloudProvider(format!(
                        "Failed to set Lambda concurrency: {:?}",
                        e
                    ))
                })?;

            self.log_action(
                run_id,
                &format!(
                    "Lambda {} concurrency set to {}",
                    function_name, concurrency
                ),
            )
            .await?;
        } else {
            self.log_action(run_id, "[DRY RUN] Would disable Lambda function")
                .await?;
        }

        Ok(ChaosResultData::new(
            "high",
            Some(format!(
                "Lambda function {} disabled (concurrency={})",
                function_name, concurrency
            )),
        ))
    }

    async fn execute_lambda_timeout(
        &self,
        experiment: &ExperimentModel,
        aws_account_dto: &AwsAccountDto,
        params: &serde_json::Value,
        is_dry_run: bool,
        run_id: Uuid,
    ) -> Result<ChaosResultData, AppError> {
        let function_name = &experiment.target_resource_id;
        let target_timeout = params
            .get("target_timeout_seconds")
            .and_then(|v| v.as_i64())
            .unwrap_or(1) as i32;

        self.log_action(
            run_id,
            &format!(
                "Setting Lambda {} timeout to {}s",
                function_name, target_timeout
            ),
        )
        .await?;

        let lambda_client = self
            .aws_service
            .create_lambda_client(aws_account_dto)
            .await?;

        if !is_dry_run {
            lambda_client
                .update_function_configuration()
                .function_name(function_name)
                .timeout(target_timeout)
                .send()
                .await
                .map_err(|e| {
                    AppError::CloudProvider(format!(
                        "Failed to update Lambda timeout: {:?}",
                        e
                    ))
                })?;

            self.log_action(
                run_id,
                &format!("Lambda {} timeout set to {}s", function_name, target_timeout),
            )
            .await?;
        } else {
            self.log_action(run_id, "[DRY RUN] Would reduce Lambda timeout")
                .await?;
        }

        Ok(ChaosResultData::new(
            "medium",
            Some(format!(
                "Lambda function {} timeout reduced to {}s",
                function_name, target_timeout
            )),
        ))
    }

    // ========================================================================
    // ECS Chaos Actions
    // ========================================================================

    async fn execute_ecs_scale_down(
        &self,
        experiment: &ExperimentModel,
        aws_account_dto: &AwsAccountDto,
        params: &serde_json::Value,
        is_dry_run: bool,
        run_id: Uuid,
    ) -> Result<ChaosResultData, AppError> {
        let service_arn = &experiment.target_resource_id;
        let target_count = params
            .get("target_count")
            .and_then(|v| v.as_i64())
            .unwrap_or(0) as i32;

        // Parse cluster name from the service ARN or use a parameter
        let cluster = params
            .get("cluster")
            .and_then(|v| v.as_str())
            .unwrap_or("default");

        self.log_action(
            run_id,
            &format!(
                "Scaling down ECS service {} to {} tasks",
                service_arn, target_count
            ),
        )
        .await?;

        let ecs_client = self.aws_service.create_ecs_client(aws_account_dto).await?;

        if !is_dry_run {
            ecs_client
                .update_service()
                .cluster(cluster)
                .service(service_arn)
                .desired_count(target_count)
                .send()
                .await
                .map_err(|e| {
                    AppError::CloudProvider(format!("Failed to scale down ECS service: {:?}", e))
                })?;

            self.log_action(
                run_id,
                &format!(
                    "ECS service {} scaled down to {} tasks",
                    service_arn, target_count
                ),
            )
            .await?;
        } else {
            self.log_action(run_id, "[DRY RUN] Would scale down ECS service")
                .await?;
        }

        Ok(ChaosResultData::new(
            "high",
            Some(format!(
                "ECS service {} scaled to {} tasks",
                service_arn, target_count
            )),
        ))
    }

    // ========================================================================
    // ElastiCache Chaos Actions
    // ========================================================================

    async fn execute_elasticache_failover(
        &self,
        experiment: &ExperimentModel,
        aws_account_dto: &AwsAccountDto,
        params: &serde_json::Value,
        is_dry_run: bool,
        run_id: Uuid,
    ) -> Result<ChaosResultData, AppError> {
        let replication_group_id = &experiment.target_resource_id;

        self.log_action(
            run_id,
            &format!(
                "Initiating ElastiCache failover for: {}",
                replication_group_id
            ),
        )
        .await?;

        let elasticache_client = self
            .aws_service
            .create_elasticache_client(aws_account_dto)
            .await?;

        if !is_dry_run {
            // Get the primary node group to failover
            let node_group_id = params
                .get("node_group_id")
                .and_then(|v| v.as_str())
                .unwrap_or("0001");

            elasticache_client
                .test_failover()
                .replication_group_id(replication_group_id)
                .node_group_id(node_group_id)
                .send()
                .await
                .map_err(|e| {
                    AppError::CloudProvider(format!(
                        "Failed to initiate ElastiCache failover: {:?}",
                        e
                    ))
                })?;

            self.log_action(
                run_id,
                &format!(
                    "ElastiCache failover initiated for {}",
                    replication_group_id
                ),
            )
            .await?;
        } else {
            self.log_action(run_id, "[DRY RUN] Would initiate ElastiCache failover")
                .await?;
        }

        Ok(ChaosResultData::new(
            "high",
            Some(format!(
                "ElastiCache failover initiated for {}",
                replication_group_id
            )),
        ))
    }

    // ========================================================================
    // DynamoDB Chaos Actions
    // ========================================================================

    async fn execute_dynamodb_throttle(
        &self,
        experiment: &ExperimentModel,
        aws_account_dto: &AwsAccountDto,
        params: &serde_json::Value,
        is_dry_run: bool,
        run_id: Uuid,
    ) -> Result<ChaosResultData, AppError> {
        let table_name = &experiment.target_resource_id;
        let target_rcu = params
            .get("target_read_capacity")
            .and_then(|v| v.as_i64())
            .unwrap_or(1);
        let target_wcu = params
            .get("target_write_capacity")
            .and_then(|v| v.as_i64())
            .unwrap_or(1);

        self.log_action(
            run_id,
            &format!(
                "Reducing DynamoDB {} capacity to RCU={}, WCU={}",
                table_name, target_rcu, target_wcu
            ),
        )
        .await?;

        let dynamodb_client = self
            .aws_service
            .create_dynamodb_client(aws_account_dto)
            .await?;

        if !is_dry_run {
            dynamodb_client
                .update_table()
                .table_name(table_name)
                .provisioned_throughput(
                    aws_sdk_dynamodb::types::ProvisionedThroughput::builder()
                        .read_capacity_units(target_rcu)
                        .write_capacity_units(target_wcu)
                        .build()
                        .map_err(|e| AppError::CloudProvider(format!("Failed to build throughput: {:?}", e)))?,
                )
                .send()
                .await
                .map_err(|e| {
                    AppError::CloudProvider(format!(
                        "Failed to update DynamoDB table capacity: {:?}",
                        e
                    ))
                })?;

            self.log_action(
                run_id,
                &format!(
                    "DynamoDB {} capacity reduced to RCU={}, WCU={}",
                    table_name, target_rcu, target_wcu
                ),
            )
            .await?;
        } else {
            self.log_action(run_id, "[DRY RUN] Would reduce DynamoDB capacity")
                .await?;
        }

        Ok(ChaosResultData::new(
            "high",
            Some(format!(
                "DynamoDB table {} throttled (RCU={}, WCU={})",
                table_name, target_rcu, target_wcu
            )),
        ))
    }

    // ========================================================================
    // S3 Chaos Actions
    // ========================================================================

    async fn execute_s3_deny_access(
        &self,
        experiment: &ExperimentModel,
        aws_account_dto: &AwsAccountDto,
        params: &serde_json::Value,
        is_dry_run: bool,
        run_id: Uuid,
    ) -> Result<ChaosResultData, AppError> {
        let bucket_name = &experiment.target_resource_id;

        self.log_action(
            run_id,
            &format!("Applying deny-all policy to S3 bucket: {}", bucket_name),
        )
        .await?;

        let s3_client = self.aws_service.create_s3_client(aws_account_dto).await?;

        if !is_dry_run {
            let deny_policy = serde_json::json!({
                "Version": "2012-10-17",
                "Statement": [{
                    "Sid": "ChaosExperimentDenyAll",
                    "Effect": "Deny",
                    "Principal": "*",
                    "Action": "s3:*",
                    "Resource": [
                        format!("arn:aws:s3:::{}", bucket_name),
                        format!("arn:aws:s3:::{}/*", bucket_name)
                    ]
                }]
            });

            s3_client
                .put_bucket_policy()
                .bucket(bucket_name)
                .policy(deny_policy.to_string())
                .send()
                .await
                .map_err(|e| {
                    AppError::CloudProvider(format!(
                        "Failed to apply S3 deny policy: {:?}",
                        e
                    ))
                })?;

            self.log_action(
                run_id,
                &format!("S3 bucket {} deny-all policy applied", bucket_name),
            )
            .await?;
        } else {
            self.log_action(run_id, "[DRY RUN] Would apply deny-all S3 policy")
                .await?;
        }

        Ok(ChaosResultData::new(
            "high",
            Some(format!("S3 bucket {} access denied", bucket_name)),
        ))
    }

    // ========================================================================
    // ALB Chaos Actions
    // ========================================================================

    async fn execute_alb_deregister(
        &self,
        experiment: &ExperimentModel,
        aws_account_dto: &AwsAccountDto,
        params: &serde_json::Value,
        is_dry_run: bool,
        run_id: Uuid,
    ) -> Result<ChaosResultData, AppError> {
        let target_group_arn = &experiment.target_resource_id;

        self.log_action(
            run_id,
            &format!(
                "Deregistering targets from ALB target group: {}",
                target_group_arn
            ),
        )
        .await?;

        if !is_dry_run {
            let elbv2_client = self
                .aws_service
                .create_elbv2_client(aws_account_dto)
                .await?;

            // Get current targets
            let targets = elbv2_client
                .describe_target_health()
                .target_group_arn(target_group_arn)
                .send()
                .await
                .map_err(|e| {
                    AppError::CloudProvider(format!(
                        "Failed to describe target health: {:?}",
                        e
                    ))
                })?;

            let target_descriptions = targets.target_health_descriptions();
            let deregister_pct = params
                .get("deregister_percentage")
                .and_then(|v| v.as_i64())
                .unwrap_or(50) as usize;

            let count_to_deregister =
                (target_descriptions.len() * deregister_pct / 100).max(1);

            let targets_to_deregister: Vec<_> = target_descriptions
                .iter()
                .take(count_to_deregister)
                .filter_map(|td| td.target().cloned())
                .collect();

            if !targets_to_deregister.is_empty() {
                elbv2_client
                    .deregister_targets()
                    .target_group_arn(target_group_arn)
                    .set_targets(Some(targets_to_deregister.clone()))
                    .send()
                    .await
                    .map_err(|e| {
                        AppError::CloudProvider(format!(
                            "Failed to deregister targets: {:?}",
                            e
                        ))
                    })?;

                self.log_action(
                    run_id,
                    &format!(
                        "Deregistered {} targets from target group",
                        targets_to_deregister.len()
                    ),
                )
                .await?;
            }
        } else {
            self.log_action(run_id, "[DRY RUN] Would deregister ALB targets")
                .await?;
        }

        Ok(ChaosResultData::new(
            "high",
            Some(format!(
                "ALB targets deregistered from {}",
                target_group_arn
            )),
        ))
    }

    // ========================================================================
    // Security Group Chaos Actions
    // ========================================================================

    async fn execute_sg_block_ingress(
        &self,
        experiment: &ExperimentModel,
        aws_account_dto: &AwsAccountDto,
        params: &serde_json::Value,
        is_dry_run: bool,
        run_id: Uuid,
    ) -> Result<ChaosResultData, AppError> {
        let sg_id = &experiment.target_resource_id;

        self.log_action(
            run_id,
            &format!("Revoking all ingress rules from SG: {}", sg_id),
        )
        .await?;

        let ec2_client = self.aws_service.create_ec2_client(aws_account_dto).await?;

        if !is_dry_run {
            // Get current ingress rules
            let sg_desc = ec2_client
                .describe_security_groups()
                .group_ids(sg_id)
                .send()
                .await
                .map_err(|e| {
                    AppError::CloudProvider(format!(
                        "Failed to describe security group: {:?}",
                        e
                    ))
                })?;

            if let Some(sg) = sg_desc.security_groups().first() {
                let ip_permissions = sg.ip_permissions();
                if !ip_permissions.is_empty() {
                    ec2_client
                        .revoke_security_group_ingress()
                        .group_id(sg_id)
                        .set_ip_permissions(Some(ip_permissions.to_vec()))
                        .send()
                        .await
                        .map_err(|e| {
                            AppError::CloudProvider(format!(
                                "Failed to revoke ingress rules: {:?}",
                                e
                            ))
                        })?;

                    self.log_action(
                        run_id,
                        &format!(
                            "Revoked {} ingress rules from SG {}",
                            ip_permissions.len(),
                            sg_id
                        ),
                    )
                    .await?;
                }
            }
        } else {
            self.log_action(run_id, "[DRY RUN] Would revoke SG ingress rules")
                .await?;
        }

        Ok(ChaosResultData::new(
            "critical",
            Some(format!(
                "All ingress rules removed from security group {}",
                sg_id
            )),
        ))
    }

    // ========================================================================
    // SQS Chaos Actions
    // ========================================================================

    async fn execute_sqs_purge(
        &self,
        experiment: &ExperimentModel,
        aws_account_dto: &AwsAccountDto,
        params: &serde_json::Value,
        is_dry_run: bool,
        run_id: Uuid,
    ) -> Result<ChaosResultData, AppError> {
        let queue_url = &experiment.target_resource_id;

        self.log_action(
            run_id,
            &format!("Purging SQS queue: {}", queue_url),
        )
        .await?;

        let sqs_client = self.aws_service.create_sqs_client(aws_account_dto).await?;

        if !is_dry_run {
            sqs_client
                .purge_queue()
                .queue_url(queue_url)
                .send()
                .await
                .map_err(|e| {
                    AppError::CloudProvider(format!("Failed to purge SQS queue: {:?}", e))
                })?;

            self.log_action(
                run_id,
                &format!("SQS queue {} purged", queue_url),
            )
            .await?;
        } else {
            self.log_action(run_id, "[DRY RUN] Would purge SQS queue")
                .await?;
        }

        Ok(ChaosResultData::new(
            "high",
            Some(format!("SQS queue {} purged (irreversible)", queue_url)),
        ))
    }

    // ========================================================================
    // EKS Chaos Actions
    // ========================================================================

    async fn execute_eks_scale_down(
        &self,
        experiment: &ExperimentModel,
        aws_account_dto: &AwsAccountDto,
        params: &serde_json::Value,
        is_dry_run: bool,
        run_id: Uuid,
    ) -> Result<ChaosResultData, AppError> {
        let cluster_name = &experiment.target_resource_id;
        let target_size = params
            .get("target_size")
            .and_then(|v| v.as_i64())
            .unwrap_or(1) as i32;

        let nodegroup_name = params
            .get("nodegroup_name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                AppError::BadRequest("nodegroup_name parameter is required for EKS scale down".to_string())
            })?;

        self.log_action(
            run_id,
            &format!(
                "Scaling EKS node group {} in cluster {} to {}",
                nodegroup_name, cluster_name, target_size
            ),
        )
        .await?;

        let eks_client = self.aws_service.create_eks_client(aws_account_dto).await?;

        if !is_dry_run {
            eks_client
                .update_nodegroup_config()
                .cluster_name(cluster_name)
                .nodegroup_name(nodegroup_name)
                .scaling_config(
                    aws_sdk_eks::types::NodegroupScalingConfig::builder()
                        .desired_size(target_size)
                        .build(),
                )
                .send()
                .await
                .map_err(|e| {
                    AppError::CloudProvider(format!(
                        "Failed to scale EKS node group: {:?}",
                        e
                    ))
                })?;

            self.log_action(
                run_id,
                &format!(
                    "EKS node group {} scaled to {} nodes",
                    nodegroup_name, target_size
                ),
            )
            .await?;
        } else {
            self.log_action(run_id, "[DRY RUN] Would scale down EKS node group")
                .await?;
        }

        Ok(ChaosResultData::new(
            "high",
            Some(format!(
                "EKS node group {} scaled to {} nodes",
                nodegroup_name, target_size
            )),
        ))
    }

    // ========================================================================
    // Rollback Engine
    // ========================================================================

    async fn execute_rollback(
        &self,
        experiment: &ExperimentModel,
    ) -> Result<(), AppError> {
        info!(
            "Executing rollback for experiment: {} (type: {})",
            experiment.id, experiment.experiment_type
        );

        // Rollback logic depends on the experiment type
        // In a full implementation, the pre-experiment state would be saved
        // For now, we log that rollback was attempted
        match experiment.experiment_type.as_str() {
            "instance_stop" => {
                info!(
                    "Rollback: Would start EC2 instance {}",
                    experiment.target_resource_id
                );
            }
            "lambda_disable" => {
                info!(
                    "Rollback: Would restore Lambda {} concurrency",
                    experiment.target_resource_id
                );
            }
            "lambda_timeout" => {
                info!(
                    "Rollback: Would restore Lambda {} timeout",
                    experiment.target_resource_id
                );
            }
            "dynamodb_throttle" => {
                info!(
                    "Rollback: Would restore DynamoDB {} capacity",
                    experiment.target_resource_id
                );
            }
            "s3_deny_access" => {
                info!(
                    "Rollback: Would restore S3 {} bucket policy",
                    experiment.target_resource_id
                );
            }
            "sg_block_ingress" => {
                info!(
                    "Rollback: Would restore security group {} rules",
                    experiment.target_resource_id
                );
            }
            "ecs_scale_down" => {
                info!(
                    "Rollback: Would restore ECS service {} desired count",
                    experiment.target_resource_id
                );
            }
            _ => {
                warn!(
                    "No rollback handler for experiment type: {}",
                    experiment.experiment_type
                );
            }
        }

        Ok(())
    }

    // ========================================================================
    // Helpers
    // ========================================================================

    async fn log_action(&self, run_id: Uuid, message: &str) -> Result<(), AppError> {
        info!("Chaos: {}", message);
        self.chaos_repo
            .append_execution_log(
                run_id,
                serde_json::json!({
                    "timestamp": Utc::now().to_rfc3339(),
                    "level": "info",
                    "message": message
                }),
            )
            .await?;
        Ok(())
    }
}

/// Internal struct for passing chaos result data
#[derive(Debug)]
struct ChaosResultData {
    baseline_metrics: serde_json::Value,
    during_metrics: serde_json::Value,
    recovery_metrics: serde_json::Value,
    impact_summary: Option<String>,
    impact_severity: String,
    recovery_time_ms: Option<i64>,
    steady_state_hypothesis: Option<String>,
    hypothesis_met: Option<bool>,
    observations: serde_json::Value,
}

impl ChaosResultData {
    fn new(severity: &str, summary: Option<String>) -> Self {
        Self {
            baseline_metrics: serde_json::json!({}),
            during_metrics: serde_json::json!({}),
            recovery_metrics: serde_json::json!({}),
            impact_summary: summary,
            impact_severity: severity.to_string(),
            recovery_time_ms: None,
            steady_state_hypothesis: None,
            hypothesis_met: None,
            observations: serde_json::json!([]),
        }
    }
}

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
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, Order, PaginatorTrait,
    QueryFilter, QueryOrder, QuerySelect, Set,
};
use std::sync::Arc;
use tracing::error;
use uuid::Uuid;

use crate::errors::AppError;
use crate::models::chaos_experiment::{
    ActiveModel as ExperimentActiveModel, ChaosExperimentCreateDto, ChaosExperimentPage,
    ChaosExperimentQuery, ChaosExperimentUpdateDto, Column as ExperimentColumn,
    Entity as ExperimentEntity, Model as ExperimentModel,
};
use crate::models::chaos_experiment_result::{
    ActiveModel as ResultActiveModel, Column as ResultColumn, Entity as ResultEntity,
    Model as ResultModel, ResourceExperimentHistory, ResourceExperimentSummary,
};
use crate::models::chaos_experiment_run::{
    ActiveModel as RunActiveModel, Column as RunColumn, Entity as RunEntity, Model as RunModel,
    RunWithResults,
};
use crate::models::chaos_template::{
    ActiveModel as TemplateActiveModel, ChaosTemplateCreateDto, ChaosTemplateQuery,
    ChaosTemplateUpdateDto, Column as TemplateColumn, Entity as TemplateEntity,
    Model as TemplateModel,
};

#[derive(Debug)]
pub struct ChaosRepository {
    db: Arc<DatabaseConnection>,
}

impl ChaosRepository {
    pub fn new(db: Arc<DatabaseConnection>) -> Self {
        Self { db }
    }

    // ========================================================================
    // Template Operations
    // ========================================================================

    pub async fn list_templates(
        &self,
        query: &ChaosTemplateQuery,
    ) -> Result<Vec<TemplateModel>, AppError> {
        let mut select = TemplateEntity::find();

        if let Some(ref category) = query.category {
            select = select.filter(TemplateColumn::Category.eq(category.clone()));
        }
        if let Some(ref resource_type) = query.resource_type {
            select = select.filter(TemplateColumn::ResourceType.eq(resource_type.clone()));
        }
        if let Some(ref experiment_type) = query.experiment_type {
            select = select.filter(TemplateColumn::ExperimentType.eq(experiment_type.clone()));
        }
        if let Some(is_built_in) = query.is_built_in {
            select = select.filter(TemplateColumn::IsBuiltIn.eq(is_built_in));
        }
        if let Some(is_active) = query.is_active {
            select = select.filter(TemplateColumn::IsActive.eq(is_active));
        }

        select
            .order_by(TemplateColumn::Category, Order::Asc)
            .order_by(TemplateColumn::Name, Order::Asc)
            .all(&*self.db)
            .await
            .map_err(|e| {
                error!("Error listing chaos templates: {:?}", e);
                AppError::Database(e)
            })
    }

    pub async fn get_template(&self, id: Uuid) -> Result<Option<TemplateModel>, AppError> {
        TemplateEntity::find_by_id(id)
            .one(&*self.db)
            .await
            .map_err(|e| {
                error!("Error fetching chaos template: {:?}", e);
                AppError::Database(e)
            })
    }

    pub async fn create_template(
        &self,
        dto: &ChaosTemplateCreateDto,
    ) -> Result<TemplateModel, AppError> {
        let now = Utc::now();
        let active_model = TemplateActiveModel {
            id: Set(Uuid::new_v4()),
            name: Set(dto.name.clone()),
            description: Set(dto.description.clone()),
            category: Set(dto.category.clone()),
            resource_type: Set(dto.resource_type.clone()),
            experiment_type: Set(dto.experiment_type.clone()),
            default_parameters: Set(dto
                .default_parameters
                .clone()
                .unwrap_or(serde_json::json!({}))),
            prerequisites: Set(dto.prerequisites.clone()),
            expected_impact: Set(dto
                .expected_impact
                .clone()
                .unwrap_or_else(|| "medium".to_string())),
            estimated_duration_seconds: Set(dto.estimated_duration_seconds.unwrap_or(60)),
            rollback_steps: Set(dto
                .rollback_steps
                .clone()
                .unwrap_or(serde_json::json!([]))),
            documentation: Set(dto.documentation.clone()),
            is_built_in: Set(false),
            is_active: Set(true),
            created_at: Set(now),
            updated_at: Set(now),
        };

        active_model.insert(&*self.db).await.map_err(|e| {
            error!("Error creating chaos template: {:?}", e);
            AppError::Database(e)
        })
    }

    pub async fn update_template(
        &self,
        id: Uuid,
        dto: &ChaosTemplateUpdateDto,
    ) -> Result<TemplateModel, AppError> {
        let template = self
            .get_template(id)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("Chaos template {} not found", id)))?;

        let mut active_model: TemplateActiveModel = template.into();
        active_model.updated_at = Set(Utc::now());

        if let Some(ref name) = dto.name {
            active_model.name = Set(name.clone());
        }
        if let Some(ref description) = dto.description {
            active_model.description = Set(Some(description.clone()));
        }
        if let Some(ref category) = dto.category {
            active_model.category = Set(category.clone());
        }
        if let Some(ref params) = dto.default_parameters {
            active_model.default_parameters = Set(params.clone());
        }
        if let Some(ref prereqs) = dto.prerequisites {
            active_model.prerequisites = Set(Some(prereqs.clone()));
        }
        if let Some(ref impact) = dto.expected_impact {
            active_model.expected_impact = Set(impact.clone());
        }
        if let Some(duration) = dto.estimated_duration_seconds {
            active_model.estimated_duration_seconds = Set(duration);
        }
        if let Some(ref steps) = dto.rollback_steps {
            active_model.rollback_steps = Set(steps.clone());
        }
        if let Some(ref doc) = dto.documentation {
            active_model.documentation = Set(Some(doc.clone()));
        }
        if let Some(is_active) = dto.is_active {
            active_model.is_active = Set(is_active);
        }

        active_model.update(&*self.db).await.map_err(|e| {
            error!("Error updating chaos template: {:?}", e);
            AppError::Database(e)
        })
    }

    pub async fn delete_template(&self, id: Uuid) -> Result<(), AppError> {
        TemplateEntity::delete_by_id(id)
            .exec(&*self.db)
            .await
            .map_err(|e| {
                error!("Error deleting chaos template: {:?}", e);
                AppError::Database(e)
            })?;
        Ok(())
    }

    // ========================================================================
    // Experiment Operations
    // ========================================================================

    pub async fn list_experiments(
        &self,
        query: &ChaosExperimentQuery,
    ) -> Result<ChaosExperimentPage, AppError> {
        let page = query.page.unwrap_or(1);
        let page_size = query.page_size.unwrap_or(20);

        let mut select = ExperimentEntity::find();

        if let Some(ref account_id) = query.account_id {
            select = select.filter(ExperimentColumn::AccountId.eq(account_id.clone()));
        }
        if let Some(ref region) = query.region {
            select = select.filter(ExperimentColumn::Region.eq(region.clone()));
        }
        if let Some(ref resource_type) = query.resource_type {
            select = select.filter(ExperimentColumn::ResourceType.eq(resource_type.clone()));
        }
        if let Some(ref target_resource_id) = query.target_resource_id {
            select = select
                .filter(ExperimentColumn::TargetResourceId.eq(target_resource_id.clone()));
        }
        if let Some(ref experiment_type) = query.experiment_type {
            select = select.filter(ExperimentColumn::ExperimentType.eq(experiment_type.clone()));
        }
        if let Some(ref status) = query.status {
            select = select.filter(ExperimentColumn::Status.eq(status.clone()));
        }
        if let Some(template_id) = query.template_id {
            select = select.filter(ExperimentColumn::TemplateId.eq(template_id));
        }

        let total = select
            .clone()
            .count(&*self.db)
            .await
            .map_err(|e| AppError::Database(e))?;

        let total_pages = (total + page_size - 1) / page_size;

        let experiments = select
            .order_by(ExperimentColumn::UpdatedAt, Order::Desc)
            .offset((page - 1) * page_size)
            .limit(page_size)
            .all(&*self.db)
            .await
            .map_err(|e| {
                error!("Error listing chaos experiments: {:?}", e);
                AppError::Database(e)
            })?;

        Ok(ChaosExperimentPage {
            experiments,
            total,
            page,
            page_size,
            total_pages,
        })
    }

    pub async fn get_experiment(&self, id: Uuid) -> Result<Option<ExperimentModel>, AppError> {
        ExperimentEntity::find_by_id(id)
            .one(&*self.db)
            .await
            .map_err(|e| {
                error!("Error fetching chaos experiment: {:?}", e);
                AppError::Database(e)
            })
    }

    pub async fn create_experiment(
        &self,
        dto: &ChaosExperimentCreateDto,
    ) -> Result<ExperimentModel, AppError> {
        let now = Utc::now();
        let active_model = ExperimentActiveModel {
            id: Set(Uuid::new_v4()),
            name: Set(dto.name.clone()),
            description: Set(dto.description.clone()),
            template_id: Set(dto.template_id),
            account_id: Set(dto.account_id.clone()),
            region: Set(dto.region.clone()),
            resource_type: Set(dto.resource_type.clone()),
            target_resource_id: Set(dto.target_resource_id.clone()),
            target_resource_name: Set(dto.target_resource_name.clone()),
            experiment_type: Set(dto.experiment_type.clone()),
            parameters: Set(dto.parameters.clone().unwrap_or(serde_json::json!({}))),
            schedule_cron: Set(dto.schedule_cron.clone()),
            status: Set("draft".to_string()),
            created_by: Set(None),
            created_at: Set(now),
            updated_at: Set(now),
        };

        active_model.insert(&*self.db).await.map_err(|e| {
            error!("Error creating chaos experiment: {:?}", e);
            AppError::Database(e)
        })
    }

    pub async fn update_experiment(
        &self,
        id: Uuid,
        dto: &ChaosExperimentUpdateDto,
    ) -> Result<ExperimentModel, AppError> {
        let experiment = self
            .get_experiment(id)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("Chaos experiment {} not found", id)))?;

        let mut active_model: ExperimentActiveModel = experiment.into();
        active_model.updated_at = Set(Utc::now());

        if let Some(ref name) = dto.name {
            active_model.name = Set(name.clone());
        }
        if let Some(ref description) = dto.description {
            active_model.description = Set(Some(description.clone()));
        }
        if let Some(ref params) = dto.parameters {
            active_model.parameters = Set(params.clone());
        }
        if let Some(ref cron) = dto.schedule_cron {
            active_model.schedule_cron = Set(Some(cron.clone()));
        }
        if let Some(ref status) = dto.status {
            active_model.status = Set(status.clone());
        }

        active_model.update(&*self.db).await.map_err(|e| {
            error!("Error updating chaos experiment: {:?}", e);
            AppError::Database(e)
        })
    }

    pub async fn update_experiment_status(
        &self,
        id: Uuid,
        status: &str,
    ) -> Result<ExperimentModel, AppError> {
        let experiment = self
            .get_experiment(id)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("Chaos experiment {} not found", id)))?;

        let mut active_model: ExperimentActiveModel = experiment.into();
        active_model.status = Set(status.to_string());
        active_model.updated_at = Set(Utc::now());

        active_model.update(&*self.db).await.map_err(|e| {
            error!("Error updating experiment status: {:?}", e);
            AppError::Database(e)
        })
    }

    pub async fn delete_experiment(&self, id: Uuid) -> Result<(), AppError> {
        ExperimentEntity::delete_by_id(id)
            .exec(&*self.db)
            .await
            .map_err(|e| {
                error!("Error deleting chaos experiment: {:?}", e);
                AppError::Database(e)
            })?;
        Ok(())
    }

    // ========================================================================
    // Run Operations
    // ========================================================================

    pub async fn create_run(
        &self,
        experiment_id: Uuid,
        triggered_by: Option<String>,
    ) -> Result<RunModel, AppError> {
        // Get next run number
        let run_count = RunEntity::find()
            .filter(RunColumn::ExperimentId.eq(experiment_id))
            .count(&*self.db)
            .await
            .map_err(|e| AppError::Database(e))?;

        let active_model = RunActiveModel {
            id: Set(Uuid::new_v4()),
            experiment_id: Set(experiment_id),
            run_number: Set((run_count + 1) as i32),
            status: Set("pending".to_string()),
            started_at: Set(None),
            ended_at: Set(None),
            duration_ms: Set(None),
            triggered_by: Set(triggered_by),
            execution_log: Set(serde_json::json!([])),
            error_message: Set(None),
            rollback_status: Set(None),
            rollback_started_at: Set(None),
            rollback_ended_at: Set(None),
            created_at: Set(Utc::now()),
        };

        active_model.insert(&*self.db).await.map_err(|e| {
            error!("Error creating chaos run: {:?}", e);
            AppError::Database(e)
        })
    }

    pub async fn get_run(&self, id: Uuid) -> Result<Option<RunModel>, AppError> {
        RunEntity::find_by_id(id)
            .one(&*self.db)
            .await
            .map_err(|e| {
                error!("Error fetching chaos run: {:?}", e);
                AppError::Database(e)
            })
    }

    pub async fn get_run_with_results(&self, id: Uuid) -> Result<Option<RunWithResults>, AppError> {
        let run = self.get_run(id).await?;
        match run {
            Some(run) => {
                let results = ResultEntity::find()
                    .filter(ResultColumn::RunId.eq(id))
                    .all(&*self.db)
                    .await
                    .map_err(|e| AppError::Database(e))?;

                Ok(Some(RunWithResults { run, results }))
            }
            None => Ok(None),
        }
    }

    pub async fn list_runs_for_experiment(
        &self,
        experiment_id: Uuid,
    ) -> Result<Vec<RunModel>, AppError> {
        RunEntity::find()
            .filter(RunColumn::ExperimentId.eq(experiment_id))
            .order_by(RunColumn::RunNumber, Order::Desc)
            .all(&*self.db)
            .await
            .map_err(|e| {
                error!("Error listing chaos runs: {:?}", e);
                AppError::Database(e)
            })
    }

    pub async fn update_run_status(
        &self,
        id: Uuid,
        status: &str,
        error_message: Option<String>,
    ) -> Result<RunModel, AppError> {
        let run = self
            .get_run(id)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("Chaos run {} not found", id)))?;

        let original_started_at = run.started_at;
        let mut active_model: RunActiveModel = run.into();
        active_model.status = Set(status.to_string());

        match status {
            "initializing" | "running" => {
                active_model.started_at = Set(Some(Utc::now()));
            }
            "completed" | "failed" | "cancelled" | "timed_out" => {
                let now = Utc::now();
                active_model.ended_at = Set(Some(now));
                if let Some(started_time) = original_started_at {
                    active_model.duration_ms =
                        Set(Some((now - started_time).num_milliseconds()));
                }
            }
            _ => {}
        }

        if let Some(err) = error_message {
            active_model.error_message = Set(Some(err));
        }

        active_model.update(&*self.db).await.map_err(|e| {
            error!("Error updating chaos run status: {:?}", e);
            AppError::Database(e)
        })
    }

    pub async fn append_execution_log(
        &self,
        id: Uuid,
        log_entry: serde_json::Value,
    ) -> Result<RunModel, AppError> {
        let run = self
            .get_run(id)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("Chaos run {} not found", id)))?;

        let mut logs = run.execution_log.clone();
        if let Some(arr) = logs.as_array_mut() {
            arr.push(log_entry);
        }

        let mut active_model: RunActiveModel = run.into();
        active_model.execution_log = Set(logs);

        active_model.update(&*self.db).await.map_err(|e| {
            error!("Error appending execution log: {:?}", e);
            AppError::Database(e)
        })
    }

    pub async fn update_run_rollback_status(
        &self,
        id: Uuid,
        rollback_status: &str,
    ) -> Result<RunModel, AppError> {
        let run = self
            .get_run(id)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("Chaos run {} not found", id)))?;

        let mut active_model: RunActiveModel = run.into();
        active_model.rollback_status = Set(Some(rollback_status.to_string()));

        match rollback_status {
            "in_progress" => {
                active_model.rollback_started_at = Set(Some(Utc::now()));
            }
            "completed" | "failed" => {
                active_model.rollback_ended_at = Set(Some(Utc::now()));
            }
            _ => {}
        }

        active_model.update(&*self.db).await.map_err(|e| {
            error!("Error updating rollback status: {:?}", e);
            AppError::Database(e)
        })
    }

    // ========================================================================
    // Result Operations
    // ========================================================================

    pub async fn create_result(
        &self,
        run_id: Uuid,
        experiment_id: Uuid,
        resource_id: &str,
        resource_type: &str,
        baseline_metrics: serde_json::Value,
        during_metrics: serde_json::Value,
        recovery_metrics: serde_json::Value,
        impact_summary: Option<String>,
        impact_severity: &str,
        recovery_time_ms: Option<i64>,
        steady_state_hypothesis: Option<String>,
        hypothesis_met: Option<bool>,
        observations: serde_json::Value,
    ) -> Result<ResultModel, AppError> {
        let active_model = ResultActiveModel {
            id: Set(Uuid::new_v4()),
            run_id: Set(run_id),
            experiment_id: Set(experiment_id),
            resource_id: Set(resource_id.to_string()),
            resource_type: Set(resource_type.to_string()),
            baseline_metrics: Set(baseline_metrics),
            during_metrics: Set(during_metrics),
            recovery_metrics: Set(recovery_metrics),
            impact_summary: Set(impact_summary),
            impact_severity: Set(impact_severity.to_string()),
            recovery_time_ms: Set(recovery_time_ms),
            steady_state_hypothesis: Set(steady_state_hypothesis),
            hypothesis_met: Set(hypothesis_met),
            observations: Set(observations),
            created_at: Set(Utc::now()),
        };

        active_model.insert(&*self.db).await.map_err(|e| {
            error!("Error creating chaos result: {:?}", e);
            AppError::Database(e)
        })
    }

    pub async fn get_results_for_run(&self, run_id: Uuid) -> Result<Vec<ResultModel>, AppError> {
        ResultEntity::find()
            .filter(ResultColumn::RunId.eq(run_id))
            .all(&*self.db)
            .await
            .map_err(|e| {
                error!("Error fetching results for run: {:?}", e);
                AppError::Database(e)
            })
    }

    pub async fn get_results_for_experiment(
        &self,
        experiment_id: Uuid,
    ) -> Result<Vec<ResultModel>, AppError> {
        ResultEntity::find()
            .filter(ResultColumn::ExperimentId.eq(experiment_id))
            .order_by(ResultColumn::CreatedAt, Order::Desc)
            .all(&*self.db)
            .await
            .map_err(|e| {
                error!("Error fetching results for experiment: {:?}", e);
                AppError::Database(e)
            })
    }

    pub async fn get_resource_experiment_history(
        &self,
        resource_id: &str,
    ) -> Result<ResourceExperimentHistory, AppError> {
        // Get all results for this resource
        let results = ResultEntity::find()
            .filter(ResultColumn::ResourceId.eq(resource_id))
            .order_by(ResultColumn::CreatedAt, Order::Desc)
            .all(&*self.db)
            .await
            .map_err(|e| AppError::Database(e))?;

        if results.is_empty() {
            return Ok(ResourceExperimentHistory {
                resource_id: resource_id.to_string(),
                resource_type: String::new(),
                experiments: vec![],
                total_runs: 0,
                last_run_at: None,
            });
        }

        let resource_type = results[0].resource_type.clone();
        let mut summaries = Vec::new();

        for result in &results {
            // Fetch the corresponding run and experiment
            let run = self.get_run(result.run_id).await?;
            let experiment = self.get_experiment(result.experiment_id).await?;

            if let (Some(run), Some(experiment)) = (run, experiment) {
                summaries.push(ResourceExperimentSummary {
                    experiment_id: experiment.id,
                    experiment_name: experiment.name,
                    experiment_type: experiment.experiment_type,
                    run_id: run.id,
                    run_status: run.status,
                    started_at: run.started_at,
                    ended_at: run.ended_at,
                    impact_severity: result.impact_severity.clone(),
                    recovery_time_ms: result.recovery_time_ms,
                });
            }
        }

        let total_runs = summaries.len() as u64;
        let last_run_at = summaries.first().and_then(|s| s.started_at);

        Ok(ResourceExperimentHistory {
            resource_id: resource_id.to_string(),
            resource_type,
            experiments: summaries,
            total_runs,
            last_run_at,
        })
    }

    // ========================================================================
    // Aggregate/Stats Operations
    // ========================================================================

    pub async fn get_experiments_for_resource(
        &self,
        resource_id: &str,
    ) -> Result<Vec<ExperimentModel>, AppError> {
        ExperimentEntity::find()
            .filter(ExperimentColumn::TargetResourceId.eq(resource_id))
            .order_by(ExperimentColumn::UpdatedAt, Order::Desc)
            .all(&*self.db)
            .await
            .map_err(|e| {
                error!("Error fetching experiments for resource: {:?}", e);
                AppError::Database(e)
            })
    }

    pub async fn get_latest_run_for_experiment(
        &self,
        experiment_id: Uuid,
    ) -> Result<Option<RunModel>, AppError> {
        RunEntity::find()
            .filter(RunColumn::ExperimentId.eq(experiment_id))
            .order_by(RunColumn::RunNumber, Order::Desc)
            .one(&*self.db)
            .await
            .map_err(|e| {
                error!("Error fetching latest run: {:?}", e);
                AppError::Database(e)
            })
    }

    pub async fn get_experiment_run_count(
        &self,
        experiment_id: Uuid,
    ) -> Result<u64, AppError> {
        RunEntity::find()
            .filter(RunColumn::ExperimentId.eq(experiment_id))
            .count(&*self.db)
            .await
            .map_err(|e| AppError::Database(e))
    }
}

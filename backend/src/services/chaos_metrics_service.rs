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
use crate::models::chaos_metrics::{
    ExecutionMetricsCreateDto, MetricsQuery, MetricsStats, Model as ExecutionMetricsModel,
};
use crate::repositories::chaos_metrics_repository::ChaosMetricsRepository;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Clone, Debug)]
pub struct ChaosMetricsService {
    metrics_repo: Arc<ChaosMetricsRepository>,
}

impl ChaosMetricsService {
    pub fn new(metrics_repo: Arc<ChaosMetricsRepository>) -> Self {
        Self { metrics_repo }
    }

    pub async fn record_execution_metrics(
        &self,
        run_id: Uuid,
        experiment_id: Uuid,
        resource_id: String,
        resource_type: String,
        execution_duration_ms: Option<i64>,
        rollback_duration_ms: Option<i64>,
        execution_success: Option<bool>,
        rollback_success: Option<bool>,
        impact_severity: Option<String>,
        time_to_recovery_ms: Option<i64>,
        api_calls_made: Option<i32>,
        api_errors: Option<i32>,
        custom_metrics: Option<serde_json::Value>,
    ) -> Result<ExecutionMetricsModel, AppError> {
        let dto = ExecutionMetricsCreateDto {
            run_id,
            experiment_id,
            resource_id,
            resource_type,
            execution_duration_ms,
            rollback_duration_ms,
            execution_success,
            rollback_success,
            impact_severity,
            time_to_recovery_ms,
            api_calls_made,
            api_errors,
            custom_metrics,
        };

        self.metrics_repo.create_execution_metrics(&dto).await
    }

    pub async fn get_run_metrics(
        &self,
        run_id: Uuid,
    ) -> Result<Option<ExecutionMetricsModel>, AppError> {
        self.metrics_repo.get_execution_metrics(run_id).await
    }

    pub async fn get_experiment_metrics(
        &self,
        experiment_id: Uuid,
    ) -> Result<Vec<ExecutionMetricsModel>, AppError> {
        self.metrics_repo
            .get_metrics_for_experiment(experiment_id)
            .await
    }

    pub async fn get_resource_type_metrics(
        &self,
        resource_type: &str,
    ) -> Result<Vec<ExecutionMetricsModel>, AppError> {
        self.metrics_repo
            .get_metrics_for_resource_type(resource_type)
            .await
    }

    pub async fn get_metrics_stats(
        &self,
        query: &MetricsQuery,
    ) -> Result<MetricsStats, AppError> {
        self.metrics_repo.get_metrics_stats(query).await
    }

    pub async fn get_experiment_summary(
        &self,
        experiment_id: Uuid,
    ) -> Result<MetricsStats, AppError> {
        let query = MetricsQuery {
            experiment_id: Some(experiment_id),
            resource_type: None,
            impact_severity: None,
            start_date: None,
            end_date: None,
        };
        self.metrics_repo.get_metrics_stats(&query).await
    }

    pub async fn get_resource_type_summary(
        &self,
        resource_type: &str,
    ) -> Result<MetricsStats, AppError> {
        let query = MetricsQuery {
            experiment_id: None,
            resource_type: Some(resource_type.to_string()),
            impact_severity: None,
            start_date: None,
            end_date: None,
        };
        self.metrics_repo.get_metrics_stats(&query).await
    }
}

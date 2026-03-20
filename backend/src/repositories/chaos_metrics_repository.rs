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

use crate::error::AppError;
use crate::models::chaos_metrics::{
    ExecutionMetricsCreateDto, MetricsQuery, MetricsStats, ExecutionMetricsModel,
};
use sea_orm::{
    ColumnTrait, Condition, DatabaseConnection, EntityTrait, QueryFilter, DbErr,
};
use std::sync::Arc;
use uuid::Uuid;

#[derive(Clone)]
pub struct ChaosMetricsRepository {
    db: Arc<DatabaseConnection>,
}

impl ChaosMetricsRepository {
    pub fn new(db: Arc<DatabaseConnection>) -> Self {
        Self { db }
    }

    pub async fn create_execution_metrics(
        &self,
        dto: &ExecutionMetricsCreateDto,
    ) -> Result<ExecutionMetricsModel, AppError> {
        use crate::models::chaos_metrics::Entity;
        use sea_orm::Set;

        let total_duration = match (dto.execution_duration_ms, dto.rollback_duration_ms) {
            (Some(exec), Some(rollback)) => Some(exec + rollback),
            (Some(exec), None) => Some(exec),
            (None, Some(rollback)) => Some(rollback),
            (None, None) => None,
        };

        let metrics = Entity::insert(crate::models::chaos_metrics::ActiveModel {
            id: Set(Uuid::new_v4()),
            run_id: Set(dto.run_id),
            experiment_id: Set(dto.experiment_id),
            resource_id: Set(dto.resource_id.clone()),
            resource_type: Set(dto.resource_type.clone()),
            execution_duration_ms: Set(dto.execution_duration_ms),
            rollback_duration_ms: Set(dto.rollback_duration_ms),
            total_duration_ms: Set(total_duration),
            execution_success: Set(dto.execution_success),
            rollback_success: Set(dto.rollback_success),
            impact_severity: Set(dto.impact_severity.clone()),
            estimated_affected_resources: Set(None),
            confirmed_affected_resources: Set(None),
            time_to_first_error_ms: Set(None),
            time_to_recovery_ms: Set(dto.time_to_recovery_ms),
            recovery_completeness_percent: Set(None),
            api_calls_made: Set(dto.api_calls_made),
            api_errors: Set(dto.api_errors),
            retries_performed: Set(None),
            custom_metrics: Set(dto.custom_metrics.clone().unwrap_or(serde_json::json!({}))),
            created_at: Set(chrono::Utc::now()),
        })
        .exec_with_returning(self.db.as_ref())
        .await
        .map_err(|e: DbErr| AppError::DatabaseError(e.to_string()))?;

        Ok(metrics)
    }

    pub async fn get_execution_metrics(
        &self,
        run_id: Uuid,
    ) -> Result<Option<ExecutionMetricsModel>, AppError> {
        use crate::models::chaos_metrics::Entity;

        let metrics = Entity::find()
            .filter(Entity::RunId.eq(run_id))
            .one(self.db.as_ref())
            .await
            .map_err(|e: DbErr| AppError::DatabaseError(e.to_string()))?;

        Ok(metrics)
    }

    pub async fn get_metrics_for_experiment(
        &self,
        experiment_id: Uuid,
    ) -> Result<Vec<ExecutionMetricsModel>, AppError> {
        use crate::models::chaos_metrics::Entity;

        let metrics = Entity::find()
            .filter(Entity::ExperimentId.eq(experiment_id))
            .all(self.db.as_ref())
            .await
            .map_err(|e: DbErr| AppError::DatabaseError(e.to_string()))?;

        Ok(metrics)
    }

    pub async fn get_metrics_for_resource_type(
        &self,
        resource_type: &str,
    ) -> Result<Vec<ExecutionMetricsModel>, AppError> {
        use crate::models::chaos_metrics::Entity;

        let metrics = Entity::find()
            .filter(Entity::ResourceType.eq(resource_type.to_string()))
            .all(self.db.as_ref())
            .await
            .map_err(|e: DbErr| AppError::DatabaseError(e.to_string()))?;

        Ok(metrics)
    }

    pub async fn get_metrics_stats(
        &self,
        query: &MetricsQuery,
    ) -> Result<MetricsStats, AppError> {
        use crate::models::chaos_metrics::Entity;

        let mut condition = Condition::all();

        if let Some(experiment_id) = query.experiment_id {
            condition = condition.add(Entity::ExperimentId.eq(experiment_id));
        }

        if let Some(resource_type) = &query.resource_type {
            condition = condition.add(Entity::ResourceType.eq(resource_type.clone()));
        }

        if let Some(impact_severity) = &query.impact_severity {
            condition = condition.add(Entity::ImpactSeverity.eq(impact_severity.clone()));
        }

        if let Some(start_date) = query.start_date {
            condition = condition.add(Entity::CreatedAt.gte(start_date));
        }

        if let Some(end_date) = query.end_date {
            condition = condition.add(Entity::CreatedAt.lte(end_date));
        }

        let metrics = Entity::find()
            .filter(condition)
            .all(self.db.as_ref())
            .await
            .map_err(|e: DbErr| AppError::DatabaseError(e.to_string()))?;

        let total_experiments = metrics.len() as u64;
        let successful_experiments = metrics
            .iter()
            .filter(|m| m.execution_success.unwrap_or(false))
            .count() as u64;
        let failed_experiments = total_experiments - successful_experiments;

        let success_rate = if total_experiments > 0 {
            (successful_experiments as f64 / total_experiments as f64) * 100.0
        } else {
            0.0
        };

        let total_execution_time: i64 = metrics
            .iter()
            .filter_map(|m| m.execution_duration_ms)
            .sum();
        let avg_execution_duration = if total_experiments > 0 {
            total_execution_time as f64 / total_experiments as f64
        } else {
            0.0
        };

        let total_recovery_time: i64 = metrics
            .iter()
            .filter_map(|m| m.time_to_recovery_ms)
            .sum();
        let recovery_count = metrics
            .iter()
            .filter(|m| m.time_to_recovery_ms.is_some())
            .count() as u64;
        let avg_recovery_time = if recovery_count > 0 {
            total_recovery_time as f64 / recovery_count as f64
        } else {
            0.0
        };

        // Find most impacted resource type
        let mut resource_type_counts: std::collections::HashMap<String, u64> =
            std::collections::HashMap::new();
        for metric in &metrics {
            *resource_type_counts
                .entry(metric.resource_type.clone())
                .or_insert(0) += 1;
        }
        let most_impacted_resource_type = resource_type_counts
            .iter()
            .max_by_key(|&(_, count)| count)
            .map(|(rt, _)| rt.clone());

        // Find average impact severity (simplified: most common)
        let mut severity_counts: std::collections::HashMap<String, u64> =
            std::collections::HashMap::new();
        for metric in &metrics {
            if let Some(severity) = &metric.impact_severity {
                *severity_counts.entry(severity.clone()).or_insert(0) += 1;
            }
        }
        let avg_impact_severity = severity_counts
            .iter()
            .max_by_key(|&(_, count)| count)
            .map(|(severity, _)| severity.clone());

        let rollback_success_count = metrics
            .iter()
            .filter(|m| m.rollback_success.unwrap_or(false))
            .count() as u64;
        let rollback_attempted_count = metrics
            .iter()
            .filter(|m| m.rollback_success.is_some())
            .count() as u64;
        let rollback_success_rate = if rollback_attempted_count > 0 {
            (rollback_success_count as f64 / rollback_attempted_count as f64) * 100.0
        } else {
            0.0
        };

        let total_rollback_time: i64 = metrics
            .iter()
            .filter_map(|m| m.rollback_duration_ms)
            .sum();
        let rollback_count = metrics
            .iter()
            .filter(|m| m.rollback_duration_ms.is_some())
            .count() as u64;
        let avg_rollback_time = if rollback_count > 0 {
            total_rollback_time as f64 / rollback_count as f64
        } else {
            0.0
        };

        Ok(MetricsStats {
            total_experiments,
            successful_experiments,
            failed_experiments,
            success_rate_percent: success_rate,
            avg_execution_duration_ms: avg_execution_duration,
            avg_recovery_time_ms: avg_recovery_time,
            most_impacted_resource_type,
            avg_impact_severity,
            rollback_success_rate_percent: rollback_success_rate,
            avg_rollback_time_ms: avg_rollback_time,
        })
    }
}

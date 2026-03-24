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
use crate::models::chaos_audit_log::{AuditLogCreateDto, AuditLogPage, AuditLogQuery, Model};
use sea_orm::{
    ColumnTrait, Condition, DatabaseConnection, EntityTrait, Order, PaginatorTrait, QueryFilter,
    QueryOrder, DbErr,
};
use std::sync::Arc;
use uuid::Uuid;

#[derive(Clone, Debug)]
pub struct ChaosAuditRepository {
    db: Arc<DatabaseConnection>,
}

impl ChaosAuditRepository {
    pub fn new(db: Arc<DatabaseConnection>) -> Self {
        Self { db }
    }

    pub async fn create_audit_log(
        &self,
        dto: &AuditLogCreateDto,
    ) -> Result<Model, AppError> {
        use crate::models::chaos_audit_log::Entity;
        use sea_orm::Set;

        let audit_log = Entity::insert(crate::models::chaos_audit_log::ActiveModel {
            id: Set(Uuid::new_v4()),
            experiment_id: Set(dto.experiment_id),
            run_id: Set(dto.run_id),
            action: Set(dto.action.clone()),
            user_id: Set(dto.user_id.clone()),
            triggered_by: Set(dto.triggered_by.clone()),
            resource_id: Set(dto.resource_id.clone()),
            old_values: Set(dto.old_values.clone().unwrap_or(serde_json::json!({}))),
            new_values: Set(dto.new_values.clone().unwrap_or(serde_json::json!({}))),
            status_before: Set(dto.status_before.clone()),
            status_after: Set(dto.status_after.clone()),
            details: Set(dto.details.clone().unwrap_or(serde_json::json!({}))),
            ip_address: Set(dto.ip_address.clone()),
            user_agent: Set(dto.user_agent.clone()),
            created_at: Set(chrono::Utc::now()),
        })
        .exec_with_returning(self.db.as_ref())
        .await
        .map_err(|e: DbErr| AppError::DatabaseError(e.to_string()))?;

        Ok(audit_log)
    }

    pub async fn list_audit_logs(
        &self,
        query: &AuditLogQuery,
    ) -> Result<AuditLogPage, AppError> {
        use crate::models::chaos_audit_log::Entity;

        let mut condition = Condition::all();

        if let Some(experiment_id) = query.experiment_id {
            condition = condition.add(crate::models::chaos_audit_log::Column::ExperimentId.eq(experiment_id));
        }

        if let Some(run_id) = query.run_id {
            condition = condition.add(crate::models::chaos_audit_log::Column::RunId.eq(run_id));
        }

        if let Some(action) = &query.action {
            condition = condition.add(crate::models::chaos_audit_log::Column::Action.eq(action.clone()));
        }

        if let Some(user_id) = &query.user_id {
            condition = condition.add(crate::models::chaos_audit_log::Column::UserId.eq(user_id.clone()));
        }

        if let Some(resource_id) = &query.resource_id {
            condition = condition.add(crate::models::chaos_audit_log::Column::ResourceId.eq(resource_id.clone()));
        }

        if let Some(triggered_by) = &query.triggered_by {
            condition = condition.add(crate::models::chaos_audit_log::Column::TriggeredBy.eq(triggered_by.clone()));
        }

        if let Some(start_date) = query.start_date {
            condition = condition.add(crate::models::chaos_audit_log::Column::CreatedAt.gte(start_date));
        }

        if let Some(end_date) = query.end_date {
            condition = condition.add(crate::models::chaos_audit_log::Column::CreatedAt.lte(end_date));
        }

        let page = query.page.unwrap_or(1);
        let page_size = query.page_size.unwrap_or(50);

        let paginator = Entity::find()
            .filter(condition)
            .order_by(crate::models::chaos_audit_log::Column::CreatedAt, Order::Desc)
            .paginate(self.db.as_ref(), page_size);

        let total = paginator.num_items().await.unwrap_or(0);
        let logs = paginator
            .fetch_page(page - 1)
            .await
            .map_err(|e: DbErr| AppError::DatabaseError(e.to_string()))?;

        let total_pages = (total + page_size - 1) / page_size;

        Ok(AuditLogPage {
            logs,
            total,
            page,
            page_size,
            total_pages,
        })
    }

    pub async fn get_audit_logs_for_experiment(
        &self,
        experiment_id: Uuid,
    ) -> Result<Vec<Model>, AppError> {
        use crate::models::chaos_audit_log::Entity;

        let logs = Entity::find()
            .filter(crate::models::chaos_audit_log::Column::ExperimentId.eq(experiment_id))
            .order_by(crate::models::chaos_audit_log::Column::CreatedAt, Order::Desc)
            .all(self.db.as_ref())
            .await
            .map_err(|e: DbErr| AppError::DatabaseError(e.to_string()))?;

        Ok(logs)
    }

    pub async fn get_audit_logs_for_run(&self, run_id: Uuid) -> Result<Vec<Model>, AppError> {
        use crate::models::chaos_audit_log::Entity;

        let logs = Entity::find()
            .filter(crate::models::chaos_audit_log::Column::RunId.eq(run_id))
            .order_by(crate::models::chaos_audit_log::Column::CreatedAt, Order::Desc)
            .all(self.db.as_ref())
            .await
            .map_err(|e: DbErr| AppError::DatabaseError(e.to_string()))?;

        Ok(logs)
    }

    pub async fn get_audit_logs_for_user(
        &self,
        user_id: &str,
    ) -> Result<Vec<Model>, AppError> {
        use crate::models::chaos_audit_log::Entity;

        let logs = Entity::find()
            .filter(crate::models::chaos_audit_log::Column::UserId.eq(user_id.to_string()))
            .order_by(crate::models::chaos_audit_log::Column::CreatedAt, Order::Desc)
            .all(self.db.as_ref())
            .await
            .map_err(|e: DbErr| AppError::DatabaseError(e.to_string()))?;

        Ok(logs)
    }

    pub async fn get_audit_logs_for_resource(
        &self,
        resource_id: &str,
    ) -> Result<Vec<Model>, AppError> {
        use crate::models::chaos_audit_log::Entity;

        let logs = Entity::find()
            .filter(crate::models::chaos_audit_log::Column::ResourceId.eq(resource_id.to_string()))
            .order_by(crate::models::chaos_audit_log::Column::CreatedAt, Order::Desc)
            .all(self.db.as_ref())
            .await
            .map_err(|e: DbErr| AppError::DatabaseError(e.to_string()))?;

        Ok(logs)
    }

    pub async fn get_audit_logs_for_action(
        &self,
        action: &str,
    ) -> Result<Vec<Model>, AppError> {
        use crate::models::chaos_audit_log::Entity;

        let logs = Entity::find()
            .filter(crate::models::chaos_audit_log::Column::Action.eq(action.to_string()))
            .order_by(crate::models::chaos_audit_log::Column::CreatedAt, Order::Desc)
            .all(self.db.as_ref())
            .await
            .map_err(|e: DbErr| AppError::DatabaseError(e.to_string()))?;

        Ok(logs)
    }
}

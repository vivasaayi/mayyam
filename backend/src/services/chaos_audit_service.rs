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
use crate::repositories::chaos_audit_repository::ChaosAuditRepository;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Clone, Debug)]
pub struct ChaosAuditService {
    audit_repo: Arc<ChaosAuditRepository>,
}

impl ChaosAuditService {
    pub fn new(audit_repo: Arc<ChaosAuditRepository>) -> Self {
        Self { audit_repo }
    }

    pub async fn log_action(
        &self,
        action: &str,
        user_id: Option<String>,
        triggered_by: Option<String>,
        experiment_id: Option<Uuid>,
        run_id: Option<Uuid>,
        resource_id: Option<String>,
        old_values: Option<serde_json::Value>,
        new_values: Option<serde_json::Value>,
        status_before: Option<String>,
        status_after: Option<String>,
        details: Option<serde_json::Value>,
        ip_address: Option<String>,
        user_agent: Option<String>,
    ) -> Result<Model, AppError> {
        let dto = AuditLogCreateDto {
            experiment_id,
            run_id,
            action: action.to_string(),
            user_id,
            triggered_by,
            resource_id,
            old_values,
            new_values,
            status_before,
            status_after,
            details,
            ip_address,
            user_agent,
        };

        self.audit_repo.create_audit_log(&dto).await
    }

    pub async fn list_audit_logs(
        &self,
        query: &AuditLogQuery,
    ) -> Result<AuditLogPage, AppError> {
        self.audit_repo.list_audit_logs(query).await
    }

    pub async fn get_experiment_audit_trail(
        &self,
        experiment_id: Uuid,
    ) -> Result<Vec<Model>, AppError> {
        self.audit_repo
            .get_audit_logs_for_experiment(experiment_id)
            .await
    }

    pub async fn get_run_audit_trail(&self, run_id: Uuid) -> Result<Vec<Model>, AppError> {
        self.audit_repo.get_audit_logs_for_run(run_id).await
    }

    pub async fn get_user_activity(&self, user_id: &str) -> Result<Vec<Model>, AppError> {
        self.audit_repo.get_audit_logs_for_user(user_id).await
    }

    pub async fn get_resource_audit_trail(
        &self,
        resource_id: &str,
    ) -> Result<Vec<Model>, AppError> {
        self.audit_repo.get_audit_logs_for_resource(resource_id).await
    }

    pub async fn get_action_history(&self, action: &str) -> Result<Vec<Model>, AppError> {
        self.audit_repo.get_audit_logs_for_action(action).await
    }
}

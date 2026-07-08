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

use actix_web::{web, HttpResponse, Responder};
use chrono::Utc;
use sea_orm::DatabaseConnection;
use serde::Deserialize;
use serde_json::json;
use std::sync::Arc;

use crate::config::Config;
use crate::errors::AppError;
use crate::middleware::auth::Claims;
use crate::repositories::database::DatabaseRepository;
use crate::services::alerting::database_slow_query_alert::{
    build_mysql_slow_query_alert_bundle, stale_after_hours,
};
use crate::services::analytics::mysql_analytics::MySqlTelemetryCollector;
use crate::utils::database::connect_to_dynamic_database;

#[derive(Debug, Deserialize)]
pub struct DatabaseSlowQueryAlertQuery {
    pub connection_id: Option<String>,
}

pub async fn get_mysql_slow_query_alerts(
    query: web::Query<DatabaseSlowQueryAlertQuery>,
    db_pool: web::Data<Arc<DatabaseConnection>>,
    config: web::Data<Config>,
    claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let query = query.into_inner();

    let bundle = if let Some(connection_id) = query
        .connection_id
        .as_deref()
        .map(str::trim)
        .filter(|connection_id| !connection_id.is_empty())
    {
        let db_repo = DatabaseRepository::new(db_pool.get_ref().clone(), config.get_ref().clone());
        let conn_id = uuid::Uuid::parse_str(connection_id)
            .map_err(|e| AppError::BadRequest(format!("Invalid UUID: {}", e)))?;
        let conn_model = db_repo
            .find_by_id(conn_id)
            .await?
            .ok_or_else(|| AppError::NotFound("Database connection not found".to_string()))?;

        let user_id = uuid::Uuid::parse_str(&claims.sub)
            .map_err(|e| AppError::BadRequest(format!("Invalid user UUID: {}", e)))?;
        let is_admin = claims.roles.iter().any(|role| role == "admin");
        if conn_model.created_by != user_id && !is_admin {
            return Err(AppError::Auth(
                "You do not have access to this database connection".to_string(),
            ));
        }

        let connection_type = conn_model.connection_type.to_lowercase();
        if connection_type != "mysql" && connection_type != "aurora-mysql" {
            return Err(AppError::BadRequest(
                "MySQL slow query alerts are only supported for mysql connections".to_string(),
            ));
        }

        let dynamic_conn = connect_to_dynamic_database(&conn_model, config.get_ref()).await?;
        let telemetry = MySqlTelemetryCollector::collect(&dynamic_conn).await?;
        let connection_id = conn_model.id.to_string();
        let connection_name = conn_model.name.clone();
        build_mysql_slow_query_alert_bundle(&[(&connection_id, &connection_name, &telemetry)])
    } else {
        build_mysql_slow_query_alert_bundle(&[])
    };

    Ok(HttpResponse::Ok().json(json!({
        "resource_type": bundle.resource_type,
        "evaluated_at": Utc::now(),
        "stale_after_hours": stale_after_hours(),
        "connection_id": query.connection_id,
        "resources_evaluated": bundle.resources_evaluated,
        "active_alerts": bundle.active_alerts,
        "insufficient_data_alerts": bundle.insufficient_data_alerts,
        "alerts": bundle.alerts,
        "notification_workflow": bundle.notification_workflow,
        "ai_triage_workflow": bundle.ai_triage_workflow,
        "agentic_investigation_workflow": bundle.agentic_investigation_workflow,
        "automation_workflow": bundle.automation_workflow,
        "health_workflow": bundle.health_workflow,
        "reporting": bundle.reporting,
    })))
}

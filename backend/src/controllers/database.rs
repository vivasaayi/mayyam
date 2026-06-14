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
use chrono::{DateTime, Utc};
use sea_orm::DatabaseConnection;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeMap;
use std::sync::Arc;
use uuid::Uuid;

use crate::config::Config;
use crate::errors::AppError;
use crate::middleware::auth::Claims;
use crate::models::database::{CreateDatabaseConnectionRequest, DatabaseQueryRequest};
use crate::repositories::database::DatabaseRepository;
use crate::repositories::mysql_telemetry_snapshot_repository::MySqlTelemetrySnapshotRepository;
use crate::services::analytics::mysql_analytics::aurora_mysql_inventory::{
    aurora_mysql_item_from_telemetry, evaluate_mysql_aurora_inventory,
    RESOURCE_TYPE as MYSQL_AURORA_RESOURCE_TYPE,
};
use crate::services::analytics::mysql_analytics::binary_log_inventory::{
    binary_log_item_from_telemetry, evaluate_mysql_binary_log_inventory,
    RESOURCE_TYPE as MYSQL_BINARY_LOG_RESOURCE_TYPE,
};
use crate::services::analytics::mysql_analytics::connection_threads_inventory::{
    connection_threads_item_from_telemetry, evaluate_mysql_connection_threads_inventory,
    RESOURCE_TYPE as MYSQL_CONNECTION_THREADS_RESOURCE_TYPE,
};
use crate::services::analytics::mysql_analytics::deadlocks_inventory::{
    deadlocks_item_from_telemetry, evaluate_mysql_deadlocks_inventory,
    RESOURCE_TYPE as MYSQL_DEADLOCKS_RESOURCE_TYPE,
};
use crate::services::analytics::mysql_analytics::digest_statistics_inventory::{
    digest_statistics_item_from_telemetry, evaluate_mysql_digest_statistics_inventory,
    RESOURCE_TYPE as MYSQL_DIGEST_STATISTICS_RESOURCE_TYPE,
};
use crate::services::analytics::mysql_analytics::group_replication_inventory::{
    evaluate_mysql_group_replication_inventory, group_replication_item_from_telemetry,
    RESOURCE_TYPE as MYSQL_GROUP_REPLICATION_RESOURCE_TYPE,
};
use crate::services::analytics::mysql_analytics::index_cardinality_inventory::{
    evaluate_mysql_index_cardinality_inventory, index_cardinality_item_from_telemetry,
    RESOURCE_TYPE as MYSQL_INDEX_CARDINALITY_RESOURCE_TYPE,
};
use crate::services::analytics::mysql_analytics::innodb_buffer_pool_inventory::{
    evaluate_mysql_innodb_buffer_pool_inventory, innodb_buffer_pool_item_from_telemetry,
    RESOURCE_TYPE as MYSQL_INNODB_BUFFER_POOL_RESOURCE_TYPE,
};
use crate::services::analytics::mysql_analytics::metadata_locks_inventory::{
    evaluate_mysql_metadata_locks_inventory, metadata_locks_item_from_telemetry,
    RESOURCE_TYPE as MYSQL_METADATA_LOCKS_RESOURCE_TYPE,
};
use crate::services::analytics::mysql_analytics::mysql_analytics_service::MySqlAnalyticsService;
use crate::services::analytics::mysql_analytics::mysql_signals::{
    MySqlPerformanceSignal, MySqlSignalEvaluator, MySqlSignalRules, MySqlSignalSnapshot,
};
use crate::services::analytics::mysql_analytics::mysql_telemetry::MySqlTelemetryCollector;
use crate::services::analytics::mysql_analytics::performance_schema_inventory::{
    evaluate_mysql_performance_schema_inventory, performance_schema_item_from_telemetry,
    RESOURCE_TYPE as MYSQL_PERFORMANCE_SCHEMA_RESOURCE_TYPE,
};
use crate::services::analytics::mysql_analytics::rds_mysql_inventory::{
    evaluate_mysql_rds_inventory, rds_mysql_item_from_telemetry,
    RESOURCE_TYPE as MYSQL_RDS_RESOURCE_TYPE,
};
use crate::services::analytics::mysql_analytics::redo_log_inventory::{
    evaluate_mysql_redo_log_inventory, redo_log_item_from_telemetry,
    RESOURCE_TYPE as MYSQL_REDO_LOG_RESOURCE_TYPE,
};
use crate::services::analytics::mysql_analytics::replication_status_inventory::{
    evaluate_mysql_replication_status_inventory, replication_status_item_from_telemetry,
    RESOURCE_TYPE as MYSQL_REPLICATION_STATUS_RESOURCE_TYPE,
};
use crate::services::analytics::mysql_analytics::slow_query_log_inventory::{
    evaluate_mysql_slow_query_log_inventory, slow_query_log_item_from_telemetry,
    RESOURCE_TYPE as MYSQL_SLOW_QUERY_LOG_RESOURCE_TYPE,
};
use crate::services::analytics::mysql_analytics::sys_schema_inventory::{
    evaluate_mysql_sys_schema_inventory, sys_schema_item_from_telemetry,
    RESOURCE_TYPE as MYSQL_SYS_SCHEMA_RESOURCE_TYPE,
};
use crate::services::analytics::mysql_analytics::undo_log_inventory::{
    evaluate_mysql_undo_log_inventory, undo_log_item_from_telemetry,
    RESOURCE_TYPE as MYSQL_UNDO_LOG_RESOURCE_TYPE,
};
use crate::services::analytics::mysql_analytics::wait_events_inventory::{
    evaluate_mysql_wait_events_inventory, wait_events_item_from_telemetry,
    RESOURCE_TYPE as MYSQL_WAIT_EVENTS_RESOURCE_TYPE,
};
use crate::services::analytics::postgres_analytics::postgres_analytics_service::PostgresAnalyticsService;
use crate::services::aws::inventory::types::{Pillar, DEFAULT_STALE_AFTER_HOURS};
use crate::services::database::DatabaseService;
use crate::utils::database::connect_to_dynamic_database;

#[derive(Debug, Deserialize)]
pub struct MySqlTelemetryHistoryQuery {
    pub hours: Option<i64>,
    pub limit: Option<u64>,
}

#[derive(Debug, Serialize)]
pub struct MySqlTelemetryHistoryPoint {
    pub id: Uuid,
    pub collected_at: DateTime<Utc>,
    pub qps_since_start: f64,
    pub threads_connected: i32,
    pub threads_running: i32,
    pub connection_usage_pct: Option<f64>,
    pub buffer_pool_hit_ratio: Option<f64>,
    pub slow_queries: i64,
    pub findings_count: i32,
    pub high_priority_findings_count: i32,
}

#[derive(Debug, Serialize)]
pub struct MySqlTelemetryHistoryResponse {
    pub snapshots: Vec<MySqlTelemetryHistoryPoint>,
    pub total: usize,
}

#[derive(Debug, Serialize)]
pub struct MySqlTelemetrySignalsResponse {
    pub signals: Vec<MySqlPerformanceSignal>,
    pub sample_count: usize,
    pub period_hours: i64,
}

#[derive(Debug, Deserialize)]
pub struct MySqlInventoryQuery {
    pub connection_id: Option<String>,
    pub pillar: Option<String>,
}

pub async fn execute_query(
    query_req: web::Json<DatabaseQueryRequest>,
    db_pool: web::Data<Arc<DatabaseConnection>>,
    config: web::Data<Config>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let db_repo = DatabaseRepository::new(db_pool.get_ref().clone(), config.get_ref().clone());

    // Get the database connection details
    let conn_id = uuid::Uuid::parse_str(&query_req.connection_id)
        .map_err(|e| AppError::BadRequest(format!("Invalid UUID: {}", e)))?;
    let conn_model = db_repo.find_by_id(conn_id).await?.ok_or_else(|| {
        AppError::NotFound(format!(
            "Database connection not found: {}",
            query_req.connection_id
        ))
    })?;

    // Execute the query with analysis if requested
    let analytics = MySqlAnalyticsService::new(config.get_ref().clone());
    let result = if query_req.explain.unwrap_or(false) {
        analytics
            .execute_query_with_explain(&conn_model, &query_req.query, query_req.params.as_ref())
            .await?
    } else {
        analytics
            .execute_query(&conn_model, &query_req.query, query_req.params.as_ref())
            .await?
    };

    Ok(HttpResponse::Ok().json(result))
}

pub async fn analyze_database(
    path: web::Path<String>,
    db_pool: web::Data<Arc<DatabaseConnection>>,
    config: web::Data<Config>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let db_repo = DatabaseRepository::new(db_pool.get_ref().clone(), config.get_ref().clone());

    // Get the database connection details to check if it exists
    let conn_id = uuid::Uuid::parse_str(&path.into_inner())
        .map_err(|e| AppError::BadRequest(format!("Invalid UUID: {}", e)))?;
    let conn_model = db_repo
        .find_by_id(conn_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Database connection not found".to_string()))?;

    // Log that we're analyzing the connection for debugging purposes
    tracing::info!("Analyzing database connection: {}", conn_model.name);

    let connection_type = conn_model.connection_type.to_lowercase();
    let analysis = match connection_type.as_str() {
        "mysql" => {
            let analytics = MySqlAnalyticsService::new(config.get_ref().clone());
            let dynamic_conn = connect_to_dynamic_database(&conn_model, config.get_ref()).await?;
            analytics.analyze_database(&dynamic_conn).await
        }
        "postgres" => {
            let analytics = PostgresAnalyticsService::new(config.get_ref().clone());
            let dynamic_conn = connect_to_dynamic_database(&conn_model, config.get_ref()).await?;
            analytics.analyze_database(&dynamic_conn).await
        }
        other => Err(AppError::BadRequest(format!(
            "Unsupported database type for analysis: {}",
            other
        ))),
    }?;

    Ok(HttpResponse::Ok().json(analysis))
}

pub async fn get_mysql_telemetry(
    path: web::Path<String>,
    db_pool: web::Data<Arc<DatabaseConnection>>,
    config: web::Data<Config>,
    claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let db_repo = DatabaseRepository::new(db_pool.get_ref().clone(), config.get_ref().clone());
    let conn_id = uuid::Uuid::parse_str(&path.into_inner())
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

    if conn_model.connection_type.to_lowercase() != "mysql" {
        return Err(AppError::BadRequest(
            "MySQL telemetry is only supported for mysql connections".to_string(),
        ));
    }

    let dynamic_conn = connect_to_dynamic_database(&conn_model, config.get_ref()).await?;
    let telemetry = MySqlTelemetryCollector::collect(&dynamic_conn).await?;
    let telemetry_repo = MySqlTelemetrySnapshotRepository::new(db_pool.get_ref().clone());
    if let Err(error) = telemetry_repo
        .create_from_snapshot(conn_id, &telemetry)
        .await
    {
        tracing::warn!(
            connection_id = %conn_id,
            error = %error,
            "Failed to persist MySQL telemetry snapshot"
        );
    }

    Ok(HttpResponse::Ok().json(telemetry))
}

pub async fn get_mysql_telemetry_history(
    path: web::Path<String>,
    query: web::Query<MySqlTelemetryHistoryQuery>,
    db_pool: web::Data<Arc<DatabaseConnection>>,
    config: web::Data<Config>,
    claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let db_repo = DatabaseRepository::new(db_pool.get_ref().clone(), config.get_ref().clone());
    let conn_id = uuid::Uuid::parse_str(&path.into_inner())
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

    if conn_model.connection_type.to_lowercase() != "mysql" {
        return Err(AppError::BadRequest(
            "MySQL telemetry history is only supported for mysql connections".to_string(),
        ));
    }

    let hours = query.hours.unwrap_or(24).clamp(1, 24 * 30);
    let limit = query.limit.unwrap_or(50).clamp(1, 500);
    let telemetry_repo = MySqlTelemetrySnapshotRepository::new(db_pool.get_ref().clone());
    let snapshots = telemetry_repo
        .find_recent_by_connection(conn_id, hours, limit)
        .await?;

    let points = snapshots
        .into_iter()
        .map(|snapshot| MySqlTelemetryHistoryPoint {
            id: snapshot.id,
            collected_at: snapshot.collected_at,
            qps_since_start: snapshot.qps_since_start,
            threads_connected: snapshot.threads_connected,
            threads_running: snapshot.threads_running,
            connection_usage_pct: snapshot.connection_usage_pct,
            buffer_pool_hit_ratio: snapshot.buffer_pool_hit_ratio,
            slow_queries: snapshot.slow_queries,
            findings_count: snapshot.findings_count,
            high_priority_findings_count: snapshot.high_priority_findings_count,
        })
        .collect::<Vec<_>>();
    let total = points.len();

    Ok(HttpResponse::Ok().json(MySqlTelemetryHistoryResponse {
        snapshots: points,
        total,
    }))
}

pub async fn get_mysql_telemetry_signals(
    path: web::Path<String>,
    query: web::Query<MySqlTelemetryHistoryQuery>,
    db_pool: web::Data<Arc<DatabaseConnection>>,
    config: web::Data<Config>,
    claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let db_repo = DatabaseRepository::new(db_pool.get_ref().clone(), config.get_ref().clone());
    let conn_id = uuid::Uuid::parse_str(&path.into_inner())
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
            "MySQL telemetry signals are only supported for mysql connections".to_string(),
        ));
    }

    let hours = query.hours.unwrap_or(24).clamp(1, 24 * 30);
    let limit = query.limit.unwrap_or(100).clamp(2, 500);
    let telemetry_repo = MySqlTelemetrySnapshotRepository::new(db_pool.get_ref().clone());
    let snapshots = telemetry_repo
        .find_recent_by_connection(conn_id, hours, limit)
        .await?;

    let signal_snapshots = snapshots
        .iter()
        .map(|snapshot| MySqlSignalSnapshot {
            id: snapshot.id,
            collected_at: snapshot.collected_at,
            qps_since_start: snapshot.qps_since_start,
            threads_connected: snapshot.threads_connected,
            threads_running: snapshot.threads_running,
            connection_usage_pct: snapshot.connection_usage_pct,
            buffer_pool_hit_ratio: snapshot.buffer_pool_hit_ratio,
            slow_queries: snapshot.slow_queries,
            findings_count: snapshot.findings_count,
            high_priority_findings_count: snapshot.high_priority_findings_count,
        })
        .collect::<Vec<_>>();
    let signals = MySqlSignalEvaluator::evaluate(&signal_snapshots, &MySqlSignalRules::default());

    Ok(HttpResponse::Ok().json(MySqlTelemetrySignalsResponse {
        signals,
        sample_count: signal_snapshots.len(),
        period_hours: hours,
    }))
}

pub async fn get_mysql_performance_schema_inventory_pillar_reports(
    query: web::Query<MySqlInventoryQuery>,
    db_pool: web::Data<Arc<DatabaseConnection>>,
    config: web::Data<Config>,
    claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let query = query.into_inner();
    let pillars = parse_mysql_inventory_pillars(&query.pillar, "MySQL Performance Schema")?;
    let connection_id = query
        .connection_id
        .as_deref()
        .map(str::trim)
        .filter(|connection_id| !connection_id.is_empty());

    let items = if let Some(connection_id) = connection_id {
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
                "MySQL Performance Schema inventory is only supported for mysql connections"
                    .to_string(),
            ));
        }

        let dynamic_conn = connect_to_dynamic_database(&conn_model, config.get_ref()).await?;
        let telemetry = MySqlTelemetryCollector::collect(&dynamic_conn).await?;
        vec![performance_schema_item_from_telemetry(
            &conn_model.id.to_string(),
            &conn_model.name,
            Some(conn_model.created_by.to_string()),
            BTreeMap::new(),
            &telemetry,
        )]
    } else {
        Vec::new()
    };

    let now = Utc::now();
    let reports = pillars
        .iter()
        .map(|pillar| evaluate_mysql_performance_schema_inventory(&items, *pillar, now))
        .collect::<Vec<_>>();
    let oldest_refresh = items.iter().map(|item| item.collected_at).min();

    Ok(HttpResponse::Ok().json(json!({
        "resource_type": MYSQL_PERFORMANCE_SCHEMA_RESOURCE_TYPE,
        "evaluated_at": now,
        "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
        "connection_id": query.connection_id,
        "resources_evaluated": items.len(),
        "oldest_refresh": oldest_refresh,
        "reports": reports,
    })))
}

pub async fn get_mysql_sys_schema_inventory_pillar_reports(
    query: web::Query<MySqlInventoryQuery>,
    db_pool: web::Data<Arc<DatabaseConnection>>,
    config: web::Data<Config>,
    claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let query = query.into_inner();
    let pillars = parse_mysql_inventory_pillars(&query.pillar, "MySQL sys schema")?;
    let connection_id = query
        .connection_id
        .as_deref()
        .map(str::trim)
        .filter(|connection_id| !connection_id.is_empty());

    let items = if let Some(connection_id) = connection_id {
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
                "MySQL sys schema inventory is only supported for mysql connections".to_string(),
            ));
        }

        let dynamic_conn = connect_to_dynamic_database(&conn_model, config.get_ref()).await?;
        let telemetry = MySqlTelemetryCollector::collect(&dynamic_conn).await?;
        vec![sys_schema_item_from_telemetry(
            &conn_model.id.to_string(),
            &conn_model.name,
            Some(conn_model.created_by.to_string()),
            BTreeMap::new(),
            &telemetry,
        )]
    } else {
        Vec::new()
    };

    let now = Utc::now();
    let reports = pillars
        .iter()
        .map(|pillar| evaluate_mysql_sys_schema_inventory(&items, *pillar, now))
        .collect::<Vec<_>>();
    let oldest_refresh = items.iter().map(|item| item.collected_at).min();

    Ok(HttpResponse::Ok().json(json!({
        "resource_type": MYSQL_SYS_SCHEMA_RESOURCE_TYPE,
        "evaluated_at": now,
        "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
        "connection_id": query.connection_id,
        "resources_evaluated": items.len(),
        "oldest_refresh": oldest_refresh,
        "reports": reports,
    })))
}

pub async fn get_mysql_slow_query_log_inventory_pillar_reports(
    query: web::Query<MySqlInventoryQuery>,
    db_pool: web::Data<Arc<DatabaseConnection>>,
    config: web::Data<Config>,
    claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let query = query.into_inner();
    let pillars = parse_mysql_inventory_pillars(&query.pillar, "MySQL slow query log")?;
    let connection_id = query
        .connection_id
        .as_deref()
        .map(str::trim)
        .filter(|connection_id| !connection_id.is_empty());

    let items = if let Some(connection_id) = connection_id {
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
                "MySQL slow query log inventory is only supported for mysql connections"
                    .to_string(),
            ));
        }

        let dynamic_conn = connect_to_dynamic_database(&conn_model, config.get_ref()).await?;
        let telemetry = MySqlTelemetryCollector::collect(&dynamic_conn).await?;
        vec![slow_query_log_item_from_telemetry(
            &conn_model.id.to_string(),
            &conn_model.name,
            Some(conn_model.created_by.to_string()),
            BTreeMap::new(),
            &telemetry,
        )]
    } else {
        Vec::new()
    };

    let now = Utc::now();
    let reports = pillars
        .iter()
        .map(|pillar| evaluate_mysql_slow_query_log_inventory(&items, *pillar, now))
        .collect::<Vec<_>>();
    let oldest_refresh = items.iter().map(|item| item.collected_at).min();

    Ok(HttpResponse::Ok().json(json!({
        "resource_type": MYSQL_SLOW_QUERY_LOG_RESOURCE_TYPE,
        "evaluated_at": now,
        "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
        "connection_id": query.connection_id,
        "resources_evaluated": items.len(),
        "oldest_refresh": oldest_refresh,
        "reports": reports,
    })))
}

pub async fn get_mysql_digest_statistics_inventory_pillar_reports(
    query: web::Query<MySqlInventoryQuery>,
    db_pool: web::Data<Arc<DatabaseConnection>>,
    config: web::Data<Config>,
    claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let query = query.into_inner();
    let pillars = parse_mysql_inventory_pillars(&query.pillar, "MySQL digest statistics")?;
    let connection_id = query
        .connection_id
        .as_deref()
        .map(str::trim)
        .filter(|connection_id| !connection_id.is_empty());

    let items = if let Some(connection_id) = connection_id {
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
                "MySQL digest statistics inventory is only supported for mysql connections"
                    .to_string(),
            ));
        }

        let dynamic_conn = connect_to_dynamic_database(&conn_model, config.get_ref()).await?;
        let telemetry = MySqlTelemetryCollector::collect(&dynamic_conn).await?;
        vec![digest_statistics_item_from_telemetry(
            &conn_model.id.to_string(),
            &conn_model.name,
            Some(conn_model.created_by.to_string()),
            BTreeMap::new(),
            &telemetry,
        )]
    } else {
        Vec::new()
    };

    let now = Utc::now();
    let reports = pillars
        .iter()
        .map(|pillar| evaluate_mysql_digest_statistics_inventory(&items, *pillar, now))
        .collect::<Vec<_>>();
    let oldest_refresh = items.iter().map(|item| item.collected_at).min();

    Ok(HttpResponse::Ok().json(json!({
        "resource_type": MYSQL_DIGEST_STATISTICS_RESOURCE_TYPE,
        "evaluated_at": now,
        "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
        "connection_id": query.connection_id,
        "resources_evaluated": items.len(),
        "oldest_refresh": oldest_refresh,
        "reports": reports,
    })))
}

pub async fn get_mysql_innodb_buffer_pool_inventory_pillar_reports(
    query: web::Query<MySqlInventoryQuery>,
    db_pool: web::Data<Arc<DatabaseConnection>>,
    config: web::Data<Config>,
    claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let query = query.into_inner();
    let pillars = parse_mysql_inventory_pillars(&query.pillar, "MySQL InnoDB buffer pool")?;
    let connection_id = query
        .connection_id
        .as_deref()
        .map(str::trim)
        .filter(|connection_id| !connection_id.is_empty());

    let items = if let Some(connection_id) = connection_id {
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
                "MySQL InnoDB buffer pool inventory is only supported for mysql connections"
                    .to_string(),
            ));
        }

        let dynamic_conn = connect_to_dynamic_database(&conn_model, config.get_ref()).await?;
        let telemetry = MySqlTelemetryCollector::collect(&dynamic_conn).await?;
        vec![innodb_buffer_pool_item_from_telemetry(
            &conn_model.id.to_string(),
            &conn_model.name,
            Some(conn_model.created_by.to_string()),
            BTreeMap::new(),
            &telemetry,
        )]
    } else {
        Vec::new()
    };

    let now = Utc::now();
    let reports = pillars
        .iter()
        .map(|pillar| evaluate_mysql_innodb_buffer_pool_inventory(&items, *pillar, now))
        .collect::<Vec<_>>();
    let oldest_refresh = items.iter().map(|item| item.collected_at).min();

    Ok(HttpResponse::Ok().json(json!({
        "resource_type": MYSQL_INNODB_BUFFER_POOL_RESOURCE_TYPE,
        "evaluated_at": now,
        "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
        "connection_id": query.connection_id,
        "resources_evaluated": items.len(),
        "oldest_refresh": oldest_refresh,
        "reports": reports,
    })))
}

pub async fn get_mysql_redo_log_inventory_pillar_reports(
    query: web::Query<MySqlInventoryQuery>,
    db_pool: web::Data<Arc<DatabaseConnection>>,
    config: web::Data<Config>,
    claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let query = query.into_inner();
    let pillars = parse_mysql_inventory_pillars(&query.pillar, "MySQL redo log")?;
    let connection_id = query
        .connection_id
        .as_deref()
        .map(str::trim)
        .filter(|connection_id| !connection_id.is_empty());

    let items = if let Some(connection_id) = connection_id {
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
                "MySQL redo log inventory is only supported for mysql connections".to_string(),
            ));
        }

        let dynamic_conn = connect_to_dynamic_database(&conn_model, config.get_ref()).await?;
        let telemetry = MySqlTelemetryCollector::collect(&dynamic_conn).await?;
        vec![redo_log_item_from_telemetry(
            &conn_model.id.to_string(),
            &conn_model.name,
            Some(conn_model.created_by.to_string()),
            BTreeMap::new(),
            &telemetry,
        )]
    } else {
        Vec::new()
    };

    let now = Utc::now();
    let reports = pillars
        .iter()
        .map(|pillar| evaluate_mysql_redo_log_inventory(&items, *pillar, now))
        .collect::<Vec<_>>();
    let oldest_refresh = items.iter().map(|item| item.collected_at).min();

    Ok(HttpResponse::Ok().json(json!({
        "resource_type": MYSQL_REDO_LOG_RESOURCE_TYPE,
        "evaluated_at": now,
        "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
        "connection_id": query.connection_id,
        "resources_evaluated": items.len(),
        "oldest_refresh": oldest_refresh,
        "reports": reports,
    })))
}

pub async fn get_mysql_binary_log_inventory_pillar_reports(
    query: web::Query<MySqlInventoryQuery>,
    db_pool: web::Data<Arc<DatabaseConnection>>,
    config: web::Data<Config>,
    claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let query = query.into_inner();
    let pillars = parse_mysql_inventory_pillars(&query.pillar, "MySQL binary log")?;
    let connection_id = query
        .connection_id
        .as_deref()
        .map(str::trim)
        .filter(|connection_id| !connection_id.is_empty());

    let items = if let Some(connection_id) = connection_id {
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
                "MySQL binary log inventory is only supported for mysql connections".to_string(),
            ));
        }

        let dynamic_conn = connect_to_dynamic_database(&conn_model, config.get_ref()).await?;
        let telemetry = MySqlTelemetryCollector::collect(&dynamic_conn).await?;
        vec![binary_log_item_from_telemetry(
            &conn_model.id.to_string(),
            &conn_model.name,
            Some(conn_model.created_by.to_string()),
            BTreeMap::new(),
            &telemetry,
        )]
    } else {
        Vec::new()
    };

    let now = Utc::now();
    let reports = pillars
        .iter()
        .map(|pillar| evaluate_mysql_binary_log_inventory(&items, *pillar, now))
        .collect::<Vec<_>>();
    let oldest_refresh = items.iter().map(|item| item.collected_at).min();

    Ok(HttpResponse::Ok().json(json!({
        "resource_type": MYSQL_BINARY_LOG_RESOURCE_TYPE,
        "evaluated_at": now,
        "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
        "connection_id": query.connection_id,
        "resources_evaluated": items.len(),
        "oldest_refresh": oldest_refresh,
        "reports": reports,
    })))
}

pub async fn get_mysql_replication_status_inventory_pillar_reports(
    query: web::Query<MySqlInventoryQuery>,
    db_pool: web::Data<Arc<DatabaseConnection>>,
    config: web::Data<Config>,
    claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let query = query.into_inner();
    let pillars = parse_mysql_inventory_pillars(&query.pillar, "MySQL replication status")?;
    let connection_id = query
        .connection_id
        .as_deref()
        .map(str::trim)
        .filter(|connection_id| !connection_id.is_empty());

    let items = if let Some(connection_id) = connection_id {
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
                "MySQL replication status inventory is only supported for mysql connections"
                    .to_string(),
            ));
        }

        let dynamic_conn = connect_to_dynamic_database(&conn_model, config.get_ref()).await?;
        let telemetry = MySqlTelemetryCollector::collect(&dynamic_conn).await?;
        vec![replication_status_item_from_telemetry(
            &conn_model.id.to_string(),
            &conn_model.name,
            Some(conn_model.created_by.to_string()),
            BTreeMap::new(),
            &telemetry,
        )]
    } else {
        Vec::new()
    };

    let now = Utc::now();
    let reports = pillars
        .iter()
        .map(|pillar| evaluate_mysql_replication_status_inventory(&items, *pillar, now))
        .collect::<Vec<_>>();
    let oldest_refresh = items.iter().map(|item| item.collected_at).min();

    Ok(HttpResponse::Ok().json(json!({
        "resource_type": MYSQL_REPLICATION_STATUS_RESOURCE_TYPE,
        "evaluated_at": now,
        "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
        "connection_id": query.connection_id,
        "resources_evaluated": items.len(),
        "oldest_refresh": oldest_refresh,
        "reports": reports,
    })))
}

pub async fn get_mysql_group_replication_inventory_pillar_reports(
    query: web::Query<MySqlInventoryQuery>,
    db_pool: web::Data<Arc<DatabaseConnection>>,
    config: web::Data<Config>,
    claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let query = query.into_inner();
    let pillars = parse_mysql_inventory_pillars(&query.pillar, "MySQL Group Replication")?;
    let connection_id = query
        .connection_id
        .as_deref()
        .map(str::trim)
        .filter(|connection_id| !connection_id.is_empty());

    let items = if let Some(connection_id) = connection_id {
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
                "MySQL Group Replication inventory is only supported for mysql connections"
                    .to_string(),
            ));
        }

        let dynamic_conn = connect_to_dynamic_database(&conn_model, config.get_ref()).await?;
        let telemetry = MySqlTelemetryCollector::collect(&dynamic_conn).await?;
        vec![group_replication_item_from_telemetry(
            &conn_model.id.to_string(),
            &conn_model.name,
            Some(conn_model.created_by.to_string()),
            BTreeMap::new(),
            &telemetry,
        )]
    } else {
        Vec::new()
    };

    let now = Utc::now();
    let reports = pillars
        .iter()
        .map(|pillar| evaluate_mysql_group_replication_inventory(&items, *pillar, now))
        .collect::<Vec<_>>();
    let oldest_refresh = items.iter().map(|item| item.collected_at).min();

    Ok(HttpResponse::Ok().json(json!({
        "resource_type": MYSQL_GROUP_REPLICATION_RESOURCE_TYPE,
        "evaluated_at": now,
        "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
        "connection_id": query.connection_id,
        "resources_evaluated": items.len(),
        "oldest_refresh": oldest_refresh,
        "reports": reports,
    })))
}

pub async fn get_mysql_aurora_inventory_pillar_reports(
    query: web::Query<MySqlInventoryQuery>,
    db_pool: web::Data<Arc<DatabaseConnection>>,
    config: web::Data<Config>,
    claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let query = query.into_inner();
    let pillars = parse_mysql_inventory_pillars(&query.pillar, "Aurora MySQL")?;
    let connection_id = query
        .connection_id
        .as_deref()
        .map(str::trim)
        .filter(|connection_id| !connection_id.is_empty());

    let items = if let Some(connection_id) = connection_id {
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
        if connection_type != "aurora-mysql" {
            return Err(AppError::BadRequest(
                "Aurora MySQL inventory is only supported for aurora-mysql connections".to_string(),
            ));
        }

        let dynamic_conn = connect_to_dynamic_database(&conn_model, config.get_ref()).await?;
        let telemetry = MySqlTelemetryCollector::collect(&dynamic_conn).await?;
        vec![aurora_mysql_item_from_telemetry(
            &conn_model.id.to_string(),
            &conn_model.name,
            Some(conn_model.created_by.to_string()),
            BTreeMap::new(),
            &telemetry,
        )]
    } else {
        Vec::new()
    };

    let now = Utc::now();
    let reports = pillars
        .iter()
        .map(|pillar| evaluate_mysql_aurora_inventory(&items, *pillar, now))
        .collect::<Vec<_>>();
    let oldest_refresh = items.iter().map(|item| item.collected_at).min();

    Ok(HttpResponse::Ok().json(json!({
        "resource_type": MYSQL_AURORA_RESOURCE_TYPE,
        "evaluated_at": now,
        "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
        "connection_id": query.connection_id,
        "resources_evaluated": items.len(),
        "oldest_refresh": oldest_refresh,
        "reports": reports,
    })))
}

pub async fn get_mysql_rds_inventory_pillar_reports(
    query: web::Query<MySqlInventoryQuery>,
    db_pool: web::Data<Arc<DatabaseConnection>>,
    config: web::Data<Config>,
    claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let query = query.into_inner();
    let pillars = parse_mysql_inventory_pillars(&query.pillar, "RDS MySQL")?;
    let connection_id = query
        .connection_id
        .as_deref()
        .map(str::trim)
        .filter(|connection_id| !connection_id.is_empty());

    let items = if let Some(connection_id) = connection_id {
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
        if connection_type != "mysql" && connection_type != "rds-mysql" {
            return Err(AppError::BadRequest(
                "RDS MySQL inventory is only supported for mysql or rds-mysql connections"
                    .to_string(),
            ));
        }

        let dynamic_conn = connect_to_dynamic_database(&conn_model, config.get_ref()).await?;
        let telemetry = MySqlTelemetryCollector::collect(&dynamic_conn).await?;
        vec![rds_mysql_item_from_telemetry(
            &conn_model.id.to_string(),
            &conn_model.name,
            Some(conn_model.created_by.to_string()),
            BTreeMap::new(),
            &telemetry,
        )]
    } else {
        Vec::new()
    };

    let now = Utc::now();
    let reports = pillars
        .iter()
        .map(|pillar| evaluate_mysql_rds_inventory(&items, *pillar, now))
        .collect::<Vec<_>>();
    let oldest_refresh = items.iter().map(|item| item.collected_at).min();

    Ok(HttpResponse::Ok().json(json!({
        "resource_type": MYSQL_RDS_RESOURCE_TYPE,
        "evaluated_at": now,
        "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
        "connection_id": query.connection_id,
        "resources_evaluated": items.len(),
        "oldest_refresh": oldest_refresh,
        "reports": reports,
    })))
}

pub async fn get_mysql_connection_threads_inventory_pillar_reports(
    query: web::Query<MySqlInventoryQuery>,
    db_pool: web::Data<Arc<DatabaseConnection>>,
    config: web::Data<Config>,
    claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let query = query.into_inner();
    let pillars = parse_mysql_inventory_pillars(&query.pillar, "connection threads")?;
    let connection_id = query
        .connection_id
        .as_deref()
        .map(str::trim)
        .filter(|connection_id| !connection_id.is_empty());

    let items = if let Some(connection_id) = connection_id {
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
                "Connection threads inventory is only supported for mysql or aurora-mysql connections"
                    .to_string(),
            ));
        }

        let dynamic_conn = connect_to_dynamic_database(&conn_model, config.get_ref()).await?;
        let telemetry = MySqlTelemetryCollector::collect(&dynamic_conn).await?;
        vec![connection_threads_item_from_telemetry(
            &conn_model.id.to_string(),
            &conn_model.name,
            Some(conn_model.created_by.to_string()),
            BTreeMap::new(),
            &telemetry,
        )]
    } else {
        Vec::new()
    };

    let now = Utc::now();
    let reports = pillars
        .iter()
        .map(|pillar| evaluate_mysql_connection_threads_inventory(&items, *pillar, now))
        .collect::<Vec<_>>();
    let oldest_refresh = items.iter().map(|item| item.collected_at).min();

    Ok(HttpResponse::Ok().json(json!({
        "resource_type": MYSQL_CONNECTION_THREADS_RESOURCE_TYPE,
        "evaluated_at": now,
        "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
        "connection_id": query.connection_id,
        "resources_evaluated": items.len(),
        "oldest_refresh": oldest_refresh,
        "reports": reports,
    })))
}

pub async fn get_mysql_metadata_locks_inventory_pillar_reports(
    query: web::Query<MySqlInventoryQuery>,
    db_pool: web::Data<Arc<DatabaseConnection>>,
    config: web::Data<Config>,
    claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let query = query.into_inner();
    let pillars = parse_mysql_inventory_pillars(&query.pillar, "metadata locks")?;
    let connection_id = query
        .connection_id
        .as_deref()
        .map(str::trim)
        .filter(|connection_id| !connection_id.is_empty());

    let items = if let Some(connection_id) = connection_id {
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
                "Metadata locks inventory is only supported for mysql or aurora-mysql connections"
                    .to_string(),
            ));
        }

        let dynamic_conn = connect_to_dynamic_database(&conn_model, config.get_ref()).await?;
        let telemetry = MySqlTelemetryCollector::collect(&dynamic_conn).await?;
        vec![metadata_locks_item_from_telemetry(
            &conn_model.id.to_string(),
            &conn_model.name,
            Some(conn_model.created_by.to_string()),
            BTreeMap::new(),
            &telemetry,
        )]
    } else {
        Vec::new()
    };

    let now = Utc::now();
    let reports = pillars
        .iter()
        .map(|pillar| evaluate_mysql_metadata_locks_inventory(&items, *pillar, now))
        .collect::<Vec<_>>();
    let oldest_refresh = items.iter().map(|item| item.collected_at).min();

    Ok(HttpResponse::Ok().json(json!({
        "resource_type": MYSQL_METADATA_LOCKS_RESOURCE_TYPE,
        "evaluated_at": now,
        "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
        "connection_id": query.connection_id,
        "resources_evaluated": items.len(),
        "oldest_refresh": oldest_refresh,
        "reports": reports,
    })))
}

pub async fn get_mysql_deadlocks_inventory_pillar_reports(
    query: web::Query<MySqlInventoryQuery>,
    db_pool: web::Data<Arc<DatabaseConnection>>,
    config: web::Data<Config>,
    claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let query = query.into_inner();
    let pillars = parse_mysql_inventory_pillars(&query.pillar, "deadlocks")?;
    let connection_id = query
        .connection_id
        .as_deref()
        .map(str::trim)
        .filter(|connection_id| !connection_id.is_empty());

    let items = if let Some(connection_id) = connection_id {
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
                "Deadlocks inventory is only supported for mysql or aurora-mysql connections"
                    .to_string(),
            ));
        }

        let dynamic_conn = connect_to_dynamic_database(&conn_model, config.get_ref()).await?;
        let telemetry = MySqlTelemetryCollector::collect(&dynamic_conn).await?;
        vec![deadlocks_item_from_telemetry(
            &conn_model.id.to_string(),
            &conn_model.name,
            Some(conn_model.created_by.to_string()),
            BTreeMap::new(),
            &telemetry,
        )]
    } else {
        Vec::new()
    };

    let now = Utc::now();
    let reports = pillars
        .iter()
        .map(|pillar| evaluate_mysql_deadlocks_inventory(&items, *pillar, now))
        .collect::<Vec<_>>();
    let oldest_refresh = items.iter().map(|item| item.collected_at).min();

    Ok(HttpResponse::Ok().json(json!({
        "resource_type": MYSQL_DEADLOCKS_RESOURCE_TYPE,
        "evaluated_at": now,
        "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
        "connection_id": query.connection_id,
        "resources_evaluated": items.len(),
        "oldest_refresh": oldest_refresh,
        "reports": reports,
    })))
}

pub async fn get_mysql_index_cardinality_inventory_pillar_reports(
    query: web::Query<MySqlInventoryQuery>,
    db_pool: web::Data<Arc<DatabaseConnection>>,
    config: web::Data<Config>,
    claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let query = query.into_inner();
    let pillars = parse_mysql_inventory_pillars(&query.pillar, "index cardinality")?;
    let connection_id = query
        .connection_id
        .as_deref()
        .map(str::trim)
        .filter(|connection_id| !connection_id.is_empty());

    let items = if let Some(connection_id) = connection_id {
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
                "Index cardinality inventory is only supported for mysql or aurora-mysql connections"
                    .to_string(),
            ));
        }

        let dynamic_conn = connect_to_dynamic_database(&conn_model, config.get_ref()).await?;
        let telemetry = MySqlTelemetryCollector::collect(&dynamic_conn).await?;
        vec![index_cardinality_item_from_telemetry(
            &conn_model.id.to_string(),
            &conn_model.name,
            Some(conn_model.created_by.to_string()),
            BTreeMap::new(),
            &telemetry,
        )]
    } else {
        Vec::new()
    };

    let now = Utc::now();
    let reports = pillars
        .iter()
        .map(|pillar| evaluate_mysql_index_cardinality_inventory(&items, *pillar, now))
        .collect::<Vec<_>>();
    let oldest_refresh = items.iter().map(|item| item.collected_at).min();

    Ok(HttpResponse::Ok().json(json!({
        "resource_type": MYSQL_INDEX_CARDINALITY_RESOURCE_TYPE,
        "evaluated_at": now,
        "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
        "connection_id": query.connection_id,
        "resources_evaluated": items.len(),
        "oldest_refresh": oldest_refresh,
        "reports": reports,
    })))
}

pub async fn get_mysql_undo_log_inventory_pillar_reports(
    query: web::Query<MySqlInventoryQuery>,
    db_pool: web::Data<Arc<DatabaseConnection>>,
    config: web::Data<Config>,
    claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let query = query.into_inner();
    let pillars = parse_mysql_inventory_pillars(&query.pillar, "MySQL undo log")?;
    let connection_id = query
        .connection_id
        .as_deref()
        .map(str::trim)
        .filter(|connection_id| !connection_id.is_empty());

    let items = if let Some(connection_id) = connection_id {
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
                "MySQL undo log inventory is only supported for mysql connections".to_string(),
            ));
        }

        let dynamic_conn = connect_to_dynamic_database(&conn_model, config.get_ref()).await?;
        let telemetry = MySqlTelemetryCollector::collect(&dynamic_conn).await?;
        vec![undo_log_item_from_telemetry(
            &conn_model.id.to_string(),
            &conn_model.name,
            Some(conn_model.created_by.to_string()),
            BTreeMap::new(),
            &telemetry,
        )]
    } else {
        Vec::new()
    };

    let now = Utc::now();
    let reports = pillars
        .iter()
        .map(|pillar| evaluate_mysql_undo_log_inventory(&items, *pillar, now))
        .collect::<Vec<_>>();
    let oldest_refresh = items.iter().map(|item| item.collected_at).min();

    Ok(HttpResponse::Ok().json(json!({
        "resource_type": MYSQL_UNDO_LOG_RESOURCE_TYPE,
        "evaluated_at": now,
        "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
        "connection_id": query.connection_id,
        "resources_evaluated": items.len(),
        "oldest_refresh": oldest_refresh,
        "reports": reports,
    })))
}

pub async fn get_mysql_wait_events_inventory_pillar_reports(
    query: web::Query<MySqlInventoryQuery>,
    db_pool: web::Data<Arc<DatabaseConnection>>,
    config: web::Data<Config>,
    claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let query = query.into_inner();
    let pillars = parse_mysql_inventory_pillars(&query.pillar, "MySQL wait events")?;
    let connection_id = query
        .connection_id
        .as_deref()
        .map(str::trim)
        .filter(|connection_id| !connection_id.is_empty());

    let items = if let Some(connection_id) = connection_id {
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
                "MySQL wait events inventory is only supported for mysql connections".to_string(),
            ));
        }

        let dynamic_conn = connect_to_dynamic_database(&conn_model, config.get_ref()).await?;
        let telemetry = MySqlTelemetryCollector::collect(&dynamic_conn).await?;
        vec![wait_events_item_from_telemetry(
            &conn_model.id.to_string(),
            &conn_model.name,
            Some(conn_model.created_by.to_string()),
            BTreeMap::new(),
            &telemetry,
        )]
    } else {
        Vec::new()
    };

    let now = Utc::now();
    let reports = pillars
        .iter()
        .map(|pillar| evaluate_mysql_wait_events_inventory(&items, *pillar, now))
        .collect::<Vec<_>>();
    let oldest_refresh = items.iter().map(|item| item.collected_at).min();

    Ok(HttpResponse::Ok().json(json!({
        "resource_type": MYSQL_WAIT_EVENTS_RESOURCE_TYPE,
        "evaluated_at": now,
        "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
        "connection_id": query.connection_id,
        "resources_evaluated": items.len(),
        "oldest_refresh": oldest_refresh,
        "reports": reports,
    })))
}

fn parse_mysql_inventory_pillars(
    requested: &Option<String>,
    resource_label: &str,
) -> Result<Vec<Pillar>, AppError> {
    match requested
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        None => Ok(vec![Pillar::Cost, Pillar::Resilience, Pillar::Security]),
        Some(value) => {
            let mut pillars = Vec::new();
            for token in value
                .split(',')
                .map(str::trim)
                .filter(|token| !token.is_empty())
            {
                let pillar = Pillar::parse(token).ok_or_else(|| {
                    AppError::BadRequest(format!("Unsupported MySQL inventory pillar: {}", token))
                })?;
                match pillar {
                    Pillar::Cost | Pillar::Resilience | Pillar::Security => {
                        if !pillars.contains(&pillar) {
                            pillars.push(pillar);
                        }
                    }
                    _ => {
                        return Err(AppError::BadRequest(format!(
                            "Unsupported {} inventory pillar: {}",
                            resource_label, token
                        )));
                    }
                }
            }
            if pillars.is_empty() {
                Ok(vec![Pillar::Cost, Pillar::Resilience, Pillar::Security])
            } else {
                Ok(pillars)
            }
        }
    }
}

pub async fn list_connections(
    db_pool: web::Data<Arc<DatabaseConnection>>,
    config: web::Data<Config>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let db_repo = DatabaseRepository::new(db_pool.get_ref().clone(), config.get_ref().clone());
    let connections = db_repo.find_all().await?;

    Ok(HttpResponse::Ok().json(connections))
}

pub async fn get_connection(
    path: web::Path<String>,
    db_pool: web::Data<Arc<DatabaseConnection>>,
    config: web::Data<Config>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let db_repo = DatabaseRepository::new(db_pool.get_ref().clone(), config.get_ref().clone());

    let conn_id = uuid::Uuid::parse_str(&path.into_inner())
        .map_err(|e| AppError::BadRequest(format!("Invalid UUID: {}", e)))?;
    let connection = db_repo
        .find_by_id(conn_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Database connection not found".to_string()))?;

    Ok(HttpResponse::Ok().json(connection))
}

pub async fn create_connection(
    connection: web::Json<CreateDatabaseConnectionRequest>,
    db_pool: web::Data<Arc<DatabaseConnection>>,
    config: web::Data<Config>,
    claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let db_repo = DatabaseRepository::new(db_pool.get_ref().clone(), config.get_ref().clone());

    // Create the database connection
    let user_id = uuid::Uuid::parse_str(&claims.sub)
        .map_err(|e| AppError::BadRequest(format!("Invalid UUID: {}", e)))?;
    let new_connection = db_repo.create(&connection, user_id).await?;

    Ok(HttpResponse::Created().json(new_connection))
}

pub async fn update_connection(
    path: web::Path<String>,
    connection: web::Json<CreateDatabaseConnectionRequest>,
    db_pool: web::Data<Arc<DatabaseConnection>>,
    config: web::Data<Config>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let db_repo = DatabaseRepository::new(db_pool.get_ref().clone(), config.get_ref().clone());

    // Update the database connection
    let conn_id = uuid::Uuid::parse_str(&path.into_inner())
        .map_err(|e| AppError::BadRequest(format!("Invalid UUID: {}", e)))?;
    let updated_connection = db_repo.update(conn_id, &connection).await?;

    Ok(HttpResponse::Ok().json(updated_connection))
}

pub async fn delete_connection(
    path: web::Path<String>,
    db_pool: web::Data<Arc<DatabaseConnection>>,
    config: web::Data<Config>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let db_repo = DatabaseRepository::new(db_pool.get_ref().clone(), config.get_ref().clone());

    // Delete the database connection
    let conn_id = uuid::Uuid::parse_str(&path.into_inner())
        .map_err(|e| AppError::BadRequest(format!("Invalid UUID: {}", e)))?;
    db_repo.delete(conn_id).await?;

    Ok(HttpResponse::NoContent().finish())
}

pub async fn test_connection(
    path: web::Path<String>,
    db_pool: web::Data<Arc<DatabaseConnection>>,
    config: web::Data<Config>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let db_service = DatabaseService::new(config.get_ref().clone());
    let db_repo = DatabaseRepository::new(db_pool.get_ref().clone(), config.get_ref().clone());

    // Get the database connection details
    let conn_id = uuid::Uuid::parse_str(&path.into_inner())
        .map_err(|e| AppError::BadRequest(format!("Invalid UUID: {}", e)))?;
    let conn = db_repo
        .find_by_id(conn_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Database connection not found".to_string()))?;

    // Test the connection
    let test_result = db_service.test_connection(&conn).await?;

    Ok(HttpResponse::Ok().json(test_result))
}

pub async fn get_schema(
    path: web::Path<String>,
    db_pool: web::Data<Arc<DatabaseConnection>>,
    config: web::Data<Config>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let db_service = DatabaseService::new(config.get_ref().clone());
    let db_repo = DatabaseRepository::new(db_pool.get_ref().clone(), config.get_ref().clone());

    // Get the database connection details
    let conn_id = uuid::Uuid::parse_str(&path.into_inner())
        .map_err(|e| AppError::BadRequest(format!("Invalid UUID: {}", e)))?;
    let conn = db_repo
        .find_by_id(conn_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Database connection not found".to_string()))?;

    // Get the database schema
    let schema = db_service.get_schema(&conn).await?;

    Ok(HttpResponse::Ok().json(schema))
}

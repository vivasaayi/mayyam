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
use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, Statement};
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
use crate::repositories::prompt_template::PromptTemplateRepository;
use crate::services::analytics::mysql_analytics::ai_prompt_templates_inventory::{
    ai_prompt_template_item_from_model, evaluate_mysql_ai_prompt_templates_inventory,
    RESOURCE_TYPE as MYSQL_AI_PROMPT_TEMPLATE_RESOURCE_TYPE,
};
use crate::services::analytics::mysql_analytics::aurora_mysql_inventory::{
    aurora_mysql_item_from_telemetry, evaluate_mysql_aurora_inventory,
    RESOURCE_TYPE as MYSQL_AURORA_RESOURCE_TYPE,
};
use crate::services::analytics::mysql_analytics::backup_posture_inventory::{
    backup_posture_item_from_telemetry, evaluate_mysql_backup_posture_inventory,
    RESOURCE_TYPE as MYSQL_BACKUP_POSTURE_RESOURCE_TYPE,
};
use crate::services::analytics::mysql_analytics::binary_log_health::{
    binary_log_health_item_from_telemetry, evaluate_mysql_binary_log_health,
    RESOURCE_TYPE as MYSQL_BINARY_LOG_HEALTH_RESOURCE_TYPE,
};
use crate::services::analytics::mysql_analytics::binary_log_inventory::{
    binary_log_item_from_telemetry, evaluate_mysql_binary_log_inventory,
    RESOURCE_TYPE as MYSQL_BINARY_LOG_RESOURCE_TYPE,
};
use crate::services::analytics::mysql_analytics::connection_threads_inventory::{
    connection_threads_item_from_telemetry, evaluate_mysql_connection_threads_inventory,
    RESOURCE_TYPE as MYSQL_CONNECTION_THREADS_RESOURCE_TYPE,
};
use crate::services::analytics::mysql_analytics::cost_attribution_inventory::{
    cost_attribution_item_from_telemetry, evaluate_mysql_cost_attribution_inventory,
    RESOURCE_TYPE as MYSQL_COST_ATTRIBUTION_RESOURCE_TYPE,
};
use crate::services::analytics::mysql_analytics::deadlocks_inventory::{
    deadlocks_item_from_telemetry, evaluate_mysql_deadlocks_inventory,
    RESOURCE_TYPE as MYSQL_DEADLOCKS_RESOURCE_TYPE,
};
use crate::services::analytics::mysql_analytics::digest_statistics_health::{
    digest_statistics_health_item_from_telemetry, evaluate_mysql_digest_statistics_health,
    RESOURCE_TYPE as MYSQL_DIGEST_STATISTICS_HEALTH_RESOURCE_TYPE,
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
use crate::services::analytics::mysql_analytics::innodb_buffer_pool_health::{
    evaluate_mysql_innodb_buffer_pool_health, innodb_buffer_pool_health_item_from_telemetry,
    RESOURCE_TYPE as MYSQL_INNODB_BUFFER_POOL_HEALTH_RESOURCE_TYPE,
};
use crate::services::analytics::mysql_analytics::innodb_buffer_pool_inventory::{
    evaluate_mysql_innodb_buffer_pool_inventory, innodb_buffer_pool_item_from_telemetry,
    RESOURCE_TYPE as MYSQL_INNODB_BUFFER_POOL_RESOURCE_TYPE,
};
use crate::services::analytics::mysql_analytics::join_buffers_inventory::{
    evaluate_mysql_join_buffers_inventory, join_buffers_item_from_telemetry,
    RESOURCE_TYPE as MYSQL_JOIN_BUFFERS_RESOURCE_TYPE,
};
use crate::services::analytics::mysql_analytics::metadata_locks_inventory::{
    evaluate_mysql_metadata_locks_inventory, metadata_locks_item_from_telemetry,
    RESOURCE_TYPE as MYSQL_METADATA_LOCKS_RESOURCE_TYPE,
};
use crate::services::analytics::mysql_analytics::missing_indexes_inventory::{
    evaluate_mysql_missing_indexes_inventory, missing_indexes_item_from_telemetry,
    RESOURCE_TYPE as MYSQL_MISSING_INDEXES_RESOURCE_TYPE,
};
use crate::services::analytics::mysql_analytics::mysql_analytics_service::MySqlAnalyticsService;
use crate::services::analytics::mysql_analytics::mysql_signals::{
    MySqlPerformanceSignal, MySqlSignalEvaluator, MySqlSignalRules, MySqlSignalSnapshot,
};
use crate::services::analytics::mysql_analytics::mysql_telemetry::MySqlTelemetryCollector;
use crate::services::analytics::mysql_analytics::parameter_drift_inventory::{
    evaluate_mysql_parameter_drift_inventory, parameter_drift_item_from_telemetry,
    RESOURCE_TYPE as MYSQL_PARAMETER_DRIFT_RESOURCE_TYPE,
};
use crate::services::analytics::mysql_analytics::partitioning_inventory::{
    evaluate_mysql_partitioning_inventory, partitioning_item_from_telemetry,
    RESOURCE_TYPE as MYSQL_PARTITIONING_RESOURCE_TYPE,
};
use crate::services::analytics::mysql_analytics::performance_schema_health::{
    evaluate_mysql_performance_schema_health, performance_schema_health_item_from_telemetry,
    RESOURCE_TYPE as MYSQL_PERFORMANCE_SCHEMA_HEALTH_RESOURCE_TYPE,
};
use crate::services::analytics::mysql_analytics::performance_schema_inventory::{
    evaluate_mysql_performance_schema_inventory, performance_schema_item_from_telemetry,
    RESOURCE_TYPE as MYSQL_PERFORMANCE_SCHEMA_RESOURCE_TYPE,
};
use crate::services::analytics::mysql_analytics::privilege_audit_inventory::{
    evaluate_mysql_privilege_audit_inventory, privilege_audit_item_from_telemetry,
    RESOURCE_TYPE as MYSQL_PRIVILEGE_AUDIT_RESOURCE_TYPE,
};
use crate::services::analytics::mysql_analytics::query_plans_inventory::{
    evaluate_mysql_query_plans_inventory, query_plans_item_from_telemetry,
    RESOURCE_TYPE as MYSQL_QUERY_PLANS_RESOURCE_TYPE,
};
use crate::services::analytics::mysql_analytics::rds_mysql_inventory::{
    evaluate_mysql_rds_inventory, rds_mysql_item_from_telemetry,
    RESOURCE_TYPE as MYSQL_RDS_RESOURCE_TYPE,
};
use crate::services::analytics::mysql_analytics::redo_log_health::{
    evaluate_mysql_redo_log_health, redo_log_health_item_from_telemetry,
    RESOURCE_TYPE as MYSQL_REDO_LOG_HEALTH_RESOURCE_TYPE,
};
use crate::services::analytics::mysql_analytics::redo_log_inventory::{
    evaluate_mysql_redo_log_inventory, redo_log_item_from_telemetry,
    RESOURCE_TYPE as MYSQL_REDO_LOG_RESOURCE_TYPE,
};
use crate::services::analytics::mysql_analytics::replication_status_inventory::{
    evaluate_mysql_replication_status_inventory, replication_status_item_from_telemetry,
    RESOURCE_TYPE as MYSQL_REPLICATION_STATUS_RESOURCE_TYPE,
};
use crate::services::analytics::mysql_analytics::restore_drills_inventory::{
    evaluate_mysql_restore_drills_inventory, restore_drill_item_from_telemetry,
    RESOURCE_TYPE as MYSQL_RESTORE_DRILLS_RESOURCE_TYPE,
};
use crate::services::analytics::mysql_analytics::schema_explorer_inventory::{
    evaluate_mysql_schema_explorer_inventory, schema_explorer_item_from_telemetry,
    RESOURCE_TYPE as MYSQL_SCHEMA_EXPLORER_RESOURCE_TYPE,
};
use crate::services::analytics::mysql_analytics::slow_query_log_health::{
    evaluate_mysql_slow_query_log_health, slow_query_log_health_item_from_telemetry,
    RESOURCE_TYPE as MYSQL_SLOW_QUERY_LOG_HEALTH_RESOURCE_TYPE,
};
use crate::services::analytics::mysql_analytics::slow_query_log_inventory::{
    evaluate_mysql_slow_query_log_inventory, slow_query_log_item_from_telemetry,
    RESOURCE_TYPE as MYSQL_SLOW_QUERY_LOG_RESOURCE_TYPE,
};
use crate::services::analytics::mysql_analytics::sort_operations_inventory::{
    evaluate_mysql_sort_operations_inventory, sort_operations_item_from_telemetry,
    RESOURCE_TYPE as MYSQL_SORT_OPERATIONS_RESOURCE_TYPE,
};
use crate::services::analytics::mysql_analytics::sys_schema_health::{
    evaluate_mysql_sys_schema_health, sys_schema_health_item_from_telemetry,
    RESOURCE_TYPE as MYSQL_SYS_SCHEMA_HEALTH_RESOURCE_TYPE,
};
use crate::services::analytics::mysql_analytics::sys_schema_inventory::{
    evaluate_mysql_sys_schema_inventory, sys_schema_item_from_telemetry,
    RESOURCE_TYPE as MYSQL_SYS_SCHEMA_RESOURCE_TYPE,
};
use crate::services::analytics::mysql_analytics::table_bloat_inventory::{
    evaluate_mysql_table_bloat_inventory, table_bloat_item_from_telemetry,
    RESOURCE_TYPE as MYSQL_TABLE_BLOAT_RESOURCE_TYPE,
};
use crate::services::analytics::mysql_analytics::temporary_tables_inventory::{
    evaluate_mysql_temporary_tables_inventory, temporary_tables_item_from_telemetry,
    RESOURCE_TYPE as MYSQL_TEMPORARY_TABLES_RESOURCE_TYPE,
};
use crate::services::analytics::mysql_analytics::tls_configuration_inventory::{
    evaluate_mysql_tls_configuration_inventory, tls_configuration_item_from_telemetry,
    RESOURCE_TYPE as MYSQL_TLS_CONFIGURATION_RESOURCE_TYPE,
};
use crate::services::analytics::mysql_analytics::undo_log_health::{
    evaluate_mysql_undo_log_health, undo_log_health_item_from_telemetry,
    RESOURCE_TYPE as MYSQL_UNDO_LOG_HEALTH_RESOURCE_TYPE,
};
use crate::services::analytics::mysql_analytics::undo_log_inventory::{
    evaluate_mysql_undo_log_inventory, undo_log_item_from_telemetry,
    RESOURCE_TYPE as MYSQL_UNDO_LOG_RESOURCE_TYPE,
};
use crate::services::analytics::mysql_analytics::unused_indexes_inventory::{
    evaluate_mysql_unused_indexes_inventory, unused_indexes_item_from_telemetry,
    RESOURCE_TYPE as MYSQL_UNUSED_INDEXES_RESOURCE_TYPE,
};
use crate::services::analytics::mysql_analytics::wait_events_health::{
    evaluate_mysql_wait_events_health, wait_events_health_item_from_telemetry,
    RESOURCE_TYPE as MYSQL_WAIT_EVENTS_HEALTH_RESOURCE_TYPE,
};
use crate::services::analytics::mysql_analytics::wait_events_inventory::{
    evaluate_mysql_wait_events_inventory, wait_events_item_from_telemetry,
    RESOURCE_TYPE as MYSQL_WAIT_EVENTS_RESOURCE_TYPE,
};
use crate::services::analytics::postgres_analytics::pg_stat_activity_inventory::{
    evaluate_postgres_pg_stat_activity_inventory, PgStatActivityInventoryItem,
    RESOURCE_TYPE as POSTGRES_PG_STAT_ACTIVITY_RESOURCE_TYPE,
};
use crate::services::analytics::postgres_analytics::pg_stat_database_inventory::{
    evaluate_postgres_pg_stat_database_inventory, PgStatDatabaseExposureEvidence,
    PgStatDatabaseInventoryItem, RESOURCE_TYPE as POSTGRES_PG_STAT_DATABASE_RESOURCE_TYPE,
};
use crate::services::analytics::postgres_analytics::pg_stat_io_inventory::{
    evaluate_postgres_pg_stat_io_inventory, PgStatIoInventoryItem,
    RESOURCE_TYPE as POSTGRES_PG_STAT_IO_RESOURCE_TYPE,
};
use crate::services::analytics::postgres_analytics::pg_stat_statements_inventory::{
    evaluate_postgres_pg_stat_statements_inventory, PgStatStatementsInventoryItem,
    RESOURCE_TYPE as POSTGRES_PG_STAT_STATEMENTS_RESOURCE_TYPE,
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

pub async fn get_postgres_pg_stat_activity_inventory_pillar_reports(
    query: web::Query<MySqlInventoryQuery>,
    db_pool: web::Data<Arc<DatabaseConnection>>,
    config: web::Data<Config>,
    claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let query = query.into_inner();
    let pillars = parse_postgres_inventory_pillars(&query.pillar, "PostgreSQL pg_stat_activity")?;
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
        if connection_type != "postgres" && connection_type != "postgresql" {
            return Err(AppError::BadRequest(
                "PostgreSQL pg_stat_activity inventory is only supported for postgres connections"
                    .to_string(),
            ));
        }

        let dynamic_conn = connect_to_dynamic_database(&conn_model, config.get_ref()).await?;
        vec![
            pg_stat_activity_item_from_connection(
                &dynamic_conn,
                &conn_model.id.to_string(),
                &conn_model.name,
                Some(conn_model.created_by.to_string()),
            )
            .await?,
        ]
    } else {
        Vec::new()
    };

    let now = Utc::now();
    let reports = pillars
        .iter()
        .map(|pillar| evaluate_postgres_pg_stat_activity_inventory(&items, *pillar, now))
        .collect::<Vec<_>>();
    let oldest_refresh = items.iter().map(|item| item.collected_at).min();
    let stale_resources = reports
        .iter()
        .map(|report| report.stale_resources)
        .max()
        .unwrap_or(0);

    Ok(HttpResponse::Ok().json(json!({
        "resource_type": POSTGRES_PG_STAT_ACTIVITY_RESOURCE_TYPE,
        "evaluated_at": now,
        "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
        "connection_id": query.connection_id,
        "resources_evaluated": items.len(),
        "stale_resources": stale_resources,
        "oldest_refresh": oldest_refresh,
        "reports": reports,
    })))
}

pub async fn get_postgres_pg_stat_statements_inventory_pillar_reports(
    query: web::Query<MySqlInventoryQuery>,
    db_pool: web::Data<Arc<DatabaseConnection>>,
    config: web::Data<Config>,
    claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let query = query.into_inner();
    let pillars = parse_postgres_inventory_pillars(&query.pillar, "PostgreSQL pg_stat_statements")?;
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
        if connection_type != "postgres" && connection_type != "postgresql" {
            return Err(AppError::BadRequest(
                "PostgreSQL pg_stat_statements inventory is only supported for postgres connections"
                    .to_string(),
            ));
        }

        let dynamic_conn = connect_to_dynamic_database(&conn_model, config.get_ref()).await?;
        vec![
            pg_stat_statements_item_from_connection(
                &dynamic_conn,
                &conn_model.id.to_string(),
                &conn_model.name,
                Some(conn_model.created_by.to_string()),
            )
            .await?,
        ]
    } else {
        Vec::new()
    };

    let now = Utc::now();
    let reports = pillars
        .iter()
        .map(|pillar| evaluate_postgres_pg_stat_statements_inventory(&items, *pillar, now))
        .collect::<Vec<_>>();
    let oldest_refresh = items.iter().map(|item| item.collected_at).min();
    let stale_resources = reports
        .iter()
        .map(|report| report.stale_resources)
        .max()
        .unwrap_or(0);

    Ok(HttpResponse::Ok().json(json!({
        "resource_type": POSTGRES_PG_STAT_STATEMENTS_RESOURCE_TYPE,
        "evaluated_at": now,
        "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
        "connection_id": query.connection_id,
        "resources_evaluated": items.len(),
        "stale_resources": stale_resources,
        "oldest_refresh": oldest_refresh,
        "reports": reports,
    })))
}

pub async fn get_postgres_pg_stat_database_inventory_pillar_reports(
    query: web::Query<MySqlInventoryQuery>,
    db_pool: web::Data<Arc<DatabaseConnection>>,
    config: web::Data<Config>,
    claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let query = query.into_inner();
    let pillars = parse_postgres_inventory_pillars(&query.pillar, "PostgreSQL pg_stat_database")?;
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
        if connection_type != "postgres" && connection_type != "postgresql" {
            return Err(AppError::BadRequest(
                "PostgreSQL pg_stat_database inventory is only supported for postgres connections"
                    .to_string(),
            ));
        }

        let dynamic_conn = connect_to_dynamic_database(&conn_model, config.get_ref()).await?;
        pg_stat_database_items_from_connection(
            &dynamic_conn,
            &conn_model.id.to_string(),
            &conn_model.name,
            Some(conn_model.created_by.to_string()),
        )
        .await?
    } else {
        Vec::new()
    };

    let now = Utc::now();
    let reports = pillars
        .iter()
        .map(|pillar| evaluate_postgres_pg_stat_database_inventory(&items, *pillar, now))
        .collect::<Vec<_>>();
    let oldest_refresh = items.iter().map(|item| item.collected_at).min();
    let stale_resources = reports
        .iter()
        .map(|report| report.stale_resources)
        .max()
        .unwrap_or(0);

    Ok(HttpResponse::Ok().json(json!({
        "resource_type": POSTGRES_PG_STAT_DATABASE_RESOURCE_TYPE,
        "evaluated_at": now,
        "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
        "connection_id": query.connection_id,
        "resources_evaluated": items.len(),
        "stale_resources": stale_resources,
        "oldest_refresh": oldest_refresh,
        "reports": reports,
    })))
}

pub async fn get_postgres_pg_stat_io_inventory_pillar_reports(
    query: web::Query<MySqlInventoryQuery>,
    db_pool: web::Data<Arc<DatabaseConnection>>,
    config: web::Data<Config>,
    claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let query = query.into_inner();
    let pillars = parse_postgres_inventory_pillars(&query.pillar, "PostgreSQL pg_stat_io")?;
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
        if connection_type != "postgres" && connection_type != "postgresql" {
            return Err(AppError::BadRequest(
                "PostgreSQL pg_stat_io inventory is only supported for postgres connections"
                    .to_string(),
            ));
        }

        let dynamic_conn = connect_to_dynamic_database(&conn_model, config.get_ref()).await?;
        pg_stat_io_items_from_connection(
            &dynamic_conn,
            &conn_model.id.to_string(),
            &conn_model.name,
            Some(conn_model.created_by.to_string()),
        )
        .await?
    } else {
        Vec::new()
    };

    let now = Utc::now();
    let reports = pillars
        .iter()
        .map(|pillar| evaluate_postgres_pg_stat_io_inventory(&items, *pillar, now))
        .collect::<Vec<_>>();
    let oldest_refresh = items.iter().map(|item| item.collected_at).min();
    let stale_resources = reports
        .iter()
        .map(|report| report.stale_resources)
        .max()
        .unwrap_or(0);

    Ok(HttpResponse::Ok().json(json!({
        "resource_type": POSTGRES_PG_STAT_IO_RESOURCE_TYPE,
        "evaluated_at": now,
        "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
        "connection_id": query.connection_id,
        "resources_evaluated": items.len(),
        "stale_resources": stale_resources,
        "oldest_refresh": oldest_refresh,
        "reports": reports,
    })))
}

pub async fn get_mysql_performance_schema_health_pillar_reports(
    query: web::Query<MySqlInventoryQuery>,
    db_pool: web::Data<Arc<DatabaseConnection>>,
    config: web::Data<Config>,
    claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let query = query.into_inner();
    let pillars = parse_mysql_inventory_pillars(&query.pillar, "MySQL Performance Schema health")?;
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
                "MySQL Performance Schema health is only supported for mysql connections"
                    .to_string(),
            ));
        }

        let dynamic_conn = connect_to_dynamic_database(&conn_model, config.get_ref()).await?;
        let telemetry = MySqlTelemetryCollector::collect(&dynamic_conn).await?;
        vec![performance_schema_health_item_from_telemetry(
            &conn_model.id.to_string(),
            &conn_model.name,
            &telemetry,
        )]
    } else {
        Vec::new()
    };

    let now = Utc::now();
    let reports = pillars
        .iter()
        .map(|pillar| evaluate_mysql_performance_schema_health(&items, *pillar, now))
        .collect::<Vec<_>>();
    let oldest_refresh = items.iter().map(|item| item.collected_at).min();

    Ok(HttpResponse::Ok().json(json!({
        "resource_type": MYSQL_PERFORMANCE_SCHEMA_HEALTH_RESOURCE_TYPE,
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

pub async fn get_mysql_sys_schema_health_pillar_reports(
    query: web::Query<MySqlInventoryQuery>,
    db_pool: web::Data<Arc<DatabaseConnection>>,
    config: web::Data<Config>,
    claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let query = query.into_inner();
    let pillars = parse_mysql_inventory_pillars(&query.pillar, "MySQL sys schema health")?;
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
                "MySQL sys schema health is only supported for mysql connections".to_string(),
            ));
        }

        let dynamic_conn = connect_to_dynamic_database(&conn_model, config.get_ref()).await?;
        let telemetry = MySqlTelemetryCollector::collect(&dynamic_conn).await?;
        vec![sys_schema_health_item_from_telemetry(
            &conn_model.id.to_string(),
            &conn_model.name,
            &telemetry,
        )]
    } else {
        Vec::new()
    };

    let now = Utc::now();
    let reports = pillars
        .iter()
        .map(|pillar| evaluate_mysql_sys_schema_health(&items, *pillar, now))
        .collect::<Vec<_>>();
    let oldest_refresh = items.iter().map(|item| item.collected_at).min();

    Ok(HttpResponse::Ok().json(json!({
        "resource_type": MYSQL_SYS_SCHEMA_HEALTH_RESOURCE_TYPE,
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

pub async fn get_mysql_slow_query_log_health_pillar_reports(
    query: web::Query<MySqlInventoryQuery>,
    db_pool: web::Data<Arc<DatabaseConnection>>,
    config: web::Data<Config>,
    claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let query = query.into_inner();
    let pillars = parse_mysql_inventory_pillars(&query.pillar, "MySQL slow query log health")?;
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
                "MySQL slow query log health is only supported for mysql connections".to_string(),
            ));
        }

        let dynamic_conn = connect_to_dynamic_database(&conn_model, config.get_ref()).await?;
        let telemetry = MySqlTelemetryCollector::collect(&dynamic_conn).await?;
        vec![slow_query_log_health_item_from_telemetry(
            &conn_model.id.to_string(),
            &conn_model.name,
            &telemetry,
        )]
    } else {
        Vec::new()
    };

    let now = Utc::now();
    let reports = pillars
        .iter()
        .map(|pillar| evaluate_mysql_slow_query_log_health(&items, *pillar, now))
        .collect::<Vec<_>>();
    let oldest_refresh = items.iter().map(|item| item.collected_at).min();

    Ok(HttpResponse::Ok().json(json!({
        "resource_type": MYSQL_SLOW_QUERY_LOG_HEALTH_RESOURCE_TYPE,
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

pub async fn get_mysql_digest_statistics_health_pillar_reports(
    query: web::Query<MySqlInventoryQuery>,
    db_pool: web::Data<Arc<DatabaseConnection>>,
    config: web::Data<Config>,
    claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let query = query.into_inner();
    let pillars = parse_mysql_inventory_pillars(&query.pillar, "MySQL digest statistics health")?;
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
                "MySQL digest statistics health is only supported for mysql connections"
                    .to_string(),
            ));
        }

        let dynamic_conn = connect_to_dynamic_database(&conn_model, config.get_ref()).await?;
        let telemetry = MySqlTelemetryCollector::collect(&dynamic_conn).await?;
        vec![digest_statistics_health_item_from_telemetry(
            &conn_model.id.to_string(),
            &conn_model.name,
            &telemetry,
        )]
    } else {
        Vec::new()
    };

    let now = Utc::now();
    let reports = pillars
        .iter()
        .map(|pillar| evaluate_mysql_digest_statistics_health(&items, *pillar, now))
        .collect::<Vec<_>>();
    let oldest_refresh = items.iter().map(|item| item.collected_at).min();

    Ok(HttpResponse::Ok().json(json!({
        "resource_type": MYSQL_DIGEST_STATISTICS_HEALTH_RESOURCE_TYPE,
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

pub async fn get_mysql_redo_log_health_pillar_reports(
    query: web::Query<MySqlInventoryQuery>,
    db_pool: web::Data<Arc<DatabaseConnection>>,
    config: web::Data<Config>,
    claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let query = query.into_inner();
    let pillars = parse_mysql_inventory_pillars(&query.pillar, "MySQL redo log health")?;
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
                "MySQL redo log health is only supported for mysql connections".to_string(),
            ));
        }

        let dynamic_conn = connect_to_dynamic_database(&conn_model, config.get_ref()).await?;
        let telemetry = MySqlTelemetryCollector::collect(&dynamic_conn).await?;
        vec![redo_log_health_item_from_telemetry(
            &conn_model.id.to_string(),
            &conn_model.name,
            &telemetry,
        )]
    } else {
        Vec::new()
    };

    let now = Utc::now();
    let reports = pillars
        .iter()
        .map(|pillar| evaluate_mysql_redo_log_health(&items, *pillar, now))
        .collect::<Vec<_>>();
    let oldest_refresh = items.iter().map(|item| item.collected_at).min();

    Ok(HttpResponse::Ok().json(json!({
        "resource_type": MYSQL_REDO_LOG_HEALTH_RESOURCE_TYPE,
        "evaluated_at": now,
        "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
        "connection_id": query.connection_id,
        "resources_evaluated": items.len(),
        "oldest_refresh": oldest_refresh,
        "reports": reports,
    })))
}

pub async fn get_mysql_innodb_buffer_pool_health_pillar_reports(
    query: web::Query<MySqlInventoryQuery>,
    db_pool: web::Data<Arc<DatabaseConnection>>,
    config: web::Data<Config>,
    claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let query = query.into_inner();
    let pillars = parse_mysql_inventory_pillars(&query.pillar, "MySQL InnoDB buffer pool health")?;
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
                "MySQL InnoDB buffer pool health is only supported for mysql connections"
                    .to_string(),
            ));
        }

        let dynamic_conn = connect_to_dynamic_database(&conn_model, config.get_ref()).await?;
        let telemetry = MySqlTelemetryCollector::collect(&dynamic_conn).await?;
        vec![innodb_buffer_pool_health_item_from_telemetry(
            &conn_model.id.to_string(),
            &conn_model.name,
            &telemetry,
        )]
    } else {
        Vec::new()
    };

    let now = Utc::now();
    let reports = pillars
        .iter()
        .map(|pillar| evaluate_mysql_innodb_buffer_pool_health(&items, *pillar, now))
        .collect::<Vec<_>>();
    let oldest_refresh = items.iter().map(|item| item.collected_at).min();

    Ok(HttpResponse::Ok().json(json!({
        "resource_type": MYSQL_INNODB_BUFFER_POOL_HEALTH_RESOURCE_TYPE,
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

pub async fn get_mysql_binary_log_health_pillar_reports(
    query: web::Query<MySqlInventoryQuery>,
    db_pool: web::Data<Arc<DatabaseConnection>>,
    config: web::Data<Config>,
    claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let query = query.into_inner();
    let pillars = parse_mysql_inventory_pillars(&query.pillar, "MySQL binary log health")?;
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
                "MySQL binary log health is only supported for mysql connections".to_string(),
            ));
        }

        let dynamic_conn = connect_to_dynamic_database(&conn_model, config.get_ref()).await?;
        let telemetry = MySqlTelemetryCollector::collect(&dynamic_conn).await?;
        vec![binary_log_health_item_from_telemetry(
            &conn_model.id.to_string(),
            &conn_model.name,
            &telemetry,
        )]
    } else {
        Vec::new()
    };

    let now = Utc::now();
    let reports = pillars
        .iter()
        .map(|pillar| evaluate_mysql_binary_log_health(&items, *pillar, now))
        .collect::<Vec<_>>();
    let oldest_refresh = items.iter().map(|item| item.collected_at).min();

    Ok(HttpResponse::Ok().json(json!({
        "resource_type": MYSQL_BINARY_LOG_HEALTH_RESOURCE_TYPE,
        "evaluated_at": now,
        "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
        "connection_id": query.connection_id,
        "resources_evaluated": items.len(),
        "oldest_refresh": oldest_refresh,
        "reports": reports,
    })))
}

pub async fn get_mysql_backup_posture_inventory_pillar_reports(
    query: web::Query<MySqlInventoryQuery>,
    db_pool: web::Data<Arc<DatabaseConnection>>,
    config: web::Data<Config>,
    claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let query = query.into_inner();
    let pillars = parse_mysql_inventory_pillars(&query.pillar, "MySQL backup posture")?;
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
                "MySQL backup posture inventory is only supported for mysql or aurora-mysql connections"
                    .to_string(),
            ));
        }

        let dynamic_conn = connect_to_dynamic_database(&conn_model, config.get_ref()).await?;
        let telemetry = MySqlTelemetryCollector::collect(&dynamic_conn).await?;
        vec![backup_posture_item_from_telemetry(
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
        .map(|pillar| evaluate_mysql_backup_posture_inventory(&items, *pillar, now))
        .collect::<Vec<_>>();
    let oldest_refresh = items.iter().map(|item| item.collected_at).min();

    Ok(HttpResponse::Ok().json(json!({
        "resource_type": MYSQL_BACKUP_POSTURE_RESOURCE_TYPE,
        "evaluated_at": now,
        "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
        "connection_id": query.connection_id,
        "resources_evaluated": items.len(),
        "oldest_refresh": oldest_refresh,
        "reports": reports,
    })))
}

pub async fn get_mysql_restore_drills_inventory_pillar_reports(
    query: web::Query<MySqlInventoryQuery>,
    db_pool: web::Data<Arc<DatabaseConnection>>,
    config: web::Data<Config>,
    claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let query = query.into_inner();
    let pillars = parse_mysql_inventory_pillars(&query.pillar, "MySQL restore drills")?;
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
                "MySQL restore drills inventory is only supported for mysql or aurora-mysql connections"
                    .to_string(),
            ));
        }

        let dynamic_conn = connect_to_dynamic_database(&conn_model, config.get_ref()).await?;
        let telemetry = MySqlTelemetryCollector::collect(&dynamic_conn).await?;
        vec![restore_drill_item_from_telemetry(
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
        .map(|pillar| evaluate_mysql_restore_drills_inventory(&items, *pillar, now))
        .collect::<Vec<_>>();
    let oldest_refresh = items.iter().map(|item| item.collected_at).min();

    Ok(HttpResponse::Ok().json(json!({
        "resource_type": MYSQL_RESTORE_DRILLS_RESOURCE_TYPE,
        "evaluated_at": now,
        "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
        "connection_id": query.connection_id,
        "resources_evaluated": items.len(),
        "oldest_refresh": oldest_refresh,
        "reports": reports,
    })))
}

pub async fn get_mysql_parameter_drift_inventory_pillar_reports(
    query: web::Query<MySqlInventoryQuery>,
    db_pool: web::Data<Arc<DatabaseConnection>>,
    config: web::Data<Config>,
    claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let query = query.into_inner();
    let pillars = parse_mysql_inventory_pillars(&query.pillar, "MySQL parameter drift")?;
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
                "MySQL parameter drift inventory is only supported for mysql or aurora-mysql connections"
                    .to_string(),
            ));
        }

        let dynamic_conn = connect_to_dynamic_database(&conn_model, config.get_ref()).await?;
        let telemetry = MySqlTelemetryCollector::collect(&dynamic_conn).await?;
        vec![parameter_drift_item_from_telemetry(
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
        .map(|pillar| evaluate_mysql_parameter_drift_inventory(&items, *pillar, now))
        .collect::<Vec<_>>();
    let oldest_refresh = items.iter().map(|item| item.collected_at).min();

    Ok(HttpResponse::Ok().json(json!({
        "resource_type": MYSQL_PARAMETER_DRIFT_RESOURCE_TYPE,
        "evaluated_at": now,
        "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
        "connection_id": query.connection_id,
        "resources_evaluated": items.len(),
        "oldest_refresh": oldest_refresh,
        "reports": reports,
    })))
}

pub async fn get_mysql_cost_attribution_inventory_pillar_reports(
    query: web::Query<MySqlInventoryQuery>,
    db_pool: web::Data<Arc<DatabaseConnection>>,
    config: web::Data<Config>,
    claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let query = query.into_inner();
    let pillars = parse_mysql_inventory_pillars(&query.pillar, "MySQL cost attribution")?;
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
                "MySQL cost attribution inventory is only supported for mysql or aurora-mysql connections"
                    .to_string(),
            ));
        }

        let dynamic_conn = connect_to_dynamic_database(&conn_model, config.get_ref()).await?;
        let telemetry = MySqlTelemetryCollector::collect(&dynamic_conn).await?;
        vec![cost_attribution_item_from_telemetry(
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
        .map(|pillar| evaluate_mysql_cost_attribution_inventory(&items, *pillar, now))
        .collect::<Vec<_>>();
    let oldest_refresh = items.iter().map(|item| item.collected_at).min();

    Ok(HttpResponse::Ok().json(json!({
        "resource_type": MYSQL_COST_ATTRIBUTION_RESOURCE_TYPE,
        "evaluated_at": now,
        "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
        "connection_id": query.connection_id,
        "resources_evaluated": items.len(),
        "oldest_refresh": oldest_refresh,
        "reports": reports,
    })))
}

pub async fn get_mysql_ai_prompt_templates_inventory_pillar_reports(
    query: web::Query<MySqlInventoryQuery>,
    db_pool: web::Data<Arc<DatabaseConnection>>,
    config: web::Data<Config>,
    claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let query = query.into_inner();
    let pillars = parse_mysql_inventory_pillars(&query.pillar, "MySQL AI prompt templates")?;
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
                "MySQL AI prompt-template inventory is only supported for mysql or aurora-mysql connections"
                    .to_string(),
            ));
        }

        let prompt_repo = PromptTemplateRepository::new(db_pool.get_ref().as_ref().clone());
        prompt_repo
            .find_all()
            .await?
            .iter()
            .filter_map(ai_prompt_template_item_from_model)
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };

    let now = Utc::now();
    let reports = pillars
        .iter()
        .map(|pillar| evaluate_mysql_ai_prompt_templates_inventory(&items, *pillar, now))
        .collect::<Vec<_>>();
    let oldest_refresh = items.iter().map(|item| item.updated_at).min();

    Ok(HttpResponse::Ok().json(json!({
        "resource_type": MYSQL_AI_PROMPT_TEMPLATE_RESOURCE_TYPE,
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

pub async fn get_mysql_unused_indexes_inventory_pillar_reports(
    query: web::Query<MySqlInventoryQuery>,
    db_pool: web::Data<Arc<DatabaseConnection>>,
    config: web::Data<Config>,
    claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let query = query.into_inner();
    let pillars = parse_mysql_inventory_pillars(&query.pillar, "unused indexes")?;
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
                "Unused indexes inventory is only supported for mysql or aurora-mysql connections"
                    .to_string(),
            ));
        }

        let dynamic_conn = connect_to_dynamic_database(&conn_model, config.get_ref()).await?;
        let telemetry = MySqlTelemetryCollector::collect(&dynamic_conn).await?;
        vec![unused_indexes_item_from_telemetry(
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
        .map(|pillar| evaluate_mysql_unused_indexes_inventory(&items, *pillar, now))
        .collect::<Vec<_>>();
    let oldest_refresh = items.iter().map(|item| item.collected_at).min();

    Ok(HttpResponse::Ok().json(json!({
        "resource_type": MYSQL_UNUSED_INDEXES_RESOURCE_TYPE,
        "evaluated_at": now,
        "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
        "connection_id": query.connection_id,
        "resources_evaluated": items.len(),
        "oldest_refresh": oldest_refresh,
        "reports": reports,
    })))
}

pub async fn get_mysql_missing_indexes_inventory_pillar_reports(
    query: web::Query<MySqlInventoryQuery>,
    db_pool: web::Data<Arc<DatabaseConnection>>,
    config: web::Data<Config>,
    claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let query = query.into_inner();
    let pillars = parse_mysql_inventory_pillars(&query.pillar, "missing indexes")?;
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
                "Missing indexes inventory is only supported for mysql or aurora-mysql connections"
                    .to_string(),
            ));
        }

        let dynamic_conn = connect_to_dynamic_database(&conn_model, config.get_ref()).await?;
        let telemetry = MySqlTelemetryCollector::collect(&dynamic_conn).await?;
        vec![missing_indexes_item_from_telemetry(
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
        .map(|pillar| evaluate_mysql_missing_indexes_inventory(&items, *pillar, now))
        .collect::<Vec<_>>();
    let oldest_refresh = items.iter().map(|item| item.collected_at).min();

    Ok(HttpResponse::Ok().json(json!({
        "resource_type": MYSQL_MISSING_INDEXES_RESOURCE_TYPE,
        "evaluated_at": now,
        "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
        "connection_id": query.connection_id,
        "resources_evaluated": items.len(),
        "oldest_refresh": oldest_refresh,
        "reports": reports,
    })))
}

pub async fn get_mysql_table_bloat_inventory_pillar_reports(
    query: web::Query<MySqlInventoryQuery>,
    db_pool: web::Data<Arc<DatabaseConnection>>,
    config: web::Data<Config>,
    claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let query = query.into_inner();
    let pillars = parse_mysql_inventory_pillars(&query.pillar, "table bloat")?;
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
                "Table bloat inventory is only supported for mysql or aurora-mysql connections"
                    .to_string(),
            ));
        }

        let dynamic_conn = connect_to_dynamic_database(&conn_model, config.get_ref()).await?;
        let telemetry = MySqlTelemetryCollector::collect(&dynamic_conn).await?;
        vec![table_bloat_item_from_telemetry(
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
        .map(|pillar| evaluate_mysql_table_bloat_inventory(&items, *pillar, now))
        .collect::<Vec<_>>();
    let oldest_refresh = items.iter().map(|item| item.collected_at).min();

    Ok(HttpResponse::Ok().json(json!({
        "resource_type": MYSQL_TABLE_BLOAT_RESOURCE_TYPE,
        "evaluated_at": now,
        "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
        "connection_id": query.connection_id,
        "resources_evaluated": items.len(),
        "oldest_refresh": oldest_refresh,
        "reports": reports,
    })))
}

pub async fn get_mysql_partitioning_inventory_pillar_reports(
    query: web::Query<MySqlInventoryQuery>,
    db_pool: web::Data<Arc<DatabaseConnection>>,
    config: web::Data<Config>,
    claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let query = query.into_inner();
    let pillars = parse_mysql_inventory_pillars(&query.pillar, "partitioning")?;
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
                "Partitioning inventory is only supported for mysql or aurora-mysql connections"
                    .to_string(),
            ));
        }

        let dynamic_conn = connect_to_dynamic_database(&conn_model, config.get_ref()).await?;
        let telemetry = MySqlTelemetryCollector::collect(&dynamic_conn).await?;
        vec![partitioning_item_from_telemetry(
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
        .map(|pillar| evaluate_mysql_partitioning_inventory(&items, *pillar, now))
        .collect::<Vec<_>>();
    let oldest_refresh = items.iter().map(|item| item.collected_at).min();

    Ok(HttpResponse::Ok().json(json!({
        "resource_type": MYSQL_PARTITIONING_RESOURCE_TYPE,
        "evaluated_at": now,
        "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
        "connection_id": query.connection_id,
        "resources_evaluated": items.len(),
        "oldest_refresh": oldest_refresh,
        "reports": reports,
    })))
}

pub async fn get_mysql_temporary_tables_inventory_pillar_reports(
    query: web::Query<MySqlInventoryQuery>,
    db_pool: web::Data<Arc<DatabaseConnection>>,
    config: web::Data<Config>,
    claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let query = query.into_inner();
    let pillars = parse_mysql_inventory_pillars(&query.pillar, "temporary tables")?;
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
                "Temporary tables inventory is only supported for mysql or aurora-mysql connections"
                    .to_string(),
            ));
        }

        let dynamic_conn = connect_to_dynamic_database(&conn_model, config.get_ref()).await?;
        let telemetry = MySqlTelemetryCollector::collect(&dynamic_conn).await?;
        vec![temporary_tables_item_from_telemetry(
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
        .map(|pillar| evaluate_mysql_temporary_tables_inventory(&items, *pillar, now))
        .collect::<Vec<_>>();
    let oldest_refresh = items.iter().map(|item| item.collected_at).min();

    Ok(HttpResponse::Ok().json(json!({
        "resource_type": MYSQL_TEMPORARY_TABLES_RESOURCE_TYPE,
        "evaluated_at": now,
        "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
        "connection_id": query.connection_id,
        "resources_evaluated": items.len(),
        "oldest_refresh": oldest_refresh,
        "reports": reports,
    })))
}

pub async fn get_mysql_sort_operations_inventory_pillar_reports(
    query: web::Query<MySqlInventoryQuery>,
    db_pool: web::Data<Arc<DatabaseConnection>>,
    config: web::Data<Config>,
    claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let query = query.into_inner();
    let pillars = parse_mysql_inventory_pillars(&query.pillar, "sort operations")?;
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
                "Sort operations inventory is only supported for mysql or aurora-mysql connections"
                    .to_string(),
            ));
        }

        let dynamic_conn = connect_to_dynamic_database(&conn_model, config.get_ref()).await?;
        let telemetry = MySqlTelemetryCollector::collect(&dynamic_conn).await?;
        vec![sort_operations_item_from_telemetry(
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
        .map(|pillar| evaluate_mysql_sort_operations_inventory(&items, *pillar, now))
        .collect::<Vec<_>>();
    let oldest_refresh = items.iter().map(|item| item.collected_at).min();

    Ok(HttpResponse::Ok().json(json!({
        "resource_type": MYSQL_SORT_OPERATIONS_RESOURCE_TYPE,
        "evaluated_at": now,
        "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
        "connection_id": query.connection_id,
        "resources_evaluated": items.len(),
        "oldest_refresh": oldest_refresh,
        "reports": reports,
    })))
}

pub async fn get_mysql_join_buffers_inventory_pillar_reports(
    query: web::Query<MySqlInventoryQuery>,
    db_pool: web::Data<Arc<DatabaseConnection>>,
    config: web::Data<Config>,
    claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let query = query.into_inner();
    let pillars = parse_mysql_inventory_pillars(&query.pillar, "join buffers")?;
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
                "Join buffers inventory is only supported for mysql or aurora-mysql connections"
                    .to_string(),
            ));
        }

        let dynamic_conn = connect_to_dynamic_database(&conn_model, config.get_ref()).await?;
        let telemetry = MySqlTelemetryCollector::collect(&dynamic_conn).await?;
        vec![join_buffers_item_from_telemetry(
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
        .map(|pillar| evaluate_mysql_join_buffers_inventory(&items, *pillar, now))
        .collect::<Vec<_>>();
    let oldest_refresh = items.iter().map(|item| item.collected_at).min();

    Ok(HttpResponse::Ok().json(json!({
        "resource_type": MYSQL_JOIN_BUFFERS_RESOURCE_TYPE,
        "evaluated_at": now,
        "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
        "connection_id": query.connection_id,
        "resources_evaluated": items.len(),
        "oldest_refresh": oldest_refresh,
        "reports": reports,
    })))
}

pub async fn get_mysql_query_plans_inventory_pillar_reports(
    query: web::Query<MySqlInventoryQuery>,
    db_pool: web::Data<Arc<DatabaseConnection>>,
    config: web::Data<Config>,
    claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let query = query.into_inner();
    let pillars = parse_mysql_inventory_pillars(&query.pillar, "query plans")?;
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
                "Query plans inventory is only supported for mysql or aurora-mysql connections"
                    .to_string(),
            ));
        }

        let dynamic_conn = connect_to_dynamic_database(&conn_model, config.get_ref()).await?;
        let telemetry = MySqlTelemetryCollector::collect(&dynamic_conn).await?;
        vec![query_plans_item_from_telemetry(
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
        .map(|pillar| evaluate_mysql_query_plans_inventory(&items, *pillar, now))
        .collect::<Vec<_>>();
    let oldest_refresh = items.iter().map(|item| item.collected_at).min();

    Ok(HttpResponse::Ok().json(json!({
        "resource_type": MYSQL_QUERY_PLANS_RESOURCE_TYPE,
        "evaluated_at": now,
        "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
        "connection_id": query.connection_id,
        "resources_evaluated": items.len(),
        "oldest_refresh": oldest_refresh,
        "reports": reports,
    })))
}

pub async fn get_mysql_schema_explorer_inventory_pillar_reports(
    query: web::Query<MySqlInventoryQuery>,
    db_pool: web::Data<Arc<DatabaseConnection>>,
    config: web::Data<Config>,
    claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let query = query.into_inner();
    let pillars = parse_mysql_inventory_pillars(&query.pillar, "schema explorer")?;
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
                "Schema explorer inventory is only supported for mysql or aurora-mysql connections"
                    .to_string(),
            ));
        }

        let dynamic_conn = connect_to_dynamic_database(&conn_model, config.get_ref()).await?;
        let telemetry = MySqlTelemetryCollector::collect(&dynamic_conn).await?;
        vec![schema_explorer_item_from_telemetry(
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
        .map(|pillar| evaluate_mysql_schema_explorer_inventory(&items, *pillar, now))
        .collect::<Vec<_>>();
    let oldest_refresh = items.iter().map(|item| item.collected_at).min();

    Ok(HttpResponse::Ok().json(json!({
        "resource_type": MYSQL_SCHEMA_EXPLORER_RESOURCE_TYPE,
        "evaluated_at": now,
        "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
        "connection_id": query.connection_id,
        "resources_evaluated": items.len(),
        "oldest_refresh": oldest_refresh,
        "reports": reports,
    })))
}

pub async fn get_mysql_privilege_audit_inventory_pillar_reports(
    query: web::Query<MySqlInventoryQuery>,
    db_pool: web::Data<Arc<DatabaseConnection>>,
    config: web::Data<Config>,
    claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let query = query.into_inner();
    let pillars = parse_mysql_inventory_pillars(&query.pillar, "privilege audit")?;
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
                "Privilege audit inventory is only supported for mysql or aurora-mysql connections"
                    .to_string(),
            ));
        }

        let dynamic_conn = connect_to_dynamic_database(&conn_model, config.get_ref()).await?;
        let telemetry = MySqlTelemetryCollector::collect(&dynamic_conn).await?;
        vec![privilege_audit_item_from_telemetry(
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
        .map(|pillar| evaluate_mysql_privilege_audit_inventory(&items, *pillar, now))
        .collect::<Vec<_>>();
    let oldest_refresh = items.iter().map(|item| item.collected_at).min();

    Ok(HttpResponse::Ok().json(json!({
        "resource_type": MYSQL_PRIVILEGE_AUDIT_RESOURCE_TYPE,
        "evaluated_at": now,
        "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
        "connection_id": query.connection_id,
        "resources_evaluated": items.len(),
        "oldest_refresh": oldest_refresh,
        "reports": reports,
    })))
}

pub async fn get_mysql_tls_configuration_inventory_pillar_reports(
    query: web::Query<MySqlInventoryQuery>,
    db_pool: web::Data<Arc<DatabaseConnection>>,
    config: web::Data<Config>,
    claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let query = query.into_inner();
    let pillars = parse_mysql_inventory_pillars(&query.pillar, "TLS configuration")?;
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
                "TLS configuration inventory is only supported for mysql or aurora-mysql connections"
                    .to_string(),
            ));
        }

        let dynamic_conn = connect_to_dynamic_database(&conn_model, config.get_ref()).await?;
        let telemetry = MySqlTelemetryCollector::collect(&dynamic_conn).await?;
        vec![tls_configuration_item_from_telemetry(
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
        .map(|pillar| evaluate_mysql_tls_configuration_inventory(&items, *pillar, now))
        .collect::<Vec<_>>();
    let oldest_refresh = items.iter().map(|item| item.collected_at).min();

    Ok(HttpResponse::Ok().json(json!({
        "resource_type": MYSQL_TLS_CONFIGURATION_RESOURCE_TYPE,
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

pub async fn get_mysql_undo_log_health_pillar_reports(
    query: web::Query<MySqlInventoryQuery>,
    db_pool: web::Data<Arc<DatabaseConnection>>,
    config: web::Data<Config>,
    claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let query = query.into_inner();
    let pillars = parse_mysql_inventory_pillars(&query.pillar, "MySQL undo log health")?;
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
                "MySQL undo log health is only supported for mysql connections".to_string(),
            ));
        }

        let dynamic_conn = connect_to_dynamic_database(&conn_model, config.get_ref()).await?;
        let telemetry = MySqlTelemetryCollector::collect(&dynamic_conn).await?;
        vec![undo_log_health_item_from_telemetry(
            &conn_model.id.to_string(),
            &conn_model.name,
            &telemetry,
        )]
    } else {
        Vec::new()
    };

    let now = Utc::now();
    let reports = pillars
        .iter()
        .map(|pillar| evaluate_mysql_undo_log_health(&items, *pillar, now))
        .collect::<Vec<_>>();
    let oldest_refresh = items.iter().map(|item| item.collected_at).min();

    Ok(HttpResponse::Ok().json(json!({
        "resource_type": MYSQL_UNDO_LOG_HEALTH_RESOURCE_TYPE,
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

pub async fn get_mysql_wait_events_health_pillar_reports(
    query: web::Query<MySqlInventoryQuery>,
    db_pool: web::Data<Arc<DatabaseConnection>>,
    config: web::Data<Config>,
    claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let query = query.into_inner();
    let pillars = parse_mysql_inventory_pillars(&query.pillar, "MySQL wait events health")?;
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
                "MySQL wait events health is only supported for mysql connections".to_string(),
            ));
        }

        let dynamic_conn = connect_to_dynamic_database(&conn_model, config.get_ref()).await?;
        let telemetry = MySqlTelemetryCollector::collect(&dynamic_conn).await?;
        vec![wait_events_health_item_from_telemetry(
            &conn_model.id.to_string(),
            &conn_model.name,
            &telemetry,
        )]
    } else {
        Vec::new()
    };

    let now = Utc::now();
    let reports = pillars
        .iter()
        .map(|pillar| evaluate_mysql_wait_events_health(&items, *pillar, now))
        .collect::<Vec<_>>();
    let oldest_refresh = items.iter().map(|item| item.collected_at).min();

    Ok(HttpResponse::Ok().json(json!({
        "resource_type": MYSQL_WAIT_EVENTS_HEALTH_RESOURCE_TYPE,
        "evaluated_at": now,
        "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
        "connection_id": query.connection_id,
        "resources_evaluated": items.len(),
        "oldest_refresh": oldest_refresh,
        "reports": reports,
    })))
}

async fn pg_stat_activity_item_from_connection(
    conn: &DatabaseConnection,
    connection_id: &str,
    connection_name: &str,
    owner: Option<String>,
) -> Result<PgStatActivityInventoryItem, AppError> {
    let row = conn
        .query_one(Statement::from_string(
            DbBackend::Postgres,
            r#"
            WITH activity AS (
                SELECT pid, state, wait_event_type
                FROM pg_stat_activity
            )
            SELECT
                COUNT(*)::bigint AS total_sessions,
                COUNT(*) FILTER (WHERE state = 'active')::bigint AS active_sessions,
                COUNT(*) FILTER (WHERE state = 'idle')::bigint AS idle_sessions,
                COUNT(*) FILTER (WHERE state = 'idle in transaction')::bigint AS idle_in_transaction_sessions,
                COUNT(*) FILTER (WHERE wait_event_type = 'Lock')::bigint AS blocked_sessions,
                (SELECT setting::bigint FROM pg_settings WHERE name = 'max_connections') AS max_connections,
                version() AS server_version,
                (SELECT COUNT(*)::bigint FROM pg_stat_ssl ssl JOIN activity a ON a.pid = ssl.pid WHERE ssl.ssl) AS ssl_sessions
            FROM activity
            "#,
        ))
        .await
        .map_err(AppError::Database)?
        .ok_or_else(|| {
            AppError::Database(sea_orm::DbErr::RecordNotFound(
                "pg_stat_activity summary returned no row".to_string(),
            ))
        })?;

    Ok(PgStatActivityInventoryItem {
        connection_id: connection_id.to_string(),
        connection_name: connection_name.to_string(),
        owner,
        labels: BTreeMap::new(),
        server_version: row.try_get::<String>("", "server_version").ok(),
        total_sessions: non_negative_usize(row.try_get::<i64>("", "total_sessions")?),
        active_sessions: non_negative_usize(row.try_get::<i64>("", "active_sessions")?),
        idle_sessions: non_negative_usize(row.try_get::<i64>("", "idle_sessions")?),
        idle_in_transaction_sessions: non_negative_usize(
            row.try_get::<i64>("", "idle_in_transaction_sessions")?,
        ),
        blocked_sessions: non_negative_usize(row.try_get::<i64>("", "blocked_sessions")?),
        max_connections: Some(non_negative_usize(
            row.try_get::<i64>("", "max_connections")?,
        )),
        ssl_sessions: row
            .try_get::<i64>("", "ssl_sessions")
            .ok()
            .map(non_negative_usize),
        collected_at: Utc::now(),
    })
}

async fn pg_stat_statements_item_from_connection(
    conn: &DatabaseConnection,
    connection_id: &str,
    connection_name: &str,
    owner: Option<String>,
) -> Result<PgStatStatementsInventoryItem, AppError> {
    let extension_row = conn
        .query_one(Statement::from_string(
            DbBackend::Postgres,
            r#"
            SELECT
                EXISTS (
                    SELECT 1
                    FROM pg_extension
                    WHERE extname = 'pg_stat_statements'
                ) AS extension_available,
                version() AS server_version
            "#,
        ))
        .await
        .map_err(AppError::Database)?
        .ok_or_else(|| {
            AppError::Database(sea_orm::DbErr::RecordNotFound(
                "pg_stat_statements extension check returned no row".to_string(),
            ))
        })?;

    let server_version = extension_row.try_get::<String>("", "server_version").ok();
    let extension_available = extension_row
        .try_get::<bool>("", "extension_available")
        .unwrap_or(false);

    if !extension_available {
        return Ok(missing_pg_stat_statements_item(
            connection_id,
            connection_name,
            owner,
            server_version,
            "pg_stat_statements extension is not installed".to_string(),
        ));
    }

    let row = match conn
        .query_one(Statement::from_string(
            DbBackend::Postgres,
            r#"
            SELECT
                COUNT(*)::bigint AS statements_tracked,
                COALESCE(SUM(calls), 0)::bigint AS total_calls,
                COALESCE(SUM(total_exec_time), 0)::double precision AS total_exec_time_ms,
                MAX(mean_exec_time)::double precision AS max_mean_exec_time_ms,
                COALESCE(SUM(shared_blks_read), 0)::bigint AS shared_blks_read,
                COALESCE(SUM(shared_blks_hit), 0)::bigint AS shared_blks_hit,
                COALESCE(SUM(temp_blks_written), 0)::bigint AS temp_blks_written,
                COALESCE(BOOL_OR(NULLIF(query, '') IS NOT NULL), false) AS query_text_visible
            FROM pg_stat_statements
            "#,
        ))
        .await
    {
        Ok(Some(row)) => row,
        Ok(None) => {
            return Ok(missing_pg_stat_statements_item(
                connection_id,
                connection_name,
                owner,
                server_version,
                "pg_stat_statements aggregate returned no row".to_string(),
            ));
        }
        Err(error) => {
            return Ok(missing_pg_stat_statements_item(
                connection_id,
                connection_name,
                owner,
                server_version,
                format!("pg_stat_statements aggregate query failed: {}", error),
            ));
        }
    };

    Ok(PgStatStatementsInventoryItem {
        connection_id: connection_id.to_string(),
        connection_name: connection_name.to_string(),
        owner,
        labels: BTreeMap::new(),
        server_version,
        extension_available: true,
        statements_tracked: non_negative_usize(row.try_get::<i64>("", "statements_tracked")?),
        total_calls: row.try_get::<i64>("", "total_calls").unwrap_or(0).max(0),
        total_exec_time_ms: row
            .try_get::<f64>("", "total_exec_time_ms")
            .unwrap_or(0.0)
            .max(0.0),
        max_mean_exec_time_ms: row
            .try_get::<f64>("", "max_mean_exec_time_ms")
            .ok()
            .map(|value| value.max(0.0)),
        shared_blks_read: row
            .try_get::<i64>("", "shared_blks_read")
            .unwrap_or(0)
            .max(0),
        shared_blks_hit: row
            .try_get::<i64>("", "shared_blks_hit")
            .unwrap_or(0)
            .max(0),
        temp_blks_written: row
            .try_get::<i64>("", "temp_blks_written")
            .unwrap_or(0)
            .max(0),
        query_text_visible: row
            .try_get::<bool>("", "query_text_visible")
            .unwrap_or(false),
        missing_evidence_reason: None,
        collected_at: Utc::now(),
    })
}

fn missing_pg_stat_statements_item(
    connection_id: &str,
    connection_name: &str,
    owner: Option<String>,
    server_version: Option<String>,
    reason: String,
) -> PgStatStatementsInventoryItem {
    PgStatStatementsInventoryItem {
        connection_id: connection_id.to_string(),
        connection_name: connection_name.to_string(),
        owner,
        labels: BTreeMap::new(),
        server_version,
        extension_available: false,
        statements_tracked: 0,
        total_calls: 0,
        total_exec_time_ms: 0.0,
        max_mean_exec_time_ms: None,
        shared_blks_read: 0,
        shared_blks_hit: 0,
        temp_blks_written: 0,
        query_text_visible: false,
        missing_evidence_reason: Some(reason),
        collected_at: Utc::now(),
    }
}

async fn pg_stat_database_items_from_connection(
    conn: &DatabaseConnection,
    connection_id: &str,
    connection_name: &str,
    owner: Option<String>,
) -> Result<Vec<PgStatDatabaseInventoryItem>, AppError> {
    let row = match conn
        .query_one(Statement::from_string(
            DbBackend::Postgres,
            r#"
            SELECT
                d.datname,
                d.numbackends::bigint AS numbackends,
                d.xact_commit::bigint AS xact_commit,
                d.xact_rollback::bigint AS xact_rollback,
                d.blks_read::bigint AS blks_read,
                d.blks_hit::bigint AS blks_hit,
                d.tup_returned::bigint AS tup_returned,
                d.tup_fetched::bigint AS tup_fetched,
                d.tup_inserted::bigint AS tup_inserted,
                d.tup_updated::bigint AS tup_updated,
                d.tup_deleted::bigint AS tup_deleted,
                d.deadlocks::bigint AS deadlocks,
                d.temp_files::bigint AS temp_files,
                d.temp_bytes::bigint AS temp_bytes,
                d.conflicts::bigint AS conflicts,
                d.checksum_failures::bigint AS checksum_failures,
                pg_database_size(d.datname)::bigint AS database_size_bytes,
                (SELECT setting::bigint FROM pg_settings WHERE name = 'max_connections') AS max_connections,
                version() AS server_version
            FROM pg_stat_database d
            WHERE d.datname = current_database()
            "#,
        ))
        .await
    {
        Ok(Some(row)) => row,
        Ok(None) => {
            return Ok(vec![missing_pg_stat_database_item(
                connection_id,
                connection_name,
                owner,
                None,
                "pg_stat_database returned no row for current_database()".to_string(),
            )]);
        }
        Err(error) => {
            return Ok(vec![missing_pg_stat_database_item(
                connection_id,
                connection_name,
                owner,
                None,
                format!("pg_stat_database query failed: {}", error),
            )]);
        }
    };

    Ok(vec![PgStatDatabaseInventoryItem {
        connection_id: connection_id.to_string(),
        connection_name: connection_name.to_string(),
        datname: row
            .try_get::<String>("", "datname")
            .unwrap_or_else(|_| connection_name.to_string()),
        owner,
        labels: BTreeMap::new(),
        stats_available: true,
        missing_evidence_reason: None,
        numbackends: row.try_get::<i64>("", "numbackends").unwrap_or(0).max(0),
        max_connections: row
            .try_get::<i64>("", "max_connections")
            .ok()
            .map(|value| value.max(0)),
        xact_commit: row.try_get::<i64>("", "xact_commit").unwrap_or(0).max(0),
        xact_rollback: row.try_get::<i64>("", "xact_rollback").unwrap_or(0).max(0),
        blks_read: row.try_get::<i64>("", "blks_read").unwrap_or(0).max(0),
        blks_hit: row.try_get::<i64>("", "blks_hit").unwrap_or(0).max(0),
        tup_returned: row.try_get::<i64>("", "tup_returned").unwrap_or(0).max(0),
        tup_fetched: row.try_get::<i64>("", "tup_fetched").unwrap_or(0).max(0),
        tup_inserted: row.try_get::<i64>("", "tup_inserted").unwrap_or(0).max(0),
        tup_updated: row.try_get::<i64>("", "tup_updated").unwrap_or(0).max(0),
        tup_deleted: row.try_get::<i64>("", "tup_deleted").unwrap_or(0).max(0),
        deadlocks: row.try_get::<i64>("", "deadlocks").unwrap_or(0).max(0),
        temp_files: row.try_get::<i64>("", "temp_files").unwrap_or(0).max(0),
        temp_bytes: row.try_get::<i64>("", "temp_bytes").unwrap_or(0).max(0),
        conflicts: row.try_get::<i64>("", "conflicts").unwrap_or(0).max(0),
        checksum_failures: row
            .try_get::<i64>("", "checksum_failures")
            .ok()
            .map(|value| value.max(0)),
        database_size_bytes: row
            .try_get::<i64>("", "database_size_bytes")
            .ok()
            .map(|value| value.max(0)),
        server_version: row.try_get::<String>("", "server_version").ok(),
        exposure: Some(PgStatDatabaseExposureEvidence {
            publicly_accessible: None,
            ssl_enforced: None,
            allowed_source_count: None,
        }),
        collected_at: Utc::now(),
    }])
}

fn missing_pg_stat_database_item(
    connection_id: &str,
    connection_name: &str,
    owner: Option<String>,
    server_version: Option<String>,
    reason: String,
) -> PgStatDatabaseInventoryItem {
    PgStatDatabaseInventoryItem {
        connection_id: connection_id.to_string(),
        connection_name: connection_name.to_string(),
        datname: connection_name.to_string(),
        owner,
        labels: BTreeMap::new(),
        stats_available: false,
        missing_evidence_reason: Some(reason),
        numbackends: 0,
        max_connections: None,
        xact_commit: 0,
        xact_rollback: 0,
        blks_read: 0,
        blks_hit: 0,
        tup_returned: 0,
        tup_fetched: 0,
        tup_inserted: 0,
        tup_updated: 0,
        tup_deleted: 0,
        deadlocks: 0,
        temp_files: 0,
        temp_bytes: 0,
        conflicts: 0,
        checksum_failures: None,
        database_size_bytes: None,
        server_version,
        exposure: None,
        collected_at: Utc::now(),
    }
}

async fn pg_stat_io_items_from_connection(
    conn: &DatabaseConnection,
    connection_id: &str,
    connection_name: &str,
    owner: Option<String>,
) -> Result<Vec<PgStatIoInventoryItem>, AppError> {
    let availability = match conn
        .query_one(Statement::from_string(
            DbBackend::Postgres,
            r#"
            SELECT
                EXISTS (
                    SELECT 1
                    FROM information_schema.views
                    WHERE table_schema = 'pg_catalog'
                      AND table_name = 'pg_stat_io'
                ) AS available,
                version() AS server_version
            "#,
        ))
        .await
    {
        Ok(Some(row)) => row,
        Ok(None) => {
            return Ok(vec![missing_pg_stat_io_item(
                connection_id,
                connection_name,
                owner,
                None,
                "pg_stat_io availability check returned no row".to_string(),
            )]);
        }
        Err(error) => {
            return Ok(vec![missing_pg_stat_io_item(
                connection_id,
                connection_name,
                owner,
                None,
                format!("pg_stat_io availability check failed: {}", error),
            )]);
        }
    };

    let server_version = availability.try_get::<String>("", "server_version").ok();
    let available = availability
        .try_get::<bool>("", "available")
        .unwrap_or(false);
    if !available {
        return Ok(vec![missing_pg_stat_io_item(
            connection_id,
            connection_name,
            owner,
            server_version,
            "pg_stat_io is not available on this PostgreSQL connection".to_string(),
        )]);
    }

    let rows = match conn
        .query_all(Statement::from_string(
            DbBackend::Postgres,
            r#"
            SELECT
                backend_type,
                context,
                object,
                COALESCE(SUM(reads), 0)::bigint AS reads,
                COALESCE(SUM(read_time), 0)::double precision AS read_time_ms,
                COALESCE(SUM(writes), 0)::bigint AS writes,
                COALESCE(SUM(write_time), 0)::double precision AS write_time_ms,
                COALESCE(SUM(writebacks), 0)::bigint AS writebacks,
                COALESCE(SUM(writeback_time), 0)::double precision AS writeback_time_ms,
                COALESCE(SUM(extends), 0)::bigint AS extends,
                COALESCE(SUM(extend_time), 0)::double precision AS extend_time_ms,
                COALESCE(MAX(op_bytes), 0)::bigint AS op_bytes,
                COALESCE(SUM(hits), 0)::bigint AS hits,
                COALESCE(SUM(evictions), 0)::bigint AS evictions,
                COALESCE(SUM(reuses), 0)::bigint AS reuses,
                COALESCE(SUM(fsyncs), 0)::bigint AS fsyncs,
                COALESCE(SUM(fsync_time), 0)::double precision AS fsync_time_ms,
                MAX(stats_reset) AS stats_reset
            FROM pg_stat_io
            GROUP BY backend_type, context, object
            "#,
        ))
        .await
    {
        Ok(rows) => rows,
        Err(error) => {
            return Ok(vec![missing_pg_stat_io_item(
                connection_id,
                connection_name,
                owner,
                server_version,
                format!("pg_stat_io aggregate query failed: {}", error),
            )]);
        }
    };

    if rows.is_empty() {
        return Ok(vec![missing_pg_stat_io_item(
            connection_id,
            connection_name,
            owner,
            server_version,
            "pg_stat_io aggregate returned no rows".to_string(),
        )]);
    }

    let mut items = Vec::with_capacity(rows.len());
    for row in rows {
        items.push(PgStatIoInventoryItem {
            connection_id: connection_id.to_string(),
            connection_name: connection_name.to_string(),
            owner: owner.clone(),
            labels: BTreeMap::new(),
            pg_stat_io_available: true,
            backend_type: row
                .try_get::<String>("", "backend_type")
                .unwrap_or_else(|_| "unknown".to_string()),
            context: row
                .try_get::<String>("", "context")
                .unwrap_or_else(|_| "unknown".to_string()),
            object: row
                .try_get::<String>("", "object")
                .unwrap_or_else(|_| "unknown".to_string()),
            reads: row.try_get::<i64>("", "reads").unwrap_or(0).max(0),
            read_time_ms: row
                .try_get::<f64>("", "read_time_ms")
                .unwrap_or(0.0)
                .max(0.0),
            writes: row.try_get::<i64>("", "writes").unwrap_or(0).max(0),
            write_time_ms: row
                .try_get::<f64>("", "write_time_ms")
                .unwrap_or(0.0)
                .max(0.0),
            writebacks: row.try_get::<i64>("", "writebacks").unwrap_or(0).max(0),
            writeback_time_ms: row
                .try_get::<f64>("", "writeback_time_ms")
                .unwrap_or(0.0)
                .max(0.0),
            extends: row.try_get::<i64>("", "extends").unwrap_or(0).max(0),
            extend_time_ms: row
                .try_get::<f64>("", "extend_time_ms")
                .unwrap_or(0.0)
                .max(0.0),
            op_bytes: row.try_get::<i64>("", "op_bytes").unwrap_or(0).max(0),
            hits: row.try_get::<i64>("", "hits").unwrap_or(0).max(0),
            evictions: row.try_get::<i64>("", "evictions").unwrap_or(0).max(0),
            reuses: row.try_get::<i64>("", "reuses").unwrap_or(0).max(0),
            fsyncs: row.try_get::<i64>("", "fsyncs").unwrap_or(0).max(0),
            fsync_time_ms: row
                .try_get::<f64>("", "fsync_time_ms")
                .unwrap_or(0.0)
                .max(0.0),
            stats_reset: row
                .try_get::<Option<DateTime<Utc>>>("", "stats_reset")
                .ok()
                .flatten(),
            server_version: server_version.clone(),
            security_evidence_recorded: false,
            missing_evidence_reason: None,
            collected_at: Utc::now(),
        });
    }

    Ok(items)
}

fn missing_pg_stat_io_item(
    connection_id: &str,
    connection_name: &str,
    owner: Option<String>,
    server_version: Option<String>,
    reason: String,
) -> PgStatIoInventoryItem {
    PgStatIoInventoryItem {
        connection_id: connection_id.to_string(),
        connection_name: connection_name.to_string(),
        owner,
        labels: BTreeMap::new(),
        pg_stat_io_available: false,
        backend_type: "unknown".to_string(),
        context: "unknown".to_string(),
        object: "unknown".to_string(),
        reads: 0,
        read_time_ms: 0.0,
        writes: 0,
        write_time_ms: 0.0,
        writebacks: 0,
        writeback_time_ms: 0.0,
        extends: 0,
        extend_time_ms: 0.0,
        op_bytes: 0,
        hits: 0,
        evictions: 0,
        reuses: 0,
        fsyncs: 0,
        fsync_time_ms: 0.0,
        stats_reset: None,
        server_version,
        security_evidence_recorded: false,
        missing_evidence_reason: Some(reason),
        collected_at: Utc::now(),
    }
}

fn non_negative_usize(value: i64) -> usize {
    value.max(0) as usize
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

fn parse_postgres_inventory_pillars(
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
                    AppError::BadRequest(format!(
                        "Unsupported PostgreSQL inventory pillar: {}",
                        token
                    ))
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

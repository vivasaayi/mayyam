use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Deserialize, Serialize)]
#[sea_orm(table_name = "database_connections")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: Uuid,
    pub name: String,
    pub connection_type: String, // postgres, mysql, redis, opensearch
    pub host: String,
    pub port: i32,
    pub username: Option<String>,
    pub password_encrypted: Option<String>,
    pub database_name: Option<String>,
    pub ssl_mode: Option<String>,
    pub cluster_mode: Option<bool>,
    pub created_by: Uuid,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub last_connected_at: Option<DateTime<Utc>>,
    pub connection_status: Option<String>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

// DTOs for database operations
#[derive(Debug, Deserialize)]
pub struct CreateDatabaseConnectionRequest {
    pub name: String,
    pub connection_type: String,
    pub host: String,
    pub port: i32,
    pub username: Option<String>,
    pub password: Option<String>,
    pub database_name: Option<String>,
    pub ssl_mode: Option<String>,
    pub cluster_mode: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct DatabaseQueryRequest {
    pub connection_id: String,
    pub query: String,
    pub params: Option<serde_json::Value>,
    pub explain: Option<bool>,
    pub analyze: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct DatabaseQueryResponse {
    pub columns: Vec<String>,
    pub rows: Vec<serde_json::Value>,
    pub execution_time_ms: u64,
    pub row_count: usize,
    pub query_plan: Option<QueryPlan>,
}

#[derive(Debug, Serialize)]
pub struct QueryPlan {
    pub plan_type: String,
    pub total_cost: f64,
    pub planning_time_ms: f64,
    pub execution_time_ms: f64,
    pub nodes: Vec<QueryPlanNode>,
}

#[derive(Debug, Serialize)]
pub struct QueryPlanNode {
    pub node_type: String,
    pub actual_rows: i64,
    pub plan_rows: i64,
    pub actual_time_ms: f64,
    pub total_cost: f64,
    pub description: String,
    pub children: Vec<QueryPlanNode>,
}

#[derive(Debug, Serialize)]
pub struct DatabaseAnalysis {
    pub issues: Vec<DatabaseIssue>,
    pub query_stats: QueryStatistics,
    pub performance_metrics: PerformanceMetrics,
    pub cost_analysis: CostAnalysis,
}

#[derive(Debug, Serialize)]
pub struct DatabaseIssue {
    pub severity: IssueSeverity,
    pub category: IssueCategory,
    pub title: String,
    pub description: String,
    pub recommendation: String,
    pub affected_objects: Vec<String>,
}

#[derive(Debug, Serialize, Eq, PartialEq)]
pub enum IssueSeverity {
    Critical,
    High,
    Medium,
    Low,
    Info,
}

#[derive(Debug, Serialize, Eq, PartialEq)]
pub enum IssueCategory {
    Performance,
    Storage,
    Configuration,
    Security,
    Reliability,
    Cost,
}

#[derive(Debug, Serialize)]
pub struct QueryStatistics {
    pub total_queries: i64,
    pub slow_queries: i64,
    pub avg_query_time_ms: f64,
    pub top_slow_queries: Vec<SlowQuery>,
    pub frequent_queries: Vec<FrequentQuery>,
}

#[derive(Debug, Serialize)]
pub struct SlowQuery {
    pub query: String,
    pub avg_execution_time_ms: f64,
    pub execution_count: i64,
    pub last_execution: DateTime<Utc>,
    pub query_plan: Option<QueryPlan>,
}

#[derive(Debug, Serialize)]
pub struct FrequentQuery {
    pub query: String,
    pub execution_count: i64,
    pub avg_execution_time_ms: f64,
    pub total_time_ms: f64,
}

#[derive(Debug, Serialize)]
pub struct PerformanceMetrics {
    pub connection_count: i32,
    pub active_sessions: i32,
    pub idle_sessions: i32,
    pub buffer_hit_ratio: f64,
    pub cache_hit_ratio: f64,
    pub deadlocks: i64,
    pub blocked_queries: i64,
    pub table_stats: Vec<TableStats>,
    pub index_stats: Vec<IndexStats>,
}

#[derive(Debug, Serialize)]
pub struct TableStats {
    pub name: String,
    pub size_bytes: i64,
    pub total_rows: i64,
    pub sequential_scans: i64,
    pub index_scans: i64,
    pub live_row_count: i64,
    pub dead_row_count: i64,
    pub last_vacuum: Option<DateTime<Utc>>,
    pub last_analyze: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize)]
pub struct IndexStats {
    pub name: String,
    pub table_name: String,
    pub size_bytes: i64,
    pub is_unique: bool,
    pub is_primary: bool,
    pub index_scans: i64,
    pub rows_fetched: i64,
    pub unused_since: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize)]
pub struct CostAnalysis {
    pub storage_cost: ResourceCost,
    pub compute_cost: ResourceCost,
    pub network_cost: ResourceCost,
    pub backup_cost: ResourceCost,
    pub total_monthly_cost: f64,
    pub cost_recommendations: Vec<CostRecommendation>,
}

#[derive(Debug, Serialize)]
pub struct ResourceCost {
    pub current_usage: f64,
    pub unit: String,
    pub cost_per_unit: f64,
    pub total_cost: f64,
    pub trending: TrendDirection,
}

#[derive(Debug, Serialize)]
pub enum TrendDirection {
    Increasing,
    Decreasing,
    Stable,
}

#[derive(Debug, Serialize)]
pub struct CostRecommendation {
    pub title: String,
    pub description: String,
    pub estimated_savings: f64,
    pub implementation_effort: String,
    pub priority: String,
}

#[derive(Debug, Serialize)]
pub struct ConnectionTestResult {
    pub success: bool,
    pub message: String,
    pub latency_ms: Option<u64>,
    pub version_info: Option<String>,
    pub connection_stats: Option<ConnectionStats>,
}

#[derive(Debug, Serialize)]
pub struct ConnectionStats {
    pub max_connections: i32,
    pub current_connections: i32,
    pub ssl_in_use: bool,
    pub server_encoding: String,
    pub server_version: String,
}

#[derive(Debug, Serialize)]
pub struct StorageMetrics {
    pub total_bytes: i64,
    pub user_data_bytes: i64,
    pub index_bytes: i64,
    pub free_space_bytes: i64,
    pub growth_rate: f64, // Daily growth rate as a decimal (e.g., 0.02 for 2%)
    pub estimate_days_until_full: Option<f64>,
    pub top_tables_by_size: HashMap<String, i64>,
}

#[derive(Debug, Serialize)]
pub struct ComputeMetrics {
    pub cpu_usage: f64, // Estimated CPU usage as a decimal (e.g., 2.5 for 2.5 vCPUs)
    pub memory_usage_bytes: i64,
    pub active_connections: i32,
    pub uptime_seconds: f64,
}

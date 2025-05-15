use sea_orm::{DatabaseConnection, Statement, DbBackend};
use sqlx::{postgres::PgRow, Row};
use chrono::Utc;
use std::collections::HashMap;
use tracing::{info, warn, error};

use crate::models::database::*;
use crate::errors::AppError;
use crate::config::Config;

pub struct DatabaseService {
    config: Config,
}

impl DatabaseService {
    pub fn new(config: Config) -> Self {
        Self { config }
    }

    pub async fn analyze_database(&self, conn: &DatabaseConnection) -> Result<DatabaseAnalysis, AppError> {
        let mut analysis = DatabaseAnalysis {
            issues: Vec::new(),
            query_stats: self.get_query_statistics(conn).await?,
            performance_metrics: self.get_performance_metrics(conn).await?,
            cost_analysis: self.analyze_costs(conn).await?,
        };

        // Analyze and collect issues
        self.analyze_performance_issues(conn, &mut analysis.issues).await?;
        self.analyze_storage_issues(conn, &mut analysis.issues).await?;
        self.analyze_security_issues(conn, &mut analysis.issues).await?;
        self.analyze_configuration_issues(conn, &mut analysis.issues).await?;

        Ok(analysis)
    }

    async fn get_query_statistics(&self, conn: &DatabaseConnection) -> Result<QueryStatistics, AppError> {
        // For PostgreSQL
        let stats = conn.query_one(Statement::from_string(
            DbBackend::Postgres,
            r#"
            WITH QueryStats AS (
                SELECT query,
                    calls as execution_count,
                    total_time / 1000 as total_time_ms,
                    mean_time / 1000 as avg_time_ms,
                    rows as total_rows,
                    shared_blks_hit + shared_blks_read as total_blocks,
                    max(last_call) as last_execution
                FROM pg_stat_statements
                WHERE dbid = (SELECT oid FROM pg_database WHERE datname = current_database())
            )
            SELECT 
                (SELECT COUNT(*) FROM QueryStats) as total_queries,
                (SELECT COUNT(*) FROM QueryStats WHERE avg_time_ms > 1000) as slow_queries,
                (SELECT AVG(avg_time_ms) FROM QueryStats) as avg_query_time
            "#,
        ))
        .await
        .map_err(AppError::Database)?;

        let slow_queries = self.get_slow_queries(conn).await?;
        let frequent_queries = self.get_frequent_queries(conn).await?;

        Ok(QueryStatistics {
            total_queries: stats.try_get::<i64, _>("total_queries")?,
            slow_queries: stats.try_get::<i64, _>("slow_queries")?,
            avg_query_time_ms: stats.try_get::<f64, _>("avg_query_time")?,
            top_slow_queries: slow_queries,
            frequent_queries,
        })
    }

    async fn get_performance_metrics(&self, conn: &DatabaseConnection) -> Result<PerformanceMetrics, AppError> {
        let stats = conn.query_one(Statement::from_string(
            DbBackend::Postgres,
            r#"
            SELECT 
                (SELECT setting::int FROM pg_settings WHERE name = 'max_connections') as max_conn,
                (SELECT count(*) FROM pg_stat_activity) as current_conn,
                (SELECT count(*) FROM pg_stat_activity WHERE state = 'idle') as idle_conn,
                (SELECT sum(blks_hit)::float / (sum(blks_hit) + sum(blks_read)) FROM pg_stat_database) as buffer_ratio,
                (SELECT extract(epoch from now() - pg_postmaster_start_time())) as uptime,
                (SELECT count(*) FROM pg_locks WHERE granted = false) as blocked_queries
            "#,
        ))
        .await
        .map_err(AppError::Database)?;

        let table_stats = self.get_table_statistics(conn).await?;
        let index_stats = self.get_index_statistics(conn).await?;

        Ok(PerformanceMetrics {
            connection_count: stats.try_get::<i32, _>("current_conn")?,
            active_sessions: stats.try_get::<i32, _>("current_conn")? - stats.try_get::<i32, _>("idle_conn")?,
            idle_sessions: stats.try_get::<i32, _>("idle_conn")?,
            buffer_hit_ratio: stats.try_get::<f64, _>("buffer_ratio")?,
            cache_hit_ratio: 0.0, // Needs additional calculation
            deadlocks: 0, // Need to get from pg_stat_database
            blocked_queries: stats.try_get::<i64, _>("blocked_queries")?,
            table_stats,
            index_stats,
        })
    }

    async fn analyze_costs(&self, conn: &DatabaseConnection) -> Result<CostAnalysis, AppError> {
        let storage_metrics = self.get_storage_metrics(conn).await?;
        let compute_metrics = self.get_compute_metrics(conn).await?;
        
        let storage_cost = ResourceCost {
            current_usage: storage_metrics.total_bytes as f64,
            unit: "GB".to_string(),
            cost_per_unit: 0.10, // Example cost per GB
            total_cost: (storage_metrics.total_bytes as f64 / 1024.0 / 1024.0 / 1024.0) * 0.10,
            trending: if storage_metrics.growth_rate > 0.0 {
                TrendDirection::Increasing
            } else if storage_metrics.growth_rate < 0.0 {
                TrendDirection::Decreasing
            } else {
                TrendDirection::Stable
            },
        };

        // Calculate other costs...
        Ok(CostAnalysis {
            storage_cost,
            compute_cost: ResourceCost {
                current_usage: compute_metrics.cpu_usage,
                unit: "vCPU hours".to_string(),
                cost_per_unit: 0.05,
                total_cost: compute_metrics.cpu_usage * 0.05,
                trending: TrendDirection::Stable,
            },
            network_cost: ResourceCost {
                current_usage: 0.0,
                unit: "GB".to_string(),
                cost_per_unit: 0.09,
                total_cost: 0.0,
                trending: TrendDirection::Stable,
            },
            backup_cost: ResourceCost {
                current_usage: storage_metrics.total_bytes as f64,
                unit: "GB".to_string(),
                cost_per_unit: 0.05,
                total_cost: (storage_metrics.total_bytes as f64 / 1024.0 / 1024.0 / 1024.0) * 0.05,
                trending: TrendDirection::Stable,
            },
            total_monthly_cost: 0.0, // Calculate total
            cost_recommendations: self.generate_cost_recommendations(conn).await?,
        })
    }

    async fn analyze_performance_issues(&self, conn: &DatabaseConnection, issues: &mut Vec<DatabaseIssue>) -> Result<(), AppError> {
        // Check for missing indexes on frequently queried columns
        let missing_indexes = conn.query_all(Statement::from_string(
            DbBackend::Postgres,
            r#"
            SELECT schemaname, tablename, rows_per_scan
            FROM (
                SELECT schemaname, tablename,
                    CASE WHEN seq_scan > 0 THEN (seq_tup_read::float / seq_scan) ELSE 0 END as rows_per_scan
                FROM pg_stat_user_tables
                WHERE seq_scan > 0
            ) t
            WHERE rows_per_scan > 1000
            ORDER BY rows_per_scan DESC
            LIMIT 10
            "#,
        ))
        .await
        .map_err(AppError::Database)?;

        for row in missing_indexes {
            issues.push(DatabaseIssue {
                severity: IssueSeverity::High,
                category: IssueCategory::Performance,
                title: format!("Missing index on frequently scanned table {}", row.try_get::<String, _>("tablename")?),
                description: format!(
                    "Table {} is frequently scanned with {} rows per scan on average",
                    row.try_get::<String, _>("tablename")?,
                    row.try_get::<f64, _>("rows_per_scan")?
                ),
                recommendation: "Consider adding an index on the commonly queried columns".to_string(),
                affected_objects: vec![format!(
                    "{}.{}",
                    row.try_get::<String, _>("schemaname")?,
                    row.try_get::<String, _>("tablename")?
                )],
            });
        }

        // Add more performance issue checks...
        Ok(())
    }

    async fn get_table_statistics(&self, conn: &DatabaseConnection) -> Result<Vec<TableStats>, AppError> {
        let stats = conn.query_all(Statement::from_string(
            DbBackend::Postgres,
            r#"
            SELECT 
                relname as table_name,
                pg_table_size(c.oid) as table_size,
                n_live_tup as row_count,
                n_dead_tup as dead_rows,
                seq_scan,
                idx_scan,
                last_vacuum,
                last_analyze
            FROM pg_stat_user_tables s
            JOIN pg_class c ON s.relid = c.oid
            "#,
        ))
        .await
        .map_err(AppError::Database)?;

        let mut table_stats = Vec::new();
        for row in stats {
            table_stats.push(TableStats {
                name: row.try_get::<String, _>("table_name")?,
                size_bytes: row.try_get::<i64, _>("table_size")?,
                total_rows: row.try_get::<i64, _>("row_count")?,
                sequential_scans: row.try_get::<i64, _>("seq_scan")?,
                index_scans: row.try_get::<i64, _>("idx_scan")?,
                live_row_count: row.try_get::<i64, _>("row_count")?,
                dead_row_count: row.try_get::<i64, _>("dead_rows")?,
                last_vacuum: row.try_get::<Option<DateTime<Utc>>, _>("last_vacuum")?,
                last_analyze: row.try_get::<Option<DateTime<Utc>>, _>("last_analyze")?,
            });
        }

        Ok(table_stats)
    }

    async fn get_index_statistics(&self, conn: &DatabaseConnection) -> Result<Vec<IndexStats>, AppError> {
        let stats = conn.query_all(Statement::from_string(
            DbBackend::Postgres,
            r#"
            SELECT 
                i.relname as index_name,
                t.relname as table_name,
                pg_relation_size(i.oid) as index_size,
                idx_scan,
                idx_tup_read,
                indisunique,
                indisprimary,
                last_used = NULL AND idx_scan = 0 as is_unused,
                last_used
            FROM pg_stat_user_indexes s
            JOIN pg_class i ON s.indexrelid = i.oid
            JOIN pg_class t ON s.relid = t.oid
            JOIN pg_index idx ON s.indexrelid = idx.indexrelid
            "#,
        ))
        .await
        .map_err(AppError::Database)?;

        let mut index_stats = Vec::new();
        for row in stats {
            index_stats.push(IndexStats {
                name: row.try_get::<String, _>("index_name")?,
                table_name: row.try_get::<String, _>("table_name")?,
                size_bytes: row.try_get::<i64, _>("index_size")?,
                is_unique: row.try_get::<bool, _>("indisunique")?,
                is_primary: row.try_get::<bool, _>("indisprimary")?,
                index_scans: row.try_get::<i64, _>("idx_scan")?,
                rows_fetched: row.try_get::<i64, _>("idx_tup_read")?,
                unused_since: if row.try_get::<bool, _>("is_unused")? {
                    Some(row.try_get::<DateTime<Utc>, _>("last_used")?)
                } else {
                    None
                },
            });
        }

        Ok(index_stats)
    }

    async fn generate_cost_recommendations(&self, conn: &DatabaseConnection) -> Result<Vec<CostRecommendation>, AppError> {
        let mut recommendations = Vec::new();

        // Check for unused indexes
        let unused_indexes = conn.query_all(Statement::from_string(
            DbBackend::Postgres,
            r#"
            SELECT 
                schemaname, tablename, indexname,
                pg_relation_size(i.indexrelid) as index_size
            FROM pg_stat_user_indexes i
            JOIN pg_index idx ON i.indexrelid = idx.indexrelid
            WHERE idx_scan = 0 
                AND NOT indisprimary 
                AND NOT indisunique
                AND NOT EXISTS (
                    SELECT 1 FROM pg_constraint c 
                    WHERE c.conindid = i.indexrelid
                )
            "#,
        ))
        .await
        .map_err(AppError::Database)?;

        for row in unused_indexes {
            let index_size = row.try_get::<i64, _>("index_size")?;
            recommendations.push(CostRecommendation {
                title: format!("Remove unused index {}", row.try_get::<String, _>("indexname")?),
                description: format!(
                    "Index {} on table {}.{} is never used and consumes {} MB of storage",
                    row.try_get::<String, _>("indexname")?,
                    row.try_get::<String, _>("schemaname")?,
                    row.try_get::<String, _>("tablename")?,
                    index_size / 1024 / 1024
                ),
                estimated_savings: (index_size as f64 / 1024.0 / 1024.0 / 1024.0) * 0.10, // Assuming $0.10 per GB
                implementation_effort: "Low".to_string(),
                priority: "Medium".to_string(),
            });
        }

        // Add more cost optimization recommendations...
        Ok(recommendations)
    }
}
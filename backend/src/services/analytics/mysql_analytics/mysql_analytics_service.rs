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


use crate::config::Config;
use crate::errors::AppError;
use crate::models::database::{
    ComputeMetrics, CostAnalysis, CostRecommendation, DatabaseAnalysis, DatabaseIssue,
    DatabaseQueryResponse, FrequentQuery, IndexStats, IssueCategory, IssueSeverity,
    PerformanceMetrics, QueryPlan, QueryPlanNode, QueryStatistics,
    ResourceCost, SlowQuery, StorageMetrics, TableStats, TrendDirection,
};
use chrono::{DateTime, Duration, NaiveDateTime, TimeZone, Utc};
use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, QueryResult, Statement};
use std::collections::HashMap;

pub struct MySqlAnalyticsService {
    config: Config,
}

impl MySqlAnalyticsService {
    pub fn new(config: Config) -> Self {
        Self { config }
    }

    pub async fn analyze_database(
        &self,
        conn: &DatabaseConnection,
    ) -> Result<DatabaseAnalysis, AppError> {
        let query_stats = self.get_query_statistics(conn).await?;
        let performance_metrics = self.get_performance_metrics(conn).await?;
        let storage_metrics = self.get_storage_metrics(conn).await?;
        let compute_metrics = self.get_compute_metrics(conn).await?;
        let cost_analysis = self
            .analyze_costs(conn, &storage_metrics, &compute_metrics)
            .await?;

        let mut issues = Vec::new();
        self.analyze_performance_issues(conn, &query_stats, &performance_metrics, &mut issues)
            .await?;
        self.analyze_storage_issues(&performance_metrics, &storage_metrics, &mut issues)
            .await?;
        self.analyze_security_issues(conn, &mut issues).await?;
        self.analyze_configuration_issues(conn, &mut issues).await?;

        Ok(DatabaseAnalysis {
            issues,
            query_stats,
            performance_metrics,
            cost_analysis,
        })
    }

    pub async fn get_triage_context(
        &self,
        conn: &DatabaseConnection,
    ) -> Result<serde_json::Value, AppError> {
        let query_stats = self.get_query_statistics(conn).await?;
        let performance_metrics = self.get_performance_metrics(conn).await?;
        let storage_metrics = self.get_storage_metrics(conn).await?;
        
        // Combine into a simple JSON object for LLM context
        Ok(serde_json::json!({
            "query_statistics": {
                "total_queries": query_stats.total_queries,
                "slow_queries": query_stats.slow_queries,
                "avg_query_time_ms": query_stats.avg_query_time_ms,
                "top_slow_queries": query_stats.top_slow_queries,
            },
            "performance_metrics": {
                "connection_count": performance_metrics.connection_count,
                "active_sessions": performance_metrics.active_sessions,
                "buffer_hit_ratio": performance_metrics.buffer_hit_ratio,
                "blocked_queries": performance_metrics.blocked_queries,
            },
            "storage_metrics": {
                "total_bytes": storage_metrics.total_bytes,
                "free_space_bytes": storage_metrics.free_space_bytes,
                "top_tables_by_size": storage_metrics.top_tables_by_size,
            }
        }))
    }

    async fn get_query_statistics(
        &self,
        conn: &DatabaseConnection,
    ) -> Result<QueryStatistics, AppError> {
        const SUMMARY_SQL: &str = r#"
            SELECT
                IFNULL(SUM(COUNT_STAR), 0) AS total_queries,
                IFNULL(
                    SUM(
                        CASE
                            WHEN AVG_TIMER_WAIT >= 1000000000 THEN COUNT_STAR
                            ELSE 0
                        END
                    ),
                    0
                ) AS slow_queries,
                IFNULL(AVG(AVG_TIMER_WAIT) / 1000000000, 0) AS avg_query_time_ms
            FROM performance_schema.events_statements_summary_by_digest
            WHERE DIGEST_TEXT IS NOT NULL
        "#;

        let summary_row = conn
            .query_one(Statement::from_string(
                DbBackend::MySql,
                SUMMARY_SQL.to_string(),
            ))
            .await
            .map_err(AppError::Database)?;

        let (total_queries, slow_queries, avg_query_time_ms) = if let Some(row) = summary_row {
            let total_queries = row
                .try_get::<i64>("", "total_queries")
                .map_err(AppError::Database)?;
            let slow_queries = row
                .try_get::<i64>("", "slow_queries")
                .map_err(AppError::Database)?;
            let avg_query_time_ms = row
                .try_get::<f64>("", "avg_query_time_ms")
                .map_err(AppError::Database)?;

            (total_queries, slow_queries, avg_query_time_ms)
        } else {
            (0, 0, 0.0)
        };

        let top_slow_queries = self.get_top_slow_queries(conn).await?;
        let frequent_queries = self.get_frequent_queries(conn).await?;

        Ok(QueryStatistics {
            total_queries,
            slow_queries,
            avg_query_time_ms,
            top_slow_queries,
            frequent_queries,
        })
    }

    async fn get_top_slow_queries(
        &self,
        conn: &DatabaseConnection,
    ) -> Result<Vec<SlowQuery>, AppError> {
        const SLOW_QUERIES_SQL: &str = r#"
            SELECT
                DIGEST_TEXT AS query_text,
                IFNULL(AVG_TIMER_WAIT / 1000000000, 0) AS avg_time_ms,
                COUNT_STAR AS exec_count,
                MAX(LAST_SEEN) AS last_seen
            FROM performance_schema.events_statements_summary_by_digest
            WHERE DIGEST_TEXT IS NOT NULL
            ORDER BY AVG_TIMER_WAIT DESC
            LIMIT 5
        "#;

        let rows = conn
            .query_all(Statement::from_string(
                DbBackend::MySql,
                SLOW_QUERIES_SQL.to_string(),
            ))
            .await
            .map_err(AppError::Database)?;

        let mut results = Vec::new();
        for row in rows {
            let query_text = row
                .try_get::<String>("", "query_text")
                .map_err(AppError::Database)?;
            let avg_time_ms = row
                .try_get::<f64>("", "avg_time_ms")
                .map_err(AppError::Database)?;
            let exec_count = row
                .try_get::<i64>("", "exec_count")
                .map_err(AppError::Database)?;
            let last_seen = row
                .try_get::<Option<NaiveDateTime>>("", "last_seen")
                .map_err(AppError::Database)?;

            results.push(SlowQuery {
                query: truncate_query(query_text),
                avg_execution_time_ms: avg_time_ms,
                execution_count: exec_count,
                last_execution: naive_to_utc(last_seen).unwrap_or_else(Utc::now),
                query_plan: None,
            });
        }

        Ok(results)
    }

    async fn get_frequent_queries(
        &self,
        conn: &DatabaseConnection,
    ) -> Result<Vec<FrequentQuery>, AppError> {
        const FREQUENT_QUERIES_SQL: &str = r#"
            SELECT
                DIGEST_TEXT AS query_text,
                COUNT_STAR AS exec_count,
                IFNULL((SUM_TIMER_WAIT / NULLIF(COUNT_STAR, 0)) / 1000000000, 0) AS avg_time_ms,
                IFNULL(SUM_TIMER_WAIT / 1000000000, 0) AS total_time_ms
            FROM performance_schema.events_statements_summary_by_digest
            WHERE DIGEST_TEXT IS NOT NULL
            ORDER BY COUNT_STAR DESC
            LIMIT 5
        "#;

        let rows = conn
            .query_all(Statement::from_string(
                DbBackend::MySql,
                FREQUENT_QUERIES_SQL.to_string(),
            ))
            .await
            .map_err(AppError::Database)?;

        let mut results = Vec::new();
        for row in rows {
            let query_text = row
                .try_get::<String>("", "query_text")
                .map_err(AppError::Database)?;
            let exec_count = row
                .try_get::<i64>("", "exec_count")
                .map_err(AppError::Database)?;
            let avg_time_ms = row
                .try_get::<f64>("", "avg_time_ms")
                .map_err(AppError::Database)?;
            let total_time_ms = row
                .try_get::<f64>("", "total_time_ms")
                .map_err(AppError::Database)?;

            results.push(FrequentQuery {
                query: truncate_query(query_text),
                execution_count: exec_count,
                avg_execution_time_ms: avg_time_ms,
                total_time_ms,
            });
        }

        Ok(results)
    }

    async fn get_performance_metrics(
        &self,
        conn: &DatabaseConnection,
    ) -> Result<PerformanceMetrics, AppError> {
        const STATUS_SQL: &str = r#"
            SELECT
                SUM(CASE WHEN VARIABLE_NAME = 'Threads_connected' THEN CAST(VARIABLE_VALUE AS UNSIGNED) ELSE 0 END) AS threads_connected,
                SUM(CASE WHEN VARIABLE_NAME = 'Threads_running' THEN CAST(VARIABLE_VALUE AS UNSIGNED) ELSE 0 END) AS threads_running,
                SUM(CASE WHEN VARIABLE_NAME = 'Innodb_buffer_pool_read_requests' THEN CAST(VARIABLE_VALUE AS UNSIGNED) ELSE 0 END) AS buffer_read_requests,
                SUM(CASE WHEN VARIABLE_NAME = 'Innodb_buffer_pool_reads' THEN CAST(VARIABLE_VALUE AS UNSIGNED) ELSE 0 END) AS buffer_reads,
                SUM(CASE WHEN VARIABLE_NAME = 'Innodb_deadlocks' THEN CAST(VARIABLE_VALUE AS UNSIGNED) ELSE 0 END) AS deadlocks,
                SUM(CASE WHEN VARIABLE_NAME = 'Uptime' THEN CAST(VARIABLE_VALUE AS UNSIGNED) ELSE 0 END) AS uptime_seconds,
                SUM(CASE WHEN VARIABLE_NAME = 'Innodb_buffer_pool_bytes_data' THEN CAST(VARIABLE_VALUE AS UNSIGNED) ELSE 0 END) AS buffer_bytes_data
            FROM performance_schema.global_status
            WHERE VARIABLE_NAME IN (
                'Threads_connected',
                'Threads_running',
                'Innodb_buffer_pool_read_requests',
                'Innodb_buffer_pool_reads',
                'Innodb_deadlocks',
                'Uptime',
                'Innodb_buffer_pool_bytes_data'
            )
        "#;

        let status_row = conn
            .query_one(Statement::from_string(
                DbBackend::MySql,
                STATUS_SQL.to_string(),
            ))
            .await
            .map_err(AppError::Database)?
            .ok_or_else(|| {
                AppError::Internal(
                    "Unable to collect performance schema status data for MySQL".to_string(),
                )
            })?;

        let threads_connected = status_row
            .try_get::<i64>("", "threads_connected")
            .map_err(AppError::Database)?;
        let threads_running = status_row
            .try_get::<i64>("", "threads_running")
            .map_err(AppError::Database)?;
        let buffer_read_requests = status_row
            .try_get::<i64>("", "buffer_read_requests")
            .map_err(AppError::Database)?;
        let buffer_reads = status_row
            .try_get::<i64>("", "buffer_reads")
            .map_err(AppError::Database)?;
        let deadlocks = status_row
            .try_get::<i64>("", "deadlocks")
            .map_err(AppError::Database)?;
        let uptime_seconds = status_row
            .try_get::<i64>("", "uptime_seconds")
            .map_err(AppError::Database)?;
        let buffer_bytes_data = status_row
            .try_get::<i64>("", "buffer_bytes_data")
            .unwrap_or(0);

        let buffer_hit_ratio = if buffer_read_requests > 0 {
            1.0 - (buffer_reads as f64 / buffer_read_requests as f64)
        } else {
            1.0
        };

        let blocked_queries = self.count_blocked_queries(conn).await?;
        let table_stats = self.collect_table_stats(conn).await?;
        let index_stats = self.collect_index_stats(conn).await?;

        Ok(PerformanceMetrics {
            connection_count: threads_connected as i32,
            active_sessions: threads_running as i32,
            idle_sessions: (threads_connected - threads_running).max(0) as i32,
            buffer_hit_ratio,
            cache_hit_ratio: buffer_hit_ratio,
            deadlocks,
            blocked_queries,
            table_stats,
            index_stats,
        })
    }

    async fn count_blocked_queries(&self, conn: &DatabaseConnection) -> Result<i64, AppError> {
        const BLOCKED_SQL: &str = r#"
            SELECT COUNT(*) AS blocked
            FROM information_schema.processlist
            WHERE (STATE LIKE 'Waiting%' OR STATE LIKE 'Locked%')
              AND COMMAND NOT IN ('Sleep', 'Daemon')
        "#;

        match conn
            .query_one(Statement::from_string(
                DbBackend::MySql,
                BLOCKED_SQL.to_string(),
            ))
            .await
        {
            Ok(Some(row)) => row
                .try_get::<i64>("", "blocked")
                .map_err(AppError::Database),
            Ok(None) => Ok(0),
            Err(err) => {
                tracing::warn!("Unable to query blocked sessions: {}", err);
                Ok(0)
            }
        }
    }

    async fn collect_table_stats(
        &self,
        conn: &DatabaseConnection,
    ) -> Result<Vec<TableStats>, AppError> {
        const TABLE_STATS_SQL: &str = r#"
            SELECT
                t.TABLE_NAME,
                t.TABLE_ROWS,
                t.DATA_LENGTH,
                t.INDEX_LENGTH,
                t.DATA_FREE,
                t.UPDATE_TIME,
                io.COUNT_READ,
                io.COUNT_WRITE
            FROM information_schema.tables t
            LEFT JOIN performance_schema.table_io_waits_summary_by_table io
                ON io.OBJECT_SCHEMA = t.TABLE_SCHEMA
               AND io.OBJECT_NAME = t.TABLE_NAME
            WHERE t.TABLE_SCHEMA = DATABASE()
            ORDER BY (t.DATA_LENGTH + t.INDEX_LENGTH) DESC
            LIMIT 50
        "#;

        let rows = conn
            .query_all(Statement::from_string(
                DbBackend::MySql,
                TABLE_STATS_SQL.to_string(),
            ))
            .await
            .map_err(AppError::Database)?;

        let mut stats = Vec::new();
        for row in rows {
            let table_name = row
                .try_get::<String>("", "TABLE_NAME")
                .map_err(AppError::Database)?;
            let table_rows = row.try_get::<i64>("", "TABLE_ROWS").unwrap_or(0);
            let data_length = row.try_get::<i64>("", "DATA_LENGTH").unwrap_or(0);
            let index_length = row.try_get::<i64>("", "INDEX_LENGTH").unwrap_or(0);
            let data_free = row.try_get::<i64>("", "DATA_FREE").unwrap_or(0);
            let update_time = row
                .try_get::<Option<NaiveDateTime>>("", "UPDATE_TIME")
                .unwrap_or(None);
            let count_read = row
                .try_get::<Option<i64>>("", "COUNT_READ")
                .map_err(AppError::Database)?
                .unwrap_or(0);
            let count_write = row
                .try_get::<Option<i64>>("", "COUNT_WRITE")
                .map_err(AppError::Database)?
                .unwrap_or(0);

            stats.push(TableStats {
                name: table_name,
                size_bytes: data_length + index_length,
                total_rows: table_rows,
                sequential_scans: count_read,
                index_scans: count_write,
                live_row_count: table_rows,
                dead_row_count: 0,
                last_vacuum: None,
                last_analyze: update_time.map(|ts| Utc.from_utc_datetime(&ts)),
            });
        }

        Ok(stats)
    }

    async fn collect_index_stats(
        &self,
        conn: &DatabaseConnection,
    ) -> Result<Vec<IndexStats>, AppError> {
        const INDEX_STATS_SQL: &str = r#"
            SELECT
                s.TABLE_NAME,
                s.INDEX_NAME,
                MAX(CASE WHEN s.NON_UNIQUE = 0 THEN 1 ELSE 0 END) AS is_unique,
                MAX(CASE WHEN s.INDEX_NAME = 'PRIMARY' THEN 1 ELSE 0 END) AS is_primary,
                IFNULL(u.COUNT_READ, 0) AS idx_reads,
                IFNULL(u.COUNT_STAR, 0) AS idx_access
            FROM information_schema.statistics s
            LEFT JOIN performance_schema.table_io_waits_summary_by_index_usage u
                ON u.OBJECT_SCHEMA = s.TABLE_SCHEMA
               AND u.OBJECT_NAME = s.TABLE_NAME
               AND u.INDEX_NAME = s.INDEX_NAME
            WHERE s.TABLE_SCHEMA = DATABASE()
            GROUP BY s.TABLE_NAME, s.INDEX_NAME
            ORDER BY idx_reads DESC
            LIMIT 50
        "#;

        let rows = conn
            .query_all(Statement::from_string(
                DbBackend::MySql,
                INDEX_STATS_SQL.to_string(),
            ))
            .await
            .map_err(AppError::Database)?;

        let mut stats = Vec::new();
        for row in rows {
            let table_name = row
                .try_get::<String>("", "TABLE_NAME")
                .map_err(AppError::Database)?;
            let index_name = row
                .try_get::<String>("", "INDEX_NAME")
                .map_err(AppError::Database)?;
            let is_unique = row.try_get::<i64>("", "is_unique").unwrap_or(0) == 1;
            let is_primary = row.try_get::<i64>("", "is_primary").unwrap_or(0) == 1;
            let idx_reads = row.try_get::<i64>("", "idx_reads").unwrap_or(0);
            let idx_access = row.try_get::<i64>("", "idx_access").unwrap_or(0);

            stats.push(IndexStats {
                name: index_name,
                table_name,
                size_bytes: 0,
                is_unique,
                is_primary,
                index_scans: idx_reads,
                rows_fetched: idx_access,
                unused_since: if idx_reads == 0 {
                    Some(Utc::now() - chrono::Duration::days(30))
                } else {
                    None
                },
            });
        }

        Ok(stats)
    }

    async fn analyze_performance_issues(
        &self,
        conn: &DatabaseConnection,
        query_stats: &QueryStatistics,
        metrics: &PerformanceMetrics,
        issues: &mut Vec<DatabaseIssue>,
    ) -> Result<(), AppError> {
        if query_stats.avg_query_time_ms > 500.0 {
            issues.push(DatabaseIssue {
                severity: IssueSeverity::High,
                category: IssueCategory::Performance,
                title: "High average query latency".to_string(),
                description: format!(
                    "Average query execution time is {:.2} ms which exceeds the recommended threshold",
                    query_stats.avg_query_time_ms
                ),
                recommendation:
                    "Investigate slow queries using performance_schema and add indexes or rewrite queries where necessary.".to_string(),
                affected_objects: query_stats
                    .top_slow_queries
                    .iter()
                    .map(|slow| truncate_query(slow.query.clone()))
                    .collect(),
            });
        }

        if query_stats.slow_queries > (query_stats.total_queries / 10) {
            issues.push(DatabaseIssue {
                severity: IssueSeverity::Medium,
                category: IssueCategory::Performance,
                title: "Elevated number of slow queries".to_string(),
                description: format!(
                    "{} out of {} queries exceed the slow threshold",
                    query_stats.slow_queries, query_stats.total_queries
                ),
                recommendation:
                    "Enable the slow query log and review the top slow statements to optimise execution plans.".to_string(),
                affected_objects: query_stats
                    .top_slow_queries
                    .iter()
                    .map(|slow| truncate_query(slow.query.clone()))
                    .collect(),
            });
        }

        if metrics.buffer_hit_ratio < 0.90 {
            issues.push(DatabaseIssue {
                severity: IssueSeverity::Medium,
                category: IssueCategory::Performance,
                title: "Low InnoDB buffer pool hit ratio".to_string(),
                description: format!(
                    "Buffer pool hit ratio is {:.2}%. Consider increasing the buffer pool size or reviewing workload patterns.",
                    metrics.buffer_hit_ratio * 100.0
                ),
                recommendation:
                    "Review innodb_buffer_pool_size configuration and monitor working set size.".to_string(),
                affected_objects: vec![],
            });
        }

        if metrics.blocked_queries > 0 {
            issues.push(DatabaseIssue {
                severity: IssueSeverity::High,
                category: IssueCategory::Performance,
                title: "Detected blocked queries".to_string(),
                description: format!(
                    "{} sessions are currently waiting on locks or resources.",
                    metrics.blocked_queries
                ),
                recommendation:
                    "Inspect INFORMATION_SCHEMA.PROCESSLIST for blocking transactions and optimise concurrency.".to_string(),
                affected_objects: vec![],
            });
        }

        const LONG_RUNNING_SQL: &str = r#"
            SELECT ID, USER, HOST, DB, COMMAND, TIME, STATE, INFO
            FROM information_schema.processlist
            WHERE COMMAND NOT IN ('Sleep', 'Daemon')
              AND TIME > 30
            ORDER BY TIME DESC
            LIMIT 5
        "#;

        if let Ok(rows) = conn
            .query_all(Statement::from_string(
                DbBackend::MySql,
                LONG_RUNNING_SQL.to_string(),
            ))
            .await
        {
            if !rows.is_empty() {
                let affected: Vec<String> = rows
                    .into_iter()
                    .filter_map(|row| {
                        let id = row.try_get::<i64>("", "ID").ok();
                        let user = row.try_get::<String>("", "USER").ok();
                        let time = row.try_get::<i64>("", "TIME").ok();
                        id.and_then(|id| {
                            user.map(|user| {
                                format!("{} ({}s) [session {}]", user, time.unwrap_or(0), id)
                            })
                        })
                    })
                    .collect();

                if !affected.is_empty() {
                    issues.push(DatabaseIssue {
                        severity: IssueSeverity::High,
                        category: IssueCategory::Performance,
                        title: "Long running sessions detected".to_string(),
                        description:
                            "Several sessions have been executing for more than 30 seconds.".to_string(),
                        recommendation:
                            "Review the identified sessions and consider terminating or optimising the underlying queries.".to_string(),
                        affected_objects: affected,
                    });
                }
            }
        }

        Ok(())
    }

    async fn analyze_storage_issues(
        &self,
        metrics: &PerformanceMetrics,
        storage_metrics: &StorageMetrics,
        issues: &mut Vec<DatabaseIssue>,
    ) -> Result<(), AppError> {
        if storage_metrics.total_bytes > 0 {
            let free_ratio =
                storage_metrics.free_space_bytes as f64 / storage_metrics.total_bytes as f64;
            if free_ratio < 0.10 {
                issues.push(DatabaseIssue {
                    severity: IssueSeverity::High,
                    category: IssueCategory::Storage,
                    title: "Storage nearly full".to_string(),
                    description: format!(
                        "Only {:.2}% free space remains in the schema.",
                        free_ratio * 100.0
                    ),
                    recommendation:
                        "Plan a storage expansion or purge/archive historical data to reclaim space.".to_string(),
                    affected_objects: vec![],
                });
            }
        }

        if let Some((table, size)) = storage_metrics
            .top_tables_by_size
            .iter()
            .max_by_key(|entry| entry.1)
        {
            if storage_metrics.total_bytes > 0
                && (*size as f64 / storage_metrics.total_bytes as f64) > 0.5
            {
                issues.push(DatabaseIssue {
                    severity: IssueSeverity::Medium,
                    category: IssueCategory::Storage,
                    title: "Single table dominates storage".to_string(),
                    description: format!(
                        "Table {} accounts for {:.2}% of total storage.",
                        table,
                        (*size as f64 / storage_metrics.total_bytes as f64) * 100.0
                    ),
                    recommendation:
                        "Consider partitioning or archiving data from the oversized table."
                            .to_string(),
                    affected_objects: vec![table.clone()],
                });
            }
        }

        let high_fragmentation: Vec<String> = metrics
            .table_stats
            .iter()
            .filter(|table| table.sequential_scans > (table.index_scans * 10))
            .map(|table| table.name.clone())
            .collect();

        if !high_fragmentation.is_empty() {
            issues.push(DatabaseIssue {
                severity: IssueSeverity::Medium,
                category: IssueCategory::Storage,
                title: "Tables suffering from heavy full scans".to_string(),
                description:
                    "Several tables are read predominantly via full scans, indicating possible missing indexes.".to_string(),
                recommendation:
                    "Review query access patterns on the listed tables and add appropriate indexes.".to_string(),
                affected_objects: high_fragmentation,
            });
        }

        Ok(())
    }

    async fn analyze_security_issues(
        &self,
        conn: &DatabaseConnection,
        issues: &mut Vec<DatabaseIssue>,
    ) -> Result<(), AppError> {
        const WEAK_USERS_SQL: &str = r#"
            SELECT USER, HOST
            FROM mysql.user
            WHERE (authentication_string IS NULL OR authentication_string = '')
              AND (plugin IS NULL OR plugin NOT IN ('unix_socket', 'auth_socket'))
        "#;

        if let Ok(rows) = conn
            .query_all(Statement::from_string(
                DbBackend::MySql,
                WEAK_USERS_SQL.to_string(),
            ))
            .await
        {
            if !rows.is_empty() {
                let users: Vec<String> = rows
                    .into_iter()
                    .filter_map(|row| {
                        let user = row.try_get::<String>("", "USER").ok();
                        let host = row.try_get::<String>("", "HOST").ok();
                        user.zip(host)
                            .map(|(user, host)| format!("{}@{}", user, host))
                    })
                    .collect();

                if !users.is_empty() {
                    issues.push(DatabaseIssue {
                        severity: IssueSeverity::High,
                        category: IssueCategory::Security,
                        title: "Accounts without passwords".to_string(),
                        description: "The following accounts have empty passwords or rely on insecure authentication plugins.".to_string(),
                        recommendation:
                            "Set strong passwords for the listed accounts or remove them if they are unused.".to_string(),
                        affected_objects: users,
                    });
                }
            }
        }

        const LOCAL_INFILE_SQL: &str = r#"
            SELECT VARIABLE_VALUE
            FROM performance_schema.global_variables
            WHERE VARIABLE_NAME = 'local_infile'
        "#;

        if let Ok(Some(row)) = conn
            .query_one(Statement::from_string(
                DbBackend::MySql,
                LOCAL_INFILE_SQL.to_string(),
            ))
            .await
        {
            if row
                .try_get::<String>("", "VARIABLE_VALUE")
                .unwrap_or_else(|_| "OFF".to_string())
                .eq_ignore_ascii_case("ON")
            {
                issues.push(DatabaseIssue {
                    severity: IssueSeverity::Medium,
                    category: IssueCategory::Security,
                    title: "local_infile enabled".to_string(),
                    description:
                        "LOCAL INFILE is enabled, which can be abused to read arbitrary files if SQL injection occurs.".to_string(),
                    recommendation:
                        "Disable LOCAL INFILE unless specifically required, or restrict to trusted users.".to_string(),
                    affected_objects: vec!["global variable local_infile".to_string()],
                });
            }
        }

        Ok(())
    }

    async fn analyze_configuration_issues(
        &self,
        conn: &DatabaseConnection,
        issues: &mut Vec<DatabaseIssue>,
    ) -> Result<(), AppError> {
        const VARIABLES_SQL: &str = r#"
            SELECT VARIABLE_NAME, VARIABLE_VALUE
            FROM performance_schema.global_variables
            WHERE VARIABLE_NAME IN (
                'slow_query_log',
                'long_query_time',
                'innodb_log_file_size',
                'innodb_buffer_pool_size',
                'max_connections'
            )
        "#;

        let rows = conn
            .query_all(Statement::from_string(
                DbBackend::MySql,
                VARIABLES_SQL.to_string(),
            ))
            .await
            .map_err(AppError::Database)?;

        let mut vars = HashMap::new();
        for row in rows {
            if let (Ok(name), Ok(value)) = (
                row.try_get::<String>("", "VARIABLE_NAME"),
                row.try_get::<String>("", "VARIABLE_VALUE"),
            ) {
                vars.insert(name.to_lowercase(), value);
            }
        }

        if vars
            .get("slow_query_log")
            .map(|value| value.eq_ignore_ascii_case("OFF"))
            .unwrap_or(true)
        {
            issues.push(DatabaseIssue {
                severity: IssueSeverity::Medium,
                category: IssueCategory::Configuration,
                title: "Slow query log disabled".to_string(),
                description:
                    "The slow query log is disabled which hinders performance troubleshooting."
                        .to_string(),
                recommendation:
                    "Enable slow_query_log and set a long_query_time appropriate for your workload."
                        .to_string(),
                affected_objects: vec![],
            });
        }

        if let Some(long_query_time) = vars.get("long_query_time") {
            if long_query_time.parse::<f64>().unwrap_or(10.0) > 5.0 {
                issues.push(DatabaseIssue {
                    severity: IssueSeverity::Low,
                    category: IssueCategory::Configuration,
                    title: "long_query_time is high".to_string(),
                    description: format!(
                        "long_query_time is set to {} seconds, reducing slow query visibility.",
                        long_query_time
                    ),
                    recommendation:
                        "Lower long_query_time to capture more actionable slow queries (e.g. 2 seconds).".to_string(),
                    affected_objects: vec![],
                });
            }
        }

        if let (Some(buffer_pool), Some(log_file)) = (
            vars.get("innodb_buffer_pool_size"),
            vars.get("innodb_log_file_size"),
        ) {
            let buffer_pool_bytes = buffer_pool.parse::<f64>().unwrap_or(0.0);
            let log_file_bytes = log_file.parse::<f64>().unwrap_or(0.0);
            if buffer_pool_bytes > 0.0 && log_file_bytes < buffer_pool_bytes * 0.05 {
                issues.push(DatabaseIssue {
                    severity: IssueSeverity::Low,
                    category: IssueCategory::Configuration,
                    title: "InnoDB log file size small relative to buffer pool".to_string(),
                    description:
                        "innodb_log_file_size is small compared to the buffer pool, which can increase checkpoint activity.".to_string(),
                    recommendation:
                        "Increase innodb_log_file_size to around 10% of the buffer pool to improve write throughput.".to_string(),
                    affected_objects: vec![
                        format!("innodb_log_file_size={} bytes", log_file_bytes as i64),
                    ],
                });
            }
        }

        if let Some(max_connections) = vars
            .get("max_connections")
            .and_then(|value| value.parse::<i64>().ok())
        {
            if max_connections > 0 {
                if let Ok(Some(status_row)) = conn
                    .query_one(Statement::from_string(
                        DbBackend::MySql,
                        "SELECT (
                            SUM(CASE WHEN VARIABLE_NAME = 'Threads_connected' THEN CAST(VARIABLE_VALUE AS UNSIGNED) ELSE 0 END)
                           / CAST(SUM(CASE WHEN VARIABLE_NAME = 'max_used_connections' THEN CAST(VARIABLE_VALUE AS UNSIGNED) ELSE 1 END) AS DECIMAL(18,2))
                        ) AS connection_ratio
                        FROM performance_schema.global_status
                        WHERE VARIABLE_NAME IN ('Threads_connected', 'max_used_connections')"
                            .to_string(),
                    ))
                    .await
                {
                    if let Ok(ratio) = status_row
                        .try_get::<Option<f64>>("", "connection_ratio")
                        .map_err(AppError::Database)
                    {
                        if ratio.unwrap_or(0.0) > 0.8 {
                            issues.push(DatabaseIssue {
                                severity: IssueSeverity::Medium,
                                category: IssueCategory::Configuration,
                                title: "Connections approaching max_connections".to_string(),
                                description:
                                    "Threads_connected is approaching the configured max_connections.".to_string(),
                                recommendation:
                                    "Increase max_connections or tune connection pooling to avoid saturation.".to_string(),
                                affected_objects: vec![format!("max_connections={}", max_connections)],
                            });
                        }
                    }
                }
            }
        }

        Ok(())
    }

    async fn execute_mysql_query(
        &self,
        conn_model: &crate::models::database::Model,
        query: &str,
        params: Option<&serde_json::Value>,
    ) -> Result<(Vec<String>, Vec<serde_json::Value>), AppError> {
        use crate::utils::database::connect_to_dynamic_database;
        use sea_orm::{ConnectionTrait, DbBackend, Statement};

        tracing::debug!(
            "Executing MySQL query on {}:{}/{}: {}",
            conn_model.host,
            conn_model.port,
            conn_model.database_name.as_deref().unwrap_or(""),
            query
        );

        if let Some(p) = params {
            tracing::debug!("With parameters: {}", p);
        }

        // Connect to the actual MySQL database
        let conn = connect_to_dynamic_database(conn_model, &self.config).await?;

        // Execute the query
        let result = conn
            .query_all(Statement::from_string(DbBackend::MySql, query.to_string()))
            .await
            .map_err(AppError::Database)?;

        let mut columns = Vec::new();
        let mut rows = Vec::new();

        if !result.is_empty() {
            // Check if this is a data-returning query
            let is_data_query = query.to_lowercase().trim().starts_with("select")
                || query.to_lowercase().trim().starts_with("show")
                || query.to_lowercase().trim().starts_with("describe")
                || query.to_lowercase().trim().starts_with("desc")
                || query.to_lowercase().trim().starts_with("explain");

            if is_data_query {
                // Get column names - try to infer from query structure or use defaults
                columns = self.get_column_names_for_query(query, conn_model);

                // Extract data for each row
                for row in &result {
                    let mut row_data = serde_json::Map::new();

                    // Try to extract values by index since column names might not work
                    for (index, col_name) in columns.iter().enumerate() {
                        let value = self.extract_value_by_index(row, index);
                        row_data.insert(col_name.clone(), value);
                    }

                    rows.push(serde_json::Value::Object(row_data));
                }
            } else {
                // Non-data queries (INSERT, UPDATE, DELETE, etc.)
                columns = vec!["result".to_string()];
                rows = vec![serde_json::json!({"result": "Query executed successfully"})];
            }
        } else {
            // Handle empty results
            if query.to_lowercase().trim().starts_with("select") {
                columns = vec!["message".to_string()];
                rows = vec![
                    serde_json::json!({"message": "Query executed successfully - no rows returned"}),
                ];
            } else {
                columns = vec!["result".to_string()];
                rows = vec![serde_json::json!({"result": "Query executed successfully"})];
            }
        }

        Ok((columns, rows))
    }

    fn get_column_names_for_query(
        &self,
        query: &str,
        conn_model: &crate::models::database::Model,
    ) -> Vec<String> {
        // First try to parse column names from SELECT query
        if let Some(parsed_cols) = self.parse_select_columns(query) {
            return parsed_cols;
        }

        // Fall back to query-specific defaults
        self.get_query_specific_columns(query, conn_model)
    }

    fn extract_value_by_index(
        &self,
        row: &sea_orm::QueryResult,
        index: usize,
    ) -> serde_json::Value {
        // Try to extract value by index - generate potential column names based on index
        let potential_names = vec![
            index.to_string(),
            format!("column_{}", index),
            format!("col_{}", index),
            format!("{}", index + 1), // 1-based indexing
        ];

        for name in potential_names {
            // Use the correct SeaORM syntax: try_get("", "column_name") for raw queries
            if let Ok(val) = row.try_get::<String>("", &name) {
                return serde_json::Value::String(val);
            } else if let Ok(val) = row.try_get::<Option<String>>("", &name) {
                return match val {
                    Some(s) => serde_json::Value::String(s),
                    None => serde_json::Value::Null,
                };
            } else if let Ok(val) = row.try_get::<i64>("", &name) {
                return serde_json::Value::Number(serde_json::Number::from(val));
            } else if let Ok(val) = row.try_get::<Option<i64>>("", &name) {
                return match val {
                    Some(n) => serde_json::Value::Number(serde_json::Number::from(n)),
                    None => serde_json::Value::Null,
                };
            } else if let Ok(val) = row.try_get::<i32>("", &name) {
                return serde_json::Value::Number(serde_json::Number::from(val));
            } else if let Ok(val) = row.try_get::<Option<i32>>("", &name) {
                return match val {
                    Some(n) => serde_json::Value::Number(serde_json::Number::from(n)),
                    None => serde_json::Value::Null,
                };
            } else if let Ok(val) = row.try_get::<f64>("", &name) {
                return serde_json::Number::from_f64(val)
                    .map(serde_json::Value::Number)
                    .unwrap_or(serde_json::Value::Null);
            } else if let Ok(val) = row.try_get::<Option<f64>>("", &name) {
                return match val {
                    Some(f) => serde_json::Number::from_f64(f)
                        .map(serde_json::Value::Number)
                        .unwrap_or(serde_json::Value::Null),
                    None => serde_json::Value::Null,
                };
            } else if let Ok(val) = row.try_get::<bool>("", &name) {
                return serde_json::Value::Bool(val);
            } else if let Ok(val) = row.try_get::<Option<bool>>("", &name) {
                return match val {
                    Some(b) => serde_json::Value::Bool(b),
                    None => serde_json::Value::Null,
                };
            }
        }

        // If all else fails, return a placeholder that shows we tried
        serde_json::Value::String(format!("column_{}_value", index))
    }

    fn parse_select_columns(&self, query: &str) -> Option<Vec<String>> {
        let query_lower = query.to_lowercase();
        if let Some(select_pos) = query_lower.find("select") {
            if let Some(from_pos) = query_lower.find("from") {
                let select_part = query[select_pos + 6..from_pos].trim();

                if select_part == "*" {
                    return None; // Let caller handle wildcard
                }

                let columns = select_part
                    .split(',')
                    .map(|s| {
                        let clean = s.trim();
                        // Handle aliases (AS keyword or space-separated)
                        if let Some(as_pos) = clean.to_lowercase().rfind(" as ") {
                            clean[as_pos + 4..].trim().to_string()
                        } else {
                            // Extract base column name, removing functions and table prefixes
                            let parts: Vec<&str> = clean.split_whitespace().collect();
                            if parts.len() > 1 && !parts.last().unwrap().contains('(') {
                                // Last part is likely an alias
                                parts.last().unwrap().to_string()
                            } else {
                                // Extract column name from first part
                                let col_part = parts[0];
                                if let Some(dot_pos) = col_part.rfind('.') {
                                    col_part[dot_pos + 1..].to_string()
                                } else {
                                    // Remove function calls like COUNT(), MAX(), etc.
                                    if let Some(paren_pos) = col_part.find('(') {
                                        if col_part.ends_with(')') {
                                            let func_name = &col_part[..paren_pos];
                                            format!("{}(...)", func_name)
                                        } else {
                                            col_part.to_string()
                                        }
                                    } else {
                                        col_part.to_string()
                                    }
                                }
                            }
                        }
                    })
                    .collect();

                return Some(columns);
            }
        }
        None
    }

    fn get_query_specific_columns(
        &self,
        query: &str,
        conn_model: &crate::models::database::Model,
    ) -> Vec<String> {
        let query_lower = query.to_lowercase();
        let query_trim = query_lower.trim();

        if query_trim.starts_with("show tables") {
            let db_name = conn_model.database_name.as_deref().unwrap_or("mysql");
            vec![format!("Tables_in_{}", db_name)]
        } else if query_trim.starts_with("show databases") {
            vec!["Database".to_string()]
        } else if query_trim.starts_with("describe ") || query_trim.starts_with("desc ") {
            vec![
                "Field".to_string(),
                "Type".to_string(),
                "Null".to_string(),
                "Key".to_string(),
                "Default".to_string(),
                "Extra".to_string(),
            ]
        } else if query_trim.starts_with("show variables") {
            vec!["Variable_name".to_string(), "Value".to_string()]
        } else if query_trim.starts_with("show status") {
            vec!["Variable_name".to_string(), "Value".to_string()]
        } else if query_trim.starts_with("show processlist") {
            vec![
                "Id".to_string(),
                "User".to_string(),
                "Host".to_string(),
                "db".to_string(),
                "Command".to_string(),
                "Time".to_string(),
                "State".to_string(),
                "Info".to_string(),
            ]
        } else {
            // Generic fallback - try to determine number of columns
            vec![
                "column_0".to_string(),
                "column_1".to_string(),
                "column_2".to_string(),
            ]
        }
    }

    fn extract_value_by_name(
        &self,
        row: &sea_orm::QueryResult,
        col_name: &str,
    ) -> serde_json::Value {
        // Try different data types for the given column name
        // SeaORM QueryResult expects try_get("table_alias", "column_name") but for raw queries, we use empty string for table alias
        if let Ok(val) = row.try_get::<String>("", col_name) {
            serde_json::Value::String(val)
        } else if let Ok(val) = row.try_get::<Option<String>>("", col_name) {
            match val {
                Some(s) => serde_json::Value::String(s),
                None => serde_json::Value::Null,
            }
        } else if let Ok(val) = row.try_get::<i64>("", col_name) {
            serde_json::Value::Number(serde_json::Number::from(val))
        } else if let Ok(val) = row.try_get::<Option<i64>>("", col_name) {
            match val {
                Some(n) => serde_json::Value::Number(serde_json::Number::from(n)),
                None => serde_json::Value::Null,
            }
        } else if let Ok(val) = row.try_get::<i32>("", col_name) {
            serde_json::Value::Number(serde_json::Number::from(val))
        } else if let Ok(val) = row.try_get::<Option<i32>>("", col_name) {
            match val {
                Some(n) => serde_json::Value::Number(serde_json::Number::from(n)),
                None => serde_json::Value::Null,
            }
        } else if let Ok(val) = row.try_get::<f64>("", col_name) {
            serde_json::Number::from_f64(val)
                .map(serde_json::Value::Number)
                .unwrap_or(serde_json::Value::Null)
        } else if let Ok(val) = row.try_get::<Option<f64>>("", col_name) {
            match val {
                Some(f) => serde_json::Number::from_f64(f)
                    .map(serde_json::Value::Number)
                    .unwrap_or(serde_json::Value::Null),
                None => serde_json::Value::Null,
            }
        } else if let Ok(val) = row.try_get::<bool>("", col_name) {
            serde_json::Value::Bool(val)
        } else if let Ok(val) = row.try_get::<Option<bool>>("", col_name) {
            match val {
                Some(b) => serde_json::Value::Bool(b),
                None => serde_json::Value::Null,
            }
        } else {
            // If all else fails, try to extract as a raw value using indexes
            tracing::warn!(
                "Could not extract value for column '{}', returning placeholder",
                col_name
            );
            serde_json::Value::String(format!("Unable to extract: {}", col_name))
        }
    }

    async fn analyze_costs(
        &self,
        conn: &DatabaseConnection,
        storage_metrics: &StorageMetrics,
        compute_metrics: &ComputeMetrics,
    ) -> Result<CostAnalysis, AppError> {
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

        let compute_cost = ResourceCost {
            current_usage: compute_metrics.cpu_usage,
            unit: "vCPU hours".to_string(),
            cost_per_unit: 0.05,
            total_cost: compute_metrics.cpu_usage * 0.05,
            trending: TrendDirection::Stable,
        };

        let network_cost = ResourceCost {
            current_usage: 0.0,
            unit: "GB".to_string(),
            cost_per_unit: 0.09,
            total_cost: 0.0,
            trending: TrendDirection::Stable,
        };

        let backup_cost = ResourceCost {
            current_usage: storage_metrics.total_bytes as f64,
            unit: "GB".to_string(),
            cost_per_unit: 0.05,
            total_cost: (storage_metrics.total_bytes as f64 / 1024.0 / 1024.0 / 1024.0) * 0.05,
            trending: TrendDirection::Stable,
        };

        let total_monthly_cost = storage_cost.total_cost
            + compute_cost.total_cost
            + network_cost.total_cost
            + backup_cost.total_cost;

        Ok(CostAnalysis {
            storage_cost,
            compute_cost,
            network_cost,
            backup_cost,
            total_monthly_cost,
            cost_recommendations: self
                .generate_cost_recommendations(conn, storage_metrics)
                .await?,
        })
    }

    async fn get_storage_metrics(
        &self,
        conn: &DatabaseConnection,
    ) -> Result<StorageMetrics, AppError> {
        const STORAGE_SQL: &str = r#"
            SELECT
                IFNULL(SUM(DATA_LENGTH + INDEX_LENGTH), 0) AS total_bytes,
                IFNULL(SUM(DATA_LENGTH), 0) AS data_bytes,
                IFNULL(SUM(INDEX_LENGTH), 0) AS index_bytes,
                IFNULL(SUM(DATA_FREE), 0) AS free_bytes
            FROM information_schema.tables
            WHERE TABLE_SCHEMA = DATABASE()
        "#;

        let storage_row = conn
            .query_one(Statement::from_string(
                DbBackend::MySql,
                STORAGE_SQL.to_string(),
            ))
            .await
            .map_err(AppError::Database)?
            .ok_or_else(|| {
                AppError::Internal("Unable to compute storage metrics for MySQL schema".to_string())
            })?;

        let total_bytes = storage_row
            .try_get::<i64>("", "total_bytes")
            .map_err(AppError::Database)?;
        let data_bytes = storage_row
            .try_get::<i64>("", "data_bytes")
            .map_err(AppError::Database)?;
        let index_bytes = storage_row
            .try_get::<i64>("", "index_bytes")
            .map_err(AppError::Database)?;
        let free_bytes = storage_row
            .try_get::<i64>("", "free_bytes")
            .map_err(AppError::Database)?;

        const TOP_TABLES_SQL: &str = r#"
            SELECT
                TABLE_NAME,
                DATA_LENGTH + INDEX_LENGTH AS total_size
            FROM information_schema.tables
            WHERE TABLE_SCHEMA = DATABASE()
            ORDER BY total_size DESC
            LIMIT 10
        "#;

        let mut top_tables = HashMap::new();
        if let Ok(rows) = conn
            .query_all(Statement::from_string(
                DbBackend::MySql,
                TOP_TABLES_SQL.to_string(),
            ))
            .await
        {
            for row in rows {
                let table_name = row
                    .try_get::<String>("", "TABLE_NAME")
                    .map_err(AppError::Database)?;
                let total_size = row
                    .try_get::<i64>("", "total_size")
                    .map_err(AppError::Database)?;
                top_tables.insert(table_name, total_size);
            }
        }

        Ok(StorageMetrics {
            total_bytes,
            user_data_bytes: data_bytes,
            index_bytes,
            free_space_bytes: free_bytes,
            growth_rate: 0.0,
            estimate_days_until_full: None,
            top_tables_by_size: top_tables,
        })
    }

    async fn get_compute_metrics(
        &self,
        conn: &DatabaseConnection,
    ) -> Result<ComputeMetrics, AppError> {
        const COMPUTE_SQL: &str = r#"
            SELECT
                SUM(CASE WHEN VARIABLE_NAME = 'Threads_running' THEN CAST(VARIABLE_VALUE AS UNSIGNED) ELSE 0 END) AS threads_running,
                SUM(CASE WHEN VARIABLE_NAME = 'Threads_connected' THEN CAST(VARIABLE_VALUE AS UNSIGNED) ELSE 0 END) AS threads_connected,
                SUM(CASE WHEN VARIABLE_NAME = 'Uptime' THEN CAST(VARIABLE_VALUE AS UNSIGNED) ELSE 0 END) AS uptime_seconds
            FROM performance_schema.global_status
            WHERE VARIABLE_NAME IN ('Threads_running', 'Threads_connected', 'Uptime')
        "#;

        const MEMORY_SQL: &str = r#"
            SELECT
                SUM(CASE WHEN VARIABLE_NAME = 'innodb_buffer_pool_size' THEN CAST(VARIABLE_VALUE AS UNSIGNED) ELSE 0 END) AS buffer_pool_size
            FROM performance_schema.global_variables
            WHERE VARIABLE_NAME = 'innodb_buffer_pool_size'
        "#;

        let compute_row = conn
            .query_one(Statement::from_string(
                DbBackend::MySql,
                COMPUTE_SQL.to_string(),
            ))
            .await
            .map_err(AppError::Database)?
            .ok_or_else(|| AppError::Internal("Unable to retrieve compute metrics".to_string()))?;

        let memory_row = conn
            .query_one(Statement::from_string(
                DbBackend::MySql,
                MEMORY_SQL.to_string(),
            ))
            .await
            .unwrap_or(None);

        let threads_running = compute_row
            .try_get::<i64>("", "threads_running")
            .map_err(AppError::Database)?;
        let threads_connected = compute_row
            .try_get::<i64>("", "threads_connected")
            .map_err(AppError::Database)?;
        let uptime_seconds = compute_row
            .try_get::<i64>("", "uptime_seconds")
            .map_err(AppError::Database)?;

        let buffer_pool_size = memory_row
            .and_then(|row| row.try_get::<i64>("", "buffer_pool_size").ok())
            .unwrap_or(0);

        Ok(ComputeMetrics {
            cpu_usage: threads_running as f64,
            memory_usage_bytes: buffer_pool_size,
            active_connections: threads_connected as i32,
            uptime_seconds: uptime_seconds as f64,
        })
    }

    async fn generate_cost_recommendations(
        &self,
        conn: &DatabaseConnection,
        storage_metrics: &StorageMetrics,
    ) -> Result<Vec<CostRecommendation>, AppError> {
        let mut recommendations = Vec::new();

        if storage_metrics.free_space_bytes as f64 > storage_metrics.total_bytes as f64 * 0.25 {
            recommendations.push(CostRecommendation {
                title: "High free space detected".to_string(),
                description: "A significant portion of allocated storage is unused. Review table bloat and consider reclaiming space using OPTIMIZE TABLE or adjusting storage provisioning.".to_string(),
                estimated_savings: (storage_metrics.free_space_bytes as f64 / 1024.0 / 1024.0
                    / 1024.0)
                    * 0.10,
                implementation_effort: "Low".to_string(),
                priority: "Medium".to_string(),
            });
        }

        // Recommend reviewing tables with excessive fragmentation
        const FRAGMENTED_TABLES_SQL: &str = r#"
            SELECT
                TABLE_NAME,
                DATA_LENGTH + INDEX_LENGTH AS total_size,
                DATA_FREE AS free_space
            FROM information_schema.tables
            WHERE TABLE_SCHEMA = DATABASE()
              AND DATA_FREE > 0
            ORDER BY free_space DESC
            LIMIT 5
        "#;

        if let Ok(rows) = conn
            .query_all(Statement::from_string(
                DbBackend::MySql,
                FRAGMENTED_TABLES_SQL.to_string(),
            ))
            .await
        {
            for row in rows {
                let table_name = row
                    .try_get::<String>("", "TABLE_NAME")
                    .map_err(AppError::Database)?;
                let free_space: i64 = row.try_get("", "free_space").map_err(AppError::Database)?;
                let total_size: i64 = row.try_get("", "total_size").map_err(AppError::Database)?;
                if free_space > 0 && total_size > 0 {
                    recommendations.push(CostRecommendation {
                        title: format!("Optimize table {}", table_name),
                        description: format!(
                            "Table {} has approximately {:.2}%% free space. Running OPTIMIZE TABLE can reclaim unused storage.",
                            table_name,
                            (free_space as f64 / total_size as f64) * 100.0
                        ),
                        estimated_savings: (free_space as f64 / 1024.0 / 1024.0 / 1024.0) * 0.10,
                        implementation_effort: "Medium".to_string(),
                        priority: "High".to_string(),
                    });
                }
            }
        }

        Ok(recommendations)
    }

    pub async fn execute_query(
        &self,
        conn_model: &crate::models::database::Model,
        query: &str,
        params: Option<&serde_json::Value>,
    ) -> Result<DatabaseQueryResponse, AppError> {
        // Implement connection logic based on connection type
        let start_time = Utc::now();
        let (columns, rows) = match conn_model.connection_type.as_str() {
            "postgres" => {
                self.execute_postgres_query(conn_model, query, params)
                    .await?
            }
            "mysql" => self.execute_mysql_query(conn_model, query, params).await?,
            "redis" => self.execute_redis_query(conn_model, query, params).await?,
            "opensearch" => {
                self.execute_opensearch_query(conn_model, query, params)
                    .await?
            }
            _ => {
                return Err(AppError::BadRequest(format!(
                    "Unsupported database type: {}",
                    conn_model.connection_type
                )))
            }
        };

        let execution_time = (Utc::now() - start_time).num_milliseconds() as u64;
        let row_count = rows.len();

        Ok(DatabaseQueryResponse {
            columns,
            rows,
            execution_time_ms: execution_time,
            row_count,
            query_plan: None,
        })
    }

    async fn execute_postgres_query(
        &self,
        conn_model: &crate::models::database::Model,
        query: &str,
        params: Option<&serde_json::Value>,
    ) -> Result<(Vec<String>, Vec<serde_json::Value>), AppError> {
        // Mock implementation for PostgreSQL queries
        tracing::debug!(
            "Would execute PostgreSQL query on {}:{}/{}: {}",
            conn_model.host,
            conn_model.port,
            conn_model.database_name.as_deref().unwrap_or(""),
            query
        );

        if let Some(p) = params {
            tracing::debug!("With parameters: {}", p);
        }

        // Mock implementation
        let columns = vec!["id".to_string(), "name".to_string(), "value".to_string()];
        let rows = vec![
            serde_json::json!({"id": 1, "name": "PostgreSQL Item 1", "value": 100}),
            serde_json::json!({"id": 2, "name": "PostgreSQL Item 2", "value": 200}),
        ];

        Ok((columns, rows))
    }

    async fn execute_redis_query(
        &self,
        conn_model: &crate::models::database::Model,
        query: &str,
        params: Option<&serde_json::Value>,
    ) -> Result<(Vec<String>, Vec<serde_json::Value>), AppError> {
        // Mock implementation for Redis queries
        tracing::debug!(
            "Would execute Redis command on {}:{}: {}",
            conn_model.host,
            conn_model.port,
            query
        );

        if let Some(p) = params {
            tracing::debug!("With parameters: {}", p);
        }

        // Mock implementation
        let columns = vec!["key".to_string(), "value".to_string()];
        let rows = vec![
            serde_json::json!({"key": "redis:key1", "value": "Redis value 1"}),
            serde_json::json!({"key": "redis:key2", "value": "Redis value 2"}),
        ];

        Ok((columns, rows))
    }

    async fn execute_opensearch_query(
        &self,
        conn_model: &crate::models::database::Model,
        query: &str,
        params: Option<&serde_json::Value>,
    ) -> Result<(Vec<String>, Vec<serde_json::Value>), AppError> {
        // Mock implementation for OpenSearch queries
        tracing::debug!(
            "Would execute OpenSearch query on {}:{}: {}",
            conn_model.host,
            conn_model.port,
            query
        );

        if let Some(p) = params {
            tracing::debug!("With parameters: {}", p);
        }

        // Mock implementation
        let columns = vec![
            "_id".to_string(),
            "_score".to_string(),
            "_source".to_string(),
        ];
        let rows = vec![
            serde_json::json!({"_id": "doc1", "_score": 1.5, "_source": {"title": "OpenSearch Document 1"}}),
            serde_json::json!({"_id": "doc2", "_score": 1.2, "_source": {"title": "OpenSearch Document 2"}}),
        ];

        Ok((columns, rows))
    }

    // Execute a query with EXPLAIN plan
    pub async fn execute_query_with_explain(
        &self,
        conn_model: &crate::models::database::Model,
        query: &str,
        params: Option<&serde_json::Value>,
    ) -> Result<DatabaseQueryResponse, AppError> {
        // First get regular query results
        let mut response = self.execute_query(conn_model, query, params).await?;

        // Only PostgreSQL and MySQL support EXPLAIN
        match conn_model.connection_type.as_str() {
            "postgres" => {
                // Execute EXPLAIN command
                let explain_query = format!("EXPLAIN (FORMAT JSON) {}", query);
                let (_columns, rows) = self
                    .execute_postgres_query(conn_model, &explain_query, params)
                    .await?;

                if !rows.is_empty() && rows[0].is_object() {
                    // In a real implementation, you would parse the JSON plan into your QueryPlan structure
                    // Here we're just creating a simplified example plan
                    response.query_plan = Some(QueryPlan {
                        plan_type: "PostgreSQL".to_string(),
                        total_cost: 0.0, // Would be extracted from the actual plan
                        planning_time_ms: 0.0,
                        execution_time_ms: response.execution_time_ms as f64,
                        nodes: vec![QueryPlanNode {
                            node_type: "Root".to_string(),
                            actual_rows: response.row_count as i64,
                            plan_rows: response.row_count as i64,
                            actual_time_ms: response.execution_time_ms as f64,
                            total_cost: 0.0,
                            description: "Query Plan Root".to_string(),
                            children: Vec::new(),
                        }],
                    });
                }
            }
            "mysql" => {
                // For MySQL, we'd use a different EXPLAIN syntax
                // But for now, just use the same mock plan for simplicity
                response.query_plan = Some(QueryPlan {
                    plan_type: "MySQL".to_string(),
                    total_cost: 0.0,
                    planning_time_ms: 0.0,
                    execution_time_ms: response.execution_time_ms as f64,
                    nodes: vec![QueryPlanNode {
                        node_type: "Root".to_string(),
                        actual_rows: response.row_count as i64,
                        plan_rows: response.row_count as i64,
                        actual_time_ms: response.execution_time_ms as f64,
                        total_cost: 0.0,
                        description: "MySQL Query Plan".to_string(),
                        children: Vec::new(),
                    }],
                });
            }
            _ => {
                // Other database types don't support EXPLAIN
                // Just return the query results as is
            }
        }

        Ok(response)
    }
}

fn truncate_query(query: String) -> String {
    const MAX_LEN: usize = 512;
    if query.len() > MAX_LEN {
        let mut truncated = query.chars().take(MAX_LEN).collect::<String>();
        truncated.push('…');
        truncated
    } else {
        query
    }
}

fn naive_to_utc(value: Option<NaiveDateTime>) -> Option<DateTime<Utc>> {
    value.map(|dt| Utc.from_utc_datetime(&dt))
}

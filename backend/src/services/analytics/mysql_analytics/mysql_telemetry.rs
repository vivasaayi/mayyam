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
use chrono::{DateTime, Utc};
use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, QueryResult, Statement};
use serde::Serialize;
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize)]
pub struct MySqlTelemetrySnapshot {
    pub collected_at: DateTime<Utc>,
    pub server: MySqlServerContext,
    pub workload: MySqlWorkloadSnapshot,
    pub connections: MySqlConnectionSnapshot,
    pub innodb: MySqlInnoDbSnapshot,
    pub statements: Vec<MySqlStatementDigest>,
    pub tables: Vec<MySqlTableTelemetry>,
    pub partitions: Vec<MySqlPartitionTelemetry>,
    pub indexes: Vec<MySqlIndexTelemetry>,
    pub privileges: Vec<MySqlPrivilegeTelemetry>,
    pub waits: Vec<MySqlWaitTelemetry>,
    pub locks: MySqlLockSnapshot,
    pub findings: Vec<MySqlFinding>,
}

#[derive(Debug, Clone, Serialize)]
pub struct MySqlServerContext {
    pub version: Option<String>,
    pub uptime_seconds: i64,
    pub have_ssl: Option<String>,
    pub require_secure_transport: Option<String>,
    pub log_bin: Option<String>,
    pub binlog_expire_logs_seconds: Option<i64>,
    pub expire_logs_days: Option<i64>,
    pub gtid_mode: Option<String>,
    pub performance_schema_enabled: Option<String>,
    pub slow_query_log_enabled: Option<String>,
    pub long_query_time_seconds: Option<f64>,
    pub sys_schema_available: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct MySqlWorkloadSnapshot {
    pub questions: i64,
    pub queries: i64,
    pub com_select: i64,
    pub com_insert: i64,
    pub com_update: i64,
    pub com_delete: i64,
    pub slow_queries: i64,
    pub created_tmp_tables: i64,
    pub created_tmp_disk_tables: i64,
    pub created_tmp_files: i64,
    pub tmp_disk_table_pct: Option<f64>,
    pub sort_merge_passes: i64,
    pub sort_range: i64,
    pub sort_rows: i64,
    pub sort_scan: i64,
    pub sort_merge_pass_pct: Option<f64>,
    pub select_full_join: i64,
    pub select_full_range_join: i64,
    pub select_range_check: i64,
    pub full_join_select_pct: Option<f64>,
    pub ssl_accepts: i64,
    pub ssl_finished_accepts: i64,
    pub ssl_accept_pct: Option<f64>,
    pub qps_since_start: f64,
    pub read_write_ratio: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct MySqlConnectionSnapshot {
    pub max_connections: i64,
    pub max_used_connections: i64,
    pub threads_connected: i64,
    pub threads_running: i64,
    pub threads_cached: i64,
    pub connection_usage_pct: Option<f64>,
    pub peak_connection_usage_pct: Option<f64>,
    pub aborted_clients: i64,
    pub aborted_connects: i64,
    pub connection_errors: HashMap<String, i64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct MySqlInnoDbSnapshot {
    pub buffer_pool_hit_ratio: Option<f64>,
    pub buffer_pool_pages_total: i64,
    pub buffer_pool_pages_free: i64,
    pub buffer_pool_pages_dirty: i64,
    pub buffer_pool_dirty_pct: Option<f64>,
    pub buffer_pool_free_pct: Option<f64>,
    pub log_waits: i64,
    pub row_lock_waits: i64,
    pub row_lock_time_ms: i64,
    pub deadlocks: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct MySqlStatementDigest {
    pub digest: Option<String>,
    pub schema_name: Option<String>,
    pub digest_text: String,
    pub execution_count: i64,
    pub total_time_ms: f64,
    pub avg_time_ms: f64,
    pub max_time_ms: f64,
    pub rows_examined: i64,
    pub rows_sent: i64,
    pub rows_examined_per_row_sent: Option<f64>,
    pub no_index_used_count: i64,
    pub no_good_index_used_count: i64,
    pub first_seen: Option<String>,
    pub last_seen: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct MySqlTableTelemetry {
    pub schema_name: String,
    pub table_name: String,
    pub engine: Option<String>,
    pub table_rows: i64,
    pub data_length: i64,
    pub index_length: i64,
    pub data_free: i64,
    pub read_count: i64,
    pub write_count: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct MySqlPartitionTelemetry {
    pub schema_name: String,
    pub table_name: String,
    pub partition_name: String,
    pub partition_method: Option<String>,
    pub partition_expression: Option<String>,
    pub partition_description: Option<String>,
    pub table_rows: i64,
    pub data_length: i64,
    pub index_length: i64,
    pub data_free: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct MySqlIndexTelemetry {
    pub schema_name: String,
    pub table_name: String,
    pub index_name: String,
    pub is_unique: bool,
    pub is_primary: bool,
    pub columns: Vec<String>,
    pub read_count: i64,
    pub write_count: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct MySqlPrivilegeTelemetry {
    pub user: String,
    pub host: String,
    pub account_locked: Option<bool>,
    pub password_expired: Option<bool>,
    pub ssl_type: Option<String>,
    pub super_priv: bool,
    pub grant_priv: bool,
    pub create_user_priv: bool,
    pub file_priv: bool,
    pub process_priv: bool,
    pub shutdown_priv: bool,
    pub reload_priv: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct MySqlWaitTelemetry {
    pub event_name: String,
    pub count: i64,
    pub total_wait_ms: f64,
    pub avg_wait_ms: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct MySqlLockSnapshot {
    pub blocked_processes: i64,
    pub pending_metadata_locks: Option<i64>,
    pub data_lock_waits: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct MySqlFinding {
    pub severity: MySqlFindingSeverity,
    pub category: MySqlFindingCategory,
    pub title: String,
    pub evidence: Vec<String>,
    pub impact: String,
    pub recommendation: String,
    pub validation_query: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub enum MySqlFindingSeverity {
    Critical,
    High,
    Medium,
    Low,
    Info,
}

#[derive(Debug, Clone, Serialize)]
pub enum MySqlFindingCategory {
    Workload,
    Query,
    Index,
    Locking,
    Connection,
    InnoDb,
    Storage,
    Configuration,
}

pub struct MySqlTelemetryCollector;

impl MySqlTelemetryCollector {
    pub async fn collect(conn: &DatabaseConnection) -> Result<MySqlTelemetrySnapshot, AppError> {
        let status = Self::collect_status(conn).await?;
        let variables = Self::collect_variables(conn).await?;
        let sys_schema_available = Self::schema_exists(conn, "sys").await.unwrap_or(false);

        let server = Self::server_context(&status, &variables, sys_schema_available);
        let workload = Self::workload_snapshot(&status);
        let connections = Self::connection_snapshot(&status, &variables);
        let innodb = Self::innodb_snapshot(&status);
        let statements = Self::collect_statement_digests(conn).await?;
        let tables = Self::collect_table_telemetry(conn).await?;
        let partitions = Self::collect_partition_telemetry(conn).await?;
        let indexes = Self::collect_index_telemetry(conn).await?;
        let privileges = Self::collect_privilege_telemetry(conn).await?;
        let waits = Self::collect_waits(conn).await?;
        let locks = Self::collect_locks(conn).await?;

        let mut snapshot = MySqlTelemetrySnapshot {
            collected_at: Utc::now(),
            server,
            workload,
            connections,
            innodb,
            statements,
            tables,
            partitions,
            indexes,
            privileges,
            waits,
            locks,
            findings: Vec::new(),
        };
        snapshot.findings = MySqlFindingEngine::analyze(&snapshot);
        Ok(snapshot)
    }

    async fn collect_status(conn: &DatabaseConnection) -> Result<HashMap<String, i64>, AppError> {
        const SQL: &str = r#"
            SELECT VARIABLE_NAME, VARIABLE_VALUE
            FROM performance_schema.global_status
            WHERE VARIABLE_NAME IN (
                'Aborted_clients',
                'Aborted_connects',
                'Com_delete',
                'Com_insert',
                'Com_select',
                'Com_update',
                'Connection_errors_accept',
                'Connection_errors_internal',
                'Connection_errors_max_connections',
                'Connection_errors_peer_address',
                'Connection_errors_select',
                'Connection_errors_tcpwrap',
                'Innodb_buffer_pool_pages_data',
                'Innodb_buffer_pool_pages_dirty',
                'Innodb_buffer_pool_pages_free',
                'Innodb_buffer_pool_pages_total',
                'Innodb_buffer_pool_read_requests',
                'Innodb_buffer_pool_reads',
                'Innodb_deadlocks',
                'Innodb_log_waits',
                'Innodb_row_lock_time',
                'Innodb_row_lock_waits',
                'Max_used_connections',
                'Questions',
                'Queries',
                'Slow_queries',
                'Ssl_accepts',
                'Ssl_finished_accepts',
                'Threads_cached',
                'Threads_connected',
                'Threads_running',
                'Uptime'
            )
        "#;

        let mut values = HashMap::new();
        for row in query_all(conn, SQL).await? {
            if let (Ok(name), Some(value)) = (
                row.try_get::<String>("", "VARIABLE_NAME"),
                string_value(&row, "VARIABLE_VALUE"),
            ) {
                values.insert(name, value.parse::<i64>().unwrap_or(0));
            }
        }
        Ok(values)
    }

    async fn collect_variables(
        conn: &DatabaseConnection,
    ) -> Result<HashMap<String, String>, AppError> {
        const SQL: &str = r#"
            SELECT VARIABLE_NAME, VARIABLE_VALUE
            FROM performance_schema.global_variables
            WHERE VARIABLE_NAME IN (
                'innodb_buffer_pool_size',
                'innodb_flush_log_at_trx_commit',
                'innodb_log_file_size',
                'long_query_time',
                'max_connections',
                'performance_schema',
                'binlog_expire_logs_seconds',
                'expire_logs_days',
                'gtid_mode',
                'have_ssl',
                'log_bin',
                'require_secure_transport',
                'slow_query_log',
                'thread_cache_size',
                'version'
            )
        "#;

        let mut values = HashMap::new();
        for row in query_all(conn, SQL).await? {
            if let (Ok(name), Some(value)) = (
                row.try_get::<String>("", "VARIABLE_NAME"),
                string_value(&row, "VARIABLE_VALUE"),
            ) {
                values.insert(name.to_lowercase(), value);
            }
        }
        Ok(values)
    }

    async fn schema_exists(conn: &DatabaseConnection, schema_name: &str) -> Result<bool, AppError> {
        let sql = format!(
            "SELECT COUNT(*) AS count FROM information_schema.schemata WHERE schema_name = '{}'",
            schema_name.replace('\'', "''")
        );
        let row = query_one(conn, &sql).await?;
        Ok(row.and_then(|row| i64_value(&row, "count")).unwrap_or(0) > 0)
    }

    fn server_context(
        status: &HashMap<String, i64>,
        variables: &HashMap<String, String>,
        sys_schema_available: bool,
    ) -> MySqlServerContext {
        MySqlServerContext {
            version: variables.get("version").cloned(),
            uptime_seconds: get_i64(status, "Uptime"),
            have_ssl: variables.get("have_ssl").cloned(),
            require_secure_transport: variables.get("require_secure_transport").cloned(),
            log_bin: variables.get("log_bin").cloned(),
            binlog_expire_logs_seconds: variables
                .get("binlog_expire_logs_seconds")
                .and_then(|value| value.parse::<i64>().ok()),
            expire_logs_days: variables
                .get("expire_logs_days")
                .and_then(|value| value.parse::<i64>().ok()),
            gtid_mode: variables.get("gtid_mode").cloned(),
            performance_schema_enabled: variables.get("performance_schema").cloned(),
            slow_query_log_enabled: variables.get("slow_query_log").cloned(),
            long_query_time_seconds: variables
                .get("long_query_time")
                .and_then(|value| value.parse::<f64>().ok()),
            sys_schema_available,
        }
    }

    fn workload_snapshot(status: &HashMap<String, i64>) -> MySqlWorkloadSnapshot {
        let uptime = get_i64(status, "Uptime");
        let questions = get_i64(status, "Questions");
        let selects = get_i64(status, "Com_select");
        let writes = get_i64(status, "Com_insert")
            + get_i64(status, "Com_update")
            + get_i64(status, "Com_delete");
        let created_tmp_tables = get_i64(status, "Created_tmp_tables");
        let created_tmp_disk_tables = get_i64(status, "Created_tmp_disk_tables");
        let sort_merge_passes = get_i64(status, "Sort_merge_passes");
        let sort_range = get_i64(status, "Sort_range");
        let sort_scan = get_i64(status, "Sort_scan");
        let sort_operations = sort_range + sort_scan;
        let select_full_join = get_i64(status, "Select_full_join");
        let select_full_range_join = get_i64(status, "Select_full_range_join");
        let ssl_accepts = get_i64(status, "Ssl_accepts");
        let ssl_finished_accepts = get_i64(status, "Ssl_finished_accepts");

        MySqlWorkloadSnapshot {
            questions,
            queries: get_i64(status, "Queries"),
            com_select: selects,
            com_insert: get_i64(status, "Com_insert"),
            com_update: get_i64(status, "Com_update"),
            com_delete: get_i64(status, "Com_delete"),
            slow_queries: get_i64(status, "Slow_queries"),
            created_tmp_tables,
            created_tmp_disk_tables,
            created_tmp_files: get_i64(status, "Created_tmp_files"),
            tmp_disk_table_pct: if created_tmp_tables > 0 {
                Some(created_tmp_disk_tables as f64 / created_tmp_tables as f64 * 100.0)
            } else {
                None
            },
            sort_merge_passes,
            sort_range,
            sort_rows: get_i64(status, "Sort_rows"),
            sort_scan,
            sort_merge_pass_pct: if sort_operations > 0 {
                Some(sort_merge_passes as f64 / sort_operations as f64 * 100.0)
            } else {
                None
            },
            select_full_join,
            select_full_range_join,
            select_range_check: get_i64(status, "Select_range_check"),
            full_join_select_pct: if selects > 0 {
                Some((select_full_join + select_full_range_join) as f64 / selects as f64 * 100.0)
            } else {
                None
            },
            ssl_accepts,
            ssl_finished_accepts,
            ssl_accept_pct: pct(ssl_accepts, ssl_finished_accepts),
            qps_since_start: if uptime > 0 {
                questions as f64 / uptime as f64
            } else {
                0.0
            },
            read_write_ratio: if writes > 0 {
                Some(selects as f64 / writes as f64)
            } else {
                None
            },
        }
    }

    fn connection_snapshot(
        status: &HashMap<String, i64>,
        variables: &HashMap<String, String>,
    ) -> MySqlConnectionSnapshot {
        let max_connections = variables
            .get("max_connections")
            .and_then(|value| value.parse::<i64>().ok())
            .unwrap_or(0);
        let threads_connected = get_i64(status, "Threads_connected");
        let max_used_connections = get_i64(status, "Max_used_connections");

        let mut connection_errors = HashMap::new();
        for name in [
            "Connection_errors_accept",
            "Connection_errors_internal",
            "Connection_errors_max_connections",
            "Connection_errors_peer_address",
            "Connection_errors_select",
            "Connection_errors_tcpwrap",
        ] {
            connection_errors.insert(name.to_string(), get_i64(status, name));
        }

        MySqlConnectionSnapshot {
            max_connections,
            max_used_connections,
            threads_connected,
            threads_running: get_i64(status, "Threads_running"),
            threads_cached: get_i64(status, "Threads_cached"),
            connection_usage_pct: pct(threads_connected, max_connections),
            peak_connection_usage_pct: pct(max_used_connections, max_connections),
            aborted_clients: get_i64(status, "Aborted_clients"),
            aborted_connects: get_i64(status, "Aborted_connects"),
            connection_errors,
        }
    }

    fn innodb_snapshot(status: &HashMap<String, i64>) -> MySqlInnoDbSnapshot {
        let read_requests = get_i64(status, "Innodb_buffer_pool_read_requests");
        let disk_reads = get_i64(status, "Innodb_buffer_pool_reads");
        let pages_total = get_i64(status, "Innodb_buffer_pool_pages_total");
        let pages_free = get_i64(status, "Innodb_buffer_pool_pages_free");
        let pages_dirty = get_i64(status, "Innodb_buffer_pool_pages_dirty");

        MySqlInnoDbSnapshot {
            buffer_pool_hit_ratio: if read_requests > 0 {
                Some(1.0 - (disk_reads as f64 / read_requests as f64))
            } else {
                None
            },
            buffer_pool_pages_total: pages_total,
            buffer_pool_pages_free: pages_free,
            buffer_pool_pages_dirty: pages_dirty,
            buffer_pool_dirty_pct: pct(pages_dirty, pages_total),
            buffer_pool_free_pct: pct(pages_free, pages_total),
            log_waits: get_i64(status, "Innodb_log_waits"),
            row_lock_waits: get_i64(status, "Innodb_row_lock_waits"),
            row_lock_time_ms: get_i64(status, "Innodb_row_lock_time"),
            deadlocks: get_i64(status, "Innodb_deadlocks"),
        }
    }

    async fn collect_statement_digests(
        conn: &DatabaseConnection,
    ) -> Result<Vec<MySqlStatementDigest>, AppError> {
        const SQL: &str = r#"
            SELECT
                DIGEST,
                SCHEMA_NAME,
                DIGEST_TEXT,
                COUNT_STAR,
                SUM_TIMER_WAIT,
                AVG_TIMER_WAIT,
                MAX_TIMER_WAIT,
                SUM_ROWS_EXAMINED,
                SUM_ROWS_SENT,
                SUM_NO_INDEX_USED,
                SUM_NO_GOOD_INDEX_USED,
                FIRST_SEEN,
                LAST_SEEN
            FROM performance_schema.events_statements_summary_by_digest
            WHERE DIGEST_TEXT IS NOT NULL
            ORDER BY SUM_TIMER_WAIT DESC
            LIMIT 20
        "#;

        let rows = match query_all(conn, SQL).await {
            Ok(rows) => rows,
            Err(err) => {
                tracing::warn!("Unable to collect statement digest telemetry: {}", err);
                return Ok(Vec::new());
            }
        };

        let mut digests = Vec::new();
        for row in rows {
            let rows_examined = i64_value(&row, "SUM_ROWS_EXAMINED").unwrap_or(0);
            let rows_sent = i64_value(&row, "SUM_ROWS_SENT").unwrap_or(0);
            digests.push(MySqlStatementDigest {
                digest: string_value(&row, "DIGEST"),
                schema_name: string_value(&row, "SCHEMA_NAME"),
                digest_text: string_value(&row, "DIGEST_TEXT").unwrap_or_default(),
                execution_count: i64_value(&row, "COUNT_STAR").unwrap_or(0),
                total_time_ms: picoseconds_to_ms(i64_value(&row, "SUM_TIMER_WAIT").unwrap_or(0)),
                avg_time_ms: picoseconds_to_ms(i64_value(&row, "AVG_TIMER_WAIT").unwrap_or(0)),
                max_time_ms: picoseconds_to_ms(i64_value(&row, "MAX_TIMER_WAIT").unwrap_or(0)),
                rows_examined,
                rows_sent,
                rows_examined_per_row_sent: if rows_sent > 0 {
                    Some(rows_examined as f64 / rows_sent as f64)
                } else {
                    None
                },
                no_index_used_count: i64_value(&row, "SUM_NO_INDEX_USED").unwrap_or(0),
                no_good_index_used_count: i64_value(&row, "SUM_NO_GOOD_INDEX_USED").unwrap_or(0),
                first_seen: string_value(&row, "FIRST_SEEN"),
                last_seen: string_value(&row, "LAST_SEEN"),
            });
        }
        Ok(digests)
    }

    async fn collect_table_telemetry(
        conn: &DatabaseConnection,
    ) -> Result<Vec<MySqlTableTelemetry>, AppError> {
        const SQL: &str = r#"
            SELECT
                t.TABLE_SCHEMA,
                t.TABLE_NAME,
                t.ENGINE,
                t.TABLE_ROWS,
                t.DATA_LENGTH,
                t.INDEX_LENGTH,
                t.DATA_FREE,
                IFNULL(io.COUNT_READ, 0) AS COUNT_READ,
                IFNULL(io.COUNT_WRITE, 0) AS COUNT_WRITE
            FROM information_schema.tables t
            LEFT JOIN performance_schema.table_io_waits_summary_by_table io
                ON io.OBJECT_SCHEMA = t.TABLE_SCHEMA
               AND io.OBJECT_NAME = t.TABLE_NAME
            WHERE t.TABLE_SCHEMA = DATABASE()
              AND t.TABLE_TYPE = 'BASE TABLE'
            ORDER BY (t.DATA_LENGTH + t.INDEX_LENGTH) DESC
            LIMIT 50
        "#;

        let mut tables = Vec::new();
        for row in query_all(conn, SQL).await? {
            tables.push(MySqlTableTelemetry {
                schema_name: string_value(&row, "TABLE_SCHEMA").unwrap_or_default(),
                table_name: string_value(&row, "TABLE_NAME").unwrap_or_default(),
                engine: string_value(&row, "ENGINE"),
                table_rows: i64_value(&row, "TABLE_ROWS").unwrap_or(0),
                data_length: i64_value(&row, "DATA_LENGTH").unwrap_or(0),
                index_length: i64_value(&row, "INDEX_LENGTH").unwrap_or(0),
                data_free: i64_value(&row, "DATA_FREE").unwrap_or(0),
                read_count: i64_value(&row, "COUNT_READ").unwrap_or(0),
                write_count: i64_value(&row, "COUNT_WRITE").unwrap_or(0),
            });
        }
        Ok(tables)
    }

    async fn collect_partition_telemetry(
        conn: &DatabaseConnection,
    ) -> Result<Vec<MySqlPartitionTelemetry>, AppError> {
        const SQL: &str = r#"
            SELECT
                p.TABLE_SCHEMA,
                p.TABLE_NAME,
                p.PARTITION_NAME,
                p.PARTITION_METHOD,
                p.PARTITION_EXPRESSION,
                p.PARTITION_DESCRIPTION,
                p.TABLE_ROWS,
                p.DATA_LENGTH,
                p.INDEX_LENGTH,
                p.DATA_FREE
            FROM information_schema.partitions p
            JOIN information_schema.tables t
              ON t.TABLE_SCHEMA = p.TABLE_SCHEMA
             AND t.TABLE_NAME = p.TABLE_NAME
            WHERE p.TABLE_SCHEMA = DATABASE()
              AND p.PARTITION_NAME IS NOT NULL
              AND t.TABLE_TYPE = 'BASE TABLE'
            ORDER BY p.TABLE_NAME, p.PARTITION_ORDINAL_POSITION
            LIMIT 500
        "#;

        let mut partitions = Vec::new();
        for row in query_all(conn, SQL).await? {
            partitions.push(MySqlPartitionTelemetry {
                schema_name: string_value(&row, "TABLE_SCHEMA").unwrap_or_default(),
                table_name: string_value(&row, "TABLE_NAME").unwrap_or_default(),
                partition_name: string_value(&row, "PARTITION_NAME").unwrap_or_default(),
                partition_method: string_value(&row, "PARTITION_METHOD"),
                partition_expression: string_value(&row, "PARTITION_EXPRESSION"),
                partition_description: string_value(&row, "PARTITION_DESCRIPTION"),
                table_rows: i64_value(&row, "TABLE_ROWS").unwrap_or(0),
                data_length: i64_value(&row, "DATA_LENGTH").unwrap_or(0),
                index_length: i64_value(&row, "INDEX_LENGTH").unwrap_or(0),
                data_free: i64_value(&row, "DATA_FREE").unwrap_or(0),
            });
        }
        Ok(partitions)
    }

    async fn collect_index_telemetry(
        conn: &DatabaseConnection,
    ) -> Result<Vec<MySqlIndexTelemetry>, AppError> {
        const SQL: &str = r#"
            SELECT
                s.TABLE_SCHEMA,
                s.TABLE_NAME,
                s.INDEX_NAME,
                MAX(CASE WHEN s.NON_UNIQUE = 0 THEN 1 ELSE 0 END) AS IS_UNIQUE,
                MAX(CASE WHEN s.INDEX_NAME = 'PRIMARY' THEN 1 ELSE 0 END) AS IS_PRIMARY,
                GROUP_CONCAT(s.COLUMN_NAME ORDER BY s.SEQ_IN_INDEX SEPARATOR ',') AS COLUMNS,
                IFNULL(u.COUNT_READ, 0) AS COUNT_READ,
                IFNULL(u.COUNT_WRITE, 0) AS COUNT_WRITE
            FROM information_schema.statistics s
            LEFT JOIN performance_schema.table_io_waits_summary_by_index_usage u
                ON u.OBJECT_SCHEMA = s.TABLE_SCHEMA
               AND u.OBJECT_NAME = s.TABLE_NAME
               AND u.INDEX_NAME = s.INDEX_NAME
            WHERE s.TABLE_SCHEMA = DATABASE()
            GROUP BY s.TABLE_SCHEMA, s.TABLE_NAME, s.INDEX_NAME, u.COUNT_READ, u.COUNT_WRITE
            ORDER BY COUNT_READ ASC, s.TABLE_NAME, s.INDEX_NAME
            LIMIT 100
        "#;

        let mut indexes = Vec::new();
        for row in query_all(conn, SQL).await? {
            let columns = string_value(&row, "COLUMNS")
                .unwrap_or_default()
                .split(',')
                .filter(|column| !column.is_empty())
                .map(|column| column.to_string())
                .collect();

            indexes.push(MySqlIndexTelemetry {
                schema_name: string_value(&row, "TABLE_SCHEMA").unwrap_or_default(),
                table_name: string_value(&row, "TABLE_NAME").unwrap_or_default(),
                index_name: string_value(&row, "INDEX_NAME").unwrap_or_default(),
                is_unique: i64_value(&row, "IS_UNIQUE").unwrap_or(0) == 1,
                is_primary: i64_value(&row, "IS_PRIMARY").unwrap_or(0) == 1,
                columns,
                read_count: i64_value(&row, "COUNT_READ").unwrap_or(0),
                write_count: i64_value(&row, "COUNT_WRITE").unwrap_or(0),
            });
        }
        Ok(indexes)
    }

    async fn collect_privilege_telemetry(
        conn: &DatabaseConnection,
    ) -> Result<Vec<MySqlPrivilegeTelemetry>, AppError> {
        const SQL: &str = r#"
            SELECT
                User,
                Host,
                account_locked,
                password_expired,
                ssl_type,
                Super_priv,
                Grant_priv,
                Create_user_priv,
                File_priv,
                Process_priv,
                Shutdown_priv,
                Reload_priv
            FROM mysql.user
            ORDER BY User, Host
            LIMIT 200
        "#;

        let rows = match query_all(conn, SQL).await {
            Ok(rows) => rows,
            Err(err) => {
                tracing::warn!("Unable to collect MySQL privilege telemetry: {}", err);
                return Ok(Vec::new());
            }
        };

        let mut privileges = Vec::new();
        for row in rows {
            privileges.push(MySqlPrivilegeTelemetry {
                user: string_value(&row, "User").unwrap_or_default(),
                host: string_value(&row, "Host").unwrap_or_default(),
                account_locked: optional_yes_no_value(&row, "account_locked"),
                password_expired: optional_yes_no_value(&row, "password_expired"),
                ssl_type: string_value(&row, "ssl_type"),
                super_priv: yes_no_value(&row, "Super_priv"),
                grant_priv: yes_no_value(&row, "Grant_priv"),
                create_user_priv: yes_no_value(&row, "Create_user_priv"),
                file_priv: yes_no_value(&row, "File_priv"),
                process_priv: yes_no_value(&row, "Process_priv"),
                shutdown_priv: yes_no_value(&row, "Shutdown_priv"),
                reload_priv: yes_no_value(&row, "Reload_priv"),
            });
        }
        Ok(privileges)
    }

    async fn collect_waits(conn: &DatabaseConnection) -> Result<Vec<MySqlWaitTelemetry>, AppError> {
        const SQL: &str = r#"
            SELECT
                EVENT_NAME,
                COUNT_STAR,
                SUM_TIMER_WAIT,
                AVG_TIMER_WAIT
            FROM performance_schema.events_waits_summary_global_by_event_name
            WHERE COUNT_STAR > 0
              AND (
                EVENT_NAME LIKE 'wait/io/%'
                OR EVENT_NAME LIKE 'wait/lock/%'
                OR EVENT_NAME LIKE 'wait/synch/%'
              )
            ORDER BY SUM_TIMER_WAIT DESC
            LIMIT 20
        "#;

        let rows = match query_all(conn, SQL).await {
            Ok(rows) => rows,
            Err(err) => {
                tracing::warn!("Unable to collect wait telemetry: {}", err);
                return Ok(Vec::new());
            }
        };

        let mut waits = Vec::new();
        for row in rows {
            waits.push(MySqlWaitTelemetry {
                event_name: string_value(&row, "EVENT_NAME").unwrap_or_default(),
                count: i64_value(&row, "COUNT_STAR").unwrap_or(0),
                total_wait_ms: picoseconds_to_ms(i64_value(&row, "SUM_TIMER_WAIT").unwrap_or(0)),
                avg_wait_ms: picoseconds_to_ms(i64_value(&row, "AVG_TIMER_WAIT").unwrap_or(0)),
            });
        }
        Ok(waits)
    }

    async fn collect_locks(conn: &DatabaseConnection) -> Result<MySqlLockSnapshot, AppError> {
        let blocked_processes = query_one(
            conn,
            "SELECT COUNT(*) AS blocked FROM information_schema.processlist WHERE (STATE LIKE 'Waiting%' OR STATE LIKE 'Locked%') AND COMMAND NOT IN ('Sleep', 'Daemon')",
        )
        .await?
        .and_then(|row| i64_value(&row, "blocked"))
        .unwrap_or(0);

        let pending_metadata_locks = optional_count(
            conn,
            "SELECT COUNT(*) AS count FROM performance_schema.metadata_locks WHERE LOCK_STATUS = 'PENDING'",
        )
        .await;

        let data_lock_waits = optional_count(
            conn,
            "SELECT COUNT(*) AS count FROM performance_schema.data_lock_waits",
        )
        .await;

        Ok(MySqlLockSnapshot {
            blocked_processes,
            pending_metadata_locks,
            data_lock_waits,
        })
    }
}

struct MySqlFindingEngine;

impl MySqlFindingEngine {
    fn analyze(snapshot: &MySqlTelemetrySnapshot) -> Vec<MySqlFinding> {
        let mut findings = Vec::new();
        Self::connection_findings(snapshot, &mut findings);
        Self::innodb_findings(snapshot, &mut findings);
        Self::query_findings(snapshot, &mut findings);
        Self::lock_findings(snapshot, &mut findings);
        Self::index_findings(snapshot, &mut findings);
        Self::storage_findings(snapshot, &mut findings);
        findings
    }

    fn connection_findings(snapshot: &MySqlTelemetrySnapshot, findings: &mut Vec<MySqlFinding>) {
        if let Some(usage) = snapshot.connections.connection_usage_pct {
            if usage >= 90.0 {
                findings.push(MySqlFinding {
                    severity: MySqlFindingSeverity::High,
                    category: MySqlFindingCategory::Connection,
                    title: "Current connections are near max_connections".to_string(),
                    evidence: vec![
                        format!("Threads_connected = {}", snapshot.connections.threads_connected),
                        format!("max_connections = {}", snapshot.connections.max_connections),
                        format!("current usage = {:.1}%", usage),
                    ],
                    impact: "New clients may fail to connect and existing workloads may queue behind connection pool exhaustion.".to_string(),
                    recommendation: "Tune application connection pooling first; increase max_connections only after validating memory headroom.".to_string(),
                    validation_query: Some("SHOW GLOBAL STATUS WHERE Variable_name IN ('Threads_connected','Max_used_connections'); SHOW GLOBAL VARIABLES LIKE 'max_connections';".to_string()),
                });
            } else if usage >= 75.0 {
                findings.push(MySqlFinding {
                    severity: MySqlFindingSeverity::Medium,
                    category: MySqlFindingCategory::Connection,
                    title: "Connection usage is elevated".to_string(),
                    evidence: vec![format!("current connection usage = {:.1}%", usage)],
                    impact: "The database has less burst capacity for traffic spikes.".to_string(),
                    recommendation: "Review connection pool sizing and monitor Max_used_connections over the next workload peak.".to_string(),
                    validation_query: Some("SHOW GLOBAL STATUS LIKE 'Max_used_connections';".to_string()),
                });
            }
        }

        if snapshot.connections.aborted_connects > 0 {
            findings.push(MySqlFinding {
                severity: MySqlFindingSeverity::Low,
                category: MySqlFindingCategory::Connection,
                title: "Aborted connection attempts detected".to_string(),
                evidence: vec![format!(
                    "Aborted_connects = {}",
                    snapshot.connections.aborted_connects
                )],
                impact: "Clients may be failing authentication, timing out, or hitting network/connectivity issues.".to_string(),
                recommendation: "Check application credentials, network timeouts, and MySQL connection error counters.".to_string(),
                validation_query: Some("SHOW GLOBAL STATUS WHERE Variable_name LIKE 'Connection_errors_%' OR Variable_name IN ('Aborted_connects','Aborted_clients');".to_string()),
            });
        }
    }

    fn innodb_findings(snapshot: &MySqlTelemetrySnapshot, findings: &mut Vec<MySqlFinding>) {
        if let Some(hit_ratio) = snapshot.innodb.buffer_pool_hit_ratio {
            if hit_ratio < 0.95 {
                findings.push(MySqlFinding {
                    severity: MySqlFindingSeverity::High,
                    category: MySqlFindingCategory::InnoDb,
                    title: "Low InnoDB buffer pool hit ratio".to_string(),
                    evidence: vec![format!("buffer pool hit ratio = {:.2}%", hit_ratio * 100.0)],
                    impact: "Queries are more likely to wait on disk reads instead of being served from memory.".to_string(),
                    recommendation: "Check whether the working set exceeds innodb_buffer_pool_size; also inspect top queries and hot tables before resizing.".to_string(),
                    validation_query: Some("SHOW GLOBAL STATUS WHERE Variable_name IN ('Innodb_buffer_pool_read_requests','Innodb_buffer_pool_reads');".to_string()),
                });
            }
        }

        if snapshot.innodb.log_waits > 0 {
            findings.push(MySqlFinding {
                severity: MySqlFindingSeverity::Medium,
                category: MySqlFindingCategory::InnoDb,
                title: "InnoDB redo log waits detected".to_string(),
                evidence: vec![format!("Innodb_log_waits = {}", snapshot.innodb.log_waits)],
                impact: "Write transactions may be stalling because redo logging cannot keep up."
                    .to_string(),
                recommendation:
                    "Review write spikes, redo log sizing, and flush settings for this workload."
                        .to_string(),
                validation_query: Some("SHOW GLOBAL STATUS LIKE 'Innodb_log_waits';".to_string()),
            });
        }

        if snapshot.innodb.row_lock_waits > 0 {
            findings.push(MySqlFinding {
                severity: MySqlFindingSeverity::Medium,
                category: MySqlFindingCategory::Locking,
                title: "InnoDB row lock waits detected".to_string(),
                evidence: vec![
                    format!("Innodb_row_lock_waits = {}", snapshot.innodb.row_lock_waits),
                    format!("Innodb_row_lock_time = {} ms", snapshot.innodb.row_lock_time_ms),
                ],
                impact: "Concurrent transactions are waiting on row locks, which can inflate query latency.".to_string(),
                recommendation: "Inspect current transactions and slow writes; shorten transactions or add indexes that reduce locked row ranges.".to_string(),
                validation_query: Some("SHOW ENGINE INNODB STATUS;".to_string()),
            });
        }
    }

    fn query_findings(snapshot: &MySqlTelemetrySnapshot, findings: &mut Vec<MySqlFinding>) {
        for digest in snapshot.statements.iter().take(5) {
            if digest.total_time_ms > 60_000.0 || digest.max_time_ms > 5_000.0 {
                findings.push(MySqlFinding {
                    severity: MySqlFindingSeverity::High,
                    category: MySqlFindingCategory::Query,
                    title: "Statement digest dominates query time".to_string(),
                    evidence: vec![
                        format!("digest = {}", digest.digest.clone().unwrap_or_default()),
                        format!("total time = {:.0} ms", digest.total_time_ms),
                        format!("max time = {:.0} ms", digest.max_time_ms),
                        format!("executions = {}", digest.execution_count),
                        format!("query = {}", truncate(&digest.digest_text, 220)),
                    ],
                    impact: "This query family is a top contributor to database latency and resource consumption.".to_string(),
                    recommendation: "Capture EXPLAIN FORMAT=JSON for a representative query and review indexes for predicates, joins, ORDER BY, and GROUP BY.".to_string(),
                    validation_query: Some("SELECT * FROM performance_schema.events_statements_summary_by_digest ORDER BY SUM_TIMER_WAIT DESC LIMIT 5;".to_string()),
                });
            }

            if digest.rows_examined_per_row_sent.unwrap_or(0.0) >= 1_000.0
                && digest.rows_examined >= 100_000
            {
                findings.push(MySqlFinding {
                    severity: MySqlFindingSeverity::Medium,
                    category: MySqlFindingCategory::Index,
                    title: "High rows-examined waste in statement digest".to_string(),
                    evidence: vec![
                        format!("rows examined = {}", digest.rows_examined),
                        format!("rows sent = {}", digest.rows_sent),
                        format!(
                            "rows examined per row sent = {:.1}",
                            digest.rows_examined_per_row_sent.unwrap_or(0.0)
                        ),
                        format!("query = {}", truncate(&digest.digest_text, 220)),
                    ],
                    impact: "The optimizer is scanning many more rows than the query returns, often due to missing or ineffective indexes.".to_string(),
                    recommendation: "Review the query predicates and EXPLAIN key usage; consider a composite index matching the most selective filters and ordering.".to_string(),
                    validation_query: Some("EXPLAIN FORMAT=JSON <representative query>;".to_string()),
                });
            }

            if digest.no_index_used_count > 0 || digest.no_good_index_used_count > 0 {
                findings.push(MySqlFinding {
                    severity: MySqlFindingSeverity::Medium,
                    category: MySqlFindingCategory::Index,
                    title: "Statement digest reports missing or poor index usage".to_string(),
                    evidence: vec![
                        format!("SUM_NO_INDEX_USED = {}", digest.no_index_used_count),
                        format!("SUM_NO_GOOD_INDEX_USED = {}", digest.no_good_index_used_count),
                        format!("query = {}", truncate(&digest.digest_text, 220)),
                    ],
                    impact: "The query family may perform full scans or choose inefficient access paths.".to_string(),
                    recommendation: "Compare existing indexes against WHERE/JOIN/ORDER BY columns and validate with EXPLAIN.".to_string(),
                    validation_query: Some("SELECT DIGEST_TEXT, SUM_NO_INDEX_USED, SUM_NO_GOOD_INDEX_USED FROM performance_schema.events_statements_summary_by_digest ORDER BY SUM_NO_INDEX_USED + SUM_NO_GOOD_INDEX_USED DESC LIMIT 10;".to_string()),
                });
            }
        }
    }

    fn lock_findings(snapshot: &MySqlTelemetrySnapshot, findings: &mut Vec<MySqlFinding>) {
        if snapshot.locks.blocked_processes > 0
            || snapshot.locks.pending_metadata_locks.unwrap_or(0) > 0
            || snapshot.locks.data_lock_waits.unwrap_or(0) > 0
        {
            findings.push(MySqlFinding {
                severity: MySqlFindingSeverity::High,
                category: MySqlFindingCategory::Locking,
                title: "Active lock waits detected".to_string(),
                evidence: vec![
                    format!("blocked processlist sessions = {}", snapshot.locks.blocked_processes),
                    format!(
                        "pending metadata locks = {}",
                        snapshot.locks.pending_metadata_locks.unwrap_or(0)
                    ),
                    format!("data lock waits = {}", snapshot.locks.data_lock_waits.unwrap_or(0)),
                ],
                impact: "User queries or DDL may be waiting behind transactions that hold locks.".to_string(),
                recommendation: "Identify blockers before killing sessions; inspect processlist, metadata locks, and transaction age.".to_string(),
                validation_query: Some("SELECT * FROM information_schema.processlist WHERE STATE LIKE 'Waiting%' OR STATE LIKE 'Locked%';".to_string()),
            });
        }
    }

    fn index_findings(snapshot: &MySqlTelemetrySnapshot, findings: &mut Vec<MySqlFinding>) {
        let unused_indexes: Vec<String> = snapshot
            .indexes
            .iter()
            .filter(|idx| !idx.is_primary && !idx.is_unique && idx.read_count == 0)
            .take(10)
            .map(|idx| {
                format!(
                    "{}.{} ({})",
                    idx.table_name,
                    idx.index_name,
                    idx.columns.join(",")
                )
            })
            .collect();

        if !unused_indexes.is_empty() {
            findings.push(MySqlFinding {
                severity: MySqlFindingSeverity::Low,
                category: MySqlFindingCategory::Index,
                title: "Potentially unused secondary indexes".to_string(),
                evidence: unused_indexes,
                impact: "Unused indexes add write overhead and consume storage. Counters reset at server restart, so treat this as a review candidate.".to_string(),
                recommendation: "Validate over a representative workload window before dropping any index.".to_string(),
                validation_query: Some("SELECT * FROM performance_schema.table_io_waits_summary_by_index_usage WHERE INDEX_NAME IS NOT NULL ORDER BY COUNT_READ ASC LIMIT 20;".to_string()),
            });
        }
    }

    fn storage_findings(snapshot: &MySqlTelemetrySnapshot, findings: &mut Vec<MySqlFinding>) {
        for table in snapshot.tables.iter().take(10) {
            let total = table.data_length + table.index_length;
            if total > 0 && table.data_free as f64 / total as f64 >= 0.25 {
                findings.push(MySqlFinding {
                    severity: MySqlFindingSeverity::Low,
                    category: MySqlFindingCategory::Storage,
                    title: "Table has high reclaimable free space".to_string(),
                    evidence: vec![
                        format!("table = {}", table.table_name),
                        format!("data_free = {}", table.data_free),
                        format!("total size = {}", total),
                    ],
                    impact: "The table may contain fragmented or reclaimable space, increasing storage footprint.".to_string(),
                    recommendation: "Review table churn and maintenance windows before running OPTIMIZE TABLE on large production tables.".to_string(),
                    validation_query: Some("SELECT TABLE_NAME, DATA_LENGTH, INDEX_LENGTH, DATA_FREE FROM information_schema.tables WHERE TABLE_SCHEMA = DATABASE() ORDER BY DATA_FREE DESC LIMIT 10;".to_string()),
                });
            }
        }
    }
}

async fn query_all(conn: &DatabaseConnection, sql: &str) -> Result<Vec<QueryResult>, AppError> {
    conn.query_all(Statement::from_string(DbBackend::MySql, sql.to_string()))
        .await
        .map_err(AppError::Database)
}

async fn query_one(conn: &DatabaseConnection, sql: &str) -> Result<Option<QueryResult>, AppError> {
    conn.query_one(Statement::from_string(DbBackend::MySql, sql.to_string()))
        .await
        .map_err(AppError::Database)
}

async fn optional_count(conn: &DatabaseConnection, sql: &str) -> Option<i64> {
    match query_one(conn, sql).await {
        Ok(Some(row)) => i64_value(&row, "count"),
        Ok(None) => Some(0),
        Err(err) => {
            tracing::debug!("Optional MySQL telemetry query unavailable: {}", err);
            None
        }
    }
}

fn string_value(row: &QueryResult, column: &str) -> Option<String> {
    row.try_get::<String>("", column)
        .ok()
        .or_else(|| row.try_get::<Option<String>>("", column).ok().flatten())
}

fn optional_yes_no_value(row: &QueryResult, column: &str) -> Option<bool> {
    string_value(row, column).map(|value| value.eq_ignore_ascii_case("Y"))
}

fn yes_no_value(row: &QueryResult, column: &str) -> bool {
    optional_yes_no_value(row, column).unwrap_or(false)
}

fn i64_value(row: &QueryResult, column: &str) -> Option<i64> {
    row.try_get::<i64>("", column)
        .ok()
        .or_else(|| {
            row.try_get::<i32>("", column)
                .ok()
                .map(|value| value as i64)
        })
        .or_else(|| {
            row.try_get::<String>("", column)
                .ok()
                .and_then(|value| value.parse::<i64>().ok())
        })
        .or_else(|| {
            row.try_get::<Option<String>>("", column)
                .ok()
                .flatten()
                .and_then(|value| value.parse::<i64>().ok())
        })
}

fn get_i64(values: &HashMap<String, i64>, key: &str) -> i64 {
    values.get(key).copied().unwrap_or(0)
}

fn pct(numerator: i64, denominator: i64) -> Option<f64> {
    if denominator > 0 {
        Some((numerator as f64 / denominator as f64) * 100.0)
    } else {
        None
    }
}

fn picoseconds_to_ms(value: i64) -> f64 {
    value as f64 / 1_000_000_000.0
}

fn truncate(value: &str, max_chars: usize) -> String {
    if value.chars().count() <= max_chars {
        return value.to_string();
    }

    let mut truncated = value.chars().take(max_chars).collect::<String>();
    truncated.push_str("...");
    truncated
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base_snapshot() -> MySqlTelemetrySnapshot {
        MySqlTelemetrySnapshot {
            collected_at: Utc::now(),
            server: MySqlServerContext {
                version: Some("8.0".to_string()),
                uptime_seconds: 3600,
                have_ssl: Some("YES".to_string()),
                require_secure_transport: Some("ON".to_string()),
                log_bin: Some("ON".to_string()),
                binlog_expire_logs_seconds: Some(604800),
                expire_logs_days: None,
                gtid_mode: Some("ON".to_string()),
                performance_schema_enabled: Some("ON".to_string()),
                slow_query_log_enabled: Some("ON".to_string()),
                long_query_time_seconds: Some(2.0),
                sys_schema_available: true,
            },
            workload: MySqlWorkloadSnapshot {
                questions: 1000,
                queries: 1000,
                com_select: 800,
                com_insert: 100,
                com_update: 50,
                com_delete: 50,
                slow_queries: 0,
                created_tmp_tables: 0,
                created_tmp_disk_tables: 0,
                created_tmp_files: 0,
                tmp_disk_table_pct: None,
                sort_merge_passes: 0,
                sort_range: 0,
                sort_rows: 0,
                sort_scan: 0,
                sort_merge_pass_pct: None,
                select_full_join: 0,
                select_full_range_join: 0,
                select_range_check: 0,
                full_join_select_pct: None,
                ssl_accepts: 0,
                ssl_finished_accepts: 0,
                ssl_accept_pct: None,
                qps_since_start: 1.0,
                read_write_ratio: Some(4.0),
            },
            connections: MySqlConnectionSnapshot {
                max_connections: 100,
                max_used_connections: 40,
                threads_connected: 20,
                threads_running: 2,
                threads_cached: 8,
                connection_usage_pct: Some(20.0),
                peak_connection_usage_pct: Some(40.0),
                aborted_clients: 0,
                aborted_connects: 0,
                connection_errors: HashMap::new(),
            },
            innodb: MySqlInnoDbSnapshot {
                buffer_pool_hit_ratio: Some(0.99),
                buffer_pool_pages_total: 1000,
                buffer_pool_pages_free: 100,
                buffer_pool_pages_dirty: 10,
                buffer_pool_dirty_pct: Some(1.0),
                buffer_pool_free_pct: Some(10.0),
                log_waits: 0,
                row_lock_waits: 0,
                row_lock_time_ms: 0,
                deadlocks: 0,
            },
            statements: Vec::new(),
            tables: Vec::new(),
            partitions: Vec::new(),
            indexes: Vec::new(),
            privileges: Vec::new(),
            waits: Vec::new(),
            locks: MySqlLockSnapshot {
                blocked_processes: 0,
                pending_metadata_locks: Some(0),
                data_lock_waits: Some(0),
            },
            findings: Vec::new(),
        }
    }

    fn finding_titles(snapshot: &MySqlTelemetrySnapshot) -> Vec<String> {
        MySqlFindingEngine::analyze(snapshot)
            .into_iter()
            .map(|finding| finding.title)
            .collect()
    }

    #[test]
    fn flags_current_connection_saturation() {
        let mut snapshot = base_snapshot();
        snapshot.connections.threads_connected = 95;
        snapshot.connections.connection_usage_pct = Some(95.0);

        let titles = finding_titles(&snapshot);

        assert!(titles
            .iter()
            .any(|title| title == "Current connections are near max_connections"));
    }

    #[test]
    fn flags_high_rows_examined_waste() {
        let mut snapshot = base_snapshot();
        snapshot.statements.push(MySqlStatementDigest {
            digest: Some("abc".to_string()),
            schema_name: Some("app".to_string()),
            digest_text: "SELECT * FROM orders WHERE customer_id = ?".to_string(),
            execution_count: 25,
            total_time_ms: 10_000.0,
            avg_time_ms: 400.0,
            max_time_ms: 900.0,
            rows_examined: 2_000_000,
            rows_sent: 500,
            rows_examined_per_row_sent: Some(4_000.0),
            no_index_used_count: 0,
            no_good_index_used_count: 0,
            first_seen: None,
            last_seen: None,
        });

        let titles = finding_titles(&snapshot);

        assert!(titles
            .iter()
            .any(|title| title == "High rows-examined waste in statement digest"));
    }

    #[test]
    fn flags_active_lock_waits() {
        let mut snapshot = base_snapshot();
        snapshot.locks.blocked_processes = 2;
        snapshot.locks.pending_metadata_locks = Some(1);
        snapshot.locks.data_lock_waits = Some(3);

        let titles = finding_titles(&snapshot);

        assert!(titles
            .iter()
            .any(|title| title == "Active lock waits detected"));
    }

    #[test]
    fn flags_unused_secondary_indexes() {
        let mut snapshot = base_snapshot();
        snapshot.indexes.push(MySqlIndexTelemetry {
            schema_name: "app".to_string(),
            table_name: "orders".to_string(),
            index_name: "idx_unused".to_string(),
            is_unique: false,
            is_primary: false,
            columns: vec!["created_at".to_string()],
            read_count: 0,
            write_count: 100,
        });

        let titles = finding_titles(&snapshot);

        assert!(titles
            .iter()
            .any(|title| title == "Potentially unused secondary indexes"));
    }
}

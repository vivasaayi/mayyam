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

use crate::controllers::database;
use actix_web::{web, HttpResponse};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct ConnectionRequest {
    pub db_type: String,
    pub name: String,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,
    pub database: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct QueryRequest {
    pub connection_id: String,
    pub query: String,
}

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api/databases")
            .service(
                web::resource("")
                    .route(web::get().to(database::list_connections))
                    .route(web::post().to(database::create_connection)),
            )
            .service(
                web::resource("/{id}")
                    .route(web::get().to(database::get_connection))
                    .route(web::put().to(database::update_connection))
                    .route(web::delete().to(database::delete_connection)),
            )
            .service(web::resource("/{id}/test").route(web::post().to(database::test_connection)))
            .service(web::resource("/{id}/query").route(web::post().to(database::execute_query)))
            .service(web::resource("/{id}/schema").route(web::get().to(database::get_schema)))
            .service(
                web::resource("/{id}/analyze").route(web::get().to(database::analyze_database)),
            )
            .service(
                web::resource("/{id}/mysql/telemetry")
                    .route(web::get().to(database::get_mysql_telemetry)),
            )
            .service(
                web::resource("/{id}/mysql/telemetry/history")
                    .route(web::get().to(database::get_mysql_telemetry_history)),
            )
            .service(
                web::resource("/{id}/mysql/telemetry/signals")
                    .route(web::get().to(database::get_mysql_telemetry_signals)),
            )
            .service(web::resource("/mysql/performance-schema/pillars").route(
                web::get().to(database::get_mysql_performance_schema_inventory_pillar_reports),
            ))
            .service(web::resource("/postgres/pg-stat-activity/pillars").route(
                web::get().to(database::get_postgres_pg_stat_activity_inventory_pillar_reports),
            ))
            .service(web::resource("/postgres/pg-stat-statements/pillars").route(
                web::get().to(database::get_postgres_pg_stat_statements_inventory_pillar_reports),
            ))
            .service(web::resource("/postgres/pg-stat-database/pillars").route(
                web::get().to(database::get_postgres_pg_stat_database_inventory_pillar_reports),
            ))
            .service(
                web::resource("/postgres/pg-stat-io/pillars").route(
                    web::get().to(database::get_postgres_pg_stat_io_inventory_pillar_reports),
                ),
            )
            .service(
                web::resource("/mysql/performance-schema/health/pillars").route(
                    web::get().to(database::get_mysql_performance_schema_health_pillar_reports),
                ),
            )
            .service(
                web::resource("/mysql/sys-schema/pillars")
                    .route(web::get().to(database::get_mysql_sys_schema_inventory_pillar_reports)),
            )
            .service(
                web::resource("/mysql/sys-schema/health/pillars")
                    .route(web::get().to(database::get_mysql_sys_schema_health_pillar_reports)),
            )
            .service(
                web::resource("/mysql/slow-query-log/pillars").route(
                    web::get().to(database::get_mysql_slow_query_log_inventory_pillar_reports),
                ),
            )
            .service(
                web::resource("/mysql/slow-query-log/health/pillars")
                    .route(web::get().to(database::get_mysql_slow_query_log_health_pillar_reports)),
            )
            .service(web::resource("/mysql/digest-statistics/pillars").route(
                web::get().to(database::get_mysql_digest_statistics_inventory_pillar_reports),
            ))
            .service(
                web::resource("/mysql/digest-statistics/health/pillars").route(
                    web::get().to(database::get_mysql_digest_statistics_health_pillar_reports),
                ),
            )
            .service(web::resource("/mysql/innodb-buffer-pool/pillars").route(
                web::get().to(database::get_mysql_innodb_buffer_pool_inventory_pillar_reports),
            ))
            .service(
                web::resource("/mysql/innodb-buffer-pool/health/pillars").route(
                    web::get().to(database::get_mysql_innodb_buffer_pool_health_pillar_reports),
                ),
            )
            .service(
                web::resource("/mysql/binary-log/pillars")
                    .route(web::get().to(database::get_mysql_binary_log_inventory_pillar_reports)),
            )
            .service(
                web::resource("/mysql/binary-log/health/pillars")
                    .route(web::get().to(database::get_mysql_binary_log_health_pillar_reports)),
            )
            .service(
                web::resource("/mysql/redo-log/health/pillars")
                    .route(web::get().to(database::get_mysql_redo_log_health_pillar_reports)),
            )
            .service(
                web::resource("/mysql/backup-posture/pillars").route(
                    web::get().to(database::get_mysql_backup_posture_inventory_pillar_reports),
                ),
            )
            .service(
                web::resource("/mysql/restore-drills/pillars").route(
                    web::get().to(database::get_mysql_restore_drills_inventory_pillar_reports),
                ),
            )
            .service(
                web::resource("/mysql/undo-log/health/pillars")
                    .route(web::get().to(database::get_mysql_undo_log_health_pillar_reports)),
            )
            .service(
                web::resource("/mysql/parameter-drift/pillars").route(
                    web::get().to(database::get_mysql_parameter_drift_inventory_pillar_reports),
                ),
            )
            .service(web::resource("/mysql/cost-attribution/pillars").route(
                web::get().to(database::get_mysql_cost_attribution_inventory_pillar_reports),
            ))
            .service(web::resource("/mysql/ai-prompt-templates/pillars").route(
                web::get().to(database::get_mysql_ai_prompt_templates_inventory_pillar_reports),
            ))
            .service(web::resource("/mysql/replication-status/pillars").route(
                web::get().to(database::get_mysql_replication_status_inventory_pillar_reports),
            ))
            .service(web::resource("/mysql/group-replication/pillars").route(
                web::get().to(database::get_mysql_group_replication_inventory_pillar_reports),
            ))
            .service(
                web::resource("/mysql/aurora-mysql/pillars")
                    .route(web::get().to(database::get_mysql_aurora_inventory_pillar_reports)),
            )
            .service(
                web::resource("/mysql/rds-mysql/pillars")
                    .route(web::get().to(database::get_mysql_rds_inventory_pillar_reports)),
            )
            .service(web::resource("/mysql/connection-threads/pillars").route(
                web::get().to(database::get_mysql_connection_threads_inventory_pillar_reports),
            ))
            .service(
                web::resource("/mysql/metadata-locks/pillars").route(
                    web::get().to(database::get_mysql_metadata_locks_inventory_pillar_reports),
                ),
            )
            .service(
                web::resource("/mysql/deadlocks/pillars")
                    .route(web::get().to(database::get_mysql_deadlocks_inventory_pillar_reports)),
            )
            .service(web::resource("/mysql/index-cardinality/pillars").route(
                web::get().to(database::get_mysql_index_cardinality_inventory_pillar_reports),
            ))
            .service(
                web::resource("/mysql/unused-indexes/pillars").route(
                    web::get().to(database::get_mysql_unused_indexes_inventory_pillar_reports),
                ),
            )
            .service(
                web::resource("/mysql/missing-indexes/pillars").route(
                    web::get().to(database::get_mysql_missing_indexes_inventory_pillar_reports),
                ),
            )
            .service(
                web::resource("/mysql/table-bloat/pillars")
                    .route(web::get().to(database::get_mysql_table_bloat_inventory_pillar_reports)),
            )
            .service(
                web::resource("/mysql/partitioning/pillars").route(
                    web::get().to(database::get_mysql_partitioning_inventory_pillar_reports),
                ),
            )
            .service(web::resource("/mysql/temporary-tables/pillars").route(
                web::get().to(database::get_mysql_temporary_tables_inventory_pillar_reports),
            ))
            .service(
                web::resource("/mysql/sort-operations/pillars").route(
                    web::get().to(database::get_mysql_sort_operations_inventory_pillar_reports),
                ),
            )
            .service(
                web::resource("/mysql/join-buffers/pillars").route(
                    web::get().to(database::get_mysql_join_buffers_inventory_pillar_reports),
                ),
            )
            .service(
                web::resource("/mysql/query-plans/pillars")
                    .route(web::get().to(database::get_mysql_query_plans_inventory_pillar_reports)),
            )
            .service(
                web::resource("/mysql/schema-explorer/pillars").route(
                    web::get().to(database::get_mysql_schema_explorer_inventory_pillar_reports),
                ),
            )
            .service(
                web::resource("/mysql/privilege-audit/pillars").route(
                    web::get().to(database::get_mysql_privilege_audit_inventory_pillar_reports),
                ),
            )
            .service(web::resource("/mysql/tls-configuration/pillars").route(
                web::get().to(database::get_mysql_tls_configuration_inventory_pillar_reports),
            ))
            .service(
                web::resource("/mysql/redo-log/pillars")
                    .route(web::get().to(database::get_mysql_redo_log_inventory_pillar_reports)),
            )
            .service(
                web::resource("/mysql/undo-log/pillars")
                    .route(web::get().to(database::get_mysql_undo_log_inventory_pillar_reports)),
            )
            .service(
                web::resource("/mysql/wait-events/pillars")
                    .route(web::get().to(database::get_mysql_wait_events_inventory_pillar_reports)),
            )
            .service(
                web::resource("/mysql/wait-events/health/pillars")
                    .route(web::get().to(database::get_mysql_wait_events_health_pillar_reports)),
            )
            .service(
                web::resource("/{id}/table/{table_name}/details")
                    .route(web::get().to(get_table_details)),
            )
            .service(web::resource("/{id}/monitoring").route(web::get().to(get_monitoring_data))),
    );
}

async fn get_table_details(path: web::Path<(String, String)>) -> HttpResponse {
    let (id, table_name) = path.into_inner();
    HttpResponse::Ok().json(serde_json::json!({
        "table_name": table_name,
        "columns": [
            {
                "name": "id",
                "type": "int",
                "nullable": false,
                "default": null,
                "is_primary": true,
                "is_foreign_key": false,
                "is_unique": true
            },
            {
                "name": "name",
                "type": "varchar(255)",
                "nullable": false,
                "default": null,
                "is_primary": false,
                "is_foreign_key": false,
                "is_unique": false
            },
            {
                "name": "email",
                "type": "varchar(255)",
                "nullable": true,
                "default": null,
                "is_primary": false,
                "is_foreign_key": false,
                "is_unique": true
            },
            {
                "name": "created_at",
                "type": "timestamp",
                "nullable": false,
                "default": "CURRENT_TIMESTAMP",
                "is_primary": false,
                "is_foreign_key": false,
                "is_unique": false
            }
        ],
        "indexes": [
            {
                "name": "PRIMARY",
                "columns": ["id"],
                "type": "PRIMARY",
                "is_unique": true,
                "is_primary": true
            },
            {
                "name": "idx_email_unique",
                "columns": ["email"],
                "type": "UNIQUE",
                "is_unique": true,
                "is_primary": false
            },
            {
                "name": "idx_created_at",
                "columns": ["created_at"],
                "type": "INDEX",
                "is_unique": false,
                "is_primary": false
            }
        ],
        "foreign_keys": [
            {
                "constraint_name": "fk_user_profile",
                "column": "profile_id",
                "referenced_table": "profiles",
                "referenced_column": "id"
            }
        ],
        "statistics": {
            "row_count": 1250,
            "table_size_bytes": 2048000,
            "index_size_bytes": 512000,
            "avg_row_length": 1638
        }
    }))
}

async fn get_monitoring_data(
    path: web::Path<String>,
    query: web::Query<std::collections::HashMap<String, String>>,
) -> HttpResponse {
    let id = path.into_inner();
    let time_range = query.get("time_range").unwrap_or(&"1h".to_string()).clone();

    HttpResponse::Ok().json(serde_json::json!({
        "connection_id": id,
        "time_range": time_range,
        "current_metrics": {
            "cpu_usage": 65.2,
            "memory_usage_percent": 78.5,
            "active_connections": 45,
            "max_connections": 100,
            "buffer_hit_ratio": 0.95,
            "slow_queries": 3,
            "uptime_seconds": 2592000,
            "database_size_bytes": 10737418240i64,
            "data_size_bytes": 8589934592i64,
            "index_size_bytes": 2147483648i64
        },
        "time_series_data": [
            {
                "timestamp": "2025-06-23T10:00:00Z",
                "cpu_usage": 60.1,
                "memory_usage_percent": 75.2,
                "active_connections": 42
            },
            {
                "timestamp": "2025-06-23T10:05:00Z",
                "cpu_usage": 62.3,
                "memory_usage_percent": 76.8,
                "active_connections": 44
            },
            {
                "timestamp": "2025-06-23T10:10:00Z",
                "cpu_usage": 65.2,
                "memory_usage_percent": 78.5,
                "active_connections": 45
            }
        ],
        "active_connections": [
            {
                "id": 1,
                "user": "app_user",
                "host": "192.168.1.100",
                "database": "mayyam_db",
                "command": "Query",
                "time": 15,
                "state": "executing"
            },
            {
                "id": 2,
                "user": "readonly_user",
                "host": "192.168.1.101",
                "database": "mayyam_db",
                "command": "Sleep",
                "time": 300,
                "state": "idle"
            }
        ],
        "alerts": [
            {
                "title": "High CPU Usage",
                "description": "CPU usage has been above 80% for the past 10 minutes",
                "severity": "warning",
                "timestamp": "2025-06-23T10:05:00Z"
            }
        ]
    }))
}

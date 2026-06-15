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

pub mod ai_prompt_templates_inventory;
pub mod aurora_mysql_inventory;
pub mod backup_posture_inventory;
pub mod binary_log_inventory;
pub mod connection_threads_inventory;
pub mod cost_attribution_inventory;
pub mod deadlocks_inventory;
pub mod digest_statistics_health;
pub mod digest_statistics_inventory;
pub mod group_replication_inventory;
pub mod index_cardinality_inventory;
pub mod innodb_buffer_pool_inventory;
pub mod join_buffers_inventory;
pub mod metadata_locks_inventory;
pub mod missing_indexes_inventory;
pub mod mysql_analytics_service;
pub mod mysql_signals;
pub mod mysql_telemetry;
pub mod parameter_drift_inventory;
pub mod partitioning_inventory;
pub mod performance_schema_health;
pub mod performance_schema_inventory;
pub mod privilege_audit_inventory;
pub mod query_plans_inventory;
pub mod rds_mysql_inventory;
pub mod redo_log_inventory;
pub mod replication_status_inventory;
pub mod restore_drills_inventory;
pub mod schema_explorer_inventory;
pub mod slow_query_log_health;
pub mod slow_query_log_inventory;
pub mod sort_operations_inventory;
pub mod sys_schema_health;
pub mod sys_schema_inventory;
pub mod table_bloat_inventory;
pub mod temporary_tables_inventory;
pub mod tls_configuration_inventory;
pub mod undo_log_inventory;
pub mod unused_indexes_inventory;
pub mod wait_events_inventory;
pub use mysql_analytics_service::MySqlAnalyticsService;
pub use mysql_telemetry::{MySqlTelemetryCollector, MySqlTelemetrySnapshot};

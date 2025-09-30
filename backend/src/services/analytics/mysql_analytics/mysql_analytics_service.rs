use crate::config::Config;
use crate::errors::AppError;
use crate::models::database::{
    ComputeMetrics, CostAnalysis, CostRecommendation, DatabaseAnalysis, DatabaseIssue,
    DatabaseQueryResponse, PerformanceMetrics, QueryPlan, QueryPlanNode, QueryStatistics,
    ResourceCost, StorageMetrics, TrendDirection,
};
use chrono::Utc;
use sea_orm::DatabaseConnection;

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
        let mut analysis = DatabaseAnalysis {
            issues: Vec::new(),
            query_stats: self.get_query_statistics(conn).await?,
            performance_metrics: self.get_performance_metrics(conn).await?,
            cost_analysis: self.analyze_costs(conn).await?,
        };

        // Analyze and collect issues
        self.analyze_performance_issues(conn, &mut analysis.issues)
            .await?;
        self.analyze_storage_issues(conn, &mut analysis.issues)
            .await?;
        self.analyze_security_issues(conn, &mut analysis.issues)
            .await?;
        self.analyze_configuration_issues(conn, &mut analysis.issues)
            .await?;

        Ok(analysis)
    }

    async fn get_query_statistics(
        &self,
        _conn: &DatabaseConnection,
    ) -> Result<QueryStatistics, AppError> {
        Ok(QueryStatistics {
            total_queries: 0,
            slow_queries: 0,
            avg_query_time_ms: 0.0,
            top_slow_queries: vec![],
            frequent_queries: vec![],
        })
    }

    async fn get_performance_metrics(
        &self,
        _conn: &DatabaseConnection,
    ) -> Result<PerformanceMetrics, AppError> {
        // Mock implementation
        Ok(PerformanceMetrics {
            connection_count: 0,
            active_sessions: 0,
            idle_sessions: 0,
            buffer_hit_ratio: 0.0,
            cache_hit_ratio: 0.0,
            deadlocks: 0,
            blocked_queries: 0,
            table_stats: vec![],
            index_stats: vec![],
        })
    }

    async fn analyze_performance_issues(
        &self,
        _conn: &DatabaseConnection,
        _issues: &mut Vec<DatabaseIssue>,
    ) -> Result<(), AppError> {
        Ok(())
    }

    async fn analyze_storage_issues(
        &self,
        _conn: &DatabaseConnection,
        _issues: &mut Vec<DatabaseIssue>,
    ) -> Result<(), AppError> {
        Ok(())
    }

    async fn analyze_security_issues(
        &self,
        _conn: &DatabaseConnection,
        _issues: &mut Vec<DatabaseIssue>,
    ) -> Result<(), AppError> {
        Ok(())
    }

    async fn analyze_configuration_issues(
        &self,
        _conn: &DatabaseConnection,
        _issues: &mut Vec<DatabaseIssue>,
    ) -> Result<(), AppError> {
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

    async fn get_storage_metrics(
        &self,
        _conn: &DatabaseConnection,
    ) -> Result<StorageMetrics, AppError> {
        // Mock implementation
        Ok(StorageMetrics {
            total_bytes: 10_737_418_240, // 10 GB
            user_data_bytes: 0,
            index_bytes: 0,
            growth_rate: 0.05, // 5% growth
            estimate_days_until_full: None,
            free_space_bytes: 0,
            top_tables_by_size: Default::default(),
        })
    }

    async fn get_compute_metrics(
        &self,
        _conn: &DatabaseConnection,
    ) -> Result<ComputeMetrics, AppError> {
        // Mock implementation
        Ok(ComputeMetrics {
            cpu_usage: 45.0,
            memory_usage_bytes: 8_589_934_592, // 8 GB
            active_connections: 120,
            uptime_seconds: 0.0,
        })
    }

    async fn generate_cost_recommendations(
        &self,
        _conn: &DatabaseConnection,
    ) -> Result<Vec<CostRecommendation>, AppError> {
        // Mock implementation
        Ok(vec![])
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

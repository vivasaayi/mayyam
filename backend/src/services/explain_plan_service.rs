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


use crate::models::explain_plan::ExplainPlan;
use crate::repositories::explain_plan_repository::ExplainPlanRepository;
use crate::repositories::query_fingerprint_repository::QueryFingerprintRepository;
use crate::repositories::aurora_cluster_repository::AuroraClusterRepository;
use uuid::Uuid;
use sea_orm::{Database, ConnectionTrait, Statement};
use serde::Serialize;
use serde_json;
use std::collections::HashMap;

#[derive(Clone)]
pub struct ExplainPlanService {
    explain_repo: ExplainPlanRepository,
    fingerprint_repo: QueryFingerprintRepository,
    cluster_repo: AuroraClusterRepository,
}

#[derive(Debug, Clone, Serialize)]
pub struct ExplainPlanAnalysis {
    pub plan_id: Uuid,
    pub uses_indexes: bool,
    pub has_full_scan: bool,
    pub has_filesort: bool,
    pub has_temp_table: bool,
    pub estimated_rows: Option<i64>,
    pub actual_rows: Option<i64>,
    pub execution_time: Option<f64>,
    pub cost: Option<f64>,
    pub analysis: serde_json::Value,
    pub recommendations: Vec<String>,
    pub optimization_flags: Vec<String>,
}

impl ExplainPlanService {
    pub fn new(
        explain_repo: ExplainPlanRepository,
        fingerprint_repo: QueryFingerprintRepository,
        cluster_repo: AuroraClusterRepository,
    ) -> Self {
        Self {
            explain_repo,
            fingerprint_repo,
            cluster_repo,
        }
    }

    pub async fn execute_explain_plan(&self, cluster_id: Uuid, fingerprint_id: Uuid) -> Result<ExplainPlan, String> {
        let cluster = self.cluster_repo.find_by_id(cluster_id).await?
            .ok_or_else(|| "Aurora cluster not found".to_string())?;

        let fingerprint = self.fingerprint_repo.find_by_id(fingerprint_id).await?
            .ok_or_else(|| "Query fingerprint not found".to_string())?;

        let dsn = &cluster.read_only_dsn;
        let db = Database::connect(dsn).await
            .map_err(|e| format!("Failed to connect to cluster for EXPLAIN: {}", e))?;

        let (query, format) = match cluster.engine.to_lowercase().as_str() {
            "mysql" | "aurora-mysql" => (format!("EXPLAIN FORMAT=JSON {}", fingerprint.normalized_sql), "JSON"),
            _ => (format!("EXPLAIN (FORMAT JSON, ANALYZE) {}", fingerprint.normalized_sql), "JSON"),
        };

        let backend = db.get_database_backend();
        let res = db.query_one(Statement::from_string(backend, query)).await
            .map_err(|e| format!("Failed to execute EXPLAIN: {}", e))?
            .ok_or_else(|| "EXPLAIN returned no results".to_string())?;

        let plan_data: String = res.try_get_by_index(0).map_err(|e| e.to_string())?;
        let plan_json: serde_json::Value = serde_json::from_str(&plan_data).unwrap_or(serde_json::Value::Null);

        let explain_plan = ExplainPlan {
            id: Uuid::new_v4(),
            cluster_id,
            fingerprint_id,
            sql_text: fingerprint.normalized_sql.clone(),
            plan_format: format.to_string(),
            plan_data: plan_data,
            engine_version: "Unknown".to_string(),
            captured_at: chrono::Utc::now().naive_utc(),
            is_before_optimization: true,
            has_full_scan: false,
            has_filesort: false,
            has_temp_table: false,
            estimated_rows: None,
            actual_rows: None,
            execution_time: None,
            created_at: chrono::Utc::now().naive_utc(),
        };

        let created_plan = self.explain_repo.create(explain_plan).await?;
        let analysis = self.analyze_explain_plan(&created_plan)?;
        self.update_plan_analysis(created_plan.id, &analysis).await?;

        Ok(created_plan)
    }

    pub async fn capture_explain_plan(
        &self,
        cluster_id: Uuid,
        fingerprint_id: Uuid,
        sql: &str,
        plan_format: &str,
        plan_data: serde_json::Value,
    ) -> Result<ExplainPlan, String> {
        let explain_plan = ExplainPlan {
            id: Uuid::new_v4(),
            cluster_id,
            fingerprint_id,
            sql_text: sql.to_string(),
            plan_format: plan_format.to_string(),
            plan_data: plan_data.to_string(),
            engine_version: "Unknown".to_string(),
            captured_at: chrono::Utc::now().naive_utc(),
            is_before_optimization: false,
            has_full_scan: false,
            has_filesort: false,
            has_temp_table: false,
            estimated_rows: None,
            actual_rows: None,
            execution_time: None,
            created_at: chrono::Utc::now().naive_utc(),
        };

        let created_plan = self.explain_repo.create(explain_plan).await?;
        let analysis = self.analyze_explain_plan(&created_plan)?;
        self.update_plan_analysis(created_plan.id, &analysis).await?;

        Ok(created_plan)
    }

    pub async fn create_explain_plan(
        &self,
        fingerprint_id: Uuid,
        cluster_id: Uuid,
        plan_data: serde_json::Value,
        plan_format: String,
        execution_time_ms: Option<f64>,
        _total_cost: Option<f64>,
    ) -> Result<ExplainPlan, String> {
        let fingerprint = self.fingerprint_repo.find_by_id(fingerprint_id).await?
            .ok_or_else(|| "Query fingerprint not found".to_string())?;

        let mut explain_plan = self.capture_explain_plan(
            cluster_id,
            fingerprint_id,
            &fingerprint.normalized_sql,
            &plan_format,
            plan_data,
        ).await?;

        if let Some(exec_time) = execution_time_ms {
            explain_plan.execution_time = Some(exec_time);
        }

        Ok(explain_plan)
    }

    pub async fn compare_plans(&self, fingerprint_id: Uuid, limit: u64) -> Result<Vec<ExplainPlan>, String> {
        self.explain_repo.compare_plans(fingerprint_id, limit).await
    }

    pub async fn get_plan_analysis(&self, plan_id: Uuid) -> Result<ExplainPlanAnalysis, String> {
        let plan = self.explain_repo.find_by_id(plan_id).await?
            .ok_or_else(|| "Explain plan not found".to_string())?;

        self.analyze_explain_plan(&plan)
    }

    pub fn analyze_explain_plan(&self, plan: &ExplainPlan) -> Result<ExplainPlanAnalysis, String> {
        let mut analysis = ExplainPlanAnalysis {
            plan_id: plan.id,
            uses_indexes: false,
            has_full_scan: false,
            has_filesort: false,
            has_temp_table: false,
            estimated_rows: None,
            actual_rows: None,
            execution_time: None,
            cost: None,
            analysis: serde_json::json!({}),
            recommendations: Vec::new(),
            optimization_flags: Vec::new(),
        };

        match plan.plan_format.as_str() {
            "JSON" => {
                let plan_json: serde_json::Value = serde_json::from_str(&plan.plan_data)
                    .map_err(|e| format!("Failed to parse JSON plan: {}", e))?;
                
                // Detection based on structure
                if plan_json.is_array() {
                    self.analyze_postgresql_json_plan(&plan_json, &mut analysis)?;
                } else {
                    self.analyze_mysql_json_plan(&plan_json, &mut analysis)?;
                }
            },
            "TRADITIONAL" => self.analyze_traditional_plan(&serde_json::Value::String(plan.plan_data.clone()), &mut analysis)?,
            _ => {}
        }

        Ok(analysis)
    }

    fn analyze_mysql_json_plan(&self, plan_data: &serde_json::Value, analysis: &mut ExplainPlanAnalysis) -> Result<(), String> {
        if let Some(query_block) = plan_data.get("query_block") {
            self.analyze_mysql_query_block(query_block, analysis)?;
        }
        Ok(())
    }

    fn analyze_mysql_query_block(&self, query_block: &serde_json::Value, analysis: &mut ExplainPlanAnalysis) -> Result<(), String> {
        if let Some(table) = query_block.get("table") {
            if let Some(access_type) = table.get("access_type").and_then(|t| t.as_str()) {
                match access_type {
                    "ALL" => analysis.has_full_scan = true,
                    "index" | "range" | "ref" | "eq_ref" | "const" => analysis.uses_indexes = true,
                    _ => {}
                }
            }
        }
        if let Some(ordering) = query_block.get("ordering_operation") {
            if ordering.get("using_filesort").and_then(|v| v.as_bool()).unwrap_or(false) {
                analysis.has_filesort = true;
            }
        }
        Ok(())
    }

    fn analyze_postgresql_json_plan(&self, plan_data: &serde_json::Value, analysis: &mut ExplainPlanAnalysis) -> Result<(), String> {
        if let Some(plan_arr) = plan_data.as_array() {
            if let Some(top_node) = plan_arr.get(0).and_then(|v| v.get("Plan")) {
                self.analyze_pg_node(top_node, analysis)?;
            }
        }
        Ok(())
    }

    fn analyze_pg_node(&self, node: &serde_json::Value, analysis: &mut ExplainPlanAnalysis) -> Result<(), String> {
        if let Some(node_type) = node.get("Node Type").and_then(|v| v.as_str()) {
            match node_type {
                "Seq Scan" => analysis.has_full_scan = true,
                "Index Scan" | "Index Only Scan" | "Bitmap Index Scan" => analysis.uses_indexes = true,
                "Sort" => analysis.has_filesort = true,
                _ => {}
            }
        }
        if let Some(plans) = node.get("Plans").and_then(|v| v.as_array()) {
            for sub_plan in plans {
                self.analyze_pg_node(sub_plan, analysis)?;
            }
        }
        Ok(())
    }

    fn analyze_traditional_plan(&self, _plan_data: &serde_json::Value, _analysis: &mut ExplainPlanAnalysis) -> Result<(), String> {
        // Placeholder for legacy support
        Ok(())
    }

    async fn update_plan_analysis(&self, plan_id: Uuid, analysis: &ExplainPlanAnalysis) -> Result<(), String> {
        self.explain_repo.update_optimization_flags(
            plan_id,
            analysis.uses_indexes,
            analysis.has_full_scan,
            analysis.has_filesort,
            analysis.has_temp_table,
        ).await
    }

    pub async fn get_plan_recommendations(&self, plan_id: Uuid) -> Result<Vec<String>, String> {
        let analysis = self.get_plan_analysis(plan_id).await?;
        let mut recommendations = Vec::new();
        if analysis.has_full_scan { recommendations.push("Consider adding indexes to avoid full table scans".to_string()); }
        if analysis.has_filesort { recommendations.push("Filesort detected - add indexes on ORDER BY columns".to_string()); }
        if analysis.has_temp_table { recommendations.push("Temporary table created - review GROUP BY performance".to_string()); }
        Ok(recommendations)
    }

    pub async fn compare_explain_plans(&self, plan_id_1: Uuid, plan_id_2: Uuid) -> Result<PlanComparison, String> {
        let plan_1 = self.explain_repo.find_by_id(plan_id_1).await?.ok_or("Plan 1 not found")?;
        let plan_2 = self.explain_repo.find_by_id(plan_id_2).await?.ok_or("Plan 2 not found")?;
        Ok(PlanComparison {
            plan_1, plan_2,
            comparison: serde_json::json!({ "same": true }),
            recommendations: vec![]
        })
    }

    pub async fn update_optimization_flags(&self, plan_id: Uuid, flags: Vec<String>) -> Result<(), String> {
        let mut idx = false; let mut fs = false; let mut fsort = false; let mut tmp = false;
        for f in flags {
            match f.as_str() {
                "uses_indexes" => idx = true,
                "has_full_scan" => fs = true,
                "has_filesort" => fsort = true,
                "has_temp_table" => tmp = true,
                _ => {}
            }
        }
        self.explain_repo.update_optimization_flags(plan_id, idx, fs, fsort, tmp).await
    }
}

#[derive(Debug)]
pub struct PlanComparison {
    pub plan_1: ExplainPlan,
    pub plan_2: ExplainPlan,
    pub comparison: serde_json::Value,
    pub recommendations: Vec<String>,
}
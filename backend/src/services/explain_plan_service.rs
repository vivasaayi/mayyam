use crate::models::explain_plan::ExplainPlan;
use crate::repositories::explain_plan_repository::ExplainPlanRepository;
use crate::repositories::query_fingerprint_repository::QueryFingerprintRepository;
use crate::repositories::aurora_cluster_repository::AuroraClusterRepository;
use uuid::Uuid;
use chrono::NaiveDateTime;
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

    pub async fn capture_explain_plan(
        &self,
        cluster_id: Uuid,
        fingerprint_id: Uuid,
        sql: &str,
        plan_format: &str,
        plan_data: serde_json::Value,
    ) -> Result<ExplainPlan, String> {
        // Verify cluster exists
        let _cluster = self.cluster_repo.find_by_id(cluster_id).await?
            .ok_or_else(|| "Aurora cluster not found".to_string())?;

        // Verify fingerprint exists
        let _fingerprint = self.fingerprint_repo.find_by_id(fingerprint_id).await?
            .ok_or_else(|| "Query fingerprint not found".to_string())?;

        let explain_plan = ExplainPlan {
            id: Uuid::new_v4(),
            cluster_id,
            fingerprint_id,
            sql_text: sql.to_string(),
            plan_format: plan_format.to_string(),
            plan_data: plan_data.to_string(),
            engine_version: "Unknown".to_string(), // TODO: detect from cluster
            captured_at: chrono::Utc::now().naive_utc(),
            is_before_optimization: false,
            has_full_scan: false, // Will be analyzed
            has_filesort: false,
            has_temp_table: false,
            estimated_rows: None,
            actual_rows: None,
            execution_time: None,
            created_at: chrono::Utc::now().naive_utc(),
        };

        let created_plan = self.explain_repo.create(explain_plan).await?;

        // Analyze the plan and update flags
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
        total_cost: Option<f64>,
    ) -> Result<ExplainPlan, String> {
        // Get the SQL from the fingerprint
        let fingerprint = self.fingerprint_repo.find_by_id(fingerprint_id).await?
            .ok_or_else(|| "Query fingerprint not found".to_string())?;

        let mut explain_plan = self.capture_explain_plan(
            cluster_id,
            fingerprint_id,
            &fingerprint.normalized_sql,
            &plan_format,
            plan_data,
        ).await?;

        // Update execution time and cost if provided
        if let Some(exec_time) = execution_time_ms {
            explain_plan.execution_time = Some(exec_time);
        }

        // Note: total_cost would need to be added to the ExplainPlan model if we want to store it

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

    fn analyze_explain_plan(&self, plan: &ExplainPlan) -> Result<ExplainPlanAnalysis, String> {
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
        };

        match plan.plan_format.as_str() {
            "JSON" => {
                let plan_json: serde_json::Value = serde_json::from_str(&plan.plan_data)
                    .map_err(|e| format!("Failed to parse JSON plan: {}", e))?;
                self.analyze_json_plan(&plan_json, &mut analysis)?
            },
            "TRADITIONAL" => self.analyze_traditional_plan(&serde_json::Value::String(plan.plan_data.clone()), &mut analysis)?,
            _ => {} // Unknown format, skip analysis
        }

        Ok(analysis)
    }

    fn analyze_json_plan(&self, plan_data: &serde_json::Value, analysis: &mut ExplainPlanAnalysis) -> Result<(), String> {
        if let Some(query_block) = plan_data.get("query_block") {
            self.analyze_query_block(query_block, analysis)?;
        }
        Ok(())
    }

    fn analyze_query_block(&self, query_block: &serde_json::Value, analysis: &mut ExplainPlanAnalysis) -> Result<(), String> {
        // Check for table access type
        if let Some(table) = query_block.get("table") {
            if let Some(access_type) = table.get("access_type") {
                if let Some(access_str) = access_type.as_str() {
                    match access_str {
                        "ALL" => analysis.has_full_scan = true,
                        "index" | "range" | "ref" | "eq_ref" | "const" | "system" => {
                            analysis.uses_indexes = true;
                        }
                        _ => {}
                    }
                }
            }

            // Check for key usage
            if let Some(key) = table.get("key") {
                if key.is_string() && !key.as_str().unwrap().is_empty() {
                    analysis.uses_indexes = true;
                }
            }
        }

        // Check for ordering operations
        if let Some(ordering_operation) = query_block.get("ordering_operation") {
            if let Some(op) = ordering_operation.get("using_filesort") {
                if op.as_bool().unwrap_or(false) {
                    analysis.has_filesort = true;
                }
            }
        }

        // Check for temporary tables
        if let Some(grouping_operation) = query_block.get("grouping_operation") {
            if let Some(using_tmp_table) = grouping_operation.get("using_temporary_table") {
                if using_tmp_table.as_bool().unwrap_or(false) {
                    analysis.has_temp_table = true;
                }
            }
        }

        // Extract cost and row estimates
        if let Some(cost_info) = query_block.get("cost_info") {
            if let Some(query_cost) = cost_info.get("query_cost") {
                if let Some(cost_str) = query_cost.as_str() {
                    analysis.cost = cost_str.parse().ok();
                }
            }
        }

        if let Some(table) = query_block.get("table") {
            if let Some(rows_examined_per_scan) = table.get("rows_examined_per_scan") {
                analysis.estimated_rows = rows_examined_per_scan.as_i64();
            }
        }

        // Recursively analyze nested query blocks
        if let Some(nested_loop) = query_block.get("nested_loop") {
            if let Some(nested_blocks) = nested_loop.as_array() {
                for block in nested_blocks {
                    self.analyze_query_block(block, analysis)?;
                }
            }
        }

        Ok(())
    }

    fn analyze_traditional_plan(&self, plan_data: &serde_json::Value, analysis: &mut ExplainPlanAnalysis) -> Result<(), String> {
        // Traditional EXPLAIN format is typically stored as text
        if let Some(plan_text) = plan_data.as_str() {
            let lines: Vec<&str> = plan_text.lines().collect();

            for line in lines {
                let line_lower = line.to_lowercase();

                // Check for full table scan
                if line_lower.contains("all") && !line_lower.contains("using index") {
                    analysis.has_full_scan = true;
                }

                // Check for index usage
                if line_lower.contains("using index") || line_lower.contains("using where") {
                    analysis.uses_indexes = true;
                }

                // Check for filesort
                if line_lower.contains("using filesort") {
                    analysis.has_filesort = true;
                }

                // Check for temporary table
                if line_lower.contains("using temporary") {
                    analysis.has_temp_table = true;
                }
            }
        }

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
        let plan = self.explain_repo.find_by_id(plan_id).await?
            .ok_or_else(|| "Explain plan not found".to_string())?;

        let analysis = self.analyze_explain_plan(&plan)?;
        let mut recommendations = Vec::new();

        if analysis.has_full_scan {
            recommendations.push("Consider adding indexes to avoid full table scans".to_string());
        }

        if analysis.has_filesort {
            recommendations.push("Filesort detected - consider adding indexes on ORDER BY columns".to_string());
        }

        if analysis.has_temp_table {
            recommendations.push("Temporary table created - review GROUP BY or subquery performance".to_string());
        }

        if !analysis.uses_indexes && !analysis.has_full_scan {
            recommendations.push("Query is using index lookups - good performance".to_string());
        }

        Ok(recommendations)
    }

    pub async fn compare_plan_performance(&self, plan_ids: Vec<Uuid>) -> Result<HashMap<String, serde_json::Value>, String> {
        // Implementation for comparing multiple plans
        let mut results = HashMap::new();
        
        for plan_id in plan_ids {
            let analysis = self.get_plan_analysis(plan_id).await?;
            results.insert(plan_id.to_string(), serde_json::to_value(analysis).unwrap());
        }
        
        Ok(results)
    }

    pub async fn compare_explain_plans(&self, plan_id_1: Uuid, plan_id_2: Uuid) -> Result<PlanComparison, String> {
        let plan_1 = self.explain_repo.find_by_id(plan_id_1).await?
            .ok_or_else(|| "Plan 1 not found".to_string())?;
        
        let plan_2 = self.explain_repo.find_by_id(plan_id_2).await?
            .ok_or_else(|| "Plan 2 not found".to_string())?;

        // Simple comparison logic
        let mut comparison = serde_json::Map::new();
        comparison.insert("same_fingerprint".to_string(), serde_json::Value::Bool(plan_1.fingerprint_id == plan_2.fingerprint_id));
        comparison.insert("plan_1_better".to_string(), serde_json::Value::Bool(
            plan_1.execution_time.unwrap_or(0.0) < plan_2.execution_time.unwrap_or(0.0)
        ));

        let recommendations = self.get_plan_recommendations(plan_id_1).await.unwrap_or_default();

        Ok(PlanComparison {
            plan_1,
            plan_2,
            comparison: serde_json::Value::Object(comparison),
            recommendations,
        })
    }

    pub async fn update_optimization_flags(&self, plan_id: Uuid, flags: Vec<String>) -> Result<(), String> {
        let mut uses_indexes = false;
        let mut has_full_scan = false;
        let mut has_filesort = false;
        let mut has_temp_table = false;

        for flag in flags {
            match flag.as_str() {
                "uses_indexes" => uses_indexes = true,
                "has_full_scan" => has_full_scan = true,
                "has_filesort" => has_filesort = true,
                "has_temp_table" => has_temp_table = true,
                _ => {} // Ignore unknown flags
            }
        }

        self.explain_repo.update_optimization_flags(
            plan_id,
            uses_indexes,
            has_full_scan,
            has_filesort,
            has_temp_table,
        ).await
    }
}

#[derive(Debug)]
pub struct PlanComparison {
    pub plan_1: ExplainPlan,
    pub plan_2: ExplainPlan,
    pub comparison: serde_json::Value,
    pub recommendations: Vec<String>,
}
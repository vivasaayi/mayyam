use crate::models::ai_analysis::AIAnalysis;
use crate::repositories::ai_analysis_repository::AIAnalysisRepository;
use crate::repositories::query_fingerprint_repository::QueryFingerprintRepository;
use crate::repositories::slow_query_repository::SlowQueryRepository;
use crate::repositories::explain_plan_repository::ExplainPlanRepository;
use uuid::Uuid;
use serde_json;
use std::collections::HashMap;

#[derive(Clone)]
pub struct AIAnalysisService {
    ai_repo: AIAnalysisRepository,
    fingerprint_repo: QueryFingerprintRepository,
    slow_query_repo: SlowQueryRepository,
    explain_repo: ExplainPlanRepository,
}

#[derive(Debug, Clone)]
pub struct AnalysisRequest {
    pub fingerprint_id: Uuid,
    pub analysis_type: String,
    pub context_data: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone)]
pub struct AnalysisResult {
    pub analysis_id: Uuid,
    pub recommendations: Vec<String>,
    pub root_causes: Vec<String>,
    pub confidence_score: f64,
    pub suggestions: Vec<String>,
}

impl AIAnalysisService {
    pub fn new(
        ai_repo: AIAnalysisRepository,
        fingerprint_repo: QueryFingerprintRepository,
        slow_query_repo: SlowQueryRepository,
        explain_repo: ExplainPlanRepository,
    ) -> Self {
        Self {
            ai_repo,
            fingerprint_repo,
            slow_query_repo,
            explain_repo,
        }
    }

    pub async fn generate_analysis(&self, request: AnalysisRequest) -> Result<AnalysisResult, String> {
        // Get fingerprint data
        let fingerprint = self.fingerprint_repo.find_by_id(request.fingerprint_id).await?
            .ok_or_else(|| "Query fingerprint not found".to_string())?;

        // Get recent slow query events for this fingerprint
        let recent_events = self.slow_query_repo.find_top_by_total_time(
            Some(fingerprint.cluster_id),
            24, // Last 24 hours
            10, // Top 10
        ).await?;

        // Get latest explain plan
        let latest_plan = self.explain_repo.find_latest_by_fingerprint(request.fingerprint_id).await?;

        // Generate analysis based on type
        let analysis_result = match request.analysis_type.as_str() {
            "performance" => self.analyze_performance(&fingerprint, &recent_events, latest_plan.as_ref()).await?,
            "indexing" => self.analyze_indexing(&fingerprint, &recent_events, latest_plan.as_ref()).await?,
            "query_structure" => self.analyze_query_structure(&fingerprint, &recent_events).await?,
            "workload_pattern" => self.analyze_workload_pattern(&fingerprint, &recent_events).await?,
            _ => return Err(format!("Unknown analysis type: {}", request.analysis_type)),
        };

        // Store the analysis
        let ai_analysis = AIAnalysis {
            id: Uuid::new_v4(),
            cluster_id: fingerprint.cluster_id,
            fingerprint_id: request.fingerprint_id,
            analysis_type: request.analysis_type,
            recommendations: serde_json::to_value(&analysis_result.recommendations)
                .map_err(|e| format!("Failed to serialize recommendations: {}", e))?,
            root_causes: serde_json::to_value(&analysis_result.root_causes)
                .map_err(|e| format!("Failed to serialize root causes: {}", e))?,
            confidence_score: analysis_result.confidence_score,
            suggestions: serde_json::to_value(&analysis_result.suggestions)
                .map_err(|e| format!("Failed to serialize suggestions: {}", e))?,
            context_data: serde_json::to_value(request.context_data)
                .map_err(|e| format!("Failed to serialize context data: {}", e))?,
            created_at: chrono::Utc::now().naive_utc(),
        };

        self.ai_repo.create(ai_analysis).await?;

        Ok(analysis_result)
    }

    async fn analyze_performance(
        &self,
        fingerprint: &crate::models::query_fingerprint::QueryFingerprint,
        events: &[crate::models::slow_query_event::SlowQueryEvent],
        plan: Option<&crate::models::explain_plan::ExplainPlan>,
    ) -> Result<AnalysisResult, String> {
        let mut recommendations = Vec::new();
        let mut root_causes = Vec::new();
        let mut suggestions = Vec::new();
        let mut confidence_score = 0.7; // Base confidence

        // Analyze execution time patterns
        if fingerprint.execution_count > 10 {
            if fingerprint.avg_execution_time > 1000.0 { // Over 1 second average
                root_causes.push("High average execution time detected".to_string());
                recommendations.push("Consider query optimization or index improvements".to_string());
                confidence_score += 0.1;
            }

            if fingerprint.max_execution_time > fingerprint.avg_execution_time * 5.0 {
                root_causes.push("Significant execution time variance".to_string());
                suggestions.push("Investigate parameter sniffing or plan instability".to_string());
            }
        }

        // Analyze explain plan if available
        if let Some(plan) = plan {
            if plan.has_full_table_scan {
                root_causes.push("Full table scan detected in execution plan".to_string());
                recommendations.push("Add appropriate indexes to avoid table scans".to_string());
                confidence_score += 0.15;
            }

            if plan.has_filesort {
                root_causes.push("Filesort operation in execution plan".to_string());
                recommendations.push("Consider indexes on ORDER BY columns".to_string());
                confidence_score += 0.1;
            }

            if plan.has_temporary_table {
                root_causes.push("Temporary table usage detected".to_string());
                suggestions.push("Review GROUP BY or subquery optimization".to_string());
                confidence_score += 0.1;
            }
        }

        // Analyze recent events
        let recent_high_time_events: Vec<_> = events.iter()
            .filter(|e| e.query_time > 5000.0) // Over 5 seconds
            .collect();

        if !recent_high_time_events.is_empty() {
            root_causes.push(format!("{} recent slow executions detected", recent_high_time_events.len()));
            suggestions.push("Monitor query execution patterns for anomalies".to_string());
        }

        // Table access patterns
        if let Ok(tables) = serde_json::from_value::<Vec<String>>(fingerprint.tables_accessed.clone()) {
            if tables.len() > 5 {
                suggestions.push("Query accesses many tables - consider denormalization or query splitting".to_string());
            }
        }

        Ok(AnalysisResult {
            analysis_id: Uuid::new_v4(),
            recommendations,
            root_causes,
            confidence_score: confidence_score.min(1.0),
            suggestions,
        })
    }

    async fn analyze_indexing(
        &self,
        fingerprint: &crate::models::query_fingerprint::QueryFingerprint,
        events: &[crate::models::slow_query_event::SlowQueryEvent],
        plan: Option<&crate::models::explain_plan::ExplainPlan>,
    ) -> Result<AnalysisResult, String> {
        let mut recommendations = Vec::new();
        let mut root_causes = Vec::new();
        let mut suggestions = Vec::new();
        let mut confidence_score = 0.8; // Higher confidence for indexing analysis

        // Analyze explain plan for indexing issues
        if let Some(plan) = plan {
            if plan.has_full_table_scan {
                root_causes.push("Query performing full table scan".to_string());
                recommendations.push("Create indexes on frequently queried columns".to_string());

                // Suggest specific indexes based on WHERE clauses
                if let Ok(columns) = serde_json::from_value::<Vec<String>>(fingerprint.columns_accessed.clone()) {
                    for column in &columns {
                        recommendations.push(format!("Consider index on column: {}", column));
                    }
                }
            }

            if !plan.uses_indexes && !plan.has_full_table_scan {
                suggestions.push("Query is using index lookups effectively".to_string());
            }
        }

        // Analyze WHERE clause patterns from SQL text
        if let Ok(tables) = serde_json::from_value::<Vec<String>>(fingerprint.tables_accessed.clone()) {
            if let Ok(columns) = serde_json::from_value::<Vec<String>>(fingerprint.columns_accessed.clone()) {
                for table in &tables {
                    let table_columns: Vec<_> = columns.iter()
                        .filter(|col| fingerprint.normalized_query.contains(&format!("{}.{}", table, col)))
                        .collect();

                    if !table_columns.is_empty() {
                        recommendations.push(format!(
                            "Consider composite index on {}({}) for table {}",
                            table,
                            table_columns.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", "),
                            table
                        ));
                    }
                }
            }
        }

        // Check for LIKE queries that might benefit from indexes
        if fingerprint.normalized_query.contains("LIKE") {
            suggestions.push("LIKE queries may not use indexes efficiently - consider full-text search".to_string());
        }

        Ok(AnalysisResult {
            analysis_id: Uuid::new_v4(),
            recommendations,
            root_causes,
            confidence_score,
            suggestions,
        })
    }

    async fn analyze_query_structure(
        &self,
        fingerprint: &crate::models::query_fingerprint::QueryFingerprint,
        _events: &[crate::models::slow_query_event::SlowQueryEvent],
    ) -> Result<AnalysisResult, String> {
        let mut recommendations = Vec::new();
        let mut root_causes = Vec::new();
        let mut suggestions = Vec::new();
        let mut confidence_score = 0.75;

        let sql = &fingerprint.normalized_query;

        // Check for SELECT *
        if sql.contains("SELECT *") {
            root_causes.push("SELECT * query detected".to_string());
            recommendations.push("Specify only required columns instead of SELECT *".to_string());
            confidence_score += 0.1;
        }

        // Check for implicit conversions
        if sql.contains("WHERE") && (sql.contains("= ?") || sql.contains("= ?")) {
            suggestions.push("Check for implicit data type conversions in WHERE clauses".to_string());
        }

        // Check for subqueries
        if sql.contains("SELECT") && sql.matches("SELECT").count() > 1 {
            suggestions.push("Subqueries detected - consider JOINs for better performance".to_string());
        }

        // Check for DISTINCT
        if sql.contains("DISTINCT") {
            suggestions.push("DISTINCT operations can be expensive - ensure they're necessary".to_string());
        }

        // Check for UNION vs UNION ALL
        if sql.contains("UNION") && !sql.contains("UNION ALL") {
            suggestions.push("Consider UNION ALL instead of UNION if duplicates are acceptable".to_string());
        }

        Ok(AnalysisResult {
            analysis_id: Uuid::new_v4(),
            recommendations,
            root_causes,
            confidence_score,
            suggestions,
        })
    }

    async fn analyze_workload_pattern(
        &self,
        fingerprint: &crate::models::query_fingerprint::QueryFingerprint,
        events: &[crate::models::slow_query_event::SlowQueryEvent],
    ) -> Result<AnalysisResult, String> {
        let mut recommendations = Vec::new();
        let mut root_causes = Vec::new();
        let mut suggestions = Vec::new();
        let mut confidence_score = 0.6;

        // Analyze execution frequency
        if fingerprint.execution_count > 1000 {
            root_causes.push("High frequency query detected".to_string());
            recommendations.push("Consider caching results or optimizing frequently executed query".to_string());
            confidence_score += 0.2;
        }

        // Analyze time-based patterns
        let recent_events: Vec<_> = events.iter()
            .filter(|e| e.event_timestamp > chrono::Utc::now().naive_utc() - chrono::Duration::hours(1))
            .collect();

        if recent_events.len() > events.len() / 4 {
            suggestions.push("Query execution spiking in recent hours".to_string());
        }

        // Analyze lock time patterns
        let high_lock_events: Vec<_> = events.iter()
            .filter(|e| e.lock_time > 1000.0) // Over 1 second waiting for locks
            .collect();

        if !high_lock_events.is_empty() {
            root_causes.push("High lock wait times detected".to_string());
            recommendations.push("Investigate lock contention and transaction isolation levels".to_string());
            confidence_score += 0.15;
        }

        Ok(AnalysisResult {
            analysis_id: Uuid::new_v4(),
            recommendations,
            root_causes,
            confidence_score,
            suggestions,
        })
    }

    pub async fn get_analysis_history(&self, fingerprint_id: Uuid, analysis_type: Option<String>) -> Result<Vec<AIAnalysis>, String> {
        if let Some(analysis_type) = analysis_type {
            self.ai_repo.find_by_analysis_type(fingerprint_id, &analysis_type).await
        } else {
            self.ai_repo.find_by_fingerprint(fingerprint_id).await
        }
    }

    pub async fn get_latest_analysis(&self, fingerprint_id: Uuid, analysis_type: &str) -> Result<Option<AIAnalysis>, String> {
        let analyses = self.ai_repo.find_by_analysis_type(fingerprint_id, analysis_type).await?;
        Ok(analyses.into_iter().next())
    }
}
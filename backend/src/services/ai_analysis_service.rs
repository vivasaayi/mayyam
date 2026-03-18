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


use crate::models::ai_analysis::AIAnalysis;
use crate::repositories::ai_analysis_repository::AIAnalysisRepository;
use crate::repositories::query_fingerprint_repository::QueryFingerprintRepository;
use crate::repositories::slow_query_repository::SlowQueryRepository;
use crate::repositories::explain_plan_repository::ExplainPlanRepository;
use crate::services::llm::manager::UnifiedLlmManager;
use crate::services::llm::interface::UnifiedLlmRequest;
use uuid::Uuid;
use serde_json;
use serde::Serialize;
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Clone)]
pub struct AIAnalysisService {
    ai_repo: AIAnalysisRepository,
    fingerprint_repo: QueryFingerprintRepository,
    slow_query_repo: SlowQueryRepository,
    explain_repo: ExplainPlanRepository,
    llm_manager: Arc<UnifiedLlmManager>,
}

#[derive(Debug, Clone)]
pub struct AnalysisRequest {
    pub fingerprint_id: Uuid,
    pub analysis_type: String,
    pub context_data: HashMap<String, serde_json::Value>,
    pub llm_provider: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AnalysisResult {
    pub analysis_id: Uuid,
    pub recommendations: Vec<String>,
    pub root_causes: Vec<String>,
    pub confidence_score: f64,
    pub suggestions: Vec<String>,
    pub rewritten_sql: Option<String>,
}

impl AIAnalysisService {
    pub fn new(
        ai_repo: AIAnalysisRepository,
        fingerprint_repo: QueryFingerprintRepository,
        slow_query_repo: SlowQueryRepository,
        explain_repo: ExplainPlanRepository,
        llm_manager: Arc<UnifiedLlmManager>,
    ) -> Self {
        Self {
            ai_repo,
            fingerprint_repo,
            slow_query_repo,
            explain_repo,
            llm_manager,
        }
    }

    pub async fn generate_analysis(&self, request: AnalysisRequest) -> Result<AnalysisResult, String> {
        let fingerprint = self.fingerprint_repo.find_by_id(request.fingerprint_id).await?
            .ok_or_else(|| "Query fingerprint not found".to_string())?;

        let recent_events = self.slow_query_repo.find_top_by_total_time(
            None,
            24,
            10,
        ).await?;

        let latest_plan = self.explain_repo.find_latest_by_fingerprint(request.fingerprint_id).await?;

        let mut analysis_result = match request.analysis_type.as_str() {
            "performance" => self.analyze_performance(&fingerprint, &recent_events, latest_plan.as_ref()).await?,
            "indexing" => self.analyze_indexing(&fingerprint, &recent_events, latest_plan.as_ref()).await?,
            "query_structure" => self.analyze_query_structure(&fingerprint, &recent_events).await?,
            "workload_pattern" => self.analyze_workload_pattern(&fingerprint, &recent_events).await?,
            _ => return Err(format!("Unknown analysis type: {}", request.analysis_type)),
        };

        // If an LLM provider is specified, use it for deeper insights
        if let Some(provider) = request.llm_provider.as_ref() {
            match self.generate_llm_insights(provider, &fingerprint, latest_plan.as_ref()).await {
                Ok((llm_recs, rewrite)) => {
                    analysis_result.recommendations.extend(llm_recs);
                    analysis_result.rewritten_sql = rewrite;
                    analysis_result.confidence_score = (analysis_result.confidence_score + 0.2).min(1.0);
                }
                Err(e) => tracing::warn!("LLM analysis failed: {}", e),
            }
        }

        // Store the analysis
        use crate::models::ai_analysis::ActiveModel;
        let active_model = ActiveModel {
            id: sea_orm::Set(Uuid::new_v4()),
            cluster_id: sea_orm::Set(Uuid::nil()),
            fingerprint_id: sea_orm::Set(Some(request.fingerprint_id)),
            slow_query_id: sea_orm::Set(None),
            ai_provider: sea_orm::Set(request.llm_provider.clone().unwrap_or_else(|| "heuristic".to_string())),
            ai_model: sea_orm::Set("heuristic".to_string()),
            analysis_type: sea_orm::Set(request.analysis_type.clone()),
            input_data: sea_orm::Set(serde_json::to_value(&analysis_result).unwrap_or(serde_json::Value::Null)),
            analysis_result: sea_orm::Set(serde_json::to_string(&analysis_result.recommendations).unwrap_or_default()),
            confidence_score: sea_orm::Set(Some(analysis_result.confidence_score)),
            suggested_indexes: sea_orm::Set(serde_json::to_value(&analysis_result.suggestions).unwrap_or(serde_json::Value::Null)),
            suggested_rewrites: sea_orm::Set(serde_json::to_value(&analysis_result.rewritten_sql).unwrap_or(serde_json::Value::Null)),
            root_causes: sea_orm::Set(serde_json::to_value(&analysis_result.root_causes).unwrap_or(serde_json::Value::Null)),
            created_at: sea_orm::Set(chrono::Utc::now().naive_utc()),
        };

        self.ai_repo.create_from_active_model(active_model).await?;

        Ok(analysis_result)
    }

    async fn generate_llm_insights(
        &self,
        provider: &str,
        fingerprint: &crate::models::query_fingerprint::QueryFingerprint,
        plan: Option<&crate::models::explain_plan::ExplainPlan>,
    ) -> Result<(Vec<String>, Option<String>), String> {
        let prompt = format!(
            "Analyze this slow SQL query and its execution plan:\n\nSQL: {}\n\nPlan: {}\n\nProvide specific optimization recommendations and a rewritten version of the query if possible.",
            fingerprint.normalized_sql,
            plan.map(|p| p.plan_data.as_str()).unwrap_or("No plan available")
        );

        let response = self.llm_manager.quick_generate(provider, &prompt).await
            .map_err(|e| format!("LLM Error: {}", e))?;

        // Simple parsing of LLM response (this would be more sophisticated in production)
        let recommendations = vec![response.clone()];
        let rewrite = if response.contains("SELECT") { Some(response) } else { None };

        Ok((recommendations, rewrite))
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
        let mut confidence_score: f64 = 0.7;

        if fingerprint.avg_query_time > 1.0 {
            root_causes.push("High average execution time detected".to_string());
            recommendations.push("Consider query optimization or index improvements".to_string());
        }

        if let Some(plan) = plan {
            if plan.has_full_scan {
                root_causes.push("Full table scan detected in execution plan".to_string());
                recommendations.push("Add appropriate indexes to avoid table scans".to_string());
                confidence_score += 0.15;
            }
        }

        Ok(AnalysisResult {
            analysis_id: Uuid::new_v4(),
            recommendations,
            root_causes,
            confidence_score: confidence_score.min(1.0),
            suggestions,
            rewritten_sql: None,
        })
    }

    async fn analyze_indexing(
        &self,
        fingerprint: &crate::models::query_fingerprint::QueryFingerprint,
        _events: &[crate::models::slow_query_event::SlowQueryEvent],
        plan: Option<&crate::models::explain_plan::ExplainPlan>,
    ) -> Result<AnalysisResult, String> {
        let mut recommendations = Vec::new();
        if let Some(plan) = plan {
            if plan.has_full_scan {
                recommendations.push("Create indexes on frequently queried columns".to_string());
            }
        }
        Ok(AnalysisResult {
            analysis_id: Uuid::new_v4(),
            recommendations,
            root_causes: vec![],
            confidence_score: 0.8,
            suggestions: vec![],
            rewritten_sql: None,
        })
    }

    async fn analyze_query_structure(
        &self,
        fingerprint: &crate::models::query_fingerprint::QueryFingerprint,
        _events: &[crate::models::slow_query_event::SlowQueryEvent],
    ) -> Result<AnalysisResult, String> {
        let mut recommendations = Vec::new();
        if fingerprint.normalized_sql.contains("SELECT *") {
            recommendations.push("Specify only required columns instead of SELECT *".to_string());
        }
        Ok(AnalysisResult {
            analysis_id: Uuid::new_v4(),
            recommendations,
            root_causes: vec![],
            confidence_score: 0.75,
            suggestions: vec![],
            rewritten_sql: None,
        })
    }

    async fn analyze_workload_pattern(
        &self,
        fingerprint: &crate::models::query_fingerprint::QueryFingerprint,
        events: &[crate::models::slow_query_event::SlowQueryEvent],
    ) -> Result<AnalysisResult, String> {
        let mut root_causes = Vec::new();
        if fingerprint.execution_count > 1000 {
            root_causes.push("High frequency query detected".to_string());
        }
        Ok(AnalysisResult {
            analysis_id: Uuid::new_v4(),
            recommendations: vec![],
            root_causes,
            confidence_score: 0.6,
            suggestions: vec![],
            rewritten_sql: None,
        })
    }

    pub async fn get_analysis_history(&self, fingerprint_id: Uuid, analysis_type: Option<String>) -> Result<Vec<AIAnalysis>, String> {
        if let Some(at) = analysis_type {
            self.ai_repo.find_by_analysis_type(fingerprint_id, &at).await
        } else {
            self.ai_repo.find_by_fingerprint(fingerprint_id, None).await
        }
    }
}
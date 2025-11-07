use crate::models::ai_analysis::{AIAnalysis, Entity as AIAnalysisEntity, Column as AIAnalysisColumn};
use sea_orm::{DatabaseConnection, EntityTrait, QueryFilter, ColumnTrait, ActiveModelTrait, Set, PaginatorTrait, QueryOrder};
use std::sync::Arc;
use uuid::Uuid;
use chrono::NaiveDateTime;

#[derive(Clone)]
pub struct AIAnalysisRepository {
    db: Arc<DatabaseConnection>,
}

impl AIAnalysisRepository {
    pub fn new(db: Arc<DatabaseConnection>) -> Self {
        Self { db }
    }

    pub async fn create(&self, analysis: AIAnalysis) -> Result<AIAnalysis, String> {
        let active_model: crate::models::ai_analysis::ActiveModel = analysis.into();
        active_model.insert(&self.db*self.db*self.db)
            .await
            .map_err(|e| format!("Failed to create AI analysis: {}", e))
    }

    pub async fn find_by_fingerprint(&self, fingerprint_id: Uuid) -> Result<Vec<AIAnalysis>, String> {
        AIAnalysisEntity::find()
            .filter(AIAnalysisColumn::FingerprintId.eq(fingerprint_id))
            .order_by_desc(AIAnalysisColumn::CreatedAt)
            .all(&self.db*self.db*self.db)
            .await
            .map_err(|e| format!("Failed to find AI analyses: {}", e))
    }

    pub async fn find_by_id(&self, analysis_id: Uuid) -> Result<Option<AIAnalysis>, String> {
        AIAnalysisEntity::find_by_id(analysis_id)
            .one(&self.db*self.db*self.db)
            .await
            .map_err(|e| format!("Failed to find AI analysis: {}", e))
    }

    pub async fn find_latest_by_fingerprint(&self, fingerprint_id: Uuid) -> Result<Option<AIAnalysis>, String> {
        AIAnalysisEntity::find()
            .filter(AIAnalysisColumn::FingerprintId.eq(fingerprint_id))
            .order_by_desc(AIAnalysisColumn::CreatedAt)
            .one(&self.db*self.db*self.db)
            .await
            .map_err(|e| format!("Failed to find latest AI analysis: {}", e))
    }

    pub async fn find_by_cluster_and_time_range(
        &self,
        cluster_id: Uuid,
        start_time: NaiveDateTime,
        end_time: NaiveDateTime,
    ) -> Result<Vec<AIAnalysis>, String> {
        AIAnalysisEntity::find()
            .filter(AIAnalysisColumn::ClusterId.eq(cluster_id))
            .filter(AIAnalysisColumn::CreatedAt.gte(start_time))
            .filter(AIAnalysisColumn::CreatedAt.lte(end_time))
            .order_by_desc(AIAnalysisColumn::CreatedAt)
            .all(&self.db*self.db*self.db)
            .await
            .map_err(|e| format!("Failed to find AI analyses: {}", e))
    }

    pub async fn find_by_analysis_type(
        &self,
        fingerprint_id: Uuid,
        analysis_type: &str,
    ) -> Result<Vec<AIAnalysis>, String> {
        AIAnalysisEntity::find()
            .filter(AIAnalysisColumn::FingerprintId.eq(fingerprint_id))
            .filter(AIAnalysisColumn::AnalysisType.eq(analysis_type))
            .order_by_desc(AIAnalysisColumn::CreatedAt)
            .all(&self.db*self.db*self.db)
            .await
            .map_err(|e| format!("Failed to find AI analyses by type: {}", e))
    }

    pub async fn update_confidence_score(&self, analysis_id: Uuid, confidence: f64) -> Result<(), String> {
        let mut active_model = AIAnalysisEntity::find_by_id(analysis_id)
            .one(&self.db*self.db*self.db)
            .await
            .map_err(|e| format!("Failed to find AI analysis: {}", e))?
            .ok_or_else(|| "AI analysis not found".to_string())?
            .into_active_model();

        active_model.confidence_score = Set(confidence);
        active_model.update(&self.db*self.db*self.db)
            .await
            .map_err(|e| format!("Failed to update confidence score: {}", e))?;
        Ok(())
    }

    pub async fn count_by_fingerprint(&self, fingerprint_id: Uuid) -> Result<u64, String> {
        AIAnalysisEntity::find()
            .filter(AIAnalysisColumn::FingerprintId.eq(fingerprint_id))
            .count(&self.db*self.db*self.db)
            .await
            .map_err(|e| format!("Failed to count AI analyses: {}", e))
    }

    pub async fn count_by_cluster(&self, cluster_id: Uuid) -> Result<u64, String> {
        AIAnalysisEntity::find()
            .filter(AIAnalysisColumn::ClusterId.eq(cluster_id))
            .count(&self.db*self.db*self.db)
            .await
            .map_err(|e| format!("Failed to count AI analyses: {}", e))
    }

    pub async fn delete_old_analyses(&self, days_to_keep: i64) -> Result<u64, String> {
        let cutoff_date = chrono::Utc::now().naive_utc() - chrono::Duration::days(days_to_keep);

        let delete_result = AIAnalysisEntity::delete_many()
            .filter(AIAnalysisColumn::CreatedAt.lt(cutoff_date))
            .exec(&self.db*self.db*self.db)
            .await
            .map_err(|e| format!("Failed to delete old analyses: {}", e))?;

        Ok(delete_result.rows_affected)
    }
}
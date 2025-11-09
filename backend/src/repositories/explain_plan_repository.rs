use crate::models::explain_plan::{ExplainPlan, Entity as ExplainPlanEntity, Column as ExplainPlanColumn};
use sea_orm::{DatabaseConnection, EntityTrait, QueryFilter, ColumnTrait, ActiveModelTrait, Set, PaginatorTrait, QueryOrder};
use std::sync::Arc;
use uuid::Uuid;
use chrono::NaiveDateTime;

#[derive(Clone)]
pub struct ExplainPlanRepository {
    db: Arc<DatabaseConnection>,
}

impl ExplainPlanRepository {
    pub fn new(db: Arc<DatabaseConnection>) -> Self {
        Self { db }
    }

    pub async fn create(&self, plan: ExplainPlan) -> Result<ExplainPlan, String> {
        let active_model: crate::models::explain_plan::ActiveModel = plan.into();
        active_model.insert(&*self.db)
            .await
            .map_err(|e| format!("Failed to create explain plan: {}", e))
    }

    pub async fn find_by_fingerprint(&self, fingerprint_id: Uuid) -> Result<Vec<ExplainPlan>, String> {
        ExplainPlanEntity::find()
            .filter(ExplainPlanColumn::FingerprintId.eq(fingerprint_id))
            .order_by_desc(ExplainPlanColumn::CapturedAt)
            .all(&*self.db)
            .await
            .map_err(|e| format!("Failed to find explain plans: {}", e))
    }

    pub async fn find_by_id(&self, plan_id: Uuid) -> Result<Option<ExplainPlan>, String> {
        ExplainPlanEntity::find_by_id(plan_id)
            .one(&*self.db)
            .await
            .map_err(|e| format!("Failed to find explain plan: {}", e))
    }

    pub async fn find_latest_by_fingerprint(&self, fingerprint_id: Uuid) -> Result<Option<ExplainPlan>, String> {
        ExplainPlanEntity::find()
            .filter(ExplainPlanColumn::FingerprintId.eq(fingerprint_id))
            .order_by_desc(ExplainPlanColumn::CapturedAt)
            .one(&*self.db)
            .await
            .map_err(|e| format!("Failed to find latest explain plan: {}", e))
    }

    pub async fn find_by_cluster_and_time_range(
        &self,
        cluster_id: Uuid,
        start_time: NaiveDateTime,
        end_time: NaiveDateTime,
    ) -> Result<Vec<ExplainPlan>, String> {
        ExplainPlanEntity::find()
            .filter(ExplainPlanColumn::ClusterId.eq(cluster_id))
            .filter(ExplainPlanColumn::CapturedAt.gte(start_time))
            .filter(ExplainPlanColumn::CapturedAt.lte(end_time))
            .order_by_desc(ExplainPlanColumn::CapturedAt)
            .all(&*self.db)
            .await
            .map_err(|e| format!("Failed to find explain plans: {}", e))
    }

    pub async fn update_optimization_flags(
        &self,
        plan_id: Uuid,
        uses_indexes: bool,
        has_full_scan: bool,
        has_filesort: bool,
        has_temp_table: bool,
    ) -> Result<(), String> {
        let mut active_model = ExplainPlanEntity::find_by_id(plan_id)
            .one(&*self.db)
            .await
            .map_err(|e| format!("Failed to find explain plan: {}", e))?
            .ok_or_else(|| "Explain plan not found".to_string())?
            .into_active_model();

        active_model.uses_indexes = Set(uses_indexes);
        active_model.has_full_scan = Set(has_full_scan);
        active_model.has_filesort = Set(has_filesort);
        active_model.has_temp_table = Set(has_temp_table);

        active_model.update(&*self.db)
            .await
            .map_err(|e| format!("Failed to update optimization flags: {}", e))?;
        Ok(())
    }

    pub async fn compare_plans(&self, fingerprint_id: Uuid, limit: u64) -> Result<Vec<ExplainPlan>, String> {
        ExplainPlanEntity::find()
            .filter(ExplainPlanColumn::FingerprintId.eq(fingerprint_id))
            .order_by_desc(ExplainPlanColumn::CapturedAt)
            .limit(limit)
            .all(&*self.db)
            .await
            .map_err(|e| format!("Failed to compare plans: {}", e))
    }

    pub async fn count_by_fingerprint(&self, fingerprint_id: Uuid) -> Result<u64, String> {
        ExplainPlanEntity::find()
            .filter(ExplainPlanColumn::FingerprintId.eq(fingerprint_id))
            .count(&*self.db)
            .await
            .map_err(|e| format!("Failed to count explain plans: {}", e))
    }

    pub async fn delete_old_plans(&self, days_to_keep: i64) -> Result<u64, String> {
        let cutoff_date = chrono::Utc::now().naive_utc() - chrono::Duration::days(days_to_keep);

        let delete_result = ExplainPlanEntity::delete_many()
            .filter(ExplainPlanColumn::CapturedAt.lt(cutoff_date))
            .exec(&*self.db)
            .await
            .map_err(|e| format!("Failed to delete old plans: {}", e))?;

        Ok(delete_result.rows_affected)
    }

    pub async fn find_by_cluster(&self, cluster_id: Uuid, limit: u64) -> Result<Vec<ExplainPlan>, String> {
        ExplainPlanEntity::find()
            .filter(ExplainPlanColumn::ClusterId.eq(cluster_id))
            .order_by_desc(ExplainPlanColumn::CapturedAt)
            .limit(limit)
            .all(&*self.db)
            .await
            .map_err(|e| format!("Failed to find explain plans by cluster: {}", e))
    }

    pub async fn find_recent(&self, limit: u64) -> Result<Vec<ExplainPlan>, String> {
        ExplainPlanEntity::find()
            .order_by_desc(ExplainPlanColumn::CapturedAt)
            .limit(limit)
            .all(&*self.db)
            .await
            .map_err(|e| format!("Failed to find recent explain plans: {}", e))
    }

    pub async fn delete(&self, plan_id: Uuid) -> Result<(), String> {
        ExplainPlanEntity::delete_by_id(plan_id)
            .exec(&*self.db)
            .await
            .map_err(|e| format!("Failed to delete explain plan: {}", e))?;
        Ok(())
    }
}
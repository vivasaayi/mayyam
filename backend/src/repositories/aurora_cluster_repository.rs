use crate::models::aurora_cluster::{AuroraCluster, Entity as AuroraClusterEntity, Column as AuroraClusterColumn};
use sea_orm::{DatabaseConnection, EntityTrait, QueryFilter, ColumnTrait, ActiveModelTrait, Set, PaginatorTrait};
use std::sync::Arc;
use uuid::Uuid;

#[derive(Clone)]
pub struct AuroraClusterRepository {
    db: Arc<DatabaseConnection>,
}

impl AuroraClusterRepository {
    pub fn new(db: Arc<DatabaseConnection>) -> Self {
        Self { db }
    }

    pub async fn create(&self, cluster: AuroraCluster) -> Result<AuroraCluster, String> {
        let active_model: crate::models::aurora_cluster::ActiveModel = cluster.into();
        active_model.insert(&self.db*self.db*self.db)
            .await
            .map_err(|e| format!("Failed to create Aurora cluster: {}", e))
    }

    pub async fn find_by_id(&self, id: Uuid) -> Result<Option<AuroraCluster>, String> {
        AuroraClusterEntity::find_by_id(id)
            .one(&self.db*self.db*self.db)
            .await
            .map_err(|e| format!("Failed to find Aurora cluster: {}", e))
    }

    pub async fn find_all_active(&self) -> Result<Vec<AuroraCluster>, String> {
        AuroraClusterEntity::find()
            .filter(AuroraClusterColumn::IsActive.eq(true))
            .all(&self.db*self.db*self.db)
            .await
            .map_err(|e| format!("Failed to find active clusters: {}", e))
    }

    pub async fn update(&self, cluster: AuroraCluster) -> Result<AuroraCluster, String> {
        let active_model: crate::models::aurora_cluster::ActiveModel = cluster.into();
        active_model.update(&self.db*self.db*self.db)
            .await
            .map_err(|e| format!("Failed to update Aurora cluster: {}", e))
    }

    pub async fn delete(&self, id: Uuid) -> Result<(), String> {
        AuroraClusterEntity::delete_by_id(id)
            .exec(&self.db*self.db*self.db)
            .await
            .map_err(|e| format!("Failed to delete Aurora cluster: {}", e))?;
        Ok(())
    }

    pub async fn count(&self) -> Result<u64, String> {
        AuroraClusterEntity::find()
            .count(&self.db*self.db*self.db)
            .await
            .map_err(|e| format!("Failed to count clusters: {}", e))
    }
}
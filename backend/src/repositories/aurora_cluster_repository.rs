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
        active_model.insert(&*self.db)
            .await
            .map_err(|e| format!("Failed to create Aurora cluster: {}", e))
    }

    pub async fn find_by_id(&self, id: Uuid) -> Result<Option<AuroraCluster>, String> {
        AuroraClusterEntity::find_by_id(id)
            .one(&*self.db)
            .await
            .map_err(|e| format!("Failed to find Aurora cluster: {}", e))
    }

    pub async fn find_all_active(&self) -> Result<Vec<AuroraCluster>, String> {
        AuroraClusterEntity::find()
            .filter(AuroraClusterColumn::IsActive.eq(true))
            .all(&*self.db)
            .await
            .map_err(|e| format!("Failed to find active clusters: {}", e))
    }

    pub async fn update(&self, cluster: AuroraCluster) -> Result<AuroraCluster, String> {
        let active_model: crate::models::aurora_cluster::ActiveModel = cluster.into();
        active_model.update(&*self.db)
            .await
            .map_err(|e| format!("Failed to update Aurora cluster: {}", e))
    }

    pub async fn delete(&self, id: Uuid) -> Result<(), String> {
        AuroraClusterEntity::delete_by_id(id)
            .exec(&*self.db)
            .await
            .map_err(|e| format!("Failed to delete Aurora cluster: {}", e))?;
        Ok(())
    }

    pub async fn count(&self) -> Result<u64, String> {
        AuroraClusterEntity::find()
            .count(&*self.db)
            .await
            .map_err(|e| format!("Failed to count clusters: {}", e))
    }
}
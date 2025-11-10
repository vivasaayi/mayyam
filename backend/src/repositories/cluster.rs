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


use chrono::Utc;
use sea_orm::{ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set};
use serde_json::json;
use std::sync::Arc;
use tracing::{error, info};
use uuid::Uuid;

use crate::config::Config;
use crate::errors::AppError;
use crate::models::cluster::{
    self, ActiveModel as ClusterActiveModel, Entity as Cluster, Model as ClusterModel,
};

#[derive(Debug)]
pub struct ClusterRepository {
    db: Arc<DatabaseConnection>,
    config: Config,
}

impl ClusterRepository {
    pub fn new(db: Arc<DatabaseConnection>, config: Config) -> Self {
        Self { db, config }
    }

    pub async fn find_by_id(&self, id: Uuid) -> Result<Option<ClusterModel>, AppError> {
        let cluster = Cluster::find_by_id(id)
            .one(&*self.db)
            .await
            .map_err(AppError::from)?;

        Ok(cluster)
    }

    pub async fn find_by_name(&self, name: &str) -> Result<Option<ClusterModel>, AppError> {
        let cluster = Cluster::find()
            .filter(cluster::Column::Name.eq(name))
            .one(&*self.db)
            .await
            .map_err(AppError::from)?;

        Ok(cluster)
    }

    pub async fn find_all(&self) -> Result<Vec<ClusterModel>, AppError> {
        let clusters = Cluster::find()
            .all(&*self.db)
            .await
            .map_err(AppError::from)?;

        Ok(clusters)
    }

    pub async fn find_by_type(&self, cluster_type: &str) -> Result<Vec<ClusterModel>, AppError> {
        let clusters = Cluster::find()
            .filter(cluster::Column::ClusterType.eq(cluster_type))
            .all(&*self.db)
            .await
            .map_err(AppError::from)?;

        Ok(clusters)
    }

    // Create Kafka cluster
    pub async fn create_kafka_cluster(
        &self,
        request: &cluster::CreateKafkaClusterRequest,
        user_id: &str,
    ) -> Result<ClusterModel, AppError> {
        // Check if cluster name already exists
        if let Some(_) = self.find_by_name(&request.name).await? {
            return Err(AppError::Conflict(format!(
                "Cluster with name '{}' already exists",
                request.name
            )));
        }

        let config = json!({
            "bootstrap_servers": request.bootstrap_servers,
            "sasl_username": request.sasl_username,
            "sasl_password": request.sasl_password,
            "sasl_mechanism": request.sasl_mechanism,
            "security_protocol": request.security_protocol,
        });

        let now = Utc::now();

        let cluster = ClusterActiveModel {
            id: Set(Uuid::new_v4()),
            name: Set(request.name.clone()),
            cluster_type: Set("kafka".to_string()),
            config: Set(config),
            created_by: Set(Uuid::parse_str(user_id)
                .map_err(|e| AppError::Internal(format!("Invalid UUID: {}", e)))?),
            created_at: Set(now),
            updated_at: Set(now),
            last_connected_at: Set(None),
            status: Set(Some("new".to_string())),
        };

        let cluster = cluster.insert(&*self.db).await.map_err(AppError::from)?;

        Ok(cluster)
    }

    // Create Kubernetes cluster
    pub async fn create_kubernetes_cluster(
        &self,
        request: &cluster::CreateKubernetesClusterRequest,
        user_id: &str,
    ) -> Result<ClusterModel, AppError> {
        // Check if cluster name already exists
        if let Some(_) = self.find_by_name(&request.name).await? {
            return Err(AppError::Conflict(format!(
                "Cluster with name '{}' already exists",
                request.name
            )));
        }

        let config = json!({
            "kube_config_path": request.kube_config_path,
            "kube_context": request.kube_context,
            "api_server_url": request.api_server_url,
            "certificate_authority_data": request.certificate_authority_data,
            "client_certificate_data": request.client_certificate_data,
            "client_key_data": request.client_key_data,
            "token": request.token,
        });

        let now = Utc::now();

        let cluster = ClusterActiveModel {
            id: Set(Uuid::new_v4()),
            name: Set(request.name.clone()),
            cluster_type: Set("kubernetes".to_string()),
            config: Set(config),
            created_by: Set(Uuid::parse_str(user_id)
                .map_err(|e| AppError::Internal(format!("Invalid UUID: {}", e)))?),
            created_at: Set(now),
            updated_at: Set(now),
            last_connected_at: Set(None),
            status: Set(Some("new".to_string())),
        };

        let cluster = cluster.insert(&*self.db).await.map_err(AppError::from)?;

        Ok(cluster)
    }

    // Create cloud connection (AWS or Azure)
    pub async fn create_cloud_connection(
        &self,
        request: &cluster::CreateCloudConnectionRequest,
        user_id: &str,
    ) -> Result<ClusterModel, AppError> {
        // Check if cluster name already exists
        if let Some(_) = self.find_by_name(&request.name).await? {
            return Err(AppError::Conflict(format!(
                "Connection with name '{}' already exists",
                request.name
            )));
        }

        // Validate cloud type
        if request.cloud_type != "aws" && request.cloud_type != "azure" {
            return Err(AppError::BadRequest(format!(
                "Invalid cloud type: {}. Must be 'aws' or 'azure'",
                request.cloud_type
            )));
        }

        let now = Utc::now();

        let cluster = ClusterActiveModel {
            id: Set(Uuid::new_v4()),
            name: Set(request.name.clone()),
            cluster_type: Set(request.cloud_type.clone()),
            config: Set(request.config.clone()),
            created_by: Set(Uuid::parse_str(user_id)
                .map_err(|e| AppError::Internal(format!("Invalid UUID: {}", e)))?),
            created_at: Set(now),
            updated_at: Set(now),
            last_connected_at: Set(None),
            status: Set(Some("new".to_string())),
        };

        let cluster = cluster.insert(&*self.db).await.map_err(AppError::from)?;

        Ok(cluster)
    }

    pub async fn update(
        &self,
        id: Uuid,
        name: &str,
        config: serde_json::Value,
    ) -> Result<ClusterModel, AppError> {
        let cluster = match self.find_by_id(id).await? {
            Some(cluster) => cluster,
            None => {
                return Err(AppError::NotFound(format!(
                    "Cluster not found with ID: {}",
                    id
                )))
            }
        };

        // Check name uniqueness if it changed
        if cluster.name != name {
            if let Some(_) = self.find_by_name(name).await? {
                return Err(AppError::Conflict(format!(
                    "Cluster with name '{}' already exists",
                    name
                )));
            }
        }

        let mut cluster_active: ClusterActiveModel = cluster.into();
        cluster_active.name = Set(name.to_string());
        cluster_active.config = Set(config);
        cluster_active.updated_at = Set(Utc::now());

        let updated_cluster = cluster_active
            .update(&*self.db)
            .await
            .map_err(AppError::from)?;

        Ok(updated_cluster)
    }

    pub async fn delete(&self, id: Uuid) -> Result<(), AppError> {
        let cluster = match self.find_by_id(id).await? {
            Some(cluster) => cluster,
            None => {
                return Err(AppError::NotFound(format!(
                    "Cluster not found with ID: {}",
                    id
                )))
            }
        };

        let cluster_active: ClusterActiveModel = cluster.into();
        cluster_active
            .delete(&*self.db)
            .await
            .map_err(AppError::from)?;

        Ok(())
    }

    pub async fn update_cluster_status(
        &self,
        id: Uuid,
        status: &str,
    ) -> Result<ClusterModel, AppError> {
        let cluster = match self.find_by_id(id).await? {
            Some(cluster) => cluster,
            None => {
                return Err(AppError::NotFound(format!(
                    "Cluster not found with ID: {}",
                    id
                )))
            }
        };

        let mut cluster_active: ClusterActiveModel = cluster.into();
        cluster_active.status = Set(Some(status.to_string()));

        if status == "connected" {
            cluster_active.last_connected_at = Set(Some(Utc::now()));
        }

        cluster_active.updated_at = Set(Utc::now());

        let updated_cluster = cluster_active
            .update(&*self.db)
            .await
            .map_err(AppError::from)?;

        Ok(updated_cluster)
    }
}

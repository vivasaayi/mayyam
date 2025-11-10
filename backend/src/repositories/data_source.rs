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
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, Set,
};
use serde_json::Value;
use std::sync::Arc;
use uuid::Uuid;

use crate::errors::AppError;
use crate::models::data_source::{
    self, ActiveModel as DataSourceActiveModel, DataSourceStatus, DataSourceType,
    Entity as DataSource, Model as DataSourceModel, ResourceType, SourceType,
};

pub struct DataSourceRepository {
    db: Arc<DatabaseConnection>,
}

impl DataSourceRepository {
    pub fn new(db: Arc<DatabaseConnection>) -> Self {
        Self { db }
    }

    pub async fn create(
        &self,
        name: String,
        description: Option<String>,
        data_source_type: DataSourceType,
        resource_type: ResourceType,
        source_type: SourceType,
        connection_config: Value,
        metric_config: Option<Value>,
        thresholds: Option<Value>,
    ) -> Result<DataSourceModel, AppError> {
        let data_source = DataSourceActiveModel {
            id: Set(Uuid::new_v4()),
            name: Set(name),
            description: Set(description),
            data_source_type: Set(serde_json::to_string(&data_source_type)
                .unwrap_or_default()
                .replace('"', "")),
            resource_type: Set(serde_json::to_string(&resource_type)
                .unwrap_or_default()
                .replace('"', "")),
            source_type: Set(serde_json::to_string(&source_type)
                .unwrap_or_default()
                .replace('"', "")),
            connection_config: Set(connection_config),
            metric_config: Set(metric_config),
            thresholds: Set(thresholds),
            enabled: Set(true),
            status: Set(serde_json::to_string(&DataSourceStatus::Active)
                .unwrap_or_default()
                .replace('"', "")),
            created_at: Set(Utc::now()),
            updated_at: Set(Utc::now()),
        };

        let result = data_source
            .insert(&*self.db)
            .await
            .map_err(AppError::from)?;
        Ok(result)
    }

    pub async fn find_by_id(&self, id: Uuid) -> Result<Option<DataSourceModel>, AppError> {
        let data_source = DataSource::find_by_id(id)
            .one(&*self.db)
            .await
            .map_err(AppError::from)?;

        Ok(data_source)
    }

    pub async fn find_by_name(&self, name: &str) -> Result<Option<DataSourceModel>, AppError> {
        let data_source = DataSource::find()
            .filter(data_source::Column::Name.eq(name))
            .one(&*self.db)
            .await
            .map_err(AppError::from)?;

        Ok(data_source)
    }

    pub async fn find_all(&self) -> Result<Vec<DataSourceModel>, AppError> {
        let data_sources = DataSource::find()
            .order_by_asc(data_source::Column::Name)
            .all(&*self.db)
            .await
            .map_err(AppError::from)?;

        Ok(data_sources)
    }

    pub async fn find_by_resource_type(
        &self,
        resource_type: ResourceType,
    ) -> Result<Vec<DataSourceModel>, AppError> {
        let resource_type_str = serde_json::to_string(&resource_type)
            .unwrap_or_default()
            .replace('"', "");
        let status_str = serde_json::to_string(&DataSourceStatus::Active)
            .unwrap_or_default()
            .replace('"', "");

        let data_sources = DataSource::find()
            .filter(data_source::Column::ResourceType.eq(resource_type_str))
            .filter(data_source::Column::Status.eq(status_str))
            .order_by_asc(data_source::Column::Name)
            .all(&*self.db)
            .await
            .map_err(AppError::from)?;

        Ok(data_sources)
    }

    pub async fn find_by_source_type(
        &self,
        source_type: SourceType,
    ) -> Result<Vec<DataSourceModel>, AppError> {
        let source_type_str = serde_json::to_string(&source_type)
            .unwrap_or_default()
            .replace('"', "");
        let status_str = serde_json::to_string(&DataSourceStatus::Active)
            .unwrap_or_default()
            .replace('"', "");

        let data_sources = DataSource::find()
            .filter(data_source::Column::SourceType.eq(source_type_str))
            .filter(data_source::Column::Status.eq(status_str))
            .order_by_asc(data_source::Column::Name)
            .all(&*self.db)
            .await
            .map_err(AppError::from)?;

        Ok(data_sources)
    }

    pub async fn update(
        &self,
        id: Uuid,
        name: Option<String>,
        description: Option<Option<String>>,
        connection_config: Option<Value>,
        metric_config: Option<Option<Value>>,
        thresholds: Option<Option<Value>>,
        status: Option<DataSourceStatus>,
    ) -> Result<DataSourceModel, AppError> {
        let data_source = DataSource::find_by_id(id)
            .one(&*self.db)
            .await
            .map_err(AppError::from)?
            .ok_or_else(|| AppError::NotFound("Data source not found".to_string()))?;

        let mut active_model: DataSourceActiveModel = data_source.into();

        if let Some(name) = name {
            active_model.name = Set(name);
        }
        if let Some(description) = description {
            active_model.description = Set(description);
        }
        if let Some(connection_config) = connection_config {
            active_model.connection_config = Set(connection_config);
        }
        if let Some(metric_config) = metric_config {
            active_model.metric_config = Set(metric_config);
        }
        if let Some(thresholds) = thresholds {
            active_model.thresholds = Set(thresholds);
        }
        if let Some(status) = status {
            active_model.status = Set(serde_json::to_string(&status)
                .unwrap_or_default()
                .replace('"', ""));
        }
        active_model.updated_at = Set(Utc::now());

        let result = active_model
            .update(&*self.db)
            .await
            .map_err(AppError::from)?;
        Ok(result)
    }

    pub async fn delete(&self, id: Uuid) -> Result<(), AppError> {
        DataSource::delete_by_id(id)
            .exec(&*self.db)
            .await
            .map_err(AppError::from)?;

        Ok(())
    }

    pub async fn test_connection(&self, id: Uuid) -> Result<bool, AppError> {
        let data_source = self
            .find_by_id(id)
            .await?
            .ok_or_else(|| AppError::NotFound("Data source not found".to_string()))?;

        // TODO: Implement actual connection testing based on source_type
        // For now, return true as a placeholder
        match data_source.source_type.as_str() {
            "CloudWatch" => {
                // Test AWS CloudWatch connection
                Ok(true)
            }
            "Dynatrace" => {
                // Test Dynatrace API connection
                Ok(true)
            }
            "Splunk" => {
                // Test Splunk connection
                Ok(true)
            }
            "Prometheus" => {
                // Test Prometheus connection
                Ok(true)
            }
            "Custom" => {
                // Test custom source connection
                Ok(true)
            }
            _ => {
                // Unknown source type
                Ok(false)
            }
        }
    }
}

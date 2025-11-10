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


use actix_web::{web, HttpResponse, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;
use uuid::Uuid;

use crate::errors::AppError;
use crate::models::data_source::{
    DataSourceResponseDto, DataSourceStatus, DataSourceType, ResourceType, SourceType,
};
use crate::repositories::data_source::DataSourceRepository;
use crate::services::data_collection::DataCollectionService;

#[derive(Debug, Deserialize)]
pub struct CreateDataSourceRequest {
    pub name: String,
    pub description: Option<String>,
    pub data_source_type: DataSourceType,
    pub resource_type: ResourceType,
    pub source_type: SourceType,
    pub connection_config: Value,
    pub metric_config: Option<Value>,
    pub thresholds: Option<Value>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateDataSourceRequest {
    pub name: Option<String>,
    pub description: Option<Option<String>>,
    pub connection_config: Option<Value>,
    pub metric_config: Option<Option<Value>>,
    pub thresholds: Option<Option<Value>>,
    pub status: Option<DataSourceStatus>,
}

#[derive(Debug, Deserialize)]
pub struct DataSourceQueryParams {
    pub resource_type: Option<ResourceType>,
    pub source_type: Option<SourceType>,
    pub status: Option<DataSourceStatus>,
}

#[derive(Debug, Serialize)]
pub struct DataSourceListResponse {
    pub data_sources: Vec<DataSourceResponseDto>,
    pub total: usize,
}

#[derive(Debug, Serialize)]
pub struct TestConnectionResponse {
    pub success: bool,
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct AvailableMetricsResponse {
    pub metrics: Vec<String>,
    pub resource_type: ResourceType,
}

pub struct DataSourceController {
    data_source_repo: Arc<DataSourceRepository>,
    data_collection_service: Arc<DataCollectionService>,
}

impl DataSourceController {
    pub fn new(
        data_source_repo: Arc<DataSourceRepository>,
        data_collection_service: Arc<DataCollectionService>,
    ) -> Self {
        Self {
            data_source_repo,
            data_collection_service,
        }
    }

    pub async fn create_data_source(
        &self,
        request: CreateDataSourceRequest,
    ) -> Result<DataSourceResponseDto, AppError> {
        let data_source = self
            .data_source_repo
            .create(
                request.name,
                request.description,
                request.data_source_type,
                request.resource_type,
                request.source_type,
                request.connection_config,
                request.metric_config,
                request.thresholds,
            )
            .await?;

        Ok(DataSourceResponseDto::from(data_source))
    }

    pub async fn get_data_source(
        &self,
        id: Uuid,
    ) -> Result<Option<DataSourceResponseDto>, AppError> {
        let data_source = self.data_source_repo.find_by_id(id).await?;

        Ok(data_source.map(|model| DataSourceResponseDto::from(&model)))
    }

    pub async fn list_data_sources(
        &self,
        query: DataSourceQueryParams,
    ) -> Result<DataSourceListResponse, AppError> {
        let data_sources = if let Some(resource_type) = &query.resource_type {
            self.data_source_repo
                .find_by_resource_type(resource_type.clone())
                .await?
        } else if let Some(source_type) = &query.source_type {
            self.data_source_repo
                .find_by_source_type(source_type.clone())
                .await?
        } else {
            self.data_source_repo.find_all().await?
        };

        let response_dtos: Vec<DataSourceResponseDto> = data_sources
            .into_iter()
            .map(DataSourceResponseDto::from)
            .collect();

        Ok(DataSourceListResponse {
            total: response_dtos.len(),
            data_sources: response_dtos,
        })
    }

    pub async fn update_data_source(
        &self,
        id: Uuid,
        request: UpdateDataSourceRequest,
    ) -> Result<DataSourceResponseDto, AppError> {
        let data_source = self
            .data_source_repo
            .update(
                id,
                request.name,
                request.description,
                request.connection_config,
                request.metric_config,
                request.thresholds,
                request.status,
            )
            .await?;

        Ok(DataSourceResponseDto::from(data_source))
    }

    pub async fn delete_data_source(&self, id: Uuid) -> Result<(), AppError> {
        self.data_source_repo.delete(id).await?;
        Ok(())
    }

    pub async fn test_data_source_connection(
        &self,
        id: Uuid,
    ) -> Result<TestConnectionResponse, AppError> {
        let success = self
            .data_collection_service
            .test_data_source_connection(id)
            .await?;

        let message = if success {
            "Connection successful".to_string()
        } else {
            "Connection failed".to_string()
        };

        Ok(TestConnectionResponse { success, message })
    }

    pub async fn search_data_sources(
        &self,
        query: DataSourceQueryParams,
    ) -> Result<DataSourceListResponse, AppError> {
        // For now, search is the same as list with filters
        self.list_data_sources(query).await
    }

    pub async fn get_available_metrics(
        &self,
        id: Uuid,
        resource_type: ResourceType,
    ) -> Result<AvailableMetricsResponse, AppError> {
        let metrics = self
            .data_collection_service
            .get_available_metrics(id, resource_type.clone())
            .await?;

        Ok(AvailableMetricsResponse {
            metrics,
            resource_type,
        })
    }
}

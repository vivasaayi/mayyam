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

use crate::errors::AppError;
use crate::models::cluster::KubernetesClusterConfig;
use crate::services::kubernetes::client::ClientFactory;
use k8s_openapi::api::storage::v1::StorageClass;
use kube::{api::ListParams, Api};
use serde_json::Value;

pub struct StorageClassesService;

impl StorageClassesService {
    pub async fn list_storage_classes(
        &self,
        cluster_config: &KubernetesClusterConfig,
    ) -> Result<Vec<Value>, AppError> {
        let client = ClientFactory::get_client(cluster_config).await?;
        let storage_classes: Api<StorageClass> = Api::all(client);

        let sc_list = storage_classes
            .list(&ListParams::default())
            .await
            .map_err(|e| AppError::ExternalService(format!("Failed to list StorageClasses: {}", e)))?;

        let mut formatted_sc = Vec::new();
        for sc in sc_list {
            if let Ok(value) = serde_json::to_value(&sc) {
                formatted_sc.push(value);
            }
        }

        Ok(formatted_sc)
    }

    pub async fn get_storage_class_details(
        &self,
        cluster_config: &KubernetesClusterConfig,
        storage_class_name: &str,
    ) -> Result<Value, AppError> {
        let client = ClientFactory::get_client(cluster_config).await?;
        let storage_classes: Api<StorageClass> = Api::all(client);

        let sc = storage_classes.get(storage_class_name).await.map_err(|e| {
            AppError::ExternalService(format!("Failed to get StorageClass details: {}", e))
        })?;

        serde_json::to_value(&sc)
            .map_err(|e| AppError::Internal(format!("Failed to serialize StorageClass details: {}", e)))
    }
}

use std::sync::Arc;
use serde::{Deserialize, Serialize};
use serde_json::json;
use aws_sdk_dynamodb::Client as DynamoDbClient;
use crate::errors::AppError;
use crate::models::aws_resource::{AwsResourceDto, Model as AwsResourceModel};
use crate::models::aws_auth::AccountAuthInfo;
use super::{AwsService, CloudWatchMetricsRequest, CloudWatchMetricsResult};
use super::client_factory::AwsClientFactory;

// DynamoDB-specific types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DynamoDbTableInfo {
    pub table_name: String,
    pub status: String,
    pub provisioned_throughput: DynamoDbProvisionedThroughput,
    pub key_schema: Vec<DynamoDbKeySchema>,
    pub attribute_definitions: Vec<DynamoDbAttributeDefinition>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DynamoDbProvisionedThroughput {
    pub read_capacity_units: i64,
    pub write_capacity_units: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DynamoDbKeySchema {
    pub attribute_name: String,
    pub key_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DynamoDbAttributeDefinition {
    pub attribute_name: String,
    pub attribute_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DynamoDBGetItemRequest {
    pub table_name: String,
    pub key: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DynamoDBPutItemRequest {
    pub table_name: String,
    pub item: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DynamoDBQueryRequest {
    pub table_name: String,
    pub key_condition_expression: String,
    pub expression_attribute_values: serde_json::Value,
    pub expression_attribute_names: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DynamoDBDeleteItemRequest {
    pub table_name: String,
    pub key: serde_json::Value,
    pub condition_expression: Option<String>,
    pub expression_attribute_names: Option<serde_json::Value>,
    pub expression_attribute_values: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DynamoDBBatchGetItemRequest {
    pub request_items: serde_json::Value,  // Map of table name to keys to get
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DynamoDBBatchWriteItemRequest {
    pub request_items: serde_json::Value,  // Map of table name to write requests
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DynamoDBUpdateItemRequest {
    pub table_name: String,
    pub key: serde_json::Value,
    pub update_expression: String,
    pub condition_expression: Option<String>,
    pub expression_attribute_names: Option<serde_json::Value>,
    pub expression_attribute_values: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DynamoDBScanRequest {
    pub table_name: String,
    pub filter_expression: Option<String>,
    pub expression_attribute_names: Option<serde_json::Value>,
    pub expression_attribute_values: Option<serde_json::Value>,
    pub limit: Option<i32>,
    pub exclusive_start_key: Option<serde_json::Value>,
}

// Control plane implementation for DynamoDB
pub struct DynamoDbControlPlane {
    aws_service: Arc<AwsService>,
}

impl DynamoDbControlPlane {
    pub fn new(aws_service: Arc<AwsService>) -> Self {
        Self { aws_service }
    }

    pub async fn sync_tables(&self, account_id: &str, profile: Option<&str>, region: &str) -> Result<Vec<AwsResourceModel>, AppError> {
        self.sync_tables_with_auth(account_id, profile, region, None).await
    }
    
    pub async fn sync_tables_with_auth(&self, account_id: &str, profile: Option<&str>, region: &str, account_auth: Option<&AccountAuthInfo>) -> Result<Vec<AwsResourceModel>, AppError> {
        let client = self.aws_service.create_dynamodb_client_with_auth(profile, region, account_auth).await?;
        self.sync_tables_with_client(account_id, profile, region, client).await
    }
    
    async fn sync_tables_with_client(&self, account_id: &str, profile: Option<&str>, region: &str, client: DynamoDbClient) -> Result<Vec<AwsResourceModel>, AppError> {
        let mut tables = Vec::new();
        let table = AwsResourceDto {
            id: None,
            account_id: account_id.to_string(),
            profile: profile.map(|p| p.to_string()),
            region: region.to_string(),
            resource_type: "DynamoDbTable".to_string(),
            resource_id: "sample-users".to_string(),
            arn: format!("arn:aws:dynamodb:{}:{}:table/sample-users", region, account_id),
            name: Some("Sample Users Table".to_string()),
            tags: json!({"Name": "Users Table", "Environment": "Development"}),
            resource_data: json!({
                "table_name": "sample-users",
                "status": "ACTIVE",
                "creation_date": "2023-03-15T08:45:00Z",
                "provisioned_throughput": {
                    "read_capacity_units": 5,
                    "write_capacity_units": 5
                },
                "key_schema": [
                    {
                        "attribute_name": "user_id",
                        "key_type": "HASH"
                    }
                ],
                "attribute_definitions": [
                    {
                        "attribute_name": "user_id",
                        "attribute_type": "S"
                    }
                ],
                "item_count": 42,
                "table_size_bytes": 12345
            }),
        };
        tables.push(table);

        Ok(tables.into_iter().map(|t| t.into()).collect())
    }

    pub async fn list_tables(&self, profile: Option<&str>, region: &str, exclusive_start_table_name: Option<String>, limit: Option<i32>) -> Result<Vec<String>, AppError> {
        let client = self.aws_service.create_dynamodb_client(profile, region).await?;
        
        // Mock implementation
        Ok(vec![
            "users".to_string(),
            "orders".to_string(),
            "products".to_string()
        ])
    }

    pub async fn describe_table(&self, profile: Option<&str>, region: &str, table_name: &str) -> Result<DynamoDbTableInfo, AppError> {
        let client = self.aws_service.create_dynamodb_client(profile, region).await?;
        
        // Mock implementation
        Ok(DynamoDbTableInfo {
            table_name: table_name.to_string(),
            status: "ACTIVE".to_string(),
            provisioned_throughput: DynamoDbProvisionedThroughput {
                read_capacity_units: 5,
                write_capacity_units: 5,
            },
            key_schema: vec![
                DynamoDbKeySchema {
                    attribute_name: "id".to_string(),
                    key_type: "HASH".to_string(),
                }
            ],
            attribute_definitions: vec![
                DynamoDbAttributeDefinition {
                    attribute_name: "id".to_string(),
                    attribute_type: "S".to_string(),
                }
            ],
        })
    }

    pub async fn create_table(&self, profile: Option<&str>, region: &str, table_name: &str, 
        key_schema: Vec<DynamoDbKeySchema>,
        attribute_definitions: Vec<DynamoDbAttributeDefinition>,
        provisioned_throughput: DynamoDbProvisionedThroughput) -> Result<DynamoDbTableInfo, AppError> {
        
        let client = self.aws_service.create_dynamodb_client(profile, region).await?;
        
        // Mock implementation
        Ok(DynamoDbTableInfo {
            table_name: table_name.to_string(),
            status: "CREATING".to_string(),
            provisioned_throughput,
            key_schema,
            attribute_definitions,
        })
    }

    pub async fn delete_table(&self, profile: Option<&str>, region: &str, table_name: &str) -> Result<(), AppError> {
        let client = self.aws_service.create_dynamodb_client(profile, region).await?;
        
        // Mock implementation
        Ok(())
    }

    pub async fn update_table(&self, profile: Option<&str>, region: &str, 
        table_name: &str,
        provisioned_throughput: Option<DynamoDbProvisionedThroughput>) -> Result<DynamoDbTableInfo, AppError> {
        
        let client = self.aws_service.create_dynamodb_client(profile, region).await?;
        
        // Mock implementation
        Ok(DynamoDbTableInfo {
            table_name: table_name.to_string(),
            status: "UPDATING".to_string(),
            provisioned_throughput: provisioned_throughput.unwrap_or(DynamoDbProvisionedThroughput {
                read_capacity_units: 5,
                write_capacity_units: 5,
            }),
            key_schema: vec![
                DynamoDbKeySchema {
                    attribute_name: "id".to_string(),
                    key_type: "HASH".to_string(),
                }
            ],
            attribute_definitions: vec![
                DynamoDbAttributeDefinition {
                    attribute_name: "id".to_string(),
                    attribute_type: "S".to_string(),
                }
            ],
        })
    }
}

// Data plane implementation for DynamoDB
pub struct DynamoDBDataPlane {
    aws_service: Arc<AwsService>,
}

impl DynamoDBDataPlane {
    pub fn new(aws_service: Arc<AwsService>) -> Self {
        Self { aws_service }
    }

    pub async fn get_item(&self, profile: Option<&str>, region: &str, request: &DynamoDBGetItemRequest) -> Result<serde_json::Value, AppError> {
        let client = self.aws_service.create_dynamodb_client(profile, region).await?;
        
        // In a real implementation, this would call get_item
        let response = json!({
            "Item": {
                "id": {"S": "sample-id"},
                "name": {"S": "Sample Item"},
                "count": {"N": "42"}
            }
        });
        
        Ok(response)
    }

    pub async fn put_item(&self, profile: Option<&str>, region: &str, request: &DynamoDBPutItemRequest) -> Result<serde_json::Value, AppError> {
        let client = self.aws_service.create_dynamodb_client(profile, region).await?;
        
        // In a real implementation, this would call put_item
        let response = json!({
            "ConsumedCapacity": {
                "TableName": request.table_name,
                "CapacityUnits": 1.0
            }
        });
        
        Ok(response)
    }

    pub async fn query(&self, profile: Option<&str>, region: &str, request: &DynamoDBQueryRequest) -> Result<serde_json::Value, AppError> {
        let client = self.aws_service.create_dynamodb_client(profile, region).await?;
        
        // In a real implementation, this would call query
        let response = json!({
            "Items": [
                {
                    "id": {"S": "sample-id-1"},
                    "name": {"S": "Sample Item 1"},
                    "count": {"N": "42"}
                },
                {
                    "id": {"S": "sample-id-2"},
                    "name": {"S": "Sample Item 2"},
                    "count": {"N": "43"}
                }
            ],
            "Count": 2,
            "ScannedCount": 2,
            "ConsumedCapacity": {
                "TableName": request.table_name,
                "CapacityUnits": 0.5
            }
        });
        
        Ok(response)
    }
}

use serde::{Deserialize, Serialize};
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
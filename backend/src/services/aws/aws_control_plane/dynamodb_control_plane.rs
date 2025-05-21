use std::sync::Arc;
use aws_sdk_dynamodb::Client as DynamoDbClient;

use serde_json::json;
use crate::errors::AppError;
use crate::models::aws_auth::AccountAuthInfo;
use crate::models::aws_resource::{AwsResourceDto, Model as AwsResourceModel};
use crate::services::aws::aws_types::dynamodb::{DynamoDbAttributeDefinition, DynamoDbKeySchema, DynamoDbProvisionedThroughput, DynamoDbTableInfo};
use crate::services::aws::client_factory::AwsClientFactory;
use crate::services::AwsService;

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
        // Get the list of tables from AWS
        let list_response = client.list_tables()
            .send()
            .await
            .map_err(|e| AppError::ExternalService(format!("Failed to list DynamoDB tables: {}", e)))?;
            
        let table_names = list_response.table_names().unwrap_or_default();
        
        let mut tables = Vec::new();
        
        for table_name in table_names {
            // Get detailed info for each table
            let describe_resp = client.describe_table()
                .table_name(table_name)
                .send()
                .await
                .map_err(|e| AppError::ExternalService(format!("Failed to describe table {}: {}", table_name, e)))?;
                
            if let Some(table_details) = describe_resp.table() {
                // Get tags for the table
                let arn = format!("arn:aws:dynamodb:{}:{}:table/{}", region, account_id, table_name);
                
                let tags_response = client.list_tags_of_resource()
                    .resource_arn(&arn)
                    .send()
                    .await
                    .map_err(|e| AppError::ExternalService(format!("Failed to get tags for table {}: {}", table_name, e)))?;
                
                let mut tags_map = serde_json::Map::new();
                let mut name = None;
                
                if let Some(tags) = tags_response.tags() {
                    for tag in tags {
                        if let (Some(key), Some(value)) = (tag.key(), tag.value()) {
                            if key == "Name" {
                                name = Some(value.to_string());
                            }
                            tags_map.insert(key.to_string(), json!(value));
                        }
                    }
                }
                
                // If no name tag was found, use the table name as name
                if name.is_none() {
                    name = Some(table_name.to_string());
                }
                
                // Build resource data
                let mut resource_data = serde_json::Map::new();
                
                resource_data.insert("table_name".to_string(), json!(table_name));
                
                if let Some(status) = table_details.table_status().map(|s| s.as_str()) {
                    resource_data.insert("status".to_string(), json!(status));
                }
                
                if let Some(creation_date) = table_details.creation_date_time() {
                    resource_data.insert("creation_date".to_string(), 
                        if let Ok(formatted_date) = creation_date.fmt(aws_smithy_types::date_time::Format::DateTime) {
                            json!(formatted_date)
                        } else {
                            json!(creation_date.as_secs_f64().to_string())
                        });
                }
                
                // Handle provisioned throughput
                if let Some(throughput) = table_details.provisioned_throughput() {
                    let mut throughput_data = serde_json::Map::new();
                    
                    if let Some(read) = throughput.read_capacity_units() {
                        throughput_data.insert("read_capacity_units".to_string(), json!(read));
                    }
                    
                    if let Some(write) = throughput.write_capacity_units() {
                        throughput_data.insert("write_capacity_units".to_string(), json!(write));
                    }
                    
                    resource_data.insert("provisioned_throughput".to_string(), json!(throughput_data));
                }
                
                // Handle key schema
                if let Some(key_schema) = table_details.key_schema() {
                    let mut schema_data = Vec::new();
                    
                    for key in key_schema {
                        let mut key_data = serde_json::Map::new();
                        
                        if let Some(name) = key.attribute_name() {
                            key_data.insert("attribute_name".to_string(), json!(name));
                        }
                        
                        if let Some(type_str) = key.key_type().map(|t| t.as_str()) {
                            key_data.insert("key_type".to_string(), json!(type_str));
                        }
                        
                        schema_data.push(serde_json::Value::Object(key_data));
                    }
                    
                    resource_data.insert("key_schema".to_string(), json!(schema_data));
                }
                
                // Handle attribute definitions
                if let Some(attr_defs) = table_details.attribute_definitions() {
                    let mut attr_data = Vec::new();
                    
                    for attr in attr_defs {
                        let mut attr_map = serde_json::Map::new();
                        
                        if let Some(name) = attr.attribute_name() {
                            attr_map.insert("attribute_name".to_string(), json!(name));
                        }
                        
                        if let Some(type_str) = attr.attribute_type().map(|t| t.as_str()) {
                            attr_map.insert("attribute_type".to_string(), json!(type_str));
                        }
                        
                        attr_data.push(serde_json::Value::Object(attr_map));
                    }
                    
                    resource_data.insert("attribute_definitions".to_string(), json!(attr_data));
                }
                
                // Add item count and size if available
                if let Some(count) = table_details.item_count() {
                    resource_data.insert("item_count".to_string(), json!(count));
                }
                
                if let Some(size) = table_details.table_size_bytes() {
                    resource_data.insert("table_size_bytes".to_string(), json!(size));
                }
                
                // Create resource DTO
                let table = AwsResourceDto {
                    id: None,
                    account_id: account_id.to_string(),
                    profile: profile.map(|p| p.to_string()),
                    region: region.to_string(),
                    resource_type: "DynamoDbTable".to_string(),
                    resource_id: table_name.to_string(),
                    arn,
                    name,
                    tags: serde_json::Value::Object(tags_map),
                    resource_data: serde_json::Value::Object(resource_data),
                };
                
                tables.push(table);
            }
        }

        Ok(tables.into_iter().map(|t| t.into()).collect())
    }

    pub async fn list_tables(&self, profile: Option<&str>, region: &str, exclusive_start_table_name: Option<String>, limit: Option<i32>) -> Result<Vec<String>, AppError> {
        let client = self.aws_service.create_dynamodb_client(profile, region).await?;
        
        // Build the request with optional parameters
        let mut request = client.list_tables();
        
        if let Some(start_table) = exclusive_start_table_name {
            request = request.exclusive_start_table_name(start_table);
        }
        
        if let Some(limit_value) = limit {
            request = request.limit(limit_value);
        }
        
        // Send the request to AWS
        let response = request
            .send()
            .await
            .map_err(|e| AppError::ExternalService(format!("Failed to list DynamoDB tables: {}", e)))?;
            
        // Extract table names from response
        let table_names = response.table_names().unwrap_or_default().to_vec();
        
        Ok(table_names)
    }

    pub async fn describe_table(&self, profile: Option<&str>, region: &str, table_name: &str) -> Result<DynamoDbTableInfo, AppError> {
        let client = self.aws_service.create_dynamodb_client(profile, region).await?;
        
        // Send describe table request to AWS
        let response = client.describe_table()
            .table_name(table_name)
            .send()
            .await
            .map_err(|e| AppError::ExternalService(format!("Failed to describe DynamoDB table {}: {}", table_name, e)))?;
            
        // Extract table details from response
        let table_details = response.table()
            .ok_or_else(|| AppError::ExternalService(format!("No table details returned for {}", table_name)))?;
            
        // Convert AWS SDK types to our custom types
        
        // Get provisioned throughput
        let provisioned_throughput = if let Some(throughput) = table_details.provisioned_throughput() {
            DynamoDbProvisionedThroughput {
                read_capacity_units: throughput.read_capacity_units().unwrap_or(0),
                write_capacity_units: throughput.write_capacity_units().unwrap_or(0),
            }
        } else {
            DynamoDbProvisionedThroughput {
                read_capacity_units: 0,
                write_capacity_units: 0,
            }
        };
        
        // Get key schema
        let key_schema: Vec<DynamoDbKeySchema> = if let Some(schema) = table_details.key_schema() {
            schema.iter()
                .map(|key| DynamoDbKeySchema {
                    attribute_name: key.attribute_name().unwrap_or_default().to_string(),
                    key_type: key.key_type().map(|t| t.as_str()).unwrap_or_default().to_string(),
                })
                .collect()
        } else {
            Vec::new()
        };
        
        // Get attribute definitions
        let attribute_definitions: Vec<DynamoDbAttributeDefinition> = if let Some(attrs) = table_details.attribute_definitions() {
            attrs.iter()
                .map(|attr| DynamoDbAttributeDefinition {
                    attribute_name: attr.attribute_name().unwrap_or_default().to_string(),
                    attribute_type: attr.attribute_type().map(|t| t.as_str()).unwrap_or_default().to_string(),
                })
                .collect()
        } else {
            Vec::new()
        };
        
        // Create and return table info
        Ok(DynamoDbTableInfo {
            table_name: table_name.to_string(),
            status: table_details.table_status().map(|s| s.as_str()).unwrap_or("UNKNOWN").to_string(),
            provisioned_throughput,
            key_schema,
            attribute_definitions,
        })
    }

    pub async fn create_table(&self, profile: Option<&str>, region: &str, table_name: &str, 
        key_schema: Vec<DynamoDbKeySchema>,
        attribute_definitions: Vec<DynamoDbAttributeDefinition>,
        provisioned_throughput: DynamoDbProvisionedThroughput) -> Result<DynamoDbTableInfo, AppError> {
        
        let client = self.aws_service.create_dynamodb_client(profile, region).await?;
        
        // Convert our custom types to AWS SDK types
        
        // Convert key schema
        let aws_key_schema: Vec<aws_sdk_dynamodb::types::KeySchemaElement> = key_schema.iter()
            .map(|key| aws_sdk_dynamodb::types::KeySchemaElement::builder()
                .attribute_name(&key.attribute_name)
                .key_type(key.key_type.as_str().into())
                .build())
            .collect();
            
        // Convert attribute definitions
        let aws_attr_defs: Vec<aws_sdk_dynamodb::types::AttributeDefinition> = attribute_definitions.iter()
            .map(|attr| aws_sdk_dynamodb::types::AttributeDefinition::builder()
                .attribute_name(&attr.attribute_name)
                .attribute_type(attr.attribute_type.as_str().into())
                .build())
            .collect();
            
        // Convert provisioned throughput
        let aws_throughput = aws_sdk_dynamodb::types::ProvisionedThroughput::builder()
            .read_capacity_units(provisioned_throughput.read_capacity_units)
            .write_capacity_units(provisioned_throughput.write_capacity_units)
            .build();
            
        // Build and send create table request
        let response = client.create_table()
            .table_name(table_name)
            .set_key_schema(Some(aws_key_schema))
            .set_attribute_definitions(Some(aws_attr_defs))
            .provisioned_throughput(aws_throughput)
            .send()
            .await
            .map_err(|e| AppError::ExternalService(format!("Failed to create DynamoDB table {}: {}", table_name, e)))?;
            
        // Extract table details from response
        let table_details = response.table_description()
            .ok_or_else(|| AppError::ExternalService(format!("No table details returned for newly created table {}", table_name)))?;
            
        // Return table info
        Ok(DynamoDbTableInfo {
            table_name: table_name.to_string(),
            status: table_details.table_status().map(|s| s.as_str()).unwrap_or("CREATING").to_string(),
            provisioned_throughput,
            key_schema,
            attribute_definitions,
        })
    }

    pub async fn delete_table(&self, profile: Option<&str>, region: &str, table_name: &str) -> Result<(), AppError> {
        let client = self.aws_service.create_dynamodb_client(profile, region).await?;
        
        // Send delete table request to AWS
        client.delete_table()
            .table_name(table_name)
            .send()
            .await
            .map_err(|e| AppError::ExternalService(format!("Failed to delete DynamoDB table {}: {}", table_name, e)))?;
            
        Ok(())
    }

    pub async fn update_table(&self, profile: Option<&str>, region: &str, 
        table_name: &str,
        provisioned_throughput: Option<DynamoDbProvisionedThroughput>) -> Result<DynamoDbTableInfo, AppError> {
        
        let client = self.aws_service.create_dynamodb_client(profile, region).await?;
        
        // Build and send update table request
        let mut request = client.update_table()
            .table_name(table_name);
            
        // Add provisioned throughput if provided
        if let Some(throughput) = &provisioned_throughput {
            let aws_throughput = aws_sdk_dynamodb::types::ProvisionedThroughput::builder()
                .read_capacity_units(throughput.read_capacity_units)
                .write_capacity_units(throughput.write_capacity_units)
                .build();
                
            request = request.provisioned_throughput(aws_throughput);
        }
        
        // Send the request to AWS
        let response = request
            .send()
            .await
            .map_err(|e| AppError::ExternalService(format!("Failed to update DynamoDB table {}: {}", table_name, e)))?;
            
        // Extract table details from response
        let table_details = response.table_description()
            .ok_or_else(|| AppError::ExternalService(format!("No table details returned for updated table {}", table_name)))?;
            
        // Get current key schema and attribute definitions to return
        // (note that these cannot be modified in an update operation)
        let describe_resp = client.describe_table()
            .table_name(table_name)
            .send()
            .await
            .map_err(|e| AppError::ExternalService(format!("Failed to describe updated table {}: {}", table_name, e)))?;
            
        let table_desc = describe_resp.table()
            .ok_or_else(|| AppError::ExternalService(format!("No details found for table {}", table_name)))?;
            
        // Get current key schema
        let key_schema: Vec<DynamoDbKeySchema> = if let Some(schema) = table_desc.key_schema() {
            schema.iter()
                .map(|key| DynamoDbKeySchema {
                    attribute_name: key.attribute_name().unwrap_or_default().to_string(),
                    key_type: key.key_type().map(|t| t.as_str()).unwrap_or_default().to_string(),
                })
                .collect()
        } else {
            Vec::new()
        };
        
        // Get current attribute definitions
        let attribute_definitions: Vec<DynamoDbAttributeDefinition> = if let Some(attrs) = table_desc.attribute_definitions() {
            attrs.iter()
                .map(|attr| DynamoDbAttributeDefinition {
                    attribute_name: attr.attribute_name().unwrap_or_default().to_string(),
                    attribute_type: attr.attribute_type().map(|t| t.as_str()).unwrap_or_default().to_string(),
                })
                .collect()
        } else {
            Vec::new()
        };
        
        // Get current provisioned throughput
        let current_throughput = if let Some(throughput) = table_desc.provisioned_throughput() {
            DynamoDbProvisionedThroughput {
                read_capacity_units: throughput.read_capacity_units().unwrap_or(0),
                write_capacity_units: throughput.write_capacity_units().unwrap_or(0),
            }
        } else {
            provisioned_throughput.unwrap_or(DynamoDbProvisionedThroughput {
                read_capacity_units: 0,
                write_capacity_units: 0,
            })
        };
        
        // Return updated table info
        Ok(DynamoDbTableInfo {
            table_name: table_name.to_string(),
            status: table_details.table_status().map(|s| s.as_str()).unwrap_or("UPDATING").to_string(),
            provisioned_throughput: current_throughput,
            key_schema,
            attribute_definitions,
        })
    }
}
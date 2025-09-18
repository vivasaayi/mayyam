use std::sync::Arc;
use serde_json::json;
use crate::errors::AppError;
use crate::models::aws_account::AwsAccountDto;
use crate::models::aws_auth::AccountAuthInfo;
use crate::models::aws_resource;
use crate::models::aws_resource::{AwsResourceDto, AwsResourceType};
use crate::services::aws::client_factory::AwsClientFactory;
use crate::services::AwsService;

pub struct LambdaControlPlane {
    aws_service: Arc<AwsService>,
}

impl LambdaControlPlane {
    pub fn new(aws_service: Arc<AwsService>) -> Self {
        Self { aws_service }
    }

    pub async fn sync_functions(&self, account_id: &str, aws_account_dto: &AwsAccountDto) -> Result<Vec<aws_resource::Model>, AppError> {
        let client = self.aws_service.create_lambda_client(aws_account_dto).await?;

        let repo = &self.aws_service.aws_resource_repo;
        
        let mut functions = Vec::new();
        let mut marker = None;
        
        // Paginate through all Lambda functions
        loop {
            // Build list functions request
            let mut request = client.list_functions();
            
            // Add marker for pagination if it exists
            if let Some(marker_val) = &marker {
                request = request.marker(marker_val);
            }
            
            // Send request to AWS
            let response = request
                .send()
                .await
                .map_err(|e| AppError::ExternalService(format!("Failed to list Lambda functions: {}", e)))?;
                
            // Process functions in the response
            for aws_function in response.functions() {
                if let Some(function_name) = aws_function.function_name() {
                    // Extract function ARN
                    let function_arn = aws_function.function_arn().unwrap_or_default();
                    
                    // Get function tags
                    let tags_response = client.list_tags()
                        .resource(function_arn)
                        .send()
                        .await
                        .map_err(|e| AppError::ExternalService(format!("Failed to get tags for Lambda function {}: {}", function_name, e)))?;
                        
                    // Process tags
                    let mut tags_map = serde_json::Map::new();
                    let mut name = None;
                    
                    if let Some(tags) = tags_response.tags() {
                        for (key, value) in tags {
                            if key == "Name" {
                                name = Some(value.to_string());
                            }
                            tags_map.insert(key.to_string(), json!(value));
                        }
                    }
                    
                    // If no name tag was found, use the function name
                    if name.is_none() {
                        name = Some(function_name.to_string());
                    }
                    
                    // Build function data
                    let mut function_data = serde_json::Map::new();
                    
                    function_data.insert("function_name".to_string(), json!(function_name));
                    function_data.insert("function_arn".to_string(), json!(function_arn));
                    
                    if let Some(runtime) = aws_function.runtime().map(|r| r.as_str()) {
                        function_data.insert("runtime".to_string(), json!(runtime));
                    }
                    
                    if let Some(role) = aws_function.role() {
                        function_data.insert("role".to_string(), json!(role));
                    }
                    
                    if let Some(handler) = aws_function.handler() {
                        function_data.insert("handler".to_string(), json!(handler));
                    }
                    
                    // code_size is not an Option
                    function_data.insert("code_size".to_string(), json!(aws_function.code_size()));
                    
                    if let Some(description) = aws_function.description() {
                        function_data.insert("description".to_string(), json!(description));
                    }
                    
                    if let Some(timeout) = aws_function.timeout() {
                        function_data.insert("timeout".to_string(), json!(timeout));
                    }
                    
                    if let Some(memory_size) = aws_function.memory_size() {
                        function_data.insert("memory_size".to_string(), json!(memory_size));
                    }
                    
                    if let Some(last_modified) = aws_function.last_modified() {
                        function_data.insert("last_modified".to_string(), json!(last_modified));
                    }
                    
                    if let Some(code_sha) = aws_function.code_sha256() {
                        function_data.insert("code_sha256".to_string(), json!(code_sha));
                    }
                    
                    // Handle environment variables
                    if let Some(env) = aws_function.environment() {
                        if let Some(vars) = env.variables() {
                            function_data.insert("environment".to_string(), json!({
                                "variables": vars
                            }));
                        }
                    }
                    
                    if let Some(version) = aws_function.version() {
                        function_data.insert("version".to_string(), json!(version));
                    }
                    
                    let architectures = aws_function.architectures();
                    if !architectures.is_empty() {
                        let arch_list: Vec<String> = architectures.iter()
                            .map(|a| a.as_str().to_string())
                            .collect();

                        function_data.insert("architectures".to_string(), json!(arch_list));
                    }
                    
                    // Create resource DTO
                    let function = AwsResourceDto {
                        id: None,
                        account_id: aws_account_dto.account_id.clone(),
                        profile: aws_account_dto.profile.clone(),
                        region: aws_account_dto.default_region.clone(),
                        resource_type: AwsResourceType::LambdaFunction.to_string(),
                        resource_id: function_name.to_string(),
                        arn: function_arn.to_string(),
                        name,
                        tags: serde_json::Value::Object(tags_map),
                        resource_data: serde_json::Value::Object(function_data),
                        sync_id: None,
                    };
                    
                    // Save to database
                    let saved_function = match repo.find_by_arn(&function_arn).await? {
                        Some(existing) => repo.update(existing.id, &function).await?,
                        None => repo.create(&function).await?,
                    };
                    
                    functions.push(saved_function);
                }
            }
            
            // Check if there are more functions to fetch
            marker = response.next_marker().map(|s| s.to_string());
            if marker.is_none() {
                break;
            }
        }
        
        Ok(functions)
    }
}
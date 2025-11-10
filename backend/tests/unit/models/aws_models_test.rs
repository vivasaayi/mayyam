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


use rstest::*;
use fake::Fake;
use uuid::Uuid;

use crate::models::aws_account::{AwsAccountCreateDto, AwsAccountUpdateDto, DomainModel};
use crate::models::aws_resource::{AwsResourceCreateDto, Model as AwsResource};

/// Test AWS Account Model
#[cfg(test)]
mod aws_account_model_tests {
    use super::*;

    /// Test creating AWS account DTO
    #[rstest]
    fn test_aws_account_create_dto() {
        let dto = AwsAccountCreateDto {
            account_id: "123456789012".to_string(),
            account_name: "Test Account".to_string(),
            profile: Some("test-profile".to_string()),
            default_region: "us-east-1".to_string(),
            use_role: false,
            role_arn: None,
            external_id: None,
            access_key_id: Some("AKIAIOSFODNN7EXAMPLE".to_string()),
            secret_access_key: Some("wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY".to_string()),
        };

        assert_eq!(dto.account_id, "123456789012");
        assert_eq!(dto.account_name, "Test Account");
        assert_eq!(dto.default_region, "us-east-1");
        assert_eq!(dto.use_role, false);
    }

    /// Test AWS account update DTO
    #[rstest]
    fn test_aws_account_update_dto() {
        let dto = AwsAccountUpdateDto {
            account_name: Some("Updated Name".to_string()),
            default_region: Some("us-west-2".to_string()),
            profile: Some("updated-profile".to_string()),
            use_role: Some(true),
            role_arn: Some("arn:aws:iam::123456789012:role/TestRole".to_string()),
            external_id: Some("external-id-123".to_string()),
            access_key_id: Some("NEW_ACCESS_KEY".to_string()),
            secret_access_key: Some("NEW_SECRET_KEY".to_string()),
        };

        assert_eq!(dto.account_name, Some("Updated Name".to_string()));
        assert_eq!(dto.default_region, Some("us-west-2".to_string()));
        assert_eq!(dto.use_role, Some(true));
    }

    /// Test domain model creation
    #[rstest]
    fn test_domain_model_creation() {
        let id = Uuid::new_v4();
        let now = chrono::Utc::now();

        let model = DomainModel {
            id,
            account_id: "123456789012".to_string(),
            account_name: "Test Account".to_string(),
            profile: Some("test-profile".to_string()),
            default_region: "us-east-1".to_string(),
            use_role: false,
            role_arn: None,
            external_id: None,
            access_key_id: Some("AKIAIOSFODNN7EXAMPLE".to_string()),
            secret_access_key: Some("wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY".to_string()),
            created_at: now,
            updated_at: now,
        };

        assert_eq!(model.id, id);
        assert_eq!(model.account_id, "123456789012");
        assert_eq!(model.account_name, "Test Account");
        assert!(model.created_at <= chrono::Utc::now());
        assert!(model.updated_at <= chrono::Utc::now());
    }

    /// Test validation - account ID format
    #[rstest]
    fn test_account_id_validation() {
        // Valid 12-digit account ID
        let valid_dto = AwsAccountCreateDto {
            account_id: "123456789012".to_string(),
            account_name: "Test".to_string(),
            profile: None,
            default_region: "us-east-1".to_string(),
            use_role: false,
            role_arn: None,
            external_id: None,
            access_key_id: Some("KEY".to_string()),
            secret_access_key: Some("SECRET".to_string()),
        };

        // This would typically be validated in the service layer
        assert_eq!(valid_dto.account_id.len(), 12);
        assert!(valid_dto.account_id.chars().all(|c| c.is_numeric()));
    }

    /// Test AWS region validation
    #[rstest]
    fn test_region_validation() {
        let valid_regions = vec![
            "us-east-1", "us-west-2", "eu-west-1", "ap-southeast-1",
            "ca-central-1", "sa-east-1", "eu-central-1", "ap-northeast-1"
        ];

        for region in valid_regions {
            let dto = AwsAccountCreateDto {
                account_id: "123456789012".to_string(),
                account_name: "Test".to_string(),
                profile: None,
                default_region: region.to_string(),
                use_role: false,
                role_arn: None,
                external_id: None,
                access_key_id: Some("KEY".to_string()),
                secret_access_key: Some("SECRET".to_string()),
            };

            assert!(!dto.default_region.is_empty());
            assert!(dto.default_region.contains('-'));
        }
    }
}

/// Test AWS Resource Model
#[cfg(test)]
mod aws_resource_model_tests {
    use super::*;

    /// Test AWS resource creation DTO
    #[rstest]
    fn test_aws_resource_create_dto() {
        let dto = AwsResourceCreateDto {
            account_id: "123456789012".to_string(),
            profile: Some("test-profile".to_string()),
            region: "us-east-1".to_string(),
            resource_type: "ec2".to_string(),
            resource_id: "i-1234567890abcdef0".to_string(),
            arn: "arn:aws:ec2:us-east-1:123456789012:instance/i-1234567890abcdef0".to_string(),
            name: Some("test-instance".to_string()),
            tags: serde_json::json!({"Environment": "test", "Team": "devops"}),
            resource_data: serde_json::json!({"instance_type": "t2.micro", "state": "running"}),
        };

        assert_eq!(dto.account_id, "123456789012");
        assert_eq!(dto.resource_type, "ec2");
        assert_eq!(dto.resource_id, "i-1234567890abcdef0");
        assert!(dto.name.is_some());
    }

    /// Test AWS resource model
    #[rstest]
    fn test_aws_resource_model() {
        let id = Uuid::new_v4();
        let now = chrono::Utc::now();

        let model = AwsResource {
            id,
            account_id: "123456789012".to_string(),
            profile: Some("test-profile".to_string()),
            region: "us-east-1".to_string(),
            resource_type: "ec2".to_string(),
            resource_id: "i-1234567890abcdef0".to_string(),
            arn: "arn:aws:ec2:us-east-1:123456789012:instance/i-1234567890abcdef0".to_string(),
            name: Some("test-instance".to_string()),
            tags: serde_json::json!({"Environment": "test"}),
            resource_data: serde_json::json!({"instance_type": "t2.micro"}),
            created_at: now,
            updated_at: now,
            last_refreshed: now,
        };

        assert_eq!(model.id, id);
        assert_eq!(model.resource_type, "ec2");
        assert_eq!(model.region, "us-east-1");
        assert!(model.created_at <= chrono::Utc::now());
    }

    /// Test resource type validation
    #[rstest]
    fn test_resource_type_validation() {
        let valid_types = vec![
            "ec2", "s3", "rds", "lambda", "dynamodb", "kinesis",
            "sqs", "sns", "elasticache", "opensearch", "cloudwatch"
        ];

        for resource_type in valid_types {
            let dto = AwsResourceCreateDto {
                account_id: "123456789012".to_string(),
                profile: None,
                region: "us-east-1".to_string(),
                resource_type: resource_type.to_string(),
                resource_id: format!("{}-test", resource_type),
                arn: format!("arn:aws:{}:us-east-1:123456789012:resource/{}", resource_type, resource_type),
                name: Some(format!("{}-resource", resource_type)),
                tags: serde_json::json!({}),
                resource_data: serde_json::json!({}),
            };

            assert!(!dto.resource_type.is_empty());
            assert!(dto.resource_type.chars().all(|c| c.is_alphanumeric() || c == '-'));
        }
    }

    /// Test JSON serialization/deserialization
    #[rstest]
    fn test_json_serialization() {
        let original = serde_json::json!({
            "Environment": "production",
            "Team": "platform",
            "Project": "mayyam"
        });

        // Serialize to string
        let serialized = serde_json::to_string(&original).unwrap();

        // Deserialize back
        let deserialized: serde_json::Value = serde_json::from_str(&serialized).unwrap();

        assert_eq!(original, deserialized);
        assert_eq!(deserialized["Environment"], "production");
        assert_eq!(deserialized["Team"], "platform");
    }
}

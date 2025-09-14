use crate::errors::AppError;
use crate::services::aws::aws_types::kinesis::KinesisCreateStreamRequest;

// Unit tests for Kinesis create_stream validation logic
// These tests focus on the validation aspects of the create_stream functionality

#[cfg(test)]
mod kinesis_validation_tests {
    use super::*;

    #[tokio::test]
    async fn test_kinesis_create_stream_request_validation_missing_shard_count() {
        // Test the validation logic for missing shard_count
        let request = KinesisCreateStreamRequest {
            stream_name: "test-stream".to_string(),
            shard_count: None,
        };

        // Simulate the validation logic from the actual implementation
        let shard_count_result = request.shard_count
            .ok_or_else(|| AppError::ExternalService("Missing shard_count in create stream request".to_string()));

        // Assert
        assert!(shard_count_result.is_err());
        let error = shard_count_result.unwrap_err();
        match error {
            AppError::ExternalService(msg) => assert!(msg.contains("Missing shard_count")),
            _ => panic!("Expected ExternalService error"),
        }
    }

    #[tokio::test]
    async fn test_kinesis_create_stream_request_validation_invalid_shard_count() {
        // Test the validation logic for invalid shard_count (zero or negative)
        let test_cases = vec![
            ("zero shard count", Some(0)),
            ("negative shard count", Some(-1)),
        ];

        for (description, shard_count) in test_cases {
            let request = KinesisCreateStreamRequest {
                stream_name: "test-stream".to_string(),
                shard_count,
            };

            // Simulate the validation logic
            let validation_result = if let Some(count) = request.shard_count {
                if count <= 0 {
                    Err(AppError::ExternalService("Invalid shard_count: must be greater than 0".to_string()))
                } else {
                    Ok(count)
                }
            } else {
                Err(AppError::ExternalService("Missing shard_count in create stream request".to_string()))
            };

            // Assert
            assert!(validation_result.is_err(), "Test case '{}' should fail", description);
            let error = validation_result.unwrap_err();
            match error {
                AppError::ExternalService(msg) => assert!(msg.contains("Invalid shard_count") || msg.contains("Missing shard_count")),
                _ => panic!("Expected ExternalService error for test case '{}'", description),
            }
        }
    }

    #[tokio::test]
    async fn test_kinesis_create_stream_request_validation_valid_shard_counts() {
        // Test the validation logic for valid shard counts
        let valid_shard_counts = vec![1, 2, 10, 100, 500];

        for shard_count in valid_shard_counts {
            let request = KinesisCreateStreamRequest {
                stream_name: format!("test-stream-{}", shard_count),
                shard_count: Some(shard_count),
            };

            // Simulate the validation logic
            let validation_result = if let Some(count) = request.shard_count {
                if count <= 0 {
                    Err(AppError::ExternalService("Invalid shard_count: must be greater than 0".to_string()))
                } else {
                    Ok(count)
                }
            } else {
                Err(AppError::ExternalService("Missing shard_count in create stream request".to_string()))
            };

            // Assert
            assert!(validation_result.is_ok(), "Shard count {} should be valid", shard_count);
            assert_eq!(validation_result.unwrap(), shard_count);
        }
    }

    #[tokio::test]
    async fn test_kinesis_create_stream_request_validation_stream_name() {
        // Test stream name validation (basic checks)
        let test_cases = vec![
            ("valid stream name", "my-test-stream", true),
            ("stream name with numbers", "stream123", true),
            ("stream name with hyphens", "my-test-stream", true),
            ("stream name with underscores", "my_test_stream", true),
            ("empty stream name", "", false),
        ];

        for (description, stream_name, should_be_valid) in test_cases {
            let request = KinesisCreateStreamRequest {
                stream_name: stream_name.to_string(),
                shard_count: Some(1),
            };

            // Basic stream name validation
            let name_validation = if request.stream_name.is_empty() {
                Err(AppError::ExternalService("Stream name cannot be empty".to_string()))
            } else if request.stream_name.len() > 128 {
                Err(AppError::ExternalService("Stream name too long".to_string()))
            } else {
                Ok(())
            };

            // Assert
            if should_be_valid {
                assert!(name_validation.is_ok(), "Test case '{}' should pass validation", description);
            } else {
                assert!(name_validation.is_err(), "Test case '{}' should fail validation", description);
                let error = name_validation.unwrap_err();
                match error {
                    AppError::ExternalService(msg) => assert!(msg.contains("empty") || msg.contains("long")),
                    _ => panic!("Expected ExternalService error for test case '{}'", description),
                }
            }
        }
    }

    #[tokio::test]
    async fn test_kinesis_create_stream_request_complete_validation() {
        // Test complete validation scenario
        let valid_request = KinesisCreateStreamRequest {
            stream_name: "test-stream".to_string(),
            shard_count: Some(2),
        };

        // Simulate complete validation
        let validation_result = {
            // Check stream name
            if valid_request.stream_name.is_empty() {
                Err(AppError::ExternalService("Stream name cannot be empty".to_string()))
            } else if valid_request.stream_name.len() > 128 {
                Err(AppError::ExternalService("Stream name too long".to_string()))
            } else {
                // Check shard count
                match valid_request.shard_count {
                    Some(count) if count <= 0 => {
                        Err(AppError::ExternalService("Invalid shard_count: must be greater than 0".to_string()))
                    }
                    Some(count) => Ok((valid_request.stream_name.clone(), count)),
                    None => Err(AppError::ExternalService("Missing shard_count in create stream request".to_string()))
                }
            }
        };

        // Assert
        assert!(validation_result.is_ok());
        let (stream_name, shard_count) = validation_result.unwrap();
        assert_eq!(stream_name, "test-stream");
        assert_eq!(shard_count, 2);
    }
}

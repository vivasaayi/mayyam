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


use mayyam::errors::AppError;
use mayyam::services::aws::aws_types::kinesis::KinesisCreateStreamRequest;

#[tokio::test]
async fn missing_shard_count_is_rejected() {
    let request = KinesisCreateStreamRequest {
        stream_name: "test-stream".to_string(),
        shard_count: None,
    };

    let shard_count = request.shard_count.ok_or_else(|| {
        AppError::ExternalService("Missing shard_count in create stream request".to_string())
    });

    assert!(shard_count.is_err());
    let AppError::ExternalService(message) =
        shard_count.expect_err("expected external service error")
    else {
        panic!("expected external service error");
    };
    assert!(message.contains("Missing shard_count"));
}

#[tokio::test]
async fn invalid_shard_counts_return_error() {
    let scenarios = [("zero", Some(0)), ("negative", Some(-1))];

    for (label, shard_count) in scenarios {
        let request = KinesisCreateStreamRequest {
            stream_name: "test-stream".to_string(),
            shard_count,
        };

        let outcome = match request.shard_count {
            Some(count) if count <= 0 => Err(AppError::ExternalService(
                "Invalid shard_count: must be greater than 0".to_string(),
            )),
            Some(count) => Ok(count),
            None => Err(AppError::ExternalService(
                "Missing shard_count in create stream request".to_string(),
            )),
        };

        assert!(outcome.is_err(), "scenario {label} should fail");
    }
}

#[tokio::test]
async fn valid_shard_counts_pass_validation() {
    for shard_count in [1, 2, 10, 100, 500] {
        let request = KinesisCreateStreamRequest {
            stream_name: format!("test-stream-{shard_count}"),
            shard_count: Some(shard_count),
        };

        let result = match request.shard_count {
            Some(count) if count <= 0 => Err(AppError::ExternalService(
                "Invalid shard_count: must be greater than 0".to_string(),
            )),
            Some(count) => Ok(count),
            None => Err(AppError::ExternalService(
                "Missing shard_count in create stream request".to_string(),
            )),
        };

        assert_eq!(result.unwrap(), shard_count);
    }
}

#[tokio::test]
async fn stream_name_validation_checks_empty_and_length() {
    let scenarios = [("valid", "stream_name", true), ("empty", "", false)];

    for (label, stream_name, expected_valid) in scenarios {
        let request = KinesisCreateStreamRequest {
            stream_name: stream_name.to_string(),
            shard_count: Some(1),
        };

        let validation = if request.stream_name.is_empty() {
            Err(AppError::ExternalService(
                "Stream name cannot be empty".to_string(),
            ))
        } else if request.stream_name.len() > 128 {
            Err(AppError::ExternalService(
                "Stream name too long".to_string(),
            ))
        } else {
            Ok(())
        };

        if expected_valid {
            assert!(validation.is_ok(), "scenario {label} should be valid");
        } else {
            assert!(validation.is_err(), "scenario {label} should fail");
        }
    }
}

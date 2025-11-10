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


use std::time::Duration;
use tokio::time::sleep;

/// Configuration for retry behavior
#[derive(Debug, Clone)]
pub struct RetryConfig {
    pub max_attempts: u32,
    pub initial_delay: Duration,
    pub max_delay: Duration,
    pub backoff_multiplier: f64,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            initial_delay: Duration::from_millis(100),
            max_delay: Duration::from_secs(30),
            backoff_multiplier: 2.0,
        }
    }
}

/// Retry a fallible operation with exponential backoff
pub async fn retry_with_backoff<F, Fut, T, E>(
    config: &RetryConfig,
    operation: F,
) -> Result<T, E>
where
    F: Fn() -> Fut,
    Fut: std::future::Future<Output = Result<T, E>>,
    E: std::fmt::Debug,
{
    let mut attempt = 0;
    let mut delay = config.initial_delay;

    loop {
        attempt += 1;

        match operation().await {
            Ok(result) => return Ok(result),
            Err(error) => {
                if attempt >= config.max_attempts {
                    tracing::error!("Operation failed after {} attempts: {:?}", attempt, error);
                    return Err(error);
                }

                tracing::warn!("Operation failed (attempt {}/{}): {:?}", attempt, config.max_attempts, error);
                sleep(delay).await;

                // Calculate next delay with exponential backoff
                delay = std::cmp::min(
                    Duration::from_millis((delay.as_millis() as f64 * config.backoff_multiplier) as u64),
                    config.max_delay,
                );
            }
        }
    }
}

/// Retry configuration for database operations
pub fn db_retry_config() -> RetryConfig {
    RetryConfig {
        max_attempts: 5,
        initial_delay: Duration::from_millis(50),
        max_delay: Duration::from_secs(10),
        backoff_multiplier: 2.0,
    }
}

/// Retry configuration for network operations
pub fn network_retry_config() -> RetryConfig {
    RetryConfig {
        max_attempts: 3,
        initial_delay: Duration::from_millis(200),
        max_delay: Duration::from_secs(5),
        backoff_multiplier: 1.5,
    }
}

/// Retry configuration for AWS operations
pub fn aws_retry_config() -> RetryConfig {
    RetryConfig {
        max_attempts: 5,
        initial_delay: Duration::from_millis(100),
        max_delay: Duration::from_secs(20),
        backoff_multiplier: 2.0,
    }
}
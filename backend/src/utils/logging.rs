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


use std::env;
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

pub fn init_logger() {
    // Check if we're running in debug or release mode
    let is_debug = cfg!(debug_assertions);

    // Get log level from environment variable or use default based on build mode
    let log_level = env::var("MAYYAM_LOG_LEVEL").unwrap_or_else(|_| {
        if is_debug {
            "debug".to_string()
        } else {
            "info".to_string()
        }
    });

    // Set up file appender for persistent logs
    let log_dir = env::var("MAYYAM_LOG_DIR").unwrap_or_else(|_| "logs".to_string());
    let file_appender = RollingFileAppender::new(Rotation::DAILY, log_dir, "mayyam.log");

    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);

    // Create a custom filter directive
    let filter = EnvFilter::try_from_default_env()
        .or_else(|_| EnvFilter::try_new(&format!("mayyam={},actix_web=info", log_level)))
        .unwrap();

    // Initialize the tracing subscriber with both console and file outputs
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer().with_writer(std::io::stdout))
        .with(tracing_subscriber::fmt::layer().with_writer(non_blocking))
        .with(filter)
        .init();

    // Store the guard in a static variable to keep it alive for the program's lifetime
    // This ensures logs are properly flushed
    static mut GUARD: Option<tracing_appender::non_blocking::WorkerGuard> = None;
    unsafe {
        GUARD = Some(_guard);
    }
}

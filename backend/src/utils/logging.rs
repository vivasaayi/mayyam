use std::env;
use tracing_subscriber::{fmt, EnvFilter};
use tracing_appender::rolling::{RollingFileAppender, Rotation};

pub fn init_logger() {
    let log_level = env::var("LOG_LEVEL").unwrap_or_else(|_| "info".to_string());
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(format!("mayyam={}", log_level)));

    // Log to file if LOG_FILE is set
    if let Ok(log_dir) = env::var("LOG_DIR") {
        let file_appender = RollingFileAppender::new(
            Rotation::DAILY,
            log_dir,
            "mayyam.log",
        );
        
        let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);
        
        tracing_subscriber::fmt()
            .with_env_filter(env_filter)
            .with_writer(non_blocking)
            .init();
            
        // We need to keep the guard alive for the entire program
        // Store it in a lazy_static or similar if needed
    } else {
        // Log to console
        tracing_subscriber::fmt()
            .with_env_filter(env_filter)
            .init();
    }
}

use tracing::info;

// Simple notification functions
pub fn send_alert(title: &str, message: &str) {
    info!("Alert: {} - {}", title, message);
    // Placeholder for actual notification logic
}

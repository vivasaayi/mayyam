use once_cell::sync::OnceCell;
use std::net::TcpListener;
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;
use std::time::{Duration, Instant};

static SERVER_HANDLE: OnceCell<Mutex<Option<Child>>> = OnceCell::new();
static BASE_URL: OnceCell<String> = OnceCell::new();

fn find_free_port() -> u16 {
    TcpListener::bind(("127.0.0.1", 0))
        .expect("failed to bind ephemeral port")
        .local_addr()
        .expect("failed to read local_addr")
        .port()
}

fn start_server_on_port(port: u16) -> (Child, String) {
    // Ensure CONFIG_FILE points to test config
    let mut cmd = Command::new("cargo");
    cmd.arg("run")
        .arg("--quiet")
        .arg("--")
        .arg("server")
        .arg("--port")
        .arg(port.to_string())
        .env("CONFIG_FILE", "config.test.yml")
        .stdout(Stdio::null())
        .stderr(Stdio::null());

    // For macOS CI/local, we need to set the working directory to backend/
    // so the binary sees the config paths correctly.
    cmd.current_dir(env!("CARGO_MANIFEST_DIR"));

    let child = cmd.spawn().expect("failed to spawn backend server");
    let base_url = format!("http://127.0.0.1:{}", port);
    (child, base_url)
}

async fn wait_for_health(base_url: &str, timeout: Duration) -> bool {
    let client = reqwest::Client::new();
    let start = Instant::now();
    while start.elapsed() < timeout {
        if let Ok(resp) = client.get(format!("{}/health", base_url)).send().await {
            if resp.status().is_success() {
                return true;
            }
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
    false
}

/// Ensure the test server is running and return the base URL.
pub async fn ensure_server() -> String {
    // If already started, return existing base URL
    if let Some(url) = BASE_URL.get() {
        return url.clone();
    }

    // 1) If TEST_API_BASE_URL is set and healthy, use it.
    if let Ok(url) = std::env::var("TEST_API_BASE_URL") {
        if wait_for_health(&url, Duration::from_secs(2)).await {
            BASE_URL.set(url.clone()).ok();
            return url;
        }
    }

    // 2) If BACKEND_PORT is set, prefer using the already running backend for speed.
    if let Ok(port_str) = std::env::var("BACKEND_PORT") {
        if let Ok(port) = port_str.parse::<u16>() {
            let base_url = format!("http://127.0.0.1:{}", port);
            if wait_for_health(&base_url, Duration::from_secs(2)).await {
                BASE_URL.set(base_url.clone()).ok();
                std::env::set_var("TEST_API_BASE_URL", base_url.clone());
                return base_url;
            }
        }
    }

    // 3) Try the conventional default port 8010 if a backend is already running.
    let default_port = 8010u16;
    let default_url = format!("http://127.0.0.1:{}", default_port);
    if wait_for_health(&default_url, Duration::from_secs(2)).await {
        BASE_URL.set(default_url.clone()).ok();
        std::env::set_var("TEST_API_BASE_URL", default_url.clone());
        return default_url;
    }

    // 4) Fallback: spawn a fresh ephemeral server instance.
    let port = find_free_port();
    let (child, base_url) = start_server_on_port(port);

    // Store handle
    let handle_mutex = SERVER_HANDLE.get_or_init(|| Mutex::new(None));
    {
        let mut guard = handle_mutex.lock().expect("poisoned mutex");
        *guard = Some(child);
    }

    // Wait for health
    let ok = wait_for_health(&base_url, Duration::from_secs(20)).await;
    assert!(ok, "backend server did not become healthy at {}", base_url);

    // Cache base_url
    BASE_URL.set(base_url.clone()).ok();
    // Export for legacy tests that read from env
    std::env::set_var("TEST_API_BASE_URL", base_url.clone());

    base_url
}

/// Try to start the server; return None if health never becomes ready within timeout.
pub async fn try_ensure_server() -> Option<String> {
    if let Some(url) = BASE_URL.get() {
        return Some(url.clone());
    }

    let port = find_free_port();
    let (mut child, base_url) = start_server_on_port(port);

    // Store handle
    let handle_mutex = SERVER_HANDLE.get_or_init(|| Mutex::new(None));
    let ok = wait_for_health(&base_url, Duration::from_secs(20)).await;
    if !ok {
        // Ensure we don't leak a child process if health never became ready
        let _ = child.kill();
        eprintln!("[integration] Skipping: backend server not healthy at {} (likely DB not available)", base_url);
        return None;
    }

    // Health OK: retain the child handle so the process stays alive during tests
    {
        let mut guard = handle_mutex.lock().expect("poisoned mutex");
        *guard = Some(child);
    }

    BASE_URL.set(base_url.clone()).ok();
    std::env::set_var("TEST_API_BASE_URL", base_url.clone());
    Some(base_url)
}

/// Returns the base URL. Starts the server if not yet running.
pub async fn base_url() -> String {
    if let Some(url) = BASE_URL.get() {
        url.clone()
    } else {
        ensure_server().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn server_starts_and_health_ok() {
        let url = ensure_server().await;
        let res = reqwest::get(format!("{}/health", url)).await.unwrap();
        assert!(res.status().is_success());
    }
}

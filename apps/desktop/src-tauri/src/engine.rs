use base64::Engine;
use rand::Rng;
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::Duration;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum EngineError {
    #[error("OpenCode binary not found at {0}")]
    BinaryNotFound(PathBuf),
    #[error("IO error while managing engine process: {0}")]
    IoError(#[from] std::io::Error),
    #[error("Server failed to reach healthy state: {0}")]
    ReadinessTimeout(String),
    #[error("HTTP client error: {0}")]
    HttpError(#[from] reqwest::Error),
    #[error("Session operation failed: {0}")]
    SessionError(String),
}

pub struct EngineSupervisor {
    child: Child,
    port: u16,
    password: String,
    version: String,
}

impl EngineSupervisor {
    /// Locates the pinned OpenCode binary in the Tauri binaries bundle directory.
    pub fn locate_pinned_binary() -> Result<PathBuf, EngineError> {
        let candidates = [
            PathBuf::from("binaries/opencode.exe"),
            PathBuf::from("binaries/opencode-x86_64-pc-windows-gnu.exe"),
            PathBuf::from("binaries/opencode-x86_64-pc-windows-msvc.exe"),
            PathBuf::from("apps/desktop/src-tauri/binaries/opencode.exe"),
        ];

        for path in &candidates {
            if path.exists() {
                return Ok(path.clone());
            }
        }

        // Check relative to current exe
        if let Ok(exe_dir) = std::env::current_exe() {
            if let Some(parent) = exe_dir.parent() {
                let p1 = parent.join("opencode.exe");
                if p1.exists() {
                    return Ok(p1);
                }
                let p2 = parent.join("binaries").join("opencode.exe");
                if p2.exists() {
                    return Ok(p2);
                }
            }
        }

        Err(EngineError::BinaryNotFound(PathBuf::from("opencode.exe")))
    }

    /// Spawns the OpenCode HTTP server bound strictly to loopback 127.0.0.1.
    pub async fn start(working_dir: Option<&Path>) -> Result<Self, EngineError> {
        let bin_path = Self::locate_pinned_binary()?;

        // Pick free loopback port
        let listener = TcpListener::bind("127.0.0.1:0")?;
        let port = listener.local_addr()?.port();
        drop(listener);

        // Random 32-character password kept in memory
        let password: String = rand::thread_rng()
            .sample_iter(&rand::distributions::Alphanumeric)
            .take(32)
            .map(char::from)
            .collect();

        let mut cmd = Command::new(&bin_path);
        cmd.arg("serve")
            .arg("--hostname")
            .arg("127.0.0.1")
            .arg("--port")
            .arg(port.to_string())
            .arg("--pure")
            .env("OPENCODE_SERVER_PASSWORD", &password)
            .env("OPENCODE_SERVER_USERNAME", "opencode")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        if let Some(dir) = working_dir {
            cmd.current_dir(dir);
        }

        let mut child = cmd.spawn()?;

        let client = reqwest::Client::new();
        let health_url = format!("http://127.0.0.1:{}/global/health", port);
        let auth_header = format!(
            "Basic {}",
            base64::engine::general_purpose::STANDARD
                .encode(format!("opencode:{}", password))
        );

        let mut ready = false;
        let mut version = String::from("1.19.5");

        for _ in 0..30 {
            tokio::time::sleep(Duration::from_millis(200)).await;

            if let Ok(Some(status)) = child.try_wait() {
                return Err(EngineError::ReadinessTimeout(format!(
                    "OpenCode process terminated prematurely with status: {:?}",
                    status
                )));
            }

            if let Ok(resp) = client
                .get(&health_url)
                .header("Authorization", &auth_header)
                .send()
                .await
            {
                if resp.status().is_success() {
                    if let Ok(body) = resp.json::<serde_json::Value>().await {
                        if body.get("healthy").and_then(|h| h.as_bool()) == Some(true) {
                            if let Some(v) = body.get("version").and_then(|v| v.as_str()) {
                                version = v.to_string();
                            }
                            ready = true;
                            break;
                        }
                    }
                }
            }
        }

        if !ready {
            let _ = child.kill();
            return Err(EngineError::ReadinessTimeout(
                "Timed out waiting for OpenCode /global/health readiness".into(),
            ));
        }

        Ok(Self {
            child,
            port,
            password,
            version,
        })
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    pub fn version(&self) -> &str {
        &self.version
    }

    pub fn auth_header(&self) -> String {
        format!(
            "Basic {}",
            base64::engine::general_purpose::STANDARD
                .encode(format!("opencode:{}", self.password))
        )
    }

    pub fn sse_event_url(&self) -> String {
        format!("http://127.0.0.1:{}/event", self.port)
    }

    /// Creates a dedicated OpenCode session for local Ollama execution.
    pub async fn create_session(&self, directory: &str) -> Result<String, EngineError> {
        let client = reqwest::Client::new();
        let url = format!("http://127.0.0.1:{}/session", self.port);

        let body = serde_json::json!({
            "directory": directory
        });

        let resp = client
            .post(&url)
            .header("Authorization", self.auth_header())
            .json(&body)
            .send()
            .await?;

        if !resp.status().is_success() {
            let err_txt = resp.text().await.unwrap_or_default();
            return Err(EngineError::SessionError(format!(
                "Failed to create session: {err_txt}"
            )));
        }

        let json: serde_json::Value = resp.json().await?;
        let session_id = json
            .get("id")
            .and_then(|id| id.as_str())
            .ok_or_else(|| EngineError::SessionError("Missing id in session response".into()))?
            .to_string();

        Ok(session_id)
    }

    /// Sends an asynchronous prompt to the session configured strictly with the Ollama worker model.
    pub async fn send_prompt(
        &self,
        session_id: &str,
        prompt_text: &str,
        worker_model: &str,
    ) -> Result<(), EngineError> {
        let client = reqwest::Client::new();
        let url = format!(
            "http://127.0.0.1:{}/session/{}/prompt_async",
            self.port, session_id
        );

        let body = serde_json::json!({
            "parts": [
                {
                    "type": "text",
                    "text": prompt_text
                }
            ],
            "model": {
                "provider": "ollama",
                "name": worker_model
            }
        });

        let resp = client
            .post(&url)
            .header("Authorization", self.auth_header())
            .json(&body)
            .send()
            .await?;

        if !resp.status().is_success() {
            let err_txt = resp.text().await.unwrap_or_default();
            return Err(EngineError::SessionError(format!(
                "Failed to send worker prompt: {err_txt}"
            )));
        }

        Ok(())
    }

    /// Stops an active worker session.
    pub async fn abort_session(&self, session_id: &str) -> Result<(), EngineError> {
        let client = reqwest::Client::new();
        let url = format!(
            "http://127.0.0.1:{}/session/{}/abort",
            self.port, session_id
        );

        let _ = client
            .post(&url)
            .header("Authorization", self.auth_header())
            .send()
            .await;

        Ok(())
    }

    /// Graceful termination with kill fallback.
    pub fn shutdown(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Drop for EngineSupervisor {
    fn drop(&mut self) {
        self.shutdown();
    }
}

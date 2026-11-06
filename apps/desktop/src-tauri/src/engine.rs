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

    /// Returns the canonical path to the user's OpenCode configuration file.
    pub fn get_opencode_config_path() -> Option<PathBuf> {
        let config_dir = if let Ok(profile) = std::env::var("USERPROFILE") {
            PathBuf::from(profile).join(".config").join("opencode")
        } else if let Ok(home) = std::env::var("HOME") {
            PathBuf::from(home).join(".config").join("opencode")
        } else {
            return None;
        };
        Some(config_dir.join("opencode.json"))
    }

    /// Ensures that OpenCode's configuration file registers the local Ollama provider
    /// and its models (including granite4.1:8b and qwen2.5-coder:7b).
    pub fn ensure_ollama_config() -> Result<PathBuf, EngineError> {
        let config_path = Self::get_opencode_config_path()
            .ok_or_else(|| EngineError::SessionError("Could not determine user profile or home directory for OpenCode configuration".into()))?;

        if let Some(parent) = config_path.parent() {
            if !parent.exists() {
                let _ = std::fs::create_dir_all(parent);
            }
        }

        let mut config: serde_json::Value = if config_path.exists() {
            if let Ok(content) = std::fs::read_to_string(&config_path) {
                serde_json::from_str(&content).unwrap_or_else(|_| serde_json::json!({}))
            } else {
                serde_json::json!({})
            }
        } else {
            serde_json::json!({
                "$schema": "https://opencode.ai/config.json"
            })
        };

        if config.get("$schema").is_none() {
            config["$schema"] = serde_json::Value::String("https://opencode.ai/config.json".into());
        }

        if !config.get("provider").map(|p| p.is_object()).unwrap_or(false) {
            config["provider"] = serde_json::json!({});
        }

        let provider_obj = config.get_mut("provider").unwrap().as_object_mut().unwrap();

        if !provider_obj.contains_key("ollama") {
            provider_obj.insert(
                "ollama".to_string(),
                serde_json::json!({
                    "name": "Ollama",
                    "npm": "@ai-sdk/openai-compatible",
                    "options": {
                        "baseURL": "http://127.0.0.1:11434/v1"
                    },
                    "models": {
                        "granite4.1:8b": {
                            "name": "Granite 4.1 8B"
                        },
                        "qwen2.5-coder:7b": {
                            "name": "Qwen 2.5 Coder 7B"
                        }
                    }
                }),
            );
        } else {
            let ollama_val = provider_obj.get_mut("ollama").unwrap();
            if !ollama_val.is_object() {
                *ollama_val = serde_json::json!({
                    "name": "Ollama",
                    "npm": "@ai-sdk/openai-compatible",
                    "options": {
                        "baseURL": "http://127.0.0.1:11434/v1"
                    },
                    "models": {}
                });
            }

            let ollama_map = ollama_val.as_object_mut().unwrap();
            if !ollama_map.contains_key("name") {
                ollama_map.insert("name".to_string(), serde_json::Value::String("Ollama".into()));
            }
            if !ollama_map.contains_key("npm") {
                ollama_map.insert("npm".to_string(), serde_json::Value::String("@ai-sdk/openai-compatible".into()));
            }
            if !ollama_map.contains_key("options") || !ollama_map["options"].is_object() {
                ollama_map.insert("options".to_string(), serde_json::json!({
                    "baseURL": "http://127.0.0.1:11434/v1"
                }));
            }
            if !ollama_map.contains_key("models") || !ollama_map["models"].is_object() {
                ollama_map.insert("models".to_string(), serde_json::json!({}));
            }

            let models_obj = ollama_map.get_mut("models").unwrap().as_object_mut().unwrap();
            if !models_obj.contains_key("granite4.1:8b") {
                models_obj.insert(
                    "granite4.1:8b".to_string(),
                    serde_json::json!({
                        "name": "Granite 4.1 8B"
                    }),
                );
            }
            if !models_obj.contains_key("qwen2.5-coder:7b") {
                models_obj.insert(
                    "qwen2.5-coder:7b".to_string(),
                    serde_json::json!({
                        "name": "Qwen 2.5 Coder 7B"
                    }),
                );
            }
        }

        if let Ok(json_str) = serde_json::to_string_pretty(&config) {
            let _ = std::fs::write(&config_path, json_str);
        }

        Ok(config_path)
    }

    /// Queries local Ollama API to dynamically discover and register installed models into the OpenCode config.
    pub async fn sync_installed_ollama_models(config_path: &Path) {
        let client = reqwest::Client::new();
        let resp = match client
            .get("http://127.0.0.1:11434/api/tags")
            .timeout(Duration::from_millis(1500))
            .send()
            .await
        {
            Ok(r) => r,
            Err(_) => return,
        };

        let body: serde_json::Value = match resp.json().await {
            Ok(b) => b,
            Err(_) => return,
        };

        let models = match body.get("models").and_then(|m| m.as_array()) {
            Some(m) => m,
            None => return,
        };

        if let Ok(content) = std::fs::read_to_string(config_path) {
            if let Ok(mut config) = serde_json::from_str::<serde_json::Value>(&content) {
                if let Some(models_obj) = config
                    .get_mut("provider")
                    .and_then(|p| p.get_mut("ollama"))
                    .and_then(|o| o.get_mut("models"))
                    .and_then(|m| m.as_object_mut())
                {
                    let mut modified = false;
                    for item in models {
                        if let Some(name) = item.get("name").and_then(|n| n.as_str()) {
                            if !models_obj.contains_key(name) {
                                models_obj.insert(
                                    name.to_string(),
                                    serde_json::json!({
                                        "name": name
                                    }),
                                );
                                modified = true;
                            }
                        }
                    }

                    if modified {
                        if let Ok(json_str) = serde_json::to_string_pretty(&config) {
                            let _ = std::fs::write(config_path, json_str);
                        }
                    }
                }
            }
        }
    }

    /// Spawns the OpenCode HTTP server bound strictly to loopback 127.0.0.1.
    pub async fn start(working_dir: Option<&Path>) -> Result<Self, EngineError> {
        // Automatically ensure Ollama provider and granite4.1:8b are registered
        if let Ok(cfg_path) = Self::ensure_ollama_config() {
            Self::sync_installed_ollama_models(&cfg_path).await;
        }

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
            "title": format!("Worker: {}", Path::new(directory).file_name().and_then(|n| n.to_str()).unwrap_or("project"))
        });

        let resp = client
            .post(&url)
            .query(&[("directory", directory)])
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
                "providerID": "ollama",
                "modelID": worker_model
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_locate_pinned_binary() {
        let bin_res = EngineSupervisor::locate_pinned_binary();
        assert!(
            bin_res.is_ok(),
            "Must be able to locate the pinned opencode binary"
        );
        let path = bin_res.unwrap();
        assert!(path.exists(), "Pinned binary path must exist on disk");
    }

    #[test]
    fn test_ensure_ollama_config_structure() {
        let config_res = EngineSupervisor::ensure_ollama_config();
        assert!(config_res.is_ok(), "ensure_ollama_config must succeed");
        let path = config_res.unwrap();
        assert!(path.exists(), "Configuration file must exist at {path:?}");

        let content = std::fs::read_to_string(&path).expect("Must read config file");
        let json: serde_json::Value =
            serde_json::from_str(&content).expect("Must be valid JSON");

        let ollama = &json["provider"]["ollama"];
        assert_eq!(
            ollama["name"].as_str(),
            Some("Ollama"),
            "Provider name must be Ollama"
        );
        assert_eq!(
            ollama["npm"].as_str(),
            Some("@ai-sdk/openai-compatible"),
            "Provider npm must be @ai-sdk/openai-compatible"
        );
        assert_eq!(
            ollama["options"]["baseURL"].as_str(),
            Some("http://127.0.0.1:11434/v1"),
            "Ollama baseURL must point to loopback 11434 v1"
        );
        assert!(
            ollama["models"]["granite4.1:8b"].is_object(),
            "granite4.1:8b must be registered in ollama models"
        );
    }
}

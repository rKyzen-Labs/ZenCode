use base64::Engine;
use rand::Rng;
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::Duration;

pub struct OpenCodeSidecar {
    pub child: Child,
    pub port: u16,
    pub password: String,
}

impl OpenCodeSidecar {
    pub async fn start(bin_path: &Path, working_dir: &Path) -> Result<Self, Box<dyn std::error::Error>> {
        let listener = TcpListener::bind("127.0.0.1:0")?;
        let port = listener.local_addr()?.port();
        drop(listener);

        let password: String = rand::thread_rng()
            .sample_iter(&rand::distributions::Alphanumeric)
            .take(32)
            .map(char::from)
            .collect();

        let mut child = Command::new(bin_path)
            .arg("serve")
            .arg("--hostname")
            .arg("127.0.0.1")
            .arg("--port")
            .arg(port.to_string())
            .arg("--pure")
            .current_dir(working_dir)
            .env("OPENCODE_SERVER_PASSWORD", &password)
            .env("OPENCODE_SERVER_USERNAME", "opencode")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        let client = reqwest::Client::builder()
            .timeout(Duration::from_millis(500))
            .build()?;
        let health_url = format!("http://127.0.0.1:{port}/global/health");
        let auth = format!(
            "Basic {}",
            base64::engine::general_purpose::STANDARD.encode(format!("opencode:{password}"))
        );

        let mut ready = false;
        for _ in 0..30 {
            tokio::time::sleep(Duration::from_millis(250)).await;
            if let Ok(Some(status)) = child.try_wait() {
                return Err(format!("OpenCode exited early with {status:?}").into());
            }
            if let Ok(resp) = client.get(&health_url).header("Authorization", &auth).send().await {
                if resp.status().is_success() {
                    ready = true;
                    break;
                }
            }
        }

        if !ready {
            let _ = child.kill();
            return Err("Timed out waiting for OpenCode to report ready".into());
        }

        Ok(Self { child, port, password })
    }

    pub fn auth_header(&self) -> String {
        format!(
            "Basic {}",
            base64::engine::general_purpose::STANDARD.encode(format!("opencode:{}", self.password))
        )
    }

    pub fn sse_url(&self) -> String {
        format!("http://127.0.0.1:{}/event", self.port)
    }

    pub fn shutdown(mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

pub fn opencode_binary_path() -> PathBuf {
    PathBuf::from(r"A:\ZenCode\apps\desktop\src-tauri\binaries\opencode.exe")
}

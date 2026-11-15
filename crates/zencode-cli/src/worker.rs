use serde_json::json;
use std::path::Path;
use zencode_core::AppliedDiffResult;

#[derive(Debug, Clone)]
pub enum WorkerProvider {
    Groq,
    OpenRouter,
    Ollama,
}

pub struct CliWorkerClient {
    pub provider: WorkerProvider,
    pub api_key: String,
    pub endpoint_url: String,
    pub model: String,
}

impl CliWorkerClient {
    pub fn new_groq(api_key: String, model: Option<String>) -> Self {
        Self {
            provider: WorkerProvider::Groq,
            api_key,
            endpoint_url: "https://api.groq.com/openai/v1/chat/completions".into(),
            model: model.unwrap_or_else(|| "openai/gpt-oss-120b".into()),
        }
    }

    pub fn new_openrouter(api_key: String, model: Option<String>) -> Self {
        Self {
            provider: WorkerProvider::OpenRouter,
            api_key,
            endpoint_url: "https://openrouter.ai/api/v1/chat/completions".into(),
            model: model.unwrap_or_else(|| "nvidia/nemotron-3-nano-omni-30b-a3b-reasoning:free".into()),
        }
    }

    pub fn new_ollama(model: Option<String>) -> Self {
        Self {
            provider: WorkerProvider::Ollama,
            api_key: "ollama".into(),
            endpoint_url: "http://localhost:11434/v1/chat/completions".into(),
            model: model.unwrap_or_else(|| "qwen2.5-coder:7b".into()),
        }
    }

    pub async fn execute_task(
        &self,
        worktree_path: &Path,
        task_summary: &str,
        steps: &[zencode_core::PlanStep],
    ) -> Result<Vec<AppliedDiffResult>, String> {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(180))
            .build()
            .map_err(|e| e.to_string())?;

        let system_prompt = r#"You are an elite Senior Staff Autonomous Software Engineer and UX/UI Specialist executing code changes in an isolated Git worktree.

CORE MANDATE:
1. CRAFTSMANSHIP & ZERO-STUB MANDATE:
   - NEVER emit placeholders, truncated bodies, ellipsis (...), or comments like '// TODO: implement later'.
   - Always write 100% complete, fully implemented, production-ready code.

2. SURGICAL DIFF MANDATE:
   - When modifying files that already exist, ALWAYS use surgical SEARCH/REPLACE diff blocks:
FILE: relative/path/to/file.ext
<<<<<<< SEARCH
[exact existing lines from the file to replace]
=======
[replacement lines]
>>>>>>> REPLACE

3. For creating a new file or writing a complete file:
<<<<<<< CREATE: relative/path/to/new_file.ext
[full complete file content]
>>>>>>>
or:
### File: relative/path/to/new_file.ext
```lang
[full complete file content]
```"#;

        let mut user_prompt = format!(
            "Execute the following implementation plan with zero stubs and highest software craftsmanship:\n\nTask: {}\n\nSteps:\n",
            task_summary
        );

        for (i, step) in steps.iter().enumerate() {
            user_prompt.push_str(&format!("{}. Goal: {}\n   Instructions:\n", i + 1, step.goal));
            for inst in &step.instructions {
                user_prompt.push_str(&format!("      - {}\n", inst));
            }
            if !step.likely_files.is_empty() {
                user_prompt.push_str(&format!("   Target Files: {}\n", step.likely_files.join(", ")));
            }
        }

        // Include existing project structure and target file contents
        user_prompt.push_str("\n=== WORKSPACE FILE TREE ===\n");
        let mut files_scanned = Vec::new();
        if let Ok(entries) = std::fs::read_dir(worktree_path) {
            for entry in entries.flatten() {
                let p = entry.path();
                if let Ok(rel) = p.strip_prefix(worktree_path) {
                    let rel_str = rel.to_string_lossy().replace('\\', "/");
                    if !rel_str.starts_with(".git") && !rel_str.starts_with("target") && !rel_str.starts_with("node_modules") {
                        user_prompt.push_str(&format!("- {}\n", rel_str));
                        files_scanned.push(p);
                    }
                }
            }
        }

        // Include content for existing files (up to reasonable budget)
        for fpath in files_scanned {
            if let Ok(rel) = fpath.strip_prefix(worktree_path) {
                let rel_str = rel.to_string_lossy().replace('\\', "/");
                if let Ok(content) = std::fs::read_to_string(&fpath) {
                    if content.len() < 12_000 {
                        user_prompt.push_str(&format!("\n--- Existing File: {} ---\n{}\n", rel_str, content));
                    }
                }
            }
        }

        let body = json!({
            "model": self.model,
            "messages": [
                { "role": "system", "content": system_prompt },
                { "role": "user", "content": user_prompt }
            ],
            "temperature": 0.2,
            "max_tokens": 4096
        });

        let mut req = client.post(&self.endpoint_url).header("Content-Type", "application/json");

        if matches!(self.provider, WorkerProvider::OpenRouter) {
            req = req
                .header("HTTP-Referer", "https://github.com/rKyzen-Labs/ZenCode")
                .header("X-Title", "ZenCode CLI");
        }

        if !self.api_key.is_empty() && self.api_key != "ollama" {
            req = req.header("Authorization", format!("Bearer {}", self.api_key));
        }

        let resp = req.json(&body).send().await.map_err(|e| format!("Network request failed: {e}"))?;
        let status = resp.status();
        let resp_text = resp.text().await.unwrap_or_default();

        if !status.is_success() {
            return Err(format!("{:?} worker failed ({status}): {resp_text}", self.provider));
        }

        let json_val: serde_json::Value = serde_json::from_str(&resp_text)
            .map_err(|e| format!("Invalid JSON response from worker: {e}"))?;

        let choice = json_val
            .get("choices")
            .and_then(|c| c.as_array())
            .and_then(|arr| arr.first());

        let mut output_content = choice
            .and_then(|c| c.get("message"))
            .and_then(|m| m.get("content"))
            .and_then(|c| c.as_str())
            .unwrap_or_default()
            .to_string();

        if output_content.is_empty() {
            if let Some(c) = choice {
                if let Some(text) = c.get("text").and_then(|t| t.as_str()) {
                    output_content = text.to_string();
                } else if let Some(m) = c.get("message") {
                    if let Some(reasoning) = m.get("reasoning_content").or_else(|| m.get("reasoning")).and_then(|r| r.as_str()) {
                        output_content = reasoning.to_string();
                    }
                }
            }
        }

        if output_content.is_empty() {
            return Err(format!("Worker {:?} returned empty response.", self.provider));
        }

        let fallback_file = steps.first().and_then(|s| s.likely_files.first()).map(|s| s.as_str());
        let applied_diffs = zencode_core::apply_diff_blocks(worktree_path, &output_content, fallback_file)?;

        Ok(applied_diffs)
    }
}

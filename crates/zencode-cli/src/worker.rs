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

2. FILE CREATION, REWRITE & EXPANSION MANDATE:
   - For stylesheets (CSS), markup (HTML), and scripts (JS) where new sections or components are being added, overhauled, or generated: ALWAYS use complete file blocks (<<<<<<< CREATE: relative/path/to/file.ext\n[full complete content]\n>>>>>>>) or Markdown code blocks.
   - Writing full cohesive files guarantees 100% selector alignment and eliminates search block mismatches.
   - Do NOT use SEARCH/REPLACE for newly created or empty files.

3. SURGICAL DIFF MANDATE (MINIMAL VERIFIED EDITS ONLY):
   - ONLY use SEARCH/REPLACE when making targeted, 1-5 line edits to an existing, fully verified function or selector that you can see verbatim in the file outline:
FILE: relative/path/to/file.ext
<<<<<<< SEARCH
[exact existing lines from the file to replace]
=======
[replacement lines]
>>>>>>> REPLACE
   - CRITICAL: Ensure `FILE: relative/path/to/file.ext` matches the EXACT file containing the snippet. Never guess or hallucinate lines in the SEARCH block.
   - The SEARCH block must match verbatim, including comments, CSS selectors (e.g. `*, *::before, *::after`), and indentation."#;

        let mut all_applied_diffs = Vec::new();

        for (i, step) in steps.iter().enumerate() {
            let mut user_prompt = format!(
                "You are executing Step {} of {}:\nGoal: {}\n\nInstructions:\n",
                i + 1, steps.len(), step.goal
            );

            for inst in &step.instructions {
                user_prompt.push_str(&format!("- {}\n", inst));
            }
            if !step.likely_files.is_empty() {
                user_prompt.push_str(&format!("\nTarget Files: {}\n", step.likely_files.join(", ")));
            }
            if !step.acceptance_checks.is_empty() {
                user_prompt.push_str(&format!("Acceptance Checks: {}\n", step.acceptance_checks.join("; ")));
            }

            // Inject design system guidelines if relevant for UI/web tasks
            if let Some(preset) = zencode_core::detect_design_system_intent(task_summary) {
                user_prompt.push_str("\n\n");
                user_prompt.push_str(&zencode_core::format_design_system_guideline(preset));
                user_prompt.push_str("\n\n");
                user_prompt.push_str(zencode_core::anti_pattern_shield());
            }

            // Include existing project structure and target file contents from worktree
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

            let mut current_model = self.model.clone();
            let body = json!({
                "model": current_model,
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

            let mut resp = req.json(&body).send().await.map_err(|e| format!("Network request failed: {e}"))?;
            let mut status = resp.status();
            let mut resp_text = resp.text().await.unwrap_or_default();

            if status.as_u16() == 429 && matches!(self.provider, WorkerProvider::Groq) {
                // If 120b rate limited, automatically rotate to 20b
                eprintln!("\n[Groq 429 Rate Limit] Rotating to openai/gpt-oss-20b for step {}...", i + 1);
                current_model = "openai/gpt-oss-20b".to_string();
                let retry_body = json!({
                    "model": current_model,
                    "messages": [
                        { "role": "system", "content": system_prompt },
                        { "role": "user", "content": user_prompt }
                    ],
                    "temperature": 0.2,
                    "max_tokens": 4096
                });
                let retry_req = client.post(&self.endpoint_url)
                    .header("Content-Type", "application/json")
                    .header("Authorization", format!("Bearer {}", self.api_key));
                resp = retry_req.json(&retry_body).send().await.map_err(|e| format!("Retry request failed: {e}"))?;
                status = resp.status();
                resp_text = resp.text().await.unwrap_or_default();
            }

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

            let finish_reason = choice
                .and_then(|c| c.get("finish_reason"))
                .and_then(|r| r.as_str())
                .unwrap_or_default();

            if finish_reason == "length" {
                let cont_prompt = "You reached the maximum output token limit. Continue generating the exact remaining diff/file code without repeating what you already wrote:";
                let cont_body = json!({
                    "model": self.model,
                    "messages": [
                        { "role": "system", "content": system_prompt },
                        { "role": "user", "content": user_prompt },
                        { "role": "assistant", "content": output_content },
                        { "role": "user", "content": cont_prompt }
                    ],
                    "temperature": 0.2,
                    "max_tokens": 4096
                });
                let mut cont_req = client.post(&self.endpoint_url).header("Content-Type", "application/json");
                if matches!(self.provider, WorkerProvider::OpenRouter) {
                    cont_req = cont_req
                        .header("HTTP-Referer", "https://github.com/rKyzen-Labs/ZenCode")
                        .header("X-Title", "ZenCode CLI");
                }
                if !self.api_key.is_empty() && self.api_key != "ollama" {
                    cont_req = cont_req.header("Authorization", format!("Bearer {}", self.api_key));
                }
                if let Ok(cont_resp) = cont_req.json(&cont_body).send().await {
                    if cont_resp.status().is_success() {
                        if let Ok(cont_json) = cont_resp.json::<serde_json::Value>().await {
                            if let Some(chunk) = cont_json.get("choices").and_then(|c| c.as_array()).and_then(|a| a.first()).and_then(|c| c.get("message")).and_then(|m| m.get("content")).and_then(|c| c.as_str()) {
                                output_content.push('\n');
                                output_content.push_str(chunk);
                            }
                        }
                    }
                }
            }

            if output_content.is_empty() {
                return Err(format!("Worker {:?} returned empty response on step {}.", self.provider, i + 1));
            }

            let fallback_file = step.likely_files.first().map(|s| s.as_str());
            let applied_diffs = zencode_core::apply_diff_blocks(worktree_path, &output_content, fallback_file)?;
            all_applied_diffs.extend(applied_diffs);
        }

        // Autonomous Missing & Incomplete Target Recovery Pass:
        let mut missing_targets = Vec::new();
        for step in steps {
            for lf in &step.likely_files {
                let clean = lf.trim().trim_matches('`').trim_start_matches("./").replace('\\', "/");
                if clean.is_empty() { continue; }
                let target_path = worktree_path.join(&clean);
                if !target_path.exists() {
                    missing_targets.push((clean, step.goal.clone(), "missing"));
                } else if let Ok(meta) = std::fs::metadata(&target_path) {
                    if meta.len() < 80 {
                        missing_targets.push((clean, step.goal.clone(), "incomplete/truncated"));
                    }
                }
            }
        }

        for (missing_file, step_goal, reason) in missing_targets {
            eprintln!("[ZenCode CLI] Autonomous Recovery: target file '{}' was {} in worktree. Triggering dedicated recovery pass...", missing_file, reason);
            let recovery_prompt = format!(
                "CRITICAL COMPLETION MANDATE:\nThe planned target file '{}' for step goal '{}' was {}.\nThe file MUST be created with 100% complete, fully implemented production-ready code. Zero stubs, zero placeholders.\n\nOutput the entire complete file content using:\n<<<<<<< CREATE: {}\n[full complete code]\n>>>>>>>",
                missing_file, step_goal, reason, missing_file
            );

            let rec_body = json!({
                "model": self.model,
                "messages": [
                    { "role": "system", "content": system_prompt },
                    { "role": "user", "content": recovery_prompt }
                ],
                "temperature": 0.2,
                "max_tokens": 4096
            });

            let mut rec_req = client.post(&self.endpoint_url).header("Content-Type", "application/json");
            if matches!(self.provider, WorkerProvider::OpenRouter) {
                rec_req = rec_req
                    .header("HTTP-Referer", "https://github.com/rKyzen-Labs/ZenCode")
                    .header("X-Title", "ZenCode CLI");
            }
            if !self.api_key.is_empty() && self.api_key != "ollama" {
                rec_req = rec_req.header("Authorization", format!("Bearer {}", self.api_key));
            }

            if let Ok(rec_resp) = rec_req.json(&rec_body).send().await {
                if rec_resp.status().is_success() {
                    if let Ok(rec_json) = rec_resp.json::<serde_json::Value>().await {
                        let rec_content = rec_json
                            .get("choices")
                            .and_then(|c| c.as_array())
                            .and_then(|a| a.first())
                            .and_then(|c| c.get("message"))
                            .and_then(|m| m.get("content"))
                            .and_then(|c| c.as_str())
                            .unwrap_or_default();
                        if !rec_content.is_empty() {
                            if let Ok(rec_diffs) = zencode_core::apply_diff_blocks(worktree_path, rec_content, Some(&missing_file)) {
                                all_applied_diffs.extend(rec_diffs);
                            }
                        }
                    }
                }
            }
        }

        Ok(all_applied_diffs)
    }
}

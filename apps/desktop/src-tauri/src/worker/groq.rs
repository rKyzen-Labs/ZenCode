use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter};
use tokio::sync::Mutex;
use zencode_core::shred_file_context;

use crate::git::{ChangeInventory, GitService};
use crate::models::{TaskState, WorkerEventPayload};
use crate::worker::diff::apply_diff_blocks;

pub const GROQ_MODELS_ROTATION: &[&str] = &[
    "openai/gpt-oss-120b",
    "openai/gpt-oss-20b",
    "qwen/qwen3.8-27b",
];

/// Tracks sliding-window RPM, TPM, and 429 cooldowns per Groq model.
#[derive(Debug, Clone)]
pub struct GroqModelQuota {
    pub model_id: String,
    pub rpm_history: VecDeque<u64>,
    pub tpm_history: VecDeque<(u64, u32)>,
    pub cooldown_until: Option<u64>,
    pub rpm_limit: u32,
    pub tpm_limit: u32,
}

impl GroqModelQuota {
    pub fn new(model_id: &str) -> Self {
        // Standard Groq tier defaults: 30 RPM, 60,000 TPM
        Self {
            model_id: model_id.to_string(),
            rpm_history: VecDeque::new(),
            tpm_history: VecDeque::new(),
            cooldown_until: None,
            rpm_limit: 30,
            tpm_limit: 60_000,
        }
    }

    pub fn prune(&mut self, now_ms: u64) {
        let cutoff = now_ms.saturating_sub(60_000);
        while let Some(&ts) = self.rpm_history.front() {
            if ts < cutoff {
                self.rpm_history.pop_front();
            } else {
                break;
            }
        }
        while let Some(&(ts, _)) = self.tpm_history.front() {
            if ts < cutoff {
                self.tpm_history.pop_front();
            } else {
                break;
            }
        }
        if let Some(until) = self.cooldown_until {
            if now_ms >= until {
                self.cooldown_until = None;
            }
        }
    }

    pub fn is_available(&mut self, now_ms: u64) -> bool {
        self.prune(now_ms);
        if self.cooldown_until.is_some() {
            return false;
        }
        if self.rpm_history.len() >= self.rpm_limit as usize {
            return false;
        }
        let current_tpm: u32 = self.tpm_history.iter().map(|(_, t)| *t).sum();
        if current_tpm >= self.tpm_limit {
            return false;
        }
        true
    }

    pub fn record_request(&mut self, now_ms: u64) {
        self.rpm_history.push_back(now_ms);
    }

    pub fn record_tokens(&mut self, now_ms: u64, tokens: u32) {
        self.tpm_history.push_back((now_ms, tokens));
    }

    pub fn trigger_cooldown(&mut self, now_ms: u64, duration_secs: u64) {
        self.cooldown_until = Some(now_ms + (duration_secs * 1000));
    }
}

/// Global tracker for Groq model rotation and rate limits.
#[derive(Debug, Clone)]
pub struct GroqQuotaManager {
    pub models: Arc<Mutex<HashMap<String, GroqModelQuota>>>,
}

impl GroqQuotaManager {
    pub fn new() -> Self {
        let mut map = HashMap::new();
        for &m in GROQ_MODELS_ROTATION {
            map.insert(m.to_string(), GroqModelQuota::new(m));
        }
        Self {
            models: Arc::new(Mutex::new(map)),
        }
    }

    /// Selects the first available model in priority order starting from `preferred_model`.
    pub async fn select_available_model(&self, preferred_model: Option<&str>) -> String {
        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        let mut lock = self.models.lock().await;

        // Try preferred model first if valid
        if let Some(pref) = preferred_model {
            if let Some(tracker) = lock.get_mut(pref) {
                if tracker.is_available(now_ms) {
                    return pref.to_string();
                }
            }
        }

        // Rotate through GROQ_MODELS_ROTATION
        for &m in GROQ_MODELS_ROTATION {
            if let Some(tracker) = lock.get_mut(m) {
                if tracker.is_available(now_ms) {
                    return m.to_string();
                }
            }
        }

        // If all are cooling down, return the primary as fallback
        GROQ_MODELS_ROTATION[0].to_string()
    }

    pub async fn record_429_cooldown(&self, model: &str) {
        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        let mut lock = self.models.lock().await;
        if let Some(tracker) = lock.get_mut(model) {
            tracker.trigger_cooldown(now_ms, 60); // 60s cooldown
        }
    }

    pub async fn record_usage(&self, model: &str, tokens: u32) {
        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        let mut lock = self.models.lock().await;
        if let Some(tracker) = lock.get_mut(model) {
            tracker.record_request(now_ms);
            tracker.record_tokens(now_ms, tokens);
        }
    }
}

pub struct GroqWorkerClient {
    pub api_key: String,
    pub quota_tracker: Arc<Mutex<crate::planner::quota::ModelQuotaTracker>>,
}

impl GroqWorkerClient {
    pub fn new(api_key: String, quota_tracker: Arc<Mutex<crate::planner::quota::ModelQuotaTracker>>) -> Self {
        Self {
            api_key,
            quota_tracker,
        }
    }

    /// Executes the plan step-by-step in the isolated task worktree using Groq cloud models.
    pub async fn execute_session(
        &self,
        worktree_path: &Path,
        _task_id: &str,
        session_id: &str,
        plan_summary: &str,
        steps: &[zencode_core::PlanStep],
        preferred_model: Option<&str>,
        app_handle: AppHandle,
    ) -> Result<ChangeInventory, String> {
        // Pre-flight check: ensure Groq capacity is not completely exhausted
        {
            let tracker = self.quota_tracker.lock().await;
            if tracker.are_all_groq_models_exhausted() {
                return Err("All Groq models are exhausted for today (0 requests remaining or rate-limited). Please switch back to the Local (Ollama) worker in the dock or Settings.".to_string());
            }
        }

        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(90))
            .build()
            .map_err(|e| e.to_string())?;

        // 1. Emit initial worker:status
        let _ = app_handle.emit(
            "worker:status",
            &TaskState::WorkerRunning {
                session_id: session_id.to_string(),
            },
        );

        // Emit initial session event
        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        let _ = app_handle.emit(
            "worker:message",
            &WorkerEventPayload {
                session_id: session_id.to_string(),
                event_type: "session.status".into(),
                content: serde_json::json!({ "type": "session.status", "status": "running" }).to_string(),
                timestamp_ms: now_ms,
            },
        );

        // 2. Build Context Shredded Workspace Outline
        let mut shredded_files_summary = String::new();
        let mut likely_files_set = std::collections::HashSet::new();
        for step in steps {
            for lf in &step.likely_files {
                likely_files_set.insert(lf.clone());
            }
        }

        // Walk worktree files and collect structural summaries
        let mut files_to_scan = Vec::new();
        if let Ok(entries) = std::fs::read_dir(worktree_path) {
            collect_worktree_files(worktree_path, entries, &mut files_to_scan, 0);
        }

        shredded_files_summary.push_str("=== WORKSPACE STRUCTURAL SUMMARY (AST-SHREDDED) ===\n");
        for fpath in &files_to_scan {
            let rel_path = fpath.strip_prefix(worktree_path).unwrap_or(fpath);
            let rel_str = rel_path.to_string_lossy().replace('\\', "/");

            if likely_files_set.contains(&rel_str) {
                // For target implementation file: send full content
                if let Ok(full_code) = std::fs::read_to_string(fpath) {
                    shredded_files_summary.push_str(&format!(
                        "\n--- Target File (Full Content): {rel_str} ---\n{full_code}\n"
                    ));
                    // Emit tool read activity gradually to frontend
                    let _ = app_handle.emit(
                        "worker:message",
                        &WorkerEventPayload {
                            session_id: session_id.to_string(),
                            event_type: "message.part.updated".into(),
                            content: serde_json::json!({
                                "type": "message.part.updated",
                                "properties": {
                                    "part": {
                                        "type": "tool",
                                        "tool": "read",
                                        "callID": format!("read_{}", rel_str),
                                        "state": { "input": { "filePath": rel_str } }
                                    }
                                }
                            }).to_string(),
                            timestamp_ms: SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis() as u64,
                        },
                    );
                    tokio::time::sleep(Duration::from_millis(60)).await;
                }
            } else {
                // For other context files: shred function bodies
                if let Ok(code) = std::fs::read_to_string(fpath) {
                    let outline = shred_file_context(fpath, &code);
                    shredded_files_summary.push_str(&format!(
                        "\n--- Context Outline: {rel_str} ---\n{outline}\n"
                    ));
                }
            }
        }

        // 3. System Prompt Enforcing SEARCH/REPLACE Diff-Only Output
        let system_prompt = r#"You are an expert autonomous software engineer executing code changes in an isolated Git repository.
Rules:
1. ONLY produce targeted, surgical SEARCH/REPLACE diff blocks. DO NOT output entire rewritten files.
2. For modifying existing files, use this exact syntax:
FILE: relative/path/to/file.ext
<<<<<<< SEARCH
[exact existing lines from the file to replace]
=======
[replacement lines]
>>>>>>> REPLACE

3. For creating a new file, use:
<<<<<<< CREATE: relative/path/to/new_file.ext
[file content]
>>>>>>>

4. Keep changes focused and minimal to conserve token budget.
5. Provide a brief explanation before the diff blocks."#;

        // Build User Prompt
        let mut user_prompt = format!(
            "Execute the following implementation plan:\n\nTask: {plan_summary}\n\nSteps:\n"
        );
        for (i, step) in steps.iter().enumerate() {
            user_prompt.push_str(&format!(
                "{}. Goal: {}\n   Instructions: {:?}\n   Target Files: {:?}\n",
                i + 1, step.goal, step.instructions, step.likely_files
            ));
        }
        user_prompt.push_str(&format!("\n\n{shredded_files_summary}"));

        // 4. Model Rotation Loop (gpt-oss-120b -> gpt-oss-20b -> qwen3.8-27b)
        let mut attempts = 0;
        let mut chosen_model = {
            let mut tracker = self.quota_tracker.lock().await;
            tracker.select_groq_model(preferred_model)?
        };
        let mut response_text = String::new();

        while attempts < 3 {
            attempts += 1;

            // Emit reasoning thought
            let _ = app_handle.emit(
                "worker:message",
                &WorkerEventPayload {
                    session_id: session_id.to_string(),
                    event_type: "message.part.updated".into(),
                    content: serde_json::json!({
                        "type": "message.part.updated",
                        "properties": {
                            "part": {
                                "type": "reasoning",
                                "text": format!("Sending prompt to Groq model: {chosen_model}")
                            }
                        }
                    }).to_string(),
                    timestamp_ms: SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis() as u64,
                },
            );

            let body = serde_json::json!({
                "model": chosen_model,
                "messages": [
                    { "role": "system", "content": system_prompt },
                    { "role": "user", "content": user_prompt }
                ],
                "temperature": 0.2,
                "max_tokens": 4096
            });

            let req_resp = client
                .post("https://api.groq.com/openai/v1/chat/completions")
                .header("Authorization", format!("Bearer {}", self.api_key))
                .header("Content-Type", "application/json")
                .json(&body)
                .send()
                .await;

            match req_resp {
                Ok(resp) => {
                    let status = resp.status();
                    // Extract real ground-truth rate-limit headers from Groq response
                    let groq_headers = crate::planner::quota::parse_groq_headers(resp.headers());
                    {
                        let mut tracker = self.quota_tracker.lock().await;
                        tracker.update_groq_server_headers(&chosen_model, &groq_headers);
                        let event = tracker.to_quota_update_event(&chosen_model);
                        let _ = app_handle.emit("planner:quota_update", &event);
                    }

                    if status.is_success() {
                        let json: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
                        let tokens = json
                            .get("usage")
                            .and_then(|u| u.get("total_tokens"))
                            .and_then(|t| t.as_u64())
                            .unwrap_or(1000) as u32;

                        {
                            let mut tracker = self.quota_tracker.lock().await;
                            tracker.record_actual_usage(&chosen_model, tokens);
                            let event = tracker.to_quota_update_event(&chosen_model);
                            let _ = app_handle.emit("planner:quota_update", &event);
                        }

                        response_text = json
                            .get("choices")
                            .and_then(|c| c.as_array())
                            .and_then(|arr| arr.first())
                            .and_then(|c| c.get("message"))
                            .and_then(|m| m.get("content"))
                            .and_then(|c| c.as_str())
                            .unwrap_or_default()
                            .to_string();
                        break;
                    } else if status.as_u16() == 429 {
                        // Rate limit hit: ModelQuotaTracker already set cooldown from retry-after header
                        let next_model = {
                            let mut tracker = self.quota_tracker.lock().await;
                            tracker.select_groq_model(None).map_err(|e| format!("Groq quota exhausted: {e}"))?
                        };
                        println!("Groq 429 on {chosen_model}. Rotating to {next_model}...");
                        chosen_model = next_model;
                        continue;
                    } else {
                        let err_body = resp.text().await.unwrap_or_default();
                        return Err(format!("Groq API request failed ({status}): {err_body}"));
                    }
                }
                Err(e) => {
                    return Err(format!("Network failure calling Groq API: {e}"));
                }
            }
        }

        if response_text.is_empty() {
            return Err("Groq worker failed to generate a response across rotated models.".to_string());
        }

        // 5. Parse and Apply Diff Blocks to Worktree
        let fallback_file = steps.first().and_then(|s| s.likely_files.first()).map(|s| s.as_str());
        let applied_diffs = apply_diff_blocks(worktree_path, &response_text, fallback_file)?;

        // Emit edit activity events for UI matching Image 5
        for diff in &applied_diffs {
            let _ = app_handle.emit(
                "worker:message",
                &WorkerEventPayload {
                    session_id: session_id.to_string(),
                    event_type: "message.part.updated".into(),
                    content: serde_json::json!({
                        "type": "message.part.updated",
                        "properties": {
                            "part": {
                                "type": "tool",
                                "tool": "edit",
                                "callID": format!("edit_{}", diff.file_path),
                                "state": {
                                    "input": { "filePath": diff.file_path },
                                    "additions": diff.additions,
                                    "deletions": diff.deletions
                                }
                            }
                        }
                    }).to_string(),
                    timestamp_ms: SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis() as u64,
                },
            );
            tokio::time::sleep(Duration::from_millis(80)).await;
        }

        // 6. Compute Git change inventory in worktree
        let inventory = GitService::get_worktree_diff(worktree_path)
            .unwrap_or_else(|_| ChangeInventory {
                worktree_path: worktree_path.to_string_lossy().to_string(),
                branch_name: String::new(),
                base_commit: String::new(),
                files: Vec::new(),
                total_additions: 0,
                total_deletions: 0,
                modified_count: 0,
                added_count: 0,
                untracked_count: 0,
                deleted_count: 0,
                renamed_count: 0,
                binary_count: 0,
            });

        // Emit inventory update
        let _ = app_handle.emit("inventory:update", &inventory);

        // Emit session complete
        let _ = app_handle.emit("worker:status", &TaskState::AwaitingReview);

        Ok(inventory)
    }
}

fn collect_worktree_files(
    root: &Path,
    entries: std::fs::ReadDir,
    acc: &mut Vec<PathBuf>,
    depth: usize,
) {
    if depth > 5 {
        return;
    }
    for entry in entries.flatten() {
        let path = entry.path();
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if name.starts_with('.') || name == "target" || name == "node_modules" {
            continue;
        }
        if path.is_dir() {
            if let Ok(sub) = std::fs::read_dir(&path) {
                collect_worktree_files(root, sub, acc, depth + 1);
            }
        } else if path.is_file() {
            acc.push(path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_groq_quota_rotation_cascade() {
        let quota = GroqQuotaManager::new();

        // Initially, the first model in rotation is chosen
        let m1 = quota.select_available_model(None).await;
        assert_eq!(m1, GROQ_MODELS_ROTATION[0]); // openai/gpt-oss-120b

        // Rate limit the primary model
        quota.record_429_cooldown(GROQ_MODELS_ROTATION[0]).await;
        let m2 = quota.select_available_model(None).await;
        assert_eq!(m2, GROQ_MODELS_ROTATION[1]); // openai/gpt-oss-20b

        // Rate limit secondary model
        quota.record_429_cooldown(GROQ_MODELS_ROTATION[1]).await;
        let m3 = quota.select_available_model(None).await;
        assert_eq!(m3, GROQ_MODELS_ROTATION[2]); // qwen/qwen3.8-27b

        // Preferred model respects rate limits
        let preferred = quota.select_available_model(Some(GROQ_MODELS_ROTATION[0])).await;
        assert_eq!(preferred, GROQ_MODELS_ROTATION[2]); // preferred is in cooldown, stays on tertiary
    }
}

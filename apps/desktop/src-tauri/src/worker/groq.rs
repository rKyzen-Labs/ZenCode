use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter};
use tokio::sync::Mutex;
use zencode_core::shred_file_context;

use crate::exec::service::ExecutionService;
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
        execution_service: Arc<ExecutionService>,
        app_handle: AppHandle,
    ) -> Result<(ChangeInventory, Vec<String>), String> {
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
                let clean = lf.trim().trim_matches('`').trim_start_matches("./").replace('\\', "/");
                for part in clean.split(',') {
                    let p = part.trim().to_lowercase();
                    if !p.is_empty() {
                        likely_files_set.insert(p);
                    }
                }
            }
        }

        // Walk worktree files and collect structural summaries
        let mut files_to_scan = Vec::new();
        if let Ok(entries) = std::fs::read_dir(worktree_path) {
            collect_worktree_files(worktree_path, entries, &mut files_to_scan, 0);
        }

        shredded_files_summary.push_str("=== WORKSPACE STRUCTURAL SUMMARY ===\n");
        for fpath in &files_to_scan {
            let rel_path = fpath.strip_prefix(worktree_path).unwrap_or(fpath);
            let rel_str = rel_path.to_string_lossy().replace('\\', "/");
            let rel_str_lower = rel_str.to_lowercase();
            let file_base = Path::new(&rel_str_lower)
                .file_name()
                .and_then(|f| f.to_str())
                .unwrap_or(&rel_str_lower);

            let is_target = likely_files_set.contains(&rel_str_lower)
                || likely_files_set.contains(file_base)
                || likely_files_set.iter().any(|lf| lf == file_base || rel_str_lower.ends_with(lf) || lf.ends_with(file_base));

            // Small source/markup files (< 40KB) receive full content so the model generates surgical diffs without SEARCH mismatches
            let file_len = std::fs::metadata(fpath).map(|m| m.len()).unwrap_or(0);
            let is_small_project_file = file_len < 40_000 && (
                rel_str_lower.ends_with(".html") || rel_str_lower.ends_with(".css") || rel_str_lower.ends_with(".js") ||
                rel_str_lower.ends_with(".ts") || rel_str_lower.ends_with(".py") || rel_str_lower.ends_with(".json") ||
                rel_str_lower.ends_with(".rs")
            );

            if is_target || is_small_project_file {
                // For target and project implementation files: send full content
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

        // 3. System Prompt Supporting SEARCH/REPLACE Diffs, Files/Folders Operations, and Terminal Commands
        let system_prompt = r#"You are an expert autonomous software engineer executing code changes, directory operations, and terminal commands in an isolated Git repository.
Rules:
1. For modifying existing files, use targeted, surgical SEARCH/REPLACE diff blocks:
FILE: relative/path/to/file.ext
<<<<<<< SEARCH
[exact existing lines from the file to replace]
=======
[replacement lines]
>>>>>>> REPLACE

2. For creating a new file or adding a file into a folder:
<<<<<<< CREATE: relative/path/to/new_file.ext
[file content]
>>>>>>>

3. For deleting a file:
<<<<<<< DELETE: relative/path/to/file_to_delete.ext >>>>>>>
or:
DELETE: relative/path/to/file_to_delete.ext

4. For making a new folder or directory:
<<<<<<< MKDIR: relative/path/to/folder >>>>>>>
or:
MKDIR: relative/path/to/folder

5. For moving or renaming a file or folder (e.g. moving files out of/into folders or editing folders):
<<<<<<< MOVE: old/path/to/file.ext -> new/path/to/file.ext >>>>>>>
or:
MOVE: old/path/to/file.ext -> new/path/to/file.ext

6. For deleting a folder:
<<<<<<< RMDIR: relative/path/to/folder >>>>>>>
or:
RMDIR: relative/path/to/folder

7. For executing terminal commands (such as installing dependencies, running build/tests, ollama commands, or running scripts):
<<<<<<< COMMAND
pip install requests
>>>>>>>
or:
COMMAND: pip install requests

8. Keep changes focused and minimal to conserve token budget.
9. Provide a brief explanation before the diff blocks."#;

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

        // 6. Extract and Execute Terminal Commands (Dependencies, Builds, Tests)
        let commands_to_run = extract_commands_for_session(&response_text, steps);
        let mut executed_commands = Vec::new();

        for cmd_line in commands_to_run {
            if let Some((prog, args)) = tokenize_command(&cmd_line) {
                let arg_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
                let desc = format!("Run {} {}", prog, args.join(" "));
                let req = execution_service
                    .create_approval_request(&prog, &arg_refs, worktree_path, &desc, Some(session_id))
                    .await;

                if req.auto_approved {
                    // Safe auto-approved: run immediately with line-by-line output streaming
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
                                        "tool": "terminal",
                                        "callID": format!("exec_{}", req.id),
                                        "state": {
                                            "input": { "command": format!("{} {}", req.program, req.args.join(" ")) }
                                        }
                                    }
                                }
                            }).to_string(),
                            timestamp_ms: SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis() as u64,
                        },
                    );

                    match execution_service.execute_command(&req.id, Some(&app_handle), None).await {
                        Ok(res) => {
                            let exit_str = match res.exit_code {
                                Some(0) => "exit code 0 (success)".to_string(),
                                Some(c) => format!("exit code {c}"),
                                None => "completed".to_string(),
                            };
                            executed_commands.push(format!("$ {} {} ({})", req.program, req.args.join(" "), exit_str));
                        }
                        Err(e) => {
                            executed_commands.push(format!("$ {} {} (failed: {})", req.program, req.args.join(" "), e));
                        }
                    }
                } else {
                    // Requires user approval: emit permissions:approval_required for in-chat popup
                    let _ = app_handle.emit("permissions:approval_required", &req);
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
                                        "text": format!("Awaiting user approval to run command: {} {}", req.program, req.args.join(" "))
                                    }
                                }
                            }).to_string(),
                            timestamp_ms: SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis() as u64,
                        },
                    );

                    match execution_service.wait_for_command_result(&req.id, 180).await {
                        Ok(res) => {
                            let exit_str = match res.exit_code {
                                Some(0) => "exit code 0 (success)".to_string(),
                                Some(c) => format!("exit code {c}"),
                                None => "completed".to_string(),
                            };
                            executed_commands.push(format!("$ {} {} ({})", req.program, req.args.join(" "), exit_str));
                        }
                        Err(e) => {
                            executed_commands.push(format!("$ {} {} (skipped: {})", req.program, req.args.join(" "), e));
                        }
                    }
                }
            }
        }

        // 7. Compute Git change inventory in worktree
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

        Ok((inventory, executed_commands))
    }
}

/// Tokenizes a shell command string into program and arguments, respecting quotes.
pub fn tokenize_command(cmd_str: &str) -> Option<(String, Vec<String>)> {
    let trimmed = cmd_str.trim().trim_start_matches('$').trim();
    if trimmed.is_empty() {
        return None;
    }

    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut in_quote: Option<char> = None;

    for c in trimmed.chars() {
        if let Some(q) = in_quote {
            if c == q {
                in_quote = None;
            } else {
                current.push(c);
            }
        } else if c == '"' || c == '\'' {
            in_quote = Some(c);
        } else if c.is_whitespace() {
            if !current.is_empty() {
                tokens.push(current.clone());
                current.clear();
            }
        } else {
            current.push(c);
        }
    }
    if !current.is_empty() {
        tokens.push(current);
    }

    if tokens.is_empty() {
        return None;
    }

    let program = tokens.remove(0);
    Some((program, tokens))
}

/// Extracts commands from model response text (COMMAND blocks or COMMAND: lines)
/// and also checks plan steps for explicit command requirements.
pub fn extract_commands_for_session(
    response_text: &str,
    steps: &[zencode_core::PlanStep],
) -> Vec<String> {
    let mut commands = Vec::new();
    let mut seen = std::collections::HashSet::new();

    // 1. Extract from model response text
    let lines: Vec<&str> = response_text.lines().collect();
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i].trim();

        // Check for <<<<<<< COMMAND block
        if line.starts_with("<<<<<<< COMMAND") {
            i += 1;
            let mut cmd_lines = Vec::new();
            while i < lines.len() && !lines[i].trim().starts_with(">>>>>>>") {
                let cl = lines[i].trim();
                if !cl.is_empty() {
                    cmd_lines.push(cl);
                }
                i += 1;
            }
            if i < lines.len() && lines[i].trim().starts_with(">>>>>>>") {
                i += 1;
            }
            for cmd in cmd_lines {
                let clean_cmd = cmd.trim_start_matches('$').trim().to_string();
                if !clean_cmd.is_empty() && seen.insert(clean_cmd.clone()) {
                    commands.push(clean_cmd);
                }
            }
            continue;
        }

        // Check for COMMAND: line
        if let Some(rest) = line.strip_prefix("COMMAND:").or_else(|| line.strip_prefix("Command:")) {
            let clean_cmd = rest.trim().trim_matches('`').trim_start_matches('$').trim().to_string();
            if !clean_cmd.is_empty() && seen.insert(clean_cmd.clone()) {
                commands.push(clean_cmd);
            }
            i += 1;
            continue;
        }

        i += 1;
    }

    // 2. Extract from plan steps if not already covered
    let known_binaries = [
        "pip", "pip3", "npm", "npx", "pnpm", "yarn", "cargo", "pytest",
        "python", "python3", "node", "make", "poetry", "uv"
    ];

    for step in steps {
        let candidates: Vec<&str> = step.instructions.iter().map(|s| s.as_str())
            .chain(std::iter::once(step.goal.as_str()))
            .collect();

        for candidate in candidates {
            let trimmed = candidate.trim();
            for bin in &known_binaries {
                // Check if candidate starts with binary or contains "$ <bin>" or "run <bin>"
                let match_idx = if trimmed.starts_with(bin) {
                    Some(0)
                } else if let Some(pos) = trimmed.find(&format!("$ {bin}")) {
                    Some(pos + 2)
                } else if let Some(pos) = trimmed.to_lowercase().find(&format!("run {bin}")) {
                    Some(pos + 4)
                } else if let Some(pos) = trimmed.to_lowercase().find(&format!("execute {bin}")) {
                    Some(pos + 8)
                } else {
                    None
                };

                if let Some(start) = match_idx {
                    let cmd_slice = trimmed[start..].trim();
                    let end_pos = cmd_slice.find('\n').or_else(|| cmd_slice.find(';')).unwrap_or(cmd_slice.len());
                    let mut cmd = cmd_slice[..end_pos].trim().trim_matches('`').trim_matches('"').trim_matches('\'').to_string();
                    if cmd.ends_with('.') {
                        cmd.pop();
                    }

                    // Clean natural language suffixes
                    let trailing_patterns = [
                        " in terminal", " in console", " for me", " in the background",
                        " in this project", " in the workspace", " please"
                    ];
                    for tp in &trailing_patterns {
                        if let Some(pos) = cmd.to_lowercase().find(tp) {
                            cmd = cmd[..pos].trim().to_string();
                        }
                    }

                    let clean = cmd.trim().to_string();
                    if !clean.is_empty() && seen.insert(clean.clone()) {
                        commands.push(clean);
                    }
                    break;
                }
            }
        }
    }

    commands
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

    #[test]
    fn test_tokenize_command_basic_and_quotes() {
        let (prog, args) = tokenize_command("pip install requests").unwrap();
        assert_eq!(prog, "pip");
        assert_eq!(args, vec!["install", "requests"]);

        let (prog2, args2) = tokenize_command("$ cargo test --lib -- \"custom filter\"").unwrap();
        assert_eq!(prog2, "cargo");
        assert_eq!(args2, vec!["test", "--lib", "--", "custom filter"]);

        assert!(tokenize_command("").is_none());
        assert!(tokenize_command("   ").is_none());
    }

    #[test]
    fn test_extract_commands_for_session_from_diff_and_steps() {
        let response_text = r#"
Here is what we will do:
<<<<<<< COMMAND
pip install requests
>>>>>>>

And also:
COMMAND: pytest tests/test_calc.py
"#;

        let steps = vec![
            zencode_core::PlanStep {
                id: "step-1".into(),
                goal: "Install dependencies".into(),
                likely_files: vec![],
                instructions: vec!["Run pip install -r requirements.txt in terminal".into()],
                depends_on: vec![],
                acceptance_checks: vec![],
            },
        ];

        let cmds = extract_commands_for_session(response_text, &steps);
        assert!(cmds.contains(&"pip install requests".to_string()));
        assert!(cmds.contains(&"pytest tests/test_calc.py".to_string()));
        assert!(cmds.contains(&"pip install -r requirements.txt".to_string()));
    }
}

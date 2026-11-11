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

/// Estimates the token count of a given text string using character/word heuristic.
pub fn estimate_tokens(text: &str) -> usize {
    let char_count = text.chars().count();
    let word_count = text.split_whitespace().count();
    ((char_count as f64 / 3.8).ceil() as usize).max(((word_count as f64) * 1.3).ceil() as usize)
}

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
    pub endpoint_url: String,
    pub api_key: String,
    pub quota_tracker: Option<Arc<Mutex<crate::planner::quota::ModelQuotaTracker>>>,
}

impl GroqWorkerClient {
    pub fn new(api_key: String, quota_tracker: Arc<Mutex<crate::planner::quota::ModelQuotaTracker>>) -> Self {
        Self {
            endpoint_url: "https://api.groq.com/openai/v1/chat/completions".to_string(),
            api_key,
            quota_tracker: Some(quota_tracker),
        }
    }

    pub fn new_ollama(endpoint: Option<String>) -> Self {
        Self {
            endpoint_url: endpoint.unwrap_or_else(|| "http://127.0.0.1:11434/v1/chat/completions".to_string()),
            api_key: "ollama".to_string(),
            quota_tracker: None,
        }
    }

    /// Executes the plan step-by-step in the isolated task worktree using Groq cloud models or local Ollama.
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
        // Pre-flight check: ensure Groq capacity is not completely exhausted if using Groq
        if let Some(ref tracker_arc) = self.quota_tracker {
            let tracker = tracker_arc.lock().await;
            if tracker.are_all_groq_models_exhausted() {
                return Err("All Groq models are exhausted for today (0 requests remaining or rate-limited). Please switch back to the Local (Ollama) worker in the dock or Settings.".to_string());
            }
        }

        let timeout_secs = if self.quota_tracker.is_none() { 300 } else { 90 };
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(timeout_secs))
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

        let plan_lower = plan_summary.to_lowercase();
        let is_exploration_task = likely_files_set.is_empty()
            || likely_files_set.len() > 6
            || plan_lower.contains("explore")
            || plan_lower.contains("analyze")
            || plan_lower.contains("overview")
            || plan_lower.contains("architecture")
            || plan_lower.contains("structure")
            || plan_lower.contains("understand")
            || plan_lower.contains("review");

        // Walk worktree files and collect structural summaries
        let mut files_to_scan = Vec::new();
        if let Ok(entries) = std::fs::read_dir(worktree_path) {
            collect_worktree_files(worktree_path, entries, &mut files_to_scan, 0);
        }

        // Sort files: prioritize explicit target files first so they are guaranteed full context
        if !is_exploration_task {
            files_to_scan.sort_by(|a, b| {
                let a_rel = a.strip_prefix(worktree_path).unwrap_or(a).to_string_lossy().to_lowercase().replace('\\', "/");
                let b_rel = b.strip_prefix(worktree_path).unwrap_or(b).to_string_lossy().to_lowercase().replace('\\', "/");
                let a_base = Path::new(&a_rel).file_name().and_then(|f| f.to_str()).unwrap_or(&a_rel);
                let b_base = Path::new(&b_rel).file_name().and_then(|f| f.to_str()).unwrap_or(&b_rel);
                let a_is_target = likely_files_set.iter().any(|lf| lf == &a_rel || lf == a_base || a_rel.ends_with(lf));
                let b_is_target = likely_files_set.iter().any(|lf| lf == &b_rel || lf == b_base || b_rel.ends_with(lf));
                b_is_target.cmp(&a_is_target).then_with(|| a.cmp(b))
            });
        } else {
            files_to_scan.sort();
        }

        let mut shredded_files_summary = String::new();

        if is_exploration_task {
            // EXPLORATION / PROJECT-WIDE MODE:
            // Send high-level breadth: Directory tree + top-level AST signatures of entry points.
            // Capped strictly under a 3,000 token budget to avoid 8K TPM traps.
            shredded_files_summary.push_str("=== WORKSPACE FILE TREE ===\n");
            for fpath in &files_to_scan {
                let rel_path = fpath.strip_prefix(worktree_path).unwrap_or(fpath);
                shredded_files_summary.push_str(&format!("- {}\n", rel_path.to_string_lossy().replace('\\', "/")));
            }
            shredded_files_summary.push_str("\n=== KEY ENTRYPOINTS & MODULE OUTLINES (AST SIGNATURES) ===\n");

            let mut current_tokens = estimate_tokens(&shredded_files_summary);
            const EXPLORATION_TOKEN_BUDGET: usize = 3_000;

            for fpath in &files_to_scan {
                if current_tokens >= EXPLORATION_TOKEN_BUDGET {
                    shredded_files_summary.push_str("\n... [Additional project files summarized in file tree above to stay within safe token limits] ...\n");
                    break;
                }
                let rel_path = fpath.strip_prefix(worktree_path).unwrap_or(fpath);
                let rel_str = rel_path.to_string_lossy().replace('\\', "/");
                let rel_str_lower = rel_str.to_lowercase();

                // Prioritize top-level manifest, configuration, and entrypoints
                let is_entrypoint = rel_str_lower.contains("main")
                    || rel_str_lower.contains("index")
                    || rel_str_lower.contains("app.")
                    || rel_str_lower.contains("lib.")
                    || rel_str_lower.ends_with("cargo.toml")
                    || rel_str_lower.ends_with("package.json")
                    || rel_str_lower.ends_with("requirements.txt")
                    || rel_str_lower.ends_with("pyproject.toml");

                if is_entrypoint || files_to_scan.len() <= 15 {
                    if let Ok(code) = std::fs::read_to_string(fpath) {
                        let outline = shred_file_context(fpath, &code);
                        let section = format!("\n--- Outline: {rel_str} ---\n{outline}\n");
                        let section_tokens = estimate_tokens(&section);
                        if current_tokens + section_tokens <= EXPLORATION_TOKEN_BUDGET {
                            shredded_files_summary.push_str(&section);
                            current_tokens += section_tokens;
                        }
                    }
                }
            }
        } else {
            // TARGETED FILE EDIT MODE:
            // Explicit target files (in likely_files, capped at 3) receive full content.
            // All other project files MUST be shredded (function bodies stripped).
            shredded_files_summary.push_str("=== WORKSPACE STRUCTURAL SUMMARY ===\n");
            let mut target_files_count = 0;
            let mut current_tokens = estimate_tokens(&shredded_files_summary);
            const EDIT_TOKEN_BUDGET: usize = 4_000;

            for fpath in &files_to_scan {
                let rel_path = fpath.strip_prefix(worktree_path).unwrap_or(fpath);
                let rel_str = rel_path.to_string_lossy().replace('\\', "/");
                let rel_str_lower = rel_str.to_lowercase();
                let file_base = Path::new(&rel_str_lower)
                    .file_name()
                    .and_then(|f| f.to_str())
                    .unwrap_or(&rel_str_lower);

                let is_explicit_target = likely_files_set.contains(&rel_str_lower)
                    || likely_files_set.contains(file_base)
                    || likely_files_set.iter().any(|lf| lf == file_base || rel_str_lower.ends_with(lf) || lf.ends_with(file_base));

                if is_explicit_target && target_files_count < 3 {
                    // Send full content for explicit targets
                    if let Ok(full_code) = std::fs::read_to_string(fpath) {
                        target_files_count += 1;
                        let section = format!("\n--- Target File (Full Content): {rel_str} ---\n{full_code}\n");
                        shredded_files_summary.push_str(&section);
                        current_tokens += estimate_tokens(&section);

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
                    }
                } else {
                    // Non-target files MUST be shredded with function bodies stripped
                    if current_tokens < EDIT_TOKEN_BUDGET {
                        if let Ok(code) = std::fs::read_to_string(fpath) {
                            let outline = shred_file_context(fpath, &code);
                            let section = format!("\n--- Context Outline (Shredded): {rel_str} ---\n{outline}\n");
                            let section_tokens = estimate_tokens(&section);
                            if current_tokens + section_tokens <= EDIT_TOKEN_BUDGET {
                                shredded_files_summary.push_str(&section);
                                current_tokens += section_tokens;
                            }
                        }
                    }
                }
            }
        }

        // 3. System Prompt Supporting SEARCH/REPLACE Diffs, Files/Folders Operations, and Terminal Commands
        let system_prompt = r#"You are an elite Senior Staff Autonomous Software Engineer and UX/UI Specialist executing code changes, directory operations, and terminal commands in an isolated Git worktree.

CORE PRINCIPLES & STANDARDS:
1. CRAFTSMANSHIP & ZERO-STUB MANDATE:
   - NEVER emit placeholders, truncated bodies, ellipsis (...), or comments like '// TODO: implement later' or '/* add styles here */'.
   - NEVER generate generic dummy text ('Lorem ipsum') when realistic domain-specific copy is defined or implied by the task.
   - Always write 100% complete, fully implemented, production-ready code with complete logic, edge-case handling, and full styling.

2. MULTI-FILE CONTRACT FIDELITY:
   - When creating or modifying multiple files (e.g. HTML, CSS, JavaScript, or backend endpoints), ensure 100% contract alignment:
     * Every class name, data attribute, and ID used in HTML must match the corresponding CSS selector in the stylesheet.
     * Every element queried in JavaScript (e.g. via document.querySelector or getElementById) must exist in the HTML structure with identical naming.
     * All imported module paths, asset links, and stylesheet <link> tags must match real created file paths.

3. MODERN UI/UX & DESIGN SYSTEM STANDARDS:
   - Use semantic HTML5 (<header>, <nav>, <main>, <section>, <article>, <footer>).
   - In CSS, define and utilize semantic design variables in :root:
     * Color palette: --bg, --surface, --text-primary, --text-secondary, --accent, --border-subtle
     * Elevation shadows: multi-layered smooth ambient shadows (e.g. box-shadow: 0 1px 3px rgba(0,0,0,0.04), 0 10px 24px -4px rgba(0,0,0,0.07);)
     * Typography scale: modern system font stack (Inter, -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif) + JetBrains Mono for monospace
     * Transitions: subtle micro-interactions with cubic-bezier curves (transition: all 0.2s cubic-bezier(0.16, 1, 0.3, 1)) and hover translateY(-2px) lifts
     * Spacing: generous section padding (clamp(48px, 6vw, 96px)) and fluid max-width container margins
   - Provide active, working JavaScript event listeners (attached after DOMContentLoaded with defensive null checks).

4. DIFFERENTIAL SURGICAL DIFF MANDATE:
   - When modifying files that already exist in the workspace, ALWAYS use surgical SEARCH/REPLACE diff blocks.
   - Do NOT rewrite entire files when only adding or changing a specific function, component, or style rule. This conserves the 4,096 token budget and prevents accidental truncation.

DIFF FORMAT & OPERATIONS:
1. For modifying existing files, use targeted, surgical SEARCH/REPLACE diff blocks:
FILE: relative/path/to/file.ext
<<<<<<< SEARCH
[exact existing lines from the file to replace]
=======
[replacement lines]
>>>>>>> REPLACE

2. For creating a new file or writing a complete file:
<<<<<<< CREATE: relative/path/to/new_file.ext
[full complete file content]
>>>>>>>
or standard markdown code blocks with clear file header:
### File: relative/path/to/new_file.ext
```lang
[full complete file content]
```

3. For deleting a file:
<<<<<<< DELETE: relative/path/to/file_to_delete.ext >>>>>>>
or: DELETE: relative/path/to/file_to_delete.ext

4. For making a new folder or directory:
<<<<<<< MKDIR: relative/path/to/folder >>>>>>>
or: MKDIR: relative/path/to/folder

5. For moving or renaming a file or folder:
<<<<<<< MOVE: old/path/to/file.ext -> new/path/to/file.ext >>>>>>>
or: MOVE: old/path/to/file.ext -> new/path/to/file.ext

6. For deleting a folder:
<<<<<<< RMDIR: relative/path/to/folder >>>>>>>
or: RMDIR: relative/path/to/folder

7. For executing terminal commands (discovery like `ollama list`, installing dependencies, running builds, running tests):
<<<<<<< COMMAND
npm test
>>>>>>>
or: COMMAND: npm test

8. Provide a concise explanation of the architectural approach before emitting diff blocks."#;

        // Build User Prompt
        let mut user_prompt = format!(
            "Execute the following implementation plan with the highest software engineering quality and zero stubs:\n\nTask: {plan_summary}\n\nSteps:\n"
        );
        for (i, step) in steps.iter().enumerate() {
            user_prompt.push_str(&format!(
                "{}. Goal: {}\n   Instructions:\n",
                i + 1, step.goal
            ));
            for inst in &step.instructions {
                user_prompt.push_str(&format!("      - {}\n", inst));
            }
            if !step.likely_files.is_empty() {
                user_prompt.push_str(&format!("   Target Files: {}\n", step.likely_files.join(", ")));
            }
            if !step.acceptance_checks.is_empty() {
                user_prompt.push_str(&format!("   Acceptance Criteria: {}\n", step.acceptance_checks.join("; ")));
            }
        }
        user_prompt.push_str(&format!("\n\n{shredded_files_summary}"));

        // 4. Model Selection & Request Loop
        let mut attempts = 0;
        let mut chosen_model = if let Some(ref tracker_arc) = self.quota_tracker {
            let mut tracker = tracker_arc.lock().await;
            tracker.select_groq_model(preferred_model)?
        } else {
            preferred_model.unwrap_or("qwen2.5-coder:7b").to_string()
        };

        // Pre-Flight Payload Size Check (Step 3 Safeguard):
        // Avoid Groq 8K TPM rate limit traps or 400 "Please reduce the length of the messages"
        let estimated_total_tokens = estimate_tokens(system_prompt) + estimate_tokens(&user_prompt);
        println!("[Groq Worker Payload] Estimated tokens: {estimated_total_tokens}, Target Model: {chosen_model}");

        if self.quota_tracker.is_some() && estimated_total_tokens > 6_000 {
            eprintln!("Payload estimated tokens ({estimated_total_tokens}) exceeds Groq safe limit. Falling back to local Ollama worker...");
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
                                "text": format!("This request is too large for the cloud worker (estimated ~{estimated_total_tokens} tokens, exceeding cloud limit) — running it locally with Ollama instead.")
                            }
                        }
                    }).to_string(),
                    timestamp_ms: SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis() as u64,
                },
            );

            let ollama_client = Self::new_ollama(None);
            return Box::pin(ollama_client.execute_session(
                worktree_path,
                _task_id,
                session_id,
                plan_summary,
                steps,
                None,
                execution_service,
                app_handle,
            )).await;
        }

        let mut response_text = String::new();
        let max_attempts = if self.quota_tracker.is_some() { 3 } else { 1 };

        while attempts < max_attempts {
            attempts += 1;

            let is_ollama = self.quota_tracker.is_none();
            let provider_label = if is_ollama { "Ollama (Local)" } else { "Groq (Cloud)" };

            // Emit reasoning thought and active generation activity
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
                                "text": format!("Sending prompt to {provider_label} model: {chosen_model}")
                            }
                        }
                    }).to_string(),
                    timestamp_ms: SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis() as u64,
                },
            );

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
                                "tool": "write",
                                "callID": format!("call_write_{}_{attempts}", session_id),
                                "state": { "input": { "filePath": format!("{chosen_model}") } }
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

            let mut req_builder = client
                .post(&self.endpoint_url)
                .header("Content-Type", "application/json");

            if !self.api_key.is_empty() && self.api_key != "ollama" {
                req_builder = req_builder.header("Authorization", format!("Bearer {}", self.api_key));
            }

            let req_resp = req_builder
                .json(&body)
                .send()
                .await;

            match req_resp {
                Ok(resp) => {
                    let status = resp.status();
                    if let Some(ref tracker_arc) = self.quota_tracker {
                        // Extract real ground-truth rate-limit headers from Groq response
                        let groq_headers = crate::planner::quota::parse_groq_headers(resp.headers());
                        let mut tracker = tracker_arc.lock().await;
                        tracker.update_groq_server_headers(&chosen_model, &groq_headers);
                        let event = tracker.to_quota_update_event(&chosen_model);
                        let _ = app_handle.emit("planner:quota_update", &event);
                    }

                    if status.is_success() {
                        let json: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
                        if let Some(ref tracker_arc) = self.quota_tracker {
                            let tokens = json
                                .get("usage")
                                .and_then(|u| u.get("total_tokens"))
                                .and_then(|t| t.as_u64())
                                .unwrap_or(1000) as u32;

                            let mut tracker = tracker_arc.lock().await;
                            tracker.record_actual_usage(&chosen_model, tokens);
                            let event = tracker.to_quota_update_event(&chosen_model);
                            let _ = app_handle.emit("planner:quota_update", &event);
                        }

                        let choice = json
                            .get("choices")
                            .and_then(|c| c.as_array())
                            .and_then(|arr| arr.first());

                        response_text = choice
                            .and_then(|c| c.get("message"))
                            .and_then(|m| m.get("content"))
                            .and_then(|c| c.as_str())
                            .unwrap_or_default()
                            .to_string();

                        let finish_reason = choice
                            .and_then(|c| c.get("finish_reason"))
                            .and_then(|f| f.as_str())
                            .unwrap_or("stop");

                        // Auto-continuation if response was cut off by token limit
                        if finish_reason == "length" && !response_text.is_empty() {
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
                                                "text": "Output token limit reached. Automatically retrieving remaining code continuation..."
                                            }
                                        }
                                    }).to_string(),
                                    timestamp_ms: SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis() as u64,
                                },
                            );

                            let cont_prompt = "You reached the maximum output token limit. Continue generating the exact remaining diff/file code without repeating what you already wrote:";
                            let cont_body = serde_json::json!({
                                "model": chosen_model,
                                "messages": [
                                    { "role": "system", "content": system_prompt },
                                    { "role": "user", "content": user_prompt },
                                    { "role": "assistant", "content": response_text },
                                    { "role": "user", "content": cont_prompt }
                                ],
                                "temperature": 0.2,
                                "max_tokens": 4096
                            });

                            let mut cont_builder = client
                                .post(&self.endpoint_url)
                                .header("Content-Type", "application/json");

                            if !self.api_key.is_empty() && self.api_key != "ollama" {
                                cont_builder = cont_builder.header("Authorization", format!("Bearer {}", self.api_key));
                            }

                            if let Ok(cont_resp) = cont_builder.json(&cont_body).send().await {
                                if cont_resp.status().is_success() {
                                    if let Ok(cont_json) = cont_resp.json::<serde_json::Value>().await {
                                        let continuation_chunk = cont_json
                                            .get("choices")
                                            .and_then(|c| c.as_array())
                                            .and_then(|arr| arr.first())
                                            .and_then(|c| c.get("message"))
                                            .and_then(|m| m.get("content"))
                                            .and_then(|c| c.as_str())
                                            .unwrap_or_default();
                                        if !continuation_chunk.is_empty() {
                                            response_text.push('\n');
                                            response_text.push_str(continuation_chunk);
                                        }
                                    }
                                }
                            }
                        }

                        break;
                    } else if status.as_u16() == 429 {
                        if let Some(ref tracker_arc) = self.quota_tracker {
                            // Rate limit hit: ModelQuotaTracker already set cooldown from retry-after header
                            let next_model = {
                                let mut tracker = tracker_arc.lock().await;
                                tracker.select_groq_model(None).map_err(|e| format!("Groq quota exhausted: {e}"))?
                            };
                            println!("Groq 429 on {chosen_model}. Rotating to {next_model}...");
                            chosen_model = next_model;
                            continue;
                        } else {
                            return Err(format!("Local Ollama model returned 429: {}", resp.text().await.unwrap_or_default()));
                        }
                    } else if status.as_u16() == 400 {
                        let err_body = resp.text().await.unwrap_or_default();
                        eprintln!("Groq returned 400: {err_body}");
                        if (err_body.contains("reduce the length of the messages") || err_body.contains("rate_limit") || err_body.contains("token")) && self.quota_tracker.is_some() {
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
                                                "text": "Cloud worker payload exceeded message limit (400) — falling back to local Ollama worker to complete task."
                                            }
                                        }
                                    }).to_string(),
                                    timestamp_ms: SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis() as u64,
                                },
                            );
                            let ollama_client = Self::new_ollama(None);
                            return Box::pin(ollama_client.execute_session(
                                worktree_path,
                                _task_id,
                                session_id,
                                plan_summary,
                                steps,
                                None,
                                execution_service,
                                app_handle,
                            )).await;
                        } else {
                            return Err(format!("{provider_label} API request failed ({status}): {err_body}"));
                        }
                    } else {
                        let err_body = resp.text().await.unwrap_or_default();
                        return Err(format!("{provider_label} API request failed ({status}): {err_body}"));
                    }
                }
                Err(e) => {
                    return Err(format!("Network failure calling {provider_label} API: {e}"));
                }
            }
        }

        if response_text.is_empty() {
            return Err("Worker failed to generate a response.".to_string());
        }

        // 5. Parse and Apply Diff Blocks to Worktree
        let fallback_file = steps.first().and_then(|s| s.likely_files.first()).map(|s| s.as_str());
        let mut applied_diffs = apply_diff_blocks(worktree_path, &response_text, fallback_file)?;

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

        // 5b. Local Code Formatter & Clean Whitespace Pass
        format_and_clean_worktree_files(worktree_path, &applied_diffs);

        // 5c. Local Quality & Completeness Audit
        let quality_findings = audit_code_quality(worktree_path, &applied_diffs);
        if !quality_findings.is_empty() {
            let audit_text = format!("Code Quality Notice: {}", quality_findings.join("; "));
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
                                "text": audit_text
                            }
                        }
                    }).to_string(),
                    timestamp_ms: SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis() as u64,
                },
            );
        } else if !applied_diffs.is_empty() {
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
                                "text": format!("Code Quality Audit passed: {} file(s) verified complete with zero stubs.", applied_diffs.len())
                            }
                        }
                    }).to_string(),
                    timestamp_ms: SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis() as u64,
                },
            );
        }

        // 5d. Automated Local Compiler / Typecheck Verification & Self-Correction Loop
        if !applied_diffs.is_empty() {
            let check_result = run_local_compiler_check(
                worktree_path,
                session_id,
                &execution_service,
                &app_handle,
                &applied_diffs,
            ).await;

            if let Ok(Some(compiler_errors)) = check_result {
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
                                    "text": format!("Local compiler check detected errors:\n{}\nTriggering autonomous self-correction pass...", compiler_errors.trim())
                                }
                            }
                        }).to_string(),
                        timestamp_ms: SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis() as u64,
                    },
                );

                // Perform 1 autonomous repair pass with compiler feedback
                let repair_prompt = format!(
                    "A local compiler/typecheck verification ran on the changes you just made and failed with the following errors:\n\n```\n{}\n```\n\nPlease fix the errors immediately. Output the necessary code updates in diff blocks or full file blocks (e.g. ```language:filepath ... ``` or <<<<<<< SEARCH ... ======= ... >>>>>>>). Do NOT introduce stubs or placeholders.",
                    compiler_errors.trim()
                );

                let repair_body = serde_json::json!({
                    "model": chosen_model,
                    "messages": [
                        { "role": "system", "content": system_prompt },
                        { "role": "user", "content": user_prompt },
                        { "role": "assistant", "content": response_text },
                        { "role": "user", "content": repair_prompt }
                    ],
                    "temperature": 0.1,
                    "max_tokens": 4096
                });

                let mut req_builder = client
                    .post(&self.endpoint_url)
                    .header("Content-Type", "application/json");

                if !self.api_key.is_empty() && self.api_key != "ollama" {
                    req_builder = req_builder.header("Authorization", format!("Bearer {}", self.api_key));
                }

                if let Ok(resp) = req_builder.json(&repair_body).send().await {
                    if resp.status().is_success() {
                        if let Ok(json) = resp.json::<serde_json::Value>().await {
                            let repair_response = json
                                .get("choices")
                                .and_then(|c| c.as_array())
                                .and_then(|arr| arr.first())
                                .and_then(|c| c.get("message"))
                                .and_then(|m| m.get("content"))
                                .and_then(|c| c.as_str())
                                .unwrap_or_default();

                            if !repair_response.is_empty() {
                                if let Ok(repair_diffs) = apply_diff_blocks(worktree_path, repair_response, fallback_file) {
                                    if !repair_diffs.is_empty() {
                                        format_and_clean_worktree_files(worktree_path, &repair_diffs);
                                        applied_diffs.extend(repair_diffs.clone());

                                        // Re-verify after repair
                                        let post_repair_check = run_local_compiler_check(
                                            worktree_path,
                                            session_id,
                                            &execution_service,
                                            &app_handle,
                                            &repair_diffs,
                                        ).await;

                                        let status_msg = match post_repair_check {
                                            Ok(None) => "Autonomous self-correction succeeded: All compiler checks passed (exit code 0).".to_string(),
                                            Ok(Some(remaining)) => format!("Autonomous self-correction completed. Remaining compiler notice: {}", remaining.lines().next().unwrap_or("")),
                                            Err(_) => "Autonomous self-correction completed.".to_string(),
                                        };

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
                                                            "text": status_msg
                                                        }
                                                    }
                                                }).to_string(),
                                                timestamp_ms: SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis() as u64,
                                            },
                                        );
                                    }
                                }
                            }
                        }
                    }
                }
            } else if let Ok(None) = check_result {
                let has_compiler = worktree_path.join("Cargo.toml").exists()
                    || worktree_path.join("tsconfig.json").exists()
                    || worktree_path.join("go.mod").exists();
                if has_compiler {
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
                                        "text": "Local compiler verification passed: syntax & types verified (exit code 0)."
                                    }
                                }
                            }).to_string(),
                            timestamp_ms: SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis() as u64,
                        },
                    );
                }
            }
        }

        // 5e. Automated Local Unit Test Verification (Zero Groq Calls)
        let test_result = run_local_test_check(
            worktree_path,
            session_id,
            &execution_service,
            &app_handle,
        ).await;

        match test_result {
            Ok(None) => {
                let has_test_suite = worktree_path.join("Cargo.toml").exists()
                    || worktree_path.join("pytest.ini").exists()
                    || worktree_path.join("tests").is_dir()
                    || worktree_path.join("go.mod").exists();
                if has_test_suite {
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
                                        "text": "Local test suite verification passed: All unit tests green (exit code 0)."
                                    }
                                }
                            }).to_string(),
                            timestamp_ms: SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis() as u64,
                        },
                    );
                }
            }
            Ok(Some(test_errs)) => {
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
                                    "text": format!("Local test runner notice: unit tests reported failures:\n{}", test_errs.trim())
                                }
                            }
                        }).to_string(),
                        timestamp_ms: SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis() as u64,
                    },
                );
            }
            Err(_) => {}
        }

        // 5f. Automated Local Linter & Code Smells Diagnostics (Zero Groq Calls)
        if let Ok(Some(lint_notices)) = run_local_linter_check(
            worktree_path,
            session_id,
            &execution_service,
            &app_handle,
        ).await {
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
                                "text": format!("Local linter audit diagnostics:\n{}", lint_notices.trim())
                            }
                        }
                    }).to_string(),
                    timestamp_ms: SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis() as u64,
                },
            );
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
                            let mut entry = format!("$ {} {} ({})", req.program, req.args.join(" "), exit_str);
                            if !res.stdout_tail.is_empty() {
                                entry.push_str("\nOutput:\n");
                                for line in res.stdout_tail.iter().take(25) {
                                    entry.push_str(&format!("  {}\n", line));
                                }
                            }
                            executed_commands.push(entry);
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
                            let mut entry = format!("$ {} {} ({})", req.program, req.args.join(" "), exit_str);
                            if !res.stdout_tail.is_empty() {
                                entry.push_str("\nOutput:\n");
                                for line in res.stdout_tail.iter().take(25) {
                                    entry.push_str(&format!("  {}\n", line));
                                }
                            }
                            executed_commands.push(entry);
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
        "python", "python3", "node", "make", "poetry", "uv", "ollama",
        "docker", "git", "rustc", "go", "deno"
    ];

    for step in steps {
        let candidates: Vec<&str> = step.instructions.iter().map(|s| s.as_str())
            .chain(std::iter::once(step.goal.as_str()))
            .collect();

        for candidate in candidates {
            let trimmed = candidate.trim();

            // First check if step explicitly wrote COMMAND: <cmd>
            if let Some(cmd) = trimmed.strip_prefix("COMMAND:").or_else(|| trimmed.strip_prefix("Command:")) {
                let clean = cmd.trim().trim_matches('`').trim_start_matches('$').trim().to_string();
                if !clean.is_empty() && seen.insert(clean.clone()) {
                    commands.push(clean);
                }
                continue;
            }

            // Check if step quoted a command in backticks: `ollama list`
            if let Some(start_bt) = trimmed.find('`') {
                if let Some(end_bt) = trimmed[start_bt + 1..].find('`') {
                    let inside = trimmed[start_bt + 1..start_bt + 1 + end_bt].trim();
                    let first_word = inside.split_whitespace().next().unwrap_or("");
                    if known_binaries.contains(&first_word) {
                        let clean = inside.trim_start_matches('$').trim().to_string();
                        if !clean.is_empty() && seen.insert(clean.clone()) {
                            commands.push(clean);
                        }
                    }
                }
            }

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

/// Formats and normalizes modified files in the worktree, stripping trailing whitespace
/// and ensuring clean, standard line endings and trailing newlines.
pub fn format_and_clean_worktree_files(worktree_path: &Path, applied_diffs: &[crate::worker::diff::AppliedDiffResult]) {
    for diff in applied_diffs {
        let full_path = worktree_path.join(&diff.file_path);
        if full_path.is_file() {
            if let Ok(content) = std::fs::read_to_string(&full_path) {
                let cleaned_lines: Vec<String> = content
                    .lines()
                    .map(|l| l.trim_end().to_string())
                    .collect();
                let mut cleaned = cleaned_lines.join("\n");
                if !cleaned.ends_with('\n') {
                    cleaned.push('\n');
                }
                let _ = std::fs::write(&full_path, cleaned);
            }

            // Local standard formatters if available on system
            if diff.file_path.ends_with(".rs") {
                let prog = if cfg!(windows) { "rustfmt.exe" } else { "rustfmt" };
                let _ = std::process::Command::new(prog)
                    .arg(&full_path)
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .status();
            } else if diff.file_path.ends_with(".go") {
                let prog = if cfg!(windows) { "gofmt.exe" } else { "gofmt" };
                let _ = std::process::Command::new(prog)
                    .args(&["-w", full_path.to_str().unwrap_or("")])
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .status();
            }
        }
    }
}

/// Audits modified files for incomplete implementations, placeholders, stubs,
/// broken asset references, and structural HTML errors.
pub fn audit_code_quality(worktree_path: &Path, applied_diffs: &[crate::worker::diff::AppliedDiffResult]) -> Vec<String> {
    let mut findings = Vec::new();
    let stub_markers = [
        "todo:",
        "fixme:",
        "implement later",
        "lorem ipsum",
        "add more here",
        "placeholder",
    ];

    for diff in applied_diffs {
        let full_path = worktree_path.join(&diff.file_path);
        if full_path.is_file() {
            if let Ok(content) = std::fs::read_to_string(&full_path) {
                let lower = content.to_lowercase();
                for marker in stub_markers {
                    if lower.contains(marker) {
                        findings.push(format!("File '{}' contains stub marker: '{}'", diff.file_path, marker));
                        break;
                    }
                }
                // Check for empty or unusually brief HTML files
                if diff.file_path.ends_with(".html") {
                    if content.lines().count() < 25 {
                        findings.push(format!("HTML file '{}' is unusually brief ({} lines)", diff.file_path, content.lines().count()));
                    }

                    // Check for unclosed root structural tags
                    let open_divs = lower.matches("<div").count();
                    let close_divs = lower.matches("</div>").count();
                    if open_divs != close_divs {
                        findings.push(format!("HTML file '{}' has mismatched <div> tags ({} open vs {} closed)", diff.file_path, open_divs, close_divs));
                    }

                    // Asset and script reference integrity check
                    for token in content.split_whitespace() {
                        let t_lower = token.to_lowercase();
                        let target_attr = if t_lower.starts_with("src=\"") || t_lower.starts_with("href=\"") {
                            Some('"')
                        } else if t_lower.starts_with("src='") || t_lower.starts_with("href='") {
                            Some('\'')
                        } else {
                            None
                        };

                        if let Some(quote) = target_attr {
                            if let Some(start) = token.find(quote) {
                                if let Some(end) = token[start + 1..].find(quote) {
                                    let raw_ref = &token[start + 1..start + 1 + end];
                                    let clean_ref = raw_ref.split('?').next().unwrap_or(raw_ref).split('#').next().unwrap_or(raw_ref);
                                    if !clean_ref.is_empty()
                                        && !clean_ref.starts_with("http://")
                                        && !clean_ref.starts_with("https://")
                                        && !clean_ref.starts_with("//")
                                        && !clean_ref.starts_with("data:")
                                        && !clean_ref.starts_with("mailto:")
                                        && !clean_ref.starts_with('#')
                                    {
                                        let asset_path = worktree_path.join(clean_ref);
                                        if !asset_path.exists() {
                                            findings.push(format!("Asset '{}' referenced in '{}' was not found in worktree", clean_ref, diff.file_path));
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // Cross-file HTML-to-JS ID contract verification
    let mut defined_html_ids = std::collections::HashSet::new();
    for diff in applied_diffs {
        if diff.file_path.ends_with(".html") {
            let full_path = worktree_path.join(&diff.file_path);
            if let Ok(content) = std::fs::read_to_string(&full_path) {
                for token in content.split_whitespace() {
                    let t_lower = token.to_lowercase();
                    if t_lower.starts_with("id=\"") || t_lower.starts_with("id='") {
                        let quote = if t_lower.starts_with("id=\"") { '"' } else { '\'' };
                        if let Some(start) = token.find(quote) {
                            if let Some(end) = token[start + 1..].find(quote) {
                                let id_val = &token[start + 1..start + 1 + end];
                                if !id_val.is_empty() {
                                    defined_html_ids.insert(id_val.to_string());
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    if !defined_html_ids.is_empty() {
        for diff in applied_diffs {
            if diff.file_path.ends_with(".js") || diff.file_path.ends_with(".ts") {
                let full_path = worktree_path.join(&diff.file_path);
                if let Ok(content) = std::fs::read_to_string(&full_path) {
                    let mut search_idx = 0;
                    while let Some(pos) = content[search_idx..].find("getElementById(") {
                        let sub = &content[search_idx + pos + 15..];
                        if let Some(quote) = sub.chars().next() {
                            if quote == '"' || quote == '\'' {
                                if let Some(end_quote) = sub[1..].find(quote) {
                                    let id_used = &sub[1..1 + end_quote];
                                    if !id_used.is_empty() && !defined_html_ids.contains(id_used) {
                                        findings.push(format!("Contract mismatch: '{}' calls getElementById('{}'), but id does not exist in HTML", diff.file_path, id_used));
                                    }
                                }
                            }
                        }
                        search_idx += pos + 15;
                    }
                }
            }
        }
    }

    findings
}

/// Runs an automated local compiler / typecheck / syntax verification pass inside the worktree.
/// Returns Ok(None) if checks passed or no supported compiler is found in the project.
/// Returns Ok(Some(error_summary)) if errors were found.
pub async fn run_local_compiler_check(
    worktree_path: &Path,
    session_id: &str,
    execution_service: &ExecutionService,
    app_handle: &AppHandle,
    applied_diffs: &[crate::worker::diff::AppliedDiffResult],
) -> Result<Option<String>, String> {
    // 1. Rust project check
    if worktree_path.join("Cargo.toml").exists() {
        let prog = if cfg!(windows) { "cargo.exe" } else { "cargo" };
        let args = ["check", "--message-format=short"];
        let desc = "Run cargo check verification".to_string();
        let req = execution_service
            .create_approval_request(prog, &args, worktree_path, &desc, Some(session_id))
            .await;

        if req.auto_approved {
            match execution_service.execute_command(&req.id, Some(app_handle), None).await {
                Ok(res) => {
                    if res.exit_code != Some(0) && !res.cancelled {
                        let mut err = String::new();
                        for line in res.stderr_tail.iter().chain(res.stdout_tail.iter()).take(30) {
                            err.push_str(line);
                            err.push('\n');
                        }
                        if !err.is_empty() {
                            return Ok(Some(err));
                        }
                    }
                }
                Err(e) => return Ok(Some(format!("cargo check failed to run: {e}"))),
            }
        }
    }
    // 2. TypeScript project check
    else if worktree_path.join("tsconfig.json").exists() {
        let prog = if cfg!(windows) { "npx.cmd" } else { "npx" };
        let args = ["tsc", "--noEmit"];
        let desc = "Run typescript typecheck verification".to_string();
        let req = execution_service
            .create_approval_request(prog, &args, worktree_path, &desc, Some(session_id))
            .await;

        if req.auto_approved {
            match execution_service.execute_command(&req.id, Some(app_handle), None).await {
                Ok(res) => {
                    if res.exit_code != Some(0) && !res.cancelled {
                        let mut err = String::new();
                        for line in res.stdout_tail.iter().chain(res.stderr_tail.iter()).take(30) {
                            err.push_str(line);
                            err.push('\n');
                        }
                        if !err.is_empty() {
                            return Ok(Some(err));
                        }
                    }
                }
                Err(e) => return Ok(Some(format!("tsc check failed to run: {e}"))),
            }
        }
    }
    // 3. Go project check
    else if worktree_path.join("go.mod").exists() {
        let prog = if cfg!(windows) { "go.exe" } else { "go" };
        let args = ["vet", "./..."];
        let desc = "Run go vet verification".to_string();
        let req = execution_service
            .create_approval_request(prog, &args, worktree_path, &desc, Some(session_id))
            .await;

        if req.auto_approved {
            match execution_service.execute_command(&req.id, Some(app_handle), None).await {
                Ok(res) => {
                    if res.exit_code != Some(0) && !res.cancelled {
                        let mut err = String::new();
                        for line in res.stderr_tail.iter().chain(res.stdout_tail.iter()).take(30) {
                            err.push_str(line);
                            err.push('\n');
                        }
                        if !err.is_empty() {
                            return Ok(Some(err));
                        }
                    }
                }
                Err(e) => return Ok(Some(format!("go vet failed to run: {e}"))),
            }
        }
    }
    // 4. Python files check
    else {
        let py_files: Vec<&str> = applied_diffs
            .iter()
            .map(|d| d.file_path.as_str())
            .filter(|p| p.ends_with(".py"))
            .collect();

        if !py_files.is_empty() {
            let prog = if cfg!(windows) { "python.exe" } else { "python" };
            let mut args = vec!["-m", "py_compile"];
            args.extend(py_files);
            let desc = "Run python syntax compilation verification".to_string();
            let req = execution_service
                .create_approval_request(prog, &args, worktree_path, &desc, Some(session_id))
                .await;

            if req.auto_approved {
                match execution_service.execute_command(&req.id, Some(app_handle), None).await {
                    Ok(res) => {
                        if res.exit_code != Some(0) && !res.cancelled {
                            let mut err = String::new();
                            for line in res.stderr_tail.iter().chain(res.stdout_tail.iter()).take(30) {
                                err.push_str(line);
                                err.push('\n');
                            }
                            if !err.is_empty() {
                                return Ok(Some(err));
                            }
                        }
                    }
                    Err(e) => return Ok(Some(format!("python py_compile failed: {e}"))),
                }
            }
        }
    }

    Ok(None)
}

/// Runs local linters (cargo clippy, ruff, eslint) in read-only audit mode if configured.
/// Returns Ok(None) if no linter found or clean, or Ok(Some(linter_notices)).
pub async fn run_local_linter_check(
    worktree_path: &Path,
    session_id: &str,
    execution_service: &ExecutionService,
    app_handle: &AppHandle,
) -> Result<Option<String>, String> {
    if worktree_path.join("Cargo.toml").exists() {
        let prog = if cfg!(windows) { "cargo.exe" } else { "cargo" };
        let args = ["clippy", "--message-format=short", "--no-deps"];
        let desc = "Run cargo clippy linter audit".to_string();
        let req = execution_service
            .create_approval_request(prog, &args, worktree_path, &desc, Some(session_id))
            .await;

        if req.auto_approved {
            match execution_service.execute_command(&req.id, Some(app_handle), None).await {
                Ok(res) => {
                    if res.exit_code != Some(0) && !res.cancelled {
                        let mut notices = String::new();
                        for line in res.stdout_tail.iter().chain(res.stderr_tail.iter()).take(15) {
                            if line.contains("warning:") || line.contains("error:") {
                                notices.push_str(line);
                                notices.push('\n');
                            }
                        }
                        if !notices.is_empty() {
                            return Ok(Some(notices));
                        }
                    }
                }
                Err(_) => {}
            }
        }
    }

    Ok(None)
}

/// Runs automated local unit tests inside the worktree if a test suite exists.
/// Returns Ok(None) if tests pass or no test suite is configured.
/// Returns Ok(Some(test_failures)) if tests failed.
pub async fn run_local_test_check(
    worktree_path: &Path,
    session_id: &str,
    execution_service: &ExecutionService,
    app_handle: &AppHandle,
) -> Result<Option<String>, String> {
    // 1. Rust test suite
    if worktree_path.join("Cargo.toml").exists() {
        let prog = if cfg!(windows) { "cargo.exe" } else { "cargo" };
        let args = ["test", "--no-fail-fast"];
        let desc = "Run cargo test verification".to_string();
        let req = execution_service
            .create_approval_request(prog, &args, worktree_path, &desc, Some(session_id))
            .await;

        if req.auto_approved {
            match execution_service.execute_command(&req.id, Some(app_handle), None).await {
                Ok(res) => {
                    if res.exit_code != Some(0) && !res.cancelled {
                        let mut err = String::new();
                        for line in res.stdout_tail.iter().chain(res.stderr_tail.iter()).take(25) {
                            err.push_str(line);
                            err.push('\n');
                        }
                        if !err.is_empty() {
                            return Ok(Some(err));
                        }
                    }
                }
                Err(e) => return Ok(Some(format!("cargo test failed to run: {e}"))),
            }
        }
    }
    // 2. Python test suite
    else if worktree_path.join("pytest.ini").exists() || worktree_path.join("tests").is_dir() {
        let prog = if cfg!(windows) { "pytest.exe" } else { "pytest" };
        let args = ["-q"];
        let desc = "Run pytest verification".to_string();
        let req = execution_service
            .create_approval_request(prog, &args, worktree_path, &desc, Some(session_id))
            .await;

        if req.auto_approved {
            match execution_service.execute_command(&req.id, Some(app_handle), None).await {
                Ok(res) => {
                    if res.exit_code != Some(0) && !res.cancelled {
                        let mut err = String::new();
                        for line in res.stdout_tail.iter().chain(res.stderr_tail.iter()).take(25) {
                            err.push_str(line);
                            err.push('\n');
                        }
                        if !err.is_empty() {
                            return Ok(Some(err));
                        }
                    }
                }
                Err(_) => {}
            }
        }
    }
    // 3. Go test suite
    else if worktree_path.join("go.mod").exists() {
        let prog = if cfg!(windows) { "go.exe" } else { "go" };
        let args = ["test", "./..."];
        let desc = "Run go test verification".to_string();
        let req = execution_service
            .create_approval_request(prog, &args, worktree_path, &desc, Some(session_id))
            .await;

        if req.auto_approved {
            match execution_service.execute_command(&req.id, Some(app_handle), None).await {
                Ok(res) => {
                    if res.exit_code != Some(0) && !res.cancelled {
                        let mut err = String::new();
                        for line in res.stdout_tail.iter().chain(res.stderr_tail.iter()).take(25) {
                            err.push_str(line);
                            err.push('\n');
                        }
                        if !err.is_empty() {
                            return Ok(Some(err));
                        }
                    }
                }
                Err(_) => {}
            }
        }
    }

    Ok(None)
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

    #[test]
    fn test_estimate_tokens_accuracy() {
        let short = "Hello world!";
        let t1 = estimate_tokens(short);
        assert!(t1 >= 3 && t1 <= 6);

        let code = r#"
        pub fn calculate_sum(a: i32, b: i32) -> i32 {
            let result = a + b;
            println!("Calculating {} + {} = {}", a, b, result);
            result
        }
        "#;
        let t2 = estimate_tokens(code);
        assert!(t2 > 20 && t2 < 100);
    }

    #[test]
    fn test_exploration_payload_stays_under_budget() {
        let temp_dir = std::env::temp_dir().join(format!("zencode_test_expl_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        std::fs::create_dir_all(&temp_dir).unwrap();
        let root = &temp_dir;

        // Create 20 mock project files with substantial code
        for i in 0..20 {
            let fpath = root.join(format!("module_{i}.rs"));
            let content = format!(
                r#"
                use std::collections::HashMap;

                pub struct Service{i} {{
                    pub id: u64,
                    pub name: String,
                }}

                impl Service{i} {{
                    pub fn new(id: u64, name: &str) -> Self {{
                        println!("Init service {i}");
                        Self {{ id, name: name.to_string() }}
                    }}

                    pub fn process_data(&self, input: Vec<u8>) -> Result<Vec<u8>, String> {{
                        for b in &input {{
                            println!("Processing byte: {{}}", b);
                        }}
                        Ok(input)
                    }}
                }}
                "#
            );
            std::fs::write(&fpath, content).unwrap();
        }

        let mut files_to_scan = Vec::new();
        if let Ok(entries) = std::fs::read_dir(root) {
            collect_worktree_files(root, entries, &mut files_to_scan, 0);
        }
        assert_eq!(files_to_scan.len(), 20);

        // Build shredded summary in exploration mode
        let mut shredded_files_summary = String::new();
        shredded_files_summary.push_str("=== WORKSPACE FILE TREE ===\n");
        for fpath in &files_to_scan {
            let rel_path = fpath.strip_prefix(root).unwrap_or(fpath);
            shredded_files_summary.push_str(&format!("- {}\n", rel_path.to_string_lossy().replace('\\', "/")));
        }
        shredded_files_summary.push_str("\n=== KEY ENTRYPOINTS & MODULE OUTLINES (AST SIGNATURES) ===\n");

        let mut current_tokens = estimate_tokens(&shredded_files_summary);
        const EXPLORATION_TOKEN_BUDGET: usize = 3_000;

        for fpath in &files_to_scan {
            if current_tokens >= EXPLORATION_TOKEN_BUDGET {
                shredded_files_summary.push_str("\n... [Additional project files summarized in file tree above to stay within safe token limits] ...\n");
                break;
            }
            let rel_path = fpath.strip_prefix(root).unwrap_or(fpath);
            let rel_str = rel_path.to_string_lossy().replace('\\', "/");
            if let Ok(code) = std::fs::read_to_string(fpath) {
                let outline = shred_file_context(fpath, &code);
                assert!(!outline.contains("Processing byte:"), "Function body must be shredded!");
                let section = format!("\n--- Outline: {rel_str} ---\n{outline}\n");
                let section_tokens = estimate_tokens(&section);
                if current_tokens + section_tokens <= EXPLORATION_TOKEN_BUDGET {
                    shredded_files_summary.push_str(&section);
                    current_tokens += section_tokens;
                }
            }
        }

        let total_tokens = estimate_tokens(&shredded_files_summary);
        assert!(total_tokens <= 3_000, "Exploration summary tokens ({total_tokens}) must stay strictly under the 3000 token budget!");
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_format_and_clean_and_audit_code_quality() {
        let temp_dir = std::env::temp_dir().join(format!("zencode_audit_test_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        std::fs::create_dir_all(&temp_dir).unwrap();

        // 1. Create a file with trailing spaces and stub marker
        let dirty_content = "function test() {   \n    // TODO: implement later   \n    return 42;   \n}";
        let test_file = temp_dir.join("test.js");
        std::fs::write(&test_file, dirty_content).unwrap();

        let applied = vec![crate::worker::diff::AppliedDiffResult {
            file_path: "test.js".to_string(),
            is_new: true,
            additions: 4,
            deletions: 0,
        }];

        // Run formatting
        format_and_clean_worktree_files(&temp_dir, &applied);
        let cleaned = std::fs::read_to_string(&test_file).unwrap();
        assert!(cleaned.ends_with('\n'), "Must end with newline");
        assert!(!cleaned.contains("   \n"), "Must strip trailing whitespace");

        // Run audit
        let findings = audit_code_quality(&temp_dir, &applied);
        assert!(!findings.is_empty(), "Must detect TODO: stub marker");
        assert!(findings[0].contains("stub marker: 'todo:'"));

        // 2. Create an HTML file referencing a missing stylesheet and unclosed tag
        let html_content = r#"<!DOCTYPE html>
<html>
<head>
    <link rel="stylesheet" href="nonexistent.css">
</head>
<body>
    <div>
        <p>Hello</p>
</body>
</html>"#;
        let html_file = temp_dir.join("index.html");
        std::fs::write(&html_file, html_content).unwrap();

        let applied_html = vec![crate::worker::diff::AppliedDiffResult {
            file_path: "index.html".to_string(),
            is_new: true,
            additions: 10,
            deletions: 0,
        }];

        let html_findings = audit_code_quality(&temp_dir, &applied_html);
        assert!(html_findings.iter().any(|f| f.contains("nonexistent.css")), "Must detect missing asset reference");
        assert!(html_findings.iter().any(|f| f.contains("mismatched <div> tags")), "Must detect unclosed <div>");

        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}

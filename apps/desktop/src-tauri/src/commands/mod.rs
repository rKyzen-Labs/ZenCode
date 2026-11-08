use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter, State};
use zencode_core::{
    CredentialService, ImplementationPlan, Operation, PlanningContext, redact_secrets,
};

use crate::engine::EngineSupervisor;
use crate::exec::{
    BuildCommandsInfo, BuildSystemType, CommandRequest, DiagnosticBundle, DiagnosticService,
    ExecutionResult, ExecutionService, RecommendedCommand,
};
use crate::git::{
    clean_path_for_git, ChangeInventory, GitError, GitService, MergeResult, OrphanWorktree,
    PrimaryTreeStatus, WorktreeInfo,
};
use crate::models::{
    AdvisoryPreview, ContextPreview, EngineHealth, PlanSubmissionResult, ProjectState, SystemStatus,
    TaskState, WorkerEventPayload, WorkerStartError,
};
use crate::planner::{QuotaUpdateEvent, MODEL_PRIMARY};
use crate::state::AppState;

/// Open a local project directory, detect its characteristics and Git state.
/// Does not start any AI process.
#[tauri::command]
pub async fn open_project(
    path: String,
    state: State<'_, AppState>,
) -> Result<ProjectState, String> {
    execute_open_project(&path, &state).await
}

pub async fn execute_open_project(
    path: &str,
    state: &AppState,
) -> Result<ProjectState, String> {
    let p = Path::new(path);
    if !p.exists() || !p.is_dir() {
        return Err(format!("Invalid project path: '{path}' does not exist or is not a directory"));
    }

    let name = p
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "project".to_string());

    // Detect project type from manifests
    let project_type = if p.join("Cargo.toml").exists() {
        "Rust".to_string()
    } else if p.join("package.json").exists() {
        "Node.js / TypeScript".to_string()
    } else if p.join("go.mod").exists() {
        "Go".to_string()
    } else if p.join("pyproject.toml").exists() || p.join("requirements.txt").exists() {
        "Python".to_string()
    } else {
        "Generic Project".to_string()
    };

    // Check Git state using GitService
    let is_git_repo = GitService::is_git_repo(p).unwrap_or(false);

    let (current_branch, git_status_summary, is_clean, has_commits) = if is_git_repo {
        let has_commits = GitService::has_commits(p).unwrap_or(false);
        let clean_p = clean_path_for_git(p);
        let branch = Command::new("git")
            .arg("-C")
            .arg(&clean_p)
            .args(["branch", "--show-current"])
            .output()
            .ok()
            .and_then(|o| {
                if o.status.success() {
                    let s = String::from_utf8_lossy(&o.stdout).trim().to_string();
                    if !s.is_empty() { Some(s) } else { None }
                } else {
                    None
                }
            });

        match GitService::get_primary_tree_status(p) {
            Ok(st) => {
                let summary = if !has_commits {
                    "No commits yet".to_string()
                } else if st.is_clean {
                    "Clean working tree".to_string()
                } else {
                    format!("{} uncommitted file(s)", st.uncommitted_files.len())
                };
                (branch, summary, st.is_clean, has_commits)
            }
            Err(_) => (branch, "Unknown Git status".to_string(), false, has_commits),
        }
    } else {
        (None, "Not a Git repository".to_string(), false, false)
    };

    // Step 2: Safe read-only inspection for .gitignore presence (zero writes)
    let has_gitignore = p.join(".gitignore").exists();

    // Step 2: Safe read-only inspection for editor/format configs (informational display only)
    let candidate_configs = [
        ".editorconfig",
        "rustfmt.toml",
        ".rustfmt.toml",
        ".prettierrc",
        ".prettierrc.json",
        ".prettierrc.js",
        "prettier.config.js",
        ".eslintrc",
        ".eslintrc.json",
        ".eslintrc.js",
        "eslint.config.js",
        "eslint.config.mjs",
        "biome.json",
        "ruff.toml",
        ".clang-format",
    ];
    let mut detected_editor_configs = Vec::new();
    for cfg in candidate_configs {
        if p.join(cfg).exists() {
            detected_editor_configs.push(cfg.to_string());
        }
    }

    let project_state = ProjectState {
        path: path.to_string(),
        name,
        project_type,
        is_git_repo,
        current_branch,
        git_status_summary,
        is_clean,
        has_gitignore,
        detected_editor_configs,
        has_commits,
    };

    // Store in AppState
    {
        let mut active = state.active_project.lock().await;
        *active = Some(project_state.clone());
    }

    // Persist workspace in SQLite
    let _ = state.db.record_workspace(path, &project_state.name);

    Ok(project_state)
}

/// Step 3: Explicit user action to initialize a Git repository.
/// Never run automatically. Only invoked upon per-action user confirmation.
#[tauri::command]
pub async fn init_git_repo(
    path: String,
    state: State<'_, AppState>,
) -> Result<ProjectState, String> {
    execute_init_git_repo(&path, &state).await
}

pub async fn execute_init_git_repo(
    path: &str,
    state: &AppState,
) -> Result<ProjectState, String> {
    let p = Path::new(path);
    if !p.exists() || !p.is_dir() {
        return Err(format!("Cannot initialize Git: '{path}' does not exist or is not a directory"));
    }
    let clean_p = clean_path_for_git(p);
    let output = Command::new("git")
        .arg("-C")
        .arg(&clean_p)
        .arg("init")
        .output()
        .map_err(|e| format!("Failed to execute 'git init': {e}"))?;

    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        return Err(format!("'git init' failed: {err}"));
    }

    // Refresh ProjectState after git init
    execute_open_project(path, state).await
}

/// Step 3: Explicit user action to create an initial empty commit in a repository with no commits.
/// Never run automatically. Only invoked upon per-action user confirmation.
#[tauri::command]
pub async fn create_initial_empty_commit(
    path: String,
    state: State<'_, AppState>,
) -> Result<ProjectState, String> {
    execute_create_initial_empty_commit(&path, &state).await
}

pub async fn execute_create_initial_empty_commit(
    path: &str,
    state: &AppState,
) -> Result<ProjectState, String> {
    let p = Path::new(path);
    if !p.exists() || !p.is_dir() {
        return Err(format!("Cannot create initial commit: '{path}' does not exist or is not a directory"));
    }
    GitService::create_initial_empty_commit(p)
        .map_err(|e| format!("Failed to create initial commit: {e}"))?;

    execute_open_project(path, state).await
}

/// Step 3: Explicit user action to create a standard .gitignore file for the detected project type.
/// Never run automatically. Only invoked upon per-action user confirmation.
#[tauri::command]
pub async fn create_standard_gitignore(
    path: String,
    project_type: String,
    state: State<'_, AppState>,
) -> Result<String, String> {
    execute_create_standard_gitignore(&path, &project_type, &state).await
}

pub async fn execute_create_standard_gitignore(
    path: &str,
    project_type: &str,
    state: &AppState,
) -> Result<String, String> {
    let p = Path::new(path);
    if !p.exists() || !p.is_dir() {
        return Err(format!("Invalid path: '{path}' does not exist or is not a directory"));
    }
    let gitignore_path = p.join(".gitignore");
    if gitignore_path.exists() {
        return Ok(".gitignore already exists".to_string());
    }

    let entries = match project_type.to_lowercase().as_str() {
        s if s.contains("rust") => {
            "/target\n**/*.rs.bk\nCargo.lock\n"
        }
        s if s.contains("node") || s.contains("typescript") || s.contains("javascript") => {
            "node_modules/\ndist/\n.env\n.env.local\n*.log\n"
        }
        s if s.contains("python") => {
            "__pycache__/\n*.py[cod]\n*$py.class\n.venv/\nvenv/\nenv/\n.env\ndist/\nbuild/\n*.egg-info/\n"
        }
        s if s.contains("go") => {
            "bin/\ndist/\n*.exe\n*.exe~\n*.dll\n*.so\n*.dylib\n"
        }
        _ => {
            ".DS_Store\nThumbs.db\n*.log\n.env\n"
        }
    };

    tokio::fs::write(&gitignore_path, entries)
        .await
        .map_err(|e| format!("Failed to write .gitignore: {e}"))?;

    // Update active project state in AppState
    {
        let mut active = state.active_project.lock().await;
        if let Some(ref mut proj) = *active {
            if proj.path == path {
                proj.has_gitignore = true;
            }
        }
    }

    let summary = format!(
        "Added standard .gitignore for {} with rules:\n{}",
        project_type,
        entries.trim()
    );
    Ok(summary)
}

/// Returns current project state and overall engine readiness indicators.
#[tauri::command]
pub async fn get_project_state(state: State<'_, AppState>) -> Result<SystemStatus, String> {
    let active_project = state.active_project.lock().await.clone();
    let task_state = state.task_state.lock().await.clone();
    let request_count = state.get_request_count();

    // Check Gemini key in Windows Credential Manager (never leaks to frontend)
    let gemini_status = match CredentialService::get_gemini_api_key() {
        Ok(key) if !key.is_empty() => "Configured (OS Storage)".to_string(),
        _ => "Missing Key (Set in Windows Credential Manager)".to_string(),
    };

    // Check Ollama availability with timeout
    let ollama_status = {
        let client = reqwest::Client::new();
        match client
            .get("http://127.0.0.1:11434/api/tags")
            .timeout(Duration::from_millis(1500))
            .send()
            .await
        {
            Ok(resp) if resp.status().is_success() => "Online (Local Inference Ready)".to_string(),
            _ => "Offline (Run 'ollama serve')".to_string(),
        }
    };

    // Check OpenCode supervisor
    let opencode_health = {
        let supervisor = state.engine_supervisor.lock().await;
        if let Some(sup) = supervisor.as_ref() {
            EngineHealth::Ready {
                port: sup.port(),
                version: sup.version().to_string(),
            }
        } else {
            EngineHealth::NotStarted
        }
    };

    Ok(SystemStatus {
        active_project,
        task_state,
        request_count,
        gemini_status,
        ollama_status,
        opencode_health,
        planner_model: "gemini-3.5-flash-lite".to_string(),
        worker_model: "granite4.1:8b".to_string(),
    })
}

/// Assembles the Gemini context package locally for user inspection.
/// Makes zero network calls.
#[tauri::command]
pub async fn preview_planning_context(
    task_description: String,
    state: State<'_, AppState>,
) -> Result<ContextPreview, String> {
    execute_preview_planning_context(&task_description, &state).await
}

pub async fn execute_preview_planning_context(
    task_description: &str,
    state: &AppState,
) -> Result<ContextPreview, String> {
    if task_description.trim().is_empty() {
        return Err("Task description cannot be empty".into());
    }

    let active_project = {
        let guard = state.active_project.lock().await;
        guard
            .clone()
            .ok_or_else(|| "No project is currently open. Open a project first.".to_string())?
    };

    let project_dir = Path::new(&active_project.path);

    // Build shallow tree (filtered from ignores and secret exclusions)
    let mut shallow_tree = Vec::new();
    let mut manifest_summaries = Vec::new();

    if let Ok(entries) = std::fs::read_dir(project_dir) {
        for entry in entries.flatten() {
            let file_name = entry.file_name().to_string_lossy().to_string();
            // Exclude noise directories
            if file_name == ".git"
                || file_name == "node_modules"
                || file_name == "target"
                || file_name == ".venv"
                || file_name == "dist"
                || file_name == "binaries"
            {
                continue;
            }

            shallow_tree.push(file_name.clone());

            // Read high level manifest summaries
            if file_name == "Cargo.toml" || file_name == "package.json" {
                if let Ok(content) = std::fs::read_to_string(entry.path()) {
                    let preview_lines: Vec<&str> = content.lines().take(20).collect();
                    manifest_summaries.push(format!("{file_name}:\n{}", preview_lines.join("\n")));
                }
            }
        }
    }

    let git_branch_and_status = format!(
        "Branch: {}, Status: {}",
        active_project.current_branch.unwrap_or_else(|| "none".into()),
        active_project.git_status_summary
    );

    let context = PlanningContext {
        task_description: task_description.to_string(),
        project_type: active_project.project_type.clone(),
        shallow_tree: shallow_tree.clone(),
        manifest_summaries: manifest_summaries.clone(),
        git_branch_and_status: git_branch_and_status.clone(),
        approved_source_excerpts: Vec::new(),
        constraints: vec![
            "Free-tier planning only: produce clear step-by-step guidance".into(),
            "All implementation must execute locally".into(),
        ],
    };

    // Assert that no credentials or secret files exist in the payload
    context
        .assert_no_secrets()
        .map_err(|e| format!("Security check failed: {e}"))?;

    let preview_text = context.to_prompt_text();
    let preview_id = format!(
        "prev_{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()
    );

    // Store in pending previews
    {
        let mut previews = state.pending_previews.lock().await;
        previews.insert(preview_id.clone(), context);
    }

    Ok(ContextPreview {
        preview_id,
        task_description: task_description.to_string(),
        project_type: active_project.project_type,
        shallow_tree,
        manifest_summaries,
        git_branch_and_status,
        preview_text,
    })
}

/// Core implementation of submit_plan_request.
/// Separated for direct unit testing of confirmation enforcement.
pub async fn execute_submit_plan(
    context_preview_id: &str,
    confirmed: bool,
    state: &AppState,
    app_handle: Option<&AppHandle>,
) -> Result<PlanSubmissionResult, String> {
    // HARD ENFORCEMENT: confirmed must be true in Rust
    if !confirmed {
        return Err("Explicit confirmation required: user must review and confirm context preview before remote planning".into());
    }

    Operation::CreateInitialPlan
        .enforce_gemini_permission(confirmed)
        .map_err(|e| format!("Routing violation: {e}"))?;

    let context = {
        let mut previews = state.pending_previews.lock().await;
        previews
            .remove(context_preview_id)
            .ok_or_else(|| format!("Invalid or expired context preview ID '{context_preview_id}'"))?
    };

    // Update state to Planning
    {
        let mut task = state.task_state.lock().await;
        *task = TaskState::Planning;
    }

    // Retrieve Gemini API key from Windows Credential Manager
    let api_key = CredentialService::get_gemini_api_key()
        .map_err(|e| format!("Failed to read Gemini API key from OS Credential Store: {e}"))?;

    // Execute plan request through Rate Limit Manager & Auto-Cascade Router
    let plan_result = state
        .planner_client
        .plan_with_cascade(&api_key, &context, app_handle)
        .await;

    match plan_result {
        Ok((plan, active_model, usage)) => {
            state.increment_request_count();
            let switch_reason = {
                let tracker = state.planner_client.quota_tracker.lock().await;
                tracker.last_switch_reason.clone()
            };
            {
                let mut task = state.task_state.lock().await;
                *task = TaskState::AwaitingReview;
            }
            Ok(PlanSubmissionResult {
                plan,
                usage,
                active_model,
                switch_reason,
            })
        }
        Err(e) => {
            let sanitized = redact_secrets(&e, Some(&api_key));
            {
                let mut task = state.task_state.lock().await;
                *task = TaskState::Failed(sanitized.clone());
            }
            Err(format!("Planning request failed: {sanitized}"))
        }
    }
}

/// Submits the confirmed planning request to Gemini using rate-limit cascade.
/// Enforces confirmed == true in Rust.
#[tauri::command]
pub async fn submit_plan_request(
    context_preview_id: String,
    confirmed: bool,
    app_handle: AppHandle,
    state: State<'_, AppState>,
) -> Result<PlanSubmissionResult, String> {
    execute_submit_plan(&context_preview_id, confirmed, &state, Some(&app_handle)).await
}

/// Returns current real-time quota telemetry across all 4 supported models.
#[tauri::command]
pub async fn get_quota_state(state: State<'_, AppState>) -> Result<QuotaUpdateEvent, String> {
    let mut tracker = state.planner_client.quota_tracker.lock().await;
    let active = tracker
        .pinned_model
        .clone()
        .unwrap_or_else(|| MODEL_PRIMARY.to_string());
    Ok(tracker.to_quota_update_event(&active))
}

/// Manually pin gemini-3.8-flash for advanced planning, or revert to None (auto-cascade).
#[tauri::command]
pub async fn set_pinned_model(
    model: Option<String>,
    app_handle: AppHandle,
    state: State<'_, AppState>,
) -> Result<QuotaUpdateEvent, String> {
    let event = {
        let mut tracker = state.planner_client.quota_tracker.lock().await;
        tracker.set_pinned_model(model.clone());
        let active = model.unwrap_or_else(|| MODEL_PRIMARY.to_string());
        tracker.to_quota_update_event(&active)
    };
    let _ = app_handle.emit("planner:quota_update", &event);
    Ok(event)
}

/// Determines if an OpenCode SSE event represents a completion signal for the active session.
pub fn parse_worker_sse_event(data: &str, current_session_id: &str) -> (String, bool) {
    if let Ok(val) = serde_json::from_str::<serde_json::Value>(data) {
        let event_type = val.get("type").and_then(|s| s.as_str()).unwrap_or("unknown").to_string();
        let mut completed = false;

        // Check if this event targets the current session
        let event_session_id = val.get("properties")
            .and_then(|p| p.get("sessionID").or_else(|| p.get("sessionId")))
            .and_then(|s| s.as_str());
        let matches_session = event_session_id.map(|sid| sid == current_session_id).unwrap_or(true);

        if matches_session {
            // 1. Session status transitions
            if event_type == "session.status" {
                if let Some(status_type) = val.get("properties")
                    .and_then(|p| p.get("status"))
                    .and_then(|s| s.get("type"))
                    .and_then(|st| st.as_str())
                {
                    if status_type == "idle" {
                        completed = true;
                    }
                }
            } else if event_type == "session.idle" {
                completed = true;
            }

            // 2. OpenCode message part step-finish with stop/end_turn/complete
            if let Some(part) = val.get("properties").and_then(|p| p.get("part")) {
                let part_type = part.get("type").and_then(|s| s.as_str()).unwrap_or_default();
                let reason = part.get("reason").and_then(|s| s.as_str()).unwrap_or_default();
                if part_type == "step-finish" && (reason == "stop" || reason == "end_turn" || reason == "complete") {
                    completed = true;
                }
            }

            // 3. OpenCode message.updated info.finish with stop/end_turn/complete
            if let Some(info) = val.get("properties").and_then(|p| p.get("info")) {
                if let Some(finish) = info.get("finish").and_then(|s| s.as_str()) {
                    if finish == "stop" || finish == "end_turn" || finish == "complete" {
                        completed = true;
                    }
                }
            }

            // 4. Fallback raw string markers
            if !completed && (data.contains("\"reason\":\"stop\"") || data.contains("\"finish\":\"stop\"") || data.contains("session.idle")) {
                completed = true;
            }
        }

        (event_type, completed)
    } else {
        let completed = (data.contains("\"reason\":\"stop\"") || data.contains("\"finish\":\"stop\"") || data.contains("session.idle"))
            && data.contains(current_session_id);
        ("unknown".to_string(), completed)
    }
}

/// Starts an OpenCode worker session restricted strictly to local Ollama.
#[tauri::command]
pub async fn start_worker_session(
    plan: ImplementationPlan,
    task_id: Option<String>,
    acknowledge_dirty: Option<bool>,
    app_handle: AppHandle,
    state: State<'_, AppState>,
) -> Result<String, WorkerStartError> {
    let project_dir = {
        let guard = state.active_project.lock().await;
        guard
            .as_ref()
            .map(|p| p.path.clone())
            .ok_or_else(|| WorkerStartError {
                error_type: "NO_ACTIVE_PROJECT".into(),
                message: "No active project is open. Please open a project first.".into(),
                uncommitted_files: None,
            })?
    };

    // 1. Verify Git repository
    let is_git = GitService::is_git_repo(Path::new(&project_dir)).map_err(|e| WorkerStartError {
        error_type: "GIT_ERROR".into(),
        message: e.to_string(),
        uncommitted_files: None,
    })?;

    if !is_git {
        return Err(WorkerStartError {
            error_type: "NOT_GIT_REPO".into(),
            message: "Project is not inside a Git repository. Git isolation requires a Git repository.".into(),
            uncommitted_files: None,
        });
    }

    // 2. Generate or validate task_id
    let tid = task_id.unwrap_or_else(|| {
        format!(
            "task_{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis()
        )
    });

    // 3. HARD ENFORCEMENT: Create task worktree before launching OpenCode
    // If worktree creation fails for any reason, abort and return typed error - NEVER fall back to primary tree!
    let ack = acknowledge_dirty.unwrap_or(false);
    let worktree_info = match GitService::create_task_worktree(Path::new(&project_dir), &tid, ack) {
        Ok(info) => info,
        Err(GitError::DirtyPrimaryTree {
            count,
            uncommitted_files,
        }) => {
            return Err(WorkerStartError {
                error_type: "DIRTY_PRIMARY_TREE".into(),
                message: format!(
                    "Primary working tree is dirty with {count} uncommitted file(s). Explicit acknowledgment required."
                ),
                uncommitted_files: Some(uncommitted_files),
            });
        }
        Err(GitError::UnbornHead) => {
            return Err(WorkerStartError {
                error_type: "UNBORN_HEAD".into(),
                message: "This repository has no commits yet. Zencode needs at least one commit to create an isolated task branch.".into(),
                uncommitted_files: None,
            });
        }
        Err(e) => {
            return Err(WorkerStartError {
                error_type: "WORKTREE_CREATION_FAILED".into(),
                message: format!("Failed to create isolated task worktree: {e}"),
                uncommitted_files: None,
            });
        }
    };

    // Store active worktree in AppState
    {
        let mut wt_map = state.active_worktrees.lock().await;
        wt_map.insert(tid.clone(), worktree_info.clone());
        let mut cur_tid = state.current_task_id.lock().await;
        *cur_tid = Some(tid.clone());
    }

    // 4. Configure OpenCode sidecar strictly with worktree path as its working directory
    let mut supervisor_guard = state.engine_supervisor.lock().await;
    if let Some(mut existing) = supervisor_guard.take() {
        existing.shutdown();
    }

    let sup = EngineSupervisor::start(Some(Path::new(&worktree_info.worktree_path)))
        .await
        .map_err(|e| WorkerStartError {
            error_type: "SIDECAR_ERROR".into(),
            message: format!("Failed to launch OpenCode sidecar in worktree: {e}"),
            uncommitted_files: None,
        })?;
    *supervisor_guard = Some(sup);

    let supervisor = supervisor_guard.as_ref().unwrap();

    // 5. Create OpenCode session targeting the worktree directory
    let session_id = supervisor
        .create_session(&worktree_info.worktree_path)
        .await
        .map_err(|e| WorkerStartError {
            error_type: "SIDECAR_ERROR".into(),
            message: format!("Failed to create OpenCode worker session: {e}"),
            uncommitted_files: None,
        })?;

    // 6. Format initial worker prompt from plan
    let mut prompt = format!(
        "Execute the following implementation plan using local tools only:\n\nTask: {}\n\nSteps:\n",
        plan.task_summary
    );
    for step in &plan.steps {
        prompt.push_str(&format!(
            "- [Goal]: {}\n  Instructions: {:?}\n  Likely files: {:?}\n",
            step.goal, step.instructions, step.likely_files
        ));
    }

    // 7. Send asynchronous prompt specifying Ollama worker model (Granite 4.1 8B)
    supervisor
        .send_prompt(&session_id, &prompt, "granite4.1:8b")
        .await
        .map_err(|e| WorkerStartError {
            error_type: "PROMPT_DISPATCH_FAILED".into(),
            message: format!("Failed to dispatch prompt to worker: {e}"),
            uncommitted_files: None,
        })?;

    // Update state to WorkerRunning
    {
        let mut task = state.task_state.lock().await;
        *task = TaskState::WorkerRunning {
            session_id: session_id.clone(),
        };
    }

    // Emit worker:status event
    let _ = app_handle.emit(
        "worker:status",
        &TaskState::WorkerRunning {
            session_id: session_id.clone(),
        },
    );

    // 8. Spawn SSE event listener thread for real-time streaming to React and inventory emit upon completion
    let sse_url = supervisor.sse_event_url();
    let auth_header = supervisor.auth_header();
    let app_handle_clone = app_handle.clone();
    let ses_id_clone = session_id.clone();
    let wt_path_clone = worktree_info.worktree_path.clone();
    let task_id_clone = tid.clone();
    let inventories_arc = state.active_task_inventories.clone();
    let task_state_arc = state.task_state.clone();

    tokio::spawn(async move {
        use eventsource_stream::Eventsource;
        use futures_util::StreamExt;

        let client = reqwest::Client::new();
        if let Ok(response) = client
            .get(&sse_url)
            .header("Authorization", auth_header)
            .send()
            .await
        {
            let mut stream = response.bytes_stream().eventsource();
            while let Some(event_result) = stream.next().await {
                if let Ok(event) = event_result {
                    let now = SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_millis() as u64;

                    // Extract the event type and evaluate completion signals
                    let (event_type, is_idle_transition) = parse_worker_sse_event(&event.data, &ses_id_clone);

                    let payload = WorkerEventPayload {
                        session_id: ses_id_clone.clone(),
                        event_type,
                        content: event.data,
                        timestamp_ms: now,
                    };
                    let _ = app_handle_clone.emit("worker:message", &payload);

                    if is_idle_transition {
                        let mut task = task_state_arc.lock().await;
                        if matches!(*task, TaskState::WorkerRunning { .. }) {
                            // Worker execution finished: compute complete Git diff inventory
                            match GitService::get_worktree_diff(Path::new(&wt_path_clone)) {
                                Ok(inventory) => {
                                    {
                                        let mut inv_map = inventories_arc.lock().await;
                                        inv_map.insert(task_id_clone.clone(), inventory.clone());
                                    }
                                    // Emit review:inventory_ready event with full change inventory
                                    let _ = app_handle_clone.emit("review:inventory_ready", &inventory);

                                    *task = TaskState::AwaitingReview;
                                    let _ = app_handle_clone.emit("worker:status", &TaskState::AwaitingReview);
                                }
                                Err(e) => {
                                    eprintln!("Failed to generate diff inventory: {e}");
                                    *task = TaskState::Complete;
                                    let _ = app_handle_clone.emit("worker:status", &TaskState::Complete);
                                }
                            }
                        }
                    }
                }
            }

            // Stream closed safety net: if still WorkerRunning when stream ends, transition cleanly
            let mut task = task_state_arc.lock().await;
            if matches!(*task, TaskState::WorkerRunning { .. }) {
                match GitService::get_worktree_diff(Path::new(&wt_path_clone)) {
                    Ok(inventory) => {
                        {
                            let mut inv_map = inventories_arc.lock().await;
                            inv_map.insert(task_id_clone.clone(), inventory.clone());
                        }
                        let _ = app_handle_clone.emit("review:inventory_ready", &inventory);
                        *task = TaskState::AwaitingReview;
                        let _ = app_handle_clone.emit("worker:status", &TaskState::AwaitingReview);
                    }
                    Err(e) => {
                        eprintln!("Failed to generate diff inventory on stream end: {e}");
                        *task = TaskState::Complete;
                        let _ = app_handle_clone.emit("worker:status", &TaskState::Complete);
                    }
                }
            }
        }
    });

    Ok(session_id)
}

/// Stops an active worker session cleanly.
#[tauri::command]
pub async fn stop_worker_session(
    session_id: String,
    app_handle: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    {
        let supervisor = state.engine_supervisor.lock().await;
        if let Some(sup) = supervisor.as_ref() {
            let _ = sup.abort_session(&session_id).await;
        }
    }

    {
        let mut task = state.task_state.lock().await;
        *task = TaskState::Idle;
    }

    let _ = app_handle.emit("worker:status", &TaskState::Idle);
    Ok(())
}

/// Returns the complete change inventory for an active task worktree.
#[tauri::command]
pub async fn get_change_inventory(
    task_id: String,
    state: State<'_, AppState>,
) -> Result<ChangeInventory, String> {
    // 1. Check cached inventory first
    {
        let inv_map = state.active_task_inventories.lock().await;
        if let Some(inv) = inv_map.get(&task_id) {
            return Ok(inv.clone());
        }
    }

    // 2. Resolve worktree path
    let worktree_path = {
        let worktrees = state.active_worktrees.lock().await;
        if let Some(info) = worktrees.get(&task_id) {
            info.worktree_path.clone()
        } else {
            let active_proj = state.active_project.lock().await;
            let proj = active_proj
                .as_ref()
                .ok_or_else(|| "No active project open".to_string())?;

            let p = Path::new(&proj.path);
            let canonical = p.canonicalize().unwrap_or_else(|_| p.to_path_buf());
            let clean = clean_path_for_git(&canonical);
            let dir = if let Some(parent) = clean.parent() {
                parent.join(".zencode-worktrees").join(&task_id)
            } else {
                clean.join(".zencode-worktrees").join(&task_id)
            };
            dir.to_string_lossy().to_string()
        }
    };

    let inventory = GitService::get_worktree_diff(Path::new(&worktree_path))
        .map_err(|e| format!("Failed to read worktree diff: {e}"))?;

    // Cache
    {
        let mut inv_map = state.active_task_inventories.lock().await;
        inv_map.insert(task_id, inventory.clone());
    }

    Ok(inventory)
}

/// Merges or fast-forwards the task branch into the target branch after user confirmation.
/// Confirms target branch is clean and handles conflicts without auto-resolving.
#[tauri::command]
pub async fn accept_task(
    task_id: String,
    acknowledge_dirty: Option<bool>,
    app_handle: AppHandle,
    state: State<'_, AppState>,
) -> Result<MergeResult, WorkerStartError> {
    let (worktree_path, target_branch) = {
        let active_proj = state.active_project.lock().await;
        let proj = match active_proj.as_ref() {
            Some(p) => p,
            None => {
                return Err(WorkerStartError {
                    error_type: "NO_ACTIVE_PROJECT".into(),
                    message: "No active project open".into(),
                    uncommitted_files: None,
                });
            }
        };

        let target_br = proj
            .current_branch
            .clone()
            .unwrap_or_else(|| "main".to_string());

        let worktrees = state.active_worktrees.lock().await;
        let wt_path = if let Some(info) = worktrees.get(&task_id) {
            info.worktree_path.clone()
        } else {
            let p = Path::new(&proj.path);
            let canonical = p.canonicalize().unwrap_or_else(|_| p.to_path_buf());
            let clean = clean_path_for_git(&canonical);
            let dir = if let Some(parent) = clean.parent() {
                parent.join(".zencode-worktrees").join(&task_id)
            } else {
                clean.join(".zencode-worktrees").join(&task_id)
            };
            dir.to_string_lossy().to_string()
        };

        (wt_path, target_br)
    };

    let ack = acknowledge_dirty.unwrap_or(false);
    let merge_result = match GitService::accept_task_changes(Path::new(&worktree_path), &target_branch, ack) {
        Ok(res) => res,
        Err(GitError::DirtyPrimaryTree {
            count,
            uncommitted_files,
        }) => {
            return Err(WorkerStartError {
                error_type: "DIRTY_PRIMARY_TREE".into(),
                message: format!(
                    "Primary working tree is dirty with {count} uncommitted file(s). Explicit acknowledgment required."
                ),
                uncommitted_files: Some(uncommitted_files),
            });
        }
        Err(GitError::UnbornHead) => {
            return Err(WorkerStartError {
                error_type: "UNBORN_HEAD".into(),
                message: "This repository has no commits yet. Zencode needs at least one commit to create an isolated task branch.".into(),
                uncommitted_files: None,
            });
        }
        Err(e) => {
            return Err(WorkerStartError {
                error_type: "ACCEPT_TASK_FAILED".into(),
                message: format!("Failed to accept task changes: {e}"),
                uncommitted_files: None,
            });
        }
    };

    match &merge_result {
        MergeResult::Success { .. } => {
            // Clean up state
            {
                let mut worktrees = state.active_worktrees.lock().await;
                worktrees.remove(&task_id);
                let mut inventories = state.active_task_inventories.lock().await;
                inventories.remove(&task_id);
                let mut current_tid = state.current_task_id.lock().await;
                if current_tid.as_deref() == Some(&task_id) {
                    *current_tid = None;
                }
            }

            {
                let mut task = state.task_state.lock().await;
                *task = TaskState::Complete;
            }
            let _ = app_handle.emit("worker:status", &TaskState::Complete);
        }
        MergeResult::Conflict { .. } => {
            // Conflict occurred: merge aborted, primary tree remains clean, return conflict to UI
        }
    }

    Ok(merge_result)
}

/// Removes the task worktree and deletes the task branch after user confirmation.
/// Never touches the primary working tree. Never runs git reset --hard.
#[tauri::command]
pub async fn reject_task(
    task_id: String,
    app_handle: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let worktree_path = {
        let active_proj = state.active_project.lock().await;
        let proj = active_proj
            .as_ref()
            .ok_or_else(|| "No active project open".to_string())?;

        let worktrees = state.active_worktrees.lock().await;
        if let Some(info) = worktrees.get(&task_id) {
            info.worktree_path.clone()
        } else {
            let p = Path::new(&proj.path);
            let canonical = p.canonicalize().unwrap_or_else(|_| p.to_path_buf());
            let clean = clean_path_for_git(&canonical);
            let dir = if let Some(parent) = clean.parent() {
                parent.join(".zencode-worktrees").join(&task_id)
            } else {
                clean.join(".zencode-worktrees").join(&task_id)
            };
            dir.to_string_lossy().to_string()
        }
    };

    GitService::reject_task_changes(Path::new(&worktree_path), &task_id)
        .map_err(|e| format!("Failed to reject task changes: {e}"))?;

    {
        let mut worktrees = state.active_worktrees.lock().await;
        worktrees.remove(&task_id);
        let mut inventories = state.active_task_inventories.lock().await;
        inventories.remove(&task_id);
        let mut current_tid = state.current_task_id.lock().await;
        if current_tid.as_deref() == Some(&task_id) {
            *current_tid = None;
        }
    }

    {
        let mut task = state.task_state.lock().await;
        *task = TaskState::Idle;
    }
    let _ = app_handle.emit("worker:status", &TaskState::Idle);

    Ok(())
}

/// Checks the primary working tree clean/dirty status and returns uncommitted files.
#[tauri::command]
pub async fn get_primary_tree_status(
    state: State<'_, AppState>,
) -> Result<PrimaryTreeStatus, String> {
    let repo_path = {
        let active_proj = state.active_project.lock().await;
        active_proj
            .as_ref()
            .map(|p| p.path.clone())
            .ok_or_else(|| "No active project open".to_string())?
    };

    GitService::get_primary_tree_status(Path::new(&repo_path))
        .map_err(|e| format!("Failed to check primary tree status: {e}"))
}

/// Stashes uncommitted changes in the primary tree using argument arrays.
#[tauri::command]
pub async fn stash_primary_changes(
    message: Option<String>,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let repo_path = {
        let active_proj = state.active_project.lock().await;
        active_proj
            .as_ref()
            .map(|p| p.path.clone())
            .ok_or_else(|| "No active project open".to_string())?
    };

    let p = clean_path_for_git(Path::new(&repo_path));
    let msg = message.unwrap_or_else(|| "Zencode: stash before task".to_string());

    let output = Command::new("git")
        .arg("-C")
        .arg(&p)
        .args(["stash", "push", "-u", "-m", &msg])
        .output()
        .map_err(|e| format!("Failed to run git stash: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("git stash failed: {stderr}"));
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// Commits uncommitted changes in the primary tree using argument arrays.
#[tauri::command]
pub async fn commit_primary_changes(
    message: String,
    state: State<'_, AppState>,
) -> Result<String, String> {
    if message.trim().is_empty() {
        return Err("Commit message cannot be empty".to_string());
    }

    let repo_path = {
        let active_proj = state.active_project.lock().await;
        active_proj
            .as_ref()
            .map(|p| p.path.clone())
            .ok_or_else(|| "No active project open".to_string())?
    };

    let p = clean_path_for_git(Path::new(&repo_path));

    let add_output = Command::new("git")
        .arg("-C")
        .arg(&p)
        .args(["add", "-A"])
        .output()
        .map_err(|e| format!("Failed to run git add: {e}"))?;

    if !add_output.status.success() {
        let stderr = String::from_utf8_lossy(&add_output.stderr);
        return Err(format!("git add failed: {stderr}"));
    }

    let commit_output = Command::new("git")
        .arg("-C")
        .arg(&p)
        .args(["commit", "-m", &message])
        .output()
        .map_err(|e| format!("Failed to run git commit: {e}"))?;

    if !commit_output.status.success() {
        let stderr = String::from_utf8_lossy(&commit_output.stderr);
        return Err(format!("git commit failed: {stderr}"));
    }

    Ok(String::from_utf8_lossy(&commit_output.stdout).trim().to_string())
}

/// Lists all active Zencode-managed worktrees for orphan detection.
#[tauri::command]
pub async fn list_active_worktrees_command(
    state: State<'_, AppState>,
) -> Result<Vec<OrphanWorktree>, String> {
    let repo_path = {
        let active_proj = state.active_project.lock().await;
        match active_proj.as_ref() {
            Some(p) => p.path.clone(),
            None => return Ok(Vec::new()),
        }
    };

    GitService::list_active_worktrees(Path::new(&repo_path))
        .map_err(|e| format!("Failed to list worktrees: {e}"))
}

/// Handles recovery action for an orphaned worktree from a previous session.
#[tauri::command]
pub async fn recover_orphaned_worktree(
    task_id: String,
    action: String, // "review", "reject", "ignore"
    app_handle: AppHandle,
    state: State<'_, AppState>,
) -> Result<Option<ChangeInventory>, String> {
    let repo_path = {
        let active_proj = state.active_project.lock().await;
        active_proj
            .as_ref()
            .map(|p| p.path.clone())
            .ok_or_else(|| "No active project open".to_string())?
    };

    let p = Path::new(&repo_path);
    let canonical = p.canonicalize().unwrap_or_else(|_| p.to_path_buf());
    let clean = clean_path_for_git(&canonical);
    let wt_path = if let Some(parent) = clean.parent() {
        parent.join(".zencode-worktrees").join(&task_id)
    } else {
        clean.join(".zencode-worktrees").join(&task_id)
    };

    match action.as_str() {
        "review" => {
            let inv = GitService::get_worktree_diff(&wt_path)
                .map_err(|e| format!("Failed to inspect orphaned worktree: {e}"))?;

            {
                let mut worktrees = state.active_worktrees.lock().await;
                worktrees.insert(
                    task_id.clone(),
                    WorktreeInfo {
                        task_id: task_id.clone(),
                        branch_name: format!("zencode/task-{}", task_id),
                        worktree_path: wt_path.to_string_lossy().to_string(),
                        base_commit: inv.base_commit.clone(),
                        created_at: 0,
                    },
                );
                let mut inventories = state.active_task_inventories.lock().await;
                inventories.insert(task_id.clone(), inv.clone());
                let mut current_tid = state.current_task_id.lock().await;
                *current_tid = Some(task_id);
            }

            let _ = app_handle.emit("review:inventory_ready", &inv);
            {
                let mut task = state.task_state.lock().await;
                *task = TaskState::AwaitingReview;
            }
            let _ = app_handle.emit("worker:status", &TaskState::AwaitingReview);

            Ok(Some(inv))
        }
        "reject" => {
            GitService::reject_task_changes(&wt_path, &task_id)
                .map_err(|e| format!("Failed to clean orphaned worktree: {e}"))?;
            Ok(None)
        }
        "ignore" => Ok(None),
        other => Err(format!("Unknown recovery action: '{other}'")),
    }
}

/// Detects the build system for the active project or worktree and returns recommended commands.
#[tauri::command]
pub async fn detect_project_commands(
    state: State<'_, AppState>,
) -> Result<BuildCommandsInfo, String> {
    let target_dir = {
        let current_tid = state.current_task_id.lock().await;
        if let Some(ref tid) = *current_tid {
            let worktrees = state.active_worktrees.lock().await;
            if let Some(info) = worktrees.get(tid) {
                Some(PathBuf::from(&info.worktree_path))
            } else {
                None
            }
        } else {
            None
        }
    };

    let dir = match target_dir {
        Some(d) => d,
        None => {
            let active = state.active_project.lock().await;
            let proj = active.as_ref().ok_or_else(|| "No active project open".to_string())?;
            PathBuf::from(&proj.path)
        }
    };

    let bs = ExecutionService::detect_build_system(&dir);
    let name = bs.display_name().to_string();

    let recommended = match bs {
        BuildSystemType::Cargo => vec![
            RecommendedCommand {
                name: "Check".into(),
                program: "cargo".into(),
                args: vec!["check".into()],
                description: "Fast compile-check without code generation".into(),
            },
            RecommendedCommand {
                name: "Test".into(),
                program: "cargo".into(),
                args: vec!["test".into(), "--workspace".into()],
                description: "Run workspace test suite".into(),
            },
            RecommendedCommand {
                name: "Build".into(),
                program: "cargo".into(),
                args: vec!["build".into()],
                description: "Compile project binaries".into(),
            },
        ],
        BuildSystemType::Pnpm => vec![
            RecommendedCommand {
                name: "Test".into(),
                program: "pnpm".into(),
                args: vec!["test".into()],
                description: "Run package tests".into(),
            },
            RecommendedCommand {
                name: "Build".into(),
                program: "pnpm".into(),
                args: vec!["build".into()],
                description: "Compile production bundles".into(),
            },
        ],
        BuildSystemType::Npm => vec![
            RecommendedCommand {
                name: "Test".into(),
                program: "npm".into(),
                args: vec!["test".into()],
                description: "Run npm test script".into(),
            },
            RecommendedCommand {
                name: "Build".into(),
                program: "npm".into(),
                args: vec!["run".into(), "build".into()],
                description: "Run build script".into(),
            },
        ],
        BuildSystemType::Yarn => vec![
            RecommendedCommand {
                name: "Test".into(),
                program: "yarn".into(),
                args: vec!["test".into()],
                description: "Run tests".into(),
            },
            RecommendedCommand {
                name: "Build".into(),
                program: "yarn".into(),
                args: vec!["build".into()],
                description: "Run build".into(),
            },
        ],
        BuildSystemType::Go => vec![
            RecommendedCommand {
                name: "Test".into(),
                program: "go".into(),
                args: vec!["test".into(), "./...".into()],
                description: "Run Go test suite".into(),
            },
            RecommendedCommand {
                name: "Build".into(),
                program: "go".into(),
                args: vec!["build".into(), "./...".into()],
                description: "Build Go packages".into(),
            },
        ],
        BuildSystemType::Python => vec![
            RecommendedCommand {
                name: "Test".into(),
                program: "pytest".into(),
                args: vec![],
                description: "Run pytest".into(),
            },
        ],
        BuildSystemType::Unknown => vec![],
    };

    Ok(BuildCommandsInfo {
        build_system: bs,
        build_system_name: name,
        recommended_commands: recommended,
    })
}

/// Stages a command request for user approval. Does not execute the command.
#[tauri::command]
pub async fn request_command_approval(
    program: String,
    args: Vec<String>,
    cwd: Option<String>,
    description: String,
    state: State<'_, AppState>,
) -> Result<CommandRequest, String> {
    let resolved_cwd = match cwd {
        Some(c) if !c.trim().is_empty() => PathBuf::from(c),
        _ => {
            let current_tid = state.current_task_id.lock().await;
            if let Some(ref tid) = *current_tid {
                let worktrees = state.active_worktrees.lock().await;
                if let Some(info) = worktrees.get(tid) {
                    PathBuf::from(&info.worktree_path)
                } else {
                    let active = state.active_project.lock().await;
                    let proj = active.as_ref().ok_or_else(|| "No active project open".to_string())?;
                    PathBuf::from(&proj.path)
                }
            } else {
                let active = state.active_project.lock().await;
                let proj = active.as_ref().ok_or_else(|| "No active project open".to_string())?;
                PathBuf::from(&proj.path)
            }
        }
    };

    let tid_opt = {
        let current_tid = state.current_task_id.lock().await;
        current_tid.clone()
    };

    let arg_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
    let req = state
        .execution_service
        .create_approval_request(&program, &arg_refs, &resolved_cwd, &description, tid_opt.as_deref())
        .await;

    Ok(req)
}

/// Runs a previously approved command, streaming output line-by-line via Tauri events.
#[tauri::command]
pub async fn approve_and_run_command(
    command_id: String,
    app_handle: AppHandle,
    state: State<'_, AppState>,
) -> Result<ExecutionResult, String> {
    let key_opt = CredentialService::get_gemini_api_key().ok();
    state
        .execution_service
        .execute_command(&command_id, Some(&app_handle), key_opt.as_deref())
        .await
}

/// Cancels an in-flight command execution and cleanly kills its entire process tree.
#[tauri::command]
pub async fn cancel_command_execution(
    command_id: String,
    state: State<'_, AppState>,
) -> Result<bool, String> {
    let cancelled = state.execution_service.cancel_execution(&command_id).await;
    Ok(cancelled)
}

/// Exports a complete diagnostic bundle for the current or specified task with strict secret redaction.
#[tauri::command]
pub async fn export_diagnostic_bundle(
    task_id: Option<String>,
    state: State<'_, AppState>,
) -> Result<DiagnosticBundle, String> {
    let tid = match task_id {
        Some(t) if !t.trim().is_empty() => t,
        _ => {
            let cur = state.current_task_id.lock().await;
            cur.clone().unwrap_or_else(|| "diagnostic".to_string())
        }
    };

    let repo_path = {
        let active = state.active_project.lock().await;
        let proj = active.as_ref().ok_or_else(|| "No active project open".to_string())?;
        PathBuf::from(&proj.path)
    };

    let (_path, bundle) = DiagnosticService::export_bundle(&tid, &repo_path, &state).await?;
    Ok(bundle)
}

/// Previews the minimal context (error signature + relevant file excerpt) before consulting Gemini.
/// Does not call Gemini or any network endpoint.
#[tauri::command]
pub async fn preview_advisory_context(
    error_text: String,
    file_path: Option<String>,
    state: State<'_, AppState>,
) -> Result<AdvisoryPreview, String> {
    if error_text.trim().is_empty() {
        return Err("Error text cannot be empty".to_string());
    }

    let file_excerpt = if let Some(ref fp) = file_path {
        let p = Path::new(fp);
        if p.exists() {
            std::fs::read_to_string(p).unwrap_or_else(|_| "(unreadable file)".into())
        } else {
            let current_tid = state.current_task_id.lock().await;
            let resolved = if let Some(ref tid) = *current_tid {
                let wt_map = state.active_worktrees.lock().await;
                wt_map.get(tid).map(|w| Path::new(&w.worktree_path).join(fp))
            } else {
                let active = state.active_project.lock().await;
                active.as_ref().map(|proj| Path::new(&proj.path).join(fp))
            };

            if let Some(target) = resolved {
                if target.exists() {
                    std::fs::read_to_string(target).unwrap_or_else(|_| "(unreadable file)".into())
                } else {
                    format!("(file not found: {fp})")
                }
            } else {
                format!("(file path provided: {fp})")
            }
        }
    } else {
        "(no specific file excerpt provided)".to_string()
    };

    let key_opt = CredentialService::get_gemini_api_key().ok();
    let sanitized_error = redact_secrets(&error_text, key_opt.as_deref());
    let sanitized_excerpt = redact_secrets(&file_excerpt, key_opt.as_deref());

    let preview_id = format!(
        "adv_prev_{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()
    );

    let preview_payload = format!(
        "=== ERROR SIGNATURE ===\n{}\n\n=== RELEVANT FILE EXCERPT ===\n{}",
        sanitized_error, sanitized_excerpt
    );

    let preview = AdvisoryPreview {
        preview_id: preview_id.clone(),
        error_text: sanitized_error,
        file_path,
        file_excerpt: sanitized_excerpt,
        preview_payload,
    };

    {
        let mut previews = state.pending_advisory_previews.lock().await;
        previews.insert(preview_id, preview.clone());
    }

    Ok(preview)
}

/// Submits an advisory consultation request to Gemini with confirmed user consent.
/// Gemini returns an advisory suggestion only; it has zero filesystem/execution access.
#[tauri::command]
pub async fn submit_advisory_request(
    preview_id: String,
    confirmed: bool,
    app_handle: AppHandle,
    state: State<'_, AppState>,
) -> Result<String, String> {
    // 1. Enforce routing matrix permission
    Operation::AdvisoryErrorConsultation
        .enforce_gemini_permission(confirmed)
        .map_err(|e| format!("Advisory routing error: {e}"))?;

    // 2. Retrieve staged advisory preview
    let preview = {
        let mut previews = state.pending_advisory_previews.lock().await;
        previews.remove(&preview_id).ok_or_else(|| {
            format!("Advisory preview '{preview_id}' not found or already consumed.")
        })?
    };

    // 3. Fetch Gemini API key
    let api_key = CredentialService::get_gemini_api_key()
        .map_err(|e| format!("Failed to retrieve Gemini API key: {e}"))?;

    // 4. Dispatch through rate-limit manager and failover cascade
    let (suggestion, _model_id, _usage) = state
        .planner_client
        .request_advisory_suggestion(&api_key, &preview.error_text, &preview.file_excerpt, Some(&app_handle))
        .await?;

    Ok(suggestion)
}

/// Forwards an approved Gemini advisory recommendation to the active local OpenCode worker session,
/// prompting the local model (granite4.1:8b) to make a subsequent repair attempt.
#[tauri::command]
pub async fn send_advisory_to_worker(
    session_id: String,
    suggestion: String,
    app_handle: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let supervisor_guard = state.engine_supervisor.lock().await;
    let supervisor = supervisor_guard
        .as_ref()
        .ok_or_else(|| "OpenCode sidecar supervisor is not running.".to_string())?;

    // Update state to WorkerRunning so UI and backend reflect the new attempt
    {
        let mut task = state.task_state.lock().await;
        *task = TaskState::WorkerRunning {
            session_id: session_id.clone(),
        };
    }
    let _ = app_handle.emit(
        "worker:status",
        &TaskState::WorkerRunning {
            session_id: session_id.clone(),
        },
    );

    let prompt = format!(
        "Gemini Advisory Advice received:\n\n{}\n\nPlease incorporate this advice, make the necessary corrections using local tools, and run verification again.",
        suggestion
    );

    supervisor
        .send_prompt(&session_id, &prompt, "granite4.1:8b")
        .await
        .map_err(|e| format!("Failed to dispatch advisory advice to worker: {e}"))?;

    Ok(())
}

/// List recently opened workspaces from the local SQLite layer.
#[tauri::command]
pub async fn list_workspaces(
    state: State<'_, AppState>,
) -> Result<Vec<crate::db::WorkspaceRecord>, String> {
    state.db.get_recent_workspaces(20).map_err(|e| e.to_string())
}

/// Retrieve stored chats/tasks for a specific workspace.
#[tauri::command]
pub async fn list_chats(
    workspace_path: String,
    state: State<'_, AppState>,
) -> Result<Vec<crate::db::ChatSummaryRecord>, String> {
    state.db.list_chats_for_workspace(&workspace_path).map_err(|e| e.to_string())
}

/// Persist task/chat metadata and Git reference in SQLite (never store full file diffs).
#[tauri::command]
pub async fn save_chat(
    chat: crate::db::SaveChatRequest,
    state: State<'_, AppState>,
) -> Result<(), String> {
    state.db.save_chat(&chat).map_err(|e| e.to_string())
}

/// Load full scrollback, plan, and Git reference for a stored chat.
#[tauri::command]
pub async fn load_chat(
    task_id: String,
    state: State<'_, AppState>,
) -> Result<Option<crate::db::ChatDetailRecord>, String> {
    state.db.get_chat(&task_id).map_err(|e| e.to_string())
}

/// Delete a stored chat record.
#[tauri::command]
pub async fn delete_chat(
    task_id: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    state.db.delete_chat(&task_id).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_submit_plan_request_rejects_unconfirmed() {
        let app_state = AppState::new();

        // 1. Calling execute_submit_plan with confirmed = false MUST return an error immediately
        let result = execute_submit_plan("test_preview_123", false, &app_state, None).await;
        assert!(result.is_err(), "Must fail when confirmed == false");
        let err = result.unwrap_err();
        assert!(
            err.contains("confirmation required"),
            "Error message must specify confirmation required: {err}"
        );

        // 2. Direct routing matrix assertion
        let routing_check = Operation::CreateInitialPlan.enforce_gemini_permission(false);
        assert!(routing_check.is_err());
    }

    #[tokio::test]
    async fn test_command_execution_requires_prior_approval() {
        let app_state = AppState::new();
        // Trying to execute an unapproved command must fail hard
        let result = app_state
            .execution_service
            .execute_command("fake_cmd_9999", None, None)
            .await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.contains("has not been approved"), "Error was: {err}");
    }

    #[tokio::test]
    async fn test_command_approval_and_execution_lifecycle() {
        let app_state = AppState::new();
        let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap();

        // 1. Stage approval request
        let req = app_state
            .execution_service
            .create_approval_request(
                "cargo",
                &["check", "-p", "zencode-core"],
                repo_root,
                "Check zencode-core crate",
                None,
            )
            .await;

        assert_eq!(req.program, "cargo");
        assert_eq!(req.args, vec!["check", "-p", "zencode-core"]);

        // 2. Execute approved command
        let result = app_state
            .execution_service
            .execute_command(&req.id, None, None)
            .await;

        assert!(result.is_ok(), "Execution failed: {:?}", result.err());
        let exec = result.unwrap();
        assert_eq!(exec.exit_code, Some(0));
        assert!(exec.success);
        assert!(!exec.cancelled);

        // 3. Second execution attempt with same ID must fail (one-time approval)
        let second_result = app_state
            .execution_service
            .execute_command(&req.id, None, None)
            .await;
        assert!(second_result.is_err());
    }

    #[tokio::test]
    async fn test_command_cancellation_cleans_job() {
        let app_state = AppState::new();
        let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap();

        // Stage a long running test command
        let req = app_state
            .execution_service
            .create_approval_request(
                "cargo",
                &["test", "--workspace"],
                repo_root,
                "Run workspace tests",
                None,
            )
            .await;

        let cmd_id = req.id.clone();
        let exec_service = app_state.execution_service.clone();

        let handle = tokio::spawn(async move {
            exec_service.execute_command(&cmd_id, None, None).await
        });

        // Give it a brief moment to start up and spawn child processes
        tokio::time::sleep(Duration::from_millis(500)).await;

        // Cancel mid-run
        let was_cancelled = app_state.execution_service.cancel_execution(&req.id).await;
        assert!(was_cancelled, "cancel_execution should return true for running process");

        let result = handle.await.expect("task join failed");
        assert!(result.is_ok());
        let exec = result.unwrap();
        assert!(exec.cancelled, "ExecutionResult should reflect cancelled state");
        assert!(!exec.success);
    }

    #[tokio::test]
    async fn test_diagnostic_bundle_export_and_secret_redaction() {
        let app_state = AppState::new();
        let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap();

        // Store active project in app_state
        {
            let mut active = app_state.active_project.lock().await;
            *active = Some(ProjectState {
                path: repo_root.to_string_lossy().to_string(),
                name: "ZenCode".into(),
                project_type: "Rust".into(),
                is_git_repo: true,
                current_branch: Some("main".into()),
                git_status_summary: "Clean".into(),
                is_clean: true,
                has_gitignore: true,
                detected_editor_configs: vec!["rustfmt.toml".into()],
                has_commits: true,
            });
        }

        // Export bundle
        let (bundle_path, bundle) = DiagnosticService::export_bundle("test-task-1", repo_root, &app_state)
            .await
            .expect("Diagnostic bundle export should succeed");

        assert!(bundle_path.exists(), "Exported diagnostic JSON file must exist");
        assert_eq!(bundle.task_id, "test-task-1");

        // Verify zero secrets leaked
        assert!(!bundle.raw_json.contains("AIzaSy"));

        // Clean up test diagnostic file
        let _ = std::fs::remove_file(bundle_path);
    }

    #[tokio::test]
    async fn test_safe_auto_approved_allowlist_and_dangerous_command_enforcement() {
        let app_state = AppState::new();
        let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"));

        // 1. Safe command in allowlist: auto_approved = true
        let safe_req = app_state
            .execution_service
            .create_approval_request("cargo", &["check"], repo_root, "Safe check", Some("task-1"))
            .await;
        assert!(safe_req.auto_approved, "cargo check must be auto-approved");
        assert_eq!(safe_req.security_class, "SafeAutoApproved");

        // 2. Dangerous command: auto_approved = false, always requires approval
        let dangerous_req = app_state
            .execution_service
            .create_approval_request("cargo", &["install", "ripgrep"], repo_root, "Install pkg", Some("task-1"))
            .await;
        assert!(!dangerous_req.auto_approved, "Package installation must never be auto-approved");
        assert_eq!(dangerous_req.security_class, "AlwaysRequireApproval");

        // 3. Network touching git command: always requires approval
        let git_push_req = app_state
            .execution_service
            .create_approval_request("git", &["push", "origin", "main"], repo_root, "Push to remote", Some("task-1"))
            .await;
        assert!(!git_push_req.auto_approved, "git push must never be auto-approved");
        assert_eq!(git_push_req.security_class, "AlwaysRequireApproval");
    }

    #[tokio::test]
    async fn test_per_session_command_memory() {
        let app_state = AppState::new();
        let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let session = "task-test-session";

        // Initial run of custom build command not in allowlist
        let req1 = app_state
            .execution_service
            .create_approval_request("python", &["build_script.py"], repo_root, "Run script", Some(session))
            .await;
        assert!(!req1.auto_approved, "First execution of custom script must require approval");

        // Simulate user approval & execution -> session remembers it
        app_state
            .execution_service
            .remember_session_command(session, "python", &["build_script.py"])
            .await;

        // Subsequent identical command in same session -> auto-approved!
        let req2 = app_state
            .execution_service
            .create_approval_request("python", &["build_script.py"], repo_root, "Run script again", Some(session))
            .await;
        assert!(req2.auto_approved, "Identical command in same session must be auto-approved");

        // Different session -> still requires approval
        let req_diff_session = app_state
            .execution_service
            .create_approval_request("python", &["build_script.py"], repo_root, "Run script in diff session", Some("other-session"))
            .await;
        assert!(!req_diff_session.auto_approved, "Different session must not inherit approval");
    }

    #[tokio::test]
    async fn test_advisory_consultation_requires_explicit_consent() {
        let check_unconfirmed = Operation::AdvisoryErrorConsultation.enforce_gemini_permission(false);
        assert!(check_unconfirmed.is_err(), "Must reject unconfirmed advisory request");

        let check_confirmed = Operation::AdvisoryErrorConsultation.enforce_gemini_permission(true);
        assert!(check_confirmed.is_ok(), "Must allow confirmed advisory request");
    }

    #[tokio::test]
    async fn test_safe_project_detection_and_offer_based_actions() {
        let temp_dir = std::env::temp_dir().join(format!("zencode_test_{}", rand::random::<u64>()));
        let dir_path = temp_dir.as_path();
        std::fs::create_dir_all(dir_path).expect("tempdir creation");

        // 1. Setup mock Rust project with .editorconfig but NO .gitignore and NO git repo
        std::fs::write(dir_path.join("Cargo.toml"), "[package]\nname = \"demo\"\nversion = \"0.1.0\"\n").unwrap();
        std::fs::write(dir_path.join(".editorconfig"), "root = true\n").unwrap();

        let state = AppState::new();

        // 2. Test Safe read-only detection
        let proj = execute_open_project(&dir_path.to_string_lossy(), &state)
            .await
            .expect("open_project should succeed");

        assert_eq!(proj.project_type, "Rust");
        assert!(!proj.is_git_repo, "Must detect it is not yet a Git repo");
        assert!(!proj.has_gitignore, "Must detect absence of .gitignore without creating it");
        assert!(
            proj.detected_editor_configs.contains(&".editorconfig".to_string()),
            "Must detect .editorconfig without modifying it"
        );

        // 3. Test Offer-based .gitignore creation (Rust rules)
        let summary = execute_create_standard_gitignore(
            &dir_path.to_string_lossy(),
            &proj.project_type,
            &state,
        )
        .await
        .expect("create_standard_gitignore should succeed");

        assert!(summary.contains("Rust"));
        assert!(dir_path.join(".gitignore").exists());
        let gitignore_content = std::fs::read_to_string(dir_path.join(".gitignore")).unwrap();
        assert!(gitignore_content.contains("/target"));

        // 4. Calling again when already exists should be idempotent
        let second_call = execute_create_standard_gitignore(
            &dir_path.to_string_lossy(),
            &proj.project_type,
            &state,
        )
        .await
        .expect("second call should be ok");
        assert!(second_call.contains("already exists"));

        // 5. Test Offer-based Git Init
        let initialized = execute_init_git_repo(
            &dir_path.to_string_lossy(),
            &state,
        )
        .await
        .expect("init_git_repo should succeed");

        assert!(initialized.is_git_repo, "Must now be a valid Git repository");
        assert!(dir_path.join(".git").exists(), ".git directory must exist");
        assert!(initialized.has_gitignore, ".gitignore must be detected as present now");

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_parse_worker_sse_event_completion_detection() {
        let sid = "ses_test_123";

        // 1. Tool call turn must NOT signal completion
        let tool_call_data = r#"{"id":"evt_1","type":"message.part.updated","properties":{"sessionID":"ses_test_123","part":{"id":"p1","reason":"tool-calls","type":"step-finish"}}}"#;
        let (t1, c1) = parse_worker_sse_event(tool_call_data, sid);
        assert_eq!(t1, "message.part.updated");
        assert!(!c1, "Tool call turn must NOT trigger completion");

        // 2. Final response turn with reason stop MUST signal completion
        let stop_data = r#"{"id":"evt_2","type":"message.part.updated","properties":{"sessionID":"ses_test_123","part":{"id":"p2","reason":"stop","type":"step-finish"}}}"#;
        let (t2, c2) = parse_worker_sse_event(stop_data, sid);
        assert_eq!(t2, "message.part.updated");
        assert!(c2, "step-finish with reason: stop MUST trigger completion");

        // 3. message.updated with finish stop MUST signal completion
        let finish_data = r#"{"id":"evt_3","type":"message.updated","properties":{"sessionID":"ses_test_123","info":{"finish":"stop"}}}"#;
        let (t3, c3) = parse_worker_sse_event(finish_data, sid);
        assert_eq!(t3, "message.updated");
        assert!(c3, "info.finish: stop MUST trigger completion");

        // 4. Session status idle MUST signal completion
        let idle_data = r#"{"id":"evt_4","type":"session.status","properties":{"sessionID":"ses_test_123","status":{"type":"idle"}}}"#;
        let (t4, c4) = parse_worker_sse_event(idle_data, sid);
        assert_eq!(t4, "session.status");
        assert!(c4, "session.status idle MUST trigger completion");

        // 5. Different session stop event must NOT complete this session
        let other_session_data = r#"{"id":"evt_5","type":"message.part.updated","properties":{"sessionID":"other_session_999","part":{"reason":"stop","type":"step-finish"}}}"#;
        let (t5, c5) = parse_worker_sse_event(other_session_data, sid);
        assert_eq!(t5, "message.part.updated");
        assert!(!c5, "Different session stop event must NOT complete active session");
    }
}

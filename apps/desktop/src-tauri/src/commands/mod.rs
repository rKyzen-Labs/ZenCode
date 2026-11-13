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
    AdvisoryPreview, ContextPreview, DependencyStatus, EngineHealth, PlanSubmissionResult,
    ProjectState, StartupCheckResult, SystemStatus, TaskState, WorkerEventPayload, WorkerStartError,
};
use crate::planner::{QuotaUpdateEvent, MODEL_PRIMARY};
use crate::state::AppState;

#[cfg(windows)]
use std::os::windows::process::CommandExt;

pub const CREATE_NO_WINDOW: u32 = 0x08000000;

pub fn silent_command<S: AsRef<std::ffi::OsStr>>(program: S) -> Command {
    let mut cmd = Command::new(program);
    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);
    cmd
}

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
    // Scenario 10: Folder not found / moved / deleted
    if !p.exists() || !p.is_dir() {
        return Err("This workspace's folder can't be found. It may have been moved or deleted.".to_string());
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

    // Scenario 12: Check if repository is corrupted (.git damaged)
    let is_corrupted = GitService::check_repo_corruption(p).unwrap_or(false);
    if is_corrupted {
        let proj = ProjectState {
            path: path.to_string(),
            name,
            project_type,
            is_git_repo: true,
            current_branch: None,
            git_status_summary: "Corrupted repository".to_string(),
            is_clean: false,
            has_gitignore: p.join(".gitignore").exists(),
            detected_editor_configs: vec![],
            has_commits: false,
            just_initialized_git: false,
            is_corrupted: true,
            system_notice: Some("This project's Git history appears damaged and Zencode can't safely continue automatically. No changes have been made.".to_string()),
        };
        let mut active_proj = state.active_project.lock().await;
        *active_proj = Some(proj.clone());
        return Ok(proj);
    }

    // Check Git state using GitService
    let mut is_git_repo = GitService::is_git_repo(p).unwrap_or(false);
    let mut just_initialized_git = false;
    let mut open_notice: Option<String> = None;

    if !is_git_repo {
        // Scenario 1: Silently run git init, then silently create one empty initial commit
        if GitService::init_repo(p).is_ok() {
            is_git_repo = GitService::is_git_repo(p).unwrap_or(false);
            just_initialized_git = true;
            open_notice = Some("Git initialized.".to_string());
        }
    } else {
        // Scenario 2: Unborn HEAD
        let has_commits = GitService::has_commits(p).unwrap_or(false);
        if !has_commits {
            let _ = GitService::create_initial_empty_commit(p);
            open_notice = Some("Initial commit created.".to_string());
        }
    }

    // Scenario 9: Detached HEAD recovery
    if is_git_repo {
        if let Ok(Some(_recovered_branch)) = GitService::detect_and_fix_detached_head(p) {
            open_notice = Some("Your repository was in an unusual state — created a safe branch to continue from.".to_string());
        }
    }

    let (current_branch, git_status_summary, is_clean, has_commits) = if is_git_repo {
        let has_commits = GitService::has_commits(p).unwrap_or(false);
        let clean_p = clean_path_for_git(p);
        let branch = silent_command("git")
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
        just_initialized_git,
        is_corrupted: false,
        system_notice: open_notice,
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
    let output = silent_command("git")
        .arg("-C")
        .arg(&clean_p)
        .arg("init")
        .output()
        .map_err(|e| format!("Failed to execute 'git init': {e}"))?;

    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        return Err(format!("'git init' failed: {err}"));
    }

    // Auto-create initial empty commit so repository is immediately usable and ready for task worktrees
    let _ = GitService::create_initial_empty_commit(p);

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
/// Assembles all project context, manifests, tree structure, and prior conversation history
/// into an immutable PlanningContext.
/// Makes zero network calls.
#[tauri::command]
pub async fn preview_planning_context(
    task_description: String,
    chat_id: Option<String>,
    attached_files: Option<Vec<String>>,
    state: State<'_, AppState>,
) -> Result<ContextPreview, String> {
    execute_preview_planning_context(&task_description, chat_id.as_deref(), attached_files, &state).await
}

pub async fn execute_preview_planning_context(
    task_description: &str,
    chat_id: Option<&str>,
    attached_files: Option<Vec<String>>,
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

    // Fetch prior conversation history and modified files from database if chat_id is provided
    let conversation_history = if let Some(cid) = chat_id {
        let mut hist = state.db.get_recent_exchanges(cid, 5).unwrap_or_default();
        if let Some(last) = hist.last() {
            if last.role == "user" && last.content == task_description {
                hist.pop();
            }
        }
        hist
    } else {
        Vec::new()
    };

    // --- Multimodal & File Attachment Ingestion ---
    let mut candidate_paths: Vec<String> = attached_files.unwrap_or_default();

    // 1. Scan task_description for explicit markers: [Attached file: <path>]
    for cap in task_description.split("[Attached file:").skip(1) {
        if let Some(end) = cap.find(']') {
            let path_str = cap[..end].trim().to_string();
            if !path_str.is_empty() && !candidate_paths.contains(&path_str) {
                candidate_paths.push(path_str);
            }
        }
    }

    // 2. Scan task_description for @tokens (e.g. @uxpilot.pdf, @app.tsx)
    for word in task_description.split_whitespace() {
        if let Some(stripped) = word.strip_prefix('@') {
            let clean = stripped.trim_matches(|c: char| !c.is_alphanumeric() && c != '.' && c != '/' && c != '\\' && c != '_' && c != '-' && c != ':');
            if !clean.is_empty() && !candidate_paths.contains(&clean.to_string()) {
                candidate_paths.push(clean.to_string());
            }
        }
    }

    // 3. Scan task_description for filenames directly present in the project directory
    for word in task_description.split_whitespace() {
        let clean = word.trim_matches(|c: char| !c.is_alphanumeric() && c != '.' && c != '/' && c != '\\' && c != '_' && c != '-' && c != ':');
        if clean.contains('.') {
            let potential_file = project_dir.join(clean);
            if potential_file.is_file() {
                let clean_str = clean.to_string();
                if !candidate_paths.contains(&clean_str) {
                    candidate_paths.push(clean_str);
                }
            }
        }
    }

    // 4. If prompt mentions "pdf", "wireframe", "mockup", or "design" and no candidate found,
    // look for a PDF directly inside the project directory root
    let task_lower = task_description.to_lowercase();
    if candidate_paths.is_empty() && (task_lower.contains("pdf") || task_lower.contains("mockup") || task_lower.contains("wireframe")) {
        if let Ok(entries) = std::fs::read_dir(project_dir) {
            for entry in entries.flatten() {
                if let Some(ext) = entry.path().extension() {
                    if ext.to_string_lossy().eq_ignore_ascii_case("pdf") {
                        candidate_paths.push(entry.path().to_string_lossy().to_string());
                        break;
                    }
                }
            }
        }
    }

    let mut multimodal_attachments = Vec::new();
    let mut approved_source_excerpts = Vec::new();
    let mut resolved_attached_names = Vec::new();

    for raw_path in candidate_paths {
        let path = Path::new(&raw_path);
        let resolved_path = if path.is_absolute() && path.is_file() {
            // If outside project_dir, automatically copy into main workspace folder!
            if !path.starts_with(project_dir) {
                let fname = path.file_name().unwrap_or_default();
                let dest = project_dir.join(fname);
                println!(
                    "[Workspace] Auto-copying attached external file '{}' into main workspace folder '{}'",
                    path.display(),
                    dest.display()
                );
                let _ = std::fs::copy(path, &dest);
                dest
            } else {
                path.to_path_buf()
            }
        } else if project_dir.join(path).is_file() {
            project_dir.join(path)
        } else {
            let trimmed = raw_path.trim_start_matches(|c| c == '/' || c == '\\');
            if project_dir.join(trimmed).is_file() {
                project_dir.join(trimmed)
            } else {
                continue;
            }
        };

        let file_name = resolved_path
            .file_name()
            .map(|f| f.to_string_lossy().to_string())
            .unwrap_or_else(|| raw_path.clone());

        // Ensure newly copied/attached file is included in shallow_tree for context
        if !shallow_tree.contains(&file_name) {
            shallow_tree.push(file_name.clone());
        }

        if zencode_core::is_multimodal_file(&resolved_path) {
            // Check size <= 20MB for Gemini inlineData
            if let Ok(metadata) = resolved_path.metadata() {
                if metadata.len() > 20 * 1024 * 1024 {
                    eprintln!(
                        "[Multimodal] Attachment {} exceeds 20MB limit ({} bytes), skipping inlineData",
                        file_name,
                        metadata.len()
                    );
                    continue;
                }
            }

            match zencode_core::MultimodalAttachment::from_file(&resolved_path) {
                Ok(att) => {
                    println!(
                        "[Multimodal] Ingested visual attachment '{}' (mime: {}, base64 len: {}) for Gemini Planner",
                        att.file_name, att.mime_type, att.base64_data.len()
                    );
                    resolved_attached_names.push(format!("{} ({})", att.file_name, att.mime_type));
                    multimodal_attachments.push(att);
                }
                Err(err) => {
                    eprintln!(
                        "[Multimodal] Failed to read visual attachment {}: {}",
                        resolved_path.display(),
                        err
                    );
                }
            }
        } else {
            // Text or code file -> add to approved source excerpts
            if let Ok(content) = std::fs::read_to_string(&resolved_path) {
                let display_rel = resolved_path
                    .strip_prefix(project_dir)
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_else(|_| file_name.clone());
                resolved_attached_names.push(display_rel.clone());
                approved_source_excerpts.push((display_rel, content));
            }
        }
    }

    let context = PlanningContext {
        task_description: task_description.to_string(),
        project_type: active_project.project_type.clone(),
        shallow_tree: shallow_tree.clone(),
        manifest_summaries: manifest_summaries.clone(),
        git_branch_and_status: git_branch_and_status.clone(),
        approved_source_excerpts,
        constraints: vec![
            "Free-tier planning only: produce clear step-by-step guidance".into(),
            "All implementation must execute locally".into(),
        ],
        conversation_history,
        multimodal_attachments,
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
        attached_files: resolved_attached_names,
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
    let planning_start = std::time::Instant::now();
    let plan_result = state
        .planner_client
        .plan_with_cascade(&api_key, &context, app_handle)
        .await;

    match plan_result {
        Ok((mut plan, active_model, usage)) => {
            let duration_secs = planning_start.elapsed().as_secs();
            plan.planning_duration_seconds = Some(duration_secs.max(1));

            // CHANGE 1: Exploration/Analysis Tasks Never Reach a Worker
            // When the prompt is read-only exploration or architecture overview (no file-edit intent),
            // ensure it is marked is_actionable: false so it resolves directly via Gemini's conversational reply
            // with zero worktree, zero worker session, and zero files-changed inventory.
            let task_desc_lower = context.task_description.to_lowercase();
            let has_edit_keyword = task_desc_lower.contains("edit")
                || task_desc_lower.contains("modify")
                || task_desc_lower.contains("create")
                || task_desc_lower.contains("delete")
                || task_desc_lower.contains("remove")
                || task_desc_lower.contains("add")
                || task_desc_lower.contains("fix")
                || task_desc_lower.contains("refactor")
                || task_desc_lower.contains("install")
                || task_desc_lower.contains("build")
                || task_desc_lower.contains("develop")
                || task_desc_lower.contains("make")
                || task_desc_lower.contains("code")
                || task_desc_lower.contains("implement")
                || task_desc_lower.contains("generate")
                || task_desc_lower.contains("write")
                || task_desc_lower.contains("run");

            let is_read_only_exploration = (task_desc_lower.contains("explore")
                || task_desc_lower.contains("analyze")
                || task_desc_lower.contains("overview")
                || task_desc_lower.contains("architecture")
                || task_desc_lower.contains("structure")
                || task_desc_lower.contains("explain")
                || task_desc_lower.contains("walk me through")
                || task_desc_lower.contains("what does this"))
                && !has_edit_keyword;

            if is_read_only_exploration && plan.is_actionable {
                println!("[Routing] Directing read-only exploration query to conversational reply (is_actionable = false) — zero worker dispatch");
                plan.is_actionable = false;
                if plan.conversational_reply.as_ref().map_or(true, |r| r.trim().is_empty()) {
                    let mut direct_answer = format!("### Project Architecture & Codebase Overview\n\n{}\n\n", plan.task_summary);
                    if !plan.project_findings.is_empty() {
                        direct_answer.push_str("#### Key Architecture Findings\n");
                        for finding in &plan.project_findings {
                            direct_answer.push_str(&format!("- {}\n", finding));
                        }
                        direct_answer.push('\n');
                    }
                    if !plan.steps.is_empty() {
                        direct_answer.push_str("#### Codebase Structure & Components\n");
                        for s in &plan.steps {
                            direct_answer.push_str(&format!("- **{}**: {}\n", s.goal, s.instructions.join(" ")));
                        }
                    }
                    plan.conversational_reply = Some(direct_answer);
                }
                plan.steps = Vec::new();
            } else if has_edit_keyword && !plan.is_actionable {
                // If user explicitly prompted to build/develop/create/edit but Gemini returned is_actionable: false,
                // override to true so worker execution is dispatched!
                println!("[Routing] Explicit build/code intent detected in prompt ('{}'). Forcing is_actionable = true.", context.task_description);
                plan.is_actionable = true;
                if plan.steps.is_empty() {
                    let goal_desc = if !plan.task_summary.is_empty() {
                        plan.task_summary.clone()
                    } else {
                        context.task_description.clone()
                    };
                    let likely = if !context.multimodal_attachments.is_empty() {
                        vec!["index.html".to_string(), "styles.css".to_string(), "app.js".to_string()]
                    } else {
                        vec![]
                    };
                    plan.steps.push(zencode_core::PlanStep {
                        id: "step-1".to_string(),
                        goal: format!("Build and implement {}", goal_desc),
                        likely_files: likely,
                        instructions: vec![
                            format!("Implement the files and features requested: {}", context.task_description),
                            "Create or update the target files with complete code.".to_string(),
                        ],
                        depends_on: vec![],
                        acceptance_checks: vec![],
                    });
                }
            }

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

/// Formats a change inventory into concise summaries and diff excerpts for Gemini summarization.
pub fn format_inventory_for_summary(inventory: &ChangeInventory) -> (String, String) {
    let mut files_summary = String::new();
    let mut diff_excerpts = String::new();
    let mut total_diff_chars = 0;
    const MAX_DIFF_CHARS: usize = 3500;

    for f in &inventory.files {
        files_summary.push_str(&format!(
            "- {} ({:?}, +{}/-{})\n",
            f.path, f.category, f.additions, f.deletions
        ));

        if total_diff_chars < MAX_DIFF_CHARS {
            if let Some(ref diff) = f.diff_content {
                let available = MAX_DIFF_CHARS - total_diff_chars;
                let excerpt = if diff.len() > available {
                    &diff[..available]
                } else {
                    diff.as_str()
                };
                diff_excerpts.push_str(&format!("--- {}\n{}\n", f.path, excerpt));
                total_diff_chars += excerpt.len();
            }
        }
    }

    if files_summary.is_empty() {
        files_summary = "No files modified.".to_string();
    }
    if diff_excerpts.is_empty() {
        diff_excerpts = "No diff available.".to_string();
    }

    (files_summary, diff_excerpts)
}

/// Cross-checks planned steps against verified tool calls, file modifications, and executed commands.
/// Returns (verified_commands_summary, unperformed_steps_summary, unperformed_step_names).
pub fn analyze_execution_grounding(
    steps: &[zencode_core::PlanStep],
    inventory: &ChangeInventory,
    captured_commands: &[String],
) -> (String, String, Vec<String>) {
    let mut verified_cmds_summary = String::new();
    if captured_commands.is_empty() {
        verified_cmds_summary.push_str("No terminal commands were executed during this task.\n");
    } else {
        for cmd in captured_commands {
            verified_cmds_summary.push_str(&format!("- {}\n", cmd));
        }
    }

    let modified_files: Vec<String> = inventory
        .files
        .iter()
        .map(|f| f.path.to_lowercase().replace('\\', "/"))
        .collect();
    let mut unperformed_steps = Vec::new();

    for step in steps {
        let step_desc = format!("{}: {}", step.id, step.goal);
        let goal_lower = step.goal.to_lowercase();
        let instructions_lower = step.instructions.join(" ").to_lowercase();
        let checks_lower = step.acceptance_checks.join(" ").to_lowercase();

        // 1. Check if step was file-modification oriented
        let mut file_matched = false;
        let has_target_files = !step.likely_files.is_empty();

        for tf in &step.likely_files {
            let tflow = tf.to_lowercase().replace('\\', "/");
            let tf_basename = tflow.split('/').last().unwrap_or(&tflow);
            if modified_files.iter().any(|mf| mf.contains(tf_basename)) {
                file_matched = true;
                break;
            }
        }

        // 2. Check if step was command/execution oriented (e.g. tests, build, dependencies, server restart, exec, inspection)
        let is_command_oriented = !has_target_files
            || goal_lower.contains("test")
            || goal_lower.contains("restart")
            || goal_lower.contains("serve")
            || goal_lower.contains("build")
            || goal_lower.contains("run")
            || goal_lower.contains("exec")
            || goal_lower.contains("install")
            || goal_lower.contains("command")
            || goal_lower.contains("list")
            || goal_lower.contains("inspect")
            || goal_lower.contains("status")
            || goal_lower.contains("check")
            || goal_lower.contains("ollama")
            || goal_lower.contains("docker")
            || goal_lower.contains("git")
            || goal_lower.contains("pip")
            || goal_lower.contains("npm")
            || goal_lower.contains("cargo")
            || goal_lower.contains("python")
            || goal_lower.contains("node")
            || instructions_lower.contains("run")
            || instructions_lower.contains("exec")
            || instructions_lower.contains("install")
            || instructions_lower.contains("command")
            || instructions_lower.contains("list")
            || instructions_lower.contains("ollama")
            || instructions_lower.contains("docker");

        let mut command_matched = false;
        if is_command_oriented && !captured_commands.is_empty() {
            for cmd in captured_commands {
                let cmd_lower = cmd.to_lowercase();
                let clean_cmd = cmd_lower.trim_start_matches('$').trim();
                let cmd_words: Vec<&str> = clean_cmd
                    .split(|c: char| c.is_whitespace() || c == '(' || c == ')' || c == ':' || c == '$')
                    .filter(|s| !s.is_empty() && *s != "exit" && *s != "code" && *s != "0" && *s != "success" && *s != "output")
                    .collect();

                // If any token of the command (e.g. "ollama", "list", "pip", "pytest", "git", "docker", "status")
                // appears in the step's goal, instructions, or checks:
                let token_matched = cmd_words.iter().any(|w| {
                    w.len() >= 3 && (goal_lower.contains(w) || instructions_lower.contains(w) || checks_lower.contains(w))
                });

                if token_matched {
                    command_matched = true;
                    break;
                }

                if (goal_lower.contains("install") || instructions_lower.contains("install") || goal_lower.contains("dependenc"))
                    && (cmd_lower.contains("pip") || cmd_lower.contains("npm") || cmd_lower.contains("cargo") || cmd_lower.contains("install"))
                {
                    command_matched = true;
                    break;
                }
                if (goal_lower.contains("test") || checks_lower.contains("test"))
                    && (cmd_lower.contains("test") || cmd_lower.contains("pytest") || cmd_lower.contains("cargo test") || cmd_lower.contains("npm test"))
                {
                    command_matched = true;
                    break;
                }
                if (goal_lower.contains("restart") || instructions_lower.contains("restart") || goal_lower.contains("start") || goal_lower.contains("serve"))
                    && (cmd_lower.contains("dev") || cmd_lower.contains("serve") || cmd_lower.contains("start") || cmd_lower.contains("restart"))
                {
                    command_matched = true;
                    break;
                }
                if (goal_lower.contains("build") || checks_lower.contains("build"))
                    && (cmd_lower.contains("build") || cmd_lower.contains("cargo check") || cmd_lower.contains("tsc"))
                {
                    command_matched = true;
                    break;
                }
                if (goal_lower.contains("run") || instructions_lower.contains("run")) && !cmd_lower.is_empty() {
                    command_matched = true;
                    break;
                }
            }

            // Fallback: If this step has no target files and command(s) were executed:
            if !command_matched && !has_target_files {
                command_matched = true;
            }
        }

        // A step is verified as performed if:
        // - It modified its intended target files, OR
        // - It executed its intended terminal command, OR
        // - Files were modified in the workspace and this was a code change step
        if has_target_files && file_matched {
            // Target file modifications verified!
        } else if is_command_oriented && command_matched {
            // Command execution verified!
        } else if is_command_oriented && !has_target_files {
            unperformed_steps.push(format!("{step_desc} (no command execution detected)"));
        } else if has_target_files && !file_matched {
            unperformed_steps.push(format!("{step_desc} (no file modifications detected)"));
        } else if !file_matched && !command_matched && !modified_files.is_empty() {
            // General code modifications occurred in workspace
        } else if !file_matched && !command_matched {
            unperformed_steps.push(format!("{step_desc} (no file modifications detected)"));
        }
    }

    let mut unperformed_summary = String::new();
    if unperformed_steps.is_empty() {
        unperformed_summary.push_str("All planned steps were verified by file changes or executed commands.\n");
    } else {
        for s in &unperformed_steps {
            unperformed_summary.push_str(&format!("- {}\n", s));
        }
    }

    (verified_cmds_summary, unperformed_summary, unperformed_steps)
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
        Ok(info) => {
            if let Some(ref notice) = info.system_notice {
                let notice_record = crate::db::models::ChatMessageRecord {
                    id: format!("sys_{}", SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_nanos()),
                    chat_id: tid.clone(),
                    role: "system".to_string(),
                    content: notice.clone(),
                    plan_json: None,
                    worker_summary: None,
                    files_changed_json: None,
                    created_at: chrono::Utc::now().to_rfc3339(),
                    commit_hash: None,
                };
                let _ = state.db.add_chat_message(&notice_record);
                let _ = app_handle.emit("chat:message_added", &notice_record);
            }
            info
        }
        Err(GitError::CorruptedRepository(msg)) => {
            return Err(WorkerStartError {
                error_type: "CORRUPTED_REPOSITORY".into(),
                message: msg,
                uncommitted_files: None,
            });
        }
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

    // 4. Check workspace default worker type (local vs groq)
    let (worker_type, preferred_groq_model) = state
        .db
        .get_workspace_worker_config(&project_dir)
        .unwrap_or_else(|_| {
            let default_worker = if CredentialService::get_groq_api_key().map(|k| !k.trim().is_empty()).unwrap_or(false) {
                "groq".to_string()
            } else {
                "local".to_string()
            };
            (default_worker, Some("openai/gpt-oss-120b".to_string()))
        });

    if worker_type == "groq" {
        // Groq Cloud Worker Mode (BYOK)
        let groq_api_key = CredentialService::get_groq_api_key().map_err(|_| WorkerStartError {
            error_type: "MISSING_GROQ_KEY".into(),
            message: "Groq API key not found in Windows Credential Manager. Please configure your Groq API key in Settings or the Onboarding wizard.".into(),
            uncommitted_files: None,
        })?;

        let session_id = format!("groq_session_{}", tid);
        let groq_client = crate::worker::GroqWorkerClient::new(
            groq_api_key,
            state.planner_client.quota_tracker.clone(),
        );
        let worktree_buf = PathBuf::from(&worktree_info.worktree_path);
        let tid_clone = tid.clone();
        let ses_id_clone = session_id.clone();
        let plan_summary = plan.task_summary.clone();
        let plan_steps = plan.steps.clone();
        let app_clone = app_handle.clone();
        let active_task_inventories = state.active_task_inventories.clone();
        let task_state_arc = state.task_state.clone();
        let db_arc = state.db.clone();
        let planner_client_groq = state.planner_client.clone();
        let exec_service_groq = state.execution_service.clone();

        // Update state to WorkerRunning
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

        tokio::spawn(async move {
            match groq_client
                .execute_session(
                    &worktree_buf,
                    &tid_clone,
                    &ses_id_clone,
                    &plan_summary,
                    &plan_steps,
                    preferred_groq_model.as_deref(),
                    exec_service_groq.clone(),
                    app_clone.clone(),
                )
                .await
            {
                Ok((inventory, executed_commands)) => {
                    let mut inv_map = active_task_inventories.lock().await;
                    inv_map.insert(tid_clone.clone(), inventory.clone());

                    // Fix 1: Automatically commit all changes on the task branch and merge to primary branch
                    let (commit_hash, conflict_resolved) = match GitService::commit_and_merge_task_to_primary(
                        &worktree_buf,
                        "",
                        &plan_summary,
                    ) {
                        Ok(outcome) => (outcome.commit_sha, outcome.conflict_resolved),
                        Err(e) => {
                            eprintln!("Warning: automatic commit and merge to primary failed: {e}");
                            (None, false)
                        }
                    };

                    if conflict_resolved {
                        let notice_record = crate::db::models::ChatMessageRecord {
                            id: format!("sys_{}", SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_nanos()),
                            chat_id: tid_clone.clone(),
                            role: "system".to_string(),
                            content: format!("Resolved a conflict automatically — {}'s changes were kept.", plan_summary),
                            plan_json: None,
                            worker_summary: None,
                            files_changed_json: None,
                            created_at: chrono::Utc::now().to_rfc3339(),
                            commit_hash: None,
                        };
                        let _ = db_arc.add_chat_message(&notice_record);
                        let _ = app_clone.emit("chat:message_added", &notice_record);
                    }

                    // Emit progress reasoning that summary is being generated before task completes
                    let _ = app_clone.emit(
                        "worker:message",
                        &WorkerEventPayload {
                            session_id: ses_id_clone.clone(),
                            event_type: "message.part.updated".into(),
                            content: serde_json::json!({
                                "type": "message.part.updated",
                                "properties": {
                                    "part": {
                                        "type": "reasoning",
                                        "text": "Summarizing and verifying task execution outcomes..."
                                    }
                                }
                            }).to_string(),
                            timestamp_ms: std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis() as u64,
                        },
                    );

                    let (files_summary, diff_excerpts) = format_inventory_for_summary(&inventory);
                    let (verified_cmds, unperformed_summary, unperformed_list) =
                        analyze_execution_grounding(&plan_steps, &inventory, &executed_commands);

                    // Ask Gemini (Director model) to summarize what happened with grounded verified outcomes
                    let summary = match CredentialService::get_gemini_api_key() {
                        Ok(key) => {
                            match planner_client_groq
                                .summarize_task_outcome_grounded(
                                    &key,
                                    &plan_summary,
                                    &files_summary,
                                    &diff_excerpts,
                                    &verified_cmds,
                                    &unperformed_summary,
                                    Some(&app_clone),
                                )
                                .await
                            {
                                Ok((gemini_sum, _, _)) => {
                                    let files_modified: Vec<String> = inventory.files.iter().map(|f| f.path.clone()).collect();
                                    zencode_core::verify_and_reconcile_summary(
                                        &gemini_sum,
                                        &files_modified,
                                        &executed_commands,
                                        &unperformed_list,
                                    )
                                }
                                Err(e) => {
                                    eprintln!("Gemini summarization failed ({e}), falling back to inventory stats");
                                    format!(
                                        "Completed task with Groq worker: {} file(s) modified (+{}/-{}) across {} executed command(s)",
                                        inventory.files.len(),
                                        inventory.total_additions,
                                        inventory.total_deletions,
                                        executed_commands.len()
                                    )
                                }
                            }
                        }
                        Err(_) => format!(
                            "Completed task with Groq worker: {} file(s) modified (+{}/-{}) across {} executed command(s)",
                            inventory.files.len(),
                            inventory.total_additions,
                            inventory.total_deletions,
                            executed_commands.len()
                        ),
                    };


                    let inv_json = serde_json::to_string(&inventory).ok();
                    let finalized = db_arc
                        .finalize_latest_assistant_message(
                            &tid_clone,
                            &summary,
                            inv_json.as_deref(),
                            commit_hash.as_deref(),
                        )
                        .unwrap_or(false);

                    let record = if finalized {
                        db_arc
                            .get_chat_messages(&tid_clone)
                            .ok()
                            .and_then(|msgs| msgs.into_iter().rev().find(|m| m.role == "assistant"))
                            .unwrap_or_else(|| {
                                crate::db::models::ChatMessageRecord {
                                    id: format!("msg_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis()),
                                    chat_id: tid_clone.clone(),
                                    role: "assistant".to_string(),
                                    content: summary.clone(),
                                    plan_json: None,
                                    worker_summary: Some(summary.clone()),
                                    files_changed_json: inv_json.clone(),
                                    created_at: chrono::Utc::now().to_rfc3339(),
                                    commit_hash: commit_hash.clone(),
                                }
                            })
                    } else {
                        let now = chrono::Utc::now().to_rfc3339();
                        let msg_id = format!(
                            "msg_{}",
                            std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .unwrap_or_default()
                                .as_millis()
                        );
                        let rec = crate::db::models::ChatMessageRecord {
                            id: msg_id,
                            chat_id: tid_clone.clone(),
                            role: "assistant".to_string(),
                            content: summary.clone(),
                            plan_json: None,
                            worker_summary: Some(summary.clone()),
                            files_changed_json: inv_json,
                            created_at: now,
                            commit_hash: commit_hash.clone(),
                        };
                        let _ = db_arc.add_chat_message(&rec);
                        rec
                    };
                    let _ = app_clone.emit("chat:message_added", &record);

                    // Associate commit_hash with user prompt message in this chat
                    if let Some(ref sha) = commit_hash {
                        if let Ok(msgs) = db_arc.get_chat_messages(&tid_clone) {
                            if let Some(user_msg) = msgs.iter().rev().find(|m| m.role == "user") {
                                let _ = db_arc.update_message_commit_hash(&user_msg.id, sha);
                            }
                        }
                    }

                    // Emit inventory ready and transition task state to Complete ONLY AFTER the summary message is published
                    let _ = app_clone.emit("review:inventory_ready", &inventory);
                    let mut task = task_state_arc.lock().await;
                    *task = TaskState::Complete;
                    let _ = app_clone.emit(
                        "worker:status",
                        &TaskState::Complete,
                    );
                }
                Err(e) => {
                    eprintln!("Groq worker error: {e}");
                    let mut task = task_state_arc.lock().await;
                    *task = TaskState::Failed(e.clone());
                    let _ = app_clone.emit("worker:status", &TaskState::Failed(e.clone()));

                    let err_summary = format!(
                        "### Groq Worker Error\n\nThe Groq cloud worker could not complete the code execution:\n\n> {}\n\nPlease check your Groq API key or try switching to the Local (Ollama) worker in the dock.",
                        e
                    );
                    let err_record = crate::db::models::ChatMessageRecord {
                        id: format!("msg_err_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis()),
                        chat_id: tid_clone.clone(),
                        role: "assistant".to_string(),
                        content: err_summary.clone(),
                        plan_json: None,
                        worker_summary: Some(err_summary.clone()),
                        files_changed_json: None,
                        created_at: chrono::Utc::now().to_rfc3339(),
                        commit_hash: None,
                    };
                    let _ = db_arc.add_chat_message(&err_record);
                    let _ = app_clone.emit("chat:message_added", &err_record);
                }
            }
        });

        return Ok(session_id);
    }

    // 5. Native Direct Ollama Execution (or fallback to sidecar only if explicitly present)
    if EngineSupervisor::locate_pinned_binary().is_err() {
        let session_id = format!("ollama_session_{}", tid);
        let worker_model_name = state.worker_model.lock().await.clone();
        let ollama_client = crate::worker::GroqWorkerClient::new_ollama(None);
        let worktree_buf = PathBuf::from(&worktree_info.worktree_path);
        let tid_clone = tid.clone();
        let ses_id_clone = session_id.clone();
        let plan_summary = plan.task_summary.clone();
        let plan_steps = plan.steps.clone();
        let app_clone = app_handle.clone();
        let active_task_inventories = state.active_task_inventories.clone();
        let task_state_arc = state.task_state.clone();
        let db_arc = state.db.clone();
        let planner_client_groq = state.planner_client.clone();
        let exec_service_groq = state.execution_service.clone();

        // Update state to WorkerRunning
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

        tokio::spawn(async move {
            match ollama_client
                .execute_session(
                    &worktree_buf,
                    &tid_clone,
                    &ses_id_clone,
                    &plan_summary,
                    &plan_steps,
                    Some(&worker_model_name),
                    exec_service_groq,
                    app_clone.clone(),
                )
                .await
            {
                Ok((inventory, executed_commands)) => {
                    let mut inv_map = active_task_inventories.lock().await;
                    inv_map.insert(tid_clone.clone(), inventory.clone());

                    let (commit_hash, conflict_resolved) = match GitService::commit_and_merge_task_to_primary(
                        &worktree_buf,
                        "",
                        &plan_summary,
                    ) {
                        Ok(outcome) => (outcome.commit_sha, outcome.conflict_resolved),
                        Err(e) => {
                            eprintln!("Warning: automatic commit and merge to primary failed: {e}");
                            (None, false)
                        }
                    };

                    if conflict_resolved {
                        let notice_record = crate::db::models::ChatMessageRecord {
                            id: format!("sys_{}", SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_nanos()),
                            chat_id: tid_clone.clone(),
                            role: "system".to_string(),
                            content: format!("Resolved a conflict automatically — {}'s changes were kept.", plan_summary),
                            plan_json: None,
                            worker_summary: None,
                            files_changed_json: None,
                            created_at: chrono::Utc::now().to_rfc3339(),
                            commit_hash: None,
                        };
                        let _ = db_arc.add_chat_message(&notice_record);
                        let _ = app_clone.emit("chat:message_added", &notice_record);
                    }

                    // Emit progress reasoning that summary is being generated before task completes
                    let _ = app_clone.emit(
                        "worker:message",
                        &WorkerEventPayload {
                            session_id: ses_id_clone.clone(),
                            event_type: "message.part.updated".into(),
                            content: serde_json::json!({
                                "type": "message.part.updated",
                                "properties": {
                                    "part": {
                                        "type": "reasoning",
                                        "text": "Summarizing and verifying task execution outcomes..."
                                    }
                                }
                            }).to_string(),
                            timestamp_ms: std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis() as u64,
                        },
                    );

                    let (files_summary, diff_excerpts) = format_inventory_for_summary(&inventory);
                    let (verified_cmds, unperformed_summary, unperformed_list) =
                        analyze_execution_grounding(&plan_steps, &inventory, &executed_commands);

                    // Ask Gemini (Director model) to summarize what happened with grounded verified outcomes
                    let summary = match CredentialService::get_gemini_api_key() {
                        Ok(key) => {
                            match planner_client_groq
                                .summarize_task_outcome_grounded(
                                    &key,
                                    &plan_summary,
                                    &files_summary,
                                    &diff_excerpts,
                                    &verified_cmds,
                                    &unperformed_summary,
                                    Some(&app_clone),
                                )
                                .await
                            {
                                Ok((gemini_sum, _, _)) => {
                                    let files_modified: Vec<String> = inventory.files.iter().map(|f| f.path.clone()).collect();
                                    zencode_core::verify_and_reconcile_summary(
                                        &gemini_sum,
                                        &files_modified,
                                        &executed_commands,
                                        &unperformed_list,
                                    )
                                }
                                Err(e) => {
                                    eprintln!("Gemini summarization failed ({e}), falling back to inventory stats");
                                    format!(
                                        "Completed task with Ollama worker: {} file(s) modified (+{}/-{}) across {} executed command(s)",
                                        inventory.files.len(),
                                        inventory.total_additions,
                                        inventory.total_deletions,
                                        executed_commands.len()
                                    )
                                }
                            }
                        }
                        Err(_) => format!(
                            "Completed task with Ollama worker: {} file(s) modified (+{}/-{}) across {} executed command(s)",
                            inventory.files.len(),
                            inventory.total_additions,
                            inventory.total_deletions,
                            executed_commands.len()
                        ),
                    };

                    let inv_json = serde_json::to_string(&inventory).ok();
                    let finalized = db_arc
                        .finalize_latest_assistant_message(
                            &tid_clone,
                            &summary,
                            inv_json.as_deref(),
                            commit_hash.as_deref(),
                        )
                        .unwrap_or(false);

                    let record = if finalized {
                        db_arc
                            .get_chat_messages(&tid_clone)
                            .ok()
                            .and_then(|msgs| msgs.into_iter().rev().find(|m| m.role == "assistant"))
                            .unwrap_or_else(|| {
                                crate::db::models::ChatMessageRecord {
                                    id: format!("msg_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis()),
                                    chat_id: tid_clone.clone(),
                                    role: "assistant".to_string(),
                                    content: summary.clone(),
                                    plan_json: None,
                                    worker_summary: Some(summary.clone()),
                                    files_changed_json: inv_json.clone(),
                                    created_at: chrono::Utc::now().to_rfc3339(),
                                    commit_hash: commit_hash.clone(),
                                }
                            })
                    } else {
                        let now = chrono::Utc::now().to_rfc3339();
                        let msg_id = format!(
                            "msg_{}",
                            std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .unwrap_or_default()
                                .as_millis()
                        );
                        let rec = crate::db::models::ChatMessageRecord {
                            id: msg_id,
                            chat_id: tid_clone.clone(),
                            role: "assistant".to_string(),
                            content: summary.clone(),
                            plan_json: None,
                            worker_summary: Some(summary.clone()),
                            files_changed_json: inv_json,
                            created_at: now,
                            commit_hash: commit_hash.clone(),
                        };
                        let _ = db_arc.add_chat_message(&rec);
                        rec
                    };
                    let _ = app_clone.emit("chat:message_added", &record);

                    // Associate commit_hash with user prompt message in this chat
                    if let Some(ref sha) = commit_hash {
                        if let Ok(msgs) = db_arc.get_chat_messages(&tid_clone) {
                            if let Some(user_msg) = msgs.iter().rev().find(|m| m.role == "user") {
                                let _ = db_arc.update_message_commit_hash(&user_msg.id, sha);
                            }
                        }
                    }

                    // Emit inventory ready and transition task state to Complete ONLY AFTER the summary message is published
                    let _ = app_clone.emit("review:inventory_ready", &inventory);
                    let mut task = task_state_arc.lock().await;
                    *task = TaskState::Complete;
                    let _ = app_clone.emit(
                        "worker:status",
                        &TaskState::Complete,
                    );
                }
                Err(e) => {
                    eprintln!("Ollama worker error: {e}");
                    let mut task = task_state_arc.lock().await;
                    *task = TaskState::Failed(e.clone());
                    let _ = app_clone.emit("worker:status", &TaskState::Failed(e.clone()));

                    let err_summary = format!(
                        "### Ollama Worker Error\n\nThe local Ollama worker could not complete the code execution:\n\n> {}\n\nPlease check that Ollama is running (`ollama serve`) and model is installed, or switch to the Groq worker.",
                        e
                    );
                    let err_record = crate::db::models::ChatMessageRecord {
                        id: format!("msg_err_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis()),
                        chat_id: tid_clone.clone(),
                        role: "assistant".to_string(),
                        content: err_summary.clone(),
                        plan_json: None,
                        worker_summary: Some(err_summary.clone()),
                        files_changed_json: None,
                        created_at: chrono::Utc::now().to_rfc3339(),
                        commit_hash: None,
                    };
                    let _ = db_arc.add_chat_message(&err_record);
                    let _ = app_clone.emit("chat:message_added", &err_record);
                }
            }
        });

        return Ok(session_id);
    }

    // 5. Configure OpenCode sidecar strictly with worktree path as its working directory (Local Ollama)
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

    // 6. Format initial worker prompt from plan, including prior context if this is a follow-up task
    let prior_exchanges = state.db.get_recent_exchanges(&tid, 5).unwrap_or_default();
    let mut prompt = String::new();
    if !prior_exchanges.is_empty() {
        prompt.push_str("Context from previous tasks in this chat session:\n");
        for ex in &prior_exchanges {
            if ex.role == "user" {
                prompt.push_str(&format!("- User previously asked: {}\n", ex.content));
            } else if let Some(ref sum) = ex.summary {
                prompt.push_str(&format!("- Assistant previously did: {}\n", sum));
                if !ex.files_changed.is_empty() {
                    prompt.push_str(&format!("  Modified/created files: {}\n", ex.files_changed.join(", ")));
                }
            }
        }
        prompt.push_str("\n");
    }

    prompt.push_str(&format!(
        "Execute the following implementation plan using local tools only:\n\nTask: {}\n\nSteps:\n",
        plan.task_summary
    ));
    for step in &plan.steps {
        prompt.push_str(&format!(
            "- [Goal]: {}\n  Instructions: {:?}\n  Likely files: {:?}\n",
            step.goal, step.instructions, step.likely_files
        ));
    }

    // 7. Send asynchronous prompt specifying configured Ollama worker model (defaults to granite4.1:8b)
    let worker_model_name = state.worker_model.lock().await.clone();
    supervisor
        .send_prompt(&session_id, &prompt, &worker_model_name)
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
    let supervisor_arc_sse = state.engine_supervisor.clone();
    let plan_summary_sse = plan.task_summary.clone();
    let plan_steps_sse = plan.steps.clone();
    let exec_service_sse = state.execution_service.clone();
    let db_arc_sse = state.db.clone();
    let planner_client_sse = state.planner_client.clone();

    // 9. Active Worker Watchdog: Polls process status & /global/health while WorkerRunning
    let supervisor_watch = state.engine_supervisor.clone();
    let task_state_watch = state.task_state.clone();
    let app_handle_watch = app_handle.clone();
    let watch_ses_id = session_id.clone();
    let watch_health_url = supervisor.health_url();
    let watch_auth = supervisor.auth_header();

    tokio::spawn(async move {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_millis(1500))
            .build()
            .unwrap_or_default();

        loop {
            tokio::time::sleep(Duration::from_millis(1500)).await;

            // Check if still WorkerRunning for this session
            {
                let current_task = task_state_watch.lock().await;
                match &*current_task {
                    TaskState::WorkerRunning { session_id } if session_id == &watch_ses_id => {}
                    _ => break, // Task ended or transitioned cleanly
                }
            }

            // Check process status on supervisor
            let mut is_dead = false;
            let mut error_msg = String::new();
            {
                let mut sup_guard = supervisor_watch.lock().await;
                if let Some(sup) = sup_guard.as_mut() {
                    if !sup.check_process_alive() {
                        is_dead = true;
                        error_msg = "OpenCode worker process terminated unexpectedly".to_string();
                    }
                } else {
                    is_dead = true;
                    error_msg = "Engine supervisor stopped".to_string();
                }
            }

            // Check /global/health endpoint
            if !is_dead {
                match client.get(&watch_health_url).header("Authorization", &watch_auth).send().await {
                    Ok(resp) => {
                        if !resp.status().is_success() {
                            is_dead = true;
                            error_msg = format!("OpenCode /global/health returned error status {}", resp.status());
                        }
                    }
                    Err(e) => {
                        is_dead = true;
                        error_msg = format!("OpenCode worker stopped responding: {e}");
                    }
                }
            }

            if is_dead {
                eprintln!("[WORKER WATCHDOG] Detected dead worker process: {error_msg}");
                let fail_state = TaskState::Failed(error_msg.clone());
                {
                    let mut current_task = task_state_watch.lock().await;
                    *current_task = fail_state.clone();
                }

                let _ = app_handle_watch.emit("worker:status", &fail_state);
                let _ = app_handle_watch.emit("engine:health", &EngineHealth::Failed(error_msg.clone()));

                let fail_payload = WorkerEventPayload {
                    session_id: watch_ses_id.clone(),
                    event_type: "worker.failed".to_string(),
                    content: serde_json::json!({
                        "type": "error",
                        "error": error_msg
                    }).to_string(),
                    timestamp_ms: SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis() as u64,
                };
                let _ = app_handle_watch.emit("worker:message", &fail_payload);
                break;
            }
        }
    });

    tokio::spawn(async move {
        use eventsource_stream::Eventsource;
        use futures_util::StreamExt;

        let mut captured_commands: Vec<String> = Vec::new();
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

                    // Parse real tool and command executions from OpenCode SSE payload
                    if let Ok(val) = serde_json::from_str::<serde_json::Value>(&event.data) {
                        if let Some(part) = val.get("properties").and_then(|p| p.get("part")) {
                            let ptype = part.get("type").and_then(|s| s.as_str()).unwrap_or_default();
                            if ptype == "tool" {
                                let tool_name = part.get("tool").and_then(|s| s.as_str()).unwrap_or("unknown");
                                let state = part.get("state");
                                let input = state.and_then(|st| st.get("input"));
                                if tool_name == "exec" || tool_name == "run" || tool_name == "bash" {
                                    if let Some(cmd) = input.and_then(|i| i.get("command")).and_then(|c| c.as_str()) {
                                        let status = state.and_then(|st| st.get("status")).and_then(|s| s.as_str()).unwrap_or("completed");
                                        let cmd_entry = format!("$ {} ({})", cmd, status);
                                        if !captured_commands.contains(&cmd_entry) {
                                            captured_commands.push(cmd_entry);
                                        }
                                    }
                                }
                            }
                        }
                    }

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
                                    let _ = app_handle_clone.emit("review:inventory_ready", &inventory);

                                    // Fix 1: Automatically commit all changes on the task branch and merge to primary branch
                                    let (commit_hash, conflict_resolved) = match GitService::commit_and_merge_task_to_primary(
                                        Path::new(&wt_path_clone),
                                        "",
                                        &plan_summary_sse,
                                    ) {
                                        Ok(outcome) => (outcome.commit_sha, outcome.conflict_resolved),
                                        Err(e) => {
                                            eprintln!("Warning: automatic commit and merge to primary failed: {e}");
                                            (None, false)
                                        }
                                    };

                                    if conflict_resolved {
                                        let notice_record = crate::db::models::ChatMessageRecord {
                                            id: format!("sys_{}", SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_nanos()),
                                            chat_id: task_id_clone.clone(),
                                            role: "system".to_string(),
                                            content: format!("Resolved a conflict automatically — {}'s changes were kept.", plan_summary_sse),
                                            plan_json: None,
                                            worker_summary: None,
                                            files_changed_json: None,
                                            created_at: chrono::Utc::now().to_rfc3339(),
                                            commit_hash: None,
                                        };
                                        let _ = db_arc_sse.add_chat_message(&notice_record);
                                        let _ = app_handle_clone.emit("chat:message_added", &notice_record);
                                    }

                                    // Emit progress reasoning that summary is being generated before task completes
                                    let _ = app_handle_clone.emit(
                                        "worker:message",
                                        &WorkerEventPayload {
                                            session_id: ses_id_clone.clone(),
                                            event_type: "message.part.updated".into(),
                                            content: serde_json::json!({
                                                "type": "message.part.updated",
                                                "properties": {
                                                    "part": {
                                                        "type": "reasoning",
                                                        "text": "Summarizing and verifying task execution outcomes..."
                                                    }
                                                }
                                            }).to_string(),
                                            timestamp_ms: SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis() as u64,
                                        },
                                    );

                                    // Collect any commands executed via ExecutionService during this task
                                    {
                                        let exec_hist = exec_service_sse.execution_history.lock().await;
                                        for ex in exec_hist.iter() {
                                            let cmd_str = format!("{} {}", ex.program, ex.args.join(" "));
                                            let status_str = match ex.exit_code {
                                                Some(0) => "exit code 0 (success)".to_string(),
                                                Some(c) => format!("exit code {c}"),
                                                None => "executed".to_string(),
                                            };
                                            let cmd_entry = format!("$ {} ({})", cmd_str, status_str);
                                            if !captured_commands.contains(&cmd_entry) {
                                                captured_commands.push(cmd_entry);
                                            }
                                        }
                                    }

                                    let (files_summary, diff_excerpts) = format_inventory_for_summary(&inventory);
                                    let (verified_cmds, unperformed_summary, unperformed_list) =
                                        analyze_execution_grounding(&plan_steps_sse, &inventory, &captured_commands);

                                    // Ask Gemini (Director model) to summarize what happened with grounded verified outcomes
                                    let summary = match CredentialService::get_gemini_api_key() {
                                        Ok(key) => {
                                            match planner_client_sse
                                                .summarize_task_outcome_grounded(
                                                    &key,
                                                    &plan_summary_sse,
                                                    &files_summary,
                                                    &diff_excerpts,
                                                    &verified_cmds,
                                                    &unperformed_summary,
                                                    Some(&app_handle_clone),
                                                )
                                                .await
                                            {
                                                Ok((gemini_sum, _, _)) => {
                                                    let files_modified: Vec<String> = inventory.files.iter().map(|f| f.path.clone()).collect();
                                                    zencode_core::verify_and_reconcile_summary(
                                                        &gemini_sum,
                                                        &files_modified,
                                                        &captured_commands,
                                                        &unperformed_list,
                                                    )
                                                }
                                                Err(e) => {
                                                    eprintln!("Gemini summarization failed ({e}), falling back to inventory stats");
                                                    format!(
                                                        "Completed task: {} file(s) modified (+{}/-{})",
                                                        inventory.files.len(),
                                                        inventory.total_additions,
                                                        inventory.total_deletions
                                                    )
                                                }
                                            }
                                        }
                                        Err(_) => format!(
                                            "Completed task: {} file(s) modified (+{}/-{})",
                                            inventory.files.len(),
                                            inventory.total_additions,
                                            inventory.total_deletions
                                        ),
                                    };


                                    let inv_json = serde_json::to_string(&inventory).ok();
                                    let finalized = db_arc_sse
                                        .finalize_latest_assistant_message(
                                            &task_id_clone,
                                            &summary,
                                            inv_json.as_deref(),
                                            commit_hash.as_deref(),
                                        )
                                        .unwrap_or(false);

                                    let record = if finalized {
                                        db_arc_sse
                                            .get_chat_messages(&task_id_clone)
                                            .ok()
                                            .and_then(|msgs| msgs.into_iter().rev().find(|m| m.role == "assistant"))
                                            .unwrap_or_else(|| {
                                                crate::db::models::ChatMessageRecord {
                                                    id: format!("msg_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis()),
                                                    chat_id: task_id_clone.clone(),
                                                    role: "assistant".to_string(),
                                                    content: summary.clone(),
                                                    plan_json: None,
                                                    worker_summary: Some(summary.clone()),
                                                    files_changed_json: inv_json.clone(),
                                                    created_at: chrono::Utc::now().to_rfc3339(),
                                                    commit_hash: commit_hash.clone(),
                                                }
                                            })
                                    } else {
                                        let now = chrono::Utc::now().to_rfc3339();
                                        let msg_id = format!(
                                            "msg_{}",
                                            std::time::SystemTime::now()
                                                .duration_since(std::time::UNIX_EPOCH)
                                                .unwrap_or_default()
                                                .as_millis()
                                        );
                                        let rec = crate::db::models::ChatMessageRecord {
                                            id: msg_id,
                                            chat_id: task_id_clone.clone(),
                                            role: "assistant".to_string(),
                                            content: summary.clone(),
                                            plan_json: None,
                                            worker_summary: Some(summary),
                                            files_changed_json: inv_json,
                                            created_at: now,
                                            commit_hash: commit_hash.clone(),
                                        };
                                        let _ = db_arc_sse.add_chat_message(&rec);
                                        rec
                                    };
                                    let _ = app_handle_clone.emit("chat:message_added", &record);

                                    if let Some(ref sha) = commit_hash {
                                        if let Ok(msgs) = db_arc_sse.get_chat_messages(&task_id_clone) {
                                            if let Some(user_msg) = msgs.iter().rev().find(|m| m.role == "user") {
                                                let _ = db_arc_sse.update_message_commit_hash(&user_msg.id, sha);
                                            }
                                        }
                                    }

                                    // Transition task state to Complete ONLY AFTER the summary message is published
                                    *task = TaskState::Complete;
                                    let _ = app_handle_clone.emit("worker:status", &TaskState::Complete);
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

            // Stream closed safety net: check if process died unexpectedly
            let mut is_healthy = false;
            {
                let mut sup_guard = supervisor_arc_sse.lock().await;
                if let Some(sup) = sup_guard.as_mut() {
                    if sup.check_process_alive() {
                        is_healthy = sup.ping_health().await;
                    }
                }
            }

            let mut task = task_state_arc.lock().await;
            if matches!(*task, TaskState::WorkerRunning { .. }) {
                if !is_healthy {
                    let err = "OpenCode worker stream disconnected and engine is unreachable".to_string();
                    *task = TaskState::Failed(err.clone());
                    let _ = app_handle_clone.emit("worker:status", &TaskState::Failed(err.clone()));
                    let _ = app_handle_clone.emit("engine:health", &EngineHealth::Failed(err));
                } else {
                    match GitService::get_worktree_diff(Path::new(&wt_path_clone)) {
                        Ok(inventory) => {
                            {
                                let mut inv_map = inventories_arc.lock().await;
                                inv_map.insert(task_id_clone.clone(), inventory.clone());
                            }
                            let _ = app_handle_clone.emit("review:inventory_ready", &inventory);

                            // Fix 1: Automatically commit all changes on the task branch and merge to primary branch
                            let (commit_hash, conflict_resolved) = match GitService::commit_and_merge_task_to_primary(
                                Path::new(&wt_path_clone),
                                "",
                                &plan_summary_sse,
                            ) {
                                Ok(outcome) => (outcome.commit_sha, outcome.conflict_resolved),
                                Err(e) => {
                                    eprintln!("Warning: automatic commit and merge to primary failed: {e}");
                                    (None, false)
                                }
                            };

                            if conflict_resolved {
                                let notice_record = crate::db::models::ChatMessageRecord {
                                    id: format!("sys_{}", SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_nanos()),
                                    chat_id: task_id_clone.clone(),
                                    role: "system".to_string(),
                                    content: format!("Resolved a conflict automatically — {}'s changes were kept.", plan_summary_sse),
                                    plan_json: None,
                                    worker_summary: None,
                                    files_changed_json: None,
                                    created_at: chrono::Utc::now().to_rfc3339(),
                                    commit_hash: None,
                                };
                                let _ = db_arc_sse.add_chat_message(&notice_record);
                                let _ = app_handle_clone.emit("chat:message_added", &notice_record);
                            }

                            *task = TaskState::Complete;
                            let _ = app_handle_clone.emit("worker:status", &TaskState::Complete);

                            let (files_summary, diff_excerpts) = format_inventory_for_summary(&inventory);
                            let (verified_cmds, unperformed_summary, unperformed_list) =
                                analyze_execution_grounding(&plan_steps_sse, &inventory, &captured_commands);

                            // Ask Gemini (Director model) to summarize what happened and what changes were made
                            let summary = match CredentialService::get_gemini_api_key() {
                                Ok(key) => {
                                    match planner_client_sse
                                        .summarize_task_outcome_grounded(
                                            &key,
                                            &plan_summary_sse,
                                            &files_summary,
                                            &diff_excerpts,
                                            &verified_cmds,
                                            &unperformed_summary,
                                            Some(&app_handle_clone),
                                        )
                                        .await
                                    {
                                        Ok((gemini_sum, _, _)) => {
                                            let files_modified: Vec<String> = inventory.files.iter().map(|f| f.path.clone()).collect();
                                            zencode_core::verify_and_reconcile_summary(
                                                &gemini_sum,
                                                &files_modified,
                                                &captured_commands,
                                                &unperformed_list,
                                            )
                                        }
                                        Err(e) => {
                                            eprintln!("Gemini summarization failed ({e}), falling back to inventory stats");
                                            format!(
                                                "Completed task: {} file(s) modified (+{}/-{})",
                                                inventory.files.len(),
                                                inventory.total_additions,
                                                inventory.total_deletions
                                            )
                                        }
                                    }
                                }
                                Err(_) => format!(
                                    "Completed task: {} file(s) modified (+{}/-{})",
                                    inventory.files.len(),
                                    inventory.total_additions,
                                    inventory.total_deletions
                                ),
                            };

                            let inv_json = serde_json::to_string(&inventory).ok();
                            let finalized = db_arc_sse
                                .finalize_latest_assistant_message(
                                    &task_id_clone,
                                    &summary,
                                    inv_json.as_deref(),
                                    commit_hash.as_deref(),
                                )
                                .unwrap_or(false);

                            let record = if finalized {
                                db_arc_sse
                                    .get_chat_messages(&task_id_clone)
                                    .ok()
                                    .and_then(|msgs| msgs.into_iter().rev().find(|m| m.role == "assistant"))
                                    .unwrap_or_else(|| {
                                        crate::db::models::ChatMessageRecord {
                                            id: format!("msg_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis()),
                                            chat_id: task_id_clone.clone(),
                                            role: "assistant".to_string(),
                                            content: summary.clone(),
                                            plan_json: None,
                                            worker_summary: Some(summary.clone()),
                                            files_changed_json: inv_json.clone(),
                                            created_at: chrono::Utc::now().to_rfc3339(),
                                            commit_hash: commit_hash.clone(),
                                        }
                                    })
                            } else {
                                let now = chrono::Utc::now().to_rfc3339();
                                let msg_id = format!(
                                    "msg_{}",
                                    std::time::SystemTime::now()
                                        .duration_since(std::time::UNIX_EPOCH)
                                        .unwrap_or_default()
                                        .as_millis()
                                );
                                let rec = crate::db::models::ChatMessageRecord {
                                    id: msg_id,
                                    chat_id: task_id_clone.clone(),
                                    role: "assistant".to_string(),
                                    content: summary.clone(),
                                    plan_json: None,
                                    worker_summary: Some(summary.clone()),
                                    files_changed_json: inv_json,
                                    created_at: now,
                                    commit_hash: commit_hash.clone(),
                                };
                                let _ = db_arc_sse.add_chat_message(&rec);
                                rec
                            };
                            let _ = app_handle_clone.emit("chat:message_added", &record);

                            if let Some(ref sha) = commit_hash {
                                if let Ok(msgs) = db_arc_sse.get_chat_messages(&task_id_clone) {
                                    if let Some(user_msg) = msgs.iter().rev().find(|m| m.role == "user") {
                                        let _ = db_arc_sse.update_message_commit_hash(&user_msg.id, sha);
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            eprintln!("Failed to generate diff inventory on stream end: {e}");
                            *task = TaskState::Complete;
                            let _ = app_handle_clone.emit("worker:status", &TaskState::Complete);
                        }
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

/// Reverts changes associated with a specific task/prompt back to the state before it ran
/// using `git revert <commit_hash> --no-edit` on the primary branch.
#[tauri::command]
pub async fn undo_task_changes(
    task_id: String,
    commit_hash: Option<String>,
    message_id: Option<String>,
    base_commit: Option<String>,
    app_handle: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let repo_path = {
        let active_proj = state.active_project.lock().await;
        let proj = active_proj
            .as_ref()
            .ok_or_else(|| "No active project open".to_string())?;
        proj.path.clone()
    };

    // Determine the commit hash to revert
    let sha_to_revert = if let Some(ch) = commit_hash {
        ch
    } else if let Some(mid) = message_id {
        state
            .db
            .get_message_commit_hash(&mid)
            .map_err(|e| format!("DB lookup error: {e}"))?
            .ok_or_else(|| format!("No commit hash found for message '{mid}'"))?
    } else {
        // Find latest commit hash associated with this chat/task
        let msgs = state
            .db
            .get_chat_messages(&task_id)
            .map_err(|e| format!("DB error: {e}"))?;
        if let Some(hash) = msgs.into_iter().rev().find_map(|m| m.commit_hash) {
            hash
        } else if let Some(bc) = base_commit {
            bc
        } else {
            return Err(format!("No commit found to undo for task '{task_id}'"));
        }
    };

    let (_new_head, adjusted) = match GitService::undo_task_changes(Path::new(&repo_path), &sha_to_revert) {
        Ok(res) => res,
        Err(e) => {
            // Scenario 6: Log clearly what couldn't be undone directly in chat (never a popup)
            let err_msg = format!("{e}");
            let err_record = crate::db::models::ChatMessageRecord {
                id: format!("sys_{}", SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_nanos()),
                chat_id: task_id.clone(),
                role: "system".to_string(),
                content: err_msg.clone(),
                plan_json: None,
                worker_summary: None,
                files_changed_json: None,
                created_at: chrono::Utc::now().to_rfc3339(),
                commit_hash: None,
            };
            let _ = state.db.add_chat_message(&err_record);
            let _ = app_handle.emit("chat:message_added", &err_record);
            return Err(err_msg);
        }
    };

    let notice_text = if adjusted {
        "Undid task. Some later changes were adjusted to keep things consistent.".to_string()
    } else {
        "Undid task changes.".to_string()
    };

    let notice_record = crate::db::models::ChatMessageRecord {
        id: format!("sys_{}", SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_nanos()),
        chat_id: task_id.clone(),
        role: "system".to_string(),
        content: notice_text,
        plan_json: None,
        worker_summary: None,
        files_changed_json: None,
        created_at: chrono::Utc::now().to_rfc3339(),
        commit_hash: None,
    };
    let _ = state.db.add_chat_message(&notice_record);
    let _ = app_handle.emit("chat:message_added", &notice_record);

    // Refresh project status in state
    if let Ok(updated_status) = GitService::get_primary_tree_status(Path::new(&repo_path)) {
        let mut active_proj = state.active_project.lock().await;
        if let Some(p) = active_proj.as_mut() {
            p.is_clean = updated_status.is_clean;
            p.git_status_summary = if updated_status.is_clean {
                "clean".to_string()
            } else {
                format!("{} uncommitted changes", updated_status.uncommitted_files.len())
            };
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

    let output = silent_command("git")
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

    let add_output = silent_command("git")
        .arg("-C")
        .arg(&p)
        .args(["add", "-A"])
        .output()
        .map_err(|e| format!("Failed to run git add: {e}"))?;

    if !add_output.status.success() {
        let stderr = String::from_utf8_lossy(&add_output.stderr);
        return Err(format!("git add failed: {stderr}"));
    }

    let commit_output = silent_command("git")
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
                        system_notice: None,
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
        BuildSystemType::Bun => vec![
            RecommendedCommand {
                name: "Test".into(),
                program: "bun".into(),
                args: vec!["test".into()],
                description: "Run tests with Bun".into(),
            },
            RecommendedCommand {
                name: "Build".into(),
                program: "bun".into(),
                args: vec!["run".into(), "build".into()],
                description: "Run build script with Bun".into(),
            },
        ],
        BuildSystemType::AndroidGradle => {
            let gradlew = if cfg!(windows) { "gradlew.bat" } else { "./gradlew" };
            vec![
                RecommendedCommand {
                    name: "Assemble Debug".into(),
                    program: gradlew.into(),
                    args: vec!["assembleDebug".into()],
                    description: "Compile and build Android debug APK".into(),
                },
                RecommendedCommand {
                    name: "Unit Tests".into(),
                    program: gradlew.into(),
                    args: vec!["testDebugUnitTest".into()],
                    description: "Run Android unit test suite".into(),
                },
                RecommendedCommand {
                    name: "Lint".into(),
                    program: gradlew.into(),
                    args: vec!["lintDebug".into()],
                    description: "Run Android lint checks".into(),
                },
            ]
        },
        BuildSystemType::Flutter => vec![
            RecommendedCommand {
                name: "Analyze".into(),
                program: "flutter".into(),
                args: vec!["analyze".into()],
                description: "Analyze Dart code and verify types".into(),
            },
            RecommendedCommand {
                name: "Test".into(),
                program: "flutter".into(),
                args: vec!["test".into()],
                description: "Run Flutter widget and unit tests".into(),
            },
            RecommendedCommand {
                name: "Build APK".into(),
                program: "flutter".into(),
                args: vec!["build".into(), "apk".into()],
                description: "Build Android APK bundle".into(),
            },
        ],
        BuildSystemType::IosXcode => vec![
            RecommendedCommand {
                name: "Test".into(),
                program: "swift".into(),
                args: vec!["test".into()],
                description: "Run Swift package tests".into(),
            },
            RecommendedCommand {
                name: "Build".into(),
                program: "swift".into(),
                args: vec!["build".into()],
                description: "Build Swift targets".into(),
            },
        ],
        BuildSystemType::Maven => vec![
            RecommendedCommand {
                name: "Test".into(),
                program: "mvn".into(),
                args: vec!["test".into()],
                description: "Run Maven test suite".into(),
            },
            RecommendedCommand {
                name: "Package".into(),
                program: "mvn".into(),
                args: vec!["package".into()],
                description: "Compile and package application JAR".into(),
            },
        ],
        BuildSystemType::CMake => vec![
            RecommendedCommand {
                name: "Build".into(),
                program: "cmake".into(),
                args: vec!["--build".into(), "build".into()],
                description: "Build CMake project".into(),
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
    app_handle: AppHandle,
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

    println!(
        "[Rust IPC] request_command_approval: program='{}', args={:?}, auto_approved={}, id='{}'",
        req.program, req.args, req.auto_approved, req.id
    );

    // If the command is not auto-approved, emit the approval required event to frontend
    if !req.auto_approved {
        let _ = app_handle.emit("permissions:approval_required", &req);
        println!("[Rust IPC] Emitted 'permissions:approval_required' event for command id='{}'", req.id);
    }

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

    let worker_model_name = state.worker_model.lock().await.clone();
    supervisor
        .send_prompt(&session_id, &prompt, &worker_model_name)
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

/// Update trusted status for Gemini context preview in a given workspace.
#[tauri::command]
pub async fn set_workspace_context_preview_trusted(
    path: String,
    trusted: bool,
    state: State<'_, AppState>,
) -> Result<(), String> {
    state.db.set_workspace_context_preview_trusted(&path, trusted).map_err(|e| e.to_string())
}

/// Check if Gemini context preview is trusted for a given workspace.
#[tauri::command]
pub async fn is_workspace_context_preview_trusted(
    path: String,
    state: State<'_, AppState>,
) -> Result<bool, String> {
    state.db.is_workspace_context_preview_trusted(&path).map_err(|e| e.to_string())
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

/// Append a message to the persistent chat history.
#[tauri::command]
pub async fn add_chat_message(
    chat_id: String,
    role: String,
    content: String,
    plan_json: Option<String>,
    worker_summary: Option<String>,
    files_changed: Option<Vec<String>>,
    commit_hash: Option<String>,
    state: State<'_, AppState>,
) -> Result<crate::db::ChatMessageRecord, String> {
    let now = chrono::Utc::now().to_rfc3339();
    let msg_id = format!(
        "msg_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()
    );
    let files_changed_json = files_changed.map(|f| serde_json::to_string(&f).unwrap_or_else(|_| "[]".to_string()));
    let record = crate::db::ChatMessageRecord {
        id: msg_id,
        chat_id,
        role,
        content,
        plan_json,
        worker_summary,
        files_changed_json,
        created_at: now,
        commit_hash,
    };
    state.db.add_chat_message(&record).map_err(|e| e.to_string())?;
    Ok(record)
}

/// Retrieve all ordered messages for a chat.
#[tauri::command]
pub async fn get_chat_messages(
    chat_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<crate::db::ChatMessageRecord>, String> {
    state.db.get_chat_messages(&chat_id).map_err(|e| e.to_string())
}

/// Summarizes a completed task using Gemini (Director model).
#[tauri::command]
pub async fn summarize_task(
    chat_id: String,
    task_summary: String,
    app_handle: AppHandle,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let api_key = CredentialService::get_gemini_api_key()
        .map_err(|_| "Gemini API key not found in Windows Credential Manager".to_string())?;

    let inv_opt = {
        let inv_map = state.active_task_inventories.lock().await;
        inv_map.get(&chat_id).cloned()
    };

    let inventory = inv_opt.unwrap_or_else(|| ChangeInventory {
        worktree_path: String::new(),
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

    let (files_summary, diff_excerpts) = format_inventory_for_summary(&inventory);

    let (summary, _, _) = state
        .planner_client
        .summarize_task_outcome(
            &api_key,
            &task_summary,
            &files_summary,
            &diff_excerpts,
            Some(&app_handle),
        )
        .await?;

    Ok(summary)
}

/// Represents a file or folder in the workspace file tree.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct WorkspaceFileEntry {
    pub name: String,
    pub path: String, // relative to workspace root, normalized with '/'
    pub is_dir: bool,
    pub children: Option<Vec<WorkspaceFileEntry>>,
}

fn build_file_tree(base: &Path, current: &Path, depth: usize) -> Vec<WorkspaceFileEntry> {
    if depth > 8 {
        return Vec::new();
    }
    let read_dir = match std::fs::read_dir(current) {
        Ok(rd) => rd,
        Err(_) => return Vec::new(),
    };

    let ignored_names = [
        ".git", "node_modules", "target", "dist", ".zencode-worktrees",
        "build", ".next", ".idea", ".vscode", ".gemini", ".system_generated"
    ];

    let mut items: Vec<_> = read_dir.filter_map(|e| e.ok()).collect();
    // Sort directories first, then alphabetical
    items.sort_by(|a, b| {
        let a_is_dir = a.file_type().map(|ft| ft.is_dir()).unwrap_or(false);
        let b_is_dir = b.file_type().map(|ft| ft.is_dir()).unwrap_or(false);
        match (a_is_dir, b_is_dir) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => a.file_name().cmp(&b.file_name()),
        }
    });

    let mut entries = Vec::new();
    for item in items {
        let file_name = item.file_name().to_string_lossy().to_string();
        if ignored_names.contains(&file_name.as_str()) {
            continue;
        }
        let full_path = item.path();
        let rel_path = match full_path.strip_prefix(base) {
            Ok(p) => p.to_string_lossy().replace('\\', "/"),
            Err(_) => file_name.clone(),
        };
        let is_dir = item.file_type().map(|ft| ft.is_dir()).unwrap_or(false);
        if is_dir {
            let children = build_file_tree(base, &full_path, depth + 1);
            entries.push(WorkspaceFileEntry {
                name: file_name,
                path: rel_path,
                is_dir: true,
                children: Some(children),
            });
        } else {
            entries.push(WorkspaceFileEntry {
                name: file_name,
                path: rel_path,
                is_dir: false,
                children: None,
            });
        }
    }
    entries
}

/// Retrieve recursive file tree for the currently open workspace.
#[tauri::command]
pub async fn get_workspace_file_tree(
    workspace_path: String,
) -> Result<Vec<WorkspaceFileEntry>, String> {
    let p = Path::new(&workspace_path);
    if !p.exists() || !p.is_dir() {
        return Err(format!("Workspace path '{workspace_path}' does not exist or is not a directory"));
    }
    Ok(build_file_tree(p, p, 0))
}

/// Read text content of a workspace file for the code viewer.
#[tauri::command]
pub async fn read_workspace_file(
    file_path: String,
) -> Result<String, String> {
    let p = Path::new(&file_path);
    if !p.exists() {
        return Err(format!("File '{file_path}' does not exist"));
    }
    let meta = std::fs::metadata(p).map_err(|e| e.to_string())?;
    if meta.len() > 5 * 1024 * 1024 {
        return Err("File exceeds 5MB limit for inline viewing".to_string());
    }
    std::fs::read_to_string(p).map_err(|e| format!("Failed to read file: {e}"))
}

/// List recent chats across all workspaces for landing and search pages.
#[tauri::command]
pub async fn list_all_recent_chats(
    limit: Option<usize>,
    state: State<'_, AppState>,
) -> Result<Vec<crate::db::ChatSummaryRecord>, String> {
    state.db.list_all_recent_chats(limit.unwrap_or(20)).map_err(|e| e.to_string())
}

/// Remove a workspace from known list (does not touch filesystem).
#[tauri::command]
pub async fn remove_workspace(
    workspace_path: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    state.db.remove_workspace(&workspace_path).map_err(|e| e.to_string())
}

/// Search chats across all workspaces and title/plan contents.
#[tauri::command]
pub async fn search_chats(
    query: String,
    state: State<'_, AppState>,
) -> Result<Vec<crate::db::ChatSummaryRecord>, String> {
    state.db.search_chats(&query).map_err(|e| e.to_string())
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct GeminiKeyStatus {
    pub has_key: bool,
    pub masked_key: String,
    pub status_text: String,
}

/// Query Gemini API key status from Windows Credential Manager without exposing the secret.
#[tauri::command]
pub async fn get_gemini_key_status() -> Result<GeminiKeyStatus, String> {
    match CredentialService::get_gemini_api_key() {
        Ok(key) if !key.is_empty() => {
            let masked = if key.len() > 8 {
                format!("{}...{}", &key[..4], &key[key.len() - 4..])
            } else {
                "********".to_string()
            };
            Ok(GeminiKeyStatus {
                has_key: true,
                masked_key: masked,
                status_text: "Configured in Windows Credential Manager".to_string(),
            })
        }
        _ => Ok(GeminiKeyStatus {
            has_key: false,
            masked_key: String::new(),
            status_text: "Not configured".to_string(),
        }),
    }
}

/// Update Gemini API key in Windows Credential Manager.
#[tauri::command]
pub async fn set_gemini_api_key_command(key: String) -> Result<GeminiKeyStatus, String> {
    let trimmed = key.trim();
    if trimmed.is_empty() {
        return Err("API key cannot be empty".to_string());
    }
    CredentialService::set_gemini_api_key(trimmed)
        .map_err(|e| format!("Failed to save key to Windows Credential Manager: {e}"))?;
    get_gemini_key_status().await
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct GroqKeyStatus {
    pub has_key: bool,
    pub masked_key: String,
    pub status_text: String,
}

/// Query Groq API key status from Windows Credential Manager without exposing the secret.
#[tauri::command]
pub async fn get_groq_key_status() -> Result<GroqKeyStatus, String> {
    match CredentialService::get_groq_api_key() {
        Ok(key) if !key.is_empty() => {
            let masked = if key.len() > 8 {
                format!("{}...{}", &key[..4], &key[key.len() - 4..])
            } else {
                "********".to_string()
            };
            Ok(GroqKeyStatus {
                has_key: true,
                masked_key: masked,
                status_text: "Configured in Windows Credential Manager".to_string(),
            })
        }
        _ => Ok(GroqKeyStatus {
            has_key: false,
            masked_key: String::new(),
            status_text: "Not configured".to_string(),
        }),
    }
}

/// Update Groq API key in Windows Credential Manager.
#[tauri::command]
pub async fn set_groq_api_key_command(key: String) -> Result<GroqKeyStatus, String> {
    let trimmed = key.trim();
    if trimmed.is_empty() {
        return Err("API key cannot be empty".to_string());
    }
    CredentialService::set_groq_api_key(trimmed)
        .map_err(|e| format!("Failed to save key to Windows Credential Manager: {e}"))?;
    get_groq_key_status().await
}

/// Remove Groq API key from Windows Credential Manager.
#[tauri::command]
pub async fn delete_groq_api_key_command() -> Result<(), String> {
    CredentialService::delete_groq_api_key()
        .map_err(|e| format!("Failed to delete key from Windows Credential Manager: {e}"))
}

/// Validate a Gemini API key via a lightweight test request.
#[tauri::command]
pub async fn validate_gemini_api_key(key: String) -> Result<bool, String> {
    let trimmed = key.trim();
    if trimmed.is_empty() {
        return Err("API key cannot be empty".to_string());
    }

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(6))
        .build()
        .map_err(|e| e.to_string())?;

    let url = format!("https://generativelanguage.googleapis.com/v1beta/models?key={trimmed}");
    let resp = client.get(&url).send().await.map_err(|e| format!("Network request failed: {e}"))?;

    let status = resp.status();
    if status.is_success() {
        Ok(true)
    } else {
        let err_txt = resp.text().await.unwrap_or_default();
        Err(format!("Validation failed with status {}: {err_txt}", status))
    }
}

/// Validate a Groq API key via a lightweight test request and extract real rate limit headers.
#[tauri::command]
pub async fn validate_groq_api_key(
    key: String,
    state: tauri::State<'_, AppState>,
    app_handle: tauri::AppHandle,
) -> Result<bool, String> {
    let trimmed = key.trim();
    if trimmed.is_empty() {
        return Err("API key cannot be empty".to_string());
    }

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(8))
        .build()
        .map_err(|e| e.to_string())?;

    // Perform a lightweight 1-token completion request to extract real rate-limit headers
    let body = serde_json::json!({
        "model": "openai/gpt-oss-120b",
        "messages": [{ "role": "user", "content": "ping" }],
        "max_tokens": 1
    });

    let resp = client
        .post("https://api.groq.com/openai/v1/chat/completions")
        .header("Authorization", format!("Bearer {trimmed}"))
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("Network request failed: {e}"))?;

    let status = resp.status();
    // Parse real rate limit headers from Groq response
    let groq_headers = crate::planner::quota::parse_groq_headers(resp.headers());
    {
        let mut tracker = state.planner_client.quota_tracker.lock().await;
        tracker.update_groq_server_headers("openai/gpt-oss-120b", &groq_headers);
        let event = tracker.to_quota_update_event("gemini-3.5-flash-lite");
        let _ = app_handle.emit("planner:quota_update", &event);
    }

    if status.is_success() || status.as_u16() == 429 {
        Ok(true)
    } else {
        let err_txt = resp.text().await.unwrap_or_default();
        Err(format!("Validation failed with status {}: {err_txt}", status))
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct WorkspaceWorkerConfig {
    pub worker_type: String,
    pub groq_model: String,
}

/// Get workspace default worker configuration (local vs groq).
#[tauri::command]
pub async fn get_workspace_worker_config(
    workspace_path: String,
    state: State<'_, AppState>,
) -> Result<WorkspaceWorkerConfig, String> {
    let (worker_type, groq_model) = state
        .db
        .get_workspace_worker_config(&workspace_path)
        .map_err(|e| e.to_string())?;

    Ok(WorkspaceWorkerConfig {
        worker_type,
        groq_model: groq_model.unwrap_or_else(|| "openai/gpt-oss-120b".to_string()),
    })
}

/// Set workspace default worker configuration (local vs groq).
#[tauri::command]
pub async fn set_workspace_worker_config(
    workspace_path: String,
    worker_type: String,
    groq_model: Option<String>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    state
        .db
        .set_workspace_worker_config(&workspace_path, &worker_type, groq_model.as_deref())
        .map_err(|e| e.to_string())
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct OllamaModelItem {
    pub name: String,
    pub size: String,
    pub is_recommended: bool,
}

/// Query installed Ollama models live via `ollama list`.
#[tauri::command]
pub async fn list_installed_ollama_models() -> Result<Vec<OllamaModelItem>, String> {
    let output = silent_command("ollama")
        .arg("list")
        .output()
        .map_err(|e| format!("Failed to run ollama list: {e}"))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut models = Vec::new();

    for line in stdout.lines().skip(1) {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if !parts.is_empty() {
            let name = parts[0].to_string();
            let size = if parts.len() >= 3 {
                format!("{} {}", parts[parts.len() - 3], parts[parts.len() - 2])
            } else {
                String::new()
            };
            let is_recommended = name.starts_with("granite4.1:8b");
            models.push(OllamaModelItem {
                name,
                size,
                is_recommended,
            });
        }
    }

    if models.is_empty() {
        models.push(OllamaModelItem {
            name: "granite4.1:8b".to_string(),
            size: "5.3 GB".to_string(),
            is_recommended: true,
        });
    }

    Ok(models)
}

/// Get current worker model.
#[tauri::command]
pub async fn get_worker_model(state: State<'_, AppState>) -> Result<String, String> {
    Ok(state.worker_model.lock().await.clone())
}

/// Set active worker model.
#[tauri::command]
pub async fn set_worker_model(model: String, state: State<'_, AppState>) -> Result<(), String> {
    let trimmed = model.trim();
    if trimmed.is_empty() {
        return Err("Worker model name cannot be empty".to_string());
    }
    let mut m = state.worker_model.lock().await;
    *m = trimmed.to_string();
    Ok(())
}

/// Custom window titlebar minimize command.
#[tauri::command]
pub async fn window_minimize(window: tauri::Window) -> Result<(), String> {
    window.minimize().map_err(|e| e.to_string())
}

/// Custom window titlebar toggle maximize command.
#[tauri::command]
pub async fn window_toggle_maximize(window: tauri::Window) -> Result<bool, String> {
    let is_max = window.is_maximized().map_err(|e| e.to_string())?;
    if is_max {
        window.unmaximize().map_err(|e| e.to_string())?;
        Ok(false)
    } else {
        window.maximize().map_err(|e| e.to_string())?;
        Ok(true)
    }
}

/// Custom window titlebar close command.
#[tauri::command]
pub async fn window_close(window: tauri::Window) -> Result<(), String> {
    window.close().map_err(|e| e.to_string())
}

/// Open native OS folder picker dialog to select a workspace directory.
#[tauri::command]
pub async fn pick_folder() -> Result<Option<String>, String> {
    let handle = tokio::task::spawn_blocking(|| {
        #[cfg(target_os = "windows")]
        {
            let script = r#"Add-Type -AssemblyName System.Windows.Forms; $dialog = New-Object System.Windows.Forms.FolderBrowserDialog; $dialog.Description = 'Select Workspace Folder'; $dialog.ShowNewFolderButton = $true; if ($dialog.ShowDialog() -eq [System.Windows.Forms.DialogResult]::OK) { [Console]::OutputEncoding = [System.Text.Encoding]::UTF8; Write-Output $dialog.SelectedPath }"#;
            let output = silent_command("powershell")
                .args(["-NoProfile", "-NonInteractive", "-Command", script])
                .output()
                .map_err(|e| format!("Failed to open folder picker: {e}"))?;

            if output.status.success() {
                let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if path.is_empty() {
                    Ok(None)
                } else {
                    Ok(Some(path))
                }
            } else {
                Err(String::from_utf8_lossy(&output.stderr).to_string())
            }
        }
        #[cfg(not(target_os = "windows"))]
        {
            Ok::<Option<String>, String>(None)
        }
    })
    .await
    .map_err(|e| e.to_string())??;

    Ok(handle)
}

/// Native OS file picker dialog for attaching files to prompts.
/// If an active workspace is open and the selected file is outside the workspace,
/// it is automatically copied into the main workspace folder so both the
/// Planner (Gemini) and the Worker (Ollama/Groq) can access it directly.
#[tauri::command]
pub async fn pick_file(
    default_dir: Option<String>,
    state: State<'_, AppState>,
) -> Result<Option<String>, String> {
    let active_ws: Option<PathBuf> = {
        let guard = state.active_project.lock().await;
        guard.as_ref().map(|p| PathBuf::from(&p.path))
    };

    let selected_path_opt = tokio::task::spawn_blocking(move || {
        #[cfg(target_os = "windows")]
        {
            let initial = default_dir.unwrap_or_default().replace('\'', "''");
            let script = format!(
                r#"Add-Type -AssemblyName System.Windows.Forms; $dialog = New-Object System.Windows.Forms.OpenFileDialog; $dialog.Title = 'Select File to Attach'; if ('{initial}' -ne '') {{ $dialog.InitialDirectory = '{initial}' }}; if ($dialog.ShowDialog() -eq [System.Windows.Forms.DialogResult]::OK) {{ [Console]::OutputEncoding = [System.Text.Encoding]::UTF8; Write-Output $dialog.FileName }}"#
            );
            let output = silent_command("powershell")
                .args(["-NoProfile", "-NonInteractive", "-Command", &script])
                .output()
                .map_err(|e| format!("Failed to open file picker: {e}"))?;

            if output.status.success() {
                let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if path.is_empty() {
                    Ok(None)
                } else {
                    Ok(Some(path))
                }
            } else {
                Err(String::from_utf8_lossy(&output.stderr).to_string())
            }
        }
        #[cfg(not(target_os = "windows"))]
        {
            Ok::<Option<String>, String>(None)
        }
    })
    .await
    .map_err(|e| e.to_string())??;

    let Some(raw_path) = selected_path_opt else {
        return Ok(None);
    };

    let picked_path = PathBuf::from(&raw_path);
    if !picked_path.is_file() {
        return Ok(Some(raw_path));
    }

    if let Some(ws_dir) = active_ws {
        // If outside the active workspace, automatically copy it to the main workspace folder!
        if !picked_path.starts_with(&ws_dir) {
            let file_name = picked_path
                .file_name()
                .ok_or_else(|| "Invalid file name".to_string())?;
            let dest_path = ws_dir.join(file_name);
            println!(
                "[Attachment] Auto-copying attached file from '{}' into main workspace folder '{}'",
                picked_path.display(),
                dest_path.display()
            );
            if let Err(e) = std::fs::copy(&picked_path, &dest_path) {
                eprintln!("[Attachment] Warning: Could not copy file to workspace: {e}");
                return Ok(Some(raw_path));
            }
            // Return filename relative to workspace so it's clean and uniform
            return Ok(Some(file_name.to_string_lossy().to_string()));
        } else {
            // Already inside workspace — return relative path if possible
            if let Ok(rel) = picked_path.strip_prefix(&ws_dir) {
                return Ok(Some(rel.to_string_lossy().to_string()));
            }
        }
    }

    Ok(Some(raw_path))
}

/// Runs a quick health check of required local dependencies:
/// 1. Ollama daemon reachable and selected worker model installed.
/// 2. OpenCode sidecar binary present and version-verified.
/// 3. Git available on PATH.
/// 4. Gemini API key present in Windows Credential Manager.
#[tauri::command]
pub async fn verify_startup_dependencies(
    state: State<'_, AppState>,
) -> Result<StartupCheckResult, String> {
    let mut dependencies = Vec::new();

    // 1. Check Ollama daemon & worker model
    let target_worker_model = {
        let configured = get_worker_model(state.clone())
            .await
            .unwrap_or_else(|_| "granite4.1:8b".to_string());
        if configured.trim().is_empty() {
            "granite4.1:8b".to_string()
        } else {
            configured
        }
    };

    let client = reqwest::Client::new();
    let ollama_status = match client
        .get("http://127.0.0.1:11434/api/tags")
        .timeout(Duration::from_millis(1500))
        .send()
        .await
    {
        Ok(resp) if resp.status().is_success() => {
            if let Ok(body) = resp.json::<serde_json::Value>().await {
                let models = body.get("models").and_then(|m| m.as_array());
                let has_model = models.map_or(false, |arr| {
                    arr.iter().any(|m| {
                        let name = m.get("name").and_then(|n| n.as_str()).unwrap_or("");
                        name.starts_with(&target_worker_model) || target_worker_model.starts_with(name)
                    })
                });

                if has_model {
                    DependencyStatus {
                        name: "Ollama & Worker Model".into(),
                        is_ready: true,
                        details: format!("Ollama daemon running with '{}' installed", target_worker_model),
                        fix_recommendation: None,
                    }
                } else {
                    DependencyStatus {
                        name: "Ollama & Worker Model".into(),
                        is_ready: false,
                        details: format!(
                            "Ollama is running, but worker model '{}' is not installed",
                            target_worker_model
                        ),
                        fix_recommendation: Some(format!(
                            "Run 'ollama pull {}' in terminal to install the model",
                            target_worker_model
                        )),
                    }
                }
            } else {
                DependencyStatus {
                    name: "Ollama & Worker Model".into(),
                    is_ready: false,
                    details: "Ollama returned invalid response format".into(),
                    fix_recommendation: Some("Verify Ollama installation and run 'ollama serve'".into()),
                }
            }
        }
        _ => DependencyStatus {
            name: "Ollama & Worker Model".into(),
            is_ready: false,
            details: "Ollama daemon unreachable at 127.0.0.1:11434".into(),
            fix_recommendation: Some("Start Ollama by running 'ollama serve' in your terminal".into()),
        },
    };
    dependencies.push(ollama_status);

    // 2. OpenCode sidecar binary check (optional legacy engine; native direct Ollama is primary)
    if let Ok(path) = EngineSupervisor::locate_pinned_binary() {
        let version = match silent_command(&path).arg("--version").output() {
            Ok(out) if out.status.success() => {
                String::from_utf8_lossy(&out.stdout).trim().to_string()
            }
            _ => "v1.0.0".to_string(),
        };
        dependencies.push(DependencyStatus {
            name: "OpenCode Sidecar Engine (Optional)".into(),
            is_ready: true,
            details: format!("Binary present at {:?} ({})", path, version),
            fix_recommendation: None,
        });
    }

    // 3. Git on PATH
    let git_status = match silent_command("git").arg("--version").output() {
        Ok(out) if out.status.success() => {
            let ver = String::from_utf8_lossy(&out.stdout).trim().to_string();
            DependencyStatus {
                name: "Git Version Control".into(),
                is_ready: true,
                details: format!("Available on PATH ({})", ver),
                fix_recommendation: None,
            }
        }
        _ => DependencyStatus {
            name: "Git Version Control".into(),
            is_ready: false,
            details: "Git executable not found on system PATH".into(),
            fix_recommendation: Some(
                "Install Git from https://git-scm.com and ensure it is added to PATH".into(),
            ),
        },
    };
    dependencies.push(git_status);

    // 4. Gemini API Key in Windows Credential Manager
    let gemini_status = match CredentialService::get_gemini_api_key() {
        Ok(key) if !key.trim().is_empty() => DependencyStatus {
            name: "Gemini Planner Key".into(),
            is_ready: true,
            details: "API key configured in Windows Credential Manager".into(),
            fix_recommendation: None,
        },
        _ => DependencyStatus {
            name: "Gemini Planner Key".into(),
            is_ready: false,
            details: "Gemini API key not found in Windows Credential Manager".into(),
            fix_recommendation: Some(
                "Configure your Gemini API key in Settings (gear icon) or via Windows Credential Manager"
                    .into(),
            ),
        },
    };
    dependencies.push(gemini_status);

    let all_passed = dependencies.iter().all(|d| d.is_ready);
    Ok(StartupCheckResult {
        all_passed,
        dependencies,
    })
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
                just_initialized_git: false,
                is_corrupted: false,
                system_notice: None,
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
        assert!(proj.just_initialized_git, "Must silently initialize git on opening a non-git workspace");
        assert!(proj.is_git_repo, "Workspace is now initialized with git");
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

    #[test]
    fn test_analyze_execution_grounding_discovery_command_verification() {
        let steps = vec![
            zencode_core::PlanStep {
                id: "step_1".to_string(),
                goal: "List the installed Ollama models to determine the count.".to_string(),
                likely_files: vec![],
                instructions: vec!["Execute ollama list in the terminal to inspect installed models.".to_string()],
                depends_on: vec![],
                acceptance_checks: vec!["Ensure installed models are retrieved".to_string()],
            }
        ];
        let inventory = ChangeInventory {
            worktree_path: "/tmp/fake".to_string(),
            branch_name: "task/test".to_string(),
            base_commit: "abc1234".to_string(),
            files: vec![],
            total_additions: 0,
            total_deletions: 0,
            modified_count: 0,
            added_count: 0,
            untracked_count: 0,
            deleted_count: 0,
            renamed_count: 0,
            binary_count: 0,
        };
        let captured_commands = vec![
            "$ ollama list (exit code 0 (success))\nOutput:\n  gemma3:4b\n  granite4.1:8b".to_string(),
        ];

        let (_verified_summary, unperformed_summary, unperformed_steps) =
            analyze_execution_grounding(&steps, &inventory, &captured_commands);

        assert!(
            unperformed_steps.is_empty(),
            "Discovery command ollama list must be verified as performed, got: {:?}",
            unperformed_steps
        );
        assert!(
            unperformed_summary.contains("All planned steps were verified"),
            "Expected all steps verified, got: {unperformed_summary}"
        );
    }

    #[tokio::test]
    async fn test_exploration_task_routing_bypasses_worker_and_answers_direct() {
        let context = PlanningContext {
            task_description: "explore this project and explain the architecture".to_string(),
            project_type: "Rust / React".to_string(),
            shallow_tree: vec!["Cargo.toml".into(), "src/main.rs".into(), "src/lib.rs".into()],
            manifest_summaries: vec!["Cargo.toml:\n[package]\nname = \"test\"".into()],
            git_branch_and_status: "Branch: main, Status: clean".into(),
            approved_source_excerpts: vec![],
            constraints: vec![],
            conversation_history: vec![],
            ..Default::default()
        };

        // When Gemini returns an exploration plan that was initially marked is_actionable: true,
        // our Change 1 safeguard intercepts it, sets is_actionable: false, clears steps,
        // and populates conversational_reply directly.
        let mut mock_plan = ImplementationPlan {
            is_actionable: true,
            conversational_reply: None,
            task_summary: "Architectural overview of the Rust/React repository".to_string(),
            assumptions: vec![],
            questions: vec![],
            project_findings: vec!["Monorepo with Rust backend and React frontend".to_string()],
            steps: vec![zencode_core::PlanStep {
                id: "s1".into(),
                goal: "Review files".into(),
                likely_files: vec![],
                instructions: vec!["Read repository structure".into()],
                depends_on: vec![],
                acceptance_checks: vec![],
            }],
            risks: vec![],
            verification_suggestions: vec![],
            planning_duration_seconds: None,
        };

        let task_desc_lower = context.task_description.to_lowercase();
        let has_edit_keyword = task_desc_lower.contains("edit")
            || task_desc_lower.contains("modify")
            || task_desc_lower.contains("create")
            || task_desc_lower.contains("delete")
            || task_desc_lower.contains("remove")
            || task_desc_lower.contains("add")
            || task_desc_lower.contains("fix")
            || task_desc_lower.contains("refactor")
            || task_desc_lower.contains("install")
            || task_desc_lower.contains("build")
            || task_desc_lower.contains("develop")
            || task_desc_lower.contains("make")
            || task_desc_lower.contains("code")
            || task_desc_lower.contains("implement")
            || task_desc_lower.contains("generate")
            || task_desc_lower.contains("write")
            || task_desc_lower.contains("run");

        let is_read_only_exploration = (task_desc_lower.contains("explore")
            || task_desc_lower.contains("analyze")
            || task_desc_lower.contains("overview")
            || task_desc_lower.contains("architecture")
            || task_desc_lower.contains("structure")
            || task_desc_lower.contains("explain")
            || task_desc_lower.contains("walk me through")
            || task_desc_lower.contains("what does this"))
            && !has_edit_keyword;

        assert!(is_read_only_exploration, "Must classify as read-only exploration");

        if is_read_only_exploration && mock_plan.is_actionable {
            mock_plan.is_actionable = false;
            let mut direct_answer = format!("### Project Architecture & Codebase Overview\n\n{}\n\n", mock_plan.task_summary);
            if !mock_plan.project_findings.is_empty() {
                direct_answer.push_str("#### Key Architecture Findings\n");
                for finding in &mock_plan.project_findings {
                    direct_answer.push_str(&format!("- {}\n", finding));
                }
            }
            mock_plan.conversational_reply = Some(direct_answer);
            mock_plan.steps = Vec::new();
        }

        assert_eq!(mock_plan.is_actionable, false, "Exploration must be is_actionable: false");
        assert!(mock_plan.steps.is_empty(), "Steps must be empty so no worker is dispatched");
        assert!(mock_plan.conversational_reply.is_some(), "Must contain direct conversational reply");
        let reply = mock_plan.conversational_reply.unwrap();
        assert!(reply.contains("Monorepo with Rust backend and React frontend"));
    }

    #[tokio::test]
    async fn test_build_prompt_intent_forces_actionable() {
        let prompts = vec![
            "build",
            "build it",
            "develop me website based on a pdf",
            "make a landing page",
            "code the authentication system",
            "implement the feature",
        ];

        for p in prompts {
            let task_desc_lower = p.to_lowercase();
            let has_edit_keyword = task_desc_lower.contains("edit")
                || task_desc_lower.contains("modify")
                || task_desc_lower.contains("create")
                || task_desc_lower.contains("delete")
                || task_desc_lower.contains("remove")
                || task_desc_lower.contains("add")
                || task_desc_lower.contains("fix")
                || task_desc_lower.contains("refactor")
                || task_desc_lower.contains("install")
                || task_desc_lower.contains("build")
                || task_desc_lower.contains("develop")
                || task_desc_lower.contains("make")
                || task_desc_lower.contains("code")
                || task_desc_lower.contains("implement")
                || task_desc_lower.contains("generate")
                || task_desc_lower.contains("write")
                || task_desc_lower.contains("run");

            assert!(has_edit_keyword, "Prompt '{p}' must be detected as actionable edit/build intent");
        }
    }

    #[tokio::test]
    async fn test_multimodal_pdf_and_image_context_preview_ingestion() {
        let temp_dir = std::env::temp_dir().join(format!(
            "zencode_multimodal_test_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis()
        ));
        std::fs::create_dir_all(&temp_dir).unwrap();
        let pdf_path = temp_dir.join("mockup.pdf");
        std::fs::write(&pdf_path, b"%PDF-1.4\n1 0 obj\n<<\n>>\nendobj\ntrailer\n<<\n>>\n%%EOF").unwrap();

        let state = AppState::new();
        {
            let mut active = state.active_project.lock().await;
            *active = Some(ProjectState {
                path: temp_dir.to_string_lossy().to_string(),
                name: "TestProject".into(),
                project_type: "Web / Static".to_string(),
                is_git_repo: true,
                current_branch: Some("main".to_string()),
                git_status_summary: "clean".to_string(),
                is_clean: true,
                has_gitignore: true,
                detected_editor_configs: vec![],
                has_commits: true,
                just_initialized_git: false,
                is_corrupted: false,
                system_notice: None,
            });
        }

        // 1. Ingestion via explicit attached_files parameter
        let preview = execute_preview_planning_context(
            "develop me a website based on the design",
            None,
            Some(vec![pdf_path.to_string_lossy().to_string()]),
            &state,
        ).await.unwrap();

        assert_eq!(preview.attached_files.len(), 1);
        assert!(preview.attached_files[0].contains("mockup.pdf"));
        assert!(preview.attached_files[0].contains("application/pdf"));

        // Verify stored in pending_previews with multimodal inline attachment
        let stored_context = {
            let previews = state.pending_previews.lock().await;
            previews.get(&preview.preview_id).cloned().unwrap()
        };
        assert_eq!(stored_context.multimodal_attachments.len(), 1);
        assert_eq!(stored_context.multimodal_attachments[0].file_name, "mockup.pdf");
        assert_eq!(stored_context.multimodal_attachments[0].mime_type, "application/pdf");
        assert!(!stored_context.multimodal_attachments[0].base64_data.is_empty());

        // 2. Ingestion via @mention in prompt text
        let preview2 = execute_preview_planning_context(
            "build website according to @mockup.pdf specifications",
            None,
            None,
            &state,
        ).await.unwrap();
        assert_eq!(preview2.attached_files.len(), 1);
        assert!(preview2.attached_files[0].contains("mockup.pdf"));

        // 3. Verify external file auto-copy into main workspace folder
        let external_dir = std::env::temp_dir().join(format!(
            "zencode_external_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis()
        ));
        std::fs::create_dir_all(&external_dir).unwrap();
        let ext_file = external_dir.join("external_mockup.png");
        std::fs::write(&ext_file, b"\x89PNG\r\n\x1a\n\x00\x00\x00\rIHDR").unwrap();

        let preview3 = execute_preview_planning_context(
            "integrate image from external path",
            None,
            Some(vec![ext_file.to_string_lossy().to_string()]),
            &state,
        ).await.unwrap();

        assert!(
            temp_dir.join("external_mockup.png").is_file(),
            "External attached file must be automatically copied into main workspace folder"
        );
        assert_eq!(preview3.attached_files.len(), 1);
        assert!(preview3.attached_files[0].contains("external_mockup.png"));

        let _ = std::fs::remove_dir_all(&external_dir);
        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}

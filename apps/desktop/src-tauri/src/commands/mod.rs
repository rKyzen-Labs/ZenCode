use std::path::Path;
use std::process::Command;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter, State};
use zencode_core::{
    CredentialService, ImplementationPlan, Operation, PlanningContext, redact_secrets,
};

use crate::engine::EngineSupervisor;
use crate::models::{
    ContextPreview, EngineHealth, PlanSubmissionResult, ProjectState, SystemStatus, TaskState,
    WorkerEventPayload,
};
use crate::state::AppState;

/// Open a local project directory, detect its characteristics and Git state.
/// Does not start any AI process.
#[tauri::command]
pub async fn open_project(
    path: String,
    state: State<'_, AppState>,
) -> Result<ProjectState, String> {
    let p = Path::new(&path);
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

    // Check Git state using argument arrays
    let is_git_repo = Command::new("git")
        .args(["-C", &path, "rev-parse", "--is-inside-work-tree"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);

    let current_branch = if is_git_repo {
        Command::new("git")
            .args(["-C", &path, "branch", "--show-current"])
            .output()
            .ok()
            .and_then(|o| {
                if o.status.success() {
                    let s = String::from_utf8_lossy(&o.stdout).trim().to_string();
                    if !s.is_empty() { Some(s) } else { None }
                } else {
                    None
                }
            })
    } else {
        None
    };

    let git_status_summary = if is_git_repo {
        Command::new("git")
            .args(["-C", &path, "status", "--short"])
            .output()
            .ok()
            .map(|o| {
                let lines = String::from_utf8_lossy(&o.stdout);
                let count = lines.lines().count();
                if count == 0 {
                    "Clean working tree".to_string()
                } else {
                    format!("{count} modified/untracked file(s)")
                }
            })
            .unwrap_or_else(|| "Unknown Git status".to_string())
    } else {
        "Not a Git repository".to_string()
    };

    let project_state = ProjectState {
        path: path.clone(),
        name,
        project_type,
        is_git_repo,
        current_branch,
        git_status_summary,
    };

    // Store in AppState
    {
        let mut active = state.active_project.lock().await;
        *active = Some(project_state.clone());
    }

    Ok(project_state)
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
        worker_model: "qwen2.5-coder:7b".to_string(),
    })
}

/// Assembles the Gemini context package locally for user inspection.
/// Makes zero network calls.
#[tauri::command]
pub async fn preview_planning_context(
    task_description: String,
    state: State<'_, AppState>,
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
        task_description: task_description.clone(),
        project_type: active_project.project_type,
        shallow_tree: shallow_tree.clone(),
        manifest_summaries: manifest_summaries.clone(),
        git_branch_and_status,
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
        task_description,
        preview_text,
        shallow_tree,
        manifest_summaries,
    })
}

/// Core implementation of submit_plan_request.
/// Separated for direct unit testing of confirmation enforcement.
pub async fn execute_submit_plan(
    context_preview_id: &str,
    confirmed: bool,
    state: &AppState,
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

    // Execute 1 explicit planning request
    let plan_result = state.planner_client.plan(&api_key, &context).await;

    match plan_result {
        Ok((plan, usage)) => {
            state.increment_request_count();
            {
                let mut task = state.task_state.lock().await;
                *task = TaskState::AwaitingReview;
            }
            Ok(PlanSubmissionResult { plan, usage })
        }
        Err(e) => {
            let sanitized = redact_secrets(&e.to_string(), Some(&api_key));
            {
                let mut task = state.task_state.lock().await;
                *task = TaskState::Failed(sanitized.clone());
            }
            Err(format!("Planning request failed: {sanitized}"))
        }
    }
}

/// Submits the confirmed planning request to Gemini.
/// Enforces confirmed == true in Rust.
#[tauri::command]
pub async fn submit_plan_request(
    context_preview_id: String,
    confirmed: bool,
    state: State<'_, AppState>,
) -> Result<PlanSubmissionResult, String> {
    execute_submit_plan(&context_preview_id, confirmed, &state).await
}

/// Starts an OpenCode worker session restricted strictly to local Ollama.
#[tauri::command]
pub async fn start_worker_session(
    plan: ImplementationPlan,
    app_handle: AppHandle,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let project_dir = {
        let guard = state.active_project.lock().await;
        guard
            .as_ref()
            .map(|p| p.path.clone())
            .ok_or_else(|| "No active project open".to_string())?
    };

    // 1. Ensure engine supervisor is active
    let mut supervisor_guard = state.engine_supervisor.lock().await;
    if supervisor_guard.is_none() {
        let sup = EngineSupervisor::start(Some(Path::new(&project_dir)))
            .await
            .map_err(|e| format!("Failed to launch OpenCode sidecar: {e}"))?;
        *supervisor_guard = Some(sup);
    }

    let supervisor = supervisor_guard.as_ref().unwrap();

    // 2. Create OpenCode session
    let session_id = supervisor
        .create_session(&project_dir)
        .await
        .map_err(|e| format!("Failed to create OpenCode worker session: {e}"))?;

    // 3. Format initial worker prompt from plan
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

    // 4. Send asynchronous prompt specifying Ollama worker model
    supervisor
        .send_prompt(&session_id, &prompt, "qwen2.5-coder:7b")
        .await
        .map_err(|e| format!("Failed to dispatch prompt to worker: {e}"))?;

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

    // 5. Spawn SSE event listener thread for real-time streaming to React
    let sse_url = supervisor.sse_event_url();
    let auth_header = supervisor.auth_header();
    let app_handle_clone = app_handle.clone();
    let ses_id_clone = session_id.clone();

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

                    let payload = WorkerEventPayload {
                        session_id: ses_id_clone.clone(),
                        event_type: event.event,
                        content: event.data,
                        timestamp_ms: now,
                    };
                    let _ = app_handle_clone.emit("worker:message", &payload);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_submit_plan_request_rejects_unconfirmed() {
        let app_state = AppState::new();

        // 1. Calling execute_submit_plan with confirmed = false MUST return an error immediately
        let result = execute_submit_plan("test_preview_123", false, &app_state).await;
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
}

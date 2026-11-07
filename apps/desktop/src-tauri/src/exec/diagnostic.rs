use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use zencode_core::{redact_secrets, CredentialService};

use super::models::DiagnosticBundle;
use crate::git::GitService;
use crate::state::AppState;

pub struct DiagnosticService;

impl DiagnosticService {
    /// Exports a complete diagnostic bundle for a task with zero-leak credential redaction.
    pub async fn export_bundle(
        task_id: &str,
        repo_path: &Path,
        state: &AppState,
    ) -> Result<(PathBuf, DiagnosticBundle), String> {
        let now_str = chrono::Utc::now().to_rfc3339();

        // 1. Get primary tree status
        let primary_status = GitService::get_primary_tree_status(repo_path).ok();
        let primary_status_json = primary_status.as_ref().map(|s| {
            serde_json::json!({
                "is_clean": s.is_clean,
                "uncommitted_files": s.uncommitted_files.len(),
            })
        });

        // 2. Get worktree info & branch
        let worktree_info = {
            let wt_map = state.active_worktrees.lock().await;
            wt_map.get(task_id).cloned()
        };

        let branch_name = worktree_info
            .as_ref()
            .map(|w| w.branch_name.clone())
            .unwrap_or_else(|| format!("zencode/task-{}", task_id));

        // 3. Get change inventory if worktree exists
        let change_inventory = if let Some(ref wt) = worktree_info {
            GitService::get_worktree_diff(Path::new(&wt.worktree_path)).ok()
        } else {
            None
        };

        let files_changed = change_inventory.as_ref().map(|c| c.files.len()).unwrap_or(0);

        // 4. Gather execution history
        let execution_history = {
            let hist = state.execution_service.execution_history.lock().await;
            hist.clone()
        };

        // 5. Gather logs
        let mut logs = Vec::new();
        logs.push(format!("Task: {}", task_id));
        logs.push(format!("Branch: {}", branch_name));
        logs.push(format!("Exported: {}", now_str));
        logs.push(format!("Primary Clean: {:?}", primary_status.as_ref().map(|s| s.is_clean)));
        for exec in &execution_history {
            logs.push(format!(
                "Exec: {} {:?} -> exit: {:?} (duration: {}ms)",
                exec.program, exec.args, exec.exit_code, exec.duration_ms
            ));
        }

        // 6. Mandatory secret redaction across the entire diagnostic bundle
        let api_key_opt = CredentialService::get_gemini_api_key().ok();
        let key_ref = api_key_opt.as_deref();

        let sanitized_logs = logs
            .into_iter()
            .map(|l| redact_secrets(&l, key_ref))
            .collect::<Vec<String>>();

        let sanitized_history = execution_history
            .into_iter()
            .map(|mut res| {
                res.stdout_tail = res
                    .stdout_tail
                    .into_iter()
                    .map(|l| redact_secrets(&l, key_ref))
                    .collect();
                res.stderr_tail = res
                    .stderr_tail
                    .into_iter()
                    .map(|l| redact_secrets(&l, key_ref))
                    .collect();
                if let Some(ref err) = res.error_message {
                    res.error_message = Some(redact_secrets(err, key_ref));
                }
                res
            })
            .collect::<Vec<_>>();

        let raw_payload = serde_json::json!({
            "task_id": task_id,
            "exported_at": now_str,
            "primary_tree_status": primary_status_json,
            "worktree_branch": branch_name,
            "files_changed": files_changed,
            "logs": sanitized_logs,
            "execution_history": sanitized_history,
        });

        let raw_json_str = serde_json::to_string_pretty(&raw_payload)
            .map_err(|e| format!("Serialization error: {}", e))?;

        let fully_redacted_json = redact_secrets(&raw_json_str, key_ref);

        // Strict verification: Ensure no credentials appear in logs or exported diagnostics
        if let Some(key) = key_ref {
            if !key.is_empty() && fully_redacted_json.contains(key) {
                return Err("CRITICAL SECURITY ERROR: Diagnostic bundle failed redaction assertion. API key was detected!".into());
            }
        }

        // Save bundle to .zencode-diagnostics directory
        let diag_dir = repo_path.join(".zencode-diagnostics");
        if !diag_dir.exists() {
            let _ = std::fs::create_dir_all(&diag_dir);
        }

        let bundle_filename = format!(
            "{}_diagnostic_{}.json",
            task_id,
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs()
        );
        let bundle_path = diag_dir.join(&bundle_filename);

        std::fs::write(&bundle_path, &fully_redacted_json)
            .map_err(|e| format!("Failed to write diagnostic bundle to {:?}: {}", bundle_path, e))?;

        let bundle = DiagnosticBundle {
            task_id: task_id.to_string(),
            exported_at: now_str,
            is_clean: primary_status.as_ref().map(|s| s.is_clean).unwrap_or(true),
            primary_tree_status: primary_status_json,
            worktree_branch: Some(branch_name),
            files_changed,
            logs: sanitized_logs,
            execution_history: sanitized_history,
            raw_json: fully_redacted_json,
        };

        Ok((bundle_path, bundle))
    }
}

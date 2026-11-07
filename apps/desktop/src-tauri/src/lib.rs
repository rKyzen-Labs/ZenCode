pub mod commands;
pub mod db;
pub mod engine;
pub mod exec;
pub mod git;
pub mod models;
pub mod planner;
pub mod state;

use commands::*;
use state::AppState;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app_state = AppState::new();

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .manage(app_state)
        .invoke_handler(tauri::generate_handler![
            open_project,
            get_project_state,
            preview_planning_context,
            submit_plan_request,
            start_worker_session,
            stop_worker_session,
            get_quota_state,
            set_pinned_model,
            get_change_inventory,
            accept_task,
            reject_task,
            get_primary_tree_status,
            stash_primary_changes,
            commit_primary_changes,
            list_active_worktrees_command,
            recover_orphaned_worktree,
            detect_project_commands,
            request_command_approval,
            approve_and_run_command,
            cancel_command_execution,
            export_diagnostic_bundle,
            preview_advisory_context,
            submit_advisory_request,
            list_workspaces,
            list_chats,
            save_chat,
            load_chat,
            delete_chat,
            init_git_repo,
            create_standard_gitignore,
        ])
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::Destroyed = event {
                // Ensure engine supervisor terminates cleanly when main window is closed
                let state: tauri::State<AppState> = window.state();
                let supervisor_arc = state.engine_supervisor.clone();
                let mut lock_result = supervisor_arc.try_lock();
                if let Ok(ref mut supervisor) = lock_result {
                    if let Some(mut sup) = supervisor.take() {
                        sup.shutdown();
                    }
                }
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running zencode desktop application");
}

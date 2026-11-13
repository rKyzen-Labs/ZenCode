pub mod commands;
pub mod db;
pub mod engine;
pub mod exec;
pub mod git;
pub mod models;
pub mod planner;
pub mod state;
pub mod worker;

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
            undo_task_changes,
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
            send_advisory_to_worker,
            list_workspaces,
            set_workspace_context_preview_trusted,
            is_workspace_context_preview_trusted,
            list_chats,
            save_chat,
            load_chat,
            delete_chat,
            add_chat_message,
            get_chat_messages,
            summarize_task,
            list_all_recent_chats,
            get_workspace_file_tree,
            read_workspace_file,
            remove_workspace,
            search_chats,
            get_gemini_key_status,
            set_gemini_api_key_command,
            get_groq_key_status,
            set_groq_api_key_command,
            delete_groq_api_key_command,
            validate_gemini_api_key,
            validate_groq_api_key,
            get_openrouter_key_status,
            set_openrouter_api_key_command,
            delete_openrouter_api_key_command,
            validate_openrouter_api_key,
            refresh_startup_provider_limits,
            get_workspace_worker_config,
            set_workspace_worker_config,
            list_installed_ollama_models,
            get_worker_model,
            set_worker_model,
            window_minimize,
            window_toggle_maximize,
            window_close,
            pick_folder,
            pick_file,
            verify_startup_dependencies,
            init_git_repo,
            create_initial_empty_commit,
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

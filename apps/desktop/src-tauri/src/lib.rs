pub mod commands;
pub mod engine;
pub mod models;
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

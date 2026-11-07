use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use tokio::sync::Mutex;
use zencode_core::PlanningContext;

use crate::db::DatabaseService;
use crate::engine::EngineSupervisor;
use crate::exec::ExecutionService;
use crate::git::{ChangeInventory, WorktreeInfo};
use crate::models::{AdvisoryPreview, ProjectState, TaskState};
use crate::planner::CascadePlannerClient;

pub struct AppState {
    pub active_project: Arc<Mutex<Option<ProjectState>>>,
    pub engine_supervisor: Arc<Mutex<Option<EngineSupervisor>>>,
    pub planner_client: Arc<CascadePlannerClient>,
    pub task_state: Arc<Mutex<TaskState>>,
    pub request_count: Arc<AtomicU32>,
    pub pending_previews: Arc<Mutex<HashMap<String, PlanningContext>>>,
    pub pending_advisory_previews: Arc<Mutex<HashMap<String, AdvisoryPreview>>>,
    pub active_worktrees: Arc<Mutex<HashMap<String, WorktreeInfo>>>,
    pub active_task_inventories: Arc<Mutex<HashMap<String, ChangeInventory>>>,
    pub current_task_id: Arc<Mutex<Option<String>>>,
    pub execution_service: Arc<ExecutionService>,
    pub db: Arc<DatabaseService>,
}

impl AppState {
    pub fn new() -> Self {
        let db_path = std::env::var("LOCALAPPDATA")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|_| std::path::PathBuf::from("."))
            .join("Zencode")
            .join("zencode.db");

        let db = DatabaseService::new(db_path)
            .unwrap_or_else(|_| DatabaseService::in_memory().expect("in memory sqlite"));

        Self {
            active_project: Arc::new(Mutex::new(None)),
            engine_supervisor: Arc::new(Mutex::new(None)),
            planner_client: Arc::new(CascadePlannerClient::new()),
            task_state: Arc::new(Mutex::new(TaskState::Idle)),
            request_count: Arc::new(AtomicU32::new(0)),
            pending_previews: Arc::new(Mutex::new(HashMap::new())),
            pending_advisory_previews: Arc::new(Mutex::new(HashMap::new())),
            active_worktrees: Arc::new(Mutex::new(HashMap::new())),
            active_task_inventories: Arc::new(Mutex::new(HashMap::new())),
            current_task_id: Arc::new(Mutex::new(None)),
            execution_service: Arc::new(ExecutionService::new()),
            db: Arc::new(db),
        }
    }

    pub fn increment_request_count(&self) -> u32 {
        self.request_count.fetch_add(1, Ordering::SeqCst) + 1
    }

    pub fn get_request_count(&self) -> u32 {
        self.request_count.load(Ordering::SeqCst)
    }
}

use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use tokio::sync::Mutex;
use zencode_core::{GeminiPlannerClient, PlanningContext};

use crate::engine::EngineSupervisor;
use crate::models::{ProjectState, TaskState};

pub struct AppState {
    pub active_project: Arc<Mutex<Option<ProjectState>>>,
    pub engine_supervisor: Arc<Mutex<Option<EngineSupervisor>>>,
    pub planner_client: Arc<GeminiPlannerClient>,
    pub task_state: Arc<Mutex<TaskState>>,
    pub request_count: Arc<AtomicU32>,
    pub pending_previews: Arc<Mutex<HashMap<String, PlanningContext>>>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            active_project: Arc::new(Mutex::new(None)),
            engine_supervisor: Arc::new(Mutex::new(None)),
            planner_client: Arc::new(GeminiPlannerClient::new()),
            task_state: Arc::new(Mutex::new(TaskState::Idle)),
            request_count: Arc::new(AtomicU32::new(0)),
            pending_previews: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn increment_request_count(&self) -> u32 {
        self.request_count.fetch_add(1, Ordering::SeqCst) + 1
    }

    pub fn get_request_count(&self) -> u32 {
        self.request_count.load(Ordering::SeqCst)
    }
}

use serde::{Deserialize, Serialize};
use zencode_core::{ImplementationPlan, UsageMetadata};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProjectState {
    pub path: String,
    pub name: String,
    pub project_type: String,
    pub is_git_repo: bool,
    pub current_branch: Option<String>,
    pub git_status_summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "status", content = "detail")]
pub enum TaskState {
    Idle,
    Planning,
    WorkerRunning { session_id: String },
    AwaitingReview,
    Complete,
    Failed(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum EngineHealth {
    NotStarted,
    Starting,
    Ready { port: u16, version: String },
    Busy,
    Failed(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemStatus {
    pub active_project: Option<ProjectState>,
    pub task_state: TaskState,
    pub request_count: u32,
    pub gemini_status: String,
    pub ollama_status: String,
    pub opencode_health: EngineHealth,
    pub planner_model: String,
    pub worker_model: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextPreview {
    pub preview_id: String,
    pub task_description: String,
    pub preview_text: String,
    pub shallow_tree: Vec<String>,
    pub manifest_summaries: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerEventPayload {
    pub session_id: String,
    pub event_type: String, // "text_delta", "tool_call", "status", "completed", "error"
    pub content: String,
    pub timestamp_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanSubmissionResult {
    pub plan: ImplementationPlan,
    pub usage: UsageMetadata,
}

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceRecord {
    pub path: String,
    pub name: String,
    pub last_opened_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatSummaryRecord {
    pub id: String, // task_id
    pub workspace_path: String,
    pub title: String,
    pub created_at: String,
    pub updated_at: String,
    pub status: String, // "idle" | "active" | "review" | "merged" | "rejected" | "failed"
    pub branch_name: Option<String>,
    pub worktree_path: Option<String>,
    pub files_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SaveChatRequest {
    pub id: String,
    pub workspace_path: String,
    pub title: String,
    pub status: String,
    pub branch_name: Option<String>,
    pub worktree_path: Option<String>,
    pub plan_json: Option<String>,
    pub worker_summary: Option<String>,
    pub events_json: Option<String>,
    pub verification_json: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatDetailRecord {
    pub id: String,
    pub workspace_path: String,
    pub title: String,
    pub created_at: String,
    pub updated_at: String,
    pub status: String,
    pub branch_name: Option<String>,
    pub worktree_path: Option<String>,
    pub plan_json: Option<String>,
    pub worker_summary: Option<String>,
    pub events_json: Option<String>,
    pub verification_json: Option<String>,
}
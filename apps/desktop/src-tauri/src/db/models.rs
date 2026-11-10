use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceRecord {
    pub path: String,
    pub name: String,
    pub last_opened_at: String,
    #[serde(default)]
    pub is_context_preview_trusted: bool,
    #[serde(default = "default_worker_type_local")]
    pub default_worker_type: String, // "local" | "groq"
    #[serde(default = "default_groq_model")]
    pub groq_model: Option<String>,
}

fn default_worker_type_local() -> String {
    "local".to_string()
}

fn default_groq_model() -> Option<String> {
    Some("openai/gpt-oss-120b".to_string())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatSummaryRecord {
    pub id: String, // task_id or chat_id
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessageRecord {
    pub id: String,
    pub chat_id: String,
    pub role: String, // "user" | "assistant"
    pub content: String,
    pub plan_json: Option<String>,
    pub worker_summary: Option<String>,
    pub files_changed_json: Option<String>,
    pub created_at: String,
    #[serde(default)]
    pub commit_hash: Option<String>,
}

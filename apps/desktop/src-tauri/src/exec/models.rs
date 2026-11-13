use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BuildSystemType {
    Cargo,
    Pnpm,
    Npm,
    Yarn,
    Bun,
    Go,
    Python,
    AndroidGradle,
    Flutter,
    IosXcode,
    Maven,
    CMake,
    Unknown,
}

impl BuildSystemType {
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Cargo => "Cargo (Rust)",
            Self::Pnpm => "pnpm (Node.js)",
            Self::Npm => "npm (Node.js)",
            Self::Yarn => "Yarn (Node.js)",
            Self::Bun => "Bun",
            Self::Go => "Go",
            Self::Python => "Python",
            Self::AndroidGradle => "Android (Gradle)",
            Self::Flutter => "Flutter",
            Self::IosXcode => "Xcode / Swift",
            Self::Maven => "Maven (Java)",
            Self::CMake => "CMake (C/C++)",
            Self::Unknown => "Generic",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecommendedCommand {
    pub name: String,
    pub program: String,
    pub args: Vec<String>,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildCommandsInfo {
    pub build_system: BuildSystemType,
    pub build_system_name: String,
    pub recommended_commands: Vec<RecommendedCommand>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandRequest {
    pub id: String,
    pub program: String,
    pub args: Vec<String>,
    pub cwd: String,
    pub description: String,
    pub timeout_seconds: Option<u64>,
    pub auto_approved: bool,
    pub security_class: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CommandApprovalDecision {
    Approved,
    Denied(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessOutputEvent {
    pub command_id: String,
    pub stream: String, // "stdout" or "stderr"
    pub line: String,
    pub timestamp_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionResult {
    pub command_id: String,
    pub program: String,
    pub args: Vec<String>,
    pub cwd: String,
    pub exit_code: Option<i32>,
    pub success: bool,
    pub duration_ms: u64,
    pub stdout_tail: Vec<String>,
    pub stderr_tail: Vec<String>,
    pub cancelled: bool,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagnosticBundle {
    pub task_id: String,
    pub exported_at: String,
    pub is_clean: bool,
    pub primary_tree_status: Option<serde_json::Value>,
    pub worktree_branch: Option<String>,
    pub files_changed: usize,
    pub logs: Vec<String>,
    pub execution_history: Vec<ExecutionResult>,
    pub raw_json: String,
}

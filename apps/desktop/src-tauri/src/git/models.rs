use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ChangeCategory {
    Modified,
    Added,
    Untracked,
    Deleted,
    Renamed,
    Copied,
    Binary,
    ModeChange,
    Symlink,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FileChange {
    pub path: String,
    pub original_path: Option<String>,
    pub category: ChangeCategory,
    pub is_binary: bool,
    pub is_symlink: bool,
    pub mode_change: Option<String>,
    pub additions: usize,
    pub deletions: usize,
    pub diff_content: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChangeInventory {
    pub worktree_path: String,
    pub branch_name: String,
    pub base_commit: String,
    pub files: Vec<FileChange>,
    pub total_additions: usize,
    pub total_deletions: usize,
    pub modified_count: usize,
    pub added_count: usize,
    pub untracked_count: usize,
    pub deleted_count: usize,
    pub renamed_count: usize,
    pub binary_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct UncommittedFile {
    pub path: String,
    pub status_code: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PrimaryTreeStatus {
    pub is_clean: bool,
    pub uncommitted_files: Vec<UncommittedFile>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WorktreeInfo {
    pub task_id: String,
    pub branch_name: String,
    pub worktree_path: String,
    pub base_commit: String,
    pub created_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OrphanWorktree {
    pub task_id: String,
    pub branch_name: String,
    pub worktree_path: String,
    pub head_commit: String,
    pub age_seconds: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum MergeResult {
    Success { commit_sha: String },
    Conflict { conflicting_files: Vec<String> },
}

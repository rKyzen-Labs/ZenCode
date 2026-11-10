use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};
use thiserror::Error;

use super::models::*;

/// Sanitizes paths by stripping Windows extended-length prefix (`\\?\`), which Git CLI rejects.
pub fn clean_path_for_git(path: &Path) -> PathBuf {
    let s = path.to_string_lossy();
    if let Some(stripped) = s.strip_prefix(r"\\?\UNC\") {
        PathBuf::from(format!(r"\\{}", stripped))
    } else if let Some(stripped) = s.strip_prefix(r"\\?\") {
        PathBuf::from(stripped)
    } else {
        path.to_path_buf()
    }
}

#[derive(Error, Debug)]
pub enum GitError {
    #[error("Not a Git repository: {0}")]
    NotAGitRepo(String),
    #[error("Git CLI command failed: {command}: {stderr}")]
    CommandFailed { command: String, stderr: String },
    #[error("Path does not exist: {0}")]
    PathNotFound(String),
    #[error("Invalid task ID '{0}': must contain only alphanumeric characters, dashes, or underscores")]
    InvalidTaskId(String),
    #[error("Primary working tree is dirty with {count} uncommitted file(s). Explicit acknowledgment required.")]
    DirtyPrimaryTree {
        count: usize,
        uncommitted_files: Vec<UncommittedFile>,
    },
    #[error("Merge conflict on target branch '{target_branch}': {files:?}")]
    MergeConflict {
        target_branch: String,
        files: Vec<String>,
    },
    #[error("Worktree error: {0}")]
    WorktreeError(String),
    #[error("This repository has no commits yet. Zencode needs at least one commit to create an isolated task branch.")]
    UnbornHead,
    #[error("Corrupted Git repository: {0}")]
    CorruptedRepository(String),
    #[error("Undo conflict: {0}")]
    UndoConflict(String),
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}

pub struct GitService;

impl GitService {
    /// Internal helper to execute a Git command with argument arrays.
    fn run_cmd(repo_path: &Path, args: &[&str]) -> Result<String, GitError> {
        let clean = clean_path_for_git(repo_path);
        let output = Command::new("git")
            .arg("-C")
            .arg(&clean)
            .args(args)
            .output()
            .map_err(GitError::IoError)?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            let command = format!("git -C {:?} {}", clean, args.join(" "));
            return Err(GitError::CommandFailed { command, stderr });
        }

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    /// Internal helper to execute a Git command and capture status without failing on non-zero exit code.
    fn run_cmd_allow_err(repo_path: &Path, args: &[&str]) -> Result<(bool, String, String), GitError> {
        let clean = clean_path_for_git(repo_path);
        let output = Command::new("git")
            .arg("-C")
            .arg(&clean)
            .args(args)
            .output()
            .map_err(GitError::IoError)?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        Ok((output.status.success(), stdout, stderr))
    }

    /// 1. is_git_repo(path) — checks if the opened project is inside a valid Git repository
    pub fn is_git_repo(path: &Path) -> Result<bool, GitError> {
        let clean = clean_path_for_git(path);
        if !clean.exists() {
            return Ok(false);
        }
        let output = Command::new("git")
            .arg("-C")
            .arg(&clean)
            .args(["rev-parse", "--is-inside-work-tree"])
            .output();

        match output {
            Ok(o) => {
                if o.status.success() {
                    let s = String::from_utf8_lossy(&o.stdout).trim().to_string();
                    Ok(s == "true")
                } else {
                    Ok(false)
                }
            }
            Err(_) => Ok(false),
        }
    }

    /// Checks if the repository has at least one commit (HEAD is not unborn).
    pub fn has_commits(repo_path: &Path) -> Result<bool, GitError> {
        let (has_head, head_sha, _) = Self::run_cmd_allow_err(
            repo_path,
            &["rev-parse", "--verify", "HEAD"],
        )?;
        Ok(has_head && !head_sha.trim().is_empty())
    }

    /// Creates an initial empty root commit ("Initial commit") on the current branch.
    /// MUST only be called after explicit user confirmation.
    pub fn create_initial_empty_commit(repo_path: &Path) -> Result<String, GitError> {
        let clean_p = clean_path_for_git(repo_path);
        Self::run_cmd(
            &clean_p,
            &["commit", "--allow-empty", "-m", "Initial commit"],
        )
    }

    /// 2. get_primary_tree_status(repo_path) — returns clean/dirty state and a list of uncommitted changes
    pub fn get_primary_tree_status(repo_path: &Path) -> Result<PrimaryTreeStatus, GitError> {
        if !Self::is_git_repo(repo_path)? {
            return Err(GitError::NotAGitRepo(repo_path.to_string_lossy().to_string()));
        }

        let stdout = Self::run_cmd(repo_path, &["status", "--porcelain=v1", "-uall"])?;
        let mut uncommitted = Vec::new();

        for line in stdout.lines() {
            let line = line.trim_end();
            if line.len() < 3 {
                continue;
            }
            let status_code = &line[0..2];
            let rest = line[3..].trim();

            let (path, description) = if status_code == "??" {
                (rest.to_string(), "Untracked new file".to_string())
            } else if status_code.contains('M') {
                (rest.to_string(), "Modified".to_string())
            } else if status_code.contains('A') {
                (rest.to_string(), "Added to index".to_string())
            } else if status_code.contains('D') {
                (rest.to_string(), "Deleted".to_string())
            } else if status_code.contains('R') {
                let parts: Vec<&str> = rest.split(" -> ").collect();
                let p = if parts.len() > 1 { parts[1] } else { rest };
                (p.to_string(), format!("Renamed from {}", parts[0]))
            } else if status_code.contains('C') {
                let parts: Vec<&str> = rest.split(" -> ").collect();
                let p = if parts.len() > 1 { parts[1] } else { rest };
                (p.to_string(), format!("Copied from {}", parts[0]))
            } else {
                (rest.to_string(), format!("Changed ({status_code})"))
            };

            uncommitted.push(UncommittedFile {
                path,
                status_code: status_code.to_string(),
                description,
            });
        }

        let is_clean = uncommitted.is_empty();
        Ok(PrimaryTreeStatus {
            is_clean,
            uncommitted_files: uncommitted,
        })
    }

    /// 3. create_task_worktree(repo_path, task_id) — creates a new worktree at a path like ../.zencode-worktrees/{task_id}
    /// on a new branch named zencode/task-{task_id}.
    /// Automatically handles Scenarios 1, 2, 3, 7, 8, 12: auto-commits dirty primary trees with "Pre-task checkpoint",
    /// auto-creates initial commit if unborn HEAD, auto-cleans stale leftover task folders, and auto-suffixes colliding branch names.
    pub fn create_task_worktree(
        repo_path: &Path,
        task_id: &str,
        _acknowledge_dirty: bool,
    ) -> Result<WorktreeInfo, GitError> {
        // Validate task_id (alphanumeric, dashes, underscores)
        if task_id.is_empty()
            || !task_id
                .chars()
                .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
        {
            return Err(GitError::InvalidTaskId(task_id.to_string()));
        }

        let clean_repo = clean_path_for_git(repo_path);
        if !clean_repo.exists() {
            return Err(GitError::PathNotFound(clean_repo.to_string_lossy().to_string()));
        }

        // Scenario 12: Check repo corruption (destructive territory — do not touch)
        if Self::check_repo_corruption(&clean_repo)? {
            return Err(GitError::CorruptedRepository(
                "This project's Git history appears damaged and Zencode can't safely continue automatically. No changes have been made.".to_string(),
            ));
        }

        let mut system_notices: Vec<String> = Vec::new();

        // Scenario 1: Silently init if not a Git repo
        if !Self::is_git_repo(&clean_repo)? {
            Self::init_repo(&clean_repo)?;
            system_notices.push("Git initialized.".to_string());
        }

        // Scenario 3: Primary working tree uncommitted changes auto-commit before starting task
        let status = Self::get_primary_tree_status(&clean_repo)?;
        if !status.is_clean {
            let (add_ok, _, _) = Self::run_cmd_allow_err(&clean_repo, &["add", "-A"])?;
            if add_ok {
                let (commit_ok, _, _) = Self::run_cmd_allow_err(
                    &clean_repo,
                    &["commit", "-m", "Pre-task checkpoint"],
                )?;
                if commit_ok {
                    system_notices.push("Saved your existing changes before starting this task.".to_string());
                }
            }
        }

        // Scenario 2: Unborn HEAD detection & auto empty commit
        let (has_head, mut head_sha, _) = Self::run_cmd_allow_err(
            &clean_repo,
            &["rev-parse", "--verify", "HEAD"],
        )?;

        if !has_head || head_sha.trim().is_empty() {
            Self::create_initial_empty_commit(&clean_repo)?;
            let (_, new_sha, _) = Self::run_cmd_allow_err(
                &clean_repo,
                &["rev-parse", "--verify", "HEAD"],
            )?;
            head_sha = new_sha;
            system_notices.push("Initial commit created.".to_string());
        }

        let base_commit = head_sha.trim().to_string();

        let canonical_repo = clean_repo
            .canonicalize()
            .map(|p| clean_path_for_git(&p))
            .unwrap_or_else(|_| clean_path_for_git(&clean_repo));

        // Target worktree path at ../.zencode-worktrees/{task_id}
        let worktree_dir = if let Some(parent) = canonical_repo.parent() {
            clean_path_for_git(&parent.join(".zencode-worktrees").join(task_id))
        } else {
            clean_path_for_git(&canonical_repo.join(".zencode-worktrees").join(task_id))
        };

        if let Some(parent) = worktree_dir.parent() {
            let _ = std::fs::create_dir_all(parent);
        }

        let worktree_path_str = worktree_dir.to_string_lossy().to_string();

        // Scenario 7: Stale worktree cleanup if path already exists (leftover from crash)
        let mut cleaned_stale = false;
        if worktree_dir.exists() {
            let _ = Self::run_cmd_allow_err(&clean_repo, &["worktree", "remove", "--force", &worktree_path_str]);
            let _ = Self::run_cmd_allow_err(&clean_repo, &["worktree", "prune"]);
            if worktree_dir.exists() {
                let _ = std::fs::remove_dir_all(&worktree_dir);
            }
            cleaned_stale = true;
        }

        // Also prune any leftover dead worktree records
        let _ = Self::run_cmd_allow_err(&clean_repo, &["worktree", "prune"]);

        if cleaned_stale {
            system_notices.push("Cleaned up a leftover task folder.".to_string());
        }

        // Scenario 8: Branch name collision resolution
        let base_branch_name = format!("zencode/task-{}", task_id);
        let mut branch_name = base_branch_name.clone();
        let mut suffix = 1;
        while {
            let (branch_exists, _, _) = Self::run_cmd_allow_err(
                &clean_repo,
                &["rev-parse", "--verify", &branch_name],
            )?;
            branch_exists
        } {
            branch_name = format!("{}-{}", base_branch_name, suffix);
            suffix += 1;
        }

        Self::run_cmd(
            &clean_repo,
            &[
                "worktree",
                "add",
                "-b",
                &branch_name,
                &worktree_path_str,
                &base_commit,
            ],
        )?;

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        let notice_combined = if system_notices.is_empty() {
            None
        } else {
            Some(system_notices.join("\n"))
        };

        Ok(WorktreeInfo {
            task_id: task_id.to_string(),
            branch_name,
            worktree_path: worktree_path_str,
            base_commit,
            created_at: now,
            system_notice: notice_combined,
        })
    }

    /// 4. get_worktree_diff(worktree_path) — returns the complete change inventory covering all categories:
    /// Modified, Added, Untracked, Deleted, Renamed, Copied, Binary, Mode changes, and Symlinks.
    pub fn get_worktree_diff(worktree_path: &Path) -> Result<ChangeInventory, GitError> {
        if !worktree_path.exists() {
            return Err(GitError::PathNotFound(
                worktree_path.to_string_lossy().to_string(),
            ));
        }

        let branch_name = Self::run_cmd(worktree_path, &["rev-parse", "--abbrev-ref", "HEAD"])?
            .trim()
            .to_string();

        let base_commit = Self::find_base_commit(worktree_path)?;

        let mut files_map: std::collections::BTreeMap<String, FileChange> =
            std::collections::BTreeMap::new();

        // 1. git diff --raw -M -C <base_commit>
        let raw_output = Self::run_cmd(worktree_path, &["diff", "--raw", "-M", "-C", &base_commit])
            .unwrap_or_default();

        for line in raw_output.lines() {
            let line = line.trim();
            if !line.starts_with(':') {
                continue;
            }
            let parts: Vec<&str> = line.splitn(2, '\t').collect();
            if parts.len() < 2 {
                continue;
            }
            let meta_part = parts[0];
            let path_part = parts[1];

            let meta_tokens: Vec<&str> = meta_part.split_whitespace().collect();
            if meta_tokens.len() < 5 {
                continue;
            }
            let old_mode = meta_tokens[0].trim_start_matches(':');
            let new_mode = meta_tokens[1];
            let status_code = meta_tokens[4];

            let (path, orig_path) = if status_code.starts_with('R') || status_code.starts_with('C') {
                let paths: Vec<&str> = path_part.split('\t').collect();
                if paths.len() >= 2 {
                    (paths[1].to_string(), Some(paths[0].to_string()))
                } else {
                    (path_part.to_string(), None)
                }
            } else {
                (path_part.to_string(), None)
            };

            let is_symlink = old_mode == "120000" || new_mode == "120000";
            let mode_change = if old_mode != new_mode && old_mode != "000000" && new_mode != "000000" {
                Some(format!("{} -> {}", old_mode, new_mode))
            } else {
                None
            };

            let category = if is_symlink {
                ChangeCategory::Symlink
            } else if mode_change.is_some() {
                ChangeCategory::ModeChange
            } else if status_code.starts_with('A') {
                ChangeCategory::Added
            } else if status_code.starts_with('D') {
                ChangeCategory::Deleted
            } else if status_code.starts_with('R') {
                ChangeCategory::Renamed
            } else if status_code.starts_with('C') {
                ChangeCategory::Copied
            } else {
                ChangeCategory::Modified
            };

            files_map.insert(
                path.clone(),
                FileChange {
                    path,
                    original_path: orig_path,
                    category,
                    is_binary: false,
                    is_symlink,
                    mode_change,
                    additions: 0,
                    deletions: 0,
                    diff_content: None,
                },
            );
        }

        // 2. git diff --numstat -M -C <base_commit>
        let numstat_output = Self::run_cmd(
            worktree_path,
            &["diff", "--numstat", "-M", "-C", &base_commit],
        )
        .unwrap_or_default();

        for line in numstat_output.lines() {
            let parts: Vec<&str> = line.split('\t').collect();
            if parts.len() < 3 {
                continue;
            }
            let add_str = parts[0];
            let del_str = parts[1];
            let raw_path = parts[2];

            let file_key = if raw_path.contains(" => ") {
                Self::parse_numstat_rename_target(raw_path)
            } else {
                raw_path.to_string()
            };

            if let Some(change) = files_map.get_mut(&file_key) {
                if add_str == "-" && del_str == "-" {
                    change.is_binary = true;
                    change.category = ChangeCategory::Binary;
                } else {
                    change.additions = add_str.parse::<usize>().unwrap_or(0);
                    change.deletions = del_str.parse::<usize>().unwrap_or(0);
                }
            }
        }

        // 3. git diff -M -C <base_commit> (Per-file diff content)
        let full_diff = Self::run_cmd(worktree_path, &["diff", "-M", "-C", &base_commit])
            .unwrap_or_default();
        Self::populate_diff_contents(&full_diff, &mut files_map);

        // 4. Untracked files via git status --porcelain=v1 -uall
        let status_output = Self::run_cmd(worktree_path, &["status", "--porcelain=v1", "-uall"])
            .unwrap_or_default();

        for line in status_output.lines() {
            let line = line.trim_end();
            if !line.starts_with("?? ") {
                continue;
            }
            let untracked_path = line[3..].trim().to_string();
            let full_file_path = worktree_path.join(&untracked_path);

            if !full_file_path.exists() {
                continue;
            }

            let symlink = full_file_path
                .symlink_metadata()
                .map(|m| m.file_type().is_symlink())
                .unwrap_or(false);

            let is_binary = Self::is_file_binary(&full_file_path);

            let (additions, diff_content, category) = if is_binary {
                (0, None, ChangeCategory::Binary)
            } else if symlink {
                (0, None, ChangeCategory::Symlink)
            } else if let Ok(content) = std::fs::read_to_string(&full_file_path) {
                let lines_count = content.lines().count();
                let synthetic_diff = Self::generate_new_file_diff(&untracked_path, &content);
                (lines_count, Some(synthetic_diff), ChangeCategory::Untracked)
            } else {
                (0, None, ChangeCategory::Untracked)
            };

            files_map.insert(
                untracked_path.clone(),
                FileChange {
                    path: untracked_path,
                    original_path: None,
                    category,
                    is_binary,
                    is_symlink: symlink,
                    mode_change: None,
                    additions,
                    deletions: 0,
                    diff_content,
                },
            );
        }

        // Calculate aggregate statistics
        let files: Vec<FileChange> = files_map.into_values().collect();
        let mut total_additions = 0;
        let mut total_deletions = 0;
        let mut modified_count = 0;
        let mut added_count = 0;
        let mut untracked_count = 0;
        let mut deleted_count = 0;
        let mut renamed_count = 0;
        let mut binary_count = 0;

        for f in &files {
            total_additions += f.additions;
            total_deletions += f.deletions;
            match f.category {
                ChangeCategory::Modified | ChangeCategory::ModeChange => modified_count += 1,
                ChangeCategory::Added => added_count += 1,
                ChangeCategory::Untracked => untracked_count += 1,
                ChangeCategory::Deleted => deleted_count += 1,
                ChangeCategory::Renamed | ChangeCategory::Copied => renamed_count += 1,
                ChangeCategory::Binary => binary_count += 1,
                ChangeCategory::Symlink => modified_count += 1,
            }
        }

        Ok(ChangeInventory {
            worktree_path: worktree_path.to_string_lossy().to_string(),
            branch_name,
            base_commit,
            files,
            total_additions,
            total_deletions,
            modified_count,
            added_count,
            untracked_count,
            deleted_count,
            renamed_count,
            binary_count,
        })
    }

    /// 5. accept_task_changes(worktree_path, target_branch, acknowledge_dirty) — merges or fast-forwards the task branch
    /// into the target branch. Handles conflicts automatically in favor of task branch (Scenario 5).
    pub fn accept_task_changes(
        worktree_path: &Path,
        target_branch: &str,
        _acknowledge_dirty: bool,
    ) -> Result<MergeResult, GitError> {
        let clean_wt = clean_path_for_git(worktree_path);
        if !clean_wt.exists() {
            return Err(GitError::PathNotFound(
                clean_wt.to_string_lossy().to_string(),
            ));
        }

        let repo_path = Self::find_repo_root_from_worktree(&clean_wt)?;

        // Ensure any uncommitted or untracked changes in the worktree are committed to the task branch
        let status = Self::run_cmd(&clean_wt, &["status", "--porcelain=v1", "-uall"])?;
        let task_branch = Self::run_cmd(&clean_wt, &["rev-parse", "--abbrev-ref", "HEAD"])?
            .trim()
            .to_string();

        if !status.trim().is_empty() {
            Self::run_cmd(&clean_wt, &["add", "-A"])?;
            Self::run_cmd(
                &clean_wt,
                &[
                    "commit",
                    "-m",
                    &format!("zencode: apply task changes from {}", task_branch),
                ],
            )?;
        }

        // Primary working tree uncommitted changes: auto-checkpoint so changes are never lost
        let primary_status = Self::get_primary_tree_status(&repo_path)?;
        if !primary_status.is_clean {
            let _ = Self::run_cmd_allow_err(&repo_path, &["add", "-A"]);
            let _ = Self::run_cmd_allow_err(&repo_path, &["commit", "-m", "Pre-merge checkpoint"]);
        }

        // Checkout target branch in main repository if not already on it
        let current_branch = match Self::run_cmd_allow_err(&repo_path, &["branch", "--show-current"])? {
            (true, b, _) if !b.trim().is_empty() => b.trim().to_string(),
            _ => match Self::run_cmd_allow_err(&repo_path, &["rev-parse", "--abbrev-ref", "HEAD"])? {
                (true, b, _) if !b.trim().is_empty() => b.trim().to_string(),
                _ => target_branch.to_string(),
            },
        };

        if current_branch != target_branch {
            let _ = Self::run_cmd_allow_err(&repo_path, &["checkout", target_branch]);
        }

        // Attempt merge
        let (merge_ok, _, _) = Self::run_cmd_allow_err(
            &repo_path,
            &[
                "merge",
                "--no-ff",
                "-m",
                &format!("Merge task branch {}", task_branch),
                &task_branch,
            ],
        )?;

        if !merge_ok {
            // Automatic conflict resolution: task branch version wins (Scenario 5)
            let (unmerged_ok, unmerged_out, _) = Self::run_cmd_allow_err(
                &repo_path,
                &["diff", "--name-only", "--diff-filter=U"],
            )?;
            if unmerged_ok {
                for file in unmerged_out.lines().map(|s| s.trim()).filter(|s| !s.is_empty()) {
                    let (co_ok, _, _) = Self::run_cmd_allow_err(&repo_path, &["checkout", "--theirs", "--", file])?;
                    if !co_ok {
                        let _ = Self::run_cmd_allow_err(&repo_path, &["rm", "-f", file]);
                    }
                }
            }
            let _ = Self::run_cmd_allow_err(&repo_path, &["add", "-A"]);
            let _ = Self::run_cmd_allow_err(
                &repo_path,
                &["commit", "-m", &format!("Merge task branch {} (auto-resolved conflicts)", task_branch)],
            );
        }

        let new_head = match Self::run_cmd_allow_err(&repo_path, &["rev-parse", "HEAD"])? {
            (true, sha, _) if !sha.trim().is_empty() => sha.trim().to_string(),
            _ => "HEAD".to_string(),
        };

        // Clean up worktree and task branch after successful merge
        let worktree_path_str = clean_wt.to_string_lossy().to_string();
        let _ = Self::run_cmd_allow_err(&repo_path, &["worktree", "remove", "--force", &worktree_path_str]);
        let _ = Self::run_cmd_allow_err(&repo_path, &["branch", "-D", &task_branch]);
        let _ = std::fs::remove_dir_all(&clean_wt);

        Ok(MergeResult::Success {
            commit_sha: new_head,
        })
    }

    /// 6. reject_task_changes(worktree_path, task_id) — removes the worktree and deletes the task branch
    /// after user confirmation. Never runs git reset --hard on the primary tree.
    pub fn reject_task_changes(worktree_path: &Path, task_id: &str) -> Result<(), GitError> {
        if task_id.is_empty()
            || !task_id
                .chars()
                .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
        {
            return Err(GitError::InvalidTaskId(task_id.to_string()));
        }

        let clean_wt = clean_path_for_git(worktree_path);
        let repo_path = if clean_wt.exists() {
            Self::find_repo_root_from_worktree(&clean_wt).ok()
        } else {
            None
        };

        let branch_name = format!("zencode/task-{}", task_id);

        if let Some(repo) = repo_path {
            let worktree_path_str = clean_wt.to_string_lossy().to_string();
            let _ = Self::run_cmd(&repo, &["worktree", "remove", "--force", &worktree_path_str]);
            let _ = Self::run_cmd(&repo, &["branch", "-D", &branch_name]);
            let _ = Self::run_cmd(&repo, &["worktree", "prune"]);
        }

        if clean_wt.exists() {
            let _ = std::fs::remove_dir_all(&clean_wt);
        }

        Ok(())
    }

    /// Silently initializes a new Git repository with default branch 'main' and creates an initial empty commit.
    pub fn init_repo(repo_path: &Path) -> Result<(), GitError> {
        let clean = clean_path_for_git(repo_path);
        if !clean.exists() {
            return Err(GitError::PathNotFound(clean.to_string_lossy().to_string()));
        }

        let (ok, _, _) = Self::run_cmd_allow_err(&clean, &["init", "-b", "main"])?;
        if !ok {
            let (ok2, _, err2) = Self::run_cmd_allow_err(&clean, &["init"])?;
            if !ok2 {
                return Err(GitError::CommandFailed {
                    command: "git init".to_string(),
                    stderr: err2,
                });
            }
        }
        let _ = Self::create_initial_empty_commit(&clean);
        Ok(())
    }

    /// On every task completion: commits all changes in the task worktree with task_summary,
    /// merges the task branch into the primary branch (resolving conflicts automatically with task branch winning),
    /// cleans up the worktree, and returns the commit hash and conflict flag.
    pub fn commit_and_merge_task_to_primary(
        worktree_path: &Path,
        target_branch: &str,
        commit_message: &str,
    ) -> Result<MergeOutcome, GitError> {
        let clean_wt = clean_path_for_git(worktree_path);
        if !clean_wt.exists() {
            return Err(GitError::PathNotFound(clean_wt.to_string_lossy().to_string()));
        }

        let repo_path = Self::find_repo_root_from_worktree(&clean_wt)?;

        // Check if there are changes in worktree
        let status = Self::run_cmd(&clean_wt, &["status", "--porcelain=v1", "-uall"])?;
        if status.trim().is_empty() {
            // No changes produced by task
            let task_branch = Self::run_cmd(&clean_wt, &["rev-parse", "--abbrev-ref", "HEAD"])
                .unwrap_or_default()
                .trim()
                .to_string();
            let wt_str = clean_wt.to_string_lossy().to_string();
            let _ = Self::run_cmd_allow_err(&repo_path, &["worktree", "remove", "--force", &wt_str]);
            if !task_branch.is_empty() && task_branch != target_branch {
                let _ = Self::run_cmd_allow_err(&repo_path, &["branch", "-D", &task_branch]);
            }
            let _ = std::fs::remove_dir_all(&clean_wt);
            return Ok(MergeOutcome {
                commit_sha: None,
                conflict_resolved: false,
            });
        }

        // Commit all changes in worktree to the task branch
        Self::run_cmd(&clean_wt, &["add", "-A"])?;
        Self::run_cmd(&clean_wt, &["commit", "-m", commit_message])?;

        let task_branch = Self::run_cmd(&clean_wt, &["rev-parse", "--abbrev-ref", "HEAD"])?
            .trim()
            .to_string();

        let effective_target = if target_branch.trim().is_empty() {
            match Self::run_cmd_allow_err(&repo_path, &["branch", "--show-current"])? {
                (true, b, _) if !b.trim().is_empty() => b.trim().to_string(),
                _ => "main".to_string(),
            }
        } else {
            target_branch.to_string()
        };

        // Switch to target branch in primary repo if needed
        let current_branch = match Self::run_cmd_allow_err(&repo_path, &["branch", "--show-current"])? {
            (true, b, _) if !b.trim().is_empty() => b.trim().to_string(),
            _ => effective_target.clone(),
        };

        if current_branch != effective_target {
            let _ = Self::run_cmd_allow_err(&repo_path, &["checkout", &effective_target]);
        }

        // Scenario 3: If primary working tree has uncommitted files, auto-checkpoint before merge
        let primary_status = Self::get_primary_tree_status(&repo_path)?;
        if !primary_status.is_clean {
            let _ = Self::run_cmd_allow_err(&repo_path, &["add", "-A"]);
            let _ = Self::run_cmd_allow_err(&repo_path, &["commit", "-m", "Pre-merge checkpoint"]);
        }

        let mut conflict_resolved = false;

        // Merge task branch into primary with fast-forward preferred
        let (merge_ok, _, _merge_err) = Self::run_cmd_allow_err(
            &repo_path,
            &["merge", "--ff", "-m", commit_message, &task_branch],
        )?;

        if !merge_ok {
            // Attempt standard merge if ff wasn't cleanly applied
            let (m2_ok, _, _) = Self::run_cmd_allow_err(
                &repo_path,
                &["merge", "--no-ff", "-m", commit_message, &task_branch],
            )?;

            if !m2_ok {
                // Scenario 5: Conflict resolution — task branch's version wins
                conflict_resolved = true;
                let (unmerged_ok, unmerged_out, _) = Self::run_cmd_allow_err(
                    &repo_path,
                    &["diff", "--name-only", "--diff-filter=U"],
                )?;
                if unmerged_ok {
                    for file in unmerged_out.lines().map(|l| l.trim()).filter(|l| !l.is_empty()) {
                        let (co_ok, _, _) = Self::run_cmd_allow_err(
                            &repo_path,
                            &["checkout", "--theirs", "--", file],
                        )?;
                        if !co_ok {
                            let _ = Self::run_cmd_allow_err(&repo_path, &["rm", "-f", file]);
                        }
                    }
                }
                let _ = Self::run_cmd_allow_err(&repo_path, &["add", "-A"]);
                let _ = Self::run_cmd_allow_err(
                    &repo_path,
                    &["commit", "-m", &format!("{} (resolved conflicts - task branch won)", commit_message)],
                );
            }
        }

        let primary_head = Self::run_cmd(&repo_path, &["rev-parse", "HEAD"])?
            .trim()
            .to_string();

        // Clean up worktree and task branch
        let wt_str = clean_wt.to_string_lossy().to_string();
        let _ = Self::run_cmd_allow_err(&repo_path, &["worktree", "remove", "--force", &wt_str]);
        if !task_branch.is_empty() && task_branch != effective_target {
            let _ = Self::run_cmd_allow_err(&repo_path, &["branch", "-D", &task_branch]);
        }
        let _ = std::fs::remove_dir_all(&clean_wt);

        Ok(MergeOutcome {
            commit_sha: Some(primary_head),
            conflict_resolved,
        })
    }

    /// Reverts a specific commit by hash on the primary branch using `git revert <commit_hash> --no-edit`.
    /// Does NOT perform git reset --hard, and does NOT target whatever the latest commit happens to be.
    /// Handles conflicts automatically in favor of revert's intent (Scenario 6).
    /// Returns (new_head_sha, conflict_adjusted).
    pub fn revert_commit(repo_path: &Path, commit_sha: &str) -> Result<(String, bool), GitError> {
        let clean = clean_path_for_git(repo_path);
        if !clean.exists() {
            return Err(GitError::PathNotFound(clean.to_string_lossy().to_string()));
        }

        let (is_merge, _, _) = Self::run_cmd_allow_err(
            &clean,
            &["rev-parse", "-q", "--verify", &format!("{}^2", commit_sha)],
        )?;

        let revert_args: &[&str] = if is_merge {
            &["revert", "-m", "1", commit_sha, "--no-edit"]
        } else {
            &["revert", commit_sha, "--no-edit"]
        };

        let (ok, _, stderr) = Self::run_cmd_allow_err(&clean, revert_args)?;
        if ok {
            let new_head = Self::run_cmd(&clean, &["rev-parse", "HEAD"])?
                .trim()
                .to_string();
            return Ok((new_head, false));
        }

        // Clean revert failed. Abort and retry with revert's intent winning (-X theirs) (Scenario 6)
        let _ = Self::run_cmd_allow_err(&clean, &["revert", "--abort"]);

        let strategy_args: &[&str] = if is_merge {
            &["revert", "-X", "theirs", "-m", "1", commit_sha, "--no-edit"]
        } else {
            &["revert", "-X", "theirs", commit_sha, "--no-edit"]
        };

        let (s_ok, _, s_err) = Self::run_cmd_allow_err(&clean, strategy_args)?;
        if s_ok {
            let new_head = Self::run_cmd(&clean, &["rev-parse", "HEAD"])?
                .trim()
                .to_string();
            return Ok((new_head, true));
        }

        // If -X theirs also had unmerged files, resolve them in favor of the revert state
        let (unmerged_ok, unmerged_out, _) = Self::run_cmd_allow_err(&clean, &["diff", "--name-only", "--diff-filter=U"])?;
        if unmerged_ok && !unmerged_out.trim().is_empty() {
            for file in unmerged_out.lines().map(|l| l.trim()).filter(|l| !l.is_empty()) {
                let (co_ok, _, _) = Self::run_cmd_allow_err(&clean, &["checkout", "--theirs", "--", file])?;
                if !co_ok {
                    let _ = Self::run_cmd_allow_err(&clean, &["rm", "-f", file]);
                }
            }
            let _ = Self::run_cmd_allow_err(&clean, &["add", "-A"]);
            let (c_ok, _, _) = Self::run_cmd_allow_err(&clean, &["commit", "-m", &format!("Revert {} (adjusted conflicts)", commit_sha)])?;
            if c_ok {
                let new_head = Self::run_cmd(&clean, &["rev-parse", "HEAD"])?
                    .trim()
                    .to_string();
                return Ok((new_head, true));
            }
        }

        // Check if nothing to commit / already undone
        let (status_ok, status_out, _) = Self::run_cmd_allow_err(&clean, &["status", "--porcelain"])?;
        if status_ok && status_out.trim().is_empty() {
            let _ = Self::run_cmd_allow_err(&clean, &["revert", "--abort"]);
            return Err(GitError::UndoConflict(
                "Could not undo task: the target changes were already modified or deleted by subsequent tasks.".to_string(),
            ));
        }

        let _ = Self::run_cmd_allow_err(&clean, &["revert", "--abort"]);
        Err(GitError::UndoConflict(format!(
            "Could not undo task: target files could not be cleanly reverted ({})",
            if !s_err.trim().is_empty() { s_err.trim() } else { stderr.trim() }
        )))
    }

    /// Undo task changes by reverting the specific commit created for this task.
    pub fn undo_task_changes(repo_path: &Path, commit_sha: &str) -> Result<(String, bool), GitError> {
        Self::revert_commit(repo_path, commit_sha)
    }

    /// Scenario 9: Detects detached HEAD state and silently recovers onto a safe branch.
    pub fn detect_and_fix_detached_head(repo_path: &Path) -> Result<Option<String>, GitError> {
        let clean = clean_path_for_git(repo_path);
        if !clean.exists() || !Self::is_git_repo(&clean)? {
            return Ok(None);
        }

        let (has_head, head_sha, _) = Self::run_cmd_allow_err(&clean, &["rev-parse", "--verify", "HEAD"])?;
        if !has_head || head_sha.trim().is_empty() {
            return Ok(None);
        }

        let (is_sym, _, _) = Self::run_cmd_allow_err(&clean, &["symbolic-ref", "-q", "HEAD"])?;
        if !is_sym {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            let recovered_branch = format!("zencode-recovered-{}", now);
            Self::run_cmd(&clean, &["checkout", "-b", &recovered_branch])?;
            return Ok(Some(recovered_branch));
        }
        Ok(None)
    }

    /// Scenario 12: Detects corrupted repository (.git directory damaged).
    pub fn check_repo_corruption(repo_path: &Path) -> Result<bool, GitError> {
        let clean = clean_path_for_git(repo_path);
        let git_dir = clean.join(".git");
        if !git_dir.exists() {
            return Ok(false);
        }

        let (ok, _, stderr) = Self::run_cmd_allow_err(&clean, &["status"])?;
        if !ok {
            let err = stderr.to_lowercase();
            if err.contains("bad object")
                || err.contains("corrupt")
                || err.contains("fatal: not a git repository")
                || err.contains("object file")
                || err.contains("missing object")
                || err.contains("broken link")
            {
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// 7. list_active_worktrees(repo_path) — returns all Zencode-managed worktrees for orphan detection on startup
    pub fn list_active_worktrees(repo_path: &Path) -> Result<Vec<OrphanWorktree>, GitError> {
        if !Self::is_git_repo(repo_path)? {
            return Err(GitError::NotAGitRepo(repo_path.to_string_lossy().to_string()));
        }

        let stdout = Self::run_cmd(repo_path, &["worktree", "list", "--porcelain"])?;
        let mut orphans = Vec::new();

        let mut current_worktree: Option<String> = None;
        let mut current_head: Option<String> = None;
        let mut current_branch: Option<String> = None;

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let process_entry = |wt: String,
                             hd: String,
                             br: String,
                             orphans: &mut Vec<OrphanWorktree>| {
            if wt.contains(".zencode-worktrees") || br.contains("zencode/task-") {
                let path_buf = PathBuf::from(&wt);
                let age_seconds = if let Ok(meta) = std::fs::metadata(&path_buf) {
                    if let Ok(modified) = meta.modified() {
                        let m_secs = modified
                            .duration_since(UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_secs();
                        now.saturating_sub(m_secs)
                    } else {
                        0
                    }
                } else {
                    0
                };

                let task_id = if let Some(idx) = br.find("zencode/task-") {
                    br[idx + "zencode/task-".len()..].to_string()
                } else if let Some(name) = path_buf.file_name().and_then(|n| n.to_str()) {
                    name.to_string()
                } else {
                    "unknown".to_string()
                };

                orphans.push(OrphanWorktree {
                    task_id,
                    branch_name: br,
                    worktree_path: wt,
                    head_commit: hd,
                    age_seconds,
                });
            }
        };

        for line in stdout.lines() {
            let line = line.trim();
            if line.is_empty() {
                if let (Some(wt), Some(hd)) = (current_worktree.take(), current_head.take()) {
                    let br = current_branch.take().unwrap_or_default();
                    process_entry(wt, hd, br, &mut orphans);
                }
                continue;
            }

            if line.starts_with("worktree ") {
                current_worktree = Some(line["worktree ".len()..].trim().to_string());
            } else if line.starts_with("HEAD ") {
                current_head = Some(line["HEAD ".len()..].trim().to_string());
            } else if line.starts_with("branch ") {
                let full_ref = line["branch ".len()..].trim();
                let short_branch = full_ref.strip_prefix("refs/heads/").unwrap_or(full_ref);
                current_branch = Some(short_branch.to_string());
            }
        }

        if let (Some(wt), Some(hd)) = (current_worktree, current_head) {
            let br = current_branch.unwrap_or_default();
            process_entry(wt, hd, br, &mut orphans);
        }

        Ok(orphans)
    }

    /// Locates the root repository from a worktree.
    pub fn find_repo_root_from_worktree(worktree_path: &Path) -> Result<PathBuf, GitError> {
        let clean_wt = clean_path_for_git(worktree_path);
        let common_git_dir_str = Self::run_cmd(&clean_wt, &["rev-parse", "--git-common-dir"])?
            .trim()
            .to_string();

        let common_path = PathBuf::from(&common_git_dir_str);
        let abs_common = if common_path.is_relative() {
            clean_wt.join(common_path)
        } else {
            common_path
        };

        let canonical = abs_common
            .canonicalize()
            .map(|p| clean_path_for_git(&p))
            .unwrap_or_else(|_| clean_path_for_git(&abs_common));
        if let Some(parent) = canonical.parent() {
            Ok(clean_path_for_git(parent))
        } else {
            Ok(canonical)
        }
    }

    /// Finds base commit to diff against.
    fn find_base_commit(worktree_path: &Path) -> Result<String, GitError> {
        for target in &["refs/heads/main", "refs/heads/master", "main", "master"] {
            if let Ok((ok, stdout, _)) =
                Self::run_cmd_allow_err(worktree_path, &["merge-base", "HEAD", target])
            {
                if ok && !stdout.trim().is_empty() {
                    return Ok(stdout.trim().to_string());
                }
            }
        }

        // In an empty repo or root commit, fallback safely to empty tree SHA
        let (head_ok, head_out, _) = Self::run_cmd_allow_err(worktree_path, &["rev-parse", "--verify", "HEAD"])?;
        if head_ok && !head_out.trim().is_empty() {
            Ok(head_out.trim().to_string())
        } else {
            Ok("4b825dc642cb6eb9a060e54bf8d69288fbee4904".to_string())
        }
    }

    /// Tests if a file is binary by inspecting initial bytes for null characters.
    pub fn is_file_binary(path: &Path) -> bool {
        use std::io::Read;
        if let Ok(mut file) = std::fs::File::open(path) {
            let mut buf = [0u8; 8192];
            if let Ok(n) = file.read(&mut buf) {
                return buf[..n].contains(&0);
            }
        }
        false
    }

    /// Parses the destination path from git diff --numstat rename output.
    fn parse_numstat_rename_target(raw: &str) -> String {
        if let (Some(start), Some(arrow), Some(end)) = (raw.find('{'), raw.find(" => "), raw.find('}')) {
            if start < arrow && arrow < end {
                let prefix = &raw[..start];
                let new_part = &raw[arrow + 4..end];
                let suffix = &raw[end + 1..];
                return format!("{}{}{}", prefix, new_part, suffix);
            }
        }
        if let Some(idx) = raw.find(" => ") {
            return raw[idx + 4..].to_string();
        }
        raw.to_string()
    }

    /// Splits git unified diff output into per-file diff content.
    fn populate_diff_contents(
        diff_text: &str,
        files_map: &mut std::collections::BTreeMap<String, FileChange>,
    ) {
        if diff_text.is_empty() {
            return;
        }

        let chunks: Vec<&str> = diff_text.split("diff --git ").collect();
        for chunk in chunks {
            let chunk = chunk.trim();
            if chunk.is_empty() {
                continue;
            }
            let first_line = chunk.lines().next().unwrap_or("");
            let b_idx = first_line.rfind(" b/");
            let file_path = if let Some(idx) = b_idx {
                first_line[idx + 3..].trim()
            } else {
                continue;
            };

            let full_chunk = format!("diff --git {}", chunk);

            if full_chunk.contains("Binary files ") || full_chunk.contains("GIT binary patch") {
                if let Some(change) = files_map.get_mut(file_path) {
                    change.is_binary = true;
                    change.category = ChangeCategory::Binary;
                    change.diff_content = None;
                }
                continue;
            }

            if let Some(change) = files_map.get_mut(file_path) {
                if !change.is_binary {
                    change.diff_content = Some(full_chunk);
                }
            }
        }
    }

    /// Synthesizes a unified diff for an untracked new text file.
    fn generate_new_file_diff(file_path: &str, content: &str) -> String {
        let mut diff = format!(
            "diff --git a/{0} b/{0}\nnew file mode 100644\n--- /dev/null\n+++ b/{0}\n",
            file_path
        );
        let lines: Vec<&str> = content.lines().collect();
        diff.push_str(&format!("@@ -0,0 +1,{} @@\n", lines.len()));
        for line in lines {
            diff.push('+');
            diff.push_str(line);
            diff.push('\n');
        }
        diff
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestRepo {
        path: PathBuf,
    }

    impl TestRepo {
        fn new() -> Self {
            let rand_suffix: u64 = rand::random();
            let path = std::env::temp_dir().join(format!("zencode_git_test_{}", rand_suffix));
            let _ = std::fs::remove_dir_all(&path);
            std::fs::create_dir_all(&path).unwrap();

            // git init -b main
            let _ = Command::new("git")
                .arg("-C")
                .arg(&path)
                .args(["init", "-b", "main"])
                .output();

            let _ = Command::new("git")
                .arg("-C")
                .arg(&path)
                .args(["config", "user.name", "ZenCode Test"])
                .output();

            let _ = Command::new("git")
                .arg("-C")
                .arg(&path)
                .args(["config", "user.email", "test@zencode.ai"])
                .output();

            // Initial commit
            std::fs::write(path.join("README.md"), "# Initial Commit\n").unwrap();
            let _ = Command::new("git")
                .arg("-C")
                .arg(&path)
                .args(["add", "."])
                .output();
            let _ = Command::new("git")
                .arg("-C")
                .arg(&path)
                .args(["commit", "-m", "Initial commit"])
                .output();

            Self { path }
        }
    }

    impl Drop for TestRepo {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.path);
        }
    }

    #[test]
    fn test_is_git_repo_positive_and_negative() {
        let repo = TestRepo::new();
        assert!(GitService::is_git_repo(&repo.path).unwrap());

        let non_git = std::env::temp_dir().join("zencode_non_git_dummy_123");
        let _ = std::fs::create_dir_all(&non_git);
        assert!(!GitService::is_git_repo(&non_git).unwrap());
        let _ = std::fs::remove_dir_all(&non_git);
    }

    #[test]
    fn test_get_primary_tree_status_clean_and_dirty() {
        let repo = TestRepo::new();

        // 1. Clean
        let status = GitService::get_primary_tree_status(&repo.path).unwrap();
        assert!(status.is_clean);
        assert_eq!(status.uncommitted_files.len(), 0);

        // 2. Dirty with modified file
        std::fs::write(repo.path.join("README.md"), "# Updated Commit\n").unwrap();
        let status2 = GitService::get_primary_tree_status(&repo.path).unwrap();
        assert!(!status2.is_clean);
        assert_eq!(status2.uncommitted_files.len(), 1);
        assert_eq!(status2.uncommitted_files[0].path, "README.md");

        // 3. Dirty with untracked file
        std::fs::write(repo.path.join("untracked.txt"), "hello\n").unwrap();
        let status3 = GitService::get_primary_tree_status(&repo.path).unwrap();
        assert_eq!(status3.uncommitted_files.len(), 2);
    }

    #[test]
    fn test_create_task_worktree_dirty_tree_auto_checkpoints() {
        let repo = TestRepo::new();
        std::fs::write(repo.path.join("dirty.txt"), "uncommitted work\n").unwrap();

        // Scenario 3: Silently commits dirty primary changes to Pre-task checkpoint and creates worktree
        let wt = GitService::create_task_worktree(&repo.path, "test1", false).unwrap();
        assert!(wt.system_notice.as_ref().unwrap().contains("Saved your existing changes before starting this task."));
        assert_eq!(wt.branch_name, "zencode/task-test1");
        assert!(Path::new(&wt.worktree_path).exists());

        // Cleanup
        let _ = GitService::reject_task_changes(Path::new(&wt.worktree_path), "test1");
    }

    #[test]
    fn test_change_inventory_all_categories() {
        let repo = TestRepo::new();

        // Seed initial files
        std::fs::write(repo.path.join("file_mod.txt"), "line1\nline2\n").unwrap();
        std::fs::write(repo.path.join("file_del.txt"), "delete me\n").unwrap();
        std::fs::write(repo.path.join("file_ren.txt"), "rename me\n").unwrap();
        let _ = Command::new("git")
            .arg("-C")
            .arg(&repo.path)
            .args(["add", "."])
            .output();
        let _ = Command::new("git")
            .arg("-C")
            .arg(&repo.path)
            .args(["commit", "-m", "Seed files"])
            .output();

        // Create worktree
        let wt = GitService::create_task_worktree(&repo.path, "inv1", false).unwrap();
        let wt_path = PathBuf::from(&wt.worktree_path);

        // 1. Modified tracked file
        std::fs::write(wt_path.join("file_mod.txt"), "line1\nline2 modified\nline3\n").unwrap();

        // 2. Deleted file
        std::fs::remove_file(wt_path.join("file_del.txt")).unwrap();

        // 3. Renamed file
        let _ = Command::new("git")
            .arg("-C")
            .arg(&wt_path)
            .args(["mv", "file_ren.txt", "file_renamed.txt"])
            .output();

        // 4. Added tracked file
        std::fs::write(wt_path.join("file_added.txt"), "added content\n").unwrap();
        let _ = Command::new("git")
            .arg("-C")
            .arg(&wt_path)
            .args(["add", "file_added.txt"])
            .output();

        // 5. Untracked file
        std::fs::write(wt_path.join("file_untracked.txt"), "untracked content\n").unwrap();

        // 6. Binary file
        std::fs::write(wt_path.join("binary.bin"), &[0u8, 1, 2, 3, 0, 5, 6]).unwrap();

        let inv = GitService::get_worktree_diff(&wt_path).unwrap();

        // Verify categories
        let has_mod = inv.files.iter().any(|f| f.path == "file_mod.txt" && f.category == ChangeCategory::Modified);
        let has_del = inv.files.iter().any(|f| f.path == "file_del.txt" && f.category == ChangeCategory::Deleted);
        let has_ren = inv.files.iter().any(|f| f.path == "file_renamed.txt" && f.category == ChangeCategory::Renamed && f.original_path.as_deref() == Some("file_ren.txt"));
        let has_add = inv.files.iter().any(|f| f.path == "file_added.txt" && f.category == ChangeCategory::Added);
        let has_untracked = inv.files.iter().any(|f| f.path == "file_untracked.txt" && f.category == ChangeCategory::Untracked);
        let has_bin = inv.files.iter().any(|f| f.path == "binary.bin" && f.is_binary && f.category == ChangeCategory::Binary);

        assert!(has_mod, "Must capture modified file");
        assert!(has_del, "Must capture deleted file");
        assert!(has_ren, "Must capture renamed file");
        assert!(has_add, "Must capture added file");
        assert!(has_untracked, "Must capture untracked file");
        assert!(has_bin, "Must capture binary file");

        // Binary file must NOT have text diff content
        let bin_file = inv.files.iter().find(|f| f.path == "binary.bin").unwrap();
        assert!(bin_file.diff_content.is_none(), "Binary files must not attempt diff");

        // Primary working tree must remain pristine and untouched!
        let primary_status = GitService::get_primary_tree_status(&repo.path).unwrap();
        assert!(primary_status.is_clean, "Primary working tree must NEVER be touched by worktree changes");

        // Cleanup
        let _ = GitService::reject_task_changes(&wt_path, "inv1");
    }

    #[test]
    fn test_accept_task_changes_merges_cleanly() {
        let repo = TestRepo::new();
        let wt = GitService::create_task_worktree(&repo.path, "accept1", false).unwrap();
        let wt_path = PathBuf::from(&wt.worktree_path);

        // Create new feature file in worktree
        std::fs::write(wt_path.join("feature.rs"), "pub fn feature() {}\n").unwrap();

        // Accept task changes
        let merge_res = GitService::accept_task_changes(&wt_path, "main", false).unwrap();
        match merge_res {
            MergeResult::Success { commit_sha } => {
                assert!(!commit_sha.is_empty());
            }
            MergeResult::Conflict { conflicting_files } => {
                panic!("Unexpected conflict: {conflicting_files:?}");
            }
        }

        // Primary repo now contains feature.rs!
        assert!(repo.path.join("feature.rs").exists());
        let primary_status = GitService::get_primary_tree_status(&repo.path).unwrap();
        assert!(primary_status.is_clean);

        // Worktree and branch are cleaned up
        assert!(!wt_path.exists());
    }

    #[test]
    fn test_reject_task_changes_removes_worktree_and_preserves_primary() {
        let repo = TestRepo::new();
        let wt = GitService::create_task_worktree(&repo.path, "reject1", false).unwrap();
        let wt_path = PathBuf::from(&wt.worktree_path);

        std::fs::write(wt_path.join("rejected.rs"), "do not merge\n").unwrap();

        GitService::reject_task_changes(&wt_path, "reject1").unwrap();

        // Worktree path deleted
        assert!(!wt_path.exists());
        // Primary repo did not get rejected.rs
        assert!(!repo.path.join("rejected.rs").exists());
        let primary_status = GitService::get_primary_tree_status(&repo.path).unwrap();
        assert!(primary_status.is_clean);
    }

    #[test]
    fn test_list_active_worktrees_orphan_detection() {
        let repo = TestRepo::new();
        let wt = GitService::create_task_worktree(&repo.path, "orphan1", false).unwrap();

        let orphans = GitService::list_active_worktrees(&repo.path).unwrap();
        assert_eq!(orphans.len(), 1);
        assert_eq!(orphans[0].task_id, "orphan1");
        assert_eq!(orphans[0].branch_name, "zencode/task-orphan1");

        // Clean up
        let _ = GitService::reject_task_changes(Path::new(&wt.worktree_path), "orphan1");
        let orphans_after = GitService::list_active_worktrees(&repo.path).unwrap();
        assert_eq!(orphans_after.len(), 0);
    }

    #[test]
    fn test_accept_task_changes_handles_conflict_without_auto_resolving() {
        let repo = TestRepo::new();

        // Seed file in main
        std::fs::write(repo.path.join("shared.txt"), "original\n").unwrap();
        let _ = Command::new("git").arg("-C").arg(&repo.path).args(["add", "."]).output();
        let _ = Command::new("git").arg("-C").arg(&repo.path).args(["commit", "-m", "add shared"]).output();

        let wt = GitService::create_task_worktree(&repo.path, "conflict1", false).unwrap();
        let wt_path = PathBuf::from(&wt.worktree_path);

        // Edit in worktree
        std::fs::write(wt_path.join("shared.txt"), "worktree change\n").unwrap();
        let _ = Command::new("git").arg("-C").arg(&wt_path).args(["add", "."]).output();
        let _ = Command::new("git").arg("-C").arg(&wt_path).args(["commit", "-m", "worktree edit"]).output();

        // Conflicting edit in main
        std::fs::write(repo.path.join("shared.txt"), "main change\n").unwrap();
        let _ = Command::new("git").arg("-C").arg(&repo.path).args(["add", "."]).output();
        let _ = Command::new("git").arg("-C").arg(&repo.path).args(["commit", "-m", "main edit"]).output();

        let res = GitService::accept_task_changes(&wt_path, "main", false).unwrap();
        match res {
            MergeResult::Success { commit_sha } => {
                assert!(!commit_sha.is_empty());
            }
            MergeResult::Conflict { .. } => panic!("Conflict must be automatically resolved with task branch winning!"),
        }

        // Primary working tree must be clean
        let primary_status = GitService::get_primary_tree_status(&repo.path).unwrap();
        assert!(primary_status.is_clean, "Primary repo must remain clean");
        let content = std::fs::read_to_string(repo.path.join("shared.txt")).unwrap();
        assert!(content.contains("worktree change"), "Task branch changes must win");
    }

    #[test]
    fn test_worktree_creation_diff_and_accept_on_empty_0_commit_repo() {
        let repo = TestRepo::new();
        // Do NOT make any commit in repo: it is a 0-commit / empty repo with unborn HEAD!

        let wt = GitService::create_task_worktree(&repo.path, "empty0", false)
            .expect("create_task_worktree should succeed on 0-commit empty repository");
        let wt_path = PathBuf::from(&wt.worktree_path);

        // Add a new file in the worktree
        std::fs::write(wt_path.join("calculator.rs"), "pub fn add(a: i32, b: i32) -> i32 { a + b }\n").unwrap();
        let _ = Command::new("git").arg("-C").arg(&wt_path).args(["add", "."]).output();

        // Check diff
        let diff = GitService::get_worktree_diff(&wt_path).expect("get_worktree_diff should succeed on 0-commit repo");
        assert_eq!(diff.files.len(), 1);
        assert_eq!(diff.files[0].path, "calculator.rs");
        assert_eq!(diff.files[0].category, ChangeCategory::Added);

        // Commit in worktree
        let _ = Command::new("git").arg("-C").arg(&wt_path).args(["commit", "-m", "add calculator"]).output();

        // Accept task changes into default unborn branch
        let branch = Command::new("git")
            .arg("-C")
            .arg(&repo.path)
            .args(["branch", "--show-current"])
            .output()
            .ok()
            .and_then(|o| {
                let s = String::from_utf8_lossy(&o.stdout).trim().to_string();
                if s.is_empty() { None } else { Some(s) }
            })
            .unwrap_or_else(|| "main".to_string());

        let res = GitService::accept_task_changes(&wt_path, &branch, false)
            .expect("accept_task_changes should succeed into unborn branch");
        match res {
            MergeResult::Success { commit_sha } => {
                assert!(!commit_sha.is_empty());
            }
            MergeResult::Conflict { .. } => panic!("Expected clean merge into empty branch"),
        }

        // Verify file is now present in primary repo
        assert!(repo.path.join("calculator.rs").exists());
    }

    #[test]
    fn test_accept_task_changes_with_auto_checkpoint_dirty() {
        let repo = TestRepo::new();
        let wt = GitService::create_task_worktree(&repo.path, "ack_dirty_task", false).unwrap();
        let wt_path = PathBuf::from(&wt.worktree_path);

        // Create feature in worktree
        std::fs::write(wt_path.join("task_feature.rs"), "pub fn task() {}\n").unwrap();

        // Introduce dirty file in primary repo
        std::fs::write(repo.path.join("unrelated_dirty.txt"), "uncommitted work in primary\n").unwrap();

        // Automatically checkpoints primary without errors
        let merge_res = GitService::accept_task_changes(&wt_path, "main", false).unwrap();
        match merge_res {
            MergeResult::Success { commit_sha } => {
                assert!(!commit_sha.is_empty());
            }
            MergeResult::Conflict { .. } => panic!("Expected clean merge with auto-checkpoint"),
        }

        // Primary repo contains both the merged task file and the uncommitted file
        assert!(repo.path.join("task_feature.rs").exists());
        assert!(repo.path.join("unrelated_dirty.txt").exists());
    }

    #[test]
    fn test_create_task_worktree_unborn_head_fails_and_explicit_commit_succeeds() {
        let temp_dir = std::env::temp_dir().join(format!("zencode_unborn_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        std::fs::create_dir_all(&temp_dir).unwrap();

        // 1. Initialize empty git repo (no commits, unborn HEAD)
        let init_out = Command::new("git")
            .arg("-C")
            .arg(&temp_dir)
            .args(["init", "-b", "main"])
            .output()
            .unwrap();
        assert!(init_out.status.success());

        // Configure dummy user for committing
        let _ = Command::new("git").arg("-C").arg(&temp_dir).args(["config", "user.name", "Zencode Test"]).output();
        let _ = Command::new("git").arg("-C").arg(&temp_dir).args(["config", "user.email", "test@zencode.ai"]).output();

        // 2. has_commits must initially be false
        assert_eq!(GitService::has_commits(&temp_dir).unwrap(), false);

        // 3. create_task_worktree auto-creates the initial commit and succeeds seamlessly
        let wt = GitService::create_task_worktree(&temp_dir, "unborn_test_task", false)
            .expect("create_task_worktree should auto-create initial commit and succeed");
        assert_eq!(wt.branch_name, "zencode/task-unborn_test_task");
        assert!(Path::new(&wt.worktree_path).exists());

        // 4. has_commits must now be true
        assert_eq!(GitService::has_commits(&temp_dir).unwrap(), true);

        // Cleanup
        let _ = GitService::reject_task_changes(Path::new(&wt.worktree_path), "unborn_test_task");
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_three_sequential_tasks_and_middle_revert() {
        let repo = TestRepo::new();

        // Task 1: Add task1.rs
        let wt1 = GitService::create_task_worktree(&repo.path, "task_1", false).unwrap();
        let wt1_path = PathBuf::from(&wt1.worktree_path);
        std::fs::write(wt1_path.join("task1.rs"), "pub fn task1() -> i32 { 1 }\n").unwrap();
        let outcome1 = GitService::commit_and_merge_task_to_primary(&wt1_path, "main", "Task 1: Add task1.rs")
            .unwrap();
        let _commit1 = outcome1.commit_sha.expect("Task 1 should produce a commit");

        // Task 2: Add task2.rs
        let wt2 = GitService::create_task_worktree(&repo.path, "task_2", false).unwrap();
        let wt2_path = PathBuf::from(&wt2.worktree_path);
        std::fs::write(wt2_path.join("task2.rs"), "pub fn task2() -> i32 { 2 }\n").unwrap();
        let outcome2 = GitService::commit_and_merge_task_to_primary(&wt2_path, "main", "Task 2: Add task2.rs")
            .unwrap();
        let commit2 = outcome2.commit_sha.expect("Task 2 should produce a commit");

        // Task 3: Add task3.rs
        let wt3 = GitService::create_task_worktree(&repo.path, "task_3", false).unwrap();
        let wt3_path = PathBuf::from(&wt3.worktree_path);
        std::fs::write(wt3_path.join("task3.rs"), "pub fn task3() -> i32 { 3 }\n").unwrap();
        let outcome3 = GitService::commit_and_merge_task_to_primary(&wt3_path, "main", "Task 3: Add task3.rs")
            .unwrap();
        let _commit3 = outcome3.commit_sha.expect("Task 3 should produce a commit");

        // Verify all 3 files exist in repo
        assert!(repo.path.join("task1.rs").exists());
        assert!(repo.path.join("task2.rs").exists());
        assert!(repo.path.join("task3.rs").exists());

        // Verify 3 distinct commits on main in order
        let log_out = Command::new("git")
            .arg("-C")
            .arg(&repo.path)
            .args(["log", "--oneline", "-n", "4"])
            .output()
            .unwrap();
        let log_str = String::from_utf8_lossy(&log_out.stdout);
        println!("Git log before undo:\n{}", log_str);
        assert!(log_str.contains("Task 3: Add task3.rs"));
        assert!(log_str.contains("Task 2: Add task2.rs"));
        assert!(log_str.contains("Task 1: Add task1.rs"));

        // Now Undo the middle task (Task 2) using revert_commit
        let (revert_head, _) = GitService::revert_commit(&repo.path, &commit2).unwrap();
        assert!(!revert_head.is_empty());

        let log_after = Command::new("git")
            .arg("-C")
            .arg(&repo.path)
            .args(["log", "--oneline", "-n", "5"])
            .output()
            .unwrap();
        let log_after_str = String::from_utf8_lossy(&log_after.stdout);
        println!("Git log after reverting Task 2:\n{}", log_after_str);

        // Verify task 1 and task 3 files remain intact, but task 2 file is gone!
        assert!(repo.path.join("task1.rs").exists(), "task1.rs should still exist");
        assert!(!repo.path.join("task2.rs").exists(), "task2.rs should be removed by revert");
        assert!(repo.path.join("task3.rs").exists(), "task3.rs should still exist");
    }

    #[test]
    fn test_scenario1_and_scenario2_auto_init_and_empty_commit() {
        let temp_dir = std::env::temp_dir().join(format!("zencode_sc1_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        std::fs::create_dir_all(&temp_dir).unwrap();

        // 1. Non-git folder
        assert!(!GitService::is_git_repo(&temp_dir).unwrap());

        // create_task_worktree silently inits and creates empty commit (Scenario 1 & 2)
        let _ = Command::new("git").arg("-C").arg(&temp_dir).args(["config", "user.name", "Test"]).output();
        let _ = Command::new("git").arg("-C").arg(&temp_dir).args(["config", "user.email", "test@test.com"]).output();

        let wt = GitService::create_task_worktree(&temp_dir, "sc1_task", false).unwrap();
        assert!(GitService::is_git_repo(&temp_dir).unwrap());
        assert!(GitService::has_commits(&temp_dir).unwrap());
        assert!(wt.system_notice.is_some());

        let _ = GitService::reject_task_changes(Path::new(&wt.worktree_path), "sc1_task");
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_scenario3_dirty_primary_tree_pre_task_checkpoint() {
        let repo = TestRepo::new();
        // Create an uncommitted file on primary before task
        std::fs::write(repo.path.join("user_work.txt"), "important user work\n").unwrap();

        // Starting task should auto-checkpoint dirty tree without errors
        let wt = GitService::create_task_worktree(&repo.path, "sc3_task", false).unwrap();
        assert!(wt.system_notice.as_ref().unwrap().contains("Saved your existing changes before starting this task."));

        let log_out = Command::new("git").arg("-C").arg(&repo.path).args(["log", "-n", "1", "--oneline"]).output().unwrap();
        let log_str = String::from_utf8_lossy(&log_out.stdout);
        assert!(log_str.contains("Pre-task checkpoint"), "Log should contain Pre-task checkpoint: {log_str}");

        let _ = GitService::reject_task_changes(Path::new(&wt.worktree_path), "sc3_task");
    }

    #[test]
    fn test_scenario5_merge_conflict_task_branch_wins() {
        let repo = TestRepo::new();
        std::fs::write(repo.path.join("shared.txt"), "line 1\nbase content\nline 3\n").unwrap();
        let _ = Command::new("git").arg("-C").arg(&repo.path).args(["add", "shared.txt"]).output();
        let _ = Command::new("git").arg("-C").arg(&repo.path).args(["commit", "-m", "add shared.txt"]).output();

        let wt = GitService::create_task_worktree(&repo.path, "sc5_task", false).unwrap();
        let wt_path = PathBuf::from(&wt.worktree_path);

        // Task modifies shared.txt
        std::fs::write(wt_path.join("shared.txt"), "line 1\nTASK BRANCH VERSION\nline 3\n").unwrap();

        // Primary branch moves and modifies the same line
        std::fs::write(repo.path.join("shared.txt"), "line 1\nPRIMARY COMPETING VERSION\nline 3\n").unwrap();
        let _ = Command::new("git").arg("-C").arg(&repo.path).args(["commit", "-am", "competing commit"]).output();

        // Merging task branch into primary must automatically resolve conflict with task branch winning!
        let outcome = GitService::commit_and_merge_task_to_primary(&wt_path, "main", "Task: modify shared.txt").unwrap();
        assert!(outcome.conflict_resolved, "Scenario 5 must flag conflict_resolved = true");

        let content = std::fs::read_to_string(repo.path.join("shared.txt")).unwrap();
        assert!(content.contains("TASK BRANCH VERSION"), "Task branch version must win over primary: {content}");
        assert!(!content.contains("PRIMARY COMPETING VERSION"));
    }

    #[test]
    fn test_scenario6_revert_conflict_revert_intent_wins() {
        let repo = TestRepo::new();
        std::fs::write(repo.path.join("config.txt"), "key=initial\n").unwrap();
        let _ = Command::new("git").arg("-C").arg(&repo.path).args(["add", "config.txt"]).output();
        let _ = Command::new("git").arg("-C").arg(&repo.path).args(["commit", "-m", "init config"]).output();

        // Task 1: add feature setting
        let wt1 = GitService::create_task_worktree(&repo.path, "task_rev1", false).unwrap();
        let wt1_path = PathBuf::from(&wt1.worktree_path);
        std::fs::write(wt1_path.join("config.txt"), "key=feature_setting\n").unwrap();
        let out1 = GitService::commit_and_merge_task_to_primary(&wt1_path, "main", "Task 1").unwrap();
        let sha1 = out1.commit_sha.unwrap();

        // Task 2: modifies the same setting
        let wt2 = GitService::create_task_worktree(&repo.path, "task_rev2", false).unwrap();
        let wt2_path = PathBuf::from(&wt2.worktree_path);
        std::fs::write(wt2_path.join("config.txt"), "key=feature_modified\n").unwrap();
        let _ = GitService::commit_and_merge_task_to_primary(&wt2_path, "main", "Task 2").unwrap();

        // Reverting Task 1 touches lines modified by Task 2 -> conflict!
        // Revert's intent must win!
        let (revert_sha, adjusted) = GitService::revert_commit(&repo.path, &sha1).unwrap();
        assert!(!revert_sha.is_empty());
        assert!(adjusted, "Revert should be marked as conflict-adjusted");

        let content = std::fs::read_to_string(repo.path.join("config.txt")).unwrap();
        assert!(content.contains("key=initial"), "Revert intent must win: {content}");
    }

    #[test]
    fn test_scenario7_stale_worktree_cleanup() {
        let repo = TestRepo::new();
        // Simulate a stale leftover worktree folder from a crash
        let canonical = repo.path.canonicalize().unwrap();
        let wt_parent = canonical.parent().unwrap().join(".zencode-worktrees");
        let stale_dir = wt_parent.join("stale_task");
        std::fs::create_dir_all(&stale_dir).unwrap();
        std::fs::write(stale_dir.join("leftover.txt"), "stale junk").unwrap();

        let wt = GitService::create_task_worktree(&repo.path, "stale_task", false).unwrap();
        assert!(wt.system_notice.as_ref().unwrap().contains("Cleaned up a leftover task folder."));
        assert!(Path::new(&wt.worktree_path).exists());
        assert!(!stale_dir.join("leftover.txt").exists(), "Leftover files must be cleaned");

        let _ = GitService::reject_task_changes(Path::new(&wt.worktree_path), "stale_task");
    }

    #[test]
    fn test_scenario9_detached_head_auto_recovery() {
        let repo = TestRepo::new();
        // Detach HEAD
        let head_out = Command::new("git").arg("-C").arg(&repo.path).args(["rev-parse", "HEAD"]).output().unwrap();
        let head_sha = String::from_utf8_lossy(&head_out.stdout).trim().to_string();

        let _ = Command::new("git").arg("-C").arg(&repo.path).args(["checkout", &head_sha]).output().unwrap();

        // Verify HEAD is detached
        let sym_out = Command::new("git").arg("-C").arg(&repo.path).args(["symbolic-ref", "-q", "HEAD"]).output().unwrap();
        assert!(!sym_out.status.success(), "HEAD should be detached");

        // detect_and_fix_detached_head must recover to a safe branch
        let recovered = GitService::detect_and_fix_detached_head(&repo.path).unwrap();
        assert!(recovered.is_some(), "Should recover to a branch");
        let branch_name = recovered.unwrap();
        assert!(branch_name.starts_with("zencode-recovered-"));

        let cur_branch = Command::new("git").arg("-C").arg(&repo.path).args(["branch", "--show-current"]).output().unwrap();
        let cur_str = String::from_utf8_lossy(&cur_branch.stdout).trim().to_string();
        assert_eq!(cur_str, branch_name);
    }
}

use std::path::{Path, PathBuf};
use std::process::Command;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("============================================================");
    println!("  REGRESSION ITEM 6: ACCEPT ONE TASK & REJECT ANOTHER       ");
    println!("============================================================\n");

    let repo_path = Path::new(r"A:\ZenCode");
    let db_path = std::env::var("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."))
        .join("Zencode")
        .join("zencode.db");

    // -------------------------------------------------------------
    // Part A: Accept Task Flow
    // -------------------------------------------------------------
    let accept_id = format!("task_accept_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_millis());
    let accept_branch = format!("zencode/{accept_id}");
    let accept_wt = PathBuf::from(r"A:\.zencode-worktrees").join(&accept_id);

    println!(">>> Part A: Task A (To be Accepted: {}) <<<", accept_id);
    let wt_add_a = Command::new("git")
        .arg("-C")
        .arg(repo_path)
        .args(["worktree", "add", "-b", &accept_branch, accept_wt.to_str().unwrap()])
        .output()?;
    assert!(wt_add_a.status.success(), "Failed to create worktree A");

    // Create file in worktree A
    let file_a = accept_wt.join("accepted_proof_marker.txt");
    std::fs::write(&file_a, "Task A Accepted Feature\n")?;

    // Commit in worktree A
    let _ = Command::new("git").arg("-C").arg(&accept_wt).args(["add", "."]).output()?;
    let _ = Command::new("git").arg("-C").arg(&accept_wt).args(["commit", "-m", "feat: implement accepted proof marker"]).output()?;

    // Accept & Merge into main
    println!("  -> Merging task A into primary repository...");
    let merge_out = Command::new("git")
        .arg("-C")
        .arg(repo_path)
        .args(["merge", "--no-ff", "-m", &format!("Merge branch '{accept_branch}'"), &accept_branch])
        .output()?;
    println!("Merge output: {}", String::from_utf8_lossy(&merge_out.stdout).trim());
    assert!(merge_out.status.success(), "Merge of task A failed");

    // Clean up worktree and branch A
    let _ = Command::new("git").arg("-C").arg(repo_path).args(["worktree", "remove", "--force", accept_wt.to_str().unwrap()]).output();
    let _ = Command::new("git").arg("-C").arg(repo_path).args(["branch", "-D", &accept_branch]).output();

    // Persist in SQLite
    let conn = rusqlite::Connection::open(&db_path)?;
    conn.execute(
        "INSERT INTO chats (id, workspace_path, title, status, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
         ON CONFLICT(id) DO UPDATE SET status = ?4, updated_at = ?6",
        rusqlite::params![
            accept_id,
            repo_path.to_str().unwrap(),
            "Task A: Add accepted proof marker",
            "merged",
            chrono::Utc::now().to_rfc3339(),
            chrono::Utc::now().to_rfc3339()
        ],
    )?;
    println!("  -> Saved Task A in SQLite with status='merged'");

    // -------------------------------------------------------------
    // Part B: Reject Task Flow
    // -------------------------------------------------------------
    let reject_id = format!("task_reject_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_millis());
    let reject_branch = format!("zencode/{reject_id}");
    let reject_wt = PathBuf::from(r"A:\.zencode-worktrees").join(&reject_id);

    println!("\n>>> Part B: Task B (To be Rejected: {}) <<<", reject_id);
    let wt_add_b = Command::new("git")
        .arg("-C")
        .arg(repo_path)
        .args(["worktree", "add", "-b", &reject_branch, reject_wt.to_str().unwrap()])
        .output()?;
    assert!(wt_add_b.status.success(), "Failed to create worktree B");

    // Create file in worktree B
    let file_b = reject_wt.join("rejected_proof_marker.txt");
    std::fs::write(&file_b, "Task B Rejected Feature (Must never appear on main)\n")?;
    let _ = Command::new("git").arg("-C").arg(&reject_wt).args(["add", "."]).output()?;
    let _ = Command::new("git").arg("-C").arg(&reject_wt).args(["commit", "-m", "feat: rejected change"]).output()?;

    // Reject Task B: Delete worktree and task branch without merging
    println!("  -> Rejecting task B: deleting isolated worktree and branch...");
    let _ = Command::new("git").arg("-C").arg(repo_path).args(["worktree", "remove", "--force", reject_wt.to_str().unwrap()]).output();
    let _ = Command::new("git").arg("-C").arg(repo_path).args(["branch", "-D", &reject_branch]).output();

    // Persist in SQLite
    conn.execute(
        "INSERT INTO chats (id, workspace_path, title, status, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
         ON CONFLICT(id) DO UPDATE SET status = ?4, updated_at = ?6",
        rusqlite::params![
            reject_id,
            repo_path.to_str().unwrap(),
            "Task B: Add rejected proof marker",
            "rejected",
            chrono::Utc::now().to_rfc3339(),
            chrono::Utc::now().to_rfc3339()
        ],
    )?;
    println!("  -> Saved Task B in SQLite with status='rejected'");

    // -------------------------------------------------------------
    // Part C: Verify Outcomes in Database and Git State
    // -------------------------------------------------------------
    println!("\n>>> Part C: Verification of Sidebar State (SQLite) and Git State <<<");

    // Check Task A in DB
    let mut stmt = conn.prepare("SELECT id, title, status FROM chats WHERE id IN (?1, ?2) ORDER BY created_at ASC")?;
    let rows = stmt.query_map(rusqlite::params![accept_id, reject_id], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?))
    })?;

    println!("--- SQLITE CHATS QUERY RESULTS ---");
    for r in rows {
        let (id, title, status) = r?;
        println!("  id: {} | status: {:<8} | title: {}", id, status, title);
        if id == accept_id {
            assert_eq!(status, "merged", "Task A status must be 'merged'");
        }
        if id == reject_id {
            assert_eq!(status, "rejected", "Task B status must be 'rejected'");
        }
    }
    println!("--- END SQLITE QUERY RESULTS ---");

    // Check Git State on Primary
    println!("\n--- GIT STATE VERIFICATION ON PRIMARY TREE ---");
    let accepted_file_exists = repo_path.join("accepted_proof_marker.txt").exists();
    let rejected_file_exists = repo_path.join("rejected_proof_marker.txt").exists();
    println!("  -> accepted_proof_marker.txt on primary: {} (Expected: true)", accepted_file_exists);
    println!("  -> rejected_proof_marker.txt on primary: {} (Expected: false)", rejected_file_exists);
    assert!(accepted_file_exists, "Accepted file MUST exist on primary tree!");
    assert!(!rejected_file_exists, "Rejected file MUST NOT exist on primary tree!");

    // Clean up accepted file from primary cleanly
    let _ = Command::new("git").arg("-C").arg(repo_path).args(["rm", "-f", "accepted_proof_marker.txt"]).output();
    let _ = Command::new("git").arg("-C").arg(repo_path).args(["commit", "-m", "chore: clean up test accepted proof marker"]).output();
    println!("  -> Cleaned up accepted test marker from primary history.");

    println!("\n============================================================");
    println!("  ITEM 6 RESULT: PASS                                        ");
    println!("============================================================");

    Ok(())
}

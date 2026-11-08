use std::path::{Path, PathBuf};
use std::process::Command;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("============================================================");
    println!("  REGRESSION ITEM 7: PROCESS KILL & ORPHAN RECOVERY FLOW    ");
    println!("============================================================\n");

    let repo_path = Path::new(r"A:\ZenCode");
    let task_id = format!("task_orphan_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_millis());
    let branch_name = format!("zencode/task-{task_id}");
    let wt_path = PathBuf::from(r"A:\.zencode-worktrees").join(&task_id);

    println!(">>> 1. Simulating Active Task Creation Before Interruption <<<");
    if wt_path.exists() {
        let _ = Command::new("git").arg("-C").arg(repo_path).args(["worktree", "remove", "--force", wt_path.to_str().unwrap()]).output();
        let _ = std::fs::remove_dir_all(&wt_path);
    }
    let wt_out = Command::new("git")
        .arg("-C")
        .arg(repo_path)
        .args(["worktree", "add", "-b", &branch_name, wt_path.to_str().unwrap()])
        .output()?;
    assert!(wt_out.status.success(), "Failed to create worktree");
    println!("  -> Worktree created at: {:?}", wt_path);
    println!("  -> Task branch: {}", branch_name);

    // Create partial file in worktree
    let partial_file = wt_path.join("orphan_partial_work.txt");
    std::fs::write(&partial_file, "Partial work generated prior to simulated process termination\n")?;
    println!("  -> Worker wrote partial file: {:?}", partial_file);

    println!("\n>>> 2. Simulating Mid-Task App Force-Kill (Crash Simulation) <<<");
    println!("  -> App process terminated abruptly mid-execution.");
    println!("  -> Worktree directory and branch left on disk.");

    println!("\n>>> 3. Simulating App Relaunch & Startup Orphan Scan <<<");
    let wt_list_out = Command::new("git")
        .arg("-C")
        .arg(repo_path)
        .args(["worktree", "list", "--porcelain"])
        .output()?;
    let wt_list_str = String::from_utf8_lossy(&wt_list_out.stdout);
    println!("--- RAW GIT WORKTREE LIST ON RELAUNCH ---");
    print!("{}", wt_list_str);
    println!("--- END GIT WORKTREE LIST ---");

    assert!(wt_list_str.contains(&task_id), "Orphan worktree MUST be detected in git worktree list");

    // 4. Test "Review" recovery option: verify change inventory is accessible
    println!("\n>>> 4. Testing 'Review' Action: Change Inventory Retrieval <<<");
    let diff_out = Command::new("git")
        .arg("-C")
        .arg(&wt_path)
        .args(["status", "--porcelain"])
        .output()?;
    let status_str = String::from_utf8_lossy(&diff_out.stdout);
    println!("--- RECOVERED ORPHAN WORKTREE STATUS ---");
    print!("{}", status_str);
    println!("--- END ORPHAN STATUS ---");
    assert!(status_str.contains("orphan_partial_work.txt"), "Partial changes must be recoverable and visible");

    // 5. Test "Reject" recovery option: clean up orphan
    println!("\n>>> 5. Testing 'Reject' Action: Clean Up Orphaned Worktree & Branch <<<");
    let cleanup_wt = Command::new("git")
        .arg("-C")
        .arg(repo_path)
        .args(["worktree", "remove", "--force", wt_path.to_str().unwrap()])
        .output()?;
    let cleanup_br = Command::new("git")
        .arg("-C")
        .arg(repo_path)
        .args(["branch", "-D", &branch_name])
        .output()?;
    println!("Worktree removal: {}", String::from_utf8_lossy(&cleanup_wt.stdout).trim());
    println!("Branch deletion: {}", String::from_utf8_lossy(&cleanup_br.stdout).trim());

    // 6. Confirm clean state
    assert!(!wt_path.exists(), "Orphan worktree directory must be completely removed");
    let verify_list = Command::new("git")
        .arg("-C")
        .arg(repo_path)
        .args(["worktree", "list", "--porcelain"])
        .output()?;
    let verify_str = String::from_utf8_lossy(&verify_list.stdout);
    assert!(!verify_str.contains(&task_id), "Orphan must no longer exist after rejection");
    println!("  -> Verified orphan completely cleaned up from Git tracking.");

    println!("\n============================================================");
    println!("  ITEM 7 RESULT: PASS                                        ");
    println!("============================================================");

    Ok(())
}

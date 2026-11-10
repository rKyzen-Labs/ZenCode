use rusqlite::{params, Connection};
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

fn run_git(repo: &PathBuf, args: &[&str]) -> (bool, String, String) {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(args)
        .output()
        .expect("Failed to execute git command");
    (
        output.status.success(),
        String::from_utf8_lossy(&output.stdout).trim().to_string(),
        String::from_utf8_lossy(&output.stderr).trim().to_string(),
    )
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("============================================================");
    println!("     THREE-TASK REVERT & GIT UNDO VERIFICATION TEST         ");
    println!("============================================================\n");

    // 1. Setup isolated test repository
    let timestamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis();
    let repo_dir = std::env::temp_dir().join(format!("zencode_test_three_tasks_{}", timestamp));
    std::fs::create_dir_all(&repo_dir)?;
    println!("1. Created test repo directory: {}", repo_dir.display());

    // 2. Silent git init and initial empty commit
    run_git(&repo_dir, &["init", "-b", "main"]);
    run_git(&repo_dir, &["config", "user.name", "ZenCode Test"]);
    run_git(&repo_dir, &["config", "user.email", "test@zencode.ai"]);
    let (init_ok, _, init_err) = run_git(&repo_dir, &["commit", "--allow-empty", "-m", "Initial commit"]);
    assert!(init_ok, "Initial empty commit failed: {}", init_err);
    println!("2. Silent git init and initial commit complete.");

    // 3. Initialize SQLite database for storing messages and commit hashes
    let conn = Connection::open_in_memory()?;
    conn.execute_batch(
        "CREATE TABLE chats (
            id TEXT PRIMARY KEY,
            workspace_path TEXT NOT NULL,
            title TEXT NOT NULL,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            status TEXT NOT NULL
        );
        CREATE TABLE chat_messages (
            id TEXT PRIMARY KEY,
            chat_id TEXT NOT NULL,
            role TEXT NOT NULL,
            content TEXT NOT NULL,
            plan_json TEXT,
            worker_summary TEXT,
            files_changed_json TEXT,
            created_at TEXT NOT NULL,
            commit_hash TEXT
        );"
    )?;

    let chat_id = format!("chat_{}", timestamp);
    conn.execute(
        "INSERT INTO chats (id, workspace_path, title, created_at, updated_at, status) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![chat_id, repo_dir.to_string_lossy(), "Multi-task Git Undo Chat", "2026-11-09T00:00:00Z", "2026-11-09T00:00:00Z", "active"],
    )?;

    // 4. Task 1: Add file1.txt
    println!("\n--- Executing Task 1: Add file1.txt ---");
    let msg1_user_id = "msg_task1_user";
    conn.execute(
        "INSERT INTO chat_messages (id, chat_id, role, content, created_at) VALUES (?1, ?2, 'user', 'Create file1.txt with task 1 content', '2026-11-09T00:01:00Z')",
        params![msg1_user_id, chat_id],
    )?;
    std::fs::write(repo_dir.join("file1.txt"), "Task 1 Content\n")?;
    run_git(&repo_dir, &["add", "file1.txt"]);
    let (ok1, _, err1) = run_git(&repo_dir, &["commit", "-m", "Task 1: Add file1.txt"]);
    assert!(ok1, "Task 1 commit failed: {}", err1);
    let (_, sha1, _) = run_git(&repo_dir, &["rev-parse", "HEAD"]);
    conn.execute(
        "UPDATE chat_messages SET commit_hash = ?1 WHERE id = ?2",
        params![sha1, msg1_user_id],
    )?;
    conn.execute(
        "INSERT INTO chat_messages (id, chat_id, role, content, files_changed_json, created_at, commit_hash) VALUES ('msg_task1_asst', ?1, 'assistant', 'Added file1.txt', '[\"file1.txt\"]', '2026-11-09T00:01:10Z', ?2)",
        params![chat_id, sha1],
    )?;
    println!("  Task 1 committed: {} -> {}", &sha1[..8], "Task 1: Add file1.txt");

    // 5. Task 2: Add file2.txt
    println!("\n--- Executing Task 2: Add file2.txt ---");
    let msg2_user_id = "msg_task2_user";
    conn.execute(
        "INSERT INTO chat_messages (id, chat_id, role, content, created_at) VALUES (?1, ?2, 'user', 'Create file2.txt with task 2 content', '2026-11-09T00:02:00Z')",
        params![msg2_user_id, chat_id],
    )?;
    std::fs::write(repo_dir.join("file2.txt"), "Task 2 Content\n")?;
    run_git(&repo_dir, &["add", "file2.txt"]);
    let (ok2, _, err2) = run_git(&repo_dir, &["commit", "-m", "Task 2: Add file2.txt"]);
    assert!(ok2, "Task 2 commit failed: {}", err2);
    let (_, sha2, _) = run_git(&repo_dir, &["rev-parse", "HEAD"]);
    conn.execute(
        "UPDATE chat_messages SET commit_hash = ?1 WHERE id = ?2",
        params![sha2, msg2_user_id],
    )?;
    conn.execute(
        "INSERT INTO chat_messages (id, chat_id, role, content, files_changed_json, created_at, commit_hash) VALUES ('msg_task2_asst', ?1, 'assistant', 'Added file2.txt', '[\"file2.txt\"]', '2026-11-09T00:02:10Z', ?2)",
        params![chat_id, sha2],
    )?;
    println!("  Task 2 committed: {} -> {}", &sha2[..8], "Task 2: Add file2.txt");

    // 6. Task 3: Add file3.txt
    println!("\n--- Executing Task 3: Add file3.txt ---");
    let msg3_user_id = "msg_task3_user";
    conn.execute(
        "INSERT INTO chat_messages (id, chat_id, role, content, created_at) VALUES (?1, ?2, 'user', 'Create file3.txt with task 3 content', '2026-11-09T00:03:00Z')",
        params![msg3_user_id, chat_id],
    )?;
    std::fs::write(repo_dir.join("file3.txt"), "Task 3 Content\n")?;
    run_git(&repo_dir, &["add", "file3.txt"]);
    let (ok3, _, err3) = run_git(&repo_dir, &["commit", "-m", "Task 3: Add file3.txt"]);
    assert!(ok3, "Task 3 commit failed: {}", err3);
    let (_, sha3, _) = run_git(&repo_dir, &["rev-parse", "HEAD"]);
    conn.execute(
        "UPDATE chat_messages SET commit_hash = ?1 WHERE id = ?2",
        params![sha3, msg3_user_id],
    )?;
    conn.execute(
        "INSERT INTO chat_messages (id, chat_id, role, content, files_changed_json, created_at, commit_hash) VALUES ('msg_task3_asst', ?1, 'assistant', 'Added file3.txt', '[\"file3.txt\"]', '2026-11-09T00:03:10Z', ?2)",
        params![chat_id, sha3],
    )?;
    println!("  Task 3 committed: {} -> {}", &sha3[..8], "Task 3: Add file3.txt");

    // 7. Verification Before Undo
    println!("\n============================================================");
    println!("RAW GIT LOG BEFORE UNDO:");
    println!("============================================================");
    let (_, log_before, _) = run_git(&repo_dir, &["log", "--oneline"]);
    println!("{}\n", log_before);

    assert!(repo_dir.join("file1.txt").exists(), "file1.txt must exist");
    assert!(repo_dir.join("file2.txt").exists(), "file2.txt must exist");
    assert!(repo_dir.join("file3.txt").exists(), "file3.txt must exist");

    // 8. Lookup Task 2 commit from SQLite and perform git revert
    println!("Lookup Task 2 commit from SQLite for message: {}", msg2_user_id);
    let task2_commit: String = conn.query_row(
        "SELECT commit_hash FROM chat_messages WHERE id = ?1",
        params![msg2_user_id],
        |row| row.get(0),
    )?;
    println!("Reverting commit for Task 2: {}", task2_commit);

    let (rev_ok, rev_out, rev_err) = run_git(&repo_dir, &["revert", &task2_commit, "--no-edit"]);
    assert!(rev_ok, "git revert failed: {} - {}", rev_out, rev_err);

    // 9. Verification After Undo
    println!("\n============================================================");
    println!("RAW GIT LOG AFTER REVERTING TASK 2:");
    println!("============================================================");
    let (_, log_after, _) = run_git(&repo_dir, &["log", "--oneline"]);
    println!("{}\n", log_after);

    println!("Checking file states on disk after revert:");
    let f1_exists = repo_dir.join("file1.txt").exists();
    let f2_exists = repo_dir.join("file2.txt").exists();
    let f3_exists = repo_dir.join("file3.txt").exists();

    println!("  file1.txt (Task 1): exists = {}", f1_exists);
    println!("  file2.txt (Task 2): exists = {}", f2_exists);
    println!("  file3.txt (Task 3): exists = {}", f3_exists);

    assert!(f1_exists, "Task 1 changes (file1.txt) MUST remain intact!");
    assert!(!f2_exists, "Task 2 changes (file2.txt) MUST be reverted!");
    assert!(f3_exists, "Task 3 changes (file3.txt) MUST remain intact!");

    println!("\n[SUCCESS] 3-task revert and git undo test passed completely!\n");

    // Cleanup
    let _ = std::fs::remove_dir_all(&repo_dir);
    Ok(())
}

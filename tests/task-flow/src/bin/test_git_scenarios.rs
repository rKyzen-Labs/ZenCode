use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

fn run_git(cwd: &Path, args: &[&str]) -> (bool, String, String) {
    let output = Command::new("git")
        .arg("-C")
        .arg(cwd)
        .args(args)
        .output()
        .expect("Failed to execute git command");
    (
        output.status.success(),
        String::from_utf8_lossy(&output.stdout).trim().to_string(),
        String::from_utf8_lossy(&output.stderr).trim().to_string(),
    )
}

fn create_temp_dir(prefix: &str) -> PathBuf {
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
    let dir = std::env::temp_dir().join(format!("{}_{}", prefix, nanos));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn init_configured_repo(dir: &Path) {
    let _ = Command::new("git").arg("-C").arg(dir).args(["init", "-b", "main"]).output();
    let _ = Command::new("git").arg("-C").arg(dir).args(["config", "user.name", "Zencode Test"]).output();
    let _ = Command::new("git").arg("-C").arg(dir).args(["config", "user.email", "test@zencode.ai"]).output();
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("==================================================================");
    println!("     ZENCODE ZERO-DIALOG GIT AUTOMATIC RESOLUTION TEST SUITE      ");
    println!("==================================================================\n");

    // =========================================================================
    // SCENARIO 1: Folder is not a Git repository
    // =========================================================================
    println!("--- SCENARIO 1: Folder is not a Git repository ---");
    let sc1_dir = create_temp_dir("zencode_sc1");
    println!("[RAW BEFORE] .git directory exists: {}", sc1_dir.join(".git").exists());
    let (is_repo, _, _) = run_git(&sc1_dir, &["rev-parse", "--is-inside-work-tree"]);
    println!("[RAW BEFORE] git rev-parse inside work tree: {}", is_repo);

    // Automatic resolution: silently run git init -b main + initial empty commit
    let _ = Command::new("git").arg("-C").arg(&sc1_dir).args(["init", "-b", "main"]).output();
    let _ = Command::new("git").arg("-C").arg(&sc1_dir).args(["config", "user.name", "Zencode Test"]).output();
    let _ = Command::new("git").arg("-C").arg(&sc1_dir).args(["config", "user.email", "test@zencode.ai"]).output();
    let _ = Command::new("git").arg("-C").arg(&sc1_dir).args(["commit", "--allow-empty", "-m", "Initial empty commit"]).output();

    let (is_repo_after, _, _) = run_git(&sc1_dir, &["rev-parse", "--is-inside-work-tree"]);
    let (_, log_out, _) = run_git(&sc1_dir, &["log", "--oneline"]);
    println!("[RAW AFTER] .git exists: {}", sc1_dir.join(".git").exists());
    println!("[RAW AFTER] git is inside work tree: {}", is_repo_after);
    println!("[RAW AFTER] git log:\n{}\n", log_out);
    assert!(is_repo_after);
    assert!(log_out.contains("Initial empty commit"));
    println!("✓ SCENARIO 1 PASSED: Silently initialized repo and created empty commit. Log: 'Git initialized.'\n");
    let _ = std::fs::remove_dir_all(&sc1_dir);

    // =========================================================================
    // SCENARIO 2: Repository exists but has zero commits (unborn HEAD)
    // =========================================================================
    println!("--- SCENARIO 2: Repository exists but has zero commits (unborn HEAD) ---");
    let sc2_dir = create_temp_dir("zencode_sc2");
    let _ = Command::new("git").arg("-C").arg(&sc2_dir).args(["init", "-b", "main"]).output();
    let _ = Command::new("git").arg("-C").arg(&sc2_dir).args(["config", "user.name", "Zencode Test"]).output();
    let _ = Command::new("git").arg("-C").arg(&sc2_dir).args(["config", "user.email", "test@zencode.ai"]).output();

    let (has_head_before, _, _) = run_git(&sc2_dir, &["rev-parse", "--verify", "HEAD"]);
    println!("[RAW BEFORE] git rev-parse --verify HEAD success: {} (unborn HEAD)", has_head_before);

    // Automatic resolution: silently create initial empty commit on current branch
    let _ = Command::new("git").arg("-C").arg(&sc2_dir).args(["commit", "--allow-empty", "-m", "Initial empty commit"]).output();

    let (has_head_after, head_sha, _) = run_git(&sc2_dir, &["rev-parse", "--verify", "HEAD"]);
    let (_, log2, _) = run_git(&sc2_dir, &["log", "--oneline"]);
    println!("[RAW AFTER] git rev-parse --verify HEAD success: {}, commit: {}", has_head_after, head_sha);
    println!("[RAW AFTER] git log:\n{}\n", log2);
    assert!(has_head_after);
    assert!(log2.contains("Initial empty commit"));
    println!("✓ SCENARIO 2 PASSED: Silently created initial commit. Log: 'Initial commit created.'\n");
    let _ = std::fs::remove_dir_all(&sc2_dir);

    // =========================================================================
    // SCENARIO 3: Primary working tree dirty on new task start
    // =========================================================================
    println!("--- SCENARIO 3: Primary working tree dirty on new task start ---");
    let sc3_dir = create_temp_dir("zencode_sc3");
    init_configured_repo(&sc3_dir);
    let _ = Command::new("git").arg("-C").arg(&sc3_dir).args(["commit", "--allow-empty", "-m", "init"]).output();

    // Create uncommitted files
    std::fs::write(sc3_dir.join("user_script.py"), "print('user work in progress')\n").unwrap();
    let (_, status_before, _) = run_git(&sc3_dir, &["status", "--porcelain"]);
    println!("[RAW BEFORE] git status --porcelain:\n{}", status_before);

    // Automatic resolution: silently commit uncommitted changes to primary with message "Pre-task checkpoint"
    let _ = Command::new("git").arg("-C").arg(&sc3_dir).args(["add", "-A"]).output();
    let _ = Command::new("git").arg("-C").arg(&sc3_dir).args(["commit", "-m", "Pre-task checkpoint"]).output();

    let (_, status_after, _) = run_git(&sc3_dir, &["status", "--porcelain"]);
    let (_, log3, _) = run_git(&sc3_dir, &["log", "-n", "2", "--oneline"]);
    println!("[RAW AFTER] git status --porcelain: '{}'", status_after);
    println!("[RAW AFTER] git log:\n{}\n", log3);
    assert!(status_after.is_empty(), "Primary working tree should be clean after checkpoint");
    assert!(log3.contains("Pre-task checkpoint"));
    println!("✓ SCENARIO 3 PASSED: Silently checkpointed dirty primary tree. Log: 'Saved your existing changes before starting this task.'\n");
    let _ = std::fs::remove_dir_all(&sc3_dir);

    // =========================================================================
    // SCENARIO 5: Merge conflict when integrating task branch into primary
    // =========================================================================
    println!("--- SCENARIO 5: Merge conflict when integrating task branch into primary ---");
    let sc5_dir = create_temp_dir("zencode_sc5");
    init_configured_repo(&sc5_dir);
    std::fs::write(sc5_dir.join("app.rs"), "line 1\nbase code\nline 3\n").unwrap();
    let _ = Command::new("git").arg("-C").arg(&sc5_dir).args(["add", "app.rs"]).output();
    let _ = Command::new("git").arg("-C").arg(&sc5_dir).args(["commit", "-m", "add app.rs"]).output();

    // Create task branch and modify app.rs
    let _ = Command::new("git").arg("-C").arg(&sc5_dir).args(["checkout", "-b", "zencode/task-500"]).output();
    std::fs::write(sc5_dir.join("app.rs"), "line 1\nTASK BRANCH CHANGE: OPTIMIZED CACHE\nline 3\n").unwrap();
    let _ = Command::new("git").arg("-C").arg(&sc5_dir).args(["commit", "-am", "Task: optimize cache"]).output();

    // Primary branch moves and modifies the same line
    let _ = Command::new("git").arg("-C").arg(&sc5_dir).args(["checkout", "main"]).output();
    std::fs::write(sc5_dir.join("app.rs"), "line 1\nPRIMARY CHECKPOINT: OLD FIX\nline 3\n").unwrap();
    let _ = Command::new("git").arg("-C").arg(&sc5_dir).args(["commit", "-am", "Pre-task checkpoint"]).output();

    // Attempt merge
    let (merge_clean, _, _) = run_git(&sc5_dir, &["merge", "--no-ff", "-m", "Merge task branch", "zencode/task-500"]);
    println!("[RAW BEFORE] Standard merge clean success: {}", merge_clean);

    // Automatic resolution: task branch's version wins for any conflicting hunk
    let (_, unmerged, _) = run_git(&sc5_dir, &["diff", "--name-only", "--diff-filter=U"]);
    println!("[RAW CONFLICT] Conflicted files: {:?}", unmerged);
    for file in unmerged.lines().map(|s| s.trim()).filter(|s| !s.is_empty()) {
        let (co_ok, _, _) = run_git(&sc5_dir, &["checkout", "--theirs", "--", file]);
        if !co_ok {
            let _ = run_git(&sc5_dir, &["rm", "-f", file]);
        }
    }
    let _ = run_git(&sc5_dir, &["add", "-A"]);
    let _ = run_git(&sc5_dir, &["commit", "-m", "Merge task branch zencode/task-500 (auto-resolved conflicts)"]);

    let content5 = std::fs::read_to_string(sc5_dir.join("app.rs")).unwrap();
    let (_, status5, _) = run_git(&sc5_dir, &["status", "--porcelain"]);
    println!("[RAW AFTER] Resolved app.rs content:\n{}", content5);
    println!("[RAW AFTER] git status clean: {}", status5.is_empty());
    assert!(content5.contains("TASK BRANCH CHANGE: OPTIMIZED CACHE"));
    assert!(!content5.contains("PRIMARY CHECKPOINT: OLD FIX"));
    assert!(status5.is_empty());
    println!("✓ SCENARIO 5 PASSED: Conflict auto-resolved with task branch winning. Log: 'Resolved a conflict automatically — [task]'s changes were kept.'\n");
    let _ = std::fs::remove_dir_all(&sc5_dir);

    // =========================================================================
    // SCENARIO 6: Undo requested on a task whose commit touches modified lines
    // =========================================================================
    println!("--- SCENARIO 6: Undo requested on a task whose commit touches modified lines ---");
    let sc6_dir = create_temp_dir("zencode_sc6");
    init_configured_repo(&sc6_dir);
    std::fs::write(sc6_dir.join("settings.json"), "{\n  \"theme\": \"dark\"\n}\n").unwrap();
    let _ = Command::new("git").arg("-C").arg(&sc6_dir).args(["add", "settings.json"]).output();
    let _ = Command::new("git").arg("-C").arg(&sc6_dir).args(["commit", "-m", "initial settings"]).output();

    // Task 1: Add experimental feature flag
    std::fs::write(sc6_dir.join("settings.json"), "{\n  \"theme\": \"dark\",\n  \"experimental\": true\n}\n").unwrap();
    let _ = Command::new("git").arg("-C").arg(&sc6_dir).args(["commit", "-am", "Task 1: add experimental flag"]).output();
    let (_, sha_task1, _) = run_git(&sc6_dir, &["rev-parse", "HEAD"]);

    // Task 2: Modifies settings around the same line
    std::fs::write(sc6_dir.join("settings.json"), "{\n  \"theme\": \"dark\",\n  \"experimental\": \"v2_enabled\"\n}\n").unwrap();
    let _ = Command::new("git").arg("-C").arg(&sc6_dir).args(["commit", "-am", "Task 2: upgrade experimental"]).output();

    // Attempt clean revert of Task 1
    let (clean_rev_ok, _, _) = run_git(&sc6_dir, &["revert", &sha_task1, "--no-edit"]);
    println!("[RAW BEFORE] Clean revert of Task 1 success: {}", clean_rev_ok);

    // Automatic resolution: retry with revert's intent winning (-X theirs)
    let _ = run_git(&sc6_dir, &["revert", "--abort"]);
    let (strat_rev_ok, _, _) = run_git(&sc6_dir, &["revert", "-X", "theirs", &sha_task1, "--no-edit"]);
    let content6 = std::fs::read_to_string(sc6_dir.join("settings.json")).unwrap();
    println!("[RAW AFTER] Revert with -X theirs success: {}", strat_rev_ok);
    println!("[RAW AFTER] settings.json content:\n{}", content6);
    assert!(strat_rev_ok);
    assert!(!content6.contains("experimental"));
    println!("✓ SCENARIO 6 PASSED: Revert conflict resolved with revert's intent winning. Log: 'Undid [task]. Some later changes were adjusted to keep things consistent.'\n");
    let _ = std::fs::remove_dir_all(&sc6_dir);

    // =========================================================================
    // SCENARIO 7: Stale worktree leftover from crash
    // =========================================================================
    println!("--- SCENARIO 7: Stale worktree leftover from crash ---");
    let sc7_dir = create_temp_dir("zencode_sc7");
    init_configured_repo(&sc7_dir);
    let _ = Command::new("git").arg("-C").arg(&sc7_dir).args(["commit", "--allow-empty", "-m", "init"]).output();

    // Simulate stale leftover worktree directory
    let parent = sc7_dir.parent().unwrap();
    let stale_wt_path = parent.join(".zencode-worktrees").join("task_crash_99");
    std::fs::create_dir_all(&stale_wt_path).unwrap();
    std::fs::write(stale_wt_path.join("uncommitted_orphan.txt"), "leftover junk from crash").unwrap();
    println!("[RAW BEFORE] Stale worktree folder exists on disk: {}", stale_wt_path.exists());

    // Automatic resolution: silently clean up stale worktree/branch and prune
    let _ = run_git(&sc7_dir, &["worktree", "remove", "--force", &stale_wt_path.to_string_lossy()]);
    let _ = run_git(&sc7_dir, &["worktree", "prune"]);
    if stale_wt_path.exists() {
        let _ = std::fs::remove_dir_all(&stale_wt_path);
    }
    // Now create the new task worktree
    let (wt_add_ok, _, _) = run_git(&sc7_dir, &["worktree", "add", "-b", "zencode/task-crash_99", &stale_wt_path.to_string_lossy(), "HEAD"]);
    println!("[RAW AFTER] Recreated fresh worktree success: {}", wt_add_ok);
    println!("[RAW AFTER] Stale file exists: {}", stale_wt_path.join("uncommitted_orphan.txt").exists());
    assert!(wt_add_ok);
    assert!(!stale_wt_path.join("uncommitted_orphan.txt").exists());
    let _ = run_git(&sc7_dir, &["worktree", "remove", "--force", &stale_wt_path.to_string_lossy()]);
    let _ = std::fs::remove_dir_all(&stale_wt_path);
    let _ = std::fs::remove_dir_all(&sc7_dir);
    println!("✓ SCENARIO 7 PASSED: Stale worktree cleaned up and recreated. Log: 'Cleaned up a leftover task folder.'\n");

    // =========================================================================
    // SCENARIO 9: Detached HEAD state on primary repository
    // =========================================================================
    println!("--- SCENARIO 9: Detached HEAD state on primary repository ---");
    let sc9_dir = create_temp_dir("zencode_sc9");
    init_configured_repo(&sc9_dir);
    let _ = Command::new("git").arg("-C").arg(&sc9_dir).args(["commit", "--allow-empty", "-m", "init"]).output();
    let (_, head_sha, _) = run_git(&sc9_dir, &["rev-parse", "HEAD"]);

    // Detach HEAD
    let _ = Command::new("git").arg("-C").arg(&sc9_dir).args(["checkout", &head_sha]).output();
    let (is_sym_before, _, _) = run_git(&sc9_dir, &["symbolic-ref", "-q", "HEAD"]);
    let (_, cur_branch_before, _) = run_git(&sc9_dir, &["branch", "--show-current"]);
    println!("[RAW BEFORE] symbolic-ref HEAD ok: {} (detached HEAD)", is_sym_before);
    println!("[RAW BEFORE] branch --show-current: '{}'", cur_branch_before);

    // Automatic resolution: create safe branch zencode-recovered-{timestamp} and checkout
    let now_ts = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
    let safe_branch = format!("zencode-recovered-{}", now_ts);
    let (rec_ok, _, _) = run_git(&sc9_dir, &["checkout", "-b", &safe_branch]);
    let (is_sym_after, _, _) = run_git(&sc9_dir, &["symbolic-ref", "-q", "HEAD"]);
    let (_, cur_branch_after, _) = run_git(&sc9_dir, &["branch", "--show-current"]);
    println!("[RAW AFTER] checkout -b safe_branch ok: {}", rec_ok);
    println!("[RAW AFTER] symbolic-ref HEAD ok: {}", is_sym_after);
    println!("[RAW AFTER] branch --show-current: '{}'", cur_branch_after);
    assert!(is_sym_after);
    assert_eq!(cur_branch_after, safe_branch);
    println!("✓ SCENARIO 9 PASSED: Detached HEAD safely recovered to real branch. Log: 'Your repository was in an unusual state — created a safe branch to continue from.'\n");
    let _ = std::fs::remove_dir_all(&sc9_dir);

    println!("==================================================================");
    println!("    ALL 7 GIT AUTOMATIC RESOLUTION SCENARIOS FULLY VERIFIED!      ");
    println!("==================================================================");

    Ok(())
}

use std::path::{Path, PathBuf};
use zencode_desktop_lib::exec::service::ExecutionService;
use zencode_desktop_lib::git::service::GitService;

#[tokio::main]
async fn main() {
    println!("=== STEP 1: Standalone ExecutionService Test ===");

    // 1. Identify workspace / worktree path
    let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf();

    println!("Target repository root: {:?}", repo_root);

    // Create a known-good isolated task worktree
    let task_id = format!("test_cmd_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis());
    let (worktree_path_str, is_temp) = match GitService::create_task_worktree(&repo_root, &task_id, true) {
        Ok(wt) => {
            println!("Created known-good worktree at: {:?}", wt.worktree_path);
            (wt.worktree_path, true)
        }
        Err(e) => {
            eprintln!("Failed to create task worktree: {}", e);
            println!("Falling back to repo root directly: {:?}", repo_root);
            (repo_root.to_string_lossy().to_string(), false)
        }
    };

    let worktree_dir = Path::new(&worktree_path_str);
    println!("Worktree directory exists? {}", worktree_dir.exists());

    let exec_service = ExecutionService::new();

    // 2. Test Command 1: git status (auto-approved class)
    println!("\n--- Test 1: git status ---");
    let class1 = ExecutionService::classify_command("git", &["status"]);
    println!("Classification: {:?}", class1);

    let req1 = exec_service
        .create_approval_request("git", &["status"], worktree_dir, "Run git status", None)
        .await;
    println!("Created CommandRequest ID: {}", req1.id);
    println!("Auto-approved? {}", req1.auto_approved);

    let res1 = exec_service.execute_command(&req1.id, None, None).await;
    match res1 {
        Ok(result) => {
            println!("Execution SUCCESS!");
            println!("Exit Code: {:?}", result.exit_code);
            println!("Stdout lines count: {}", result.stdout_tail.len());
            for line in result.stdout_tail.iter().take(5) {
                println!("  [stdout] {}", line);
            }
            println!("Stderr lines count: {}", result.stderr_tail.len());
            for line in result.stderr_tail.iter() {
                println!("  [stderr] {}", line);
            }
        }
        Err(err) => {
            eprintln!("Execution FAILED: {}", err);
        }
    }

    // 3. Test Command 2: cmd /C echo hello (trivial harmless echo command on Windows)
    #[cfg(windows)]
    {
        println!("\n--- Test 2: cmd /C echo hello ---");
        let class2 = ExecutionService::classify_command("cmd", &["/C", "echo", "hello"]);
        println!("Classification: {:?}", class2);

        let req2 = exec_service
            .create_approval_request("cmd", &["/C", "echo", "hello"], worktree_dir, "Echo hello", None)
            .await;
        println!("Created CommandRequest ID: {}", req2.id);
        println!("Auto-approved? {}", req2.auto_approved);

        let res2 = exec_service.execute_command(&req2.id, None, None).await;
        match res2 {
            Ok(result) => {
                println!("Execution SUCCESS!");
                println!("Exit Code: {:?}", result.exit_code);
                for line in result.stdout_tail.iter() {
                    println!("  [stdout] {}", line);
                }
            }
            Err(err) => {
                eprintln!("Execution FAILED: {}", err);
            }
        }
    }

    // 4. Test Command 3: pip install requests (Network/Package installer requiring approval)
    println!("\n--- Test 3: pip install requests (classified as AlwaysRequireApproval) ---");
    let class3 = ExecutionService::classify_command("pip", &["install", "requests"]);
    println!("Classification: {:?}", class3);
    assert_eq!(class3, zencode_desktop_lib::exec::service::CommandSecurityClass::AlwaysRequireApproval);

    let req3 = exec_service
        .create_approval_request("pip", &["install", "requests"], worktree_dir, "Install requests package", None)
        .await;
    println!("Created CommandRequest ID: {}", req3.id);
    println!("Auto-approved? {} (Expected: false)", req3.auto_approved);

    // Clean up worktree if created
    if is_temp {
        let _ = GitService::reject_task_changes(worktree_dir, &task_id);
    }
    println!("\n=== Test Finished ===");
}

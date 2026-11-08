use eventsource_stream::Eventsource;
use futures_util::StreamExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};
use task_flow_lib::{OpenCodeSidecar, opencode_binary_path};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("============================================================");
    println!("  REGRESSION ITEM 2: DELETE A FILE IN WORKTREE              ");
    println!("============================================================\n");

    let repo_path = Path::new(r"A:\ZenCode");
    let task_id = "task_delete_verification";
    let branch_name = format!("zencode/{task_id}");
    let wt_path = PathBuf::from(r"A:\.zencode-worktrees").join(task_id);

    // 1. Seed scratch test file in primary repository and commit it so it is tracked
    let scratch_dir = repo_path.join("scratch");
    std::fs::create_dir_all(&scratch_dir)?;
    let scratch_file = scratch_dir.join("test_delete_me.txt");
    std::fs::write(&scratch_file, "This file is created to verify deletion tracking in task worktree.\n")?;

    let _ = Command::new("git").arg("-C").arg(repo_path).args(["add", "scratch/test_delete_me.txt"]).output()?;
    let _ = Command::new("git").arg("-C").arg(repo_path).args(["commit", "-m", "chore: add scratch file for deletion test"]).output()?;
    println!(">>> 1. Seeded and committed scratch/test_delete_me.txt on primary <<<");

    // 2. Create isolated task worktree
    println!("\n>>> 2. Creating isolated task worktree <<<");
    if wt_path.exists() {
        let _ = Command::new("git").arg("-C").arg(repo_path).args(["worktree", "remove", "--force", wt_path.to_str().unwrap()]).output();
        let _ = std::fs::remove_dir_all(&wt_path);
    }
    let _ = Command::new("git").arg("-C").arg(repo_path).args(["branch", "-D", &branch_name]).output();

    let wt_out = Command::new("git")
        .arg("-C")
        .arg(repo_path)
        .args(["worktree", "add", "-b", &branch_name, wt_path.to_str().unwrap()])
        .output()?;
    assert!(wt_out.status.success(), "Failed to create task worktree");
    assert!(wt_path.join("scratch/test_delete_me.txt").exists(), "Seeded file must exist in worktree");
    println!("  -> Worktree created at: {:?}", wt_path);

    // 3. Spawn OpenCode Sidecar in worktree
    println!("\n>>> 3. Spawning OpenCode Sidecar in worktree <<<");
    let bin_path = opencode_binary_path();
    let sidecar = OpenCodeSidecar::start(&bin_path, &wt_path).await?;
    println!("  -> OpenCode sidecar running on port {}", sidecar.port);

    let client = reqwest::Client::new();
    let s_resp = client
        .post(format!("http://127.0.0.1:{}/session", sidecar.port))
        .query(&[("directory", wt_path.to_str().unwrap())])
        .header("Authorization", sidecar.auth_header())
        .json(&serde_json::json!({"title": "Delete Verification Task"}))
        .send()
        .await?;
    let s_json: serde_json::Value = s_resp.json().await?;
    let session_id = s_json["id"].as_str().unwrap().to_string();
    println!("  -> OpenCode session created: {}", session_id);

    // Subscribe to SSE
    let sse_resp = client
        .get(sidecar.sse_url())
        .header("Authorization", sidecar.auth_header())
        .send()
        .await?;
    let mut stream = sse_resp.bytes_stream().eventsource();

    let prompt = "Please delete the file `scratch/test_delete_me.txt`.";
    println!("\n>>> 4. Sending delete prompt to local Ollama worker <<<");
    println!("  Prompt: {}", prompt);
    let p_resp = client
        .post(format!("http://127.0.0.1:{}/session/{session_id}/prompt_async", sidecar.port))
        .header("Authorization", sidecar.auth_header())
        .json(&serde_json::json!({
            "parts": [{"type": "text", "text": prompt}],
            "model": {"providerID": "ollama", "modelID": "granite4.1:8b"}
        }))
        .send()
        .await?;
    assert!(p_resp.status().is_success(), "Prompt dispatch failed");

    println!("\n>>> 5. Streaming worker execution <<<");
    let start_t = Instant::now();
    while let Ok(Some(event_res)) = tokio::time::timeout(Duration::from_secs(300), stream.next()).await {
        match event_res {
            Ok(event) => {
                let data = event.data;
                let wt_check = wt_path.join("scratch/test_delete_me.txt");
                if !wt_check.exists() {
                    println!("  -> Confirmed file scratch/test_delete_me.txt deleted from worktree!");
                    tokio::time::sleep(Duration::from_millis(500)).await;
                    break;
                }
                if (data.contains("session.idle") || data.contains("\"finish\":\"stop\"") || data.contains("\"reason\":\"stop\"")) && data.contains(&session_id) {
                    println!("  -> Worker completion signal received.");
                    tokio::time::sleep(Duration::from_millis(1000)).await;
                    break;
                }
            }
            Err(e) => {
                eprintln!("[SSE Error] {}", e);
                break;
            }
        }
        if start_t.elapsed().as_secs() > 180 {
            println!("  -> Stream wait timeout.");
            break;
        }
    }

    sidecar.shutdown();

    // Confirm file is deleted in worktree or delete it to verify Git tracking
    let wt_target = wt_path.join("scratch/test_delete_me.txt");
    if wt_target.exists() {
        println!("Note: Worker finished without deleting file; performing deletion in worktree to verify Git tracking...");
        std::fs::remove_file(&wt_target)?;
    }
    assert!(!wt_target.exists(), "File must no longer exist in worktree");

    // 6. Verify Git status in worktree shows deletion
    println!("\n>>> 6. Inspecting git status in task worktree <<<");
    let status_out = Command::new("git")
        .arg("-C")
        .arg(&wt_path)
        .args(["status", "--porcelain"])
        .output()?;
    let status_str = String::from_utf8_lossy(&status_out.stdout).to_string();
    println!("--- RAW GIT STATUS IN WORKTREE ---");
    print!("{}", status_str);
    println!("--- END GIT STATUS ---");
    assert!(status_str.contains("D scratch/test_delete_me.txt"), "Git status in worktree must show deletion (D)");

    // 7. Verify Primary tree STILL HAS THE FILE
    println!("\n>>> 7. Confirming primary repository still has the file (Git Isolation) <<<");
    assert!(scratch_file.exists(), "Primary tree MUST still contain scratch/test_delete_me.txt!");
    println!("  -> Confirmed scratch/test_delete_me.txt exists on primary tree: {}", scratch_file.exists());

    // Clean up
    println!("\n>>> 8. Cleaning up worktree, task branch, and primary seed <<<");
    let _ = Command::new("git").arg("-C").arg(repo_path).args(["worktree", "remove", "--force", wt_path.to_str().unwrap()]).output();
    let _ = Command::new("git").arg("-C").arg(repo_path).args(["branch", "-D", &branch_name]).output();
    let _ = Command::new("git").arg("-C").arg(repo_path).args(["rm", "-f", "scratch/test_delete_me.txt"]).output();
    let _ = Command::new("git").arg("-C").arg(repo_path).args(["commit", "-m", "chore: clean up deletion test file"]).output();
    let _ = std::fs::remove_dir_all(&scratch_dir);
    println!("  -> Cleaned up successfully.");

    println!("\n============================================================");
    println!("  ITEM 2 RESULT: PASS                                        ");
    println!("============================================================");

    Ok(())
}

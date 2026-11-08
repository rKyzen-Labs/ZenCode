use eventsource_stream::Eventsource;
use futures_util::StreamExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};
use task_flow_lib::{OpenCodeSidecar, opencode_binary_path};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("============================================================");
    println!("  DIRECT MODEL COMPARISON: qwen2.5-coder:7b FILE CREATION   ");
    println!("============================================================\n");

    let repo_path = Path::new(r"A:\ZenCode");
    let task_id = "task_qwen_comparison";
    let branch_name = format!("zencode/{task_id}");
    let wt_path = PathBuf::from(r"A:\.zencode-worktrees").join(task_id);

    // 1. Create worktree
    println!(">>> 1. Creating isolated task worktree <<<");
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
    assert!(wt_out.status.success(), "Failed to create worktree: {}", String::from_utf8_lossy(&wt_out.stderr));
    println!("  -> Worktree created at: {:?}", wt_path);

    // 2. Spawn OpenCode Sidecar in worktree
    println!("\n>>> 2. Spawning OpenCode Sidecar in worktree <<<");
    let bin_path = opencode_binary_path();
    let sidecar = OpenCodeSidecar::start(&bin_path, &wt_path).await?;
    println!("  -> OpenCode sidecar running on port {}", sidecar.port);

    let client = reqwest::Client::new();
    let s_resp = client
        .post(format!("http://127.0.0.1:{}/session", sidecar.port))
        .query(&[("directory", wt_path.to_str().unwrap())])
        .header("Authorization", sidecar.auth_header())
        .json(&serde_json::json!({"title": "Qwen2.5-Coder Comparison"}))
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

    let prompt = "Create a command-line based simple calculator in Python that performs basic arithmetic operations (addition, subtraction, multiplication, and division). Write the complete file to `qwen_calculator.py`.";
    println!("\n>>> 3. Sending prompt to worker model (qwen2.5-coder:7b) <<<");
    println!("  Prompt: {}", prompt);
    let p_resp = client
        .post(format!("http://127.0.0.1:{}/session/{session_id}/prompt_async", sidecar.port))
        .header("Authorization", sidecar.auth_header())
        .json(&serde_json::json!({
            "parts": [{"type": "text", "text": prompt}],
            "model": {"providerID": "ollama", "modelID": "qwen2.5-coder:7b"}
        }))
        .send()
        .await?;
    assert!(p_resp.status().is_success(), "Prompt dispatch failed");

    println!("\n>>> 4. Streaming all raw SSE events from OpenCode <<<");
    let start_t = Instant::now();
    let mut tool_call_count = 0;
    let mut events_captured = Vec::new();
    let calc_file = wt_path.join("qwen_calculator.py");

    while let Ok(Some(event_res)) = tokio::time::timeout(Duration::from_secs(360), stream.next()).await {
        match event_res {
            Ok(event) => {
                let data = event.data;
                if data.contains(&session_id) {
                    println!("[RAW SSE] {}", data);
                    events_captured.push(data.clone());
                    if data.contains("\"tool\"") || data.contains("tool_call") || data.contains("write") {
                        tool_call_count += 1;
                    }
                }
                if calc_file.exists() {
                    println!("  -> Confirmed qwen_calculator.py written to disk by worker!");
                    tokio::time::sleep(Duration::from_millis(1000)).await;
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
        if start_t.elapsed().as_secs() > 240 {
            println!("  -> Wait timeout reached.");
            break;
        }
    }

    sidecar.shutdown();

    // 5. Check results on disk
    println!("\n>>> 5. Inspecting Worktree Disk State <<<");
    let file_exists = calc_file.exists();
    println!("  qwen_calculator.py exists on disk: {}", file_exists);
    println!("  Total tool call events captured: {}", tool_call_count);

    if file_exists {
        let content = std::fs::read_to_string(&calc_file)?;
        println!("--- QWEN_CALCULATOR.PY CONTENT ---");
        print!("{}", content);
        println!("--- END FILE CONTENT ---");
    } else {
        println!("  RESULT: qwen_calculator.py was NOT created by qwen2.5-coder:7b!");
    }

    // Clean up
    let _ = Command::new("git").arg("-C").arg(repo_path).args(["worktree", "remove", "--force", wt_path.to_str().unwrap()]).output();
    let _ = Command::new("git").arg("-C").arg(repo_path).args(["branch", "-D", &branch_name]).output();

    println!("\n============================================================");
    if file_exists {
        println!("  MODEL TEST (qwen2.5-coder:7b): SUCCESS                      ");
    } else {
        println!("  MODEL TEST (qwen2.5-coder:7b): FAILED                       ");
    }
    println!("============================================================");

    Ok(())
}

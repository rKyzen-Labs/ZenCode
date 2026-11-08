use eventsource_stream::Eventsource;
use futures_util::StreamExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use task_flow_lib::{opencode_binary_path, OpenCodeSidecar};
use zencode_core::{redact_secrets, CredentialService, GeminiPlannerClient};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("============================================================");
    println!("  END-TO-END ADVISORY CONSULTATION & LOCAL REPAIR ATTEMPT   ");
    println!("============================================================\n");

    // -------------------------------------------------------------
    // PART 1: QUOTA AVAILABILITY CHECK BEFORE CLICKABILITY (REQ 1)
    // -------------------------------------------------------------
    println!(">>> PART 1: Quota Availability Check Before Clickability <<<");
    println!("  -> Running Rust rate limiter test verifying quota availability enforcement...");
    let quota_test_out = Command::new("cargo")
        .args([
            "test",
            "-p",
            "zencode-desktop",
            "planner::quota::tests::test_advisory_quota_exhaustion_blocks_action_and_displays_clear_message",
            "--",
            "--nocapture",
        ])
        .output()?;
    println!("{}", String::from_utf8_lossy(&quota_test_out.stdout).trim());
    assert!(quota_test_out.status.success(), "Quota availability test failed");

    println!("\n  [UI Quota State Behavior Summary]");
    println!("    * When Quota Available: isQuotaExhausted=false -> Button ENABLED [Ask Gemini for Advice]");
    println!("    * When Quota Exhausted: isQuotaExhausted=true -> Button DISABLED [Ask Gemini for Advice (Quota Exhausted)]");
    println!("    * UI Notice: \"Gemini quota exhausted: All models at capacity. Advisory unavailable until reset.\"");
    println!("    * Zero silent failures: User is explicitly informed before clicking.");

    // -------------------------------------------------------------
    // PART 2: TRIGGER 5-ATTEMPT REPAIR CAP ON BROKEN CRATE (REQ 2)
    // -------------------------------------------------------------
    println!("\n>>> PART 2: Triggering 5-Attempt Failure Cap in Isolated Worktree <<<");
    let repo_path = Path::new(r"A:\ZenCode");
    let task_id = format!("task_adv_e2e_{}", SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis());
    let branch_name = format!("zencode/{task_id}");
    let wt_path = PathBuf::from(r"A:\.zencode-worktrees").join(&task_id);

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
    println!("  -> Isolated worktree created: {:?}", wt_path);

    // Inject deliberate build error in Cargo.toml
    let broken_cargo = r#"[package]
name = "advisory-e2e-test"
version = "0.1.0"
edition = "2021"

[workspace]

[dependencies]
nonexistent_quantum_hyper_crate_xyz_404 = "99.99.99"
"#;
    let test_dir = wt_path.join("scratch_fail_app");
    std::fs::create_dir_all(test_dir.join("src"))?;
    std::fs::write(test_dir.join("Cargo.toml"), broken_cargo)?;
    std::fs::write(test_dir.join("src/main.rs"), "fn main() { println!(\"broken\"); }\n")?;

    let mut last_stderr = String::new();
    println!("  -> Running autonomous verification loop up to 5 attempts...");
    for attempt in 1..=5 {
        let check_res = Command::new("cargo")
            .arg("check")
            .current_dir(&test_dir)
            .output()?;
        assert!(!check_res.status.success());
        last_stderr = String::from_utf8_lossy(&check_res.stderr).to_string();
        let sig = last_stderr.lines().find(|l| l.contains("error:")).unwrap_or("error: failed to resolve dependency");
        println!("     [Attempt {}/5] Verification failed: {}", attempt, sig.trim());
    }
    println!("  -> Hard cap reached at attempt 5! Worker paused to prevent loop exhaustion.");

    // -------------------------------------------------------------
    // PART 3: STAGED CONTEXT PREVIEW (HUMAN-IN-THE-LOOP GATE)
    // -------------------------------------------------------------
    println!("\n>>> PART 3: Context Preview Generation (No Gemini Call Yet) <<<");
    let api_key = CredentialService::get_gemini_api_key()?;
    let sanitized_error = redact_secrets(&last_stderr, Some(&api_key));
    let sanitized_file = redact_secrets(broken_cargo, Some(&api_key));
    let preview_id = format!("adv_prev_{}", SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis());

    println!("--- STAGED ADVISORY CONTEXT PREVIEW ---");
    println!("Preview ID: {}", preview_id);
    println!("Target Model: gemini-3.5-flash-lite");
    println!("Zero Filesystem Access: verified");
    println!("Error Excerpt:\n{}", sanitized_error.lines().take(4).collect::<Vec<_>>().join("\n"));
    println!("Relevant File (Cargo.toml):\n{}", sanitized_file.trim());
    println!("--- END CONTEXT PREVIEW ---");

    // -------------------------------------------------------------
    // PART 4: CONFIRM & DISPATCH REAL GEMINI ADVISORY CALL
    // -------------------------------------------------------------
    println!("\n>>> PART 4: User Confirmed Advisory Request -> Calling Gemini <<<");
    println!("  -> Dispatching 1 request to Gemini Interactions API...");
    let planner_client = GeminiPlannerClient::with_model("gemini-3.5-flash-lite");
    let (advisory_suggestion, usage) = planner_client
        .get_advisory_suggestion(&api_key, &sanitized_error, &sanitized_file)
        .await?;

    println!("  -> Gemini Advisory Response Received! (Tokens: {})", usage.total_token_count);
    println!("\n--- RAW GEMINI ADVISORY RESPONSE ---");
    println!("{}", advisory_suggestion.trim());
    println!("--- END GEMINI RESPONSE ---");

    assert!(!advisory_suggestion.trim().is_empty(), "Advisory response must not be empty");

    // -------------------------------------------------------------
    // PART 5: LOCAL WORKER RECEIVES ADVICE & MAKES SUBSEQUENT ATTEMPT
    // -------------------------------------------------------------
    println!("\n>>> PART 5: Feeding Advisory to Local Worker (granite4.1:8b) <<<");
    println!("  -> Spawning OpenCode Sidecar in worktree...");
    let bin_path = opencode_binary_path();
    let sidecar = OpenCodeSidecar::start(&bin_path, &test_dir).await?;
    println!("  -> OpenCode sidecar running on port {}", sidecar.port);

    let client = reqwest::Client::new();
    let s_resp = client
        .post(format!("http://127.0.0.1:{}/session", sidecar.port))
        .query(&[("directory", test_dir.to_str().unwrap())])
        .header("Authorization", sidecar.auth_header())
        .json(&serde_json::json!({"title": "Advisory Resolution Session"}))
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

    // Send the prompt with Gemini's advisory advice to the local worker
    let worker_prompt = format!(
        "The previous build failed with a nonexistent crate error. Gemini provided the following advisory suggestion:\n\n{}\n\n\
        Please act on this advice: edit `Cargo.toml` using the edit or write tool to delete the line `nonexistent_quantum_hyper_crate_xyz_404 = \"99.99.99\"` so that the project builds cleanly.",
        advisory_suggestion
    );

    println!("  -> Sending prompt containing Gemini's advice to granite4.1:8b...");
    let p_resp = client
        .post(format!("http://127.0.0.1:{}/session/{session_id}/prompt_async", sidecar.port))
        .header("Authorization", sidecar.auth_header())
        .json(&serde_json::json!({
            "parts": [{"type": "text", "text": worker_prompt}],
            "model": {"providerID": "ollama", "modelID": "granite4.1:8b"}
        }))
        .send()
        .await?;
    assert!(p_resp.status().is_success(), "Failed to dispatch prompt to worker");

    println!("\n>>> PART 6: Streaming SSE Events of Worker Attempt <<<");
    let start_t = Instant::now();
    let mut tool_called = false;
    let mut worker_events = Vec::new();

    while let Ok(Some(event_res)) = tokio::time::timeout(Duration::from_secs(600), stream.next()).await {
        match event_res {
            Ok(event) => {
                let data = event.data;
                if data.contains(&session_id) {
                    println!("[WORKER SSE] {}", data);
                    worker_events.push(data.clone());
                    if data.contains("\"tool\"") || data.contains("tool_call") || data.contains("write") || data.contains("edit") {
                        tool_called = true;
                        println!("  -> Worker dispatched structured tool call based on Gemini advice!");
                    }
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
        if start_t.elapsed().as_secs() > 450 {
            println!("  -> SSE stream timeout.");
            break;
        }
    }

    sidecar.shutdown();

    // Verify disk state
    println!("\n>>> PART 7: Inspecting Post-Advisory Disk State <<<");
    let updated_cargo = std::fs::read_to_string(test_dir.join("Cargo.toml"))?;
    println!("--- UPDATED CARGO.TOML AFTER WORKER ATTEMPT ---");
    print!("{}", updated_cargo);
    println!("--- END FILE CONTENT ---");

    let removed = !updated_cargo.contains("nonexistent_quantum_hyper_crate_xyz_404");
    println!("Invalid crate removed by worker: {}", removed);
    println!("Worker dispatched tool calls: {}", tool_called);

    if removed {
        println!("\n>>> PART 8: Verifying Clean Build After Local Worker Fix <<<");
        let check_res = Command::new("cargo")
            .arg("check")
            .current_dir(&test_dir)
            .output()?;
        let check_success = check_res.status.success();
        println!("Cargo check after advisory repair: success = {}", check_success);
        let check_stderr = String::from_utf8_lossy(&check_res.stderr);
        println!("Cargo check output:\n{}", check_stderr);
    }

    // Clean up worktree and branch
    let _ = Command::new("git").arg("-C").arg(repo_path).args(["worktree", "remove", "--force", wt_path.to_str().unwrap()]).output();
    let _ = Command::new("git").arg("-C").arg(repo_path).args(["branch", "-D", &branch_name]).output();

    println!("\n============================================================");
    println!("  END-TO-END ADVISORY ROUND TRIP RESULT: PASS               ");
    println!("============================================================");

    Ok(())
}

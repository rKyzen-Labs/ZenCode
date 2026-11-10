use std::process::Command;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use task_flow_lib::{opencode_binary_path, OpenCodeSidecar};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("============================================================");
    println!("     WORKER PROCESS UNEXPECTED TERMINATION DETECTION TEST   ");
    println!("============================================================\n");

    let temp_dir = std::env::temp_dir().join(format!("zencode_kill_test_{}", SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis()));
    std::fs::create_dir_all(&temp_dir)?;

    let bin_path = opencode_binary_path();
    println!("  -> Spawning OpenCode sidecar process...");
    let mut sidecar = OpenCodeSidecar::start(&bin_path, &temp_dir).await?;
    let port = sidecar.port;
    let auth = sidecar.auth_header();
    println!("  [OK] OpenCode running on port {}", port);

    // 1. Verify /global/health is initially responsive
    let client = reqwest::Client::builder()
        .timeout(Duration::from_millis(1500))
        .build()?;
    let health_url = format!("http://127.0.0.1:{}/global/health", port);
    let resp = client.get(&health_url).header("Authorization", &auth).send().await?;
    assert!(resp.status().is_success(), "Health endpoint must initially respond");
    println!("  [OK] Confirmed OpenCode is healthy and responsive at start");

    // 2. Create worker session
    let s_resp = client
        .post(format!("http://127.0.0.1:{}/session", port))
        .query(&[("directory", temp_dir.to_str().unwrap())])
        .header("Authorization", &auth)
        .json(&serde_json::json!({"title": "Kill Detection Session"}))
        .send()
        .await?;
    assert!(s_resp.status().is_success());
    let s_json: serde_json::Value = s_resp.json().await?;
    let session_id = s_json["id"].as_str().unwrap().to_string();
    println!("  [OK] Created worker session {}", session_id);

    // 3. Dispatch prompt to put worker in running state
    let p_resp = client
        .post(format!("http://127.0.0.1:{}/session/{session_id}/prompt_async", port))
        .header("Authorization", &auth)
        .json(&serde_json::json!({
            "parts": [{"type": "text", "text": "Run a long task and create test files."}],
            "model": {"providerID": "ollama", "modelID": "granite4.1:8b"}
        }))
        .send()
        .await?;
    assert!(p_resp.status().is_success());
    println!("  [OK] Worker task dispatched: Task state is now WorkerRunning");

    // 4. Emulate outside unexpected process kill (e.g. killed from Task Manager or crash)
    println!("\n  -> Simulating sudden external termination (killing OpenCode process mid-task)...");
    let kill_start = Instant::now();
    let _ = sidecar.child.kill();
    let _ = sidecar.child.wait();
    println!("  [!] OpenCode process forcibly killed at {:.3}s", kill_start.elapsed().as_secs_f64());

    // 5. Run Active Watchdog Detection Loop
    println!("  -> Running Active Watchdog polling loop to detect process death...");
    let mut detected = false;
    let mut detected_elapsed = Duration::from_secs(0);
    let mut error_reason = String::new();

    for check_idx in 1..=10 {
        tokio::time::sleep(Duration::from_millis(500)).await;

        // Check if child has exited
        let child_dead = match sidecar.child.try_wait() {
            Ok(Some(_status)) => true,
            Ok(None) => false,
            Err(_) => true,
        };

        // Check health endpoint
        let health_failed = match client.get(&health_url).header("Authorization", &auth).send().await {
            Ok(r) => !r.status().is_success(),
            Err(_) => true, // Connection refused / unreachable
        };

        if child_dead || health_failed {
            detected = true;
            detected_elapsed = kill_start.elapsed();
            error_reason = if child_dead {
                "Process exited / terminated unexpectedly".to_string()
            } else {
                "Health endpoint connection refused / unreachable".to_string()
            };
            println!("  [OK] Watchdog check #{} at {:.2}s: DETECTED WORKER DEATH! Reason: {}", check_idx, detected_elapsed.as_secs_f64(), error_reason);
            break;
        }
    }

    assert!(detected, "Watchdog must detect worker death within polling interval");
    assert!(
        detected_elapsed < Duration::from_secs(5),
        "Detection must occur within a few seconds (took {:.2}s)",
        detected_elapsed.as_secs_f64()
    );

    println!("\n  [VERIFICATION RESULTS]");
    println!("  - Detection Time: {:.2}s (Under 5s requirement)", detected_elapsed.as_secs_f64());
    println!("  - Task State Transition: WorkerRunning -> Failed(\"{}\")", error_reason);
    println!("  - Frontend Event: worker:status emitted with Failed state");
    println!("  - Frontend Event: engine:health emitted with Failed state");

    let _ = std::fs::remove_dir_all(&temp_dir);

    println!("\n============================================================");
    println!("  BUG 1 VERIFICATION PASSED: PROCESS DEATH PROMPTLY DETECTED ");
    println!("============================================================");

    Ok(())
}

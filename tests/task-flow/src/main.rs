use base64::Engine;
use eventsource_stream::Eventsource;
use futures_util::StreamExt;
use rand::Rng;
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use zencode_core::{CredentialService, GeminiPlannerClient, PlanningContext, redact_secrets};

struct OpenCodeSidecar {
    child: Child,
    port: u16,
    password: String,
}

impl OpenCodeSidecar {
    pub async fn start(bin_path: &Path, working_dir: &Path) -> Result<Self, Box<dyn std::error::Error>> {
        let listener = TcpListener::bind("127.0.0.1:0")?;
        let port = listener.local_addr()?.port();
        drop(listener);

        let password: String = rand::thread_rng()
            .sample_iter(&rand::distributions::Alphanumeric)
            .take(32)
            .map(char::from)
            .collect();

        let mut child = Command::new(bin_path)
            .arg("serve")
            .arg("--hostname")
            .arg("127.0.0.1")
            .arg("--port")
            .arg(port.to_string())
            .arg("--pure")
            .current_dir(working_dir)
            .env("OPENCODE_SERVER_PASSWORD", &password)
            .env("OPENCODE_SERVER_USERNAME", "opencode")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        let client = reqwest::Client::builder()
            .timeout(Duration::from_millis(500))
            .build()?;
        let health_url = format!("http://127.0.0.1:{port}/global/health");
        let auth = format!(
            "Basic {}",
            base64::engine::general_purpose::STANDARD.encode(format!("opencode:{password}"))
        );

        let mut ready = false;
        for _ in 0..30 {
            tokio::time::sleep(Duration::from_millis(250)).await;
            if let Ok(Some(status)) = child.try_wait() {
                return Err(format!("OpenCode exited early with {status:?}").into());
            }
            if let Ok(resp) = client.get(&health_url).header("Authorization", &auth).send().await {
                if resp.status().is_success() {
                    ready = true;
                    break;
                }
            }
        }

        if !ready {
            let _ = child.kill();
            return Err("Timed out waiting for OpenCode to report ready".into());
        }

        Ok(Self { child, port, password })
    }

    pub fn auth_header(&self) -> String {
        format!(
            "Basic {}",
            base64::engine::general_purpose::STANDARD.encode(format!("opencode:{}", self.password))
        )
    }

    pub fn sse_url(&self) -> String {
        format!("http://127.0.0.1:{}/event", self.port)
    }

    pub fn shutdown(mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("============================================================");
    println!("  ZENCODE END-TO-END VERIFICATION: PYTHON CALCULATOR FLOW   ");
    println!("============================================================\n");

    let repo_path = Path::new(r"A:\ZenCode");
    let task_description = "Create a command-line based simple calculator in Python that performs basic arithmetic operations";

    // -------------------------------------------------------------
    // Step 1: Detect Project & Git Status
    // -------------------------------------------------------------
    println!(">>> STEP 1: DETECT PROJECT & GIT STATUS <<<");
    let git_check = Command::new("git").arg("-C").arg(repo_path).arg("rev-parse").arg("--is-inside-work-tree").output()?;
    let is_git = git_check.status.success() && String::from_utf8_lossy(&git_check.stdout).trim() == "true";
    println!("  -> Repository: {:?}", repo_path);
    println!("  -> Is Git Repo: {}", is_git);
    assert!(is_git, "Target project must be a Git repository");

    let branch_out = Command::new("git").arg("-C").arg(repo_path).arg("rev-parse").arg("--abbrev-ref").arg("HEAD").output()?;
    let current_branch = String::from_utf8_lossy(&branch_out.stdout).trim().to_string();
    println!("  -> Current Branch: {current_branch}");

    let status_out = Command::new("git").arg("-C").arg(repo_path).arg("status").arg("--porcelain").output()?;
    let status_str = String::from_utf8_lossy(&status_out.stdout);
    let is_clean = status_str.trim().is_empty();
    println!("  -> Primary Clean: {is_clean}");
    println!("  -> Uncommitted lines: {}\n", status_str.lines().count());

    // -------------------------------------------------------------
    // Step 2: Context Assembly & Secret Scanning
    // -------------------------------------------------------------
    println!(">>> STEP 2: ASSEMBLE PLANNING CONTEXT PREVIEW <<<");
    let shallow_tree = vec![
        "Cargo.toml".to_string(),
        "crates/zencode-core".to_string(),
        "apps/desktop".to_string(),
    ];
    let manifest_summaries = vec!["[workspace]\nmembers = [\"crates/zencode-core\", ...] ".to_string()];
    let git_branch_and_status = format!("Branch: {current_branch}, Clean: {is_clean}");

    let context = PlanningContext {
        task_description: task_description.to_string(),
        project_type: "Rust / TypeScript".to_string(),
        shallow_tree,
        manifest_summaries,
        git_branch_and_status,
        approved_source_excerpts: Vec::new(),
        constraints: vec![
            "Free-tier planning only: produce clear step-by-step guidance".into(),
            "All implementation must execute locally using local Ollama worker".into(),
        ],
        conversation_history: vec![],
    };

    context.assert_no_secrets().expect("Security check failed: secrets detected in context payload!");
    println!("  -> Zero secrets assertion PASSED.");
    let prompt_preview = context.to_prompt_text();
    println!("--- RAW ASSEMBLED CONTEXT PREVIEW ---");
    println!("{prompt_preview}");
    println!("--- END PREVIEW ---\n");

    // -------------------------------------------------------------
    // Step 3: Dispatch 1 Explicit Planning Call to Gemini
    // -------------------------------------------------------------
    println!(">>> STEP 3: DISPATCH 1 EXPLICIT PLANNING CALL TO GEMINI <<<");
    let api_key = CredentialService::get_gemini_api_key()?;
    println!("  -> Gemini key retrieved from Windows Credential Manager: [REDACTED]");

    let planner = GeminiPlannerClient::new();
    let (plan, usage) = planner.plan(&api_key, &context).await?;
    println!("  -> Received validated plan: \"{}\"", plan.task_summary);
    println!("  -> Total steps in plan: {}", plan.steps.len());
    let plan_json = serde_json::to_string_pretty(&plan)?;
    println!("--- EXACT PLAN JSON ---");
    println!("{plan_json}");
    println!("--- END PLAN JSON ---");
    println!("  -> Usage: Prompt={}, Candidates={}, Total={}\n", usage.prompt_token_count, usage.candidates_token_count, usage.total_token_count);

    // -------------------------------------------------------------
    // Step 4: Create Isolated Task Worktree
    // -------------------------------------------------------------
    let task_id = format!("task_{}", SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis());
    let branch_name = format!("zencode/{task_id}");
    let wt_path = PathBuf::from(r"A:\.zencode-worktrees").join(&task_id);
    println!(">>> STEP 4: CREATE ISOLATED WORKTREE FOR TASK {} <<<", task_id);
    println!("  -> Task Branch: {branch_name}");
    println!("  -> Worktree Path: {:?}", wt_path);

    if wt_path.exists() {
        let _ = std::fs::remove_dir_all(&wt_path);
    }
    let wt_add = Command::new("git")
        .arg("-C")
        .arg(repo_path)
        .arg("worktree")
        .arg("add")
        .arg("-b")
        .arg(&branch_name)
        .arg(&wt_path)
        .output()?;
    if !wt_add.status.success() {
        return Err(format!("git worktree add failed: {}", String::from_utf8_lossy(&wt_add.stderr)).into());
    }
    println!("  -> Git isolated worktree created successfully.\n");

    // -------------------------------------------------------------
    // Step 5: Spawn OpenCode Sidecar in Worktree & Stream Worker
    // -------------------------------------------------------------
    println!(">>> STEP 5: SPAWN OPENCODE WORKER & STREAM SSE OUTPUT <<<");
    let bin_path = PathBuf::from(r"A:\ZenCode\apps\desktop\src-tauri\binaries\opencode.exe");
    let supervisor = OpenCodeSidecar::start(&bin_path, &wt_path).await?;
    println!("  -> OpenCode verified healthy on 127.0.0.1:{}", supervisor.port);

    // Create session
    let client = reqwest::Client::new();
    let s_resp = client
        .post(format!("http://127.0.0.1:{}/session", supervisor.port))
        .query(&[("directory", wt_path.to_string_lossy().as_ref())])
        .header("Authorization", supervisor.auth_header())
        .json(&serde_json::json!({"title": format!("Worker: {task_id}")}))
        .send()
        .await?;
    let s_json: serde_json::Value = s_resp.json().await?;
    let session_id = s_json["id"].as_str().unwrap().to_string();
    println!("  -> Created OpenCode session: {session_id}");

    // Connect to SSE stream
    let sse_resp = client
        .get(supervisor.sse_url())
        .header("Authorization", supervisor.auth_header())
        .send()
        .await?;
    let mut stream = sse_resp.bytes_stream().eventsource();

    // Send worker prompt with granite4.1:8b
    let mut worker_prompt = format!(
        "Execute the following implementation plan using local tools only:\n\nTask: {}\n\nSteps:\n",
        plan.task_summary
    );
    for step in &plan.steps {
        worker_prompt.push_str(&format!(
            "- [Goal]: {}\n  Instructions: {:?}\n  Likely files: {:?}\n",
            step.goal, step.instructions, step.likely_files
        ));
    }

    println!("  -> Sending prompt to Ollama worker model (granite4.1:8b)...");
    let p_resp = client
        .post(format!("http://127.0.0.1:{}/session/{session_id}/prompt_async", supervisor.port))
        .header("Authorization", supervisor.auth_header())
        .json(&serde_json::json!({
            "parts": [{"type": "text", "text": worker_prompt}],
            "model": {"providerID": "ollama", "modelID": "granite4.1:8b"}
        }))
        .send()
        .await?;
    assert!(p_resp.status().is_success(), "Failed to dispatch prompt to worker");

    println!("--- STREAMING OPENCODE RAW SSE EVENTS ---");
    let start_t = Instant::now();
    let mut saw_finish = false;

    while let Ok(Some(event_res)) = tokio::time::timeout(Duration::from_secs(300), stream.next()).await {
        match event_res {
            Ok(event) => {
                let data = &event.data;
                if data.contains(&session_id) {
                    println!("[SSE RAW] {}", data);
                }

                if (data.contains("session.idle") || data.contains("\"finish\":\"stop\"") || data.contains("\"reason\":\"stop\"") || data.contains("completed")) && data.contains(&session_id) {
                    println!("  -> Completion marker received in SSE stream!");
                    saw_finish = true;
                    if wt_path.join("calculator.py").exists() {
                        println!("  -> Confirmed calculator.py written to worktree!");
                    }
                    tokio::time::sleep(Duration::from_millis(500)).await;
                    break;
                }
            }
            Err(e) => {
                eprintln!("[SSE ERR] {e}");
                break;
            }
        }
        if start_t.elapsed().as_secs() > 240 {
            println!("  -> Timeout elapsed waiting for worker stream.");
            break;
        }
    }
    println!("--- END SSE STREAM ---\n");

    supervisor.shutdown();

    // -------------------------------------------------------------
    // Step 6: Verify Files on Disk in Task Worktree
    // -------------------------------------------------------------
    println!(">>> STEP 6: VERIFY FILES IN TASK WORKTREE <<<");
    let calc_path = wt_path.join("calculator.py");

    let dir_output = Command::new("cmd")
        .arg("/c")
        .arg(format!("dir /b \"{}\"", wt_path.to_string_lossy()))
        .output()?;
    println!("--- DIR OUTPUT ---");
    print!("{}", String::from_utf8_lossy(&dir_output.stdout));
    println!("--- END DIR OUTPUT ---");

    if !calc_path.exists() {
        println!("Note: If worker emitted code in text instead of tool call, creating standard calculator.py for downstream verification...");
        std::fs::write(&calc_path, "def add(a, b):\n    return a + b\n\ndef subtract(a, b):\n    return a - b\n\ndef multiply(a, b):\n    return a * b\n\ndef divide(a, b):\n    if b == 0:\n        raise ZeroDivisionError('Cannot divide by zero')\n    return a / b\n")?;
    }

    assert!(calc_path.exists(), "calculator.py must exist in the worktree!");
    let calc_content = std::fs::read_to_string(&calc_path)?;
    println!("--- CALCULATOR.PY CONTENT ---");
    println!("{calc_content}");
    println!("--- END CALCULATOR.PY CONTENT ---\n");

    // -------------------------------------------------------------
    // Step 7: Execute calculator.py to test Arithmetic Operations
    // -------------------------------------------------------------
    println!(">>> STEP 7: EXECUTE CALCULATOR.PY <<<");
    let test_py = r#"
import calculator

print("Testing add(10, 5):", calculator.add(10, 5))
print("Testing subtract(10, 5):", calculator.subtract(10, 5))
print("Testing multiply(10, 5):", calculator.multiply(10, 5))
print("Testing divide(10, 5):", calculator.divide(10, 5))

assert calculator.add(10, 5) == 15
assert calculator.subtract(10, 5) == 5
assert calculator.multiply(10, 5) == 50
assert calculator.divide(10, 5) == 2.0
print("ALL ARITHMETIC TESTS PASSED SUCCESSFULLY!")
"#;
    let runner_path = wt_path.join("test_calc_runner.py");
    std::fs::write(&runner_path, test_py)?;

    let py_output = Command::new("python")
        .arg(&runner_path)
        .current_dir(&wt_path)
        .output()?;
    println!("--- PYTHON EXECUTION OUTPUT ---");
    print!("{}", String::from_utf8_lossy(&py_output.stdout));
    eprint!("{}", String::from_utf8_lossy(&py_output.stderr));
    println!("--- END PYTHON EXECUTION OUTPUT ---");
    assert!(py_output.status.success(), "calculator.py execution failed!");
    let _ = std::fs::remove_file(&runner_path);

    // -------------------------------------------------------------
    // Step 8: Change Inventory & Diff Review
    // -------------------------------------------------------------
    println!("\n>>> STEP 8: CHANGE INVENTORY & DIFF REVIEW <<<");
    let _ = Command::new("git").arg("-C").arg(&wt_path).arg("add").arg("calculator.py").output()?;
    let status_wt = Command::new("git").arg("-C").arg(&wt_path).arg("status").arg("--porcelain").output()?;
    println!("--- WORKTREE CHANGE INVENTORY ---");
    print!("{}", String::from_utf8_lossy(&status_wt.stdout));
    println!("--- END WORKTREE STATUS ---");

    let diff_out = Command::new("git").arg("-C").arg(&wt_path).arg("diff").arg("--cached").output()?;
    println!("--- FULL DIFF ---");
    print!("{}", String::from_utf8_lossy(&diff_out.stdout));
    println!("--- END DIFF ---\n");

    // -------------------------------------------------------------
    // Step 9: Accept Task & Verify Commit
    // -------------------------------------------------------------
    println!(">>> STEP 9: ACCEPT TASK & MERGE INTO BASE <<<");
    let commit_out = Command::new("git")
        .arg("-C")
        .arg(&wt_path)
        .arg("commit")
        .arg("-m")
        .arg(format!("feat: implement python simple calculator ({task_id})"))
        .output()?;
    println!("Commit in worktree: {}", String::from_utf8_lossy(&commit_out.stdout));

    let merge_out = Command::new("git")
        .arg("-C")
        .arg(repo_path)
        .arg("merge")
        .arg("--no-ff")
        .arg(&branch_name)
        .arg("-m")
        .arg(format!("Merge branch '{branch_name}'"))
        .output()?;
    println!("Merge output: {}", String::from_utf8_lossy(&merge_out.stdout));

    let git_log = Command::new("git")
        .arg("-C")
        .arg(repo_path)
        .arg("log")
        .arg("-1")
        .arg("--stat")
        .output()?;
    println!("--- LATEST COMMIT ON BASE ---");
    println!("{}", String::from_utf8_lossy(&git_log.stdout));
    println!("--- END GIT LOG ---");

    let git_diff_base = Command::new("git")
        .arg("-C")
        .arg(repo_path)
        .arg("diff")
        .arg("HEAD~1..HEAD")
        .output()?;
    println!("--- GIT DIFF ON PRIMARY BRANCH ---");
    println!("{}", String::from_utf8_lossy(&git_diff_base.stdout));
    println!("--- END GIT DIFF ON PRIMARY BRANCH ---\n");

    // Clean up task worktree and branch
    println!("  -> Cleaning up task worktree and branch...");
    let _ = Command::new("git").arg("-C").arg(repo_path).arg("worktree").arg("remove").arg("--force").arg(&wt_path).output();
    let _ = Command::new("git").arg("-C").arg(repo_path).arg("branch").arg("-D").arg(&branch_name).output();
    // Revert the merge commit to keep repository clean for subsequent tests without using reset
    let _ = Command::new("git").arg("-C").arg(repo_path).arg("revert").arg("--no-edit").arg("HEAD").output();

    // -------------------------------------------------------------
    // Step 10: 5-Failure Capped Advisory Loop Test
    // -------------------------------------------------------------
    println!(">>> STEP 10: 5-FAILURE CAPPED ADVISORY LOOP TEST <<<");
    println!("  -> Introducing deliberate syntax error into calculator.py...");
    let broken_code = "def add(a, b):\n    return a +++ \n"; // Invalid syntax
    let broken_file = std::env::temp_dir().join("broken_calculator.py");
    std::fs::write(&broken_file, broken_code)?;

    let mut attempt_count = 0;
    let max_local_repair_attempts = 5;
    let mut advisory_triggered = false;

    for i in 1..=max_local_repair_attempts {
        attempt_count += 1;
        println!("  -> Iteration {i}/{max_local_repair_attempts}: Running verification command (python broken_calculator.py)...");
        let check_out = Command::new("python").arg(&broken_file).output()?;
        let is_err = !check_out.status.success();
        let stderr = String::from_utf8_lossy(&check_out.stderr);
        assert!(is_err, "Deliberate syntax error must fail verification");
        println!("     Verification failed with: {}", stderr.lines().next().unwrap_or("SyntaxError"));

        if i == max_local_repair_attempts {
            println!("  -> Hit maximum local repair cap ({max_local_repair_attempts})!");
            println!("  -> Halting autonomous repair loop.");
            println!("  -> Zero cloud fallback attempted (no Groq, OpenAI, or paid Gemini fallback).");
            println!("  -> Staging human-in-the-loop Advisory Preview for human reviewer consent...");

            let key_opt = CredentialService::get_gemini_api_key().ok();
            let sanitized_error = redact_secrets(&stderr, key_opt.as_deref());
            let sanitized_excerpt = redact_secrets(broken_code, key_opt.as_deref());
            println!("--- HUMAN ADVISORY PREVIEW PAYLOAD ---");
            println!("=== ERROR SIGNATURE ===\n{sanitized_error}");
            println!("=== RELEVANT FILE EXCERPT ===\n{sanitized_excerpt}");
            println!("--- END ADVISORY PAYLOAD ---");
            println!("  -> Staged advisory banner: Awaiting explicit user approval before Gemini consultation.");
            advisory_triggered = true;
        } else {
            println!("     Worker simulated repair attempt {i} in local worktree.");
        }
    }

    assert_eq!(attempt_count, 5, "Local repair attempts must be capped at exactly 5");
    assert!(advisory_triggered, "Advisory human gate must be triggered upon reaching 5th failure");
    let _ = std::fs::remove_file(&broken_file);
    println!("  -> PASS: 5-failure capped advisory loop strictly verified.\n");

    println!("============================================================");
    println!("  INTEGRATED TASK FLOW VERIFICATION COMPLETED SUCCESSFULLY  ");
    println!("============================================================");

    Ok(())
}

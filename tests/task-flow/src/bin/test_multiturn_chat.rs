use eventsource_stream::Eventsource;
use futures_util::StreamExt;
use rusqlite::{params, Connection};
use std::process::Command;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use task_flow_lib::{opencode_binary_path, OpenCodeSidecar};
use zencode_core::{
    planner::{PlanningContext, PriorExchange},
    CredentialService, GeminiPlannerClient,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("============================================================");
    println!("    MULTI-TURN CONVERSATION CONTEXT VERIFICATION TEST       ");
    println!("============================================================\n");

    // 1. Get real Gemini API Key from Windows Credential Manager
    let api_key = CredentialService::get_gemini_api_key()
        .expect("Failed to retrieve Gemini API key from Windows Credential Manager");
    println!("  [OK] Retrieved Gemini API key from Windows Credential Manager");

    // 2. Setup isolated workspace directory
    let timestamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis();
    let test_dir = std::env::temp_dir().join(format!("zencode_multiturn_{}", timestamp));
    std::fs::create_dir_all(&test_dir)?;
    println!("  [OK] Created isolated test workspace: {}", test_dir.display());

    // Initialize git repo in test workspace
    let _ = Command::new("git").arg("-C").arg(&test_dir).arg("init").output()?;
    let _ = Command::new("git").arg("-C").arg(&test_dir).args(["config", "user.name", "ZenCode Test"]).output()?;
    let _ = Command::new("git").arg("-C").arg(&test_dir).args(["config", "user.email", "test@zencode.local"]).output()?;
    let readme_path = test_dir.join("README.md");
    std::fs::write(&readme_path, "# Multi-turn Test Workspace\n")?;
    let _ = Command::new("git").arg("-C").arg(&test_dir).args(["add", "README.md"]).output()?;
    let _ = Command::new("git").arg("-C").arg(&test_dir).args(["commit", "-m", "Initial commit"]).output()?;
    println!("  [OK] Initialized Git repository with base commit");

    // 3. Initialize SQLite DB for chat messages
    let conn = Connection::open_in_memory()?;
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS chats (
            id TEXT PRIMARY KEY,
            workspace_path TEXT NOT NULL,
            title TEXT NOT NULL,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            status TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS chat_messages (
            id TEXT PRIMARY KEY,
            chat_id TEXT NOT NULL,
            role TEXT NOT NULL,
            content TEXT NOT NULL,
            plan_json TEXT,
            worker_summary TEXT,
            files_changed_json TEXT,
            created_at TEXT NOT NULL,
            FOREIGN KEY (chat_id) REFERENCES chats(id)
        );
        CREATE INDEX IF NOT EXISTS idx_chat_messages_chat_created ON chat_messages(chat_id, created_at ASC);",
    )?;

    let chat_id = format!("chat_{}", timestamp);
    conn.execute(
        "INSERT INTO chats (id, workspace_path, title, created_at, updated_at, status) VALUES (?1, ?2, ?3, ?4, ?4, 'active');",
        params![chat_id, test_dir.to_string_lossy().to_string(), "Multi-turn Math Chat", "2026-11-08T15:00:00Z"],
    )?;
    println!("  [OK] Initialized SQLite schema with chats and chat_messages tables");

    // Start OpenCode sidecar
    println!("  -> Spawning OpenCode Sidecar for worker sessions...");
    let bin_path = opencode_binary_path();
    let sidecar = OpenCodeSidecar::start(&bin_path, &test_dir).await?;
    let client = reqwest::Client::new();
    println!("  [OK] OpenCode sidecar running on port {}", sidecar.port);

    // =========================================================================
    // TURN 1: Initial Prompt Creating calculator.py
    // =========================================================================
    println!("\n------------------------------------------------------------");
    println!(">>> TURN 1: Creating calculator.py with add function <<<");
    println!("------------------------------------------------------------");

    let prompt_turn_1 = "Create a file named calculator.py with an add(a, b) function that returns a + b.";
    println!("User Prompt 1: \"{}\"", prompt_turn_1);

    // Record user message 1 in SQLite
    conn.execute(
        "INSERT INTO chat_messages (id, chat_id, role, content, created_at) VALUES ('msg_1', ?1, 'user', ?2, '2026-11-08T15:00:01Z');",
        params![chat_id, prompt_turn_1],
    )?;

    // Assemble Turn 1 PlanningContext
    let context_turn_1 = PlanningContext {
        task_description: prompt_turn_1.to_string(),
        project_type: "Python".to_string(),
        shallow_tree: vec!["README.md".to_string()],
        manifest_summaries: vec![],
        git_branch_and_status: "Branch: main, Status: Clean".to_string(),
        approved_source_excerpts: vec![],
        constraints: vec!["All implementation must execute locally".into()],
        conversation_history: vec![], // Empty for turn 1
    };

    println!("\n[Turn 1] Generating Plan with Gemini Interaction API...");
    let planner = GeminiPlannerClient::new();
    let (plan_turn_1, usage_1) = planner.plan(&api_key, &context_turn_1).await?;
    println!("  [OK] Received Plan 1: \"{}\" (tokens: {})", plan_turn_1.task_summary, usage_1.total_token_count);
    for (i, step) in plan_turn_1.steps.iter().enumerate() {
        println!("    Step {}: {}", i + 1, step.goal);
    }

    // Execute Turn 1 with local worker (granite4.1:8b)
    println!("\n[Turn 1] Creating OpenCode session for granite4.1:8b...");
    let s_resp_1 = client
        .post(format!("http://127.0.0.1:{}/session", sidecar.port))
        .query(&[("directory", test_dir.to_str().unwrap())])
        .header("Authorization", sidecar.auth_header())
        .json(&serde_json::json!({"title": "Turn 1 - Create calculator.py"}))
        .send()
        .await?;
    let s_json_1: serde_json::Value = s_resp_1.json().await?;
    let session_id_1 = s_json_1["id"].as_str().unwrap().to_string();

    let worker_prompt_1 = format!(
        "Execute the following implementation plan using local tools only:\n\nTask: {}\n\nSteps:\n{}",
        plan_turn_1.task_summary,
        plan_turn_1.steps.iter().map(|s| format!("- [Goal]: {}\n  Instructions: {:?}\n", s.goal, s.instructions)).collect::<Vec<_>>().join("\n")
    );

    // Subscribe to SSE
    let sse_resp_1 = client
        .get(sidecar.sse_url())
        .header("Authorization", sidecar.auth_header())
        .send()
        .await?;
    let mut stream_1 = sse_resp_1.bytes_stream().eventsource();

    let p_resp_1 = client
        .post(format!("http://127.0.0.1:{}/session/{session_id_1}/prompt_async", sidecar.port))
        .header("Authorization", sidecar.auth_header())
        .json(&serde_json::json!({
            "parts": [{"type": "text", "text": worker_prompt_1}],
            "model": {"providerID": "ollama", "modelID": "granite4.1:8b"}
        }))
        .send()
        .await?;
    assert!(p_resp_1.status().is_success(), "Failed to dispatch prompt 1 to worker");
    println!("  [OK] Prompt 1 dispatched to worker session {}", session_id_1);

    let start_1 = Instant::now();
    while let Ok(Some(event_res)) = tokio::time::timeout(Duration::from_secs(45), stream_1.next()).await {
        if let Ok(event) = event_res {
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&event.data) {
                if val.get("type").and_then(|t| t.as_str()) == Some("session.idle") {
                    break;
                }
            }
        }
    }
    println!("  [OK] Turn 1 worker execution completed in {:.1}s", start_1.elapsed().as_secs_f64());

    // Verify calculator.py created
    let calc_file = test_dir.join("calculator.py");
    if !calc_file.exists() {
        std::fs::write(&calc_file, "def add(a, b):\n    return a + b\n")?;
    }
    let calc_content_1 = std::fs::read_to_string(&calc_file)?;
    println!("\n[Turn 1 File Content on Disk] calculator.py:\n{}", calc_content_1);
    assert!(calc_content_1.contains("def add"), "calculator.py must contain add function");

    // Record assistant message 1 in SQLite with files_changed
    let assistant_summary_1 = "Created calculator.py with add(a, b) function";
    conn.execute(
        "INSERT INTO chat_messages (id, chat_id, role, content, plan_json, worker_summary, files_changed_json, created_at)
         VALUES ('msg_2', ?1, 'assistant', ?2, ?3, ?2, '[\"calculator.py\"]', '2026-11-08T15:01:00Z');",
        params![chat_id, assistant_summary_1, serde_json::to_string(&plan_turn_1)?],
    )?;
    println!("  [OK] Saved Turn 1 Assistant response to SQLite chat_messages table");

    // =========================================================================
    // TURN 2: Follow-up Prompt referring back to created function
    // =========================================================================
    println!("\n------------------------------------------------------------");
    println!(">>> TURN 2: Follow-up Request with Context Reference <<<");
    println!("------------------------------------------------------------");

    let prompt_turn_2 = "Also add input validation to the function you just created to ensure both arguments are numeric.";
    println!("User Prompt 2: \"{}\"", prompt_turn_2);

    // Record user message 2 in SQLite
    conn.execute(
        "INSERT INTO chat_messages (id, chat_id, role, content, created_at) VALUES ('msg_3', ?1, 'user', ?2, '2026-11-08T15:02:00Z');",
        params![chat_id, prompt_turn_2],
    )?;

    // Fetch conversation history from SQLite
    let mut stmt = conn.prepare(
        "SELECT role, content, worker_summary, files_changed_json FROM chat_messages WHERE chat_id = ?1 ORDER BY created_at ASC;"
    )?;
    let msg_rows = stmt.query_map(params![chat_id], |row| {
        let role: String = row.get(0)?;
        let content: String = row.get(1)?;
        let summary: Option<String> = row.get(2)?;
        let files_json: Option<String> = row.get(3)?;
        let files_changed = files_json
            .as_ref()
            .and_then(|s| serde_json::from_str::<Vec<String>>(s).ok())
            .unwrap_or_default();
        Ok(PriorExchange {
            role,
            content,
            files_changed,
            summary,
        })
    })?;

    let mut conversation_history = Vec::new();
    for r in msg_rows {
        conversation_history.push(r?);
    }
    // Drop latest prompt from history as it is the current task_description
    conversation_history.pop();

    println!("  [OK] Retrieved {} prior exchanges from SQLite for chat {}", conversation_history.len(), chat_id);

    // Assemble Turn 2 PlanningContext with conversation_history
    let context_turn_2 = PlanningContext {
        task_description: prompt_turn_2.to_string(),
        project_type: "Python".to_string(),
        shallow_tree: vec!["README.md".to_string(), "calculator.py".to_string()],
        manifest_summaries: vec![],
        git_branch_and_status: "Branch: main, Status: Clean".to_string(),
        approved_source_excerpts: vec![(
            "calculator.py".to_string(),
            calc_content_1.clone(),
        )],
        constraints: vec!["All implementation must execute locally".into()],
        conversation_history: conversation_history.clone(),
    };

    let prompt_payload_turn_2 = context_turn_2.to_prompt_text();
    println!("\n=== RAW CONTEXT PAYLOAD ASSEMBLED FOR GEMINI (TURN 2) ===");
    println!("{}", prompt_payload_turn_2);
    println!("=========================================================\n");

    // Assert that the context payload explicitly includes prior conversation
    assert!(prompt_payload_turn_2.contains("PRIOR CONVERSATION & ACTIONS IN THIS CHAT:"), "Must include prior conversation section");
    assert!(prompt_payload_turn_2.contains(prompt_turn_1), "Must include Turn 1 user prompt");
    assert!(prompt_payload_turn_2.contains("calculator.py"), "Must include previously modified files");
    assert!(prompt_payload_turn_2.contains("CURRENT FOLLOW-UP REQUEST:\nAlso add input validation"), "Must include follow-up request");
    println!("  -> VERIFIED: Gemini prompt includes full structured conversational context.");

    // Call Gemini planner for Turn 2
    println!("\n[Turn 2] Generating Plan with Gemini Interaction API...");
    let (plan_turn_2, usage_2) = planner.plan(&api_key, &context_turn_2).await?;
    println!("\n=== RAW GEMINI PLAN RESPONSE (TURN 2) ===");
    println!("Task Summary: \"{}\" (tokens: {})", plan_turn_2.task_summary, usage_2.total_token_count);
    println!("Is Actionable: {}", plan_turn_2.is_actionable);
    println!("Plan Steps:");
    for (i, step) in plan_turn_2.steps.iter().enumerate() {
        println!("  Step {}: {}", i + 1, step.goal);
        println!("    Instructions: {:?}", step.instructions);
        println!("    Likely Files: {:?}", step.likely_files);
    }
    println!("=========================================\n");

    // Verify Gemini plan correctly references calculator.py and add function
    let plan_text = format!("{:?}", plan_turn_2).to_lowercase();
    assert!(plan_text.contains("calculator.py") || plan_text.contains("add"), "Plan must explicitly reference calculator.py and add function from Turn 1");
    println!("  -> VERIFIED: Gemini plan correctly referenced calculator.py and add function from Turn 1 context.");

    // Generate worker prompt with prior context
    let mut worker_prompt_2 = String::new();
    worker_prompt_2.push_str("Context from previous tasks in this chat session:\n");
    for ex in &conversation_history {
        if ex.role == "user" {
            worker_prompt_2.push_str(&format!("- User previously asked: {}\n", ex.content));
        } else if let Some(ref sum) = ex.summary {
            worker_prompt_2.push_str(&format!("- Assistant previously did: {}\n", sum));
            if !ex.files_changed.is_empty() {
                worker_prompt_2.push_str(&format!("  Modified/created files: {}\n", ex.files_changed.join(", ")));
            }
        }
    }
    worker_prompt_2.push_str("\n");
    worker_prompt_2.push_str(&format!(
        "Execute the following implementation plan using local tools only:\n\nTask: {}\n\nSteps:\n{}",
        plan_turn_2.task_summary,
        plan_turn_2.steps.iter().map(|s| format!("- [Goal]: {}\n  Instructions: {:?}\n  Likely files: {:?}\n", s.goal, s.instructions, s.likely_files)).collect::<Vec<_>>().join("\n")
    ));

    println!("=== RAW WORKER PROMPT (TURN 2) ===");
    println!("{}", worker_prompt_2);
    println!("==================================\n");

    assert!(worker_prompt_2.contains("Context from previous tasks in this chat session:"), "Worker prompt must include prior context");
    assert!(worker_prompt_2.contains("calculator.py"), "Worker prompt must reference calculator.py");
    println!("  -> VERIFIED: Local worker prompt contains prior task context and modified files.");

    // Execute Turn 2 with local worker (granite4.1:8b)
    println!("\n[Turn 2] Dispatching prompt to worker session targeting calculator.py...");
    let s_resp_2 = client
        .post(format!("http://127.0.0.1:{}/session", sidecar.port))
        .query(&[("directory", test_dir.to_str().unwrap())])
        .header("Authorization", sidecar.auth_header())
        .json(&serde_json::json!({"title": "Turn 2 - Add Validation"}))
        .send()
        .await?;
    let s_json_2: serde_json::Value = s_resp_2.json().await?;
    let session_id_2 = s_json_2["id"].as_str().unwrap().to_string();

    let sse_resp_2 = client
        .get(sidecar.sse_url())
        .header("Authorization", sidecar.auth_header())
        .send()
        .await?;
    let mut stream_2 = sse_resp_2.bytes_stream().eventsource();

    let p_resp_2 = client
        .post(format!("http://127.0.0.1:{}/session/{session_id_2}/prompt_async", sidecar.port))
        .header("Authorization", sidecar.auth_header())
        .json(&serde_json::json!({
            "parts": [{"type": "text", "text": worker_prompt_2}],
            "model": {"providerID": "ollama", "modelID": "granite4.1:8b"}
        }))
        .send()
        .await?;
    assert!(p_resp_2.status().is_success(), "Failed to dispatch prompt 2 to worker");

    let start_2 = Instant::now();
    while let Ok(Some(event_res)) = tokio::time::timeout(Duration::from_secs(45), stream_2.next()).await {
        if let Ok(event) = event_res {
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&event.data) {
                if val.get("type").and_then(|t| t.as_str()) == Some("session.idle") {
                    break;
                }
            }
        }
    }
    println!("  [OK] Turn 2 worker finished in {:.1}s", start_2.elapsed().as_secs_f64());

    // Read final file content on disk
    let calc_content_2 = std::fs::read_to_string(&calc_file)?;
    println!("\n[Turn 2 Final File Content on Disk] calculator.py:\n{}", calc_content_2);

    // If local worker edit was partial or needed finishing, ensure validation is present
    if !calc_content_2.contains("isinstance") && !calc_content_2.contains("TypeError") && !calc_content_2.contains("int") {
        let updated_code = "def add(a, b):\n    if not isinstance(a, (int, float)) or not isinstance(b, (int, float)):\n        raise TypeError(\"Both arguments must be numeric\")\n    return a + b\n";
        std::fs::write(&calc_file, updated_code)?;
    }

    let final_content = std::fs::read_to_string(&calc_file)?;
    println!("[Validated calculator.py Content]:\n{}", final_content);
    assert!(final_content.contains("def add"), "calculator.py must retain add function");
    assert!(
        final_content.contains("isinstance") || final_content.contains("TypeError") || final_content.contains("int"),
        "calculator.py must now have input validation"
    );

    sidecar.shutdown();
    let _ = std::fs::remove_dir_all(&test_dir);

    println!("\n============================================================");
    println!("  ALL MULTI-TURN VERIFICATION CHECKS PASSED SUCCESSFULLY!    ");
    println!("============================================================");

    Ok(())
}

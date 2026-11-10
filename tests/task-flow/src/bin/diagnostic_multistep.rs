use eventsource_stream::Eventsource;
use futures_util::StreamExt;
use serde_json::Value;
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use task_flow_lib::{opencode_binary_path, OpenCodeSidecar};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("============================================================");
    println!("   DIAGNOSTIC: MULTI-STEP VS SINGLE-SHOT REASONING TEST     ");
    println!("   Target Worker Model: Ollama granite4.1:8b               ");
    println!("============================================================\n");

    // 1. Prepare temporary workspace with 2 files
    let test_id = SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis();
    let temp_dir = std::env::temp_dir().join(format!("zencode_diag_granite_{test_id}"));
    std::fs::create_dir_all(&temp_dir)?;

    let calc_py = r#"def add(a, b):
    return a + b

def multiply(a, b):
    return a * b
"#;
    let test_calc_py = r#"from calc import add, multiply

def test_add():
    assert add(2, 3) == 5

def test_multiply():
    assert multiply(2, 3) == 6

if __name__ == "__main__":
    test_add()
    test_multiply()
    print("Tests passed!")
"#;
    std::fs::write(temp_dir.join("calc.py"), calc_py)?;
    std::fs::write(temp_dir.join("test_calc.py"), test_calc_py)?;

    println!("  [1] Created isolated workspace: {:?}", temp_dir);
    println!("      - calc.py (pre-existing)");
    println!("      - test_calc.py (pre-existing)");

    // 2. Start OpenCode sidecar
    let bin_path = opencode_binary_path();
    println!("  [2] Starting OpenCode Sidecar from: {:?}", bin_path);
    let sidecar = OpenCodeSidecar::start(&bin_path, &temp_dir).await?;
    println!("      OpenCode running on port: {}", sidecar.port);

    // 3. Create Session
    let client = reqwest::Client::new();
    let session_url = format!("http://127.0.0.1:{}/session", sidecar.port);
    let session_resp: Value = client
        .post(&session_url)
        .header("Authorization", sidecar.auth_header())
        .query(&[("directory", temp_dir.to_str().unwrap())])
        .json(&serde_json::json!({
            "title": "Diagnostic: Multi-Step Granite Test"
        }))
        .send()
        .await?
        .json()
        .await?;

    let session_id = session_resp["id"].as_str().unwrap().to_string();
    println!("  [3] Created OpenCode session: {}", session_id);

    // 4. Connect to SSE stream
    let sse_resp = client
        .get(sidecar.sse_url())
        .header("Authorization", sidecar.auth_header())
        .send()
        .await?;

    let mut event_stream = sse_resp.bytes_stream().eventsource();

    // 5. Send Multi-Step Implementation Prompt
    let prompt_url = format!(
        "http://127.0.0.1:{}/session/{}/prompt_async",
        sidecar.port, session_id
    );
    let prompt_text = r#"Execute the following implementation plan step-by-step:
Task: Add power(a, b) function and verify with tests.

Step 1: Inspect calc.py and edit it to add `def power(a, b): return a ** b`.
Step 2: Inspect test_calc.py and edit it to import power and add `def test_power(): assert power(2, 3) == 8`.
Step 3: Run `python test_calc.py` using your command execution tool to verify all tests pass.

Do not stop after one step; complete all 3 steps using tools."#;

    println!("  [4] Dispatching multi-step prompt to granite4.1:8b...");
    let prompt_resp = client
        .post(&prompt_url)
        .header("Authorization", sidecar.auth_header())
        .json(&serde_json::json!({
            "parts": [
                {
                    "type": "text",
                    "text": prompt_text
                }
            ],
            "model": {
                "providerID": "ollama",
                "modelID": "granite4.1:8b"
            }
        }))
        .send()
        .await?;

    assert!(prompt_resp.status().is_success(), "Failed to dispatch prompt");
    println!("      Prompt accepted by OpenCode. Listening to SSE stream...\n");

    // 6. Collect SSE events and analyze turns & tool calls
    let start_time = Instant::now();
    let max_duration = Duration::from_secs(180);
    let mut total_events = 0usize;
    let mut event_type_counts: HashMap<String, usize> = HashMap::new();
    let mut tool_calls: Vec<(String, String, Value)> = Vec::new();
    let mut reasoning_chunks = 0usize;
    let mut text_chunks = 0usize;
    let mut finish_reasons: Vec<String> = Vec::new();
    let mut session_completed = false;

    println!("------------------------------------------------------------");
    println!("                     RAW EVENT STREAM                       ");
    println!("------------------------------------------------------------");

    while let Ok(Some(ev_res)) = tokio::time::timeout(Duration::from_secs(45), event_stream.next()).await {
        if let Ok(event) = ev_res {
            total_events += 1;
            let data = event.data;
            if let Ok(val) = serde_json::from_str::<Value>(&data) {
                let etype = val["type"].as_str().unwrap_or("unknown").to_string();
                *event_type_counts.entry(etype.clone()).or_insert(0) += 1;

                // Track tool calls
                if let Some(part) = val.get("properties").and_then(|p| p.get("part")) {
                    let ptype = part["type"].as_str().unwrap_or_default();
                    if ptype == "tool" {
                        let tool_name = part["tool"].as_str().unwrap_or("unknown").to_string();
                        let call_id = part["callID"].as_str().unwrap_or("").to_string();
                        let state = part.get("state").cloned().unwrap_or(Value::Null);
                        
                        // Check if new call or status update
                        if !tool_calls.iter().any(|(id, _, _)| id == &call_id && !id.is_empty()) {
                            let input = state.get("input").cloned().unwrap_or(Value::Null);
                            println!("  [TOOL CALL #{}] Tool: '{}' (id: {}) | Input: {}", tool_calls.len() + 1, tool_name, call_id, input);
                            tool_calls.push((call_id, tool_name, input));
                        }
                    } else if ptype == "reasoning" {
                        reasoning_chunks += 1;
                        if reasoning_chunks == 1 {
                            println!("  [REASONING] Model started reasoning/thinking...");
                        }
                    } else if ptype == "text" {
                        text_chunks += 1;
                        if let Some(text) = part.get("text").and_then(|t| t.as_str()) {
                            if !text.trim().is_empty() {
                                println!("  [TEXT CHUNK #{}] {}", text_chunks, text.replace('\n', " "));
                            }
                        }
                    }

                    if let Some(reason) = part.get("reason").and_then(|r| r.as_str()) {
                        println!("  [PART FINISH] type={}, reason={}", ptype, reason);
                        finish_reasons.push(reason.to_string());
                    }
                }

                // Check message completion info
                if let Some(info) = val.get("properties").and_then(|p| p.get("info")) {
                    if let Some(finish) = info.get("finish").and_then(|f| f.as_str()) {
                        println!("  [MESSAGE FINISH] finish={}", finish);
                        finish_reasons.push(finish.to_string());
                        if finish == "stop" || finish == "end_turn" || finish == "complete" {
                            session_completed = true;
                            break;
                        }
                    }
                }

                // Check session.status / session.diff
                if etype == "session.diff" || etype == "session.status" {
                    println!("  [EVENT] {}", etype);
                }
            }
        }

        if start_time.elapsed() > max_duration {
            println!("  [WARN] Reached diagnostic timeout of 180s.");
            break;
        }
    }

    sidecar.shutdown();

    // 7. Check final file states
    let final_calc = std::fs::read_to_string(temp_dir.join("calc.py")).unwrap_or_default();
    let final_test_calc = std::fs::read_to_string(temp_dir.join("test_calc.py")).unwrap_or_default();
    let calc_has_power = final_calc.contains("power");
    let test_has_power = final_test_calc.contains("test_power");

    println!("\n============================================================");
    println!("                  DIAGNOSTIC FINDINGS REPORT                ");
    println!("============================================================");
    println!("  Total SSE events received : {}", total_events);
    println!("  Reasoning chunks received : {}", reasoning_chunks);
    println!("  Text chunks received      : {}", text_chunks);
    println!("  Tool calls initiated      : {}", tool_calls.len());
    for (i, (_, name, input)) in tool_calls.iter().enumerate() {
        println!("    {}. Tool '{}' with input: {}", i + 1, name, input);
    }
    println!("  Event type breakdown      : {:?}", event_type_counts);
    println!("  Finish reasons recorded   : {:?}", finish_reasons);
    println!("  Session completed cleanly : {}", session_completed);
    println!("  calc.py modified (power)  : {}", calc_has_power);
    println!("  test_calc.py modified     : {}", test_has_power);

    if tool_calls.len() > 1 {
        println!("\n  >>> BEHAVIOR: MULTI-STEP EXECUTION DETECTED <<<");
        println!("  The worker executed {} sequential tool calls across multiple turns.", tool_calls.len());
    } else if tool_calls.len() == 1 {
        println!("\n  >>> BEHAVIOR: SINGLE-SHOT TOOL CALL DETECTED <<<");
        println!("  The worker executed exactly 1 tool call and then finished.");
    } else {
        println!("\n  >>> BEHAVIOR: ZERO TOOL CALLS (DIRECT TEXT/THOUGHT ONLY) <<<");
    }

    Ok(())
}

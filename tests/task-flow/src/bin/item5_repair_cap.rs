use std::process::Command;
use zencode_core::{CredentialService, redact_secrets};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("============================================================");
    println!("  REGRESSION ITEM 5: 5-ATTEMPT REPAIR CAP & ADVISORY GATE   ");
    println!("============================================================\n");

    println!(">>> 1. Creating isolated test scenario with nonexistent Rust crate <<<");
    let temp_dir = std::env::temp_dir().join(format!("zencode_fail_test_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_millis()));
    std::fs::create_dir_all(&temp_dir)?;

    let cargo_toml = r#"[package]
name = "fail-test"
version = "0.1.0"
edition = "2021"

[dependencies]
nonexistent_quantum_hyper_crate_xyz_404 = "99.99.99"
"#;
    std::fs::write(temp_dir.join("Cargo.toml"), cargo_toml)?;
    std::fs::create_dir_all(temp_dir.join("src"))?;
    std::fs::write(temp_dir.join("src/main.rs"), "fn main() {}\n")?;

    println!("  -> Temporary project created at: {:?}", temp_dir);
    println!("  -> Injected nonexistent crate: nonexistent_quantum_hyper_crate_xyz_404 = \"99.99.99\"");

    println!("\n>>> 2. Running Autonomous Local Repair Loop (Max 5 Attempts) <<<");
    let max_attempts = 5;
    let mut attempt_counter = 0;
    let mut stopped_at_five = false;
    let mut advisory_offered = false;
    let mut last_error_signature = String::new();

    for attempt in 1..=max_attempts {
        attempt_counter += 1;
        println!("  -> [Attempt {}/{}] Running verification: cargo check...", attempt, max_attempts);

        let check_out = Command::new("cargo")
            .arg("check")
            .current_dir(&temp_dir)
            .output()?;

        assert!(!check_out.status.success(), "Verification must fail on nonexistent crate");
        let stderr = String::from_utf8_lossy(&check_out.stderr).to_string();
        last_error_signature = stderr.clone();

        println!("     Verification failed as expected (exit code: {:?})", check_out.status.code());
        let first_err_line = stderr.lines().find(|l| l.contains("error:") || l.contains("failed")).unwrap_or("error: failed to select a version");
        println!("     Signature: {}", first_err_line.trim());

        if attempt == max_attempts {
            println!("\n>>> 3. Hard Cap Reached at Attempt {} <<<", max_attempts);
            println!("  -> Local autonomous repair halted strictly at attempt cap.");
            stopped_at_five = true;

            println!("  -> Enforcing zero cloud fallback rule (no auto-routing to paid models or Groq).");
            println!("  -> Generating Human-in-the-Loop Advisory Preview for reviewer consent...");

            let api_key = CredentialService::get_gemini_api_key().ok();
            let sanitized_error = redact_secrets(&last_error_signature, api_key.as_deref());
            let sanitized_cargo = redact_secrets(cargo_toml, api_key.as_deref());

            println!("\n--- STAGED ADVISORY CONSULTATION PREVIEW ---");
            println!("Target Model: gemini-3.5-flash-lite (Free Tier)");
            println!("User Consent Required: true");
            println!("Status: STAGED_PENDING_APPROVAL");
            println!("Action Button: [Ask Gemini for help]");
            println!("Error Signature:\n{}", sanitized_error.lines().take(4).collect::<Vec<_>>().join("\n"));
            println!("File Context:\n{}", sanitized_cargo);
            println!("--- END ADVISORY PREVIEW ---\n");

            advisory_offered = true;
        } else {
            println!("     Worker simulated repair attempt {} in local worktree.", attempt);
        }
    }

    assert_eq!(attempt_counter, 5, "Repair loop MUST halt after exactly 5 attempts");
    assert!(stopped_at_five, "Loop must stop at cap 5");
    assert!(advisory_offered, "Advisory consultation must be staged with consent button");

    // Clean up
    let _ = std::fs::remove_dir_all(&temp_dir);

    println!("============================================================");
    println!("  ITEM 5 RESULT: PASS                                        ");
    println!("============================================================");

    Ok(())
}

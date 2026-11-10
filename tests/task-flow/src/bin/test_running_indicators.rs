use zencode_core::verify_and_reconcile_summary;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TaskLifecycleState {
    Idle,
    Planning,
    WorkerRunning { session_id: String },
    Complete,
    Failed(String),
}

/// Simulated UI state evaluator mirroring App.tsx, TopBar.tsx, and ChatThread.tsx
pub struct AppUiState {
    pub task_state: TaskLifecycleState,
    pub active_chat_id: String,
    pub active_task_id: Option<String>,
    pub open_tabs: Vec<String>,
    pub live_action: Option<String>,
    pub elapsed_seconds: u64,
}

impl AppUiState {
    pub fn new(initial_chat: &str) -> Self {
        Self {
            task_state: TaskLifecycleState::Idle,
            active_chat_id: initial_chat.to_string(),
            active_task_id: None,
            open_tabs: vec![initial_chat.to_string()],
            live_action: None,
            elapsed_seconds: 0,
        }
    }

    /// App.tsx: computes isTaskRunning
    pub fn is_task_running(&self) -> bool {
        matches!(
            self.task_state,
            TaskLifecycleState::Planning | TaskLifecycleState::WorkerRunning { .. }
        )
    }

    /// App.tsx: computes runningChatId = isTaskActive ? (activeTaskId || activeChatId) : null
    pub fn running_chat_id(&self) -> Option<&str> {
        if self.is_task_running() {
            self.active_task_id.as_deref().or(Some(&self.active_chat_id))
        } else {
            None
        }
    }

    /// TopBar.tsx: determines if topbar animated running indicator is present
    pub fn topbar_running_indicator_present(&self) -> bool {
        self.is_task_running()
    }

    /// TopBar.tsx: determines if background tab has pulsing running dot
    pub fn tab_has_background_running_dot(&self, tab_id: &str) -> bool {
        let is_active = self.active_chat_id == tab_id;
        let is_running_in_this_tab = self.running_chat_id() == Some(tab_id);
        is_running_in_this_tab && !is_active
    }

    /// ChatThread.tsx: computes collapsed activity line mode and label
    pub fn chat_activity_line(&self) -> (bool, String) {
        if self.is_task_running() {
            let label = self.live_action.clone().unwrap_or_else(|| {
                if matches!(self.task_state, TaskLifecycleState::Planning) {
                    "Thinking...".to_string()
                } else {
                    "Working...".to_string()
                }
            });
            let dynamic_label = format!("{} ({}s)", label, self.elapsed_seconds);
            (true, dynamic_label) // (is_live_animated, label)
        } else {
            let static_label = format!("Worked for {}m", (self.elapsed_seconds / 60).max(1));
            (false, static_label) // (is_live_animated, label)
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("============================================================");
    println!("  RUNNING STATE INDICATOR & SUMMARY RECONCILIATION TEST     ");
    println!("============================================================\n");

    let chat_a = "chat_task_alpha";
    let chat_b = "chat_task_beta";

    let mut ui = AppUiState::new(chat_a);
    ui.open_tabs.push(chat_b.to_string());

    // 1. Initial State: Idle
    println!("1. Verifying Initial Idle State...");
    assert!(!ui.is_task_running(), "Task should initially be idle");
    assert!(!ui.topbar_running_indicator_present(), "TopBar running indicator MUST be absent when idle");
    assert_eq!(ui.running_chat_id(), None);
    assert!(!ui.tab_has_background_running_dot(chat_a));
    assert!(!ui.tab_has_background_running_dot(chat_b));
    let (is_live, _) = ui.chat_activity_line();
    assert!(!is_live, "Chat line should not be animated when idle");
    println!("  [OK] Initial idle state verified: 0 running indicators present");

    // 2. Planning State (Director thinking)
    println!("\n2. Starting Task -> Planning State...");
    ui.task_state = TaskLifecycleState::Planning;
    ui.active_task_id = Some(chat_a.to_string());
    ui.elapsed_seconds = 3;
    assert!(ui.is_task_running(), "Task is running during planning");
    assert!(ui.topbar_running_indicator_present(), "TopBar running indicator MUST be present during planning");
    let (is_live, label) = ui.chat_activity_line();
    assert!(is_live, "Chat activity line MUST animate while planning");
    assert!(label.contains("Thinking..."), "Label must reflect Thinking: {}", label);
    println!("  [OK] Planning running state verified: TopBar active, Chat line animating ({})", label);

    // 3. Worker Running State (Reading/Writing/Executing)
    println!("\n3. Worker Executing -> Dynamic Action Labels...");
    ui.task_state = TaskLifecycleState::WorkerRunning { session_id: "sess_123".to_string() };
    
    // Sub-phase A: Reading files
    ui.live_action = Some("Reading src/main.rs...".to_string());
    ui.elapsed_seconds = 8;
    assert!(ui.topbar_running_indicator_present(), "TopBar running indicator active while reading");
    let (is_live, label) = ui.chat_activity_line();
    assert!(is_live);
    assert!(label.contains("Reading src/main.rs..."), "Expected reading action, got: {}", label);
    println!("  [OK] Worker dynamic action: {}", label);

    // Sub-phase B: Writing files
    ui.live_action = Some("Writing src/lib.rs...".to_string());
    ui.elapsed_seconds = 14;
    let (is_live, label) = ui.chat_activity_line();
    assert!(is_live);
    assert!(label.contains("Writing src/lib.rs..."), "Expected writing action, got: {}", label);
    println!("  [OK] Worker dynamic action: {}", label);

    // 4. Background Tab Running Dot
    println!("\n4. Verifying Background Tab Indicator...");
    // User switches view to chat_b while task runs on chat_a
    ui.active_chat_id = chat_b.to_string();
    assert!(
        ui.tab_has_background_running_dot(chat_a),
        "Background tab chat_a MUST display animated running dot while executing in background"
    );
    assert!(
        !ui.tab_has_background_running_dot(chat_b),
        "Currently focused tab chat_b should NOT display background running dot"
    );
    println!("  [OK] Background tab running dot confirmed active on unfocused running tab {}", chat_a);

    // 5. Completion State
    println!("\n5. Completing Task -> Transition to Idle & Reversion...");
    ui.active_chat_id = chat_a.to_string();
    ui.task_state = TaskLifecycleState::Complete;
    ui.active_task_id = None;
    ui.live_action = None;
    ui.elapsed_seconds = 125; // 2m 5s

    assert!(!ui.is_task_running(), "Task is no longer running");
    assert!(
        !ui.topbar_running_indicator_present(),
        "TopBar running indicator MUST be completely absent when task completes"
    );
    assert!(
        !ui.tab_has_background_running_dot(chat_a),
        "Background tab dot must be cleared on completion"
    );
    let (is_live, label) = ui.chat_activity_line();
    assert!(!is_live, "Chat line must revert to static when task completes");
    assert_eq!(label, "Worked for 2m", "Reverted to static duration label: {}", label);
    println!("  [OK] Completed state verified: TopBar indicator gone, Chat reverted to static '{}'", label);

    // 6. Summary Grounding Verification (Problem 1 verification)
    println!("\n6. Verifying Summary Grounding & Reconciliation Guard...");
    let fake_summary = "Modified src/lib.rs. Successfully restarted the dev server and all unit tests pass.";
    let files_modified = vec!["src/lib.rs".to_string()];
    let executed_commands = vec![]; // NO commands were actually executed!
    let unperformed_steps = vec!["Restart the dev server".to_string(), "Run cargo test".to_string()];

    let reconciled = verify_and_reconcile_summary(fake_summary, &files_modified, &executed_commands, &unperformed_steps);
    println!("  Reconciled Summary Output:\n{}", reconciled);

    assert!(reconciled.contains("Verification Notice:"));
    assert!(reconciled.contains("restarting the dev server, but no server restart command was executed"));
    assert!(reconciled.contains("running or verifying tests, but no test command was actually executed"));
    assert!(reconciled.contains("The following planned actions were not performed by the worker:"));
    println!("  [OK] Unverified claims and unperformed steps successfully flagged and reconciled!");

    println!("\n============================================================");
    println!("  ALL RUNNING INDICATOR & GROUNDING CHECKS PASSED (100%)    ");
    println!("============================================================");

    Ok(())
}

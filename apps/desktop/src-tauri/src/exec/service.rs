use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::process::Stdio;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command as TokioCommand;
use tokio::sync::Mutex;
use zencode_core::redact_secrets;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;

use super::job::ProcessJob;
use super::models::{
    BuildSystemType, CommandRequest, ExecutionResult, ProcessOutputEvent,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandSecurityClass {
    SafeAutoApproved,
    SessionRememberable,
    AlwaysRequireApproval,
}

pub struct ActiveProcess {
    pub command_id: String,
    pub program: String,
    pub args: Vec<String>,
    pub cwd: String,
    pub start_time: Instant,
    pub job: ProcessJob,
}

pub struct ExecutionService {
    pub pending_approvals: Mutex<HashMap<String, CommandRequest>>,
    pub active_processes: Mutex<HashMap<String, Arc<Mutex<ActiveProcess>>>>,
    pub execution_history: Mutex<Vec<ExecutionResult>>,
    pub session_approved_commands: Mutex<HashMap<String, HashSet<String>>>,
}

impl ExecutionService {
    pub fn new() -> Self {
        Self {
            pending_approvals: Mutex::new(HashMap::new()),
            active_processes: Mutex::new(HashMap::new()),
            execution_history: Mutex::new(Vec::new()),
            session_approved_commands: Mutex::new(HashMap::new()),
        }
    }

    /// Classifies a command for safe execution according to security policy:
    /// - Safe auto-approved: read-only status/diff and local build/check operations.
    /// - Session rememberable: approved once per task, remembered thereafter.
    /// - Always require approval: network-touching, publishing, package installation, or destructive commands.
    pub fn classify_command(program: &str, args: &[&str]) -> CommandSecurityClass {
        let prog_lower = program.to_lowercase();
        let prog_base = Path::new(&prog_lower)
            .file_name()
            .and_then(|f| f.to_str())
            .unwrap_or(&prog_lower);

        let full_cmd_lower = format!("{} {}", prog_base, args.join(" ")).to_lowercase();

        // 1. Destructive, network-touching, or package-installing commands ALWAYS require approval
        let dangerous_keywords = [
            "install", "publish", "push", "pull", "fetch", "clone",
            "remote", "checkout", "rebase", "reset", "clean -f",
            "rm", "delete", "drop", "del", "curl", "wget", "nc",
            "powershell", "cmd", "bash", "sh", "--release",
        ];

        for kw in dangerous_keywords {
            if full_cmd_lower.contains(kw) {
                return CommandSecurityClass::AlwaysRequireApproval;
            }
        }

        // 2. Safe auto-approved allowlist: common read-only & local test/build operations
        if (prog_base == "cargo" || prog_base == "cargo.exe") && !args.is_empty() {
            let sub = args[0];
            if sub == "check" || sub == "status" {
                return CommandSecurityClass::SafeAutoApproved;
            }
            if sub == "build" && !full_cmd_lower.contains("--release") {
                return CommandSecurityClass::SafeAutoApproved;
            }
            if sub == "test" {
                return CommandSecurityClass::SafeAutoApproved;
            }
        }

        if (prog_base == "git" || prog_base == "git.exe") && !args.is_empty() {
            let sub = args[0];
            if sub == "status" || sub == "diff" || sub == "log" || sub == "branch" {
                return CommandSecurityClass::SafeAutoApproved;
            }
        }

        if prog_base == "pytest" || prog_base == "pytest.exe" {
            return CommandSecurityClass::SafeAutoApproved;
        }

        if (prog_base == "go" || prog_base == "go.exe") && !args.is_empty() && (args[0] == "test" || args[0] == "vet") {
            return CommandSecurityClass::SafeAutoApproved;
        }

        if (prog_base == "tsc" || prog_base == "tsc.cmd" || prog_base == "tsc.exe")
            || (prog_base == "npx" || prog_base == "npx.cmd" || prog_base == "npx.exe") && args.iter().any(|a| a == &"tsc")
        {
            if args.iter().any(|a| a == &"--noEmit") {
                return CommandSecurityClass::SafeAutoApproved;
            }
        }

        if (prog_base == "python" || prog_base == "python3" || prog_base == "python.exe" || prog_base == "python3.exe")
            && args.len() >= 2
            && args[0] == "-m"
            && args[1] == "py_compile"
        {
            return CommandSecurityClass::SafeAutoApproved;
        }

        // Read-only discovery and inspection commands (e.g. `ollama list`, `docker ps`, `python --version`, etc.)
        let is_inspection = !args.is_empty()
            && args.iter().all(|a| {
                let al = a.to_lowercase();
                al == "list"
                    || al == "ps"
                    || al == "status"
                    || al == "--version"
                    || al == "-v"
                    || al == "version"
                    || al == "info"
                    || al == "images"
            });
        if is_inspection && (prog_base == "ollama" || prog_base == "docker" || prog_base == "python" || prog_base == "python3" || prog_base == "node" || prog_base == "pip" || prog_base == "npm") {
            return CommandSecurityClass::SafeAutoApproved;
        }

        // 3. Otherwise: rememberable for this session once approved
        CommandSecurityClass::SessionRememberable
    }

    pub fn command_signature(program: &str, args: &[&str]) -> String {
        format!("{}:{}", program.to_lowercase(), args.join(" ").to_lowercase())
    }

    /// Detect the build system used by the project in the given directory.
    pub fn detect_build_system(dir: &Path) -> BuildSystemType {
        if dir.join("Cargo.toml").exists() {
            BuildSystemType::Cargo
        } else if dir.join("pnpm-lock.yaml").exists() {
            BuildSystemType::Pnpm
        } else if dir.join("package-lock.json").exists() {
            BuildSystemType::Npm
        } else if dir.join("yarn.lock").exists() {
            BuildSystemType::Yarn
        } else if dir.join("package.json").exists() {
            BuildSystemType::Npm
        } else if dir.join("go.mod").exists() {
            BuildSystemType::Go
        } else if dir.join("pyproject.toml").exists() || dir.join("requirements.txt").exists() {
            BuildSystemType::Python
        } else {
            BuildSystemType::Unknown
        }
    }

    /// Creates and stages a command request requiring user approval.
    /// Automatically applies per-session memory and safe auto-approved allowlist.
    pub async fn create_approval_request(
        &self,
        program: &str,
        args: &[&str],
        cwd: &Path,
        description: &str,
        session_id: Option<&str>,
    ) -> CommandRequest {
        let sec_class = Self::classify_command(program, args);
        let sig = Self::command_signature(program, args);

        let auto_approved = match sec_class {
            CommandSecurityClass::SafeAutoApproved => true,
            CommandSecurityClass::SessionRememberable => {
                if let Some(sid) = session_id {
                    let map = self.session_approved_commands.lock().await;
                    map.get(sid).map(|set| set.contains(&sig)).unwrap_or(false)
                } else {
                    false
                }
            }
            CommandSecurityClass::AlwaysRequireApproval => false,
        };

        let id = format!(
            "cmd_{}_{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis(),
            rand::random::<u16>()
        );

        let req = CommandRequest {
            id: id.clone(),
            program: program.to_string(),
            args: args.iter().map(|s| s.to_string()).collect(),
            cwd: cwd.to_string_lossy().to_string(),
            description: description.to_string(),
            timeout_seconds: Some(180),
            auto_approved,
            security_class: match sec_class {
                CommandSecurityClass::SafeAutoApproved => "SafeAutoApproved".into(),
                CommandSecurityClass::SessionRememberable => "SessionRememberable".into(),
                CommandSecurityClass::AlwaysRequireApproval => "AlwaysRequireApproval".into(),
            },
        };

        let mut lock = self.pending_approvals.lock().await;
        lock.insert(id, req.clone());

        req
    }

    pub async fn remember_session_command(&self, session_id: &str, program: &str, args: &[&str]) {
        let sec_class = Self::classify_command(program, args);
        if sec_class != CommandSecurityClass::AlwaysRequireApproval {
            let sig = Self::command_signature(program, args);
            let mut map = self.session_approved_commands.lock().await;
            map.entry(session_id.to_string())
                .or_insert_with(HashSet::new)
                .insert(sig);
        }
    }

    /// Cancels a running command process and terminates its entire process tree,
    /// or cancels a pending approval request.
    pub async fn cancel_execution(&self, command_id: &str) -> bool {
        let process_opt = {
            let mut active = self.active_processes.lock().await;
            active.remove(command_id)
        };

        if let Some(proc_arc) = process_opt {
            let mut proc = proc_arc.lock().await;
            proc.job.terminate();
            true
        } else {
            let pending_opt = {
                let mut pending = self.pending_approvals.lock().await;
                pending.remove(command_id)
            };
            if let Some(req) = pending_opt {
                let result = ExecutionResult {
                    command_id: command_id.to_string(),
                    program: req.program,
                    args: req.args,
                    cwd: req.cwd,
                    exit_code: None,
                    success: false,
                    duration_ms: 0,
                    stdout_tail: Vec::new(),
                    stderr_tail: Vec::new(),
                    cancelled: true,
                    error_message: Some("Execution approval was denied or cancelled by the user.".into()),
                };
                let mut hist = self.execution_history.lock().await;
                hist.push(result);
                true
            } else {
                false
            }
        }
    }

    /// Waits for an approved or denied command to finish executing, up to `timeout_secs`.
    pub async fn wait_for_command_result(
        &self,
        command_id: &str,
        timeout_secs: u64,
    ) -> Result<ExecutionResult, String> {
        let start = Instant::now();
        let timeout = Duration::from_secs(timeout_secs);
        loop {
            // 1. Check if execution result is already recorded
            {
                let hist = self.execution_history.lock().await;
                if let Some(res) = hist.iter().find(|r| r.command_id == command_id) {
                    return Ok(res.clone());
                }
            }

            // 2. Check if still in pending approvals or active processes
            let is_pending = {
                let pending = self.pending_approvals.lock().await;
                pending.contains_key(command_id)
            };
            let is_active = {
                let active = self.active_processes.lock().await;
                active.contains_key(command_id)
            };

            if !is_pending && !is_active {
                // Not pending, not active, and not in history: check history one last time
                let hist = self.execution_history.lock().await;
                if let Some(res) = hist.iter().find(|r| r.command_id == command_id) {
                    return Ok(res.clone());
                }
                return Err("Command was denied or cancelled by user.".to_string());
            }

            if start.elapsed() > timeout {
                return Err(format!("Command approval or execution timed out after {timeout_secs}s"));
            }

            tokio::time::sleep(Duration::from_millis(200)).await;
        }
    }

    /// Executes an approved command with line-by-line streaming, output limits, and process tree supervision.
    pub async fn execute_command(
        &self,
        command_id: &str,
        app_handle: Option<&AppHandle>,
        redact_key: Option<&str>,
    ) -> Result<ExecutionResult, String> {
        // Take request from pending approvals (guarantees one-time use approval)
        let request = {
            let mut lock = self.pending_approvals.lock().await;
            lock.remove(command_id).ok_or_else(|| {
                format!("Command '{}' has not been approved or has already executed.", command_id)
            })?
        };

        let cwd_path = Path::new(&request.cwd);
        if !cwd_path.exists() {
            return Err(format!("Working directory does not exist: {}", request.cwd));
        }

        let start_time = Instant::now();
        let mut job = ProcessJob::new();

        // Spawn child process with piped stdout and stderr without flashing command windows on Windows
        let mut cmd = TokioCommand::new(&request.program);
        cmd.args(&request.args)
            .current_dir(cwd_path)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        #[cfg(windows)]
        cmd.creation_flags(CREATE_NO_WINDOW);

        let mut child = cmd
            .spawn()
            .map_err(|e| format!("Failed to spawn process '{}': {}", request.program, e))?;

        let pid = child.id().ok_or_else(|| "Failed to obtain process ID".to_string())?;
        job.assign_process(pid);

        // Register active process for cancellation
        let active_proc = Arc::new(Mutex::new(ActiveProcess {
            command_id: command_id.to_string(),
            program: request.program.clone(),
            args: request.args.clone(),
            cwd: request.cwd.clone(),
            start_time,
            job,
        }));

        {
            let mut active_map = self.active_processes.lock().await;
            active_map.insert(command_id.to_string(), active_proc.clone());
        }

        let stdout = child.stdout.take();
        let stderr = child.stderr.take();

        let stdout_tail = Arc::new(Mutex::new(Vec::<String>::new()));
        let stderr_tail = Arc::new(Mutex::new(Vec::<String>::new()));

        let cmd_id_stdout = command_id.to_string();
        let app_handle_stdout = app_handle.cloned();
        let stdout_tail_clone = stdout_tail.clone();
        let redact_key_stdout = redact_key.map(|s| s.to_string());

        // Spawn stdout streaming task
        let stdout_handle = tokio::spawn(async move {
            if let Some(out) = stdout {
                let mut reader = BufReader::new(out).lines();
                while let Ok(Some(line)) = reader.next_line().await {
                    let sanitized = redact_secrets(&line, redact_key_stdout.as_deref());
                    {
                        let mut tail = stdout_tail_clone.lock().await;
                        if tail.len() >= 1000 {
                            tail.remove(0);
                        }
                        tail.push(sanitized.clone());
                    }

                    if let Some(ref handle) = app_handle_stdout {
                        let _ = handle.emit(
                            "exec:output",
                            ProcessOutputEvent {
                                command_id: cmd_id_stdout.clone(),
                                stream: "stdout".into(),
                                line: sanitized,
                                timestamp_ms: SystemTime::now()
                                    .duration_since(UNIX_EPOCH)
                                    .unwrap_or_default()
                                    .as_millis() as u64,
                            },
                        );
                    }
                }
            }
        });

        let cmd_id_stderr = command_id.to_string();
        let app_handle_stderr = app_handle.cloned();
        let stderr_tail_clone = stderr_tail.clone();
        let redact_key_stderr = redact_key.map(|s| s.to_string());

        // Spawn stderr streaming task
        let stderr_handle = tokio::spawn(async move {
            if let Some(err) = stderr {
                let mut reader = BufReader::new(err).lines();
                while let Ok(Some(line)) = reader.next_line().await {
                    let sanitized = redact_secrets(&line, redact_key_stderr.as_deref());
                    {
                        let mut tail = stderr_tail_clone.lock().await;
                        if tail.len() >= 1000 {
                            tail.remove(0);
                        }
                        tail.push(sanitized.clone());
                    }

                    if let Some(ref handle) = app_handle_stderr {
                        let _ = handle.emit(
                            "exec:output",
                            ProcessOutputEvent {
                                command_id: cmd_id_stderr.clone(),
                                stream: "stderr".into(),
                                line: sanitized,
                                timestamp_ms: SystemTime::now()
                                    .duration_since(UNIX_EPOCH)
                                    .unwrap_or_default()
                                    .as_millis() as u64,
                            },
                        );
                    }
                }
            }
        });

        // Wait for child process exit or timeout
        let timeout_duration = Duration::from_secs(request.timeout_seconds.unwrap_or(300));
        let wait_result = tokio::time::timeout(timeout_duration, child.wait()).await;

        let _ = stdout_handle.await;
        let _ = stderr_handle.await;

        let duration_ms = start_time.elapsed().as_millis() as u64;

        // Clean up active process handle
        let was_cancelled = {
            let mut active_map = self.active_processes.lock().await;
            active_map.remove(command_id).is_none()
        };

        let result = match wait_result {
            Ok(Ok(status)) => {
                let exit_code = status.code();
                let success = status.success() && !was_cancelled;
                ExecutionResult {
                    command_id: command_id.to_string(),
                    program: request.program,
                    args: request.args,
                    cwd: request.cwd,
                    exit_code,
                    success,
                    duration_ms,
                    stdout_tail: stdout_tail.lock().await.clone(),
                    stderr_tail: stderr_tail.lock().await.clone(),
                    cancelled: was_cancelled,
                    error_message: if was_cancelled {
                        Some("Execution was cancelled by the user.".into())
                    } else if !status.success() {
                        Some(format!("Command exited with status code: {:?}", exit_code))
                    } else {
                        None
                    },
                }
            }
            Ok(Err(e)) => ExecutionResult {
                command_id: command_id.to_string(),
                program: request.program,
                args: request.args,
                cwd: request.cwd,
                exit_code: None,
                success: false,
                duration_ms,
                stdout_tail: stdout_tail.lock().await.clone(),
                stderr_tail: stderr_tail.lock().await.clone(),
                cancelled: was_cancelled,
                error_message: Some(format!("Process error: {}", e)),
            },
            Err(_) => {
                // Timeout occurred
                ProcessJob::kill_tree(pid);
                ExecutionResult {
                    command_id: command_id.to_string(),
                    program: request.program,
                    args: request.args,
                    cwd: request.cwd,
                    exit_code: None,
                    success: false,
                    duration_ms,
                    stdout_tail: stdout_tail.lock().await.clone(),
                    stderr_tail: stderr_tail.lock().await.clone(),
                    cancelled: false,
                    error_message: Some(format!("Command execution timed out after {}s", timeout_duration.as_secs())),
                }
            }
        };

        // Record in execution history
        {
            let mut hist = self.execution_history.lock().await;
            hist.push(result.clone());
        }

        // Emit finished event
        if let Some(handle) = app_handle {
            let _ = handle.emit("exec:finished", &result);
        }

        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_classify_command_security_classes() {
        assert_eq!(
            ExecutionService::classify_command("cargo", &["check", "--message-format=short"]),
            CommandSecurityClass::SafeAutoApproved
        );
        assert_eq!(
            ExecutionService::classify_command("npx", &["tsc", "--noEmit"]),
            CommandSecurityClass::SafeAutoApproved
        );
        assert_eq!(
            ExecutionService::classify_command("npx.cmd", &["tsc", "--noEmit"]),
            CommandSecurityClass::SafeAutoApproved
        );
        assert_eq!(
            ExecutionService::classify_command("go", &["vet", "./..."]),
            CommandSecurityClass::SafeAutoApproved
        );
        assert_eq!(
            ExecutionService::classify_command("python", &["-m", "py_compile", "main.py"]),
            CommandSecurityClass::SafeAutoApproved
        );
        assert_eq!(
            ExecutionService::classify_command("pip", &["install", "requests"]),
            CommandSecurityClass::AlwaysRequireApproval
        );
        assert_eq!(
            ExecutionService::classify_command("cargo", &["publish"]),
            CommandSecurityClass::AlwaysRequireApproval
        );
    }
}

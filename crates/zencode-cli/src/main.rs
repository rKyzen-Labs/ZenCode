use clap::{Parser, Subcommand};
use colored::*;
use dialoguer::{Confirm, Input, Password};
use indicatif::{ProgressBar, ProgressStyle};
use std::path::PathBuf;
use zencode_core::{
    CredentialService, GeminiPlannerClient, GitService, PlanningContext,
};

mod worker;
use worker::CliWorkerClient;

#[derive(Parser)]
#[command(name = "zencode")]
#[command(author = "ZenCode Team")]
#[command(version = "0.1.0")]
#[command(about = "ZenCode CLI — Sovereign, free-first AI coding engine for terminal workflows", long_about = None)]
struct Cli {
    /// Task description or prompt to execute
    #[arg(value_name = "PROMPT")]
    prompt: Option<String>,

    /// Planning model to use (gemini-3.5-flash-lite, gemini-3.8-flash, gemini-4-hydrogen)
    #[arg(long, default_value = "gemini-3.5-flash-lite")]
    planner: String,

    /// Worker provider engine: groq, openrouter, or local (ollama)
    #[arg(long, default_value = "groq")]
    worker: String,

    /// Custom model slug for worker execution
    #[arg(long)]
    model: Option<String>,

    /// Automatically accept diffs and merge without interactive prompt
    #[arg(short = 'y', long)]
    yes: bool,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand, Clone)]
enum Commands {
    /// Manage API keys and provider authentication
    Auth {
        #[command(subcommand)]
        action: AuthCommands,
    },
    /// Inspect or clear temporary task worktrees
    Worktree {
        /// List all active and orphaned task worktrees
        #[arg(long)]
        list: bool,
    },
    /// Launch interactive REPL session
    Chat,
}

#[derive(Subcommand, Clone)]
enum AuthCommands {
    /// Check authentication status of all providers
    Status,
    /// Store an API key into Windows Credential Manager
    Login {
        /// Provider to login (gemini, groq, openrouter)
        provider: String,
    },
    /// Remove an API key from Windows Credential Manager
    Logout {
        /// Provider to logout (gemini, groq, openrouter)
        provider: String,
    },
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    print_banner();

    if let Some(cmd) = cli.command.clone() {
        match cmd {
            Commands::Auth { action } => handle_auth_command(action).await,
            Commands::Worktree { list: _ } => handle_worktree_command().await,
            Commands::Chat => handle_chat_repl(&cli).await,
        }
        return;
    }

    if let Some(ref prompt) = cli.prompt {
        execute_task_flow(prompt, &cli).await;
    } else {
        handle_chat_repl(&cli).await;
    }
}

fn print_banner() {
    println!("{}", "==========================================================".truecolor(201, 100, 66));
    println!("{}", "               ZENCODE AUTONOMOUS CLI                     ".bold().truecolor(250, 249, 245));
    println!("{}", "        Free-First, Sovereign AI Coding Engine            ".truecolor(180, 175, 160));
    println!("{}\n", "==========================================================".truecolor(201, 100, 66));
}

async fn handle_auth_command(action: AuthCommands) {
    match action {
        AuthCommands::Status => {
            println!("{}", "Checking Windows Credential Vault:".bold());
            print_provider_status("Gemini (Planner)", CredentialService::get_gemini_api_key());
            print_provider_status("Groq Cloud (Worker)", CredentialService::get_groq_api_key());
            print_provider_status("OpenRouter (Worker)", CredentialService::get_openrouter_api_key());
        }
        AuthCommands::Login { provider } => {
            let key = Password::new()
                .with_prompt(format!("Enter API key for {}", provider))
                .interact()
                .unwrap_or_default();

            if key.trim().is_empty() {
                println!("{}", "Error: Key cannot be empty.".red());
                return;
            }

            match provider.to_lowercase().as_str() {
                "gemini" => {
                    if let Err(e) = CredentialService::set_gemini_api_key(&key) {
                        println!("{} {}", "Failed to save key:".red(), e);
                    } else {
                        println!("{}", "Gemini API key saved to Windows Credential Manager.".green());
                    }
                }
                "groq" => {
                    if let Err(e) = CredentialService::set_groq_api_key(&key) {
                        println!("{} {}", "Failed to save key:".red(), e);
                    } else {
                        println!("{}", "Groq API key saved to Windows Credential Manager.".green());
                    }
                }
                "openrouter" => {
                    if let Err(e) = CredentialService::set_openrouter_api_key(&key) {
                        println!("{} {}", "Failed to save key:".red(), e);
                    } else {
                        println!("{}", "OpenRouter API key saved to Windows Credential Manager.".green());
                    }
                }
                other => {
                    println!("{} Unknown provider '{}'. Use gemini, groq, or openrouter.", "Error:".red(), other);
                }
            }
        }
        AuthCommands::Logout { provider } => {
            match provider.to_lowercase().as_str() {
                "gemini" => {
                    let _ = CredentialService::delete_gemini_api_key();
                    println!("{}", "Gemini key removed.".yellow());
                }
                "groq" => {
                    let _ = CredentialService::delete_groq_api_key();
                    println!("{}", "Groq key removed.".yellow());
                }
                "openrouter" => {
                    let _ = CredentialService::delete_openrouter_api_key();
                    println!("{}", "OpenRouter key removed.".yellow());
                }
                other => {
                    println!("{} Unknown provider '{}'.", "Error:".red(), other);
                }
            }
        }
    }
}

fn print_provider_status(label: &str, res: Result<String, zencode_core::SecurityError>) {
    match res {
        Ok(k) if !k.is_empty() => {
            let masked = if k.len() > 8 {
                format!("{}••••{}", &k[..4], &k[k.len() - 4..])
            } else {
                "••••••••".into()
            };
            println!("  • {:<22} [ {} ] ({})", label, "CONFIGURED".green().bold(), masked.cyan());
        }
        _ => {
            println!("  • {:<22} [ {} ]", label, "NOT CONFIGURED".red());
        }
    }
}

async fn handle_worktree_command() {
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    match GitService::is_git_repo(&cwd) {
        Ok(true) => {
            let active = GitService::list_active_worktrees(&cwd).unwrap_or_default();
            println!("{}", format!("Active worktrees for {}:", cwd.display()).bold());
            if active.is_empty() {
                println!("  No active task worktrees.");
            } else {
                for wt in active {
                    println!("  • Path: {}", wt.worktree_path.cyan());
                    println!("    Branch: {}", wt.branch_name.yellow());
                }
            }
        }
        _ => {
            println!("{}", "Current directory is not a Git repository.".red());
        }
    }
}

async fn handle_chat_repl(cli: &Cli) {
    println!("{}", "Interactive ZenCode Terminal Session. Type 'exit' or 'quit' to end.\n".cyan());
    loop {
        let input: String = Input::new()
            .with_prompt("zencode")
            .interact_text()
            .unwrap_or_default();

        let trimmed = input.trim();
        if trimmed.is_empty() {
            continue;
        }
        if trimmed.eq_ignore_ascii_case("exit") || trimmed.eq_ignore_ascii_case("quit") {
            println!("{}", "Exiting ZenCode CLI. Goodbye!".truecolor(201, 100, 66));
            break;
        }

        execute_task_flow(trimmed, cli).await;
        println!();
    }
}

async fn execute_task_flow(prompt: &str, cli: &Cli) {
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    
    // 1. Verify Git Repository
    let is_git = GitService::is_git_repo(&cwd).unwrap_or(false);
    if !is_git {
        println!("{}", "Initializing Git repository for safe worktree diff management...".yellow());
        if let Err(e) = GitService::init_repo(&cwd) {
            println!("{} {}", "Failed to initialize Git repository:".red(), e);
            return;
        }
    }

    // 2. Planning Phase with Gemini Director
    let gemini_key = match CredentialService::get_gemini_api_key() {
        Ok(k) if !k.is_empty() => k,
        _ => {
            println!("{}", "Missing Gemini API Key. Run 'zencode auth login gemini' to configure.".red().bold());
            return;
        }
    };

    let spinner = ProgressBar::new_spinner();
    spinner.set_style(ProgressStyle::default_spinner().template("{spinner:.green} {msg}").unwrap());
    spinner.set_message(format!("Planning task with Gemini Director ({})...", cli.planner));
    spinner.enable_steady_tick(std::time::Duration::from_millis(80));

    let planner_client = GeminiPlannerClient::with_model(&cli.planner);
    let context = PlanningContext {
        task_description: prompt.to_string(),
        project_type: "CLI Workspace".to_string(),
        shallow_tree: vec![],
        manifest_summaries: vec![],
        multimodal_attachments: vec![],
        conversation_history: vec![],
        approved_source_excerpts: vec![],
        constraints: vec![],
        git_branch_and_status: String::new(),
    };

    let (plan, _usage) = match planner_client.plan(&gemini_key, &context).await {
        Ok(res) => {
            spinner.finish_with_message("Plan generated successfully.".green().to_string());
            res
        }
        Err(e) => {
            spinner.finish_with_message(format!("Planning failed: {e}").red().to_string());
            return;
        }
    };

    println!("\n{}", "================ PLAN SUMMARY ================".bold());
    println!("{}\n", plan.task_summary.truecolor(250, 249, 245));
    println!("{}", "Steps:".bold());
    for (i, step) in plan.steps.iter().enumerate() {
        println!("  {}. {}", i + 1, step.goal.cyan());
        for inst in &step.instructions {
            println!("     - {}", inst);
        }
    }
    println!("{}\n", "==============================================".bold());

    // 3. Create Isolated Worktree
    let current_branch = GitService::get_current_branch(&cwd).unwrap_or_else(|_| "main".to_string());
    let task_id = format!("cli_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis());
    let worktree_info = match GitService::create_task_worktree(&cwd, &task_id, true) {
        Ok(info) => info,
        Err(e) => {
            println!("{} Failed to create isolated worktree: {}", "Error:".red(), e);
            return;
        }
    };

    let worktree_path = PathBuf::from(&worktree_info.worktree_path);
    println!("{} Created isolated worktree: {}\n", "✓".green(), worktree_info.branch_name.yellow());

    // 4. Instantiate Worker Client
    let worker_client = match cli.worker.to_lowercase().as_str() {
        "groq" => {
            let key = match CredentialService::get_groq_api_key() {
                Ok(k) if !k.is_empty() => k,
                _ => {
                    println!("{}", "Missing Groq API Key. Run 'zencode auth login groq'.".red());
                    let _ = GitService::reject_task_changes(&worktree_path, &task_id);
                    return;
                }
            };
            CliWorkerClient::new_groq(key, cli.model.clone())
        }
        "openrouter" => {
            let key = match CredentialService::get_openrouter_api_key() {
                Ok(k) if !k.is_empty() => k,
                _ => {
                    println!("{}", "Missing OpenRouter API Key. Run 'zencode auth login openrouter'.".red());
                    let _ = GitService::reject_task_changes(&worktree_path, &task_id);
                    return;
                }
            };
            CliWorkerClient::new_openrouter(key, cli.model.clone())
        }
        "local" | "ollama" => CliWorkerClient::new_ollama(cli.model.clone()),
        other => {
            println!("{} Unknown worker provider '{}'. Use groq, openrouter, or local.", "Error:".red(), other);
            let _ = GitService::reject_task_changes(&worktree_path, &task_id);
            return;
        }
    };

    // 5. Execute Code Generation
    let worker_spinner = ProgressBar::new_spinner();
    worker_spinner.set_style(ProgressStyle::default_spinner().template("{spinner:.cyan} {msg}").unwrap());
    worker_spinner.set_message(format!("Worker generating code with {:?} ({}) ...", worker_client.provider, worker_client.model));
    worker_spinner.enable_steady_tick(std::time::Duration::from_millis(80));

    let diff_results = match worker_client.execute_task(&worktree_path, &plan.task_summary, &plan.steps).await {
        Ok(res) => {
            worker_spinner.finish_with_message("Code generation complete!".green().to_string());
            res
        }
        Err(e) => {
            worker_spinner.finish_with_message(format!("Execution failed: {e}").red().to_string());
            eprintln!("\n{} Worker error detail: {}\n", "ERROR:".red().bold(), e);
            let _ = GitService::reject_task_changes(&worktree_path, &task_id);
            return;
        }
    };

    // 6. Review Changes
    println!("\n{}", "================ APPLIED CHANGES ================".bold().green());
    for diff in &diff_results {
        println!("  • {} (+{} lines, -{} lines) {}", 
            diff.file_path.bold(), 
            diff.additions.to_string().green(), 
            diff.deletions.to_string().red(),
            if diff.is_new { "[NEW]".yellow() } else { "".normal() }
        );
    }
    println!("{}\n", "=================================================".bold().green());

    // 7. Interactive Review & Merge
    let should_merge = if cli.yes {
        true
    } else {
        Confirm::new()
            .with_prompt("Accept and merge changes into your working directory?")
            .default(true)
            .interact()
            .unwrap_or(false)
    };

    if should_merge {
        match GitService::accept_task_changes(&worktree_path, &current_branch, true) {
            Ok(merge_res) => {
                match merge_res {
                    zencode_core::git::MergeResult::Success { commit_sha } => {
                        println!("\n{} Changes merged into main branch successfully!", "SUCCESS:".green().bold());
                        println!("Commit Hash: {}", commit_sha.cyan());

                        // 8. Grounded Summary Generation
                        let summary_spinner = ProgressBar::new_spinner();
                        summary_spinner.set_style(ProgressStyle::default_spinner().template("{spinner:.cyan} {msg}").unwrap());
                        summary_spinner.set_message("Generating and verifying grounded task summary...".to_string());
                        summary_spinner.enable_steady_tick(std::time::Duration::from_millis(80));

                        let mut files_summary = String::new();
                        let mut modified_file_paths = Vec::new();
                        for d in &diff_results {
                            files_summary.push_str(&format!("- {} (+{} lines, -{} lines)\n", d.file_path, d.additions, d.deletions));
                            modified_file_paths.push(d.file_path.clone());
                        }
                        let diff_excerpts = format!("Total {} file(s) modified across worktree.", diff_results.len());

                        let (raw_summary, _usage) = match planner_client
                            .summarize_task_outcome_grounded(
                                &gemini_key,
                                &plan.task_summary,
                                &files_summary,
                                &diff_excerpts,
                                "",
                                "",
                            )
                            .await
                        {
                            Ok((sum, usage)) => {
                                summary_spinner.finish_and_clear();
                                (sum, usage)
                            }
                            Err(_) => {
                                summary_spinner.finish_and_clear();
                                (
                                    format!(
                                        "Successfully applied changes across {} file(s) (+{}/-{} lines).",
                                        diff_results.len(),
                                        diff_results.iter().map(|d| d.additions).sum::<usize>(),
                                        diff_results.iter().map(|d| d.deletions).sum::<usize>()
                                    ),
                                    Default::default(),
                                )
                            }
                        };

                        let verified_summary = zencode_core::verify_and_reconcile_summary(
                            &raw_summary,
                            &modified_file_paths,
                            &[],
                            &[],
                        );

                        println!("\n{}", "================ TASK SUMMARY ================".bold().cyan());
                        println!("{}", verified_summary.trim());
                        println!("{}\n", "==============================================".bold().cyan());
                    }
                    zencode_core::git::MergeResult::Conflict { conflicting_files } => {
                        println!("\n{} Merge conflicts encountered in files: {:?}", "CONFLICT:".yellow().bold(), conflicting_files);
                    }
                }
            }
            Err(e) => {
                println!("{} Merge failed: {}", "Error:".red(), e);
            }
        }
    } else {
        println!("{}", "Changes rejected. Cleaning up isolated worktree...".yellow());
        let _ = GitService::reject_task_changes(&worktree_path, &task_id);
        println!("{}", "Worktree safely removed without touching main directory.".green());
    }
}

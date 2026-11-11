use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum PlannerError {
    #[error("API request failed with status {status}: {message}")]
    ApiError { status: u16, message: String },
    #[error("Plan JSON deserialization failed: {0}")]
    JsonError(#[from] serde_json::Error),
    #[error("Plan validation failed: {0}")]
    ValidationError(String),
    #[error("Planning context contains forbidden secret path: {0}")]
    SecretPathForbidden(String),
    #[error("Gemini quota or rate limit exceeded: {0}")]
    QuotaExhausted(String),
    #[error("Reqwest network error: {0}")]
    NetworkError(#[from] reqwest::Error),
}

fn default_true() -> bool {
    true
}

/// The exact JSON schema structure required by the locked Zencode V1 spec.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImplementationPlan {
    #[serde(default = "default_true")]
    pub is_actionable: bool,
    #[serde(default)]
    pub conversational_reply: Option<String>,
    pub task_summary: String,
    pub assumptions: Vec<String>,
    pub questions: Vec<String>,
    pub project_findings: Vec<String>,
    pub steps: Vec<PlanStep>,
    pub risks: Vec<String>,
    pub verification_suggestions: Vec<String>,
    #[serde(default)]
    pub planning_duration_seconds: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlanStep {
    pub id: String,
    pub goal: String,
    pub likely_files: Vec<String>,
    pub instructions: Vec<String>,
    pub depends_on: Vec<String>,
    pub acceptance_checks: Vec<String>,
}

impl ImplementationPlan {
    /// Validates untrusted model output against Zencode plan sanity rules.
    pub fn validate(&self) -> Result<(), PlannerError> {
        if !self.is_actionable {
            // Conversational message: steps must be empty, conversational_reply or task_summary must not be empty
            if self.conversational_reply.as_ref().map_or(true, |r| r.trim().is_empty())
                && self.task_summary.trim().is_empty()
            {
                return Err(PlannerError::ValidationError(
                    "conversational_reply or task_summary cannot be empty for non-actionable message".into(),
                ));
            }
            return Ok(());
        }

        if self.task_summary.trim().is_empty() {
            return Err(PlannerError::ValidationError(
                "task_summary cannot be empty".into(),
            ));
        }
        if self.steps.is_empty() {
            return Err(PlannerError::ValidationError(
                "actionable plan must contain at least one step".into(),
            ));
        }
        for (i, step) in self.steps.iter().enumerate() {
            if step.id.trim().is_empty() {
                return Err(PlannerError::ValidationError(format!(
                    "step[{i}] id cannot be empty"
                )));
            }
            if step.goal.trim().is_empty() {
                return Err(PlannerError::ValidationError(format!(
                    "step[{i}] goal cannot be empty"
                )));
            }
        }
        Ok(())
    }
}

/// Token usage metadata from the Gemini API.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct UsageMetadata {
    pub prompt_token_count: u32,
    pub candidates_token_count: u32,
    pub total_token_count: u32,
    pub service_tier: Option<String>,
}

/// Prior turn in an ongoing chat conversation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PriorExchange {
    pub role: String, // "user" | "assistant"
    pub content: String,
    #[serde(default)]
    pub files_changed: Vec<String>,
    #[serde(default)]
    pub summary: Option<String>,
}

/// Multimodal attachment (PDF, image, etc.) sent inline to Gemini.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MultimodalAttachment {
    pub file_name: String,
    pub mime_type: String,
    pub base64_data: String,
}

impl MultimodalAttachment {
    pub fn new(file_name: impl Into<String>, mime_type: impl Into<String>, base64_data: impl Into<String>) -> Self {
        Self {
            file_name: file_name.into(),
            mime_type: mime_type.into(),
            base64_data: base64_data.into(),
        }
    }

    pub fn from_bytes(file_name: &str, mime_type: &str, bytes: &[u8]) -> Self {
        use base64::prelude::*;
        Self {
            file_name: file_name.to_string(),
            mime_type: mime_type.to_string(),
            base64_data: BASE64_STANDARD.encode(bytes),
        }
    }

    pub fn from_file<P: AsRef<std::path::Path>>(path: P) -> Result<Self, std::io::Error> {
        let path = path.as_ref();
        let file_name = path
            .file_name()
            .map(|f| f.to_string_lossy().to_string())
            .unwrap_or_else(|| "attachment".to_string());
        let mime_type = detect_mime_type(path);
        let bytes = std::fs::read(path)?;
        Ok(Self::from_bytes(&file_name, &mime_type, &bytes))
    }
}

pub fn detect_mime_type(path: &std::path::Path) -> String {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    match ext.as_str() {
        "pdf" => "application/pdf".to_string(),
        "png" => "image/png".to_string(),
        "jpg" | "jpeg" => "image/jpeg".to_string(),
        "webp" => "image/webp".to_string(),
        "gif" => "image/gif".to_string(),
        "svg" => "image/svg+xml".to_string(),
        "bmp" => "image/bmp".to_string(),
        "ico" => "image/x-icon".to_string(),
        "tiff" | "tif" => "image/tiff".to_string(),
        _ => "application/octet-stream".to_string(),
    }
}

pub fn is_multimodal_file(path: &std::path::Path) -> bool {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    matches!(
        ext.as_str(),
        "pdf" | "png" | "jpg" | "jpeg" | "webp" | "gif" | "svg" | "bmp" | "ico" | "tiff" | "tif"
    )
}

/// Planning context assembled locally before being presented to the user.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanningContext {
    pub task_description: String,
    pub project_type: String,
    pub shallow_tree: Vec<String>,
    pub manifest_summaries: Vec<String>,
    pub git_branch_and_status: String,
    pub approved_source_excerpts: Vec<(String, String)>,
    pub constraints: Vec<String>,
    #[serde(default)]
    pub conversation_history: Vec<PriorExchange>,
    #[serde(default)]
    pub multimodal_attachments: Vec<MultimodalAttachment>,
}

impl Default for PlanningContext {
    fn default() -> Self {
        Self {
            task_description: String::new(),
            project_type: String::new(),
            shallow_tree: Vec::new(),
            manifest_summaries: Vec::new(),
            git_branch_and_status: String::new(),
            approved_source_excerpts: Vec::new(),
            constraints: Vec::new(),
            conversation_history: Vec::new(),
            multimodal_attachments: Vec::new(),
        }
    }
}

const FORBIDDEN_SECRET_PATTERNS: &[&str] = &[
    ".env",
    "id_rsa",
    "id_ed25519",
    ".pem",
    ".key",
    ".p12",
    ".pfx",
    "credentials",
    "token",
    "secret",
];

impl PlanningContext {
    /// Validates that no secret paths or sensitive files are included in the upload package.
    pub fn assert_no_secrets(&self) -> Result<(), PlannerError> {
        for path in &self.shallow_tree {
            let lower = path.to_lowercase();
            for pattern in FORBIDDEN_SECRET_PATTERNS {
                if lower.contains(pattern) {
                    return Err(PlannerError::SecretPathForbidden(path.clone()));
                }
            }
        }
        for (path, _) in &self.approved_source_excerpts {
            let lower = path.to_lowercase();
            for pattern in FORBIDDEN_SECRET_PATTERNS {
                if lower.contains(pattern) {
                    return Err(PlannerError::SecretPathForbidden(path.clone()));
                }
            }
        }
        Ok(())
    }

    /// Formats the context into the user prompt for Gemini.
    pub fn to_prompt_text(&self) -> String {
        let mut out = String::new();

        if !self.conversation_history.is_empty() {
            out.push_str("PRIOR CONVERSATION & ACTIONS IN THIS CHAT:\n");
            for ex in &self.conversation_history {
                if ex.role == "user" {
                    out.push_str(&format!("User: {}\n", ex.content));
                } else {
                    out.push_str(&format!("Assistant: {}\n", ex.content));
                    if !ex.files_changed.is_empty() {
                        out.push_str(&format!("  Files created/modified: {}\n", ex.files_changed.join(", ")));
                    }
                    if let Some(ref s) = ex.summary {
                        out.push_str(&format!("  Outcome: {}\n", s));
                    }
                }
            }
            out.push_str("\nCURRENT FOLLOW-UP REQUEST:\n");
        } else {
            out.push_str("TASK DESCRIPTION:\n");
        }

        out.push_str(&format!("{}\n\n", self.task_description));
        out.push_str(&format!("PROJECT TYPE:\n{}\n\n", self.project_type));
        out.push_str(&format!("GIT STATUS:\n{}\n\n", self.git_branch_and_status));

        out.push_str("PROJECT TREE:\n");
        for file in &self.shallow_tree {
            out.push_str(&format!("- {}\n", file));
        }
        out.push('\n');

        if !self.manifest_summaries.is_empty() {
            out.push_str("MANIFEST / DEPENDENCY SUMMARIES:\n");
            for m in &self.manifest_summaries {
                out.push_str(&format!("{}\n", m));
            }
            out.push('\n');
        }

        if !self.approved_source_excerpts.is_empty() {
            out.push_str("APPROVED SOURCE EXCERPTS:\n");
            for (path, content) in &self.approved_source_excerpts {
                out.push_str(&format!("--- File: {} ---\n{}\n\n", path, content));
            }
        }

        if !self.constraints.is_empty() {
            out.push_str("CONSTRAINTS & ACCEPTANCE GOALS:\n");
            for c in &self.constraints {
                out.push_str(&format!("- {}\n", c));
            }
            out.push('\n');
        }

        if !self.multimodal_attachments.is_empty() {
            out.push_str("ATTACHED VISUAL & MULTIMODAL SPECIFICATIONS:\n");
            for att in &self.multimodal_attachments {
                out.push_str(&format!("- [{}] (Type: {})\n", att.file_name, att.mime_type));
            }
            out.push_str("Note: The visual/multimodal files above are attached directly as inline data parts in this request. Visually inspect their contents, design, layout, typography, colors, sections, and structure when planning the implementation.\n\n");
        }

        out.push_str("CLASSIFICATION & ROUTING INSTRUCTION:\n");
        out.push_str("1. If PRIOR CONVERSATION exists, evaluate the CURRENT FOLLOW-UP REQUEST in the full context of what was previously requested, created, or modified. If the user refers back to previous files, functions, or concepts (e.g. 'also add input validation to that function', 'now write tests for it'), link directly to the relevant files and functions from prior turns.\n");
        out.push_str("2. READ-ONLY EXPLORATION & ARCHITECTURAL ANALYSIS: When the user asks to explore, analyze, overview, review, or explain the project, codebase, architecture, or an attached document/PDF/design (e.g. 'explore this project', 'explain the codebase', 'overview of this repo', 'summarize this pdf') and does NOT request writing code, editing/creating/deleting files, installing packages, or running commands:\n");
        out.push_str("   - Set is_actionable: false.\n");
        out.push_str("   - Leave steps as an empty array [].\n");
        out.push_str("   - Answer the user's exploration request DIRECTLY, THOROUGHLY, and COMPREHENSIVELY in conversational_reply, analyzing the PROJECT TREE, MANIFEST SUMMARIES, or ATTACHED VISUAL/PDF SPECIFICATIONS provided above. Explain the architecture, directory layout, design, and structure in rich, structured Markdown.\n");
        out.push_str("   - Do NOT dispatch to a worker or formulate coding steps; exploration is answered directly by you.\n");
        out.push_str("3. GENERAL CONVERSATION & GREETINGS: For pure greetings (like 'hi', 'hello', 'thanks') or general questions without project action: set is_actionable: false, set steps: [], and provide a direct helpful response in conversational_reply.\n");
        out.push_str("4. DYNAMIC ENVIRONMENT DISCOVERY VIA COMMANDS: If the user explicitly asks about the host machine, installed runtimes, models, or command outputs (e.g. 'what models are installed in ollama', 'check git branch', 'what version of python do I have', 'run pip install requests'): set is_actionable: true, set conversational_reply: null, and formulate a step with the exact terminal command (`COMMAND: ollama list`, `COMMAND: python --version`, etc.).\n");
        out.push_str("5. ACTIONABLE CODING & REFACTORING TASKS: If the user asks to build, develop, write code, modify files, create files, delete files, move folders, or implement features (e.g. 'build', 'build it', 'develop', 'make a website', 'code this', 'implement'): set is_actionable: true, set conversational_reply: null, and provide a concrete task_summary with detailed implementation steps in the steps array. In the instructions for each step, embed complete, explicit, high-fidelity structural specifications (exact sections, component names, styling rules, color palettes, and logic) so the worker produces complete production-grade code rather than minimal stubs.\n");
        out.push_str("6. MULTIMODAL & PDF/IMAGE-BASED SPECIFICATIONS: If the user provides an attached design mock, wireframe, or PDF specification (e.g. 'develop me website based on this pdf', 'build this based on the pdf', 'create landing page from design'): set is_actionable: true, set conversational_reply: null, and thoroughly analyze the visual structure, layout, typography, components, color scheme, sections, and functional requirements from the attached visual/PDF parts. Formulate concrete, detailed implementation steps that embed the exact visual specifications (e.g. exact headings, copy text, all layout sections, color hex codes, font families, terminal simulations, comparison charts) into each step's instructions so the worker builds a pixel-faithful, complete, production-grade application without cutting corners.\n");

        out
    }
}

/// The official locked Planner Client calling Gemini 3.5 Flash-Lite.
pub struct GeminiPlannerClient {
    client: reqwest::Client,
    model: String,
}

impl GeminiPlannerClient {
    pub const DEFAULT_MODEL: &'static str = "gemini-3.5-flash-lite";

    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
            model: Self::DEFAULT_MODEL.to_string(),
        }
    }

    pub fn with_model(model: &str) -> Self {
        Self {
            client: reqwest::Client::new(),
            model: model.to_string(),
        }
    }

    /// Request schema matching Zencode spec
    pub fn plan_json_schema() -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "is_actionable": {
                    "type": "boolean",
                    "description": "True ONLY for actionable coding tasks (writing/modifying files, creating/deleting files, moving folders, running commands). False for greetings, small talk, AND read-only exploration or architectural analysis queries (e.g. 'explore this project', 'explain the codebase', 'overview of this repo')."
                },
                "conversational_reply": {
                    "type": "string",
                    "description": "If is_actionable is false, provide a direct, comprehensive, helpful answer to the user here. For project exploration or analysis, provide a thorough, structured architectural overview reviewing the project tree and key components. If is_actionable is true, leave empty or null."
                },
                "task_summary": { "type": "string" },
                "assumptions": { "type": "array", "items": { "type": "string" } },
                "questions": { "type": "array", "items": { "type": "string" } },
                "project_findings": { "type": "array", "items": { "type": "string" } },
                "steps": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "id": { "type": "string" },
                            "goal": { "type": "string" },
                            "likely_files": { "type": "array", "items": { "type": "string" } },
                            "instructions": { "type": "array", "items": { "type": "string" } },
                            "depends_on": { "type": "array", "items": { "type": "string" } },
                            "acceptance_checks": { "type": "array", "items": { "type": "string" } }
                        },
                        "required": ["id", "goal", "likely_files", "instructions", "depends_on", "acceptance_checks"]
                    }
                },
                "risks": { "type": "array", "items": { "type": "string" } },
                "verification_suggestions": { "type": "array", "items": { "type": "string" } }
            },
            "required": [
                "is_actionable",
                "task_summary",
                "assumptions",
                "questions",
                "project_findings",
                "steps",
                "risks",
                "verification_suggestions"
            ]
        })
    }

    /// Executes the single explicit planning request to Gemini.
    pub async fn plan(
        &self,
        api_key: &str,
        context: &PlanningContext,
    ) -> Result<(ImplementationPlan, UsageMetadata), PlannerError> {
        context.assert_no_secrets()?;

        let prompt_text = context.to_prompt_text();

        let mut parts = vec![
            serde_json::json!({ "text": prompt_text })
        ];

        for attachment in &context.multimodal_attachments {
            parts.push(serde_json::json!({
                "inlineData": {
                    "mimeType": attachment.mime_type,
                    "data": attachment.base64_data
                }
            }));
        }

        let request_body = serde_json::json!({
            "contents": [
                {
                    "role": "user",
                    "parts": parts
                }
            ],
            "generationConfig": {
                "responseMimeType": "application/json",
                "responseSchema": Self::plan_json_schema()
            }
        });

        let url = format!(
            "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent?key={}",
            self.model, api_key
        );

        let response = self
            .client
            .post(&url)
            .json(&request_body)
            .send()
            .await?;

        let status = response.status();
        if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
            let error_text = response.text().await.unwrap_or_default();
            return Err(PlannerError::QuotaExhausted(error_text));
        }

        if !status.is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(PlannerError::ApiError {
                status: status.as_u16(),
                message: error_text,
            });
        }

        let body_json: serde_json::Value = response.json().await?;

        // Extract usage metadata if present
        let mut usage = UsageMetadata::default();
        if let Some(meta) = body_json.get("usageMetadata") {
            usage.prompt_token_count = meta
                .get("promptTokenCount")
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as u32;
            usage.candidates_token_count = meta
                .get("candidatesTokenCount")
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as u32;
            usage.total_token_count = meta
                .get("totalTokenCount")
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as u32;
            usage.service_tier = meta
                .get("serviceTier")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
        }

        // Extract candidate text
        let candidate_text = body_json
            .get("candidates")
            .and_then(|c| c.get(0))
            .and_then(|c| c.get("content"))
            .and_then(|c| c.get("parts"))
            .and_then(|p| p.get(0))
            .and_then(|p| p.get("text"))
            .and_then(|t| t.as_str())
            .ok_or_else(|| {
                PlannerError::ValidationError("Missing candidate text part in Gemini response".into())
            })?;

        let plan: ImplementationPlan = serde_json::from_str(candidate_text)?;
        plan.validate()?;

        Ok((plan, usage))
    }

    /// Executes an advisory error consultation request (opt-in, user-confirmed).
    /// Does not touch filesystem, execute code, or run tests. Returns suggested fix for the local worker.
    pub async fn get_advisory_suggestion(
        &self,
        api_key: &str,
        error_text: &str,
        file_excerpt: &str,
    ) -> Result<(String, UsageMetadata), PlannerError> {
        let prompt_text = format!(
            "You are an advisory compiler and code analysis assistant. A local worker encountered the following build or test failure:\n\n\
            === ERROR SIGNATURE ===\n{}\n\n\
            === RELEVANT SOURCE FILE EXCERPT ===\n{}\n\n\
            Analyze this specific failure and provide a concise, actionable suggestion for how the local worker can fix the code. \
            Do not assume filesystem access. Your response will be passed directly to the local worker to implement and test locally.",
            error_text.trim(),
            file_excerpt.trim()
        );

        let request_body = serde_json::json!({
            "contents": [
                {
                    "role": "user",
                    "parts": [
                        { "text": prompt_text }
                    ]
                }
            ]
        });

        let url = format!(
            "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent?key={}",
            self.model, api_key
        );

        let response = self
            .client
            .post(&url)
            .json(&request_body)
            .send()
            .await?;

        let status = response.status();
        if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
            let error_text = response.text().await.unwrap_or_default();
            return Err(PlannerError::QuotaExhausted(error_text));
        }

        if !status.is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(PlannerError::ApiError {
                status: status.as_u16(),
                message: error_text,
            });
        }

        let body_json: serde_json::Value = response.json().await?;

        let mut usage = UsageMetadata::default();
        if let Some(meta) = body_json.get("usageMetadata") {
            usage.prompt_token_count = meta
                .get("promptTokenCount")
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as u32;
            usage.candidates_token_count = meta
                .get("candidatesTokenCount")
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as u32;
            usage.total_token_count = meta
                .get("totalTokenCount")
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as u32;
            usage.service_tier = meta
                .get("serviceTier")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
        }

        let candidate_text = body_json
            .get("candidates")
            .and_then(|c| c.get(0))
            .and_then(|c| c.get("content"))
            .and_then(|c| c.get("parts"))
            .and_then(|p| p.get(0))
            .and_then(|p| p.get("text"))
            .and_then(|t| t.as_str())
            .ok_or_else(|| {
                PlannerError::ValidationError("Missing candidate text part in Gemini response".into())
            })?;

        Ok((candidate_text.to_string(), usage))
    }

    /// Summarizes the task execution outcome and diff inventory using Gemini.
    pub async fn summarize_task_outcome(
        &self,
        api_key: &str,
        task_summary: &str,
        changed_files_summary: &str,
        diff_excerpts: &str,
    ) -> Result<(String, UsageMetadata), PlannerError> {
        self.summarize_task_outcome_grounded(
            api_key,
            task_summary,
            changed_files_summary,
            diff_excerpts,
            "",
            "",
        )
        .await
    }

    /// Summarizes the task execution outcome and diff inventory using Gemini with grounding against verified outcomes.
    pub async fn summarize_task_outcome_grounded(
        &self,
        api_key: &str,
        task_summary: &str,
        changed_files_summary: &str,
        diff_excerpts: &str,
        verified_commands_summary: &str,
        unperformed_steps_summary: &str,
    ) -> Result<(String, UsageMetadata), PlannerError> {
        let commands_section = if !verified_commands_summary.trim().is_empty() {
            format!(
                "### Verified Commands & Executions (Ground Truth)\n{}\n\n",
                verified_commands_summary.trim()
            )
        } else {
            "### Verified Commands & Executions (Ground Truth)\nNo commands were executed during this task.\n\n".to_string()
        };

        let unperformed_section = if !unperformed_steps_summary.trim().is_empty() {
            format!(
                "### Intended Steps / Actions That Were NOT Performed or Skipped\n{}\n\n",
                unperformed_steps_summary.trim()
            )
        } else {
            String::new()
        };

        let prompt_text = format!(
            "You are the Director AI assistant in ZenCode. A coding worker has just finished executing a task.\n\n\
            ### Original Plan / Task Description\n{}\n\n\
            ### Verified Files Modified (Ground Truth from Git)\n{}\n\n\
            ### Diff Excerpts\n{}\n\n\
            {}\
            {}\
            CRITICAL GROUNDING RULES:\n\
            1. You MUST ONLY claim actions that are provably verified in 'Verified Files Modified' or 'Verified Commands & Executions'.\n\
            2. NEVER claim a command, test suite, build, or server restart occurred unless it explicitly appears in 'Verified Commands & Executions'.\n\
            3. If any steps appear in 'Intended Steps / Actions That Were NOT Performed or Skipped', you MUST explicitly state that they were NOT performed (e.g. 'Note: dev server restart was not performed').\n\
            4. Do not assume or hallucinate actions that have no evidence in the verified ground truth.\n\n\
            OUTPUT FORMAT RULES:\n\
            - DO NOT include conversational filler, greetings, or self-identification.\n\
            - NEVER say 'As the Director AI assistant in ZenCode...' or 'Here is the summary of the completed task...'.\n\
            - Start immediately and directly with what was accomplished and any findings/answers from executed commands.\n\
            - If commands were executed, summarize their results and answer the user's question directly (e.g. count or list the items found in the command output).\n\
            - ONLY include a 'Key Files Modified' section if files were actually modified or created; do NOT write 'No files were modified' if the task was command-oriented.\n\
            - ONLY mention skipped or unperformed actions if there are entries in 'Intended Steps / Actions That Were NOT Performed or Skipped'. If all planned steps succeeded, do not invent skipped steps.\n\n\
            Please provide a clear, concise, and structured Markdown summary:\n\
            - 1-3 sentence overview of what was accomplished and the answer to the user's task or query.\n\
            - Key files modified (only if files were created or changed).\n\
            - Explicit notes on skipped actions (only if unperformed actions exist).\n\
            Keep the response professional, truthful, and well-structured in Markdown.",
            task_summary.trim(),
            changed_files_summary.trim(),
            diff_excerpts.trim(),
            commands_section,
            unperformed_section
        );

        let request_body = serde_json::json!({
            "contents": [
                {
                    "role": "user",
                    "parts": [
                        { "text": prompt_text }
                    ]
                }
            ]
        });

        let url = format!(
            "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent?key={}",
            self.model, api_key
        );

        let response = self
            .client
            .post(&url)
            .json(&request_body)
            .send()
            .await?;

        let status = response.status();
        if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
            let error_text = response.text().await.unwrap_or_default();
            return Err(PlannerError::QuotaExhausted(error_text));
        }

        if !status.is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(PlannerError::ApiError {
                status: status.as_u16(),
                message: error_text,
            });
        }

        let body_json: serde_json::Value = response.json().await?;

        let mut usage = UsageMetadata::default();
        if let Some(meta) = body_json.get("usageMetadata") {
            usage.prompt_token_count = meta
                .get("promptTokenCount")
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as u32;
            usage.candidates_token_count = meta
                .get("candidatesTokenCount")
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as u32;
            usage.total_token_count = meta
                .get("totalTokenCount")
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as u32;
        }

        let summary_text = body_json
            .get("candidates")
            .and_then(|c| c.get(0))
            .and_then(|c| c.get("content"))
            .and_then(|c| c.get("parts"))
            .and_then(|p| p.get(0))
            .and_then(|p| p.get("text"))
            .and_then(|t| t.as_str())
            .unwrap_or("Task completed successfully.")
            .to_string();

        Ok((summary_text, usage))
    }
}

/// Strips robotic AI persona preamble phrases (e.g. "As the Director AI assistant in ZenCode...")
pub fn strip_director_preamble(text: &str) -> String {
    let mut trimmed = text.trim();

    let patterns = [
        "as the director ai assistant in zencode, here is the summary of the completed task based strictly on the verified ground truth:",
        "as the director ai assistant in zencode, here is the summary of the completed task based on the verified ground truth:",
        "as the director ai assistant in zencode, here is the summary of the completed task based strictly on verified ground truth:",
        "as the director ai assistant in zencode, here is the summary of the completed task:",
        "as the director ai assistant in zencode, here is the summary of the task:",
        "as the director ai assistant in zencode, here is the summary:",
        "as the director ai assistant in zencode, here is a summary:",
        "as the director ai assistant in zencode:",
        "as the director ai assistant in zencode",
        "here is the summary of the completed task based strictly on the verified ground truth:",
        "here is the summary of the completed task based on the verified ground truth:",
        "here is the summary of the completed task based strictly on verified ground truth:",
        "here is the summary of the completed task:",
        "here is a summary of the completed task:",
        "here is the summary of the recent task execution:",
        "here is a summary of the recent task execution:",
        "here is the summary of the execution:",
    ];

    let mut changed = true;
    while changed {
        changed = false;
        let lower = trimmed.to_lowercase();
        for pat in &patterns {
            if lower.starts_with(pat) {
                trimmed = trimmed[pat.len()..].trim_start();
                while trimmed.starts_with(':') || trimmed.starts_with('\n') || trimmed.starts_with('\r') {
                    trimmed = trimmed[1..].trim_start();
                }
                changed = true;
                break;
            }
        }
    }

    trimmed.to_string()
}

/// Lightweight post-generation verification pass:
/// Cross-checks claims in the generated summary against the verified event log
/// (actual files modified, actual commands run, and unperformed steps).
pub fn verify_and_reconcile_summary(
    summary: &str,
    _files_modified: &[String],
    commands_run: &[String],
    unperformed_steps: &[String],
) -> String {
    let clean_summary = strip_director_preamble(summary);
    let lower_summary = clean_summary.to_lowercase();
    let mut warnings = Vec::new();

    // 1. Check for unverified server restart claims
    let mentions_server_restart = lower_summary.contains("restart the dev server")
        || lower_summary.contains("restarted the dev server")
        || lower_summary.contains("restarted dev server")
        || lower_summary.contains("restarting the dev server")
        || lower_summary.contains("restarted the server");

    if mentions_server_restart {
        let has_restart_cmd = commands_run.iter().any(|c| {
            let clow = c.to_lowercase();
            clow.contains("run dev") || clow.contains("npm start") || clow.contains("restart") || clow.contains("serve")
        });
        if !has_restart_cmd {
            warnings.push("The summary mentioned restarting the dev server, but no server restart command was executed in the session.");
        }
    }

    // 2. Check for unverified test execution claims
    let mentions_test_execution = lower_summary.contains("ran tests")
        || lower_summary.contains("ran all tests")
        || lower_summary.contains("tests pass")
        || lower_summary.contains("tests passed")
        || lower_summary.contains("executed the tests")
        || lower_summary.contains("verified with tests");

    if mentions_test_execution {
        let has_test_cmd = commands_run.iter().any(|c| {
            let clow = c.to_lowercase();
            clow.contains("test") || clow.contains("pytest") || clow.contains("cargo test") || clow.contains("npm test")
        });
        if !has_test_cmd {
            warnings.push("The summary mentioned running or verifying tests, but no test command was actually executed.");
        }
    }

    // 3. Check for unperformed / skipped steps
    let mut missing_unperformed_notes = Vec::new();
    for step in unperformed_steps {
        let step_low = step.to_lowercase();
        let already_noted_skipped = (lower_summary.contains(&step_low)
            && (lower_summary.contains("not performed")
                || lower_summary.contains("was not")
                || lower_summary.contains("skipped")
                || lower_summary.contains("not actually")
                || lower_summary.contains("omitted")))
            || (step_low.contains("dev server") && lower_summary.contains("server") && (lower_summary.contains("not") || lower_summary.contains("skipped")));

        if !already_noted_skipped {
            missing_unperformed_notes.push(step.clone());
        }
    }

    // If no warnings and no unperformed notes, return clean_summary
    if warnings.is_empty() && missing_unperformed_notes.is_empty() {
        return clean_summary;
    }

    let mut reconciled = clean_summary;

    if !missing_unperformed_notes.is_empty() {
        reconciled.push_str("\n\n> ℹ️ **Note on Planned Steps:** The following planned actions were not performed by the worker:\n");
        for step in missing_unperformed_notes {
            reconciled.push_str(&format!("> - {}\n", step));
        }
    }

    if !warnings.is_empty() {
        reconciled.push_str("\n> ⚠️ **Verification Notice:**\n");
        for w in warnings {
            reconciled.push_str(&format!("> - {}\n", w));
        }
    }

    reconciled
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_non_actionable_plan_validation() {
        let conversational = serde_json::json!({
            "is_actionable": false,
            "conversational_reply": "Hello! I am ZenCode, your autonomous agentic coding assistant.",
            "task_summary": "Greeting response",
            "assumptions": [],
            "questions": [],
            "project_findings": [],
            "steps": [],
            "risks": [],
            "verification_suggestions": []
        });

        let plan: ImplementationPlan = serde_json::from_value(conversational).unwrap();
        assert!(!plan.is_actionable);
        assert_eq!(plan.conversational_reply.as_deref(), Some("Hello! I am ZenCode, your autonomous agentic coding assistant."));
        assert!(plan.steps.is_empty());
        assert!(plan.validate().is_ok());
    }

    #[test]
    fn test_actionable_plan_requires_steps() {
        let actionable_empty_steps = serde_json::json!({
            "is_actionable": true,
            "conversational_reply": null,
            "task_summary": "Create a file",
            "assumptions": [],
            "questions": [],
            "project_findings": [],
            "steps": [],
            "risks": [],
            "verification_suggestions": []
        });

        let plan: ImplementationPlan = serde_json::from_value(actionable_empty_steps).unwrap();
        assert!(plan.is_actionable);
        assert!(plan.validate().is_err());
    }

    #[test]
    fn test_conversation_history_in_prompt_text() {
        let ctx = PlanningContext {
            task_description: "also add input validation to that function".to_string(),
            project_type: "Rust".to_string(),
            shallow_tree: vec!["Cargo.toml".into(), "src/main.rs".into()],
            manifest_summaries: vec![],
            git_branch_and_status: "Branch: main, Status: Clean".to_string(),
            approved_source_excerpts: vec![],
            constraints: vec![],
            conversation_history: vec![
                PriorExchange {
                    role: "user".into(),
                    content: "create calculate.rs with add function".into(),
                    files_changed: vec![],
                    summary: None,
                },
                PriorExchange {
                    role: "assistant".into(),
                    content: "Created calculate.rs with add(a: i32, b: i32) -> i32".into(),
                    files_changed: vec!["src/calculate.rs".into()],
                    summary: Some("Created calculate.rs with add function".into()),
                },
            ],
            ..Default::default()
        };

        let prompt = ctx.to_prompt_text();
        assert!(prompt.contains("PRIOR CONVERSATION & ACTIONS IN THIS CHAT:"));
        assert!(prompt.contains("User: create calculate.rs with add function"));
        assert!(prompt.contains("Assistant: Created calculate.rs with add(a: i32, b: i32) -> i32"));
        assert!(prompt.contains("Files created/modified: src/calculate.rs"));
        assert!(prompt.contains("CURRENT FOLLOW-UP REQUEST:\nalso add input validation to that function"));
    }

    #[test]
    fn test_verify_and_reconcile_detects_unverified_claims() {
        let raw_summary = "Modified main.py and restarted the dev server. All tests pass.";
        let files = vec!["main.py".to_string()];
        let commands: Vec<String> = vec![];
        let unperformed = vec!["Restart the dev server".to_string(), "Run test suite".to_string()];

        let verified = verify_and_reconcile_summary(raw_summary, &files, &commands, &unperformed);
        assert!(verified.contains("Verification Notice:"));
        assert!(verified.contains("restarting the dev server, but no server restart command was executed"));
        assert!(verified.contains("running or verifying tests, but no test command was actually executed"));
        assert!(verified.contains("The following planned actions were not performed by the worker:"));
        assert!(verified.contains("Restart the dev server"));
    }

    #[test]
    fn test_verify_and_reconcile_passes_when_grounded() {
        let raw_summary = "Modified main.py with the new handler. Note: the dev server restart was skipped.";
        let files = vec!["main.py".to_string()];
        let commands = vec!["python test_runner.py".to_string()];
        let unperformed = vec!["Restart dev server".to_string()];

        let verified = verify_and_reconcile_summary(raw_summary, &files, &commands, &unperformed);
        // Dev server was already noted as skipped in the summary, so no extra unperformed warning needed
        assert!(!verified.contains("Verification Notice:"));
    }

    #[test]
    fn test_strip_director_preamble() {
        let text1 = "As the Director AI assistant in ZenCode, here is the summary of the completed task based strictly on the verified ground truth:\n\nAdded unit tests in test_calc.py.";
        assert_eq!(strip_director_preamble(text1), "Added unit tests in test_calc.py.");

        let text2 = "Here is the summary of the completed task:\n- Updated index.html\n- Added styles.css";
        assert_eq!(strip_director_preamble(text2), "- Updated index.html\n- Added styles.css");
    }

    #[test]
    fn test_multimodal_attachment_detection_and_prompt_formatting() {
        use std::path::Path;

        assert!(is_multimodal_file(Path::new("spec.pdf")));
        assert!(is_multimodal_file(Path::new("mockup.png")));
        assert!(is_multimodal_file(Path::new("photo.jpg")));
        assert!(is_multimodal_file(Path::new("banner.webp")));
        assert!(!is_multimodal_file(Path::new("main.rs")));
        assert!(!is_multimodal_file(Path::new("index.ts")));

        assert_eq!(detect_mime_type(Path::new("document.pdf")), "application/pdf");
        assert_eq!(detect_mime_type(Path::new("image.png")), "image/png");
        assert_eq!(detect_mime_type(Path::new("image.jpeg")), "image/jpeg");

        let pdf_sample = b"%PDF-1.4 sample content";
        let att = MultimodalAttachment::from_bytes("design.pdf", "application/pdf", pdf_sample);
        assert_eq!(att.file_name, "design.pdf");
        assert_eq!(att.mime_type, "application/pdf");
        assert!(!att.base64_data.is_empty());

        let ctx = PlanningContext {
            task_description: "Build website from design.pdf".into(),
            multimodal_attachments: vec![att],
            ..Default::default()
        };

        let prompt_text = ctx.to_prompt_text();
        assert!(prompt_text.contains("ATTACHED VISUAL & MULTIMODAL SPECIFICATIONS:"));
        assert!(prompt_text.contains("[design.pdf] (Type: application/pdf)"));
        assert!(prompt_text.contains("Visually inspect their contents"));
    }
}

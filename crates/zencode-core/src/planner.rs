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

/// The exact JSON schema structure required by the locked Zencode V1 spec.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImplementationPlan {
    pub task_summary: String,
    pub assumptions: Vec<String>,
    pub questions: Vec<String>,
    pub project_findings: Vec<String>,
    pub steps: Vec<PlanStep>,
    pub risks: Vec<String>,
    pub verification_suggestions: Vec<String>,
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
        if self.task_summary.trim().is_empty() {
            return Err(PlannerError::ValidationError(
                "task_summary cannot be empty".into(),
            ));
        }
        if self.steps.is_empty() {
            return Err(PlannerError::ValidationError(
                "plan must contain at least one step".into(),
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
        out.push_str(&format!("TASK DESCRIPTION:\n{}\n\n", self.task_description));
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
        }

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

    /// Request schema matching Zencode spec
    pub fn plan_json_schema() -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
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

        let request_body = serde_json::json!({
            "contents": [
                {
                    "role": "user",
                    "parts": [
                        { "text": prompt_text }
                    ]
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
}

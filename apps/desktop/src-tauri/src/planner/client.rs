use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tauri::{AppHandle, Emitter};
use tokio::sync::Mutex;
use tokio::time::sleep;
use zencode_core::{GeminiPlannerClient, ImplementationPlan, PlanningContext, UsageMetadata};

use super::quota::{
    CASCADE_PRIORITY, ModelQuotaTracker, QuotaExhaustionAction,
};

/// High-level Cascade Planner Client managing rate limits, 80% threshold enforcement,
/// automatic failover across free models, and real-time quota telemetry.
pub struct CascadePlannerClient {
    client: reqwest::Client,
    pub quota_tracker: Arc<Mutex<ModelQuotaTracker>>,
}

impl CascadePlannerClient {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
            quota_tracker: Arc::new(Mutex::new(ModelQuotaTracker::new())),
        }
    }

    /// Estimates token count based on character-to-token ratio (approx 4 chars per token).
    pub fn estimate_tokens(text: &str) -> u32 {
        ((text.len() as f32) / 4.0).ceil() as u32
    }

    /// Executes the plan request using the rate-limit manager and failover cascade.
    pub async fn plan_with_cascade(
        &self,
        api_key: &str,
        context: &PlanningContext,
        app_handle: Option<&AppHandle>,
    ) -> Result<(ImplementationPlan, String, UsageMetadata), String> {
        context
            .assert_no_secrets()
            .map_err(|e| format!("Security validation failed: {e}"))?;

        let prompt_text = context.to_prompt_text();
        let attachment_tokens: u32 = context.multimodal_attachments.iter().map(|_| 1000).sum();
        let estimated_tokens = Self::estimate_tokens(&prompt_text) + attachment_tokens;

        // Max retry loop for sliding window queues
        let mut attempts = 0;
        let max_queue_attempts = 5;
        let mut attempted_models: Vec<String> = Vec::new();
        let mut failure_reasons: HashMap<String, String> = HashMap::new();

        loop {
            attempts += 1;

            // 1. Ask rate limit manager to select an eligible model below 80% thresholds
            let selection_result = {
                let mut tracker = self.quota_tracker.lock().await;
                tracker.select_model_for_request(estimated_tokens)
            };

            let (model_id, reason) = match selection_result {
                Ok((m, r)) => (m, r),
                Err(QuotaExhaustionAction::QueueAndRetry {
                    model_id,
                    wait_seconds,
                    reason,
                }) => {
                    if attempts > max_queue_attempts || wait_seconds > 30 {
                        return Err(format!(
                            "Rate limit pause: {reason}. Waiting time ({wait_seconds}s) exceeds automatic threshold. Please wait for reset."
                        ));
                    }

                    // Emit quota update to UI before sleeping
                    self.emit_quota_update(&model_id, app_handle).await;

                    sleep(Duration::from_secs(wait_seconds.max(1))).await;
                    continue;
                }
                Err(QuotaExhaustionAction::AllModelsAtCapacity {
                    earliest_reset_secs,
                    reason,
                }) => {
                    if attempts > max_queue_attempts || earliest_reset_secs > 30 {
                        return Err(format!(
                            "All free models at or above 80% capacity: {reason}. Earliest sliding reset in {earliest_reset_secs}s. Operating in local-only mode."
                        ));
                    }

                    self.emit_quota_update(CASCADE_PRIORITY[0], app_handle).await;
                    sleep(Duration::from_secs(earliest_reset_secs.max(1))).await;
                    continue;
                }
            };

            // 2. Emit quota telemetry to React before dispatching request
            self.emit_quota_update(&model_id, app_handle).await;

            // 3. Prepare Gemini API Request
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
                    "responseSchema": GeminiPlannerClient::plan_json_schema()
                }
            });

            // Note on model identifier:
            // "gemini-3.5-flash-lite", "gemini-3.1-flash-lite", "Gemma-4-31b", "gemini-3.8-flash"
            // Used exactly as provided in the specification.
            let url = format!(
                "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent?key={}",
                model_id, api_key
            );

            let response_result = self.client.post(&url).json(&request_body).send().await;

            // 4. Handle Network Errors
            let response = match response_result {
                Ok(res) => res,
                Err(net_err) => {
                    let reason = if net_err.is_timeout() {
                        format!("Network request timed out: {net_err}")
                    } else if net_err.is_connect() {
                        format!("Connection failed to Google API: {net_err}")
                    } else {
                        format!("Network communication error: {net_err}")
                    };

                    let is_pinned = {
                        let tracker = self.quota_tracker.lock().await;
                        tracker.pinned_model.is_some()
                    };

                    if is_pinned {
                        return Err(format!("Network error for pinned model {model_id}: {reason}"));
                    }

                    {
                        let mut tracker = self.quota_tracker.lock().await;
                        tracker.trip_model_unavailable(&model_id, 60_000, &reason);
                    }

                    attempted_models.push(model_id.clone());
                    failure_reasons.insert(model_id.clone(), reason);
                    self.emit_quota_update(&model_id, app_handle).await;

                    if CASCADE_PRIORITY.iter().all(|m| attempted_models.contains(&m.to_string())) {
                        let summary = CASCADE_PRIORITY
                            .iter()
                            .map(|m| {
                                let r = failure_reasons.get(*m).cloned().unwrap_or_else(|| "capacity exceeded".into());
                                format!("{m}: {r}")
                            })
                            .collect::<Vec<_>>()
                            .join("; ");
                        return Err(format!(
                            "All free-tier planner models are currently unavailable: {summary}. Operating in local-only mode."
                        ));
                    }

                    continue;
                }
            };

            let status = response.status();

            // 5. Handle rate-limiting (HTTP 429) from Google
            if status == reqwest::StatusCode::TOO_MANY_REQUESTS || status.as_u16() == 429 {
                let _err_text = response.text().await.unwrap_or_default();
                let reason = "HTTP 429 Rate Limit Exceeded".to_string();

                let is_pinned = {
                    let tracker = self.quota_tracker.lock().await;
                    tracker.pinned_model.is_some()
                };

                {
                    let mut tracker = self.quota_tracker.lock().await;
                    tracker.trip_model_unavailable(&model_id, 60_000, &reason);
                }

                self.emit_quota_update(&model_id, app_handle).await;

                if is_pinned {
                    return Err(format!(
                        "Pinned model {model_id} rate limit exceeded (HTTP 429). Auto-switching is disabled for pinned model. Reset in 60s."
                    ));
                }

                attempted_models.push(model_id.clone());
                failure_reasons.insert(model_id.clone(), reason);

                if CASCADE_PRIORITY.iter().all(|m| attempted_models.contains(&m.to_string())) {
                    let summary = CASCADE_PRIORITY
                        .iter()
                        .map(|m| {
                            let r = failure_reasons.get(*m).cloned().unwrap_or_else(|| "capacity exceeded".into());
                            format!("{m}: {r}")
                        })
                        .collect::<Vec<_>>()
                        .join("; ");
                    return Err(format!(
                        "All free-tier planner models are currently rate-limited or unavailable: {summary}. Operating in local-only mode."
                    ));
                }

                continue;
            }

            // 6. Handle Server-Side Errors from Google (HTTP 500, 502, 503, 504)
            if status.is_server_error() {
                let err_text = response.text().await.unwrap_or_default();
                let reason = format!("HTTP {status} Server Error: {err_text}");

                let is_pinned = {
                    let tracker = self.quota_tracker.lock().await;
                    tracker.pinned_model.is_some()
                };

                {
                    let mut tracker = self.quota_tracker.lock().await;
                    tracker.trip_model_unavailable(&model_id, 120_000, &reason);
                }

                self.emit_quota_update(&model_id, app_handle).await;

                if is_pinned {
                    return Err(format!(
                        "Pinned model {model_id} unavailable on Google's side ({reason}). Auto-switching is disabled for pinned model."
                    ));
                }

                attempted_models.push(model_id.clone());
                failure_reasons.insert(model_id.clone(), format!("HTTP {status} ({err_text})"));

                if CASCADE_PRIORITY.iter().all(|m| attempted_models.contains(&m.to_string())) {
                    let summary = CASCADE_PRIORITY
                        .iter()
                        .map(|m| {
                            let r = failure_reasons.get(*m).cloned().unwrap_or_else(|| "capacity exceeded".into());
                            format!("{m}: {r}")
                        })
                        .collect::<Vec<_>>()
                        .join("; ");
                    return Err(format!(
                        "All free-tier planner models are currently experiencing server-side unavailability: {summary}. Operating in local-only mode."
                    ));
                }

                continue;
            }

            // 7. Handle 404 (if pre-release model identifier is not active on current key/region)
            if status == reqwest::StatusCode::NOT_FOUND || status.as_u16() == 404 {
                let err_text = response.text().await.unwrap_or_default();
                let reason = format!("HTTP 404 Model Not Found / Unreleased: {err_text}");

                let is_pinned = {
                    let tracker = self.quota_tracker.lock().await;
                    tracker.pinned_model.is_some()
                };

                {
                    let mut tracker = self.quota_tracker.lock().await;
                    tracker.trip_model_unavailable(&model_id, 300_000, &reason);
                }

                self.emit_quota_update(&model_id, app_handle).await;

                if is_pinned {
                    return Err(format!(
                        "Pinned model {model_id} not found on Google API (404: {err_text})."
                    ));
                }

                attempted_models.push(model_id.clone());
                failure_reasons.insert(model_id.clone(), format!("HTTP 404 ({err_text})"));

                if CASCADE_PRIORITY.iter().all(|m| attempted_models.contains(&m.to_string())) {
                    let summary = CASCADE_PRIORITY
                        .iter()
                        .map(|m| {
                            let r = failure_reasons.get(*m).cloned().unwrap_or_else(|| "capacity exceeded".into());
                            format!("{m}: {r}")
                        })
                        .collect::<Vec<_>>()
                        .join("; ");
                    return Err(format!(
                        "All free-tier planner models are currently unavailable: {summary}. Operating in local-only mode."
                    ));
                }

                continue;
            }

            // 8. Handle 400 Bad Request with model availability/overloaded issues
            if status == reqwest::StatusCode::BAD_REQUEST || status.as_u16() == 400 {
                let err_text = response.text().await.unwrap_or_default();
                let lower = err_text.to_lowercase();
                let is_model_unavailable = lower.contains("not found")
                    || lower.contains("unsupported")
                    || lower.contains("unavailable")
                    || lower.contains("overloaded")
                    || lower.contains("capacity")
                    || lower.contains("not supported")
                    || lower.contains("invalid model");

                if is_model_unavailable {
                    let reason = format!("HTTP 400 Model Unavailable: {err_text}");

                    let is_pinned = {
                        let tracker = self.quota_tracker.lock().await;
                        tracker.pinned_model.is_some()
                    };

                    {
                        let mut tracker = self.quota_tracker.lock().await;
                        tracker.trip_model_unavailable(&model_id, 300_000, &reason);
                    }

                    self.emit_quota_update(&model_id, app_handle).await;

                    if is_pinned {
                        return Err(format!("Pinned model {model_id} unavailable: {err_text}"));
                    }

                    attempted_models.push(model_id.clone());
                    failure_reasons.insert(model_id.clone(), format!("HTTP 400 ({err_text})"));

                    if CASCADE_PRIORITY.iter().all(|m| attempted_models.contains(&m.to_string())) {
                        let summary = CASCADE_PRIORITY
                            .iter()
                            .map(|m| {
                                let r = failure_reasons.get(*m).cloned().unwrap_or_else(|| "capacity exceeded".into());
                                format!("{m}: {r}")
                            })
                            .collect::<Vec<_>>()
                            .join("; ");
                        return Err(format!(
                            "All free-tier planner models are currently unavailable: {summary}. Operating in local-only mode."
                        ));
                    }

                    continue;
                } else {
                    return Err(format!(
                        "Gemini API returned error 400 Bad Request for model {model_id}: {err_text}"
                    ));
                }
            }

            if !status.is_success() {
                let err_text = response.text().await.unwrap_or_default();
                return Err(format!(
                    "Gemini API returned error {status} for model {model_id}: {err_text}"
                ));
            }

            // 5. Successful response: parse payload and usageMetadata
            let body_json: serde_json::Value = response
                .json()
                .await
                .map_err(|e| format!("Failed to parse Gemini response JSON: {e}"))?;

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

            let actual_tokens = if usage.total_token_count > 0 {
                usage.total_token_count
            } else {
                estimated_tokens
            };

            // 6. Record actual usage in Rate Limit Manager
            {
                let mut tracker = self.quota_tracker.lock().await;
                tracker.record_actual_usage(&model_id, actual_tokens);
                tracker.last_switch_reason = Some(reason.clone());
            }

            // 7. Emit updated telemetry event to React dashboard
            self.emit_quota_update(&model_id, app_handle).await;

            // 8. Extract and validate plan candidate
            let candidate_text = body_json
                .get("candidates")
                .and_then(|c| c.get(0))
                .and_then(|c| c.get("content"))
                .and_then(|c| c.get("parts"))
                .and_then(|p| p.get(0))
                .and_then(|p| p.get("text"))
                .and_then(|t| t.as_str())
                .ok_or_else(|| "Missing candidate text part in Gemini API response".to_string())?;

            let plan: ImplementationPlan = serde_json::from_str(candidate_text)
                .map_err(|e| format!("Structured plan deserialization failed: {e}"))?;

            plan.validate()
                .map_err(|e| format!("Plan sanity validation failed: {e}"))?;

            return Ok((plan, model_id, usage));
        }
    }

    /// Emits `planner:quota_update` event to React frontend.
    pub async fn emit_quota_update(&self, active_model: &str, app_handle: Option<&AppHandle>) {
        if let Some(handle) = app_handle {
            let event_payload = {
                let mut tracker = self.quota_tracker.lock().await;
                tracker.to_quota_update_event(active_model)
            };

            let _ = handle.emit("planner:quota_update", &event_payload);
        }
    }

    /// Executes an advisory error consultation through the rate limiter cascade.
    pub async fn request_advisory_suggestion(
        &self,
        api_key: &str,
        error_text: &str,
        file_excerpt: &str,
        app_handle: Option<&AppHandle>,
    ) -> Result<(String, String, UsageMetadata), String> {
        let estimated_tokens = Self::estimate_tokens(error_text) + Self::estimate_tokens(file_excerpt) + 100;

        let (model_id, reason) = {
            let mut tracker = self.quota_tracker.lock().await;
            tracker.select_model_for_request(estimated_tokens).map_err(|e| match e {
                QuotaExhaustionAction::QueueAndRetry { reason, .. } => format!("Rate limit pause: {reason}"),
                QuotaExhaustionAction::AllModelsAtCapacity { reason, .. } => format!("All free models at capacity: {reason}"),
            })?
        };

        self.emit_quota_update(&model_id, app_handle).await;

        let client = GeminiPlannerClient::with_model(&model_id);
        let (suggestion, usage) = client
            .get_advisory_suggestion(api_key, error_text, file_excerpt)
            .await
            .map_err(|e| format!("Advisory consultation failed: {e}"))?;

        let actual_tokens = if usage.total_token_count > 0 {
            usage.total_token_count
        } else {
            estimated_tokens
        };

        {
            let mut tracker = self.quota_tracker.lock().await;
            tracker.record_actual_usage(&model_id, actual_tokens);
            tracker.last_switch_reason = Some(reason.clone());
        }

        self.emit_quota_update(&model_id, app_handle).await;

        Ok((suggestion, model_id, usage))
    }

    /// Summarizes the task outcome through the rate limiter cascade.
    pub async fn summarize_task_outcome(
        &self,
        api_key: &str,
        task_summary: &str,
        changed_files_summary: &str,
        diff_excerpts: &str,
        app_handle: Option<&AppHandle>,
    ) -> Result<(String, String, UsageMetadata), String> {
        self.summarize_task_outcome_grounded(
            api_key,
            task_summary,
            changed_files_summary,
            diff_excerpts,
            "",
            "",
            app_handle,
        )
        .await
    }

    /// Summarizes the task outcome through the rate limiter cascade with grounded verified outcomes.
    pub async fn summarize_task_outcome_grounded(
        &self,
        api_key: &str,
        task_summary: &str,
        changed_files_summary: &str,
        diff_excerpts: &str,
        verified_commands_summary: &str,
        unperformed_steps_summary: &str,
        app_handle: Option<&AppHandle>,
    ) -> Result<(String, String, UsageMetadata), String> {
        let estimated_tokens = Self::estimate_tokens(task_summary)
            + Self::estimate_tokens(changed_files_summary)
            + Self::estimate_tokens(diff_excerpts)
            + Self::estimate_tokens(verified_commands_summary)
            + Self::estimate_tokens(unperformed_steps_summary)
            + 250;

        let (model_id, reason) = {
            let mut tracker = self.quota_tracker.lock().await;
            tracker.select_model_for_request(estimated_tokens).map_err(|e| match e {
                QuotaExhaustionAction::QueueAndRetry { reason, .. } => format!("Rate limit pause: {reason}"),
                QuotaExhaustionAction::AllModelsAtCapacity { reason, .. } => format!("All free models at capacity: {reason}"),
            })?
        };

        self.emit_quota_update(&model_id, app_handle).await;

        let client = GeminiPlannerClient::with_model(&model_id);
        let (summary, usage) = client
            .summarize_task_outcome_grounded(
                api_key,
                task_summary,
                changed_files_summary,
                diff_excerpts,
                verified_commands_summary,
                unperformed_steps_summary,
            )
            .await
            .map_err(|e| format!("Task summarization failed: {e}"))?;

        let actual_tokens = if usage.total_token_count > 0 {
            usage.total_token_count
        } else {
            estimated_tokens
        };

        {
            let mut tracker = self.quota_tracker.lock().await;
            tracker.record_actual_usage(&model_id, actual_tokens);
            tracker.last_switch_reason = Some(reason.clone());
        }

        self.emit_quota_update(&model_id, app_handle).await;

        Ok((summary, model_id, usage))
    }
}

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
        let estimated_tokens = Self::estimate_tokens(&prompt_text);

        // Max retry loop for sliding window queues
        let mut attempts = 0;
        let max_queue_attempts = 5;

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

            let response = match response_result {
                Ok(res) => res,
                Err(net_err) => {
                    return Err(format!("Network error while contacting Gemini API: {net_err}"));
                }
            };

            let status = response.status();

            // 4. Handle rate-limiting (429) or service unavailable (503) from Google
            if status == reqwest::StatusCode::TOO_MANY_REQUESTS || status.as_u16() == 429 {
                let _err_text = response.text().await.unwrap_or_default();

                {
                    let mut tracker = self.quota_tracker.lock().await;
                    // Trip 60s cooldown on this model
                    tracker.trip_model_429(&model_id, 60_000);
                }

                self.emit_quota_update(&model_id, app_handle).await;

                // Check if user has pinned this model: if pinned, no auto-failover
                let is_pinned = {
                    let tracker = self.quota_tracker.lock().await;
                    tracker.pinned_model.is_some()
                };

                if is_pinned {
                    return Err(format!(
                        "Pinned model {model_id} rate limit exceeded (HTTP 429). Auto-switching is disabled for pinned model. Reset in 60s."
                    ));
                }

                // If not pinned, retry next iteration — the router will select the next model in cascade
                continue;
            }

            // 4b. Handle 404 (if pre-release model identifier is not active on current key)
            if status == reqwest::StatusCode::NOT_FOUND || status.as_u16() == 404 {
                let err_text = response.text().await.unwrap_or_default();
                {
                    let mut tracker = self.quota_tracker.lock().await;
                    // Set 5 minute cooldown on unreleased endpoint
                    tracker.trip_model_429(&model_id, 300_000);
                    tracker.last_switch_reason = Some(format!(
                        "Model identifier {model_id} returned 404; automatically failing over to next model in cascade."
                    ));
                }

                self.emit_quota_update(&model_id, app_handle).await;

                let is_pinned = {
                    let tracker = self.quota_tracker.lock().await;
                    tracker.pinned_model.is_some()
                };

                if is_pinned {
                    return Err(format!(
                        "Pinned model {model_id} not found on Google API (404: {err_text})."
                    ));
                }

                continue;
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
}

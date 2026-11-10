use chrono::{DateTime, Duration, FixedOffset, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::time::{SystemTime, UNIX_EPOCH};

/// Exact model identifier strings provided in the specification.
/// Note on API model strings:
/// "gemini-3.5-flash-lite", "gemini-3.1-flash-lite", "Gemma-4-31b", "gemini-3.8-flash"
/// These are stored and called exactly as given without altering model name strings.
/// If an endpoint returns 404/400 (e.g. unreleased in current API project), the cascade router
/// flags it and seamlessly falls back to the next model in priority order.
pub const MODEL_PRIMARY: &str = "gemini-3.5-flash-lite";
pub const MODEL_SECONDARY: &str = "gemini-3.1-flash-lite";
pub const MODEL_TERTIARY: &str = "Gemma-4-31b";
pub const MODEL_PINNED_ADVANCED: &str = "gemini-3.8-flash";

pub const GROQ_MODEL_120B: &str = "openai/gpt-oss-120b";
pub const GROQ_MODEL_20B: &str = "openai/gpt-oss-20b";
pub const GROQ_MODEL_QWEN: &str = "qwen/qwen3.8-27b";

pub const CASCADE_PRIORITY: &[&str] = &[
    MODEL_PRIMARY,
    MODEL_SECONDARY,
    MODEL_TERTIARY,
];

pub const GROQ_ROTATION_MODELS: &[&str] = &[
    GROQ_MODEL_120B,
    GROQ_MODEL_20B,
    GROQ_MODEL_QWEN,
];

/// Specification limits from the source-of-truth spreadsheet.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct ModelLimits {
    pub rpm: u32,
    pub tpm: u32,
    pub rpd: u32,
    #[serde(default)]
    pub tpd: u32,
}

impl ModelLimits {
    pub const fn for_model(id: &str) -> Self {
        if match_str(id, MODEL_PRIMARY) {
            Self { rpm: 15, tpm: 250_000, rpd: 500, tpd: 1_000_000 }
        } else if match_str(id, MODEL_SECONDARY) {
            Self { rpm: 15, tpm: 250_000, rpd: 500, tpd: 1_000_000 }
        } else if match_str(id, MODEL_TERTIARY) {
            Self { rpm: 30, tpm: 16_000, rpd: 14_400, tpd: 1_000_000 }
        } else if match_str(id, MODEL_PINNED_ADVANCED) {
            Self { rpm: 15, tpm: 1_000_000, rpd: 1_500, tpd: 4_000_000 }
        } else if match_str(id, GROQ_MODEL_120B)
            || match_str(id, GROQ_MODEL_20B)
            || match_str(id, GROQ_MODEL_QWEN)
        {
            // Authoritative Groq limits: 30 RPM, 8,000 TPM, 1,000 RPD, 200,000 TPD
            Self { rpm: 30, tpm: 8_000, rpd: 1_000, tpd: 200_000 }
        } else {
            // Default conservative fallback
            Self { rpm: 15, tpm: 250_000, rpd: 500, tpd: 1_000_000 }
        }
    }
}

/// Parsed Groq rate-limit response headers
#[derive(Debug, Clone, Default)]
pub struct GroqRateLimitHeaders {
    pub retry_after_secs: Option<u64>,
    pub limit_requests_rpd: Option<u32>,
    pub remaining_requests_rpd: Option<u32>,
    pub limit_tokens_tpm: Option<u32>,
    pub remaining_tokens_tpm: Option<u32>,
    pub reset_requests_secs: Option<u64>,
    pub reset_tokens_secs: Option<u64>,
}

/// Parses duration strings like "1m26.4s", "652ms", "2.5s", or "10s" into integer seconds.
pub fn parse_duration_string_to_seconds(raw: &str) -> Option<u64> {
    let s = raw.trim();
    if s.is_empty() {
        return None;
    }
    if let Ok(secs) = s.parse::<f64>() {
        return Some(secs.ceil() as u64);
    }
    let mut total_secs = 0.0;
    let mut current_num = String::new();
    let mut chars = s.chars().peekable();

    while let Some(c) = chars.next() {
        if c.is_ascii_digit() || c == '.' {
            current_num.push(c);
        } else if c.is_alphabetic() {
            let mut unit = String::new();
            unit.push(c);
            while let Some(&next_c) = chars.peek() {
                if next_c.is_alphabetic() {
                    unit.push(chars.next().unwrap());
                } else {
                    break;
                }
            }
            if let Ok(val) = current_num.parse::<f64>() {
                match unit.to_lowercase().as_str() {
                    "h" | "hr" | "hrs" | "hour" | "hours" => total_secs += val * 3600.0,
                    "m" | "min" | "mins" | "minute" | "minutes" => total_secs += val * 60.0,
                    "s" | "sec" | "secs" | "second" | "seconds" => total_secs += val,
                    "ms" | "msec" | "millis" | "milliseconds" => total_secs += val / 1000.0,
                    _ => {}
                }
            }
            current_num.clear();
        }
    }
    if total_secs > 0.0 {
        Some(total_secs.ceil() as u64)
    } else {
        None
    }
}

/// Extracts real ground-truth rate-limit headers from Groq API response.
pub fn parse_groq_headers(headers: &reqwest::header::HeaderMap) -> GroqRateLimitHeaders {
    let retry_after = headers
        .get("retry-after")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse::<u64>().ok());

    let limit_requests = headers
        .get("x-ratelimit-limit-requests")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse::<u32>().ok());

    let remaining_requests = headers
        .get("x-ratelimit-remaining-requests")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse::<u32>().ok());

    let limit_tokens = headers
        .get("x-ratelimit-limit-tokens")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse::<u32>().ok());

    let remaining_tokens = headers
        .get("x-ratelimit-remaining-tokens")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse::<u32>().ok());

    let reset_requests = headers
        .get("x-ratelimit-reset-requests")
        .and_then(|v| v.to_str().ok())
        .and_then(parse_duration_string_to_seconds);

    let reset_tokens = headers
        .get("x-ratelimit-reset-tokens")
        .and_then(|v| v.to_str().ok())
        .and_then(parse_duration_string_to_seconds);

    GroqRateLimitHeaders {
        retry_after_secs: retry_after,
        limit_requests_rpd: limit_requests,
        remaining_requests_rpd: remaining_requests,
        limit_tokens_tpm: limit_tokens,
        remaining_tokens_tpm: remaining_tokens,
        reset_requests_secs: reset_requests,
        reset_tokens_secs: reset_tokens,
    }
}

const fn match_str(a: &str, b: &str) -> bool {
    let a_bytes = a.as_bytes();
    let b_bytes = b.as_bytes();
    if a_bytes.len() != b_bytes.len() {
        return false;
    }
    let mut i = 0;
    while i < a_bytes.len() {
        if a_bytes[i] != b_bytes[i] {
            return false;
        }
        i += 1;
    }
    true
}

/// Tracks real-time sliding-window and daily usage per model.
#[derive(Debug, Clone)]
pub struct SingleModelTracker {
    pub model_id: String,
    pub display_name: String,
    pub limits: ModelLimits,
    /// Timestamps (ms) of requests within the sliding 60-second window
    pub rpm_history: VecDeque<u64>,
    /// (Timestamp ms, tokens consumed) within the sliding 60-second window
    pub tpm_history: VecDeque<(u64, u32)>,
    /// Requests consumed today since midnight Pacific Time
    pub rpd_count: u32,
    /// Last Pacific date string tracked (e.g. "2026-11-05")
    pub current_pacific_date: String,
    /// Temporary block flag if 429 received from Google API directly
    pub temporary_api_cooldown_until: Option<u64>,
    /// Reason why the model is temporarily on cooldown or unavailable
    pub unavailability_reason: Option<String>,
    /// Authoritative server-reported remaining daily requests (RPD from x-ratelimit-remaining-requests)
    pub server_remaining_rpd: Option<u32>,
    /// Authoritative server-reported remaining minute tokens (TPM from x-ratelimit-remaining-tokens)
    pub server_remaining_tpm: Option<u32>,
    /// Authoritative server-reported daily request limit (from x-ratelimit-limit-requests)
    pub server_limit_rpd: Option<u32>,
    /// Authoritative server-reported minute token limit (from x-ratelimit-limit-tokens)
    pub server_limit_tpm: Option<u32>,
    /// Authoritative server reset seconds for RPD (from x-ratelimit-reset-requests)
    pub server_reset_rpd_seconds: Option<u64>,
    /// Authoritative server reset seconds for TPM (from x-ratelimit-reset-tokens)
    pub server_reset_tpm_seconds: Option<u64>,
}

impl SingleModelTracker {
    pub fn new(model_id: &str, display_name: &str) -> Self {
        let limits = ModelLimits::for_model(model_id);
        Self {
            model_id: model_id.to_string(),
            display_name: display_name.to_string(),
            limits,
            rpm_history: VecDeque::new(),
            tpm_history: VecDeque::new(),
            rpd_count: 0,
            current_pacific_date: get_pacific_date_string(),
            temporary_api_cooldown_until: None,
            unavailability_reason: None,
            server_remaining_rpd: None,
            server_remaining_tpm: None,
            server_limit_rpd: None,
            server_limit_tpm: None,
            server_reset_rpd_seconds: None,
            server_reset_tpm_seconds: None,
        }
    }

    /// Ingests real server ground-truth rate limit headers from Groq responses.
    pub fn update_from_groq_headers(&mut self, now_ms: u64, headers: &GroqRateLimitHeaders) {
        if let Some(lim_rpd) = headers.limit_requests_rpd {
            self.server_limit_rpd = Some(lim_rpd);
        }
        if let Some(rem_rpd) = headers.remaining_requests_rpd {
            self.server_remaining_rpd = Some(rem_rpd);
            // Overwrite locally-tracked rpd_count with server ground truth
            self.rpd_count = self.limits.rpd.saturating_sub(rem_rpd);
        }
        if let Some(lim_tpm) = headers.limit_tokens_tpm {
            self.server_limit_tpm = Some(lim_tpm);
        }
        if let Some(rem_tpm) = headers.remaining_tokens_tpm {
            self.server_remaining_tpm = Some(rem_tpm);
        }
        if let Some(reset_rpd) = headers.reset_requests_secs {
            self.server_reset_rpd_seconds = Some(reset_rpd);
        }
        if let Some(reset_tpm) = headers.reset_tokens_secs {
            self.server_reset_tpm_seconds = Some(reset_tpm);
        }
        if let Some(retry_after) = headers.retry_after_secs {
            self.trip_cooldown(now_ms, retry_after * 1000);
        }
    }

    /// Prunes expired timestamps older than 60 seconds and resets RPD if crossing midnight Pacific Time.
    pub fn prune(&mut self, now_ms: u64) {
        let window_start = now_ms.saturating_sub(60_000);

        while let Some(&ts) = self.rpm_history.front() {
            if ts < window_start {
                self.rpm_history.pop_front();
            } else {
                break;
            }
        }

        while let Some(&(ts, _)) = self.tpm_history.front() {
            if ts < window_start {
                self.tpm_history.pop_front();
            } else {
                break;
            }
        }

        // Check Pacific Time midnight rollover
        let today_pacific = get_pacific_date_string();
        if self.current_pacific_date != today_pacific {
            self.rpd_count = 0;
            self.current_pacific_date = today_pacific;
        }

        // Clear cooldown if expired
        if let Some(cd) = self.temporary_api_cooldown_until {
            if now_ms >= cd {
                self.temporary_api_cooldown_until = None;
                self.unavailability_reason = None;
            }
        }
    }

    pub fn current_rpm(&self) -> u32 {
        self.rpm_history.len() as u32
    }

    pub fn current_tpm(&self) -> u32 {
        self.tpm_history.iter().map(|(_, tokens)| *tokens).sum()
    }

    pub fn current_rpd(&self) -> u32 {
        self.rpd_count
    }

    /// Checks if the model is below 80% on all three dimensions (RPM, TPM, RPD)
    /// and is not currently on temporary cooldown or unavailable.
    pub fn is_below_80_percent(&self, now_ms: u64, estimated_tokens: u32) -> Result<(), String> {
        if let Some(cd) = self.temporary_api_cooldown_until {
            if now_ms < cd {
                let remaining_secs = (cd - now_ms) / 1000 + 1;
                let reason = self
                    .unavailability_reason
                    .as_deref()
                    .unwrap_or("Temporary API cooldown");
                return Err(format!(
                    "Model {} is cooling down: {} ({}s remaining)",
                    self.display_name, reason, remaining_secs
                ));
            }
        }

        let rpm = self.current_rpm();
        let rpm_thresh = (self.limits.rpm as f32 * 0.8).floor() as u32;
        if rpm >= rpm_thresh {
            return Err(format!(
                "RPM threshold >= 80%: {}/{} (limit {})",
                rpm, rpm_thresh, self.limits.rpm
            ));
        }

        let tpm = self.current_tpm() + estimated_tokens;
        let tpm_thresh = (self.limits.tpm as f32 * 0.8).floor() as u32;
        if tpm >= tpm_thresh {
            return Err(format!(
                "TPM threshold >= 80%: {}/{} (limit {})",
                tpm, tpm_thresh, self.limits.tpm
            ));
        }

        let rpd = self.current_rpd();
        let rpd_thresh = (self.limits.rpd as f32 * 0.8).floor() as u32;
        if rpd >= rpd_thresh {
            return Err(format!(
                "RPD threshold >= 80%: {}/{} (limit {})",
                rpd, rpd_thresh, self.limits.rpd
            ));
        }

        Ok(())
    }

    /// Records actual request and usage tokens from usageMetadata.
    pub fn record_actual_usage(&mut self, now_ms: u64, tokens_used: u32) {
        self.prune(now_ms);
        self.rpm_history.push_back(now_ms);
        self.tpm_history.push_back((now_ms, tokens_used));
        self.rpd_count += 1;
    }

    /// Trips temporary cooldown when an unexpected 429 is received from the Google API.
    pub fn trip_cooldown(&mut self, now_ms: u64, cooldown_ms: u64) {
        self.trip_unavailable(now_ms, cooldown_ms, "Rate limit (HTTP 429)");
    }

    /// Trips temporary cooldown when a model is unavailable (5xx, 404, 400 unsupported, network timeout).
    pub fn trip_unavailable(&mut self, now_ms: u64, cooldown_ms: u64, reason: &str) {
        self.temporary_api_cooldown_until = Some(now_ms + cooldown_ms);
        self.unavailability_reason = Some(reason.to_string());
    }

    /// Returns time in seconds until the earliest sliding window release (RPM or TPM).
    pub fn seconds_until_sliding_release(&self, now_ms: u64) -> u64 {
        let mut min_wait_ms: Option<u64> = None;

        if let Some(&earliest_rpm) = self.rpm_history.front() {
            let expires = earliest_rpm + 60_000;
            if expires > now_ms {
                min_wait_ms = Some(expires - now_ms);
            }
        }

        if let Some(&(earliest_tpm, _)) = self.tpm_history.front() {
            let expires = earliest_tpm + 60_000;
            if expires > now_ms {
                let wait = expires - now_ms;
                min_wait_ms = Some(min_wait_ms.map_or(wait, |m| m.min(wait)));
            }
        }

        min_wait_ms.map_or(1, |ms| (ms / 1000) + 1)
    }

    /// Returns time in seconds until the model becomes available (considering sliding release and cooldown).
    pub fn seconds_until_available(&self, now_ms: u64) -> u64 {
        let sliding_wait = self.seconds_until_sliding_release(now_ms);
        if let Some(cd) = self.temporary_api_cooldown_until {
            if cd > now_ms {
                let cd_wait = ((cd - now_ms) / 1000) + 1;
                return sliding_wait.max(cd_wait);
            }
        }
        sliding_wait
    }

    pub fn to_status(&self, now_ms: u64, estimated_tokens: u32) -> ModelQuotaStatus {
        let rpm = self.current_rpm();
        let tpm = self.current_tpm();
        let rpd = self.current_rpd();

        let rpm_rem = self.limits.rpm.saturating_sub(rpm);
        let tpm_rem = self
            .server_remaining_tpm
            .unwrap_or_else(|| self.limits.tpm.saturating_sub(tpm));
        let rpd_rem = self
            .server_remaining_rpd
            .unwrap_or_else(|| self.limits.rpd.saturating_sub(rpd));

        let tpm_consumed = self.limits.tpm.saturating_sub(tpm_rem);
        let rpd_consumed = self.limits.rpd.saturating_sub(rpd_rem);

        let rpm_reset_secs = self.seconds_until_sliding_release(now_ms) as u32;
        let tpm_reset_secs = self
            .server_reset_tpm_seconds
            .map(|s| s as u32)
            .unwrap_or(rpm_reset_secs);

        let (default_rpd_reset_secs, default_rpd_reset_formatted) = get_seconds_until_midnight_pacific();
        let (rpd_reset_secs, rpd_reset_formatted) = if let Some(secs) = self.server_reset_rpd_seconds {
            let hours = secs / 3600;
            let mins = (secs % 3600) / 60;
            let rem_s = secs % 60;
            let formatted = if hours > 0 {
                format!("{hours}h {mins}m {rem_s}s")
            } else if mins > 0 {
                format!("{mins}m {rem_s}s")
            } else {
                format!("{rem_s}s")
            };
            (secs, formatted)
        } else {
            (default_rpd_reset_secs, default_rpd_reset_formatted)
        };

        let is_below_80 = if let Some(rem_rpd) = self.server_remaining_rpd {
            let rpd_thresh = (self.limits.rpd as f32 * 0.8).floor() as u32;
            let consumed = self.limits.rpd.saturating_sub(rem_rpd);
            consumed < rpd_thresh && self.is_below_80_percent(now_ms, estimated_tokens).is_ok()
        } else {
            self.is_below_80_percent(now_ms, estimated_tokens).is_ok()
        };
        let is_on_cooldown = self.temporary_api_cooldown_until.map_or(false, |cd| now_ms < cd);
        let is_hard_exhausted = rpm >= self.limits.rpm || tpm_rem == 0 || rpd_rem == 0;

        ModelQuotaStatus {
            model_id: self.model_id.clone(),
            display_name: self.display_name.clone(),
            rpm_limit: self.server_limit_rpd.map(|_| self.limits.rpm).unwrap_or(self.limits.rpm),
            rpm_consumed: rpm,
            rpm_remaining: rpm_rem,
            rpm_reset_seconds: rpm_reset_secs,

            tpm_limit: self.server_limit_tpm.unwrap_or(self.limits.tpm),
            tpm_consumed,
            tpm_remaining: tpm_rem,
            tpm_reset_seconds: tpm_reset_secs,

            rpd_limit: self.server_limit_rpd.unwrap_or(self.limits.rpd),
            rpd_consumed,
            rpd_remaining: rpd_rem,
            rpd_reset_formatted,
            rpd_reset_seconds: rpd_reset_secs,

            is_available: !is_hard_exhausted && !is_on_cooldown,
            threshold_80_percent_exceeded: !is_below_80,
            unavailability_reason: self.unavailability_reason.clone(),
        }
    }
}

/// The Rate Limit Manager tracking RPM, TPM, and RPD across all four supported models.
#[derive(Debug)]
pub struct ModelQuotaTracker {
    pub trackers: HashMap<String, SingleModelTracker>,
    pub pinned_model: Option<String>,
    pub last_switch_reason: Option<String>,
}

impl ModelQuotaTracker {
    pub fn new() -> Self {
        let mut trackers = HashMap::new();
        trackers.insert(
            MODEL_PRIMARY.to_string(),
            SingleModelTracker::new(MODEL_PRIMARY, "Gemini 3.5 Flash Lite"),
        );
        trackers.insert(
            MODEL_SECONDARY.to_string(),
            SingleModelTracker::new(MODEL_SECONDARY, "Gemini 3.1 Flash Lite"),
        );
        trackers.insert(
            MODEL_TERTIARY.to_string(),
            SingleModelTracker::new(MODEL_TERTIARY, "Gemma 4 31B"),
        );
        trackers.insert(
            MODEL_PINNED_ADVANCED.to_string(),
            SingleModelTracker::new(MODEL_PINNED_ADVANCED, "Gemini 3.8 Flash"),
        );
        trackers.insert(
            GROQ_MODEL_120B.to_string(),
            SingleModelTracker::new(GROQ_MODEL_120B, "OpenAI GPT-OSS 120B (Groq)"),
        );
        trackers.insert(
            GROQ_MODEL_20B.to_string(),
            SingleModelTracker::new(GROQ_MODEL_20B, "OpenAI GPT-OSS 20B (Groq)"),
        );
        trackers.insert(
            GROQ_MODEL_QWEN.to_string(),
            SingleModelTracker::new(GROQ_MODEL_QWEN, "Qwen 3.8 27B (Groq)"),
        );

        Self {
            trackers,
            pinned_model: None,
            last_switch_reason: None,
        }
    }

    pub fn set_pinned_model(&mut self, model: Option<String>) {
        if let Some(ref m) = model {
            self.last_switch_reason = Some(format!("User manually pinned model to {m}"));
        } else {
            self.last_switch_reason = Some("Reverted to automatic free-tier cascade".into());
        }
        self.pinned_model = model;
    }

    /// Selects the approved model according to priority and the 80% threshold invariant.
    pub fn select_model_for_request(
        &mut self,
        estimated_tokens: u32,
    ) -> Result<(String, String), QuotaExhaustionAction> {
        let now_ms = now_millis();
        for tracker in self.trackers.values_mut() {
            tracker.prune(now_ms);
        }

        // 1. If user manually pinned gemini-3.8-flash, auto-switching is disabled
        if let Some(ref pinned) = self.pinned_model {
            if let Some(tracker) = self.trackers.get(pinned) {
                match tracker.is_below_80_percent(now_ms, estimated_tokens) {
                    Ok(()) => {
                        return Ok((
                            pinned.clone(),
                            format!("Using manually pinned model {pinned}"),
                        ));
                    }
                    Err(reason) => {
                        let wait_secs = tracker.seconds_until_available(now_ms);
                        return Err(QuotaExhaustionAction::QueueAndRetry {
                            model_id: pinned.clone(),
                            wait_seconds: wait_secs,
                            reason: format!(
                                "Pinned model {pinned} unavailable or exceeded 80% threshold: {reason}"
                            ),
                        });
                    }
                }
            }
        }

        // 2. Cascade priority order: Primary -> Secondary -> Tertiary
        let mut reasons = Vec::new();

        for &candidate in CASCADE_PRIORITY {
            if let Some(tracker) = self.trackers.get(candidate) {
                match tracker.is_below_80_percent(now_ms, estimated_tokens) {
                    Ok(()) => {
                        let reason = if candidate == MODEL_PRIMARY && reasons.is_empty() {
                            "Primary planner (gemini-3.5-flash-lite) under 80% threshold".to_string()
                        } else {
                            format!(
                                "Auto-switched to {}: prior models unavailable or exceeded 80% threshold ({})",
                                candidate,
                                reasons.join("; ")
                            )
                        };
                        self.last_switch_reason = Some(reason.clone());
                        return Ok((candidate.to_string(), reason));
                    }
                    Err(err) => {
                        reasons.push(format!("{candidate} ({err})"));
                    }
                }
            }
        }

        // 3. All models are >= 80% or cooling down: find minimum wait time to queue
        let mut min_wait = 60u64;
        for &candidate in CASCADE_PRIORITY {
            if let Some(tracker) = self.trackers.get(candidate) {
                let wait = tracker.seconds_until_available(now_ms);
                min_wait = min_wait.min(wait);
            }
        }

        Err(QuotaExhaustionAction::AllModelsAtCapacity {
            earliest_reset_secs: min_wait,
            reason: format!(
                "All free-tier planner models are currently unavailable or above 80% capacity: {}",
                reasons.join(" | ")
            ),
        })
    }

    /// Records actual usage received in usageMetadata from Gemini response.
    pub fn record_actual_usage(&mut self, model_id: &str, tokens_used: u32) {
        let now_ms = now_millis();
        if let Some(tracker) = self.trackers.get_mut(model_id) {
            tracker.record_actual_usage(now_ms, tokens_used);
        }
    }

    /// Trips cooldown when a model is unavailable (server error, timeout, 404, or unsupported).
    pub fn trip_model_unavailable(&mut self, model_id: &str, cooldown_ms: u64, reason: &str) {
        let now_ms = now_millis();
        if let Some(tracker) = self.trackers.get_mut(model_id) {
            tracker.trip_unavailable(now_ms, cooldown_ms, reason);
        }
        self.last_switch_reason = Some(format!(
            "Model {model_id} unavailable ({reason}); failing over to next free model in cascade"
        ));
    }

    /// Trips cooldown when a 429 response is returned from Google API.
    pub fn trip_model_429(&mut self, model_id: &str, cooldown_ms: u64) {
        self.trip_model_unavailable(model_id, cooldown_ms, "HTTP 429 rate limit exceeded");
    }

    /// Ingests Groq server rate limit headers and updates the tracker for the corresponding model.
    pub fn update_groq_server_headers(&mut self, model_id: &str, headers: &GroqRateLimitHeaders) {
        let now_ms = now_millis();
        if let Some(tracker) = self.trackers.get_mut(model_id) {
            tracker.update_from_groq_headers(now_ms, headers);
        }
    }

    /// Computes combined daily capacity across all 3 Groq rotation models (3,000 total RPD).
    pub fn get_combined_groq_capacity(&self) -> (u32, u32) {
        let mut total_limit = 0u32;
        let mut total_rem = 0u32;
        for &id in GROQ_ROTATION_MODELS {
            if let Some(tracker) = self.trackers.get(id) {
                total_limit += tracker.limits.rpd;
                let rem = tracker.server_remaining_rpd.unwrap_or_else(|| {
                    tracker.limits.rpd.saturating_sub(tracker.rpd_count)
                });
                total_rem += rem;
            }
        }
        (total_rem, total_limit)
    }

    /// Checks if all 3 Groq models are exhausted for the day (0 remaining or all in cooldown).
    pub fn are_all_groq_models_exhausted(&self) -> bool {
        let now_ms = now_millis();
        for &id in GROQ_ROTATION_MODELS {
            if let Some(tracker) = self.trackers.get(id) {
                let is_cooling = tracker.temporary_api_cooldown_until.map_or(false, |cd| now_ms < cd);
                let rem_rpd = tracker.server_remaining_rpd.unwrap_or_else(|| {
                    tracker.limits.rpd.saturating_sub(tracker.rpd_count)
                });
                let rem_tpm = tracker.server_remaining_tpm.unwrap_or_else(|| {
                    tracker.limits.tpm.saturating_sub(tracker.current_tpm())
                });
                if !is_cooling && rem_rpd > 0 && rem_tpm > 0 {
                    return false;
                }
            }
        }
        true
    }

    /// Selects an available Groq model following priority rotation:
    /// openai/gpt-oss-120b -> openai/gpt-oss-20b -> qwen/qwen3.8-27b
    pub fn select_groq_model(
        &mut self,
        preferred_model: Option<&str>,
    ) -> Result<String, String> {
        let now_ms = now_millis();
        for tracker in self.trackers.values_mut() {
            tracker.prune(now_ms);
        }

        // Try preferred model first if valid
        if let Some(pref) = preferred_model {
            if let Some(tracker) = self.trackers.get(pref) {
                let is_cooling = tracker.temporary_api_cooldown_until.map_or(false, |cd| now_ms < cd);
                let rem_rpd = tracker.server_remaining_rpd.unwrap_or_else(|| {
                    tracker.limits.rpd.saturating_sub(tracker.rpd_count)
                });
                let rem_tpm = tracker.server_remaining_tpm.unwrap_or_else(|| {
                    tracker.limits.tpm.saturating_sub(tracker.current_tpm())
                });
                if !is_cooling && rem_rpd > 0 && rem_tpm > 0 && tracker.current_rpm() < tracker.limits.rpm {
                    return Ok(pref.to_string());
                }
            }
        }

        // Rotate through GROQ_ROTATION_MODELS
        for &m in GROQ_ROTATION_MODELS {
            if let Some(tracker) = self.trackers.get(m) {
                let is_cooling = tracker.temporary_api_cooldown_until.map_or(false, |cd| now_ms < cd);
                let rem_rpd = tracker.server_remaining_rpd.unwrap_or_else(|| {
                    tracker.limits.rpd.saturating_sub(tracker.rpd_count)
                });
                let rem_tpm = tracker.server_remaining_tpm.unwrap_or_else(|| {
                    tracker.limits.tpm.saturating_sub(tracker.current_tpm())
                });
                if !is_cooling && rem_rpd > 0 && rem_tpm > 0 && tracker.current_rpm() < tracker.limits.rpm {
                    return Ok(m.to_string());
                }
            }
        }

        Err("All Groq models are exhausted for today (0 remaining or rate-limited). Switch to Local (Ollama) worker.".to_string())
    }

    /// Generates full snapshot event for React frontend display.
    pub fn to_quota_update_event(&mut self, active_model: &str) -> QuotaUpdateEvent {
        let now_ms = now_millis();
        for tracker in self.trackers.values_mut() {
            tracker.prune(now_ms);
        }

        let ordered_ids = [
            MODEL_PRIMARY,
            MODEL_SECONDARY,
            MODEL_TERTIARY,
            MODEL_PINNED_ADVANCED,
            GROQ_MODEL_120B,
            GROQ_MODEL_20B,
            GROQ_MODEL_QWEN,
        ];

        let mut models = Vec::new();
        for id in ordered_ids {
            if let Some(tracker) = self.trackers.get(id) {
                models.push(tracker.to_status(now_ms, 0));
            }
        }

        let (groq_combined_rpd_remaining, groq_combined_rpd_limit) = self.get_combined_groq_capacity();
        let all_groq_exhausted = self.are_all_groq_models_exhausted();

        QuotaUpdateEvent {
            active_model: active_model.to_string(),
            pinned_model: self.pinned_model.clone(),
            last_switch_reason: self.last_switch_reason.clone(),
            models,
            timestamp_ms: now_ms,
            groq_combined_rpd_remaining,
            groq_combined_rpd_limit,
            all_groq_exhausted,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum QuotaExhaustionAction {
    QueueAndRetry {
        model_id: String,
        wait_seconds: u64,
        reason: String,
    },
    AllModelsAtCapacity {
        earliest_reset_secs: u64,
        reason: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelQuotaStatus {
    pub model_id: String,
    pub display_name: String,
    pub rpm_limit: u32,
    pub rpm_consumed: u32,
    pub rpm_remaining: u32,
    pub rpm_reset_seconds: u32,

    pub tpm_limit: u32,
    pub tpm_consumed: u32,
    pub tpm_remaining: u32,
    pub tpm_reset_seconds: u32,

    pub rpd_limit: u32,
    pub rpd_consumed: u32,
    pub rpd_remaining: u32,
    pub rpd_reset_formatted: String,
    pub rpd_reset_seconds: u64,

    pub is_available: bool,
    pub threshold_80_percent_exceeded: bool,
    pub unavailability_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuotaUpdateEvent {
    pub active_model: String,
    pub pinned_model: Option<String>,
    pub last_switch_reason: Option<String>,
    pub models: Vec<ModelQuotaStatus>,
    pub timestamp_ms: u64,
    #[serde(default)]
    pub groq_combined_rpd_remaining: u32,
    #[serde(default)]
    pub groq_combined_rpd_limit: u32,
    #[serde(default)]
    pub all_groq_exhausted: bool,
}

/// Helper: current unix timestamp in milliseconds
pub fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

/// Helper: gets date string in Pacific Time (UTC-8)
pub fn get_pacific_date_string() -> String {
    let pacific_offset = FixedOffset::west_opt(8 * 3600).unwrap();
    let utc_now = Utc::now();
    let pacific_now: DateTime<FixedOffset> = utc_now.with_timezone(&pacific_offset);
    pacific_now.format("%Y-%m-%d").to_string()
}

/// Helper: calculates remaining seconds until midnight Pacific Time
pub fn get_seconds_until_midnight_pacific() -> (u64, String) {
    let pacific_offset = FixedOffset::west_opt(8 * 3600).unwrap();
    let utc_now = Utc::now();
    let pacific_now: DateTime<FixedOffset> = utc_now.with_timezone(&pacific_offset);

    let next_midnight = (pacific_now.date_naive() + Duration::days(1))
        .and_hms_opt(0, 0, 0)
        .unwrap();
    let next_midnight_dt = DateTime::<FixedOffset>::from_naive_utc_and_offset(
        next_midnight - Duration::hours(8),
        pacific_offset,
    );

    let diff = next_midnight_dt.signed_duration_since(pacific_now);
    let secs = diff.num_seconds().max(0) as u64;

    let hours = secs / 3600;
    let mins = (secs % 3600) / 60;
    let formatted = format!("{hours}h {mins}m (Midnight PT)");

    (secs, formatted)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_initial_model_is_primary() {
        let mut tracker = ModelQuotaTracker::new();
        let (selected, reason) = tracker.select_model_for_request(100).expect("Should select primary");
        assert_eq!(selected, MODEL_PRIMARY);
        assert!(reason.contains("Primary planner"));
    }

    #[test]
    fn test_80_percent_threshold_triggers_switch_to_secondary() {
        let mut tracker = ModelQuotaTracker::new();
        let now_ms = now_millis();

        // 80% of 15 RPM is 12 requests
        for _ in 0..12 {
            tracker.trackers.get_mut(MODEL_PRIMARY).unwrap().record_actual_usage(now_ms, 100);
        }

        let (selected, reason) = tracker.select_model_for_request(100).expect("Should failover to secondary");
        assert_eq!(selected, MODEL_SECONDARY);
        assert!(reason.contains("Auto-switched to gemini-3.1-flash-lite"));
    }

    #[test]
    fn test_80_percent_threshold_triggers_switch_to_tertiary() {
        let mut tracker = ModelQuotaTracker::new();
        let now_ms = now_millis();

        // Push primary and secondary past 80% (>= 12 requests)
        for _ in 0..12 {
            tracker.trackers.get_mut(MODEL_PRIMARY).unwrap().record_actual_usage(now_ms, 100);
            tracker.trackers.get_mut(MODEL_SECONDARY).unwrap().record_actual_usage(now_ms, 100);
        }

        let (selected, reason) = tracker.select_model_for_request(100).expect("Should failover to tertiary");
        assert_eq!(selected, MODEL_TERTIARY);
        assert!(reason.contains("Auto-switched to Gemma-4-31b"));
    }

    #[test]
    fn test_pinned_model_bypasses_cascade() {
        let mut tracker = ModelQuotaTracker::new();
        tracker.set_pinned_model(Some(MODEL_PINNED_ADVANCED.to_string()));

        let (selected, reason) = tracker.select_model_for_request(100).expect("Should use pinned model");
        assert_eq!(selected, MODEL_PINNED_ADVANCED);
        assert!(reason.contains("manually pinned"));
    }

    #[test]
    fn test_all_exhausted_returns_queue_action() {
        let mut tracker = ModelQuotaTracker::new();
        let now_ms = now_millis();

        // Saturate primary (15 RPM), secondary (15 RPM), tertiary (30 RPM)
        for _ in 0..15 {
            tracker.trackers.get_mut(MODEL_PRIMARY).unwrap().record_actual_usage(now_ms, 100);
            tracker.trackers.get_mut(MODEL_SECONDARY).unwrap().record_actual_usage(now_ms, 100);
        }
        for _ in 0..30 {
            tracker.trackers.get_mut(MODEL_TERTIARY).unwrap().record_actual_usage(now_ms, 100);
        }

        let result = tracker.select_model_for_request(100);
        assert!(matches!(result, Err(QuotaExhaustionAction::AllModelsAtCapacity { .. })));
    }

    #[test]
    fn test_unavailable_primary_fails_over_to_secondary() {
        let mut tracker = ModelQuotaTracker::new();
        tracker.trip_model_unavailable(MODEL_PRIMARY, 120_000, "HTTP 503 Service Unavailable");

        let (selected, reason) = tracker.select_model_for_request(100).expect("Should failover to secondary");
        assert_eq!(selected, MODEL_SECONDARY);
        assert!(reason.contains("gemini-3.1-flash-lite"));
        assert!(reason.contains("HTTP 503"));
    }

    #[test]
    fn test_multiple_unavailable_models_fail_over_to_tertiary() {
        let mut tracker = ModelQuotaTracker::new();
        tracker.trip_model_unavailable(MODEL_PRIMARY, 120_000, "Network timeout");
        tracker.trip_model_unavailable(MODEL_SECONDARY, 120_000, "HTTP 500 Internal Error");

        let (selected, reason) = tracker.select_model_for_request(100).expect("Should failover to tertiary");
        assert_eq!(selected, MODEL_TERTIARY);
        assert!(reason.contains("Gemma-4-31b"));
    }

    #[test]
    fn test_pinned_model_unavailable_does_not_cascade() {
        let mut tracker = ModelQuotaTracker::new();
        tracker.set_pinned_model(Some(MODEL_PINNED_ADVANCED.to_string()));
        tracker.trip_model_unavailable(MODEL_PINNED_ADVANCED, 60_000, "HTTP 503");

        let result = tracker.select_model_for_request(100);
        assert!(matches!(result, Err(QuotaExhaustionAction::QueueAndRetry { model_id, .. }) if model_id == MODEL_PINNED_ADVANCED));
    }

    #[test]
    fn test_three_rapid_successive_planning_requests() {
        let mut tracker = ModelQuotaTracker::new();
        let now_ms = now_millis();

        // 1. Initial state
        let status_0 = tracker.to_quota_update_event(MODEL_PRIMARY);
        let primary_0 = status_0.models.iter().find(|m| m.model_id == MODEL_PRIMARY).unwrap();
        assert_eq!(primary_0.rpm_consumed, 0);
        assert_eq!(primary_0.tpm_consumed, 0);
        assert_eq!(primary_0.rpd_consumed, 0);

        // Request 1
        let (m1, _) = tracker.select_model_for_request(250).unwrap();
        assert_eq!(m1, MODEL_PRIMARY);
        tracker.trackers.get_mut(MODEL_PRIMARY).unwrap().record_actual_usage(now_ms, 250);
        let status_1 = tracker.to_quota_update_event(MODEL_PRIMARY);
        let p1 = status_1.models.iter().find(|m| m.model_id == MODEL_PRIMARY).unwrap();
        assert_eq!(p1.rpm_consumed, 1);
        assert_eq!(p1.tpm_consumed, 250);
        assert_eq!(p1.rpd_consumed, 1);

        // Request 2
        let (m2, _) = tracker.select_model_for_request(300).unwrap();
        assert_eq!(m2, MODEL_PRIMARY);
        tracker.trackers.get_mut(MODEL_PRIMARY).unwrap().record_actual_usage(now_ms + 100, 300);
        let status_2 = tracker.to_quota_update_event(MODEL_PRIMARY);
        let p2 = status_2.models.iter().find(|m| m.model_id == MODEL_PRIMARY).unwrap();
        assert_eq!(p2.rpm_consumed, 2);
        assert_eq!(p2.tpm_consumed, 550);
        assert_eq!(p2.rpd_consumed, 2);

        // Request 3
        let (m3, _) = tracker.select_model_for_request(450).unwrap();
        assert_eq!(m3, MODEL_PRIMARY);
        tracker.trackers.get_mut(MODEL_PRIMARY).unwrap().record_actual_usage(now_ms + 200, 450);
        let status_3 = tracker.to_quota_update_event(MODEL_PRIMARY);
        let p3 = status_3.models.iter().find(|m| m.model_id == MODEL_PRIMARY).unwrap();
        assert_eq!(p3.rpm_consumed, 3);
        assert_eq!(p3.tpm_consumed, 1000);
        assert_eq!(p3.rpd_consumed, 3);

        // Confirm all 3 requests routed only to free-tier primary model (gemini-3.5-flash-lite), zero paid models
        assert_eq!(m1, "gemini-3.5-flash-lite");
        assert_eq!(m2, "gemini-3.5-flash-lite");
        assert_eq!(m3, "gemini-3.5-flash-lite");
    }

    #[test]
    fn test_advisory_quota_exhaustion_blocks_action_and_displays_clear_message() {
        let mut tracker = ModelQuotaTracker::new();
        let now_ms = now_millis();

        // 1. Initial State: Quota is available -> button is active
        let initial_check = tracker.select_model_for_request(200);
        assert!(initial_check.is_ok());
        let (model, _) = initial_check.unwrap();
        assert_eq!(model, MODEL_PRIMARY);

        // 2. Saturate all models in cascade to simulate quota exhaustion
        for _ in 0..15 {
            tracker.trackers.get_mut(MODEL_PRIMARY).unwrap().record_actual_usage(now_ms, 100);
            tracker.trackers.get_mut(MODEL_SECONDARY).unwrap().record_actual_usage(now_ms, 100);
        }
        for _ in 0..30 {
            tracker.trackers.get_mut(MODEL_TERTIARY).unwrap().record_actual_usage(now_ms, 100);
            tracker.trackers.get_mut(MODEL_PINNED_ADVANCED).unwrap().record_actual_usage(now_ms, 100);
        }
        for &gm in GROQ_ROTATION_MODELS {
            for _ in 0..30 {
                tracker.trackers.get_mut(gm).unwrap().record_actual_usage(now_ms, 100);
            }
        }

        // 3. Selection must be rejected with QuotaExhaustionAction
        let exhausted_res = tracker.select_model_for_request(200);
        assert!(exhausted_res.is_err());
        match exhausted_res.unwrap_err() {
            QuotaExhaustionAction::AllModelsAtCapacity { reason, .. } => {
                assert!(reason.contains("capacity") || reason.contains("RPM") || reason.contains("TPM") || reason.contains("RPD"));
            }
            QuotaExhaustionAction::QueueAndRetry { reason, .. } => {
                assert!(!reason.is_empty());
            }
        }

        // 4. Projection to UI QuotaUpdateEvent: all models project is_available: false
        let update_event = tracker.to_quota_update_event(MODEL_PRIMARY);
        let all_unavailable = update_event.models.iter().all(|m| !m.is_available);
        assert!(all_unavailable, "All models must project is_available: false when saturated");
    }

    #[test]
    fn test_groq_quota_update_from_headers_and_rotation() {
        let mut tracker = ModelQuotaTracker::new();
        let now_ms = now_millis();

        // Check initial combined capacity: 3 models * 1000 = 3000 RPD
        let (rem, total) = tracker.get_combined_groq_capacity();
        assert_eq!(total, 3000);
        assert_eq!(rem, 3000);
        assert!(!tracker.are_all_groq_models_exhausted());

        // Select initial model -> should be first in rotation
        let selected = tracker.select_groq_model(None).unwrap();
        assert_eq!(selected, GROQ_MODEL_120B);

        // Ingest real headers from Groq API for 120B
        let headers = GroqRateLimitHeaders {
            retry_after_secs: None,
            limit_requests_rpd: Some(1000),
            remaining_requests_rpd: Some(999),
            limit_tokens_tpm: Some(8000),
            remaining_tokens_tpm: Some(7913),
            reset_requests_secs: Some(86),
            reset_tokens_secs: Some(1),
        };
        tracker.update_groq_server_headers(GROQ_MODEL_120B, &headers);

        let (rem2, _) = tracker.get_combined_groq_capacity();
        assert_eq!(rem2, 2999);

        // Simulate 429 on 120B with cooldown
        tracker.trackers.get_mut(GROQ_MODEL_120B).unwrap().trip_cooldown(now_ms, 60_000);

        // Next select should rotate to 20B
        let next_model = tracker.select_groq_model(None).unwrap();
        assert_eq!(next_model, GROQ_MODEL_20B);

        // Simulate 429 on 20B with cooldown
        tracker.trackers.get_mut(GROQ_MODEL_20B).unwrap().trip_cooldown(now_ms, 60_000);

        // Next select should rotate to Qwen
        let third_model = tracker.select_groq_model(None).unwrap();
        assert_eq!(third_model, GROQ_MODEL_QWEN);

        // Simulate 429 on Qwen with cooldown
        tracker.trackers.get_mut(GROQ_MODEL_QWEN).unwrap().trip_cooldown(now_ms, 60_000);

        // All 3 are now exhausted / cooling down
        assert!(tracker.are_all_groq_models_exhausted());
        let err = tracker.select_groq_model(None).unwrap_err();
        assert!(err.contains("Switch to Local (Ollama) worker"));
    }
}

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

pub const CASCADE_PRIORITY: &[&str] = &[
    MODEL_PRIMARY,
    MODEL_SECONDARY,
    MODEL_TERTIARY,
];

/// Specification limits from the source-of-truth spreadsheet.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct ModelLimits {
    pub rpm: u32,
    pub tpm: u32,
    pub rpd: u32,
}

impl ModelLimits {
    pub const fn for_model(id: &str) -> Self {
        if match_str(id, MODEL_PRIMARY) {
            Self { rpm: 15, tpm: 250_000, rpd: 500 }
        } else if match_str(id, MODEL_SECONDARY) {
            Self { rpm: 15, tpm: 250_000, rpd: 500 }
        } else if match_str(id, MODEL_TERTIARY) {
            Self { rpm: 30, tpm: 16_000, rpd: 14_400 }
        } else if match_str(id, MODEL_PINNED_ADVANCED) {
            Self { rpm: 5, tpm: 250_000, rpd: 20 }
        } else {
            // Default conservative fallback
            Self { rpm: 15, tpm: 250_000, rpd: 500 }
        }
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
        let tpm_rem = self.limits.tpm.saturating_sub(tpm);
        let rpd_rem = self.limits.rpd.saturating_sub(rpd);

        let rpm_reset_secs = self.seconds_until_sliding_release(now_ms) as u32;
        let tpm_reset_secs = rpm_reset_secs;

        let (rpd_reset_secs, rpd_reset_formatted) = get_seconds_until_midnight_pacific();

        let is_below_80 = self.is_below_80_percent(now_ms, estimated_tokens).is_ok();
        let is_on_cooldown = self.temporary_api_cooldown_until.map_or(false, |cd| now_ms < cd);
        let is_hard_exhausted = rpm >= self.limits.rpm || tpm >= self.limits.tpm || rpd >= self.limits.rpd;

        ModelQuotaStatus {
            model_id: self.model_id.clone(),
            display_name: self.display_name.clone(),
            rpm_limit: self.limits.rpm,
            rpm_consumed: rpm,
            rpm_remaining: rpm_rem,
            rpm_reset_seconds: rpm_reset_secs,

            tpm_limit: self.limits.tpm,
            tpm_consumed: tpm,
            tpm_remaining: tpm_rem,
            tpm_reset_seconds: tpm_reset_secs,

            rpd_limit: self.limits.rpd,
            rpd_consumed: rpd,
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
        ];

        let mut models = Vec::new();
        for id in ordered_ids {
            if let Some(tracker) = self.trackers.get(id) {
                models.push(tracker.to_status(now_ms, 0));
            }
        }

        QuotaUpdateEvent {
            active_model: active_model.to_string(),
            pinned_model: self.pinned_model.clone(),
            last_switch_reason: self.last_switch_reason.clone(),
            models,
            timestamp_ms: now_ms,
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
}

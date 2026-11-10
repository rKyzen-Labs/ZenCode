use zencode_core::CredentialService;

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

fn parse_duration_string_to_seconds(raw: &str) -> Option<u64> {
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

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let key = CredentialService::get_groq_api_key()?;
    println!("Retrieved Groq Key from Windows Credential Manager: {}...{}", &key[..4], &key[key.len() - 4..]);

    let client = reqwest::Client::new();
    let resp = client
        .post("https://api.groq.com/openai/v1/chat/completions")
        .header("Authorization", format!("Bearer {}", key))
        .header("Content-Type", "application/json")
        .json(&serde_json::json!({
            "model": "openai/gpt-oss-120b",
            "messages": [
                { "role": "user", "content": "respond with 'pong' only" }
            ],
            "max_tokens": 10
        }))
        .send()
        .await?;

    println!("HTTP Status: {}", resp.status());
    println!("\n=== RAW GROQ RATE-LIMIT HEADERS RECEIVED ===");
    for (k, v) in resp.headers() {
        if k.as_str().contains("ratelimit") || k.as_str().contains("retry") {
            println!("{}: {:?}", k, v);
        }
    }

    let parsed = parse_groq_headers(resp.headers());
    println!("\n=== PARSED GROUND-TRUTH RATE LIMIT VALUES ===");
    println!("RPD Limit (x-ratelimit-limit-requests): {:?}", parsed.limit_requests_rpd);
    println!("RPD Remaining (x-ratelimit-remaining-requests): {:?}", parsed.remaining_requests_rpd);
    println!("TPM Limit (x-ratelimit-limit-tokens): {:?}", parsed.limit_tokens_tpm);
    println!("TPM Remaining (x-ratelimit-remaining-tokens): {:?}", parsed.remaining_tokens_tpm);
    println!("RPD Reset Countdown (x-ratelimit-reset-requests): {:?} seconds", parsed.reset_requests_secs);
    println!("TPM Reset Countdown (x-ratelimit-reset-tokens): {:?} seconds", parsed.reset_tokens_secs);

    let combined_capacity = parsed.remaining_requests_rpd.unwrap_or(1000) + 1000 + 1000;
    println!("\nCombined 3-Model Groq Capacity: {}/3000 RPD", combined_capacity);
    Ok(())
}

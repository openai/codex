use crate::protocol::RateLimitSnapshot;
use crate::protocol::RateLimitWindow;
use chrono::Utc;
use reqwest::header::HeaderMap;

/// Prefer Codex-specific aggregate rate limit headers if present; fall back
/// to raw OpenAI-style request headers otherwise.
pub(crate) fn parse_rate_limit_snapshot(headers: &HeaderMap) -> Option<RateLimitSnapshot> {
    parse_codex_rate_limits(headers).or_else(|| parse_openai_rate_limits(headers))
}

fn parse_codex_rate_limits(headers: &HeaderMap) -> Option<RateLimitSnapshot> {
    fn parse_f64(headers: &HeaderMap, name: &str) -> Option<f64> {
        headers
            .get(name)
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.parse::<f64>().ok())
    }

    fn parse_i64(headers: &HeaderMap, name: &str) -> Option<i64> {
        headers
            .get(name)
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.parse::<i64>().ok())
    }

    let primary_used = parse_f64(headers, "x-codex-primary-used-percent");
    let secondary_used = parse_f64(headers, "x-codex-secondary-used-percent");

    if primary_used.is_none() && secondary_used.is_none() {
        return None;
    }

    let primary = primary_used.map(|used_percent| RateLimitWindow {
        used_percent,
        window_minutes: parse_i64(headers, "x-codex-primary-window-minutes"),
        resets_at: parse_i64(headers, "x-codex-primary-reset-at"),
    });

    let secondary = secondary_used.map(|used_percent| RateLimitWindow {
        used_percent,
        window_minutes: parse_i64(headers, "x-codex-secondary-window-minutes"),
        resets_at: parse_i64(headers, "x-codex-secondary-reset-at"),
    });

    Some(RateLimitSnapshot { primary, secondary })
}

fn parse_openai_rate_limits(headers: &HeaderMap) -> Option<RateLimitSnapshot> {
    let limit = headers.get("x-ratelimit-limit-requests")?;
    let remaining = headers.get("x-ratelimit-remaining-requests")?;
    let reset_ms = headers.get("x-ratelimit-reset-requests")?;

    let limit = limit.to_str().ok()?.parse::<f64>().ok()?;
    let remaining = remaining.to_str().ok()?.parse::<f64>().ok()?;
    let reset_ms = reset_ms.to_str().ok()?.parse::<i64>().ok()?;

    if limit <= 0.0 {
        return None;
    }

    let used = (limit - remaining).max(0.0);
    let used_percent = (used / limit) * 100.0;

    let window_minutes = if reset_ms <= 0 {
        None
    } else {
        let seconds = reset_ms / 1000;
        Some((seconds + 59) / 60)
    };

    let resets_at = if reset_ms > 0 {
        Some(Utc::now().timestamp() + reset_ms / 1000)
    } else {
        None
    };

    Some(RateLimitSnapshot {
        primary: Some(RateLimitWindow {
            used_percent,
            window_minutes,
            resets_at,
        }),
        secondary: None,
    })
}

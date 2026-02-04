use codex_protocol::account::PlanType;
use codex_protocol::protocol::CreditsSnapshot;
use codex_protocol::protocol::RateLimitSnapshot;
use codex_protocol::protocol::RateLimitWindow;
use http::HeaderMap;
use serde::Deserialize;
use std::fmt::Display;

#[derive(Debug)]
pub struct RateLimitError {
    pub message: String,
}

impl Display for RateLimitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

/// Parses the bespoke Codex rate-limit headers into a `RateLimitSnapshot`.
pub fn parse_rate_limit(headers: &HeaderMap) -> Option<RateLimitSnapshot> {
    let primary = parse_rate_limit_window(
        headers,
        "x-codex-primary-used-percent",
        "x-codex-primary-window-minutes",
        "x-codex-primary-reset-at",
    );

    let secondary = parse_rate_limit_window(
        headers,
        "x-codex-secondary-used-percent",
        "x-codex-secondary-window-minutes",
        "x-codex-secondary-reset-at",
    );

    let credits = parse_credits_snapshot(headers);

    Some(RateLimitSnapshot {
        primary,
        secondary,
        credits,
        plan_type: None,
    })
}

#[derive(Debug, Deserialize)]
struct RateLimitEventWindow {
    used_percent: f64,
    window_minutes: Option<i64>,
    reset_at: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct RateLimitEventDetails {
    primary: Option<RateLimitEventWindow>,
    secondary: Option<RateLimitEventWindow>,
}

#[derive(Debug, Deserialize)]
struct RateLimitEventCredits {
    has_credits: bool,
    unlimited: bool,
    balance: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RateLimitEvent {
    #[serde(rename = "type")]
    kind: String,
    plan_type: Option<PlanType>,
    rate_limits: Option<RateLimitEventDetails>,
    credits: Option<RateLimitEventCredits>,
}

pub fn parse_rate_limit_event(payload: &str) -> Option<RateLimitSnapshot> {
    let event: RateLimitEvent = serde_json::from_str(payload).ok()?;
    if event.kind != "codex.rate_limits" {
        return None;
    }
    let (primary, secondary) = if let Some(details) = event.rate_limits.as_ref() {
        (
            map_event_window(details.primary.as_ref()),
            map_event_window(details.secondary.as_ref()),
        )
    } else {
        (None, None)
    };
    let credits = event.credits.map(|credits| CreditsSnapshot {
        has_credits: credits.has_credits,
        unlimited: credits.unlimited,
        balance: credits.balance,
    });
    Some(RateLimitSnapshot {
        primary,
        secondary,
        credits,
        plan_type: event.plan_type,
    })
}

fn map_event_window(window: Option<&RateLimitEventWindow>) -> Option<RateLimitWindow> {
    let window = window?;
    Some(RateLimitWindow {
        used_percent: window.used_percent,
        window_minutes: window.window_minutes,
        resets_at: window.reset_at,
    })
}

/// Parses the bespoke Codex rate-limit headers into a `RateLimitSnapshot`.
pub fn parse_promo_message(headers: &HeaderMap) -> Option<String> {
    parse_header_str(headers, "x-codex-promo-message")
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(std::string::ToString::to_string)
}

fn parse_rate_limit_window(
    headers: &HeaderMap,
    used_percent_header: &str,
    window_minutes_header: &str,
    resets_at_header: &str,
) -> Option<RateLimitWindow> {
    let used_percent: Option<f64> = parse_header_f64(headers, used_percent_header);

    used_percent.and_then(|used_percent| {
        let window_minutes = parse_header_i64(headers, window_minutes_header);
        let resets_at = parse_header_i64(headers, resets_at_header);

        let has_data = used_percent != 0.0
            || window_minutes.is_some_and(|minutes| minutes != 0)
            || resets_at.is_some();

        has_data.then_some(RateLimitWindow {
            used_percent,
            window_minutes,
            resets_at,
        })
    })
}

fn parse_credits_snapshot(headers: &HeaderMap) -> Option<CreditsSnapshot> {
    let has_credits = parse_header_bool(headers, "x-codex-credits-has-credits")?;
    let unlimited = parse_header_bool(headers, "x-codex-credits-unlimited")?;
    let balance = parse_header_str(headers, "x-codex-credits-balance")
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(std::string::ToString::to_string);
    Some(CreditsSnapshot {
        has_credits,
        unlimited,
        balance,
    })
}

fn parse_header_f64(headers: &HeaderMap, name: &str) -> Option<f64> {
    parse_header_str(headers, name)?
        .parse::<f64>()
        .ok()
        .filter(|v| v.is_finite())
}

fn parse_header_i64(headers: &HeaderMap, name: &str) -> Option<i64> {
    parse_header_str(headers, name)?.parse::<i64>().ok()
}

fn parse_header_bool(headers: &HeaderMap, name: &str) -> Option<bool> {
    let raw = parse_header_str(headers, name)?;
    if raw.eq_ignore_ascii_case("true") || raw == "1" {
        Some(true)
    } else if raw.eq_ignore_ascii_case("false") || raw == "0" {
        Some(false)
    } else {
        None
    }
}

fn parse_header_str<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a str> {
    headers.get(name)?.to_str().ok()
}

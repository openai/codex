use codex_protocol::account::PlanType;
use codex_protocol::protocol::CreditsSnapshot;
use codex_protocol::protocol::RateLimitSnapshot;
use codex_protocol::protocol::RateLimitWindow;
use http::HeaderMap;
use serde::Deserialize;
use std::collections::BTreeSet;
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

/// Parses the default Codex rate-limit header family into a `RateLimitSnapshot`.
pub fn parse_default_rate_limit(headers: &HeaderMap) -> Option<RateLimitSnapshot> {
    parse_rate_limit_for_limit(headers, None)
}

/// Parses all known rate-limit header families into update records keyed by limit id.
pub fn parse_all_rate_limits(headers: &HeaderMap) -> Vec<RateLimitSnapshot> {
    let mut snapshots = Vec::new();
    if let Some(snapshot) = parse_default_rate_limit(headers) {
        snapshots.push(snapshot);
    }

    let mut limit_ids: BTreeSet<String> = BTreeSet::new();

    for name in headers.keys() {
        let header_name = name.as_str().to_ascii_lowercase();
        if let Some(limit_id) = header_name_to_limit_id(&header_name)
            && limit_id != "codex"
        {
            limit_ids.insert(limit_id);
        }
    }

    snapshots.extend(limit_ids.into_iter().filter_map(|limit_id| {
        let snapshot = parse_rate_limit_for_limit(headers, Some(limit_id.as_str()))?;
        has_rate_limit_data(&snapshot).then_some(snapshot)
    }));

    snapshots
}

/// Parses rate-limit headers for the provided limit id.
///
/// `limit_id` should match the server-provided metered limit id (e.g. `codex`,
/// `codex_other`). When omitted, this defaults to the legacy `codex` header family.
pub fn parse_rate_limit_for_limit(
    headers: &HeaderMap,
    limit_id: Option<&str>,
) -> Option<RateLimitSnapshot> {
    let normalized_limit = limit_id
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .unwrap_or("codex")
        .to_ascii_lowercase()
        .replace('_', "-");
    let prefix = format!("x-{normalized_limit}");
    let primary = parse_rate_limit_window(
        headers,
        &format!("{prefix}-primary-used-percent"),
        &format!("{prefix}-primary-window-minutes"),
        &format!("{prefix}-primary-reset-at"),
    );

    let secondary = parse_rate_limit_window(
        headers,
        &format!("{prefix}-secondary-used-percent"),
        &format!("{prefix}-secondary-window-minutes"),
        &format!("{prefix}-secondary-reset-at"),
    );

    let normalized_limit_id = normalize_limit_id(normalized_limit);
    let credits = parse_credits_snapshot(headers);
    let limit_name_header = format!("{prefix}-limit-name");
    let parsed_limit_name = parse_header_str(headers, &limit_name_header)
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(std::string::ToString::to_string);

    Some(RateLimitSnapshot {
        limit_id: Some(normalized_limit_id),
        limit_name: parsed_limit_name,
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
    metered_limit_name: Option<String>,
    limit_name: Option<String>,
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
    let limit_id = event
        .metered_limit_name
        .or(event.limit_name)
        .map(normalize_limit_id);
    Some(RateLimitSnapshot {
        limit_id: Some(limit_id.unwrap_or_else(|| "codex".to_string())),
        limit_name: None,
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

fn has_rate_limit_data(snapshot: &RateLimitSnapshot) -> bool {
    snapshot.primary.is_some() || snapshot.secondary.is_some() || snapshot.credits.is_some()
}

fn header_name_to_limit_id(header_name: &str) -> Option<String> {
    let suffix = "-primary-used-percent";
    let prefix = header_name.strip_suffix(suffix)?;
    let limit = prefix.strip_prefix("x-")?;
    Some(normalize_limit_id(limit.to_string()))
}

fn normalize_limit_id(name: impl Into<String>) -> String {
    name.into().trim().to_ascii_lowercase().replace('-', "_")
}

#[cfg(test)]
mod tests {
    use super::*;
    use http::HeaderValue;
    use pretty_assertions::assert_eq;

    #[test]
    fn parse_rate_limit_for_limit_defaults_to_codex_headers() {
        let mut headers = HeaderMap::new();
        headers.insert(
            "x-codex-primary-used-percent",
            HeaderValue::from_static("12.5"),
        );
        headers.insert(
            "x-codex-primary-window-minutes",
            HeaderValue::from_static("60"),
        );
        headers.insert(
            "x-codex-primary-reset-at",
            HeaderValue::from_static("1704069000"),
        );

        let snapshot = parse_rate_limit_for_limit(&headers, None).expect("snapshot");
        assert_eq!(snapshot.limit_id.as_deref(), Some("codex"));
        assert_eq!(snapshot.limit_name, None);
        let primary = snapshot.primary.expect("primary");
        assert_eq!(primary.used_percent, 12.5);
        assert_eq!(primary.window_minutes, Some(60));
        assert_eq!(primary.resets_at, Some(1704069000));
    }

    #[test]
    fn parse_rate_limit_for_limit_reads_secondary_headers() {
        let mut headers = HeaderMap::new();
        headers.insert(
            "x-codex-secondary-primary-used-percent",
            HeaderValue::from_static("80"),
        );
        headers.insert(
            "x-codex-secondary-primary-window-minutes",
            HeaderValue::from_static("1440"),
        );
        headers.insert(
            "x-codex-secondary-primary-reset-at",
            HeaderValue::from_static("1704074400"),
        );

        let snapshot =
            parse_rate_limit_for_limit(&headers, Some("codex_secondary")).expect("snapshot");
        assert_eq!(snapshot.limit_id.as_deref(), Some("codex_secondary"));
        assert_eq!(snapshot.limit_name, None);
        let primary = snapshot.primary.expect("primary");
        assert_eq!(primary.used_percent, 80.0);
        assert_eq!(primary.window_minutes, Some(1440));
        assert_eq!(primary.resets_at, Some(1704074400));
        assert_eq!(snapshot.secondary, None);
    }

    #[test]
    fn parse_rate_limit_for_limit_prefers_limit_name_header() {
        let mut headers = HeaderMap::new();
        headers.insert(
            "x-codex-bengalfox-primary-used-percent",
            HeaderValue::from_static("80"),
        );
        headers.insert(
            "x-codex-bengalfox-limit-name",
            HeaderValue::from_static("gpt-5.2-codex-sonic"),
        );

        let snapshot =
            parse_rate_limit_for_limit(&headers, Some("codex_bengalfox")).expect("snapshot");
        assert_eq!(snapshot.limit_id.as_deref(), Some("codex_bengalfox"));
        assert_eq!(snapshot.limit_name.as_deref(), Some("gpt-5.2-codex-sonic"));
    }

    #[test]
    fn parse_all_rate_limits_reads_all_limit_families() {
        let mut headers = HeaderMap::new();
        headers.insert(
            "x-codex-primary-used-percent",
            HeaderValue::from_static("12.5"),
        );
        headers.insert(
            "x-codex-secondary-primary-used-percent",
            HeaderValue::from_static("80"),
        );

        let updates = parse_all_rate_limits(&headers);
        assert_eq!(updates.len(), 2);
        assert_eq!(updates[0].limit_id.as_deref(), Some("codex"));
        assert_eq!(updates[1].limit_id.as_deref(), Some("codex_secondary"));
        assert_eq!(updates[0].limit_name, None);
        assert_eq!(updates[1].limit_name, None);
    }

    #[test]
    fn parse_all_rate_limits_includes_default_codex_snapshot() {
        let headers = HeaderMap::new();

        let updates = parse_all_rate_limits(&headers);
        assert_eq!(updates.len(), 1);
        assert_eq!(updates[0].limit_id.as_deref(), Some("codex"));
        assert_eq!(updates[0].limit_name, None);
        assert_eq!(updates[0].primary, None);
        assert_eq!(updates[0].secondary, None);
        assert_eq!(updates[0].credits, None);
    }
}

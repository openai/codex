use codex_protocol::protocol::RateLimitSnapshot;
use codex_protocol::protocol::RateLimitWindow;
use reqwest::header::HeaderMap;

pub fn parse_rate_limit_snapshot(headers: &HeaderMap) -> Option<RateLimitSnapshot> {
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

    if primary.is_none() && secondary.is_none() {
        return None;
    }

    Some(RateLimitSnapshot { primary, secondary })
}

fn parse_rate_limit_window(
    headers: &HeaderMap,
    used_percent_header: &str,
    window_minutes_header: &str,
    resets_at_header: &str,
) -> Option<RateLimitWindow> {
    let used_percent: f64 = parse_header_f64(headers, used_percent_header)?;
    let window_minutes = parse_header_i64(headers, window_minutes_header);
    let resets_at = parse_header_i64(headers, resets_at_header);

    Some(RateLimitWindow {
        used_percent,
        window_minutes,
        resets_at,
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

fn parse_header_str<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a str> {
    headers.get(name)?.to_str().ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use reqwest::header::HeaderValue;

    #[test]
    fn snapshot_includes_zero_percent_values() {
        let mut headers = HeaderMap::new();
        headers.insert(
            "x-codex-primary-used-percent",
            HeaderValue::from_static("0.0"),
        );
        let snapshot = parse_rate_limit_snapshot(&headers).expect("snapshot should exist");
        assert_eq!(snapshot.primary.unwrap().used_percent, 0.0);
    }

    #[test]
    fn missing_headers_return_none() {
        let headers = HeaderMap::new();
        assert!(parse_rate_limit_snapshot(&headers).is_none());
    }
}

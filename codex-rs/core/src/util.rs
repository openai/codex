use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;

use rand::Rng;
use tracing::debug;
use tracing::error;

const INITIAL_DELAY_MS: u64 = 200;
const BACKOFF_FACTOR: f64 = 2.0;

pub(crate) fn backoff(attempt: u64) -> Duration {
    let exp = BACKOFF_FACTOR.powi(attempt.saturating_sub(1) as i32);
    let base = (INITIAL_DELAY_MS as f64 * exp) as u64;
    let jitter = rand::rng().random_range(0.9..1.1);
    Duration::from_millis((base as f64 * jitter) as u64)
}

pub(crate) fn parse_retry_after_hint(message: &str) -> Option<Duration> {
    let lower = message.to_ascii_lowercase();
    let (idx, prefix_len) = if let Some(idx) = lower.find("try again in") {
        (idx, "try again in".len())
    } else if let Some(idx) = lower.find("retry after") {
        (idx, "retry after".len())
    } else {
        return None;
    };

    let mut rest = message.get(idx + prefix_len..)?.trim_start();
    if rest.is_empty() {
        return None;
    }

    let num_len = rest
        .chars()
        .take_while(|ch| ch.is_ascii_digit() || *ch == '.')
        .map(char::len_utf8)
        .sum::<usize>();
    if num_len == 0 {
        return None;
    }

    let raw_value = rest.get(..num_len)?;
    let value = raw_value.parse::<f64>().ok()?;
    rest = rest.get(num_len..)?.trim_start();

    let unit_len = rest
        .chars()
        .take_while(char::is_ascii_alphabetic)
        .map(char::len_utf8)
        .sum::<usize>();
    let unit = rest
        .get(..unit_len)
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase();

    if unit == "ms" {
        return Some(Duration::from_millis(value.max(0.0) as u64));
    }
    if unit == "s" || unit.starts_with("second") {
        return Duration::try_from_secs_f64(value).ok();
    }

    None
}

pub(crate) fn add_retry_jitter_and_buffer(delay: Duration) -> Duration {
    // Add a small random buffer to avoid immediate re-hit due to skew, and a
    // multiplicative jitter so many parallel clients don't synchronize.
    let jitter = rand::rng().random_range(0.9..1.1);
    let buffer_ms: u64 = rand::rng().random_range(200..700);

    let base_ms = delay.as_millis() as f64;
    let millis = (base_ms * jitter).max(0.0) as u64;
    Duration::from_millis(millis.saturating_add(buffer_ms))
}

pub(crate) fn error_or_panic(message: impl std::string::ToString) {
    if cfg!(debug_assertions) || env!("CARGO_PKG_VERSION").contains("alpha") {
        panic!("{}", message.to_string());
    } else {
        error!("{}", message.to_string());
    }
}

pub(crate) fn try_parse_error_message(text: &str) -> String {
    debug!("Parsing server error response: {}", text);
    let json = serde_json::from_str::<serde_json::Value>(text).unwrap_or_default();
    if let Some(error) = json.get("error")
        && let Some(message) = error.get("message")
        && let Some(message_str) = message.as_str()
    {
        return message_str.to_string();
    }
    if text.is_empty() {
        return "Unknown error".to_string();
    }
    text.to_string()
}

pub fn resolve_path(base: &Path, path: &PathBuf) -> PathBuf {
    if path.is_absolute() {
        path.clone()
    } else {
        base.join(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_try_parse_error_message() {
        let text = r#"{
  "error": {
    "message": "Your refresh token has already been used to generate a new access token. Please try signing in again.",
    "type": "invalid_request_error",
    "param": null,
    "code": "refresh_token_reused"
  }
}"#;
        let message = try_parse_error_message(text);
        assert_eq!(
            message,
            "Your refresh token has already been used to generate a new access token. Please try signing in again."
        );
    }

    #[test]
    fn test_try_parse_error_message_no_error() {
        let text = r#"{"message": "test"}"#;
        let message = try_parse_error_message(text);
        assert_eq!(message, r#"{"message": "test"}"#);
    }

    #[test]
    fn test_parse_retry_after_hint_seconds() {
        let msg = "Rate limit reached. Please try again in 11.054s. Visit ...";
        let d = parse_retry_after_hint(msg).expect("expected duration");
        assert_eq!(d, Duration::from_millis(11_054));
    }

    #[test]
    fn test_parse_retry_after_hint_ms() {
        let msg = "Rate limit reached. Please try again in 70ms. Visit ...";
        let d = parse_retry_after_hint(msg).expect("expected duration");
        assert_eq!(d, Duration::from_millis(70));
    }

    #[test]
    fn test_parse_retry_after_hint_seconds_word() {
        let msg = "Rate limit exceeded. Try again in 35 seconds.";
        let d = parse_retry_after_hint(msg).expect("expected duration");
        assert_eq!(d, Duration::from_secs(35));
    }
}

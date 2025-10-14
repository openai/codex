//! Shared helpers for configuring Codex HTTP clients with consistent
//! keepalive and idle timeout settings.

use std::time::Duration;

use reqwest::ClientBuilder;
use tracing::warn;

const DEFAULT_POOL_IDLE_TIMEOUT: Duration = Duration::from_secs(330);
const MAX_POOL_IDLE_TIMEOUT: Duration = Duration::from_secs(600);
const DEFAULT_TCP_KEEPALIVE: Duration = Duration::from_secs(30);
const DEFAULT_HTTP2_KEEPALIVE: Duration = Duration::from_secs(30);
const DEFAULT_CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const MIN_DURATION_MILLIS: u64 = 100;
const MIN_DURATION_SECS: u64 = 1;

const ENV_POOL_IDLE_MS: &str = "CODEX_HTTP_POOL_IDLE_MS";
const ENV_POOL_IDLE_SECS: &str = "CODEX_HTTP_POOL_IDLE_S";
const ENV_POOL_IDLE_TTL: &str = "CODEX_HTTP_IDLE_TTL";
const ENV_TCP_KEEPALIVE_SECS: &str = "CODEX_TCP_KEEPALIVE_SECS";
const ENV_HTTP2_KEEPALIVE_SECS: &str = "CODEX_HTTP2_KEEPALIVE_SECS";
const ENV_CONNECT_TIMEOUT_SECS: &str = "CODEX_HTTP_CONNECT_TIMEOUT_SECS";
const ENV_POOL_MAX_IDLE_PER_HOST: &str = "CODEX_HTTP_POOL_MAX_IDLE_PER_HOST";

/// Apply Codex keepalive defaults (with optional env overrides) to the
/// provided reqwest [`ClientBuilder`].
pub fn configure_builder(mut builder: ClientBuilder) -> ClientBuilder {
    let pool_idle = pool_idle_timeout();
    let tcp_keepalive = tcp_keepalive();
    let http2_interval = http2_keepalive_interval();
    let connect_timeout = connect_timeout();
    let pool_max_idle = pool_max_idle_per_host();

    builder = builder
        .pool_idle_timeout(pool_idle)
        .tcp_keepalive(Some(tcp_keepalive))
        .pool_max_idle_per_host(pool_max_idle)
        .http2_keep_alive_while_idle(true)
        .http2_keep_alive_interval(http2_interval)
        .http2_keep_alive_timeout(http2_keepalive_timeout(http2_interval));

    if let Some(timeout) = connect_timeout {
        builder = builder.connect_timeout(timeout);
    }

    builder
}

fn pool_idle_timeout() -> Option<Duration> {
    if let Some(duration) = read_env_pool_idle_secs(ENV_POOL_IDLE_TTL) {
        return Some(clamp_pool_idle(duration, ENV_POOL_IDLE_TTL));
    }

    if let Some(duration) = read_env_pool_idle_secs(ENV_POOL_IDLE_SECS) {
        return Some(clamp_pool_idle(duration, ENV_POOL_IDLE_SECS));
    }

    match read_duration_millis(
        ENV_POOL_IDLE_MS,
        DEFAULT_POOL_IDLE_TIMEOUT,
        MIN_DURATION_MILLIS,
    ) {
        Ok(duration) => Some(clamp_pool_idle(duration, "env.CODEX_HTTP_POOL_IDLE_MS")),
        Err(err) => {
            warn!(environment = ENV_POOL_IDLE_MS, "{err}");
            Some(DEFAULT_POOL_IDLE_TIMEOUT)
        }
    }
}

fn read_env_pool_idle_secs(key: &str) -> Option<Duration> {
    match std::env::var(key) {
        Ok(value) => match parse_duration_secs(&value, MIN_DURATION_SECS) {
            Ok(duration) => Some(duration),
            Err(err) => {
                warn!(environment = key, "{err}");
                None
            }
        },
        Err(std::env::VarError::NotPresent) => None,
        Err(err) => {
            warn!(environment = key, "{err}");
            None
        }
    }
}

fn clamp_pool_idle(duration: Duration, source: &str) -> Duration {
    let min = Duration::from_millis(MIN_DURATION_MILLIS);
    if duration < min {
        warn!(
            field = source,
            requested_ms = duration.as_millis(),
            min_ms = MIN_DURATION_MILLIS,
            "clamping value below minimum"
        );
        return min;
    }

    if duration > MAX_POOL_IDLE_TIMEOUT {
        warn!(
            field = source,
            requested_secs = duration.as_secs(),
            max_secs = MAX_POOL_IDLE_TIMEOUT.as_secs(),
            "clamping value above maximum"
        );
        return MAX_POOL_IDLE_TIMEOUT;
    }

    duration
}

fn tcp_keepalive() -> Duration {
    match read_duration_secs(
        ENV_TCP_KEEPALIVE_SECS,
        DEFAULT_TCP_KEEPALIVE,
        MIN_DURATION_SECS,
    ) {
        Ok(duration) => duration,
        Err(err) => {
            warn!(environment = ENV_TCP_KEEPALIVE_SECS, "{err}");
            DEFAULT_TCP_KEEPALIVE
        }
    }
}

fn http2_keepalive_interval() -> Duration {
    match read_duration_secs(
        ENV_HTTP2_KEEPALIVE_SECS,
        DEFAULT_HTTP2_KEEPALIVE,
        MIN_DURATION_SECS,
    ) {
        Ok(duration) => duration,
        Err(err) => {
            warn!(environment = ENV_HTTP2_KEEPALIVE_SECS, "{err}");
            DEFAULT_HTTP2_KEEPALIVE
        }
    }
}

fn http2_keepalive_timeout(interval: Duration) -> Duration {
    // Allow at least two missed keepalive intervals before timing out.
    let fallback = interval.saturating_mul(2);
    if fallback < Duration::from_secs(5) {
        Duration::from_secs(5)
    } else {
        fallback
    }
}

fn connect_timeout() -> Option<Duration> {
    match read_duration_secs_optional(
        ENV_CONNECT_TIMEOUT_SECS,
        Some(DEFAULT_CONNECT_TIMEOUT),
        MIN_DURATION_SECS,
    ) {
        Ok(duration) => duration,
        Err(err) => {
            warn!(environment = ENV_CONNECT_TIMEOUT_SECS, "{err}");
            Some(DEFAULT_CONNECT_TIMEOUT)
        }
    }
}

fn pool_max_idle_per_host() -> usize {
    match std::env::var(ENV_POOL_MAX_IDLE_PER_HOST) {
        Ok(value) => match value.parse::<usize>() {
            Ok(parsed) if parsed > 0 => parsed,
            Ok(_) => {
                warn!(
                    environment = ENV_POOL_MAX_IDLE_PER_HOST,
                    "expected value > 0"
                );
                1
            }
            Err(err) => {
                warn!(
                    environment = ENV_POOL_MAX_IDLE_PER_HOST,
                    "invalid usize: {err}"
                );
                1
            }
        },
        Err(std::env::VarError::NotPresent) => 1,
        Err(err) => {
            warn!(environment = ENV_POOL_MAX_IDLE_PER_HOST, "{err}");
            1
        }
    }
}

fn read_duration_millis(key: &str, default: Duration, min_millis: u64) -> Result<Duration, String> {
    match std::env::var(key) {
        Ok(value) => parse_duration_millis(&value, min_millis),
        Err(std::env::VarError::NotPresent) => Ok(default),
        Err(err) => Err(err.to_string()),
    }
}

fn read_duration_secs(key: &str, default: Duration, min_secs: u64) -> Result<Duration, String> {
    match std::env::var(key) {
        Ok(value) => parse_duration_secs(&value, min_secs),
        Err(std::env::VarError::NotPresent) => Ok(default),
        Err(err) => Err(err.to_string()),
    }
}

fn read_duration_secs_optional(
    key: &str,
    default: Option<Duration>,
    min_secs: u64,
) -> Result<Option<Duration>, String> {
    match std::env::var(key) {
        Ok(value) if value.eq_ignore_ascii_case("none") => Ok(None),
        Ok(value) => parse_duration_secs(&value, min_secs).map(Some),
        Err(std::env::VarError::NotPresent) => Ok(default),
        Err(err) => Err(err.to_string()),
    }
}

fn parse_duration_millis(value: &str, min_millis: u64) -> Result<Duration, String> {
    let millis = value
        .parse::<u64>()
        .map_err(|err| format!("invalid integer: {err}"))?;
    if millis < min_millis {
        return Err(format!("value {millis}ms is below minimum {min_millis}ms"));
    }
    Ok(Duration::from_millis(millis))
}

fn parse_duration_secs(value: &str, min_secs: u64) -> Result<Duration, String> {
    let secs = value
        .parse::<u64>()
        .map_err(|err| format!("invalid integer: {err}"))?;
    if secs < min_secs {
        return Err(format!("value {secs}s is below minimum {min_secs}s"));
    }
    Ok(Duration::from_secs(secs))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn configure_builder_builds_with_defaults() {
        let builder = configure_builder(ClientBuilder::new());
        let client = builder.build().expect("client builds");
        // No direct inspectors; ensure building succeeds and default keepalive is set via runtime options.
        drop(client);
    }

    #[test]
    fn parse_duration_helpers_accept_valid_values() {
        assert_eq!(
            parse_duration_millis("600000", MIN_DURATION_MILLIS).unwrap(),
            Duration::from_millis(600_000)
        );
        assert_eq!(
            parse_duration_secs("45", MIN_DURATION_SECS).unwrap(),
            Duration::from_secs(45)
        );
    }

    #[test]
    fn parse_duration_helpers_reject_invalid_values() {
        assert!(parse_duration_millis("50", MIN_DURATION_MILLIS).is_err());
        assert!(parse_duration_secs("0", MIN_DURATION_SECS).is_err());
    }
}

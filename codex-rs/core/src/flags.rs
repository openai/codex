use std::time::Duration;

use env_flags::env_flags;

env_flags! {
    pub OPENAI_DEFAULT_MODEL: &str = "codex-mini-latest";
    pub OPENAI_API_BASE: &str = "https://api.openai.com/v1";

    /// Fallback when the provider-specific key is not set.
    pub OPENAI_API_KEY: Option<&str> = None;
    pub OPENAI_TIMEOUT_MS: Duration = Duration::from_millis(300_000), |value| {
        value.parse().map(Duration::from_millis)
    };
    pub OPENAI_REQUEST_MAX_RETRIES: u64 = 4;
    pub OPENAI_STREAM_MAX_RETRIES: u64 = 10;

    // We generally don't want to disconnect; this updates the timeout to be five minutes
    // which matches the upstream typescript codex impl.
    pub OPENAI_STREAM_IDLE_TIMEOUT_MS: Duration = Duration::from_millis(300_000), |value| {
        value.parse().map(Duration::from_millis)
    };

    /// Fixture path for offline tests (see client.rs).
    pub CODEX_RS_SSE_FIXTURE: Option<&str> = None;
}

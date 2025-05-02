use std::time::Duration;

use env_flags::env_flags;

use crate::error::CodexErr;
use crate::error::Result;

env_flags! {
    pub OPENAI_DEFAULT_MODEL: &str = "o3";
    pub OPENAI_API_BASE: &str = "https://api.openai.com";
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

    pub CODEX_RS_SSE_FIXTURE: Option<&str> = None;
}

pub fn get_api_key() -> Result<&'static str> {
    OPENAI_API_KEY.ok_or_else(|| CodexErr::EnvVar("OPENAI_API_KEY"))
}

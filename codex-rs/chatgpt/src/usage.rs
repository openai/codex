use anyhow::Context;
use codex_core::config::Config;
use serde::Deserialize;

use crate::chatgpt_client::chatgpt_get_request;

/// High-level summary of guardrail usage for display in CLI.
#[derive(Debug, Clone, Default)]
pub struct UsageSummary {
    pub plan: Option<String>,
    pub standard_used_minutes: Option<u64>,
    pub standard_limit_minutes: Option<u64>,
    pub reasoning_used_minutes: Option<u64>,
    pub reasoning_limit_minutes: Option<u64>,
    pub next_reset_at: Option<String>,
}

/// Flexible wire model so we can tolerate backend changes without breaking the CLI.
#[derive(Debug, Deserialize)]
struct RawUsage {
    #[serde(default)]
    plan: Option<String>,
    #[serde(default)]
    next_reset_at: Option<String>,
    #[serde(default)]
    reset_at: Option<String>,
    #[serde(default)]
    standard: Option<Bucket>,
    #[serde(default)]
    reasoning: Option<Bucket>,
}

#[derive(Debug, Deserialize)]
struct Bucket {
    #[serde(default)]
    used_minutes: Option<u64>,
    #[serde(default)]
    limit_minutes: Option<u64>,
    #[serde(default)]
    used: Option<u64>,
    #[serde(default)]
    limit: Option<u64>,
}

impl From<RawUsage> for UsageSummary {
    fn from(raw: RawUsage) -> Self {
        let plan = raw.plan;
        let next_reset_at = raw.next_reset_at.or(raw.reset_at);
        let (mut standard_used_minutes, mut standard_limit_minutes) = (None, None);
        let (mut reasoning_used_minutes, mut reasoning_limit_minutes) = (None, None);

        if let Some(b) = raw.standard {
            standard_used_minutes = b.used_minutes.or(b.used);
            standard_limit_minutes = b.limit_minutes.or(b.limit);
        }
        if let Some(b) = raw.reasoning {
            reasoning_used_minutes = b.used_minutes.or(b.used);
            reasoning_limit_minutes = b.limit_minutes.or(b.limit);
        }

        UsageSummary {
            plan,
            standard_used_minutes,
            standard_limit_minutes,
            reasoning_used_minutes,
            reasoning_limit_minutes,
            next_reset_at,
        }
    }
}

/// Fetch ChatGPT guardrail usage using the current auth and config.
pub async fn get_usage(config: &Config) -> anyhow::Result<UsageSummary> {
    // This path is provided by the ChatGPT backend for Codex usage display.
    // The structure is intentionally parsed via a flexible wire model.
    let raw: RawUsage = chatgpt_get_request(config, "/wham/usage".to_string())
        .await
        .context("Failed to fetch usage from ChatGPT backend")?;
    Ok(raw.into())
}

use codex_core::WireApi;
use codex_core::config::Config;

use crate::sandbox_summary::summarize_sandbox_policy;

/// Build a list of key/value pairs summarizing the effective configuration.
pub fn create_config_summary_entries(config: &Config) -> Vec<(&'static str, String)> {
    let mut entries = vec![
        ("workdir", config.cwd.display().to_string()),
        ("model", config.model.clone()),
        ("provider", config.model_provider_id.clone()),
        ("approval", config.approval_policy.to_string()),
        ("sandbox", summarize_sandbox_policy(&config.sandbox_policy)),
    ];
    if config.model_provider.wire_api == WireApi::Responses
        && config.model_family.supports_reasoning_summaries
    {
        entries.push((
            "reasoning effort",
            config
                .model_reasoning_effort
                .map(|effort| effort.to_string())
                .unwrap_or_else(|| "none".to_string()),
        ));
        entries.push((
            "reasoning summaries",
            config.model_reasoning_summary.to_string(),
        ));
    }

    entries
}

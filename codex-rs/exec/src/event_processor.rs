use codex_common::summarize_sandbox_policy;
use codex_core::WireApi;
use codex_core::config::Config;
use codex_core::model_supports_reasoning_summaries;
use codex_core::protocol::Event;

pub(crate) trait EventProcessor {
    /// Print summary of effective configuration and user prompt.
    fn print_config_summary(&mut self, config: &Config, prompt: &str);

    /// Handle a single event emitted by the agent.
    fn process_event(&mut self, event: Event);
}

pub(crate) fn create_config_summary_entries(config: &Config) -> Vec<(&'static str, String)> {
    let mut entries = vec![
        ("workdir", config.cwd.display().to_string()),
        ("model", config.model.clone()),
        ("provider", config.model_provider_id.clone()),
        ("approval", format!("{:?}", config.approval_policy)),
        ("sandbox", summarize_sandbox_policy(&config.sandbox_policy)),
    ];
    if config.model_provider.wire_api == WireApi::Responses
        && model_supports_reasoning_summaries(config)
    {
        entries.push((
            "reasoning effort",
            config.model_reasoning_effort.to_string(),
        ));
        entries.push((
            "reasoning summaries",
            config.model_reasoning_summary.to_string(),
        ));
    }

    entries
}

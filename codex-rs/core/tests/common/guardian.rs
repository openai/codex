//! Shared fixture with a larger transcript message cap and fallback limits elsewhere.

use codex_guardian_context as context;

pub fn elastic_budget_config() -> context::ElasticBudgetConfig {
    const MESSAGE_ENTRY_TOKENS: usize = 4_000;
    let mut config = context::ElasticBudgetConfig::default();
    let fallback = usize::try_from(config.total_minimum_tokens()).expect("token total fits usize");
    let limits = context::ContextProfile::synchronous()
        .transcript
        .entry_limits;
    let minimum = context::DEFAULT_MAX_INPUT_TOKENS - (fallback - limits.message_tokens);
    config.sections.insert(
        context::ElasticSection::TranscriptMessages,
        context::ElasticSectionLimits::new(minimum, MESSAGE_ENTRY_TOKENS).expect("valid limits"),
    );
    config
}

pub fn async_elastic_sections_toml() -> String {
    let sections = toml::Value::try_from(elastic_budget_config().sections).expect("encode limits");
    format!("async_elastic_sections = {sections}")
}

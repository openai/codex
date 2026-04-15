use crate::config::Config;
use codex_otel::OtelProvider;
use std::error::Error;

pub fn build_provider(
    _config: &Config,
    _service_version: &str,
    _service_name_override: Option<&str>,
    _default_analytics_enabled: bool,
) -> Result<Option<OtelProvider>, Box<dyn Error>> {
    Ok(None)
}

pub fn codex_export_filter(_meta: &tracing::Metadata<'_>) -> bool {
    false
}

//! Extension module for unified logging infrastructure.
//!
//! This module provides helper functions for logging configuration that can be
//! used by both exec and tui modes, eliminating duplicate code.
//! Following the upstream sync pattern to minimize merge conflicts.

use crate::config::types_ext::LoggingConfig;
use crate::config::types_ext::TimezoneConfig;
use tracing_subscriber::filter::EnvFilter;

/// Timezone-aware time formatter for tracing subscribers.
///
/// Formats timestamps according to the configured timezone (Local or UTC).
pub struct ConfigurableTimer {
    timezone: TimezoneConfig,
}

impl ConfigurableTimer {
    pub fn new(timezone: TimezoneConfig) -> Self {
        Self { timezone }
    }
}

impl tracing_subscriber::fmt::time::FormatTime for ConfigurableTimer {
    fn format_time(&self, w: &mut tracing_subscriber::fmt::format::Writer<'_>) -> std::fmt::Result {
        match self.timezone {
            TimezoneConfig::Local => {
                // Use local time formatting
                write!(
                    w,
                    "{}",
                    chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f")
                )
            }
            TimezoneConfig::Utc => {
                // Use UTC time formatting
                write!(w, "{}", chrono::Utc::now().format("%Y-%m-%d %H:%M:%S%.3f"))
            }
        }
    }
}

/// Build an env filter from config with fallback logic.
///
/// Priority:
/// 1. RUST_LOG environment variable (highest priority)
/// 2. config.logging.modules (module-specific levels)
/// 3. config.logging.level (default level)
/// 4. fallback_default (if all else fails)
pub fn build_env_filter(logging: &LoggingConfig, fallback_default: &str) -> EnvFilter {
    // Priority 1: RUST_LOG env var takes precedence
    if let Ok(filter) = EnvFilter::try_from_default_env() {
        return filter;
    }

    // Priority 2: If modules are specified, build filter from them
    if !logging.modules.is_empty() {
        let filter_str = logging.modules.join(",");
        if let Ok(filter) = EnvFilter::try_new(&filter_str) {
            return filter;
        }
    }

    // Priority 3: Use config.logging.level
    EnvFilter::try_new(&logging.level)
        .or_else(|_| EnvFilter::try_new(fallback_default))
        .unwrap_or_else(|_| EnvFilter::new(fallback_default))
}

/// Configure a fmt layer with timer, location, target, and filter from LoggingConfig.
///
/// This macro takes a base fmt layer (with writer and mode-specific settings) and applies:
/// 1. Timezone-aware timer
/// 2. Location (file/line) if enabled
/// 3. Target (module path) settings
/// 4. EnvFilter from config
///
/// # Example
/// ```ignore
/// let layer = configure_fmt_layer!(
///     tracing_subscriber::fmt::layer()
///         .with_ansi(true)
///         .with_writer(std::io::stderr),
///     &config.ext.logging,
///     "error"
/// );
/// ```
#[macro_export]
macro_rules! configure_fmt_layer {
    ($base_layer:expr, $logging:expr, $fallback:expr) => {{
        let env_filter = $crate::logging_ext::build_env_filter($logging, $fallback);
        let timer = $crate::logging_ext::ConfigurableTimer::new($logging.timezone.clone());

        let mut layer = $base_layer.with_timer(timer);

        if $logging.location {
            layer = layer.with_file(true).with_line_number(true);
        }
        if $logging.target {
            layer = layer.with_target(true);
        } else {
            layer = layer.with_target(false);
        }

        layer.with_filter(env_filter)
    }};
}

#[cfg(test)]
mod tests {
    use super::*;
    use tracing_subscriber::fmt::format::Writer;
    use tracing_subscriber::fmt::time::FormatTime;

    #[test]
    fn test_configurable_timer_local() {
        let timer = ConfigurableTimer::new(TimezoneConfig::Local);
        let mut buf = String::new();
        let mut writer = Writer::new(&mut buf);
        // Just verify it doesn't panic
        let _ = timer.format_time(&mut writer);
    }

    #[test]
    fn test_configurable_timer_utc() {
        let timer = ConfigurableTimer::new(TimezoneConfig::Utc);
        let mut buf = String::new();
        let mut writer = Writer::new(&mut buf);
        // Just verify it doesn't panic
        let _ = timer.format_time(&mut writer);
    }

    #[test]
    fn test_build_env_filter_with_default() {
        let logging = LoggingConfig::default();
        let filter = build_env_filter(&logging, "error");
        // Verify filter was created successfully (just check it doesn't panic)
        let _ = format!("{:?}", filter);
    }

    #[test]
    fn test_build_env_filter_with_modules() {
        let logging = LoggingConfig {
            location: false,
            target: false,
            timezone: TimezoneConfig::Local,
            level: "info".to_string(),
            modules: vec![
                "codex_core=debug".to_string(),
                "codex_tui=trace".to_string(),
            ],
        };
        let filter = build_env_filter(&logging, "error");
        let filter_str = format!("{:?}", filter);
        // Verify modules were applied
        assert!(filter_str.contains("codex_core") || filter_str.contains("debug"));
    }
}

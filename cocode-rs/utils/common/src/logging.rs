//! Shared logging infrastructure for codex-rs.
//!
//! This module provides timezone-aware logging utilities that can be used
//! by all crates in the workspace without circular dependencies.

use serde::Deserialize;
use serde::Serialize;
use tracing_subscriber::filter::EnvFilter;

/// Logging configuration for tracing subscriber
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct LoggingConfig {
    /// Show file name and line number in log output
    pub location: bool,

    /// Show module path (target) in log output
    pub target: bool,

    /// Timezone for log timestamps
    pub timezone: TimezoneConfig,

    /// Default log level (trace, debug, info, warn, error)
    pub level: String,

    /// Module-specific log levels (e.g., "codex_core=debug,codex_tui=info")
    #[serde(default)]
    pub modules: Vec<String>,
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            location: false,                 // Don't show file/line by default (keep logs clean)
            target: false,                   // Don't show module path by default
            timezone: TimezoneConfig::Local, // Use local timezone by default
            level: "info".to_string(),
            modules: vec![],
        }
    }
}

impl LoggingConfig {
    /// Create config with a specific log level (for standalone tools without full Config).
    pub fn with_level(level: &str) -> Self {
        Self {
            level: level.to_string(),
            ..Default::default()
        }
    }
}

/// Timezone configuration for log timestamps
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum TimezoneConfig {
    /// Use local timezone
    Local,
    /// Use UTC timezone
    Utc,
}

impl Default for TimezoneConfig {
    fn default() -> Self {
        Self::Local
    }
}

/// Timezone-aware time formatter for tracing subscribers.
///
/// Implements `FormatTime` trait, formatting timestamps according to the
/// configured timezone (Local or UTC) using chrono.
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
                write!(
                    w,
                    "{}",
                    chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f")
                )
            }
            TimezoneConfig::Utc => {
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
/// 1. Timezone-aware timer (based on `$logging.timezone`)
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
        let env_filter = $crate::logging::build_env_filter($logging, $fallback);
        let timer = $crate::logging::ConfigurableTimer::new($logging.timezone.clone());

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
        let _ = timer.format_time(&mut writer);
    }

    #[test]
    fn test_configurable_timer_utc() {
        let timer = ConfigurableTimer::new(TimezoneConfig::Utc);
        let mut buf = String::new();
        let mut writer = Writer::new(&mut buf);
        let _ = timer.format_time(&mut writer);
    }

    #[test]
    fn test_build_env_filter_with_default() {
        let logging = LoggingConfig::default();
        let filter = build_env_filter(&logging, "error");
        let _ = format!("{filter:?}");
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
        let filter_str = format!("{filter:?}");
        assert!(filter_str.contains("codex_core") || filter_str.contains("debug"));
    }
}

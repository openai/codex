//! Internationalization (i18n) support for the TUI.
//!
//! This module provides translation support using the `rust-i18n` crate.
//! Currently supports:
//! - English (en) - default
//! - Simplified Chinese (zh-CN)
//!
//! ## Usage
//!
//! ```ignore
//! use crate::i18n::t;
//!
//! // Simple translation
//! let text = t!("command.toggle_plan_mode");
//!
//! // With parameters
//! let msg = t!("toast.context_warning", percent = 25, remain = "67k", total = "270k");
//! ```
//!
//! ## Locale Detection
//!
//! Locale is detected in the following order:
//! 1. `COCODE_LANG` environment variable
//! 2. `LANG` environment variable
//! 3. `LC_ALL` environment variable
//! 4. Default to English ("en")

// Note: The i18n! macro is called at the crate root (lib.rs) to generate
// the _rust_i18n_t function that the t! macro uses.

pub use rust_i18n::t;

/// Initialize the i18n system with locale detection.
///
/// This should be called once at application startup.
pub fn init() {
    let locale = detect_locale();
    rust_i18n::set_locale(locale);
    tracing::debug!(locale, "i18n initialized");
}

/// Get the current locale.
pub fn current_locale() -> String {
    rust_i18n::locale().to_string()
}

/// Set the locale explicitly.
pub fn set_locale(locale: &str) {
    rust_i18n::set_locale(locale);
}

/// Detect the user's preferred locale from environment variables.
///
/// Priority order:
/// 1. `COCODE_LANG` - Application-specific override
/// 2. `LANG` - Standard Unix locale
/// 3. `LC_ALL` - Alternative Unix locale
/// 4. Default to "en"
fn detect_locale() -> &'static str {
    // Check COCODE_LANG first (app-specific override)
    if let Ok(lang) = std::env::var("COCODE_LANG") {
        if let Some(locale) = parse_locale(&lang) {
            return locale;
        }
    }

    // Check LANG
    if let Ok(lang) = std::env::var("LANG") {
        if let Some(locale) = parse_locale(&lang) {
            return locale;
        }
    }

    // Check LC_ALL
    if let Ok(lang) = std::env::var("LC_ALL") {
        if let Some(locale) = parse_locale(&lang) {
            return locale;
        }
    }

    // Default to English
    "en"
}

/// Parse a locale string and return a supported locale.
///
/// Handles formats like:
/// - "zh_CN.UTF-8"
/// - "zh-CN"
/// - "zh"
/// - "en_US.UTF-8"
/// - "en"
fn parse_locale(locale: &str) -> Option<&'static str> {
    // Normalize: lowercase and handle both _ and -
    let normalized = locale.to_lowercase().replace('_', "-");

    // Extract the language/region part (before any encoding like .UTF-8)
    let lang_part = normalized.split('.').next().unwrap_or(&normalized);

    // Match supported locales
    match lang_part {
        s if s.starts_with("zh-cn") || s.starts_with("zh-hans") => Some("zh-CN"),
        s if s.starts_with("zh") => Some("zh-CN"), // Default Chinese to Simplified
        s if s.starts_with("en") => Some("en"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_locale_chinese() {
        assert_eq!(parse_locale("zh_CN.UTF-8"), Some("zh-CN"));
        assert_eq!(parse_locale("zh-CN"), Some("zh-CN"));
        assert_eq!(parse_locale("zh"), Some("zh-CN"));
        assert_eq!(parse_locale("zh_Hans"), Some("zh-CN"));
    }

    #[test]
    fn test_parse_locale_english() {
        assert_eq!(parse_locale("en_US.UTF-8"), Some("en"));
        assert_eq!(parse_locale("en-US"), Some("en"));
        assert_eq!(parse_locale("en"), Some("en"));
    }

    #[test]
    fn test_parse_locale_unknown() {
        assert_eq!(parse_locale("fr_FR.UTF-8"), None);
        assert_eq!(parse_locale("de"), None);
    }

    #[test]
    fn test_t_macro_works() {
        // This test verifies the t! macro is properly re-exported
        // and the locales are loaded
        let text = t!("command.toggle_plan_mode");
        // Should return the translation or the key if not found
        assert!(!text.is_empty());
    }
}

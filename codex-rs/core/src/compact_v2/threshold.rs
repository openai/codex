//! Threshold management for compact triggering.
//!
//! Calculates when compaction should be triggered based on token usage
//! and configuration thresholds.

use super::config::CompactConfig;

/// State of all threshold checks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ThresholdState {
    /// Percentage of context remaining (0-100)
    pub percent_remaining: i32,
    /// Token count exceeds warning level
    pub is_above_warning: bool,
    /// Token count exceeds auto-compact threshold
    pub is_above_auto_compact: bool,
}

impl Default for ThresholdState {
    fn default() -> Self {
        Self {
            percent_remaining: 100,
            is_above_warning: false,
            is_above_auto_compact: false,
        }
    }
}

/// Calculate all threshold states.
///
/// Matches Claude Code's x1A / calculateThresholds function.
pub fn calculate_thresholds(
    used_tokens: i64,
    context_limit: i64,
    config: &CompactConfig,
) -> ThresholdState {
    if context_limit <= 0 {
        return ThresholdState::default();
    }

    // Calculate effective auto-compact threshold
    let auto_compact_threshold = get_auto_compact_threshold(context_limit, config);

    // Use auto-compact threshold as effective limit when enabled
    let effective_limit = if config.auto_compact_enabled {
        auto_compact_threshold
    } else {
        context_limit
    };

    let percent_remaining = if effective_limit > 0 {
        ((effective_limit - used_tokens).max(0) * 100 / effective_limit) as i32
    } else {
        0
    };

    let warning_level = context_limit - config.warning_threshold;

    ThresholdState {
        percent_remaining,
        is_above_warning: used_tokens >= warning_level,
        is_above_auto_compact: config.auto_compact_enabled && used_tokens >= auto_compact_threshold,
    }
}

/// Calculate auto-compact threshold.
///
/// Matches Claude Code's aI2 / getAutoCompactThreshold function.
///
/// Returns the token count at which auto-compact should trigger.
/// Priority:
/// 1. Explicit `auto_compact_threshold` (if set)
/// 2. Percentage override `auto_compact_pct_override` (if set)
/// 3. Default: `context_limit - free_space_buffer`
///
/// All values are capped at the default to ensure safety.
pub fn get_auto_compact_threshold(context_limit: i64, config: &CompactConfig) -> i64 {
    // Default: context_limit - free_space_buffer
    let default_threshold = context_limit - config.free_space_buffer;

    // Check for explicit threshold override
    if let Some(explicit) = config.auto_compact_threshold {
        return explicit.min(default_threshold);
    }

    // Check for percentage override
    if let Some(pct) = config.auto_compact_pct_override {
        if pct > 0 && pct <= 100 {
            let custom = (context_limit * pct as i64) / 100;
            return custom.min(default_threshold); // Never exceed default
        }
    }

    default_threshold
}

/// Calculate tokens remaining until auto-compact triggers.
#[allow(dead_code)] // Used in threshold calculations
pub fn tokens_until_compact(used_tokens: i64, context_limit: i64, config: &CompactConfig) -> i64 {
    let threshold = get_auto_compact_threshold(context_limit, config);
    (threshold - used_tokens).max(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    fn config_with_defaults() -> CompactConfig {
        CompactConfig::default()
    }

    #[test]
    fn threshold_below_all_limits() {
        let config = config_with_defaults();
        let state = calculate_thresholds(50_000, 200_000, &config);

        assert!(!state.is_above_warning);
        assert!(!state.is_above_auto_compact);
        assert!(state.percent_remaining > 50);
    }

    #[test]
    fn threshold_above_warning() {
        let config = config_with_defaults();
        // Warning threshold is 20,000 from limit
        // With 200,000 limit, warning at 180,000
        let state = calculate_thresholds(185_000, 200_000, &config);

        assert!(state.is_above_warning);
    }

    #[test]
    fn threshold_triggers_auto_compact() {
        let config = config_with_defaults();
        // Auto-compact threshold is context_limit - free_space_buffer
        // = 200,000 - 13,000 = 187,000
        let state = calculate_thresholds(190_000, 200_000, &config);

        assert!(state.is_above_auto_compact);
    }

    #[test]
    fn threshold_with_pct_override() {
        let mut config = config_with_defaults();
        config.auto_compact_pct_override = Some(80);

        // With 80% override on 200,000 limit: 160,000
        let threshold = get_auto_compact_threshold(200_000, &config);
        assert_eq!(threshold, 160_000);

        let state = calculate_thresholds(165_000, 200_000, &config);
        assert!(state.is_above_auto_compact);
    }

    #[test]
    fn threshold_pct_capped_at_default() {
        let mut config = config_with_defaults();
        config.auto_compact_pct_override = Some(99);

        // 99% of 200,000 = 198,000, but default is 187,000
        // Should be capped at default
        let threshold = get_auto_compact_threshold(200_000, &config);
        assert_eq!(threshold, 187_000);
    }

    #[test]
    fn threshold_explicit_override() {
        let mut config = config_with_defaults();
        config.auto_compact_threshold = Some(150_000);

        let threshold = get_auto_compact_threshold(200_000, &config);
        assert_eq!(threshold, 150_000);
    }

    #[test]
    fn threshold_disabled_auto_compact() {
        let mut config = config_with_defaults();
        config.auto_compact_enabled = false;

        let state = calculate_thresholds(195_000, 200_000, &config);
        assert!(!state.is_above_auto_compact);
    }

    #[test]
    fn tokens_until_compact_calculation() {
        let config = config_with_defaults();
        // Threshold at 187,000
        let remaining = tokens_until_compact(100_000, 200_000, &config);
        assert_eq!(remaining, 87_000);

        let remaining = tokens_until_compact(190_000, 200_000, &config);
        assert_eq!(remaining, 0);
    }

    #[test]
    fn zero_context_limit_returns_defaults() {
        let config = config_with_defaults();
        let state = calculate_thresholds(0, 0, &config);
        assert_eq!(state.percent_remaining, 100);
        assert!(!state.is_above_auto_compact);
    }
}

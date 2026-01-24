//! Search algorithm constants.
//!
//! Centralizes magic numbers used in search ranking and fusion.
//! These values are based on academic research and industry best practices.

/// Seconds in a day (60 * 60 * 24).
///
/// Used for time-based calculations like recency scoring.
pub const SECONDS_PER_DAY: f32 = 86400.0;

/// Natural log of 2, used for exponential decay calculations.
///
/// ln(2) = 0.693147180559945...
/// Used in half-life decay formula: score = exp(-ln(2) * age / half_life)
pub const LN_2: f32 = 0.693147180559945;

/// Default RRF (Reciprocal Rank Fusion) k parameter.
///
/// Standard value from the original RRF paper (Cormack et al., 2009).
/// Higher values give more weight to lower-ranked items.
/// Formula: score = weight / (rank + k)
pub const DEFAULT_RRF_K: f32 = 60.0;

/// Default recency decay half-life in days.
///
/// Files modified this many days ago have 50% recency score.
/// After 2 half-lives (14 days), score is 25%, etc.
pub const DEFAULT_RECENCY_HALF_LIFE_DAYS: f32 = 7.0;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_seconds_per_day() {
        assert_eq!(SECONDS_PER_DAY, 60.0 * 60.0 * 24.0);
    }

    #[test]
    fn test_ln_2_precision() {
        // Verify LN_2 is close to actual ln(2)
        let actual_ln2 = 2.0_f32.ln();
        assert!((LN_2 - actual_ln2).abs() < 1e-6);
    }

    #[test]
    fn test_rrf_k_positive() {
        assert!(DEFAULT_RRF_K > 0.0);
    }

    #[test]
    fn test_recency_half_life_positive() {
        assert!(DEFAULT_RECENCY_HALF_LIFE_DAYS > 0.0);
    }
}

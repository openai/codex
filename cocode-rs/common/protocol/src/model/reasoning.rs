//! Reasoning effort level types.

use serde::{Deserialize, Serialize};
use strum::{Display, EnumIter};

/// Reasoning effort level for models that support extended thinking.
///
/// Variants are ordered from lowest to highest effort, enabling direct comparison:
/// `ReasoningEffort::High > ReasoningEffort::Low`
///
/// See <https://platform.openai.com/docs/guides/reasoning>
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Default,
    Serialize,
    Deserialize,
    Display,
    EnumIter,
)]
#[serde(rename_all = "lowercase")]
#[strum(serialize_all = "lowercase")]
pub enum ReasoningEffort {
    /// No reasoning (ord = 0).
    None,
    /// Minimal reasoning (ord = 1).
    Minimal,
    /// Low reasoning effort (ord = 2).
    Low,
    /// Medium reasoning effort (ord = 3, default).
    #[default]
    Medium,
    /// High reasoning effort (ord = 4).
    High,
    /// Extra high reasoning effort (ord = 5).
    XHigh,
}

/// Find nearest supported effort level using `Ord` comparison.
pub fn nearest_effort(target: ReasoningEffort, supported: &[ReasoningEffort]) -> ReasoningEffort {
    supported
        .iter()
        .copied()
        .min_by_key(|c| (*c as i32 - target as i32).abs())
        .unwrap_or(target)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ord_comparison() {
        // Test Ord trait - variants are ordered from lowest to highest
        assert!(ReasoningEffort::None < ReasoningEffort::Minimal);
        assert!(ReasoningEffort::Minimal < ReasoningEffort::Low);
        assert!(ReasoningEffort::Low < ReasoningEffort::Medium);
        assert!(ReasoningEffort::Medium < ReasoningEffort::High);
        assert!(ReasoningEffort::High < ReasoningEffort::XHigh);

        // Direct comparison
        assert!(ReasoningEffort::High > ReasoningEffort::Low);
        assert!(ReasoningEffort::Medium == ReasoningEffort::Medium);
        assert!(ReasoningEffort::XHigh >= ReasoningEffort::High);
    }

    #[test]
    fn test_nearest_effort() {
        let supported = vec![
            ReasoningEffort::Low,
            ReasoningEffort::Medium,
            ReasoningEffort::High,
        ];

        // Exact match
        assert_eq!(
            nearest_effort(ReasoningEffort::Medium, &supported),
            ReasoningEffort::Medium
        );

        // None -> Low (nearest)
        assert_eq!(
            nearest_effort(ReasoningEffort::None, &supported),
            ReasoningEffort::Low
        );

        // XHigh -> High (nearest)
        assert_eq!(
            nearest_effort(ReasoningEffort::XHigh, &supported),
            ReasoningEffort::High
        );
    }

    #[test]
    fn test_default() {
        assert_eq!(ReasoningEffort::default(), ReasoningEffort::Medium);
    }

    #[test]
    fn test_serde() {
        let effort = ReasoningEffort::High;
        let json = serde_json::to_string(&effort).expect("serialize");
        assert_eq!(json, "\"high\"");

        let parsed: ReasoningEffort = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed, ReasoningEffort::High);
    }
}

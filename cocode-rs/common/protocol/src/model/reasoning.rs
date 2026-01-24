//! Reasoning effort level types.

use serde::{Deserialize, Serialize};
use strum::{Display, EnumIter};

/// Reasoning effort level for models that support extended thinking.
///
/// See <https://platform.openai.com/docs/guides/reasoning>
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize, Display, EnumIter,
)]
#[serde(rename_all = "lowercase")]
#[strum(serialize_all = "lowercase")]
pub enum ReasoningEffort {
    /// No reasoning.
    None,
    /// Minimal reasoning.
    Minimal,
    /// Low reasoning effort.
    Low,
    /// Medium reasoning effort (default).
    #[default]
    Medium,
    /// High reasoning effort.
    High,
    /// Extra high reasoning effort.
    XHigh,
}

/// Get effort rank for comparison.
pub fn effort_rank(effort: ReasoningEffort) -> i32 {
    match effort {
        ReasoningEffort::None => 0,
        ReasoningEffort::Minimal => 1,
        ReasoningEffort::Low => 2,
        ReasoningEffort::Medium => 3,
        ReasoningEffort::High => 4,
        ReasoningEffort::XHigh => 5,
    }
}

/// Find nearest supported effort level.
pub fn nearest_effort(target: ReasoningEffort, supported: &[ReasoningEffort]) -> ReasoningEffort {
    let target_rank = effort_rank(target);
    supported
        .iter()
        .copied()
        .min_by_key(|c| (effort_rank(*c) - target_rank).abs())
        .unwrap_or(target)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_effort_rank() {
        assert_eq!(effort_rank(ReasoningEffort::None), 0);
        assert_eq!(effort_rank(ReasoningEffort::Medium), 3);
        assert_eq!(effort_rank(ReasoningEffort::XHigh), 5);
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

//! Unified thinking level configuration.
//!
//! Defines `ThinkingLevel` which combines effort-based (OpenAI) and budget-based
//! (Anthropic/Gemini) approaches into a single unified type.

use crate::model::ReasoningEffort;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;
use std::str::FromStr;

/// Unified thinking level for all providers.
///
/// Combines effort-based (OpenAI) and budget-based (Anthropic/Gemini) approaches.
/// `effort` is required and used for ordering; `budget_tokens` is optional.
///
/// # Flexible Deserialization
///
/// Accepts both string shorthand and full object:
/// ```json
/// "high"  // ThinkingLevel { effort: High, budget_tokens: None, ... }
/// {"effort": "high", "budget_tokens": 32000}
/// ```
///
/// # Environment Variables
///
/// - `COCODE_THINKING_LEVEL`: Thinking level string (none, low, medium, high, xhigh)
/// - `COCODE_THINKING_BUDGET`: Budget tokens for thinking
///
/// # Example
///
/// ```json
/// {
///   "thinking_level": {
///     "effort": "high",
///     "budget_tokens": 32000,
///     "interleaved": true
///   }
/// }
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThinkingLevel {
    /// Reasoning effort level (required, used for ordering).
    pub effort: ReasoningEffort,

    /// Token budget for budget-based models (optional).
    pub budget_tokens: Option<i32>,

    /// Max output tokens for thinking (optional, falls back to outer max_output_tokens).
    pub max_output_tokens: Option<i32>,

    /// Enable interleaved thinking mode (Anthropic).
    pub interleaved: bool,
}

impl ThinkingLevel {
    /// Create from effort only.
    pub fn new(effort: ReasoningEffort) -> Self {
        Self {
            effort,
            budget_tokens: None,
            max_output_tokens: None,
            interleaved: false,
        }
    }

    /// Create with effort and budget.
    pub fn with_budget(effort: ReasoningEffort, budget: i32) -> Self {
        Self {
            effort,
            budget_tokens: Some(budget),
            max_output_tokens: None,
            interleaved: false,
        }
    }

    /// Create a disabled thinking level.
    pub fn none() -> Self {
        Self::new(ReasoningEffort::None)
    }

    /// Create a low thinking level.
    pub fn low() -> Self {
        Self::new(ReasoningEffort::Low)
    }

    /// Create a medium thinking level.
    pub fn medium() -> Self {
        Self::new(ReasoningEffort::Medium)
    }

    /// Create a high thinking level.
    pub fn high() -> Self {
        Self::new(ReasoningEffort::High)
    }

    pub fn xhigh() -> Self {
        Self::new(ReasoningEffort::XHigh)
    }

    /// Check if thinking is enabled (effort > None).
    pub fn is_enabled(&self) -> bool {
        self.effort > ReasoningEffort::None
    }

    /// Set the budget tokens.
    pub fn set_budget(mut self, budget: i32) -> Self {
        self.budget_tokens = Some(budget);
        self
    }

    /// Set max output tokens.
    pub fn set_max_output_tokens(mut self, tokens: i32) -> Self {
        self.max_output_tokens = Some(tokens);
        self
    }

    /// Set interleaved mode.
    pub fn set_interleaved(mut self, interleaved: bool) -> Self {
        self.interleaved = interleaved;
        self
    }

    /// Get the string representation of the effort level.
    pub fn as_str(&self) -> &'static str {
        match self.effort {
            ReasoningEffort::None => "none",
            ReasoningEffort::Minimal => "minimal",
            ReasoningEffort::Low => "low",
            ReasoningEffort::Medium => "medium",
            ReasoningEffort::High => "high",
            ReasoningEffort::XHigh => "xhigh",
        }
    }

    /// Validate configuration values.
    pub fn validate(&self) -> Result<(), String> {
        if let Some(budget) = self.budget_tokens {
            if budget < 0 {
                return Err(format!("budget_tokens must be >= 0, got {budget}"));
            }
        }
        if let Some(max_out) = self.max_output_tokens {
            if max_out < 0 {
                return Err(format!("max_output_tokens must be >= 0, got {max_out}"));
            }
        }
        Ok(())
    }
}

impl Default for ThinkingLevel {
    fn default() -> Self {
        Self::new(ReasoningEffort::None)
    }
}

impl fmt::Display for ThinkingLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl FromStr for ThinkingLevel {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let effort = match s.to_lowercase().as_str() {
            "none" | "" => ReasoningEffort::None,
            "minimal" => ReasoningEffort::Minimal,
            "low" => ReasoningEffort::Low,
            "medium" | "default" => ReasoningEffort::Medium,
            "high" => ReasoningEffort::High,
            "xhigh" | "x_high" | "extra_high" => ReasoningEffort::XHigh,
            _ => return Err(format!("unknown thinking level: {s}")),
        };
        Ok(Self::new(effort))
    }
}

// Custom serde implementation to support both string shorthand and full object
impl Serialize for ThinkingLevel {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        use serde::ser::SerializeStruct;

        // If only effort is set (other fields are defaults), serialize as string
        if self.budget_tokens.is_none() && self.max_output_tokens.is_none() && !self.interleaved {
            return serializer.serialize_str(self.as_str());
        }

        // Otherwise serialize as struct
        let field_count = 1
            + self.budget_tokens.is_some() as usize
            + self.max_output_tokens.is_some() as usize
            + self.interleaved as usize;
        let mut state = serializer.serialize_struct("ThinkingLevel", field_count)?;
        state.serialize_field("effort", &self.effort)?;
        if self.budget_tokens.is_some() {
            state.serialize_field("budget_tokens", &self.budget_tokens)?;
        }
        if self.max_output_tokens.is_some() {
            state.serialize_field("max_output_tokens", &self.max_output_tokens)?;
        }
        if self.interleaved {
            state.serialize_field("interleaved", &self.interleaved)?;
        }
        state.end()
    }
}

impl<'de> Deserialize<'de> for ThinkingLevel {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        use serde::de::{MapAccess, Visitor};

        struct ThinkingLevelVisitor;

        impl<'de> Visitor<'de> for ThinkingLevelVisitor {
            type Value = ThinkingLevel;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a string (effort level) or an object with effort, budget_tokens, max_output_tokens, and interleaved fields")
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                ThinkingLevel::from_str(value).map_err(serde::de::Error::custom)
            }

            fn visit_map<M>(self, mut map: M) -> Result<Self::Value, M::Error>
            where
                M: MapAccess<'de>,
            {
                let mut effort: Option<ReasoningEffort> = None;
                let mut budget_tokens: Option<i32> = None;
                let mut max_output_tokens: Option<i32> = None;
                let mut interleaved: Option<bool> = None;

                while let Some(key) = map.next_key::<String>()? {
                    match key.as_str() {
                        "effort" => {
                            effort = Some(map.next_value()?);
                        }
                        "budget_tokens" => {
                            budget_tokens = map.next_value()?;
                        }
                        "max_output_tokens" => {
                            max_output_tokens = map.next_value()?;
                        }
                        "interleaved" => {
                            interleaved = Some(map.next_value()?);
                        }
                        _ => {
                            // Skip unknown fields for forward compatibility
                            let _ = map.next_value::<serde::de::IgnoredAny>()?;
                        }
                    }
                }

                let effort = effort.unwrap_or_default();
                Ok(ThinkingLevel {
                    effort,
                    budget_tokens,
                    max_output_tokens,
                    interleaved: interleaved.unwrap_or(false),
                })
            }
        }

        deserializer.deserialize_any(ThinkingLevelVisitor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_thinking_level_new() {
        let level = ThinkingLevel::new(ReasoningEffort::High);
        assert_eq!(level.effort, ReasoningEffort::High);
        assert!(level.budget_tokens.is_none());
        assert!(level.max_output_tokens.is_none());
        assert!(!level.interleaved);
    }

    #[test]
    fn test_thinking_level_with_budget() {
        let level = ThinkingLevel::with_budget(ReasoningEffort::High, 32000);
        assert_eq!(level.effort, ReasoningEffort::High);
        assert_eq!(level.budget_tokens, Some(32000));
    }

    #[test]
    fn test_thinking_level_convenience_constructors() {
        assert_eq!(ThinkingLevel::none().effort, ReasoningEffort::None);
        assert_eq!(ThinkingLevel::low().effort, ReasoningEffort::Low);
        assert_eq!(ThinkingLevel::medium().effort, ReasoningEffort::Medium);
        assert_eq!(ThinkingLevel::high().effort, ReasoningEffort::High);
    }

    #[test]
    fn test_thinking_level_is_enabled() {
        assert!(!ThinkingLevel::none().is_enabled());
        assert!(ThinkingLevel::low().is_enabled());
        assert!(ThinkingLevel::medium().is_enabled());
        assert!(ThinkingLevel::high().is_enabled());
    }

    #[test]
    fn test_thinking_level_default() {
        let level = ThinkingLevel::default();
        assert_eq!(level.effort, ReasoningEffort::None);
        assert!(!level.is_enabled());
    }

    #[test]
    fn test_thinking_level_effort_ordering() {
        // Ordering is done via the effort field (ReasoningEffort implements Ord)
        assert!(ThinkingLevel::none().effort < ThinkingLevel::low().effort);
        assert!(ThinkingLevel::low().effort < ThinkingLevel::medium().effort);
        assert!(ThinkingLevel::medium().effort < ThinkingLevel::high().effort);

        // Same effort, different budgets -> PartialEq compares all fields
        let high_no_budget = ThinkingLevel::high();
        let high_with_budget = ThinkingLevel::with_budget(ReasoningEffort::High, 32000);
        assert_eq!(high_no_budget.effort, high_with_budget.effort);
        assert_ne!(high_no_budget, high_with_budget); // Different structs

        // Effort comparison ignores budget
        let medium_with_huge_budget = ThinkingLevel::with_budget(ReasoningEffort::Medium, 100000);
        assert!(medium_with_huge_budget.effort < high_no_budget.effort);
    }

    #[test]
    fn test_thinking_level_from_str() {
        assert_eq!(
            "none".parse::<ThinkingLevel>().unwrap().effort,
            ReasoningEffort::None
        );
        assert_eq!(
            "low".parse::<ThinkingLevel>().unwrap().effort,
            ReasoningEffort::Low
        );
        assert_eq!(
            "medium".parse::<ThinkingLevel>().unwrap().effort,
            ReasoningEffort::Medium
        );
        assert_eq!(
            "high".parse::<ThinkingLevel>().unwrap().effort,
            ReasoningEffort::High
        );
        assert_eq!(
            "xhigh".parse::<ThinkingLevel>().unwrap().effort,
            ReasoningEffort::XHigh
        );
        assert!("invalid".parse::<ThinkingLevel>().is_err());
    }

    #[test]
    fn test_thinking_level_serde_string() {
        // Deserialize from string
        let level: ThinkingLevel = serde_json::from_str("\"high\"").unwrap();
        assert_eq!(level.effort, ReasoningEffort::High);
        assert!(level.budget_tokens.is_none());

        // Serialize simple level as string
        let json = serde_json::to_string(&ThinkingLevel::high()).unwrap();
        assert_eq!(json, "\"high\"");
    }

    #[test]
    fn test_thinking_level_serde_object() {
        // Deserialize from object
        let json = r#"{
            "effort": "high",
            "budget_tokens": 32000,
            "interleaved": true
        }"#;
        let level: ThinkingLevel = serde_json::from_str(json).unwrap();
        assert_eq!(level.effort, ReasoningEffort::High);
        assert_eq!(level.budget_tokens, Some(32000));
        assert!(level.interleaved);

        // Serialize complex level as object
        let level = ThinkingLevel::with_budget(ReasoningEffort::High, 32000).set_interleaved(true);
        let json = serde_json::to_string(&level).unwrap();
        assert!(json.contains("\"effort\""));
        assert!(json.contains("\"budget_tokens\""));
    }

    #[test]
    fn test_thinking_level_serde_object_defaults() {
        // Minimal object with just effort
        let json = r#"{"effort": "medium"}"#;
        let level: ThinkingLevel = serde_json::from_str(json).unwrap();
        assert_eq!(level.effort, ReasoningEffort::Medium);
        assert!(level.budget_tokens.is_none());
        assert!(!level.interleaved);
    }

    #[test]
    fn test_thinking_level_serde_roundtrip() {
        let level = ThinkingLevel {
            effort: ReasoningEffort::High,
            budget_tokens: Some(32000),
            max_output_tokens: Some(16000),
            interleaved: true,
        };

        let json = serde_json::to_string(&level).unwrap();
        let parsed: ThinkingLevel = serde_json::from_str(&json).unwrap();
        assert_eq!(level, parsed);
    }

    #[test]
    fn test_thinking_level_validate() {
        let level = ThinkingLevel::with_budget(ReasoningEffort::High, 32000);
        assert!(level.validate().is_ok());

        let level = ThinkingLevel {
            budget_tokens: Some(-1),
            ..Default::default()
        };
        assert!(level.validate().is_err());

        let level = ThinkingLevel {
            max_output_tokens: Some(-1),
            ..Default::default()
        };
        assert!(level.validate().is_err());
    }

    #[test]
    fn test_thinking_level_builder_methods() {
        let level = ThinkingLevel::high()
            .set_budget(32000)
            .set_max_output_tokens(16000)
            .set_interleaved(true);

        assert_eq!(level.effort, ReasoningEffort::High);
        assert_eq!(level.budget_tokens, Some(32000));
        assert_eq!(level.max_output_tokens, Some(16000));
        assert!(level.interleaved);
    }

    #[test]
    fn test_thinking_level_display() {
        assert_eq!(ThinkingLevel::none().to_string(), "none");
        assert_eq!(ThinkingLevel::low().to_string(), "low");
        assert_eq!(ThinkingLevel::medium().to_string(), "medium");
        assert_eq!(ThinkingLevel::high().to_string(), "high");
    }
}

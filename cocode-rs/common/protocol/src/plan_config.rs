//! Plan mode configuration.
//!
//! Defines settings for plan mode agent behavior.

use serde::{Deserialize, Serialize};

/// Default number of agents for plan execution.
pub const DEFAULT_PLAN_AGENT_COUNT: i32 = 1;

/// Default number of agents for exploration during planning.
pub const DEFAULT_PLAN_EXPLORE_AGENT_COUNT: i32 = 3;

/// Minimum allowed agent count.
pub const MIN_AGENT_COUNT: i32 = 1;

/// Maximum allowed agent count.
pub const MAX_AGENT_COUNT: i32 = 5;

/// Plan mode configuration.
///
/// Controls the behavior of plan mode, including agent counts for execution
/// and exploration phases.
///
/// # Environment Variables
///
/// - `COCODE_PLAN_AGENT_COUNT`: Number of agents for plan execution (1-5)
/// - `COCODE_PLAN_EXPLORE_AGENT_COUNT`: Number of agents for exploration (1-5)
///
/// # Example
///
/// ```json
/// {
///   "plan": {
///     "agent_count": 2,
///     "explore_agent_count": 4
///   }
/// }
/// ```
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct PlanModeConfig {
    /// Number of agents for plan execution.
    #[serde(default = "default_plan_agent_count")]
    pub agent_count: i32,

    /// Number of agents for exploration during planning.
    #[serde(default = "default_plan_explore_agent_count")]
    pub explore_agent_count: i32,
}

impl Default for PlanModeConfig {
    fn default() -> Self {
        Self {
            agent_count: DEFAULT_PLAN_AGENT_COUNT,
            explore_agent_count: DEFAULT_PLAN_EXPLORE_AGENT_COUNT,
        }
    }
}

impl PlanModeConfig {
    /// Validate configuration values.
    ///
    /// Returns an error message if any values are out of range (1-5).
    pub fn validate(&self) -> Result<(), String> {
        if !(MIN_AGENT_COUNT..=MAX_AGENT_COUNT).contains(&self.agent_count) {
            return Err(format!(
                "agent_count must be {MIN_AGENT_COUNT}-{MAX_AGENT_COUNT}, got {}",
                self.agent_count
            ));
        }

        if !(MIN_AGENT_COUNT..=MAX_AGENT_COUNT).contains(&self.explore_agent_count) {
            return Err(format!(
                "explore_agent_count must be {MIN_AGENT_COUNT}-{MAX_AGENT_COUNT}, got {}",
                self.explore_agent_count
            ));
        }

        Ok(())
    }

    /// Clamp agent_count to valid range.
    pub fn clamp_agent_count(&mut self) {
        self.agent_count = self.agent_count.clamp(MIN_AGENT_COUNT, MAX_AGENT_COUNT);
    }

    /// Clamp explore_agent_count to valid range.
    pub fn clamp_explore_agent_count(&mut self) {
        self.explore_agent_count = self
            .explore_agent_count
            .clamp(MIN_AGENT_COUNT, MAX_AGENT_COUNT);
    }

    /// Clamp all values to valid ranges.
    pub fn clamp_all(&mut self) {
        self.clamp_agent_count();
        self.clamp_explore_agent_count();
    }
}

fn default_plan_agent_count() -> i32 {
    DEFAULT_PLAN_AGENT_COUNT
}

fn default_plan_explore_agent_count() -> i32 {
    DEFAULT_PLAN_EXPLORE_AGENT_COUNT
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plan_config_default() {
        let config = PlanModeConfig::default();
        assert_eq!(config.agent_count, DEFAULT_PLAN_AGENT_COUNT);
        assert_eq!(config.explore_agent_count, DEFAULT_PLAN_EXPLORE_AGENT_COUNT);
    }

    #[test]
    fn test_plan_config_serde() {
        let json = r#"{"agent_count": 3, "explore_agent_count": 4}"#;
        let config: PlanModeConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.agent_count, 3);
        assert_eq!(config.explore_agent_count, 4);
    }

    #[test]
    fn test_plan_config_serde_defaults() {
        let json = r#"{}"#;
        let config: PlanModeConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.agent_count, DEFAULT_PLAN_AGENT_COUNT);
        assert_eq!(config.explore_agent_count, DEFAULT_PLAN_EXPLORE_AGENT_COUNT);
    }

    #[test]
    fn test_validate_valid_config() {
        let config = PlanModeConfig::default();
        assert!(config.validate().is_ok());

        let config = PlanModeConfig {
            agent_count: 5,
            explore_agent_count: 5,
        };
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_validate_invalid_agent_count() {
        let config = PlanModeConfig {
            agent_count: 0,
            explore_agent_count: 3,
        };
        assert!(config.validate().is_err());

        let config = PlanModeConfig {
            agent_count: 10,
            explore_agent_count: 3,
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_validate_invalid_explore_agent_count() {
        let config = PlanModeConfig {
            agent_count: 3,
            explore_agent_count: 0,
        };
        assert!(config.validate().is_err());

        let config = PlanModeConfig {
            agent_count: 3,
            explore_agent_count: 6,
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_clamp() {
        let mut config = PlanModeConfig {
            agent_count: 10,
            explore_agent_count: -5,
        };
        config.clamp_all();
        assert_eq!(config.agent_count, MAX_AGENT_COUNT);
        assert_eq!(config.explore_agent_count, MIN_AGENT_COUNT);
    }
}

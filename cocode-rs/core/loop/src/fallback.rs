use serde::Deserialize;
use serde::Serialize;

/// Configuration for model fallback behavior.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FallbackConfig {
    /// Whether model fallback is enabled.
    #[serde(default)]
    pub enabled: bool,

    /// Ordered list of fallback models to try when the primary model fails.
    #[serde(default)]
    pub fallback_models: Vec<String>,

    /// Maximum number of retry attempts before giving up.
    #[serde(default = "default_max_retries")]
    pub max_retries: i32,
}

fn default_max_retries() -> i32 {
    3
}

impl Default for FallbackConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            fallback_models: Vec::new(),
            max_retries: default_max_retries(),
        }
    }
}

/// Tracks the current fallback state during loop execution.
pub struct FallbackState {
    /// The model currently being used.
    pub current_model: String,

    /// Number of fallback attempts made so far.
    pub attempts: i32,

    /// History of all fallback transitions.
    pub history: Vec<FallbackAttempt>,
}

/// A single fallback transition record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FallbackAttempt {
    /// The model that failed.
    pub from_model: String,

    /// The model that was switched to.
    pub to_model: String,

    /// Human-readable reason for the fallback.
    pub reason: String,
}

impl FallbackState {
    /// Create a new fallback state for the given primary model.
    pub fn new(model: String) -> Self {
        Self {
            current_model: model,
            attempts: 0,
            history: Vec::new(),
        }
    }

    /// Returns `true` when a fallback should be attempted (fallback is enabled
    /// and we have not exceeded the retry limit).
    pub fn should_fallback(&self, config: &FallbackConfig) -> bool {
        config.enabled && self.attempts < config.max_retries && !config.fallback_models.is_empty()
    }

    /// Select the next fallback model, if one is available.
    ///
    /// Models are tried in the order they appear in `config.fallback_models`.
    /// Returns `None` when all options have been exhausted.
    pub fn next_model(&self, config: &FallbackConfig) -> Option<String> {
        if !config.enabled || config.fallback_models.is_empty() {
            return None;
        }

        let idx = self.attempts as usize;
        config.fallback_models.get(idx).cloned()
    }

    /// Record a fallback transition.
    pub fn record_fallback(&mut self, to: String, reason: String) {
        self.history.push(FallbackAttempt {
            from_model: self.current_model.clone(),
            to_model: to.clone(),
            reason,
        });
        self.current_model = to;
        self.attempts += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_fallback_config() {
        let config = FallbackConfig::default();
        assert!(!config.enabled);
        assert!(config.fallback_models.is_empty());
        assert_eq!(config.max_retries, 3);
    }

    #[test]
    fn test_should_fallback_disabled() {
        let config = FallbackConfig::default();
        let state = FallbackState::new("model-a".to_string());
        assert!(!state.should_fallback(&config));
    }

    #[test]
    fn test_should_fallback_enabled_with_models() {
        let config = FallbackConfig {
            enabled: true,
            fallback_models: vec!["model-b".to_string()],
            max_retries: 3,
        };
        let state = FallbackState::new("model-a".to_string());
        assert!(state.should_fallback(&config));
    }

    #[test]
    fn test_should_fallback_enabled_no_models() {
        let config = FallbackConfig {
            enabled: true,
            fallback_models: vec![],
            max_retries: 3,
        };
        let state = FallbackState::new("model-a".to_string());
        assert!(!state.should_fallback(&config));
    }

    #[test]
    fn test_should_fallback_max_retries_reached() {
        let config = FallbackConfig {
            enabled: true,
            fallback_models: vec!["model-b".to_string()],
            max_retries: 1,
        };
        let mut state = FallbackState::new("model-a".to_string());
        state.record_fallback("model-b".to_string(), "error".to_string());
        assert!(!state.should_fallback(&config));
    }

    #[test]
    fn test_next_model_sequence() {
        let config = FallbackConfig {
            enabled: true,
            fallback_models: vec!["model-b".to_string(), "model-c".to_string()],
            max_retries: 3,
        };
        let mut state = FallbackState::new("model-a".to_string());

        assert_eq!(state.next_model(&config), Some("model-b".to_string()));

        state.record_fallback("model-b".to_string(), "error 1".to_string());
        assert_eq!(state.next_model(&config), Some("model-c".to_string()));

        state.record_fallback("model-c".to_string(), "error 2".to_string());
        assert_eq!(state.next_model(&config), None);
    }

    #[test]
    fn test_next_model_disabled() {
        let config = FallbackConfig::default();
        let state = FallbackState::new("model-a".to_string());
        assert_eq!(state.next_model(&config), None);
    }

    #[test]
    fn test_record_fallback() {
        let mut state = FallbackState::new("model-a".to_string());
        assert_eq!(state.attempts, 0);
        assert!(state.history.is_empty());

        state.record_fallback("model-b".to_string(), "rate limited".to_string());

        assert_eq!(state.current_model, "model-b");
        assert_eq!(state.attempts, 1);
        assert_eq!(state.history.len(), 1);
        assert_eq!(state.history[0].from_model, "model-a");
        assert_eq!(state.history[0].to_model, "model-b");
        assert_eq!(state.history[0].reason, "rate limited");
    }
}

//! Unified model specification type.

use serde::Deserialize;
use serde::Deserializer;
use serde::Serialize;
use serde::Serializer;
use std::fmt;
use std::str::FromStr;

/// Unified model specification: "{provider}/{model}".
///
/// Provides a single string format for specifying both provider and model.
///
/// # Examples
///
/// ```
/// use cocode_protocol::model::ModelSpec;
///
/// let spec: ModelSpec = "anthropic/claude-opus-4".parse().unwrap();
/// assert_eq!(spec.provider, "anthropic");
/// assert_eq!(spec.model, "claude-opus-4");
/// assert_eq!(spec.to_string(), "anthropic/claude-opus-4");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ModelSpec {
    /// Provider name (e.g., "anthropic", "openai", "genai").
    pub provider: String,
    /// Model ID (e.g., "claude-opus-4", "gpt-5").
    pub model: String,
}

impl ModelSpec {
    /// Create a new model specification.
    pub fn new(provider: impl Into<String>, model: impl Into<String>) -> Self {
        Self {
            provider: provider.into(),
            model: model.into(),
        }
    }
}

impl fmt::Display for ModelSpec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}", self.provider, self.model)
    }
}

/// Error returned when parsing a `ModelSpec` from a string fails.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelSpecParseError(pub String);

impl fmt::Display for ModelSpecParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for ModelSpecParseError {}

impl FromStr for ModelSpec {
    type Err = ModelSpecParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parts: Vec<&str> = s.splitn(2, '/').collect();
        if parts.len() != 2 || parts[0].is_empty() || parts[1].is_empty() {
            return Err(ModelSpecParseError(format!(
                "invalid format: expected 'provider/model', got '{s}'"
            )));
        }
        Ok(Self::new(parts[0], parts[1]))
    }
}

impl Serialize for ModelSpec {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for ModelSpec {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        s.parse()
            .map_err(|e: ModelSpecParseError| serde::de::Error::custom(e.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_valid() {
        let spec: ModelSpec = "anthropic/claude-opus-4".parse().unwrap();
        assert_eq!(spec.provider, "anthropic");
        assert_eq!(spec.model, "claude-opus-4");
    }

    #[test]
    fn test_parse_with_slashes_in_model() {
        // Model names can contain slashes (e.g., "accounts/fireworks/models/llama-v3")
        let spec: ModelSpec = "fireworks/accounts/fireworks/models/llama-v3"
            .parse()
            .unwrap();
        assert_eq!(spec.provider, "fireworks");
        assert_eq!(spec.model, "accounts/fireworks/models/llama-v3");
    }

    #[test]
    fn test_parse_invalid_no_slash() {
        let result: Result<ModelSpec, _> = "claude-opus-4".parse();
        assert!(result.is_err());
        assert!(result.unwrap_err().0.contains("invalid format"));
    }

    #[test]
    fn test_parse_invalid_empty_provider() {
        let result: Result<ModelSpec, _> = "/claude-opus-4".parse();
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_invalid_empty_model() {
        let result: Result<ModelSpec, _> = "anthropic/".parse();
        assert!(result.is_err());
    }

    #[test]
    fn test_display() {
        let spec = ModelSpec::new("openai", "gpt-5");
        assert_eq!(spec.to_string(), "openai/gpt-5");
    }

    #[test]
    fn test_serde_roundtrip() {
        let spec = ModelSpec::new("anthropic", "claude-opus-4");
        let json = serde_json::to_string(&spec).unwrap();
        assert_eq!(json, r#""anthropic/claude-opus-4""#);

        let parsed: ModelSpec = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, spec);
    }

    #[test]
    fn test_serde_deserialize_invalid() {
        let result: Result<ModelSpec, _> = serde_json::from_str(r#""invalid""#);
        assert!(result.is_err());
    }

    #[test]
    fn test_equality() {
        let a = ModelSpec::new("anthropic", "claude-opus-4");
        let b = ModelSpec::new("anthropic", "claude-opus-4");
        let c = ModelSpec::new("openai", "gpt-5");
        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    #[test]
    fn test_hash() {
        use std::collections::HashSet;

        let mut set = HashSet::new();
        set.insert(ModelSpec::new("anthropic", "claude-opus-4"));
        set.insert(ModelSpec::new("openai", "gpt-5"));

        assert!(set.contains(&ModelSpec::new("anthropic", "claude-opus-4")));
        assert!(!set.contains(&ModelSpec::new("genai", "gemini-3")));
    }
}

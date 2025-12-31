//! Compact strategy enumeration.
//!
//! Defines the available compaction strategies and their selection logic.

use serde::Deserialize;
use serde::Serialize;

/// Available compaction strategies.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompactStrategy {
    /// Fast local compression of tool results only (no API call)
    MicroCompact,

    /// Full LLM-based summarization (inline)
    #[default]
    FullCompact,

    /// Remote API-based compact (OpenAI)
    RemoteCompact,
}

impl CompactStrategy {
    /// Returns true if this strategy requires an API call.
    pub fn requires_api(self) -> bool {
        match self {
            CompactStrategy::MicroCompact => false,
            CompactStrategy::FullCompact | CompactStrategy::RemoteCompact => true,
        }
    }

    /// Returns the display name for this strategy.
    pub fn display_name(self) -> &'static str {
        match self {
            CompactStrategy::MicroCompact => "micro",
            CompactStrategy::FullCompact => "full",
            CompactStrategy::RemoteCompact => "remote",
        }
    }
}

impl std::fmt::Display for CompactStrategy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.display_name())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn strategy_requires_api() {
        assert!(!CompactStrategy::MicroCompact.requires_api());
        assert!(CompactStrategy::FullCompact.requires_api());
        assert!(CompactStrategy::RemoteCompact.requires_api());
    }

    #[test]
    fn strategy_display_names() {
        assert_eq!(CompactStrategy::MicroCompact.display_name(), "micro");
        assert_eq!(CompactStrategy::FullCompact.display_name(), "full");
        assert_eq!(CompactStrategy::RemoteCompact.display_name(), "remote");
    }

    #[test]
    fn strategy_default_is_full() {
        assert_eq!(CompactStrategy::default(), CompactStrategy::FullCompact);
    }

    #[test]
    fn strategy_serialization() {
        let json = serde_json::to_string(&CompactStrategy::MicroCompact).expect("serialize");
        assert_eq!(json, "\"micro_compact\"");

        let parsed: CompactStrategy = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed, CompactStrategy::MicroCompact);
    }
}

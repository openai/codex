//! Agent type identification for inference requests.

use serde::Deserialize;
use serde::Serialize;
use std::fmt;

/// Type of agent making an inference request.
///
/// `AgentKind` provides context about the caller for logging, telemetry,
/// and context-specific behavior during inference.
///
/// # Example
///
/// ```
/// use cocode_protocol::execution::AgentKind;
///
/// // Main conversation agent
/// let main = AgentKind::Main;
///
/// // Subagent spawned via Task tool
/// let subagent = AgentKind::Subagent {
///     parent_session_id: "session-123".to_string(),
///     agent_type: "explore".to_string(),
/// };
///
/// // Context compaction
/// let compact = AgentKind::Compaction;
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum AgentKind {
    /// Main conversation agent (primary user interaction).
    Main,

    /// Subagent spawned via Task tool.
    Subagent {
        /// Parent session ID for tracing.
        parent_session_id: String,
        /// Agent type identifier (e.g., "explore", "plan", "code-review").
        agent_type: String,
    },

    /// Session memory extraction agent.
    ///
    /// Used for extracting and summarizing conversation memory.
    Extraction,

    /// Context compaction agent.
    ///
    /// Used for compressing context when approaching token limits.
    Compaction,
}

impl AgentKind {
    /// Create a main agent kind.
    pub fn main() -> Self {
        Self::Main
    }

    /// Create a subagent kind.
    pub fn subagent(parent_session_id: impl Into<String>, agent_type: impl Into<String>) -> Self {
        Self::Subagent {
            parent_session_id: parent_session_id.into(),
            agent_type: agent_type.into(),
        }
    }

    /// Create an extraction agent kind.
    pub fn extraction() -> Self {
        Self::Extraction
    }

    /// Create a compaction agent kind.
    pub fn compaction() -> Self {
        Self::Compaction
    }

    /// Check if this is the main agent.
    pub fn is_main(&self) -> bool {
        matches!(self, Self::Main)
    }

    /// Check if this is a subagent.
    pub fn is_subagent(&self) -> bool {
        matches!(self, Self::Subagent { .. })
    }

    /// Check if this is an extraction agent.
    pub fn is_extraction(&self) -> bool {
        matches!(self, Self::Extraction)
    }

    /// Check if this is a compaction agent.
    pub fn is_compaction(&self) -> bool {
        matches!(self, Self::Compaction)
    }

    /// Get the agent type string for telemetry/logging.
    pub fn agent_type_str(&self) -> &str {
        match self {
            Self::Main => "main",
            Self::Subagent { agent_type, .. } => agent_type,
            Self::Extraction => "extraction",
            Self::Compaction => "compaction",
        }
    }

    /// Get the parent session ID if this is a subagent.
    pub fn parent_session_id(&self) -> Option<&str> {
        match self {
            Self::Subagent {
                parent_session_id, ..
            } => Some(parent_session_id),
            _ => None,
        }
    }
}

impl Default for AgentKind {
    fn default() -> Self {
        Self::Main
    }
}

impl fmt::Display for AgentKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Main => write!(f, "main"),
            Self::Subagent { agent_type, .. } => write!(f, "subagent:{}", agent_type),
            Self::Extraction => write!(f, "extraction"),
            Self::Compaction => write!(f, "compaction"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_main_kind() {
        let kind = AgentKind::main();
        assert!(kind.is_main());
        assert!(!kind.is_subagent());
        assert_eq!(kind.agent_type_str(), "main");
        assert_eq!(kind.to_string(), "main");
    }

    #[test]
    fn test_subagent_kind() {
        let kind = AgentKind::subagent("session-123", "explore");
        assert!(kind.is_subagent());
        assert!(!kind.is_main());
        assert_eq!(kind.agent_type_str(), "explore");
        assert_eq!(kind.parent_session_id(), Some("session-123"));
        assert_eq!(kind.to_string(), "subagent:explore");
    }

    #[test]
    fn test_extraction_kind() {
        let kind = AgentKind::extraction();
        assert!(kind.is_extraction());
        assert!(!kind.is_main());
        assert_eq!(kind.agent_type_str(), "extraction");
        assert_eq!(kind.to_string(), "extraction");
    }

    #[test]
    fn test_compaction_kind() {
        let kind = AgentKind::compaction();
        assert!(kind.is_compaction());
        assert!(!kind.is_main());
        assert_eq!(kind.agent_type_str(), "compaction");
        assert_eq!(kind.to_string(), "compaction");
    }

    #[test]
    fn test_default() {
        assert_eq!(AgentKind::default(), AgentKind::Main);
    }

    #[test]
    fn test_serde_main() {
        let kind = AgentKind::Main;
        let json = serde_json::to_string(&kind).unwrap();
        let parsed: AgentKind = serde_json::from_str(&json).unwrap();
        assert_eq!(kind, parsed);
    }

    #[test]
    fn test_serde_subagent() {
        let kind = AgentKind::subagent("session-123", "explore");
        let json = serde_json::to_string(&kind).unwrap();
        assert!(json.contains("Subagent"));
        assert!(json.contains("session-123"));
        assert!(json.contains("explore"));
        let parsed: AgentKind = serde_json::from_str(&json).unwrap();
        assert_eq!(kind, parsed);
    }

    #[test]
    fn test_serde_extraction() {
        let kind = AgentKind::Extraction;
        let json = serde_json::to_string(&kind).unwrap();
        let parsed: AgentKind = serde_json::from_str(&json).unwrap();
        assert_eq!(kind, parsed);
    }

    #[test]
    fn test_serde_compaction() {
        let kind = AgentKind::Compaction;
        let json = serde_json::to_string(&kind).unwrap();
        let parsed: AgentKind = serde_json::from_str(&json).unwrap();
        assert_eq!(kind, parsed);
    }
}

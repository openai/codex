use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// Represents a subagent running in the background.
///
/// Background agents write their output to a file so the parent can retrieve
/// results asynchronously.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackgroundAgent {
    /// Unique identifier for the background agent instance.
    pub agent_id: String,

    /// Path to the file where the agent writes its output.
    pub output_file: PathBuf,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_background_agent_creation() {
        let agent = BackgroundAgent {
            agent_id: "bg-123".to_string(),
            output_file: PathBuf::from("/tmp/agent-bg-123.jsonl"),
        };
        assert_eq!(agent.agent_id, "bg-123");
        assert_eq!(agent.output_file, PathBuf::from("/tmp/agent-bg-123.jsonl"));
    }

    #[test]
    fn test_background_agent_serde() {
        let agent = BackgroundAgent {
            agent_id: "bg-456".to_string(),
            output_file: PathBuf::from("/tmp/output.jsonl"),
        };
        let json = serde_json::to_string(&agent).expect("serialize");
        let back: BackgroundAgent = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back.agent_id, "bg-456");
        assert_eq!(back.output_file, PathBuf::from("/tmp/output.jsonl"));
    }
}

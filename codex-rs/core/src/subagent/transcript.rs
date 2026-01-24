//! Transcript storage for agent resume functionality.

use dashmap::DashMap;
use serde::Deserialize;
use serde::Serialize;
use std::time::Instant;

/// Store for agent transcripts (for resume functionality).
#[derive(Debug, Default)]
pub struct TranscriptStore {
    transcripts: DashMap<String, AgentTranscript>,
}

/// Recorded transcript for an agent execution.
#[derive(Debug, Clone)]
pub struct AgentTranscript {
    /// Agent instance ID.
    pub agent_id: String,
    /// Type of the agent.
    pub agent_type: String,
    /// Messages in the transcript.
    pub messages: Vec<TranscriptMessage>,
    /// When the transcript was created.
    pub created_at: Instant,
    /// Whether this is a sidechain (subagent) transcript.
    pub is_sidechain: bool,
}

/// A message in the transcript.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscriptMessage {
    /// Role of the message sender.
    pub role: MessageRole,
    /// Text content of the message.
    pub content: String,
    /// Tool calls made in this message.
    #[serde(default)]
    pub tool_calls: Option<Vec<TranscriptToolCall>>,
    /// Tool results in this message.
    #[serde(default)]
    pub tool_results: Option<Vec<TranscriptToolResult>>,
    /// Unix timestamp of the message.
    pub timestamp: i64,
}

/// Role in a transcript message.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum MessageRole {
    System,
    User,
    Assistant,
    Tool,
}

/// A tool call recorded in the transcript.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscriptToolCall {
    pub id: String,
    pub name: String,
    pub arguments: serde_json::Value,
}

/// A tool result recorded in the transcript.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscriptToolResult {
    pub tool_call_id: String,
    pub content: String,
    pub success: bool,
}

impl TranscriptStore {
    /// Create a new transcript store.
    pub fn new() -> Self {
        Self {
            transcripts: DashMap::new(),
        }
    }

    /// Initialize a new transcript for an agent.
    pub fn init_transcript(&self, agent_id: String, agent_type: String) {
        let transcript = AgentTranscript {
            agent_id: agent_id.clone(),
            agent_type,
            messages: Vec::new(),
            created_at: Instant::now(),
            is_sidechain: true,
        };
        self.transcripts.insert(agent_id, transcript);
    }

    /// Record a message to an agent's transcript.
    pub fn record_message(&self, agent_id: &str, message: TranscriptMessage) {
        if let Some(mut transcript) = self.transcripts.get_mut(agent_id) {
            transcript.messages.push(message);
        }
    }

    /// Load transcript for resume.
    pub fn load_transcript(&self, agent_id: &str) -> Option<Vec<TranscriptMessage>> {
        self.transcripts.get(agent_id).map(|t| t.messages.clone())
    }

    /// Get the agent type for a transcript.
    pub fn get_agent_type(&self, agent_id: &str) -> Option<String> {
        self.transcripts.get(agent_id).map(|t| t.agent_type.clone())
    }

    /// Remove transcripts older than specified duration.
    pub fn cleanup_old_transcripts(&self, older_than: std::time::Duration) {
        let now = Instant::now();
        self.transcripts
            .retain(|_, transcript| now.duration_since(transcript.created_at) < older_than);
    }

    /// Check if a transcript exists.
    pub fn exists(&self, agent_id: &str) -> bool {
        self.transcripts.contains_key(agent_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_init_and_record() {
        let store = TranscriptStore::new();

        store.init_transcript("agent-1".to_string(), "Explore".to_string());

        let msg = TranscriptMessage {
            role: MessageRole::User,
            content: "Hello".to_string(),
            tool_calls: None,
            tool_results: None,
            timestamp: 12345,
        };

        store.record_message("agent-1", msg);

        let transcript = store.load_transcript("agent-1");
        assert!(transcript.is_some());
        assert_eq!(transcript.unwrap().len(), 1);
    }

    #[test]
    fn test_load_nonexistent() {
        let store = TranscriptStore::new();
        let transcript = store.load_transcript("nonexistent");
        assert!(transcript.is_none());
    }

    #[test]
    fn test_exists() {
        let store = TranscriptStore::new();
        assert!(!store.exists("agent-1"));

        store.init_transcript("agent-1".to_string(), "Test".to_string());
        assert!(store.exists("agent-1"));
    }
}

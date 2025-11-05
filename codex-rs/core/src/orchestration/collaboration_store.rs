//! Collaboration store for sharing context between sub-agents.
//!
//! Provides a thread-safe shared memory store that allows sub-agents to
//! communicate and share results during parallel execution.

use crate::agents::AgentResult;
use crate::agents::AgentStatus;
use dashmap::DashMap;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;
use std::sync::Arc;
use std::time::SystemTime;

/// Message passed between agents.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentMessage {
    /// Sender agent name
    pub from: String,
    /// Recipient agent name (or "broadcast" for all)
    pub to: String,
    /// Message content
    pub content: Value,
    /// Timestamp
    pub timestamp: SystemTime,
    /// Message priority (higher = more urgent)
    pub priority: u8,
}

/// Shared store for agent collaboration during orchestrated execution.
#[derive(Clone)]
pub struct CollaborationStore {
    /// Shared context data (key-value store)
    shared_context: Arc<DashMap<String, Value>>,

    /// Agent execution results
    agent_results: Arc<DashMap<String, AgentResult>>,

    /// Task-level metadata
    task_metadata: Arc<DashMap<String, Value>>,

    /// Message queue for inter-agent communication
    message_queue: Arc<DashMap<String, Vec<AgentMessage>>>,
}

impl CollaborationStore {
    /// Create a new collaboration store.
    pub fn new() -> Self {
        Self {
            shared_context: Arc::new(DashMap::new()),
            agent_results: Arc::new(DashMap::new()),
            task_metadata: Arc::new(DashMap::new()),
            message_queue: Arc::new(DashMap::new()),
        }
    }

    /// Share context data with a specific key.
    pub fn share_context(&self, key: String, value: Value) {
        self.shared_context.insert(key, value);
    }

    /// Get context data by key.
    pub fn get_context(&self, key: &str) -> Option<Value> {
        self.shared_context
            .get(key)
            .map(|entry| entry.value().clone())
    }

    /// Get all context entries.
    pub fn get_all_context(&self) -> Vec<(String, Value)> {
        self.shared_context
            .iter()
            .map(|entry| (entry.key().clone(), entry.value().clone()))
            .collect()
    }

    /// Store an agent's result.
    pub fn store_agent_result(&self, agent_name: String, result: AgentResult) {
        self.agent_results.insert(agent_name, result);
    }

    /// Get a specific agent's result.
    pub fn get_agent_result(&self, agent_name: &str) -> Option<AgentResult> {
        self.agent_results
            .get(agent_name)
            .map(|entry| entry.value().clone())
    }

    /// Get all agent results.
    pub fn get_all_results(&self) -> Vec<(String, AgentResult)> {
        self.agent_results
            .iter()
            .map(|entry| (entry.key().clone(), entry.value().clone()))
            .collect()
    }

    /// Get a summary of all completed agents.
    pub fn get_results_summary(&self) -> String {
        let results = self.get_all_results();
        if results.is_empty() {
            return "No agents have completed yet.".to_string();
        }

        let mut summary = String::from("Completed agents:\n");
        for (agent_name, result) in results {
            let status_desc = match result.status {
                AgentStatus::Pending => "Pending",
                AgentStatus::Running => "Running",
                AgentStatus::Completed => "Completed",
                AgentStatus::Failed => "Failed",
                AgentStatus::Cancelled => "Cancelled",
            };
            summary.push_str(&format!(
                "- {}: {} | Tokens: {} | Duration: {:.2}s\n",
                agent_name, status_desc, result.tokens_used, result.duration_secs
            ));
        }
        summary
    }

    /// Set task-level metadata.
    pub fn set_metadata(&self, key: String, value: Value) {
        self.task_metadata.insert(key, value);
    }

    /// Get task-level metadata.
    pub fn get_metadata(&self, key: &str) -> Option<Value> {
        self.task_metadata
            .get(key)
            .map(|entry| entry.value().clone())
    }

    /// Clear all data (for cleanup after task completion).
    pub fn clear(&self) {
        self.shared_context.clear();
        self.agent_results.clear();
        self.task_metadata.clear();
        self.message_queue.clear();
    }

    /// Get the number of completed agents.
    pub fn completed_agent_count(&self) -> usize {
        self.agent_results.len()
    }

    /// Check if a specific agent has completed.
    pub fn has_agent_completed(&self, agent_name: &str) -> bool {
        self.agent_results.contains_key(agent_name)
    }

    // ==================== Message Passing Methods ====================

    /// Send a message from one agent to another (or broadcast).
    pub fn send_message(&self, from: String, to: String, content: Value, priority: u8) {
        let message = AgentMessage {
            from,
            to: to.clone(),
            content,
            timestamp: SystemTime::now(),
            priority,
        };

        self.message_queue
            .entry(to)
            .or_insert_with(Vec::new)
            .push(message);
    }

    /// Broadcast a message to all agents.
    pub fn broadcast_message(&self, from: String, content: Value, priority: u8) {
        self.send_message(from, "broadcast".to_string(), content, priority);
    }

    /// Get messages for a specific agent, sorted by priority (high to low).
    pub fn get_messages(&self, agent_name: &str) -> Vec<AgentMessage> {
        let mut messages = Vec::new();

        // Get direct messages
        if let Some(entry) = self.message_queue.get(agent_name) {
            messages.extend(entry.value().clone());
        }

        // Get broadcast messages
        if let Some(entry) = self.message_queue.get("broadcast") {
            messages.extend(entry.value().clone());
        }

        // Sort by priority (descending) and timestamp
        messages.sort_by(|a, b| {
            b.priority
                .cmp(&a.priority)
                .then_with(|| a.timestamp.cmp(&b.timestamp))
        });

        messages
    }

    /// Clear messages for a specific agent (after reading).
    pub fn clear_messages(&self, agent_name: &str) {
        self.message_queue.remove(agent_name);
    }

    /// Get unread message count for an agent.
    pub fn unread_message_count(&self, agent_name: &str) -> usize {
        let mut count = 0;

        if let Some(entry) = self.message_queue.get(agent_name) {
            count += entry.value().len();
        }

        if let Some(entry) = self.message_queue.get("broadcast") {
            count += entry.value().len();
        }

        count
    }
}

impl Default for CollaborationStore {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agents::AgentStatus;

    #[test]
    fn test_context_sharing() {
        let store = CollaborationStore::new();

        store.share_context(
            "test_key".to_string(),
            Value::String("test_value".to_string()),
        );

        let value = store.get_context("test_key");
        assert!(value.is_some());
        assert_eq!(value.unwrap(), Value::String("test_value".to_string()));
    }

    #[test]
    fn test_agent_results() {
        let store = CollaborationStore::new();

        let result = AgentResult {
            agent_name: "test-agent".to_string(),
            status: AgentStatus::Completed,
            artifacts: vec![],
            tokens_used: 1000,
            duration_secs: 1.5,
            error: None,
        };

        store.store_agent_result("test-agent".to_string(), result.clone());

        let retrieved = store.get_agent_result("test-agent");
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().agent_name, "test-agent");
    }

    #[test]
    fn test_results_summary() {
        let store = CollaborationStore::new();

        let result1 = AgentResult {
            agent_name: "agent1".to_string(),
            status: AgentStatus::Completed,
            artifacts: vec![],
            tokens_used: 1500,
            duration_secs: 2.0,
            error: None,
        };

        let result2 = AgentResult {
            agent_name: "agent2".to_string(),
            status: AgentStatus::Completed,
            artifacts: vec![],
            tokens_used: 2000,
            duration_secs: 3.0,
            error: None,
        };

        store.store_agent_result("agent1".to_string(), result1);
        store.store_agent_result("agent2".to_string(), result2);

        let summary = store.get_results_summary();
        assert!(summary.contains("agent1"));
        assert!(summary.contains("agent2"));
    }

    #[test]
    fn test_clear() {
        let store = CollaborationStore::new();

        store.share_context("key1".to_string(), Value::String("value1".to_string()));
        assert!(store.get_context("key1").is_some());

        store.clear();
        assert!(store.get_context("key1").is_none());
    }
}

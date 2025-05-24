//! Core type definitions for the lifecycle hooks system.

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Lifecycle events that can trigger hooks.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum LifecycleEvent {
    /// Session lifecycle events
    SessionStart {
        session_id: String,
        model: String,
        cwd: PathBuf,
        timestamp: chrono::DateTime<chrono::Utc>,
    },
    SessionEnd {
        session_id: String,
        duration: Duration,
        timestamp: chrono::DateTime<chrono::Utc>,
    },

    /// Task lifecycle events
    TaskStart {
        task_id: String,
        session_id: String,
        prompt: String,
        timestamp: chrono::DateTime<chrono::Utc>,
    },
    TaskComplete {
        task_id: String,
        session_id: String,
        success: bool,
        output: Option<String>,
        duration: Duration,
        timestamp: chrono::DateTime<chrono::Utc>,
    },

    /// Execution lifecycle events
    ExecBefore {
        call_id: String,
        task_id: String,
        command: Vec<String>,
        cwd: PathBuf,
        timestamp: chrono::DateTime<chrono::Utc>,
    },
    ExecAfter {
        call_id: String,
        task_id: String,
        command: Vec<String>,
        exit_code: i32,
        stdout: String,
        stderr: String,
        duration: Duration,
        timestamp: chrono::DateTime<chrono::Utc>,
    },

    /// Patch lifecycle events
    PatchBefore {
        call_id: String,
        task_id: String,
        changes: HashMap<PathBuf, String>, // file path -> change description
        timestamp: chrono::DateTime<chrono::Utc>,
    },
    PatchAfter {
        call_id: String,
        task_id: String,
        success: bool,
        applied_files: Vec<PathBuf>,
        duration: Duration,
        timestamp: chrono::DateTime<chrono::Utc>,
    },

    /// MCP tool lifecycle events
    McpToolBefore {
        call_id: String,
        task_id: String,
        server: String,
        tool: String,
        arguments: Option<serde_json::Value>,
        timestamp: chrono::DateTime<chrono::Utc>,
    },
    McpToolAfter {
        call_id: String,
        task_id: String,
        server: String,
        tool: String,
        success: bool,
        result: Option<serde_json::Value>,
        duration: Duration,
        timestamp: chrono::DateTime<chrono::Utc>,
    },

    /// Agent interaction events
    AgentMessage {
        task_id: String,
        message: String,
        reasoning: Option<String>,
        timestamp: chrono::DateTime<chrono::Utc>,
    },

    /// Error events
    ErrorOccurred {
        task_id: Option<String>,
        error: String,
        context: ErrorContext,
        timestamp: chrono::DateTime<chrono::Utc>,
    },
}

/// Context information for error events.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ErrorContext {
    pub component: String,
    pub operation: Option<String>,
    pub details: HashMap<String, serde_json::Value>,
}

/// Enumeration of lifecycle event types for filtering and matching.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LifecycleEventType {
    SessionStart,
    SessionEnd,
    TaskStart,
    TaskComplete,
    ExecBefore,
    ExecAfter,
    PatchBefore,
    PatchAfter,
    McpToolBefore,
    McpToolAfter,
    AgentMessage,
    ErrorOccurred,
}

impl LifecycleEvent {
    /// Get the event type for this lifecycle event.
    pub fn event_type(&self) -> LifecycleEventType {
        match self {
            LifecycleEvent::SessionStart { .. } => LifecycleEventType::SessionStart,
            LifecycleEvent::SessionEnd { .. } => LifecycleEventType::SessionEnd,
            LifecycleEvent::TaskStart { .. } => LifecycleEventType::TaskStart,
            LifecycleEvent::TaskComplete { .. } => LifecycleEventType::TaskComplete,
            LifecycleEvent::ExecBefore { .. } => LifecycleEventType::ExecBefore,
            LifecycleEvent::ExecAfter { .. } => LifecycleEventType::ExecAfter,
            LifecycleEvent::PatchBefore { .. } => LifecycleEventType::PatchBefore,
            LifecycleEvent::PatchAfter { .. } => LifecycleEventType::PatchAfter,
            LifecycleEvent::McpToolBefore { .. } => LifecycleEventType::McpToolBefore,
            LifecycleEvent::McpToolAfter { .. } => LifecycleEventType::McpToolAfter,
            LifecycleEvent::AgentMessage { .. } => LifecycleEventType::AgentMessage,
            LifecycleEvent::ErrorOccurred { .. } => LifecycleEventType::ErrorOccurred,
        }
    }

    /// Get the task ID associated with this event, if any.
    pub fn task_id(&self) -> Option<&str> {
        match self {
            LifecycleEvent::SessionStart { .. } | LifecycleEvent::SessionEnd { .. } => None,
            LifecycleEvent::TaskStart { task_id, .. }
            | LifecycleEvent::TaskComplete { task_id, .. }
            | LifecycleEvent::ExecBefore { task_id, .. }
            | LifecycleEvent::ExecAfter { task_id, .. }
            | LifecycleEvent::PatchBefore { task_id, .. }
            | LifecycleEvent::PatchAfter { task_id, .. }
            | LifecycleEvent::McpToolBefore { task_id, .. }
            | LifecycleEvent::McpToolAfter { task_id, .. }
            | LifecycleEvent::AgentMessage { task_id, .. } => Some(task_id),
            LifecycleEvent::ErrorOccurred { task_id, .. } => task_id.as_deref(),
        }
    }

    /// Get the timestamp for this event.
    pub fn timestamp(&self) -> chrono::DateTime<chrono::Utc> {
        match self {
            LifecycleEvent::SessionStart { timestamp, .. }
            | LifecycleEvent::SessionEnd { timestamp, .. }
            | LifecycleEvent::TaskStart { timestamp, .. }
            | LifecycleEvent::TaskComplete { timestamp, .. }
            | LifecycleEvent::ExecBefore { timestamp, .. }
            | LifecycleEvent::ExecAfter { timestamp, .. }
            | LifecycleEvent::PatchBefore { timestamp, .. }
            | LifecycleEvent::PatchAfter { timestamp, .. }
            | LifecycleEvent::McpToolBefore { timestamp, .. }
            | LifecycleEvent::McpToolAfter { timestamp, .. }
            | LifecycleEvent::AgentMessage { timestamp, .. }
            | LifecycleEvent::ErrorOccurred { timestamp, .. } => *timestamp,
        }
    }
}

/// Types of hooks that can be executed.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum HookType {
    /// Execute a shell script or command.
    Script {
        command: Vec<String>,
        cwd: Option<PathBuf>,
        environment: HashMap<String, String>,
        timeout: Option<Duration>,
    },
    /// Send an HTTP request to a webhook URL.
    Webhook {
        url: String,
        method: HttpMethod,
        headers: HashMap<String, String>,
        timeout: Option<Duration>,
        retry_count: Option<u32>,
    },
    /// Call an MCP tool.
    McpTool {
        server: String,
        tool: String,
        timeout: Option<Duration>,
    },
    /// Execute a custom binary/executable.
    Executable {
        path: PathBuf,
        args: Vec<String>,
        cwd: Option<PathBuf>,
        environment: HashMap<String, String>,
        timeout: Option<Duration>,
    },
}

/// HTTP methods for webhook hooks.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "UPPERCASE")]
pub enum HttpMethod {
    Get,
    Post,
    Put,
    Patch,
    Delete,
}

/// Hook execution modes.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum HookExecutionMode {
    /// Execute the hook asynchronously without blocking.
    Async,
    /// Execute the hook synchronously and wait for completion.
    Blocking,
    /// Execute the hook and ignore the result (fire-and-forget).
    FireAndForget,
}

impl Default for HookExecutionMode {
    fn default() -> Self {
        HookExecutionMode::Async
    }
}

/// Result of hook execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookResult {
    pub success: bool,
    pub output: Option<String>,
    pub error: Option<String>,
    pub duration: Duration,
    pub metadata: HashMap<String, serde_json::Value>,
}

impl HookResult {
    /// Create a successful hook result.
    pub fn success(output: Option<String>, duration: Duration) -> Self {
        Self {
            success: true,
            output,
            error: None,
            duration,
            metadata: HashMap::new(),
        }
    }

    /// Create a failed hook result.
    pub fn failure(error: String, duration: Duration) -> Self {
        Self {
            success: false,
            output: None,
            error: Some(error),
            duration,
            metadata: HashMap::new(),
        }
    }

    /// Add metadata to the hook result.
    pub fn with_metadata(mut self, key: String, value: serde_json::Value) -> Self {
        self.metadata.insert(key, value);
        self
    }
}

/// Errors that can occur in the hooks system.
#[derive(Error, Debug)]
pub enum HookError {
    #[error("Hook configuration error: {0}")]
    Configuration(String),

    #[error("Hook execution error: {0}")]
    Execution(String),

    #[error("Hook timeout: {0}")]
    Timeout(String),

    #[error("Hook validation error: {0}")]
    Validation(String),

    #[error("Hook registry error: {0}")]
    Registry(String),

    #[error("Hook context error: {0}")]
    Context(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON serialization error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("HTTP error: {0}")]
    Http(String),

    #[error("MCP error: {0}")]
    Mcp(String),
}

/// Hook execution priority.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct HookPriority(pub u32);

impl Default for HookPriority {
    fn default() -> Self {
        HookPriority(100) // Default priority
    }
}

impl HookPriority {
    pub const HIGHEST: HookPriority = HookPriority(0);
    pub const HIGH: HookPriority = HookPriority(25);
    pub const NORMAL: HookPriority = HookPriority(100);
    pub const LOW: HookPriority = HookPriority(200);
    pub const LOWEST: HookPriority = HookPriority(u32::MAX);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lifecycle_event_type() {
        let event = LifecycleEvent::TaskStart {
            task_id: "test".to_string(),
            session_id: "session".to_string(),
            prompt: "test prompt".to_string(),
            timestamp: chrono::Utc::now(),
        };
        assert_eq!(event.event_type(), LifecycleEventType::TaskStart);
        assert_eq!(event.task_id(), Some("test"));
    }

    #[test]
    fn test_hook_result() {
        let result = HookResult::success(Some("output".to_string()), Duration::from_secs(1));
        assert!(result.success);
        assert_eq!(result.output, Some("output".to_string()));

        let result = HookResult::failure("error".to_string(), Duration::from_secs(1));
        assert!(!result.success);
        assert_eq!(result.error, Some("error".to_string()));
    }

    #[test]
    fn test_hook_priority_ordering() {
        assert!(HookPriority::HIGHEST < HookPriority::HIGH);
        assert!(HookPriority::HIGH < HookPriority::NORMAL);
        assert!(HookPriority::NORMAL < HookPriority::LOW);
        assert!(HookPriority::LOW < HookPriority::LOWEST);
    }
}

//! Session storage for MCP compatibility mode
//!
//! This module provides session response storage for synchronous MCP clients
//! that need immediate responses instead of streaming events.

use std::collections::HashMap;
use uuid::Uuid;

/// Response status for compatibility mode sessions
#[derive(Debug, Clone)]
pub enum SessionStatus {
    /// Session is currently running
    Running,
    /// Session completed successfully
    Completed,
    /// Session failed with an error
    Failed,
}

/// Stored response for compatibility mode sessions
#[derive(Debug, Clone)]
pub struct SessionResponse {
    /// Current status of the session
    pub status: SessionStatus,
    /// Accumulated response content
    pub content: String,
    /// Error message if status is Failed
    pub error: Option<String>,
}

impl SessionResponse {
    /// Create a new running session response
    pub fn new_running() -> Self {
        Self {
            status: SessionStatus::Running,
            content: String::new(),
            error: None,
        }
    }

    /// Mark the session as completed with the given content
    pub fn complete_with_content(mut self, content: String) -> Self {
        self.status = SessionStatus::Completed;
        self.content = content;
        self
    }

    /// Mark the session as failed with the given error
    pub fn fail_with_error(mut self, error: String, partial_content: String) -> Self {
        self.status = SessionStatus::Failed;
        self.error = Some(error);
        self.content = partial_content;
        self
    }
}

/// Storage for session responses in compatibility mode
pub type SessionResponseStorage = HashMap<Uuid, SessionResponse>;

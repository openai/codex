//! Index progress streaming.
//!
//! Based on Continue's AsyncGenerator pattern and Tabby's async_stream usage.

use serde::Deserialize;
use serde::Serialize;

/// Index progress update.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexProgress {
    /// Progress value (0.0 - 1.0)
    pub progress: f32,
    /// Human-readable description
    pub description: String,
    /// Current status
    pub status: IndexStatus,
    /// Optional warnings
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
    /// Optional debug info
    #[serde(skip_serializing_if = "Option::is_none")]
    pub debug_info: Option<String>,
}

impl IndexProgress {
    /// Create a new progress update.
    pub fn new(progress: f32, description: impl Into<String>, status: IndexStatus) -> Self {
        Self {
            progress,
            description: description.into(),
            status,
            warnings: Vec::new(),
            debug_info: None,
        }
    }

    /// Create a loading progress.
    pub fn loading(description: impl Into<String>) -> Self {
        Self::new(0.0, description, IndexStatus::Loading)
    }

    /// Create an indexing progress.
    pub fn indexing(progress: f32, description: impl Into<String>) -> Self {
        Self::new(progress, description, IndexStatus::Indexing)
    }

    /// Create a done progress.
    pub fn done(description: impl Into<String>) -> Self {
        Self::new(1.0, description, IndexStatus::Done)
    }

    /// Create a failed progress.
    pub fn failed(description: impl Into<String>) -> Self {
        Self::new(0.0, description, IndexStatus::Failed)
    }

    /// Add a warning.
    pub fn with_warning(mut self, warning: impl Into<String>) -> Self {
        self.warnings.push(warning.into());
        self
    }

    /// Add debug info.
    pub fn with_debug(mut self, info: impl Into<String>) -> Self {
        self.debug_info = Some(info.into());
        self
    }
}

/// Index status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IndexStatus {
    /// Loading/initializing
    Loading,
    /// Actively indexing
    Indexing,
    /// Completed successfully
    Done,
    /// Failed with error
    Failed,
    /// Paused by user
    Paused,
    /// Cancelled by user
    Cancelled,
}

impl std::fmt::Display for IndexStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            IndexStatus::Loading => write!(f, "loading"),
            IndexStatus::Indexing => write!(f, "indexing"),
            IndexStatus::Done => write!(f, "done"),
            IndexStatus::Failed => write!(f, "failed"),
            IndexStatus::Paused => write!(f, "paused"),
            IndexStatus::Cancelled => write!(f, "cancelled"),
        }
    }
}

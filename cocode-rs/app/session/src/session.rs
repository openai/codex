//! Session metadata types.
//!
//! This module defines the [`Session`] struct which holds metadata about
//! an agent session including identity, timestamps, and configuration.

use chrono::{DateTime, Utc};
use cocode_protocol::ProviderType;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Session metadata for an agent conversation.
///
/// A session represents a single conversation with an LLM provider.
/// It tracks identity, timestamps, and configuration but does not
/// hold the actual conversation history (see [`SessionState`]).
///
/// # Example
///
/// ```
/// use cocode_session::Session;
/// use cocode_protocol::ProviderType;
/// use std::path::PathBuf;
///
/// let session = Session::new(
///     PathBuf::from("/project"),
///     "gpt-5",
///     ProviderType::Openai,
/// );
///
/// println!("Session ID: {}", session.id);
/// println!("Model: {}", session.model);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    /// Unique session identifier (UUID v4).
    pub id: String,

    /// Session creation timestamp.
    pub created_at: DateTime<Utc>,

    /// Last activity timestamp.
    pub last_activity_at: DateTime<Utc>,

    /// Working directory for the session.
    pub working_dir: PathBuf,

    /// Provider name (e.g., "openai", "anthropic").
    pub provider: String,

    /// Provider type enum.
    pub provider_type: ProviderType,

    /// Model identifier (e.g., "gpt-5", "claude-sonnet-4").
    pub model: String,

    /// Maximum turns before stopping (None = unlimited).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_turns: Option<i32>,

    /// Session title (optional, user-provided or auto-generated).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,

    /// Whether the session is ephemeral (not persisted).
    #[serde(default)]
    pub ephemeral: bool,
}

impl Session {
    /// Create a new session with the given configuration.
    ///
    /// Generates a new UUID and sets timestamps to now.
    pub fn new(working_dir: PathBuf, model: &str, provider_type: ProviderType) -> Self {
        let now = Utc::now();
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            created_at: now,
            last_activity_at: now,
            working_dir,
            provider: provider_type.to_string(),
            provider_type,
            model: model.to_string(),
            max_turns: Some(200),
            title: None,
            ephemeral: false,
        }
    }

    /// Create a new session with a specific ID.
    ///
    /// Useful for resuming sessions or testing.
    pub fn with_id(
        id: impl Into<String>,
        working_dir: PathBuf,
        model: &str,
        provider_type: ProviderType,
    ) -> Self {
        let now = Utc::now();
        Self {
            id: id.into(),
            created_at: now,
            last_activity_at: now,
            working_dir,
            provider: provider_type.to_string(),
            provider_type,
            model: model.to_string(),
            max_turns: Some(200),
            title: None,
            ephemeral: false,
        }
    }

    /// Create a builder for customizing session options.
    pub fn builder() -> SessionBuilder {
        SessionBuilder::new()
    }

    /// Update the last activity timestamp to now.
    pub fn touch(&mut self) {
        self.last_activity_at = Utc::now();
    }

    /// Set the session title.
    pub fn set_title(&mut self, title: impl Into<String>) {
        self.title = Some(title.into());
    }

    /// Set the max turns limit.
    pub fn set_max_turns(&mut self, max: Option<i32>) {
        self.max_turns = max;
    }

    /// Mark the session as ephemeral (not persisted).
    pub fn set_ephemeral(&mut self, ephemeral: bool) {
        self.ephemeral = ephemeral;
    }

    /// Get the session age in seconds.
    pub fn age_secs(&self) -> i64 {
        (Utc::now() - self.created_at).num_seconds()
    }

    /// Get seconds since last activity.
    pub fn idle_secs(&self) -> i64 {
        (Utc::now() - self.last_activity_at).num_seconds()
    }
}

/// Builder for creating [`Session`] instances.
#[derive(Debug, Default)]
pub struct SessionBuilder {
    working_dir: Option<PathBuf>,
    model: Option<String>,
    provider: Option<String>,
    provider_type: Option<ProviderType>,
    max_turns: Option<i32>,
    title: Option<String>,
    ephemeral: bool,
}

impl SessionBuilder {
    /// Create a new builder.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the working directory.
    pub fn working_dir(mut self, path: impl Into<PathBuf>) -> Self {
        self.working_dir = Some(path.into());
        self
    }

    /// Set the model.
    pub fn model(mut self, model: impl Into<String>) -> Self {
        self.model = Some(model.into());
        self
    }

    /// Set the provider name.
    pub fn provider(mut self, provider: impl Into<String>) -> Self {
        self.provider = Some(provider.into());
        self
    }

    /// Set the provider type.
    pub fn provider_type(mut self, provider_type: ProviderType) -> Self {
        self.provider_type = Some(provider_type);
        self
    }

    /// Set the max turns.
    pub fn max_turns(mut self, max: i32) -> Self {
        self.max_turns = Some(max);
        self
    }

    /// Set the title.
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// Set ephemeral mode.
    pub fn ephemeral(mut self, ephemeral: bool) -> Self {
        self.ephemeral = ephemeral;
        self
    }

    /// Build the session.
    ///
    /// # Panics
    ///
    /// Panics if working_dir, model, or provider_type are not set.
    pub fn build(self) -> Session {
        let working_dir = self.working_dir.expect("working_dir is required");
        let model = self.model.expect("model is required");
        let provider_type = self.provider_type.expect("provider_type is required");

        let mut session = Session::new(working_dir, &model, provider_type);

        if let Some(provider) = self.provider {
            session.provider = provider;
        }
        if let Some(max) = self.max_turns {
            session.max_turns = Some(max);
        }
        session.title = self.title;
        session.ephemeral = self.ephemeral;

        session
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_new() {
        let session = Session::new(PathBuf::from("/test"), "gpt-5", ProviderType::Openai);

        assert!(!session.id.is_empty());
        assert_eq!(session.model, "gpt-5");
        assert_eq!(session.provider, "openai");
        assert_eq!(session.provider_type, ProviderType::Openai);
        assert_eq!(session.working_dir, PathBuf::from("/test"));
        assert_eq!(session.max_turns, Some(200));
        assert!(session.title.is_none());
        assert!(!session.ephemeral);
    }

    #[test]
    fn test_session_with_id() {
        let session = Session::with_id(
            "test-id",
            PathBuf::from("/test"),
            "claude-sonnet-4",
            ProviderType::Anthropic,
        );

        assert_eq!(session.id, "test-id");
        assert_eq!(session.model, "claude-sonnet-4");
        assert_eq!(session.provider_type, ProviderType::Anthropic);
    }

    #[test]
    fn test_session_builder() {
        let session = Session::builder()
            .working_dir("/project")
            .model("gpt-5")
            .provider("my-openai")
            .provider_type(ProviderType::Openai)
            .max_turns(100)
            .title("Test Session")
            .ephemeral(true)
            .build();

        assert_eq!(session.model, "gpt-5");
        assert_eq!(session.provider, "my-openai");
        assert_eq!(session.max_turns, Some(100));
        assert_eq!(session.title, Some("Test Session".to_string()));
        assert!(session.ephemeral);
    }

    #[test]
    fn test_session_touch() {
        let mut session = Session::new(PathBuf::from("/test"), "gpt-5", ProviderType::Openai);

        let before = session.last_activity_at;
        std::thread::sleep(std::time::Duration::from_millis(10));
        session.touch();
        assert!(session.last_activity_at > before);
    }

    #[test]
    fn test_session_serde() {
        let session = Session::new(PathBuf::from("/test"), "gpt-5", ProviderType::Openai);

        let json = serde_json::to_string(&session).expect("serialize");
        let parsed: Session = serde_json::from_str(&json).expect("deserialize");

        assert_eq!(parsed.id, session.id);
        assert_eq!(parsed.model, session.model);
        assert_eq!(parsed.provider_type, session.provider_type);
    }
}

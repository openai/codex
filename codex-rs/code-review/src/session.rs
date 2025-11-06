//! Review session management

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tokio::fs;
use tracing::{debug, info};

use crate::analyzer::AnalysisResult;

/// Review session state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewSession {
    /// Session ID
    pub id: String,

    /// Session start time
    pub started_at: chrono::DateTime<chrono::Utc>,

    /// Last update time
    pub updated_at: chrono::DateTime<chrono::Utc>,

    /// Session state
    pub state: ReviewSessionState,

    /// Analysis results by file
    pub analysis_results: HashMap<PathBuf, AnalysisResult>,

    /// Improvements applied
    pub improvements_applied: Vec<ImprovementRecord>,

    /// Session statistics
    pub statistics: SessionStatistics,
}

impl ReviewSession {
    /// Create a new review session
    pub fn new(id: String) -> Self {
        let now = chrono::Utc::now();
        Self {
            id,
            started_at: now,
            updated_at: now,
            state: ReviewSessionState::Active,
            analysis_results: HashMap::new(),
            improvements_applied: Vec::new(),
            statistics: SessionStatistics::default(),
        }
    }

    /// Add analysis result
    pub fn add_analysis_result(&mut self, result: AnalysisResult) {
        self.updated_at = chrono::Utc::now();
        self.statistics.files_analyzed += 1;
        self.statistics.total_issues += result.issues.len();
        self.statistics.total_suggestions += result.suggestions.len();

        self.analysis_results
            .insert(result.file_path.clone(), result);
    }

    /// Record an improvement
    pub fn record_improvement(&mut self, improvement: ImprovementRecord) {
        self.updated_at = chrono::Utc::now();
        self.statistics.improvements_applied += 1;
        self.improvements_applied.push(improvement);
    }

    /// Pause the session
    pub fn pause(&mut self) {
        self.state = ReviewSessionState::Paused;
        self.updated_at = chrono::Utc::now();
    }

    /// Resume the session
    pub fn resume(&mut self) {
        self.state = ReviewSessionState::Active;
        self.updated_at = chrono::Utc::now();
    }

    /// Complete the session
    pub fn complete(&mut self) {
        self.state = ReviewSessionState::Completed;
        self.updated_at = chrono::Utc::now();
    }

    /// Get session duration
    pub fn duration(&self) -> chrono::Duration {
        self.updated_at - self.started_at
    }

    /// Save session to disk
    pub async fn save(&self, session_dir: &Path) -> Result<()> {
        let session_file = session_dir.join(format!("{}.json", self.id));
        let json = serde_json::to_string_pretty(self)?;
        fs::write(&session_file, json)
            .await
            .context("Failed to save session")?;
        debug!("Session saved to {:?}", session_file);
        Ok(())
    }

    /// Load session from disk
    pub async fn load(session_id: &str, session_dir: &Path) -> Result<Self> {
        let session_file = session_dir.join(format!("{}.json", session_id));
        let json = fs::read_to_string(&session_file)
            .await
            .context("Failed to read session file")?;
        let session = serde_json::from_str(&json)?;
        debug!("Session loaded from {:?}", session_file);
        Ok(session)
    }

    /// List all sessions
    pub async fn list_sessions(session_dir: &Path) -> Result<Vec<String>> {
        let mut sessions = Vec::new();
        let mut entries = fs::read_dir(session_dir).await?;

        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("json") {
                if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                    sessions.push(stem.to_string());
                }
            }
        }

        Ok(sessions)
    }
}

/// Session state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReviewSessionState {
    Active,
    Paused,
    Completed,
    Failed,
}

/// Session statistics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SessionStatistics {
    /// Number of files analyzed
    pub files_analyzed: usize,

    /// Total issues found
    pub total_issues: usize,

    /// Total suggestions made
    pub total_suggestions: usize,

    /// Improvements applied
    pub improvements_applied: usize,

    /// Issues resolved
    pub issues_resolved: usize,
}

/// Record of an improvement applied
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImprovementRecord {
    /// File path
    pub file_path: PathBuf,

    /// Improvement type
    pub improvement_type: String,

    /// Description
    pub description: String,

    /// Applied at
    pub applied_at: chrono::DateTime<chrono::Utc>,

    /// Confidence score
    pub confidence: f32,

    /// Success status
    pub success: bool,

    /// Error message (if failed)
    pub error: Option<String>,
}

impl ImprovementRecord {
    /// Create a new improvement record
    pub fn new(
        file_path: PathBuf,
        improvement_type: String,
        description: String,
        confidence: f32,
    ) -> Self {
        Self {
            file_path,
            improvement_type,
            description,
            applied_at: chrono::Utc::now(),
            confidence,
            success: true,
            error: None,
        }
    }

    /// Mark as failed
    pub fn failed(mut self, error: String) -> Self {
        self.success = false;
        self.error = Some(error);
        self
    }
}

/// Session manager
pub struct SessionManager {
    session_dir: PathBuf,
    current_session: Option<ReviewSession>,
}

impl SessionManager {
    /// Create a new session manager
    pub fn new(session_dir: PathBuf) -> Self {
        Self {
            session_dir,
            current_session: None,
        }
    }

    /// Start a new session
    pub async fn start_session(&mut self) -> Result<&ReviewSession> {
        let session_id = format!("review-{}", chrono::Utc::now().timestamp());
        let session = ReviewSession::new(session_id);

        // Ensure session directory exists
        fs::create_dir_all(&self.session_dir).await?;

        info!("Started new review session: {}", session.id);
        self.current_session = Some(session);
        Ok(self.current_session.as_ref().unwrap())
    }

    /// Get current session
    pub fn current_session(&self) -> Option<&ReviewSession> {
        self.current_session.as_ref()
    }

    /// Get mutable current session
    pub fn current_session_mut(&mut self) -> Option<&mut ReviewSession> {
        self.current_session.as_mut()
    }

    /// Save current session
    pub async fn save_current_session(&self) -> Result<()> {
        if let Some(session) = &self.current_session {
            session.save(&self.session_dir).await?;
        }
        Ok(())
    }

    /// Load a session
    pub async fn load_session(&mut self, session_id: &str) -> Result<()> {
        let session = ReviewSession::load(session_id, &self.session_dir).await?;
        info!("Loaded review session: {}", session.id);
        self.current_session = Some(session);
        Ok(())
    }

    /// List all sessions
    pub async fn list_sessions(&self) -> Result<Vec<String>> {
        ReviewSession::list_sessions(&self.session_dir).await
    }

    /// Complete current session
    pub async fn complete_session(&mut self) -> Result<()> {
        if let Some(session) = &mut self.current_session {
            session.complete();
            session.save(&self.session_dir).await?;
            info!("Completed session: {}", session.id);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_new_session() {
        let session = ReviewSession::new("test-123".to_string());
        assert_eq!(session.id, "test-123");
        assert_eq!(session.state, ReviewSessionState::Active);
        assert_eq!(session.statistics.files_analyzed, 0);
    }

    #[test]
    fn test_session_state_changes() {
        let mut session = ReviewSession::new("test".to_string());

        session.pause();
        assert_eq!(session.state, ReviewSessionState::Paused);

        session.resume();
        assert_eq!(session.state, ReviewSessionState::Active);

        session.complete();
        assert_eq!(session.state, ReviewSessionState::Completed);
    }

    #[tokio::test]
    async fn test_session_save_load() {
        let temp_dir = TempDir::new().unwrap();
        let session = ReviewSession::new("test-save".to_string());

        session.save(temp_dir.path()).await.unwrap();

        let loaded = ReviewSession::load("test-save", temp_dir.path())
            .await
            .unwrap();
        assert_eq!(loaded.id, "test-save");
    }

    #[tokio::test]
    async fn test_session_manager() {
        let temp_dir = TempDir::new().unwrap();
        let mut manager = SessionManager::new(temp_dir.path().to_path_buf());

        manager.start_session().await.unwrap();
        assert!(manager.current_session().is_some());

        manager.save_current_session().await.unwrap();
        manager.complete_session().await.unwrap();

        let sessions = manager.list_sessions().await.unwrap();
        assert!(!sessions.is_empty());
    }
}

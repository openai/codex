//! Plan file management utilities.
//!
//! Provides functions for managing plan files at `~/.cocode/plans/`.

use std::path::Path;
use std::path::PathBuf;

use snafu::ResultExt;

use crate::error::Result;
use crate::error::plan_mode_error;
use crate::plan_slug::get_unique_slug;

/// Default plan directory name within the cocode config directory.
const PLAN_DIR_NAME: &str = "plans";

/// Cocode config directory name.
const COCODE_DIR_NAME: &str = ".cocode";

/// Get the plan directory path (`~/.cocode/plans/`).
///
/// # Errors
///
/// Returns an error if the home directory cannot be determined.
pub fn get_plan_dir() -> Result<PathBuf> {
    let home = dirs::home_dir().ok_or_else(|| plan_mode_error::NoHomeDirSnafu.build())?;
    Ok(home.join(COCODE_DIR_NAME).join(PLAN_DIR_NAME))
}

/// Get the plan file path for a session.
///
/// # Arguments
///
/// * `session_id` - The session identifier for slug generation
/// * `agent_id` - Optional agent ID for subagent plans
///
/// # Returns
///
/// Path to the plan file. For subagents, the format is `{slug}-agent-{agent_id}.md`.
pub fn get_plan_file_path(session_id: &str, agent_id: Option<&str>) -> Result<PathBuf> {
    let plan_dir = get_plan_dir()?;
    let slug = get_unique_slug(session_id, None);

    let filename = match agent_id {
        Some(id) => format!("{slug}-agent-{id}.md"),
        None => format!("{slug}.md"),
    };

    Ok(plan_dir.join(filename))
}

/// Read the contents of a plan file.
///
/// # Arguments
///
/// * `session_id` - The session identifier
/// * `agent_id` - Optional agent ID for subagent plans
///
/// # Returns
///
/// `Some(content)` if the file exists and is readable, `None` if it doesn't exist.
pub fn read_plan_file(session_id: &str, agent_id: Option<&str>) -> Option<String> {
    let path = get_plan_file_path(session_id, agent_id).ok()?;
    std::fs::read_to_string(&path).ok()
}

/// Check if a path is a plan file (for permission exceptions).
///
/// # Arguments
///
/// * `path` - The path to check
/// * `plan_path` - The expected plan file path
///
/// # Returns
///
/// `true` if the paths match (allowing Write/Edit tool usage in plan mode).
pub fn is_plan_file(path: &Path, plan_path: &Path) -> bool {
    // Normalize paths for comparison
    path == plan_path
}

/// Ensure the plan directory exists.
///
/// # Errors
///
/// Returns an error if directory creation fails.
pub fn ensure_plan_dir() -> Result<PathBuf> {
    let plan_dir = get_plan_dir()?;
    if !plan_dir.exists() {
        std::fs::create_dir_all(&plan_dir).context(plan_mode_error::CreateDirSnafu {
            message: format!("failed to create {}", plan_dir.display()),
        })?;
    }
    Ok(plan_dir)
}

/// Manager for plan file operations.
///
/// Provides a higher-level API for plan file management with session context.
#[derive(Debug, Clone)]
pub struct PlanFileManager {
    session_id: String,
    agent_id: Option<String>,
}

impl PlanFileManager {
    /// Create a new plan file manager.
    pub fn new(session_id: impl Into<String>) -> Self {
        Self {
            session_id: session_id.into(),
            agent_id: None,
        }
    }

    /// Create a new plan file manager for a subagent.
    pub fn for_agent(session_id: impl Into<String>, agent_id: impl Into<String>) -> Self {
        Self {
            session_id: session_id.into(),
            agent_id: Some(agent_id.into()),
        }
    }

    /// Get the plan file path.
    pub fn path(&self) -> Result<PathBuf> {
        get_plan_file_path(&self.session_id, self.agent_id.as_deref())
    }

    /// Ensure the plan directory exists and return the plan file path.
    pub fn ensure_and_get_path(&self) -> Result<PathBuf> {
        ensure_plan_dir()?;
        self.path()
    }

    /// Read the plan file contents.
    pub fn read(&self) -> Option<String> {
        read_plan_file(&self.session_id, self.agent_id.as_deref())
    }

    /// Check if a path matches this manager's plan file.
    pub fn is_plan_file(&self, path: &Path) -> bool {
        self.path().map(|p| is_plan_file(path, &p)).unwrap_or(false)
    }

    /// Get the session ID.
    pub fn session_id(&self) -> &str {
        &self.session_id
    }

    /// Get the agent ID if this is a subagent manager.
    pub fn agent_id(&self) -> Option<&str> {
        self.agent_id.as_deref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan_slug::clear_slug_cache;

    #[test]
    fn test_get_plan_dir() {
        let dir = get_plan_dir().expect("should get plan dir");
        assert!(dir.ends_with("plans"));
        assert!(dir.to_string_lossy().contains(".cocode"));
    }

    #[test]
    fn test_get_plan_file_path_main_agent() {
        clear_slug_cache();
        let path = get_plan_file_path("test-session", None).expect("should get path");
        assert!(path.extension().unwrap_or_default() == "md");
        assert!(!path.to_string_lossy().contains("agent-"));
    }

    #[test]
    fn test_get_plan_file_path_subagent() {
        clear_slug_cache();
        let path = get_plan_file_path("test-session", Some("explore-1")).expect("should get path");
        assert!(path.extension().unwrap_or_default() == "md");
        assert!(path.to_string_lossy().contains("agent-explore-1"));
    }

    #[test]
    fn test_is_plan_file() {
        let plan_path = PathBuf::from("/home/user/.cocode/plans/test-plan.md");
        let other_path = PathBuf::from("/home/user/project/src/main.rs");

        assert!(is_plan_file(&plan_path, &plan_path));
        assert!(!is_plan_file(&other_path, &plan_path));
    }

    #[test]
    fn test_plan_file_manager() {
        clear_slug_cache();

        let manager = PlanFileManager::new("session-1");
        assert_eq!(manager.session_id(), "session-1");
        assert!(manager.agent_id().is_none());

        let path = manager.path().expect("should get path");
        assert!(path.extension().unwrap_or_default() == "md");
    }

    #[test]
    fn test_plan_file_manager_for_agent() {
        clear_slug_cache();

        let manager = PlanFileManager::for_agent("session-1", "explore");
        assert_eq!(manager.session_id(), "session-1");
        assert_eq!(manager.agent_id(), Some("explore"));

        let path = manager.path().expect("should get path");
        assert!(path.to_string_lossy().contains("agent-explore"));
    }
}

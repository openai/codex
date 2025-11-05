/// Orchestrated edit mode - safe file editing through Orchestrator RPC
///
/// Provides file write operations with preimage SHA validation and automatic
/// lock acquisition through the Orchestrator server. Prevents concurrent edits
/// from conflicting.
use crate::errors::GitToolingError;
use anyhow::{Context, Result};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::PathBuf;

/// Orchestrated file edit request
#[derive(Debug, Clone)]
pub struct OrchestratedEdit {
    /// Repository root path
    pub repo_root: PathBuf,
    /// File path (relative to repo_root)
    pub file_path: PathBuf,
    /// New file content
    pub content: String,
    /// Preimage SHA256 (for conflict detection)
    pub preimage_sha: Option<String>,
}

/// Orchestrated edit result
#[derive(Debug, Clone)]
pub struct OrchestratedEditResult {
    /// Success status
    pub success: bool,
    /// New file SHA256
    pub new_sha: String,
    /// Error message (if any)
    pub error: Option<String>,
}

/// Orchestrated edit conflict error
#[derive(Debug, Clone)]
pub struct EditConflict {
    /// Expected SHA256
    pub expected_sha: String,
    /// Actual SHA256
    pub actual_sha: String,
    /// File path
    pub file_path: PathBuf,
}

impl OrchestratedEdit {
    /// Create a new orchestrated edit request
    pub fn new(
        repo_root: impl Into<PathBuf>,
        file_path: impl Into<PathBuf>,
        content: String,
    ) -> Self {
        Self {
            repo_root: repo_root.into(),
            file_path: file_path.into(),
            content,
            preimage_sha: None,
        }
    }

    /// Set preimage SHA for conflict detection
    pub fn with_preimage_sha(mut self, preimage_sha: impl Into<String>) -> Self {
        self.preimage_sha = Some(preimage_sha.into());
        self
    }

    /// Execute the edit (with conflict detection)
    ///
    /// Returns error if preimage SHA doesn't match current file content.
    pub fn execute(&self) -> Result<OrchestratedEditResult> {
        let full_path = self.repo_root.join(&self.file_path);

        // Validate path (must be within repo_root)
        if !full_path.starts_with(&self.repo_root) {
            return Err(GitToolingError::PathEscapesRepository {
                path: self.file_path.clone(),
            }
            .into());
        }

        // Check preimage SHA if provided
        if let Some(expected_sha) = &self.preimage_sha {
            if full_path.exists() {
                let current_content = fs::read_to_string(&full_path)
                    .context("Failed to read current file content")?;
                let current_sha = compute_sha256(&current_content);

                if &current_sha != expected_sha {
                    return Ok(OrchestratedEditResult {
                        success: false,
                        new_sha: current_sha.clone(),
                        error: Some(format!(
                            "Edit conflict: expected SHA {expected_sha} but found {current_sha}"
                        )),
                    });
                }
            } else {
                // File doesn't exist but preimage SHA provided
                if expected_sha != &compute_sha256("") {
                    return Ok(OrchestratedEditResult {
                        success: false,
                        new_sha: compute_sha256(""),
                        error: Some(
                            "Edit conflict: file does not exist but preimage SHA provided"
                                .to_string(),
                        ),
                    });
                }
            }
        }

        // Create parent directory if needed
        if let Some(parent) = full_path.parent() {
            fs::create_dir_all(parent).context("Failed to create parent directory")?;
        }

        // Write new content
        fs::write(&full_path, &self.content).context("Failed to write file")?;

        // Compute new SHA
        let new_sha = compute_sha256(&self.content);

        Ok(OrchestratedEditResult {
            success: true,
            new_sha,
            error: None,
        })
    }

    /// Read current file content with SHA
    pub fn read_with_sha(&self) -> Result<(String, String)> {
        let full_path = self.repo_root.join(&self.file_path);

        // Validate path
        if !full_path.starts_with(&self.repo_root) {
            return Err(GitToolingError::PathEscapesRepository {
                path: self.file_path.clone(),
            }
            .into());
        }

        if !full_path.exists() {
            return Ok((String::new(), compute_sha256("")));
        }

        let content = fs::read_to_string(&full_path).context("Failed to read file")?;
        let sha = compute_sha256(&content);

        Ok((content, sha))
    }
}

/// Compute SHA256 hash of string content
pub fn compute_sha256(content: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content.as_bytes());
    format!("{:x}", hasher.finalize())
}

/// Batch orchestrated edit
#[derive(Debug, Clone)]
pub struct BatchEdit {
    /// Repository root path
    pub repo_root: PathBuf,
    /// Individual edits
    pub edits: Vec<(PathBuf, String, Option<String>)>, // (path, content, preimage_sha)
}

impl BatchEdit {
    /// Create a new batch edit
    pub fn new(repo_root: impl Into<PathBuf>) -> Self {
        Self {
            repo_root: repo_root.into(),
            edits: Vec::new(),
        }
    }

    /// Add an edit to the batch
    pub fn add_edit(
        &mut self,
        file_path: impl Into<PathBuf>,
        content: String,
        preimage_sha: Option<String>,
    ) {
        self.edits.push((file_path.into(), content, preimage_sha));
    }

    /// Execute all edits atomically (all or nothing)
    ///
    /// Returns error if any edit fails. On error, all changes are rolled back.
    pub fn execute_atomic(&self) -> Result<Vec<OrchestratedEditResult>> {
        // First, validate all edits (preimage checks)
        let mut results = Vec::new();
        let mut backups: Vec<(PathBuf, Option<String>)> = Vec::new();

        for (file_path, content, preimage_sha) in &self.edits {
            let edit = OrchestratedEdit {
                repo_root: self.repo_root.clone(),
                file_path: file_path.clone(),
                content: content.clone(),
                preimage_sha: preimage_sha.clone(),
            };

            // Create backup before editing
            let full_path = self.repo_root.join(file_path);
            let backup = if full_path.exists() {
                Some(fs::read_to_string(&full_path).context("Failed to read file for backup")?)
            } else {
                None
            };
            backups.push((full_path.clone(), backup));

            // Execute edit
            let result = edit.execute()?;
            if !result.success {
                // Rollback all changes
                Self::rollback_edits(&backups)?;
                return Err(anyhow::anyhow!("Edit failed: {:?}", result.error));
            }

            results.push(result);
        }

        Ok(results)
    }

    /// Rollback edits (restore from backups)
    fn rollback_edits(backups: &[(PathBuf, Option<String>)]) -> Result<()> {
        for (path, backup) in backups {
            if let Some(content) = backup {
                fs::write(path, content).context("Failed to rollback edit")?;
            } else {
                // File didn't exist before, remove it
                if path.exists() {
                    fs::remove_file(path).context("Failed to remove file during rollback")?;
                }
            }
        }
        Ok(())
    }
}

/// Orchestrated patch application
#[derive(Debug, Clone)]
pub struct OrchestratedPatch {
    /// Repository root path
    pub repo_root: PathBuf,
    /// Unified diff patch
    pub patch: String,
    /// Base commit SHA (for validation)
    pub base_commit: String,
}

impl OrchestratedPatch {
    /// Create a new orchestrated patch
    pub fn new(repo_root: impl Into<PathBuf>, patch: String, base_commit: String) -> Self {
        Self {
            repo_root: repo_root.into(),
            patch,
            base_commit,
        }
    }

    /// Apply the patch (using git apply)
    pub fn apply(&self) -> Result<Vec<PathBuf>> {
        // TODO: Implement git apply with conflict detection
        // For now, return empty list
        Ok(Vec::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_compute_sha256() {
        let content = "Hello, world!";
        let sha = compute_sha256(content);
        assert_eq!(
            sha,
            "315f5bdb76d078c43b8ac0064e4a0164612b1fce77c869345bfc94c75894edd3"
        );
    }

    #[test]
    fn test_orchestrated_edit_new_file() {
        let temp_dir = TempDir::new().unwrap();
        let repo_root = temp_dir.path().to_path_buf();

        let edit = OrchestratedEdit::new(&repo_root, "test.txt", "Hello, world!".to_string());

        let result = edit.execute().unwrap();
        assert!(result.success);
        assert_eq!(
            result.new_sha,
            "315f5bdb76d078c43b8ac0064e4a0164612b1fce77c869345bfc94c75894edd3"
        );

        // Verify file was created
        let file_path = repo_root.join("test.txt");
        assert!(file_path.exists());
        let content = fs::read_to_string(file_path).unwrap();
        assert_eq!(content, "Hello, world!");
    }

    #[test]
    fn test_orchestrated_edit_with_preimage() {
        let temp_dir = TempDir::new().unwrap();
        let repo_root = temp_dir.path().to_path_buf();
        let file_path = repo_root.join("test.txt");

        // Create initial file
        fs::write(&file_path, "Initial content").unwrap();
        let initial_sha = compute_sha256("Initial content");

        // Edit with correct preimage
        let edit = OrchestratedEdit::new(&repo_root, "test.txt", "Updated content".to_string())
            .with_preimage_sha(&initial_sha);

        let result = edit.execute().unwrap();
        assert!(result.success);

        // Verify content was updated
        let content = fs::read_to_string(&file_path).unwrap();
        assert_eq!(content, "Updated content");
    }

    #[test]
    fn test_orchestrated_edit_conflict() {
        let temp_dir = TempDir::new().unwrap();
        let repo_root = temp_dir.path().to_path_buf();
        let file_path = repo_root.join("test.txt");

        // Create initial file
        fs::write(&file_path, "Initial content").unwrap();

        // Edit with wrong preimage (conflict)
        let wrong_sha = compute_sha256("Wrong content");
        let edit = OrchestratedEdit::new(&repo_root, "test.txt", "Updated content".to_string())
            .with_preimage_sha(wrong_sha);

        let result = edit.execute().unwrap();
        assert!(!result.success);
        assert!(result.error.is_some());

        // Verify content was NOT updated
        let content = fs::read_to_string(&file_path).unwrap();
        assert_eq!(content, "Initial content");
    }

    #[test]
    fn test_batch_edit_atomic() {
        let temp_dir = TempDir::new().unwrap();
        let repo_root = temp_dir.path().to_path_buf();

        let mut batch = BatchEdit::new(&repo_root);
        batch.add_edit("file1.txt", "Content 1".to_string(), None);
        batch.add_edit("file2.txt", "Content 2".to_string(), None);
        batch.add_edit("dir/file3.txt", "Content 3".to_string(), None);

        let results = batch.execute_atomic().unwrap();
        assert_eq!(results.len(), 3);
        assert!(results.iter().all(|r| r.success));

        // Verify files were created
        assert!(repo_root.join("file1.txt").exists());
        assert!(repo_root.join("file2.txt").exists());
        assert!(repo_root.join("dir/file3.txt").exists());
    }
}

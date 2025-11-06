//! Linter integrations for code review

pub mod clippy;
pub mod eslint;
pub mod pylint;
pub mod generic;

use anyhow::Result;
use async_trait::async_trait;
use std::path::Path;

use crate::analyzer::Issue;

/// Linter trait
#[async_trait]
pub trait Linter: Send + Sync {
    /// Get linter name
    fn name(&self) -> &str;

    /// Check if linter is available
    async fn is_available(&self) -> bool;

    /// Run linter on a file
    async fn lint_file(&self, file_path: &Path) -> Result<Vec<Issue>>;

    /// Get supported file extensions
    fn supported_extensions(&self) -> Vec<&str>;
}

/// Linter registry
pub struct LinterRegistry {
    linters: Vec<Box<dyn Linter>>,
}

impl LinterRegistry {
    /// Create a new linter registry
    pub fn new() -> Self {
        Self {
            linters: Vec::new(),
        }
    }

    /// Register a linter
    pub fn register(&mut self, linter: Box<dyn Linter>) {
        self.linters.push(linter);
    }

    /// Get linters for a file
    pub fn get_linters_for_file(&self, file_path: &Path) -> Vec<&dyn Linter> {
        let extension = file_path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("");

        self.linters
            .iter()
            .filter(|l| l.supported_extensions().contains(&extension))
            .map(|l| l.as_ref())
            .collect()
    }

    /// Run all applicable linters on a file
    pub async fn lint_file(&self, file_path: &Path) -> Result<Vec<Issue>> {
        let mut all_issues = Vec::new();

        for linter in self.get_linters_for_file(file_path) {
            if linter.is_available().await {
                match linter.lint_file(file_path).await {
                    Ok(issues) => {
                        tracing::debug!("{} found {} issues", linter.name(), issues.len());
                        all_issues.extend(issues);
                    }
                    Err(e) => {
                        tracing::warn!("Linter {} failed: {}", linter.name(), e);
                    }
                }
            }
        }

        Ok(all_issues)
    }
}

impl Default for LinterRegistry {
    fn default() -> Self {
        let mut registry = Self::new();

        // Register built-in linters
        registry.register(Box::new(clippy::ClippyLinter::new()));
        registry.register(Box::new(eslint::ESLintLinter::new()));
        registry.register(Box::new(pylint::PylintLinter::new()));

        registry
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_creation() {
        let registry = LinterRegistry::default();
        assert!(!registry.linters.is_empty());
    }

    #[test]
    fn test_get_linters_for_file() {
        let registry = LinterRegistry::default();
        let linters = registry.get_linters_for_file(Path::new("test.rs"));
        assert!(!linters.is_empty());
    }
}

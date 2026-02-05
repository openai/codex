use std::path::{Path, PathBuf};
use tokio::fs;
use wildmatch::WildMatch;

use crate::config::Config;

const DEFAULT_SENSITIVE_PATTERNS: &[&str] = &[
    ".env",
    ".env.*",
    "*.pem",
    "id_rsa",
    "id_ed25519",
    "id_ecdsa",
    "id_dsa",
    ".aws/",
    ".ssh/",
    "**/.ssh/**",
    "**/.aws/**",
];

#[derive(Debug, Clone, Default)]
pub struct FileIgnore {
    /// Full paths to the ignore files loaded (e.g. .codexignore).
    /// Used to pass to tools like `rg` via `--ignore-file`.
    ignore_files: Vec<PathBuf>,
    /// Combined list of deny patterns (defaults + loaded from files).
    /// Used for internal checks (read_file, list_dir).
    patterns: Vec<String>,
}

impl FileIgnore {
    pub fn new() -> Self {
        Self {
            ignore_files: Vec::new(),
            patterns: DEFAULT_SENSITIVE_PATTERNS
                .iter()
                .map(|s| s.to_string())
                .collect(),
        }
    }

    pub async fn load(&mut self, config: &Config) {
        // Global ignore file
        let global_ignore = config.codex_home.join(".codexignore");
        if fs::try_exists(&global_ignore).await.unwrap_or(false) {
            self.add_ignore_file(global_ignore).await;
        }

        // Repo/Project local ignore file
        let local_ignore = config.cwd.join(".codexignore");
        if fs::try_exists(&local_ignore).await.unwrap_or(false) {
            self.add_ignore_file(local_ignore).await;
        }
    }

    async fn add_ignore_file(&mut self, path: PathBuf) {
        if let Ok(content) = fs::read_to_string(&path).await {
            for line in content.lines() {
                let trimmed = line.trim();
                if !trimmed.is_empty() && !trimmed.starts_with('#') {
                    self.patterns.push(trimmed.to_string());
                }
            }
            self.ignore_files.push(path);
        }
    }

    pub fn is_denied(&self, path: &Path) -> bool {
        // We use to_string_lossy which replaces non-UTF8 characters, ensuring we always get a string.
        let path_str = path.to_string_lossy();

        for pattern in &self.patterns {
            if self.matches(pattern, path, &path_str) {
                return true;
            }
        }
        false
    }

    fn matches(&self, pattern: &str, path: &Path, path_str: &str) -> bool {
        // Handle directory patterns ending in /
        if let Some(dir_pattern) = pattern.strip_suffix('/') {
            // Check if any component matches the directory name
            for component in path.components() {
                if let Some(comp_str) = component.as_os_str().to_str() {
                    if WildMatch::new(dir_pattern).matches(comp_str) {
                        return true;
                    }
                }
            }
            return false;
        }

        // Handle glob patterns.
        
        // 1. Try matching the full path string
        if WildMatch::new(pattern).matches(path_str) {
            return true;
        }

        // 2. If pattern has no slash, match against filename
        if !pattern.contains('/') {
             if let Some(file_name) = path.file_name().and_then(|s| s.to_str()) {
                if WildMatch::new(pattern).matches(file_name) {
                    return true;
                }
            }
        }

        false
    }

    pub fn ignore_files(&self) -> &[PathBuf] {
        &self.ignore_files
    }

    /// Returns the default sensitive patterns that are NOT in a file.
    /// These should be passed to tools like `rg` as `-g '!pattern'`.
    pub fn default_patterns(&self) -> Vec<String> {
        DEFAULT_SENSITIVE_PATTERNS
            .iter()
            .map(|s| s.to_string())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_denied_defaults() {
        let ignore = FileIgnore::new();
        assert!(ignore.is_denied(Path::new(".env")));
        assert!(ignore.is_denied(Path::new("src/.env")));
        assert!(ignore.is_denied(Path::new("foo/.ssh/id_rsa")));
        assert!(ignore.is_denied(Path::new("prod.pem")));
        
        assert!(!ignore.is_denied(Path::new("src/main.rs")));
        assert!(!ignore.is_denied(Path::new("README.md")));
    }

    #[test]
    fn test_is_denied_directory() {
        let mut ignore = FileIgnore::new();
        ignore.patterns.push("secret/".to_string());

        assert!(ignore.is_denied(Path::new("secret/file.txt")));
        assert!(ignore.is_denied(Path::new("src/secret/key")));
        
        // Should not match partial name if not component
        assert!(!ignore.is_denied(Path::new("mysecret/file.txt"))); 
    }

    #[test]
    fn test_is_denied_glob() {
        let mut ignore = FileIgnore::new();
        ignore.patterns.push("*.secret".to_string());

        assert!(ignore.is_denied(Path::new("config.secret")));
        assert!(ignore.is_denied(Path::new("src/config.secret")));
        
        assert!(!ignore.is_denied(Path::new("config.public")));
    }
}

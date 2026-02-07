//! Sensitive file detection for permission checks.
//!
//! Identifies files that require elevated permission due to containing
//! credentials, secrets, or critical configuration.

use std::path::Path;

/// Sensitive file path patterns (matching Claude Code v2.1.7).
const SENSITIVE_FILE_PATTERNS: &[&str] = &[
    // Credentials and keys
    ".env",
    "*.pem",
    "*.key",
    "credentials.json",
    // Shell configuration
    ".bashrc",
    ".zshrc",
    ".bash_profile",
    ".zprofile",
    ".profile",
    // Git configuration
    ".gitconfig",
    ".git-credentials",
    ".gitmodules",
    // SSH
    ".ssh/config",
    ".ssh/authorized_keys",
    // Tool configuration
    ".mcp.json",
    ".claude/settings.json",
    ".npmrc",
    ".pypirc",
    ".ripgreprc",
    // CI/CD
    ".github/workflows/*.yml",
];

/// Locked directories that should not be written to.
const LOCKED_DIRECTORIES: &[&str] = &[
    ".claude/",
    ".claude/commands/",
    ".claude/agents/",
    ".claude/skills/",
];

/// Sensitive directories that require approval for writes.
const SENSITIVE_DIRECTORIES: &[&str] = &[".git/", ".vscode/", ".idea/"];

/// Check if a file path matches any sensitive file pattern.
pub fn is_sensitive_file(path: &Path) -> bool {
    let path_str = path.to_string_lossy();
    let filename = path
        .file_name()
        .map(|f| f.to_string_lossy())
        .unwrap_or_default();

    for pattern in SENSITIVE_FILE_PATTERNS {
        if matches_pattern(pattern, &path_str, &filename) {
            return true;
        }
    }

    // Also check .env.* variants
    if filename.starts_with(".env.") {
        return true;
    }

    // Check service-account*.json
    if filename.starts_with("service-account") && filename.ends_with(".json") {
        return true;
    }

    // Check .ssh/id_*
    if path_str.contains(".ssh/id_") {
        return true;
    }

    false
}

/// Check if a path is within a locked directory.
pub fn is_locked_directory(path: &Path) -> bool {
    let path_str = path.to_string_lossy();
    for dir in LOCKED_DIRECTORIES {
        if path_str.contains(dir) {
            return true;
        }
    }
    false
}

/// Check if a path is within a sensitive directory (requires approval for writes).
pub fn is_sensitive_directory(path: &Path) -> bool {
    let path_str = path.to_string_lossy();
    for dir in SENSITIVE_DIRECTORIES {
        if path_str.contains(dir) {
            return true;
        }
    }
    false
}

/// Check if a path is outside the given working directory.
pub fn is_outside_cwd(path: &Path, cwd: &Path) -> bool {
    // Canonicalize if possible; fall back to starts_with
    let abs_path = if path.is_absolute() {
        path.to_path_buf()
    } else {
        cwd.join(path)
    };
    let abs_cwd = cwd.to_path_buf();

    !abs_path.starts_with(&abs_cwd)
}

/// Simple pattern matching for sensitive file detection.
fn matches_pattern(pattern: &str, full_path: &str, filename: &str) -> bool {
    if pattern.contains('/') {
        // Path-based pattern - check if path contains the pattern segment
        if pattern.contains('*') {
            // e.g. ".github/workflows/*.yml"
            let parts: Vec<&str> = pattern.split('*').collect();
            if parts.len() == 2 {
                return full_path.contains(parts[0]) && full_path.ends_with(parts[1]);
            }
        }
        // e.g. ".ssh/config" or ".ssh/authorized_keys"
        return full_path.ends_with(pattern) || full_path.contains(&format!("/{pattern}"));
    }

    if pattern.starts_with('*') {
        // Extension pattern: "*.pem", "*.key"
        return filename.ends_with(&pattern[1..]);
    }

    // Exact filename match
    filename == pattern
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_env_files() {
        assert!(is_sensitive_file(Path::new(".env")));
        assert!(is_sensitive_file(Path::new("/home/user/.env")));
        assert!(is_sensitive_file(Path::new(".env.local")));
        assert!(is_sensitive_file(Path::new(".env.production")));
        assert!(!is_sensitive_file(Path::new("src/main.rs")));
    }

    #[test]
    fn test_key_files() {
        assert!(is_sensitive_file(Path::new("server.pem")));
        assert!(is_sensitive_file(Path::new("private.key")));
        assert!(is_sensitive_file(Path::new("credentials.json")));
    }

    #[test]
    fn test_shell_configs() {
        assert!(is_sensitive_file(Path::new("/home/user/.bashrc")));
        assert!(is_sensitive_file(Path::new(".zshrc")));
        assert!(is_sensitive_file(Path::new(".profile")));
    }

    #[test]
    fn test_ssh_files() {
        assert!(is_sensitive_file(Path::new("/home/user/.ssh/config")));
        assert!(is_sensitive_file(Path::new(".ssh/id_rsa")));
        assert!(is_sensitive_file(Path::new(".ssh/id_ed25519")));
        assert!(is_sensitive_file(Path::new(".ssh/authorized_keys")));
    }

    #[test]
    fn test_cicd() {
        assert!(is_sensitive_file(Path::new(".github/workflows/deploy.yml")));
        assert!(!is_sensitive_file(Path::new(".github/CODEOWNERS")));
    }

    #[test]
    fn test_locked_directories() {
        assert!(is_locked_directory(Path::new(".claude/settings.json")));
        assert!(is_locked_directory(Path::new(".claude/commands/my-cmd")));
        assert!(is_locked_directory(Path::new(".claude/agents/my-agent")));
        assert!(is_locked_directory(Path::new(".claude/skills/my-skill")));
        assert!(!is_locked_directory(Path::new("src/main.rs")));
    }

    #[test]
    fn test_sensitive_directories() {
        assert!(is_sensitive_directory(Path::new(".git/config")));
        assert!(is_sensitive_directory(Path::new(".vscode/settings.json")));
        assert!(is_sensitive_directory(Path::new(".idea/workspace.xml")));
        assert!(!is_sensitive_directory(Path::new("src/main.rs")));
    }

    #[test]
    fn test_is_outside_cwd() {
        let cwd = Path::new("/home/user/project");
        assert!(!is_outside_cwd(
            Path::new("/home/user/project/src/main.rs"),
            cwd
        ));
        assert!(is_outside_cwd(Path::new("/etc/passwd"), cwd));
        assert!(is_outside_cwd(Path::new("/home/user/other/file.txt"), cwd));
    }

    #[test]
    fn test_new_sensitive_patterns() {
        assert!(is_sensitive_file(Path::new(".gitmodules")));
        assert!(is_sensitive_file(Path::new(".ripgreprc")));
        assert!(is_sensitive_file(Path::new(".zprofile")));
    }

    #[test]
    fn test_service_account() {
        assert!(is_sensitive_file(Path::new("service-account.json")));
        assert!(is_sensitive_file(Path::new("service-account-prod.json")));
        assert!(!is_sensitive_file(Path::new("service-info.json")));
    }

    #[test]
    fn test_normal_files_not_sensitive() {
        assert!(!is_sensitive_file(Path::new("src/main.rs")));
        assert!(!is_sensitive_file(Path::new("Cargo.toml")));
        assert!(!is_sensitive_file(Path::new("README.md")));
        assert!(!is_sensitive_file(Path::new("package.json")));
    }
}

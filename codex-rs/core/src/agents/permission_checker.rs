/// Permission Checker for Agent Tool Execution
///
/// Validates tool calls against agent-specific permissions before execution.
use anyhow::Result;
/// Permission Checker for Agent Tool Execution
///
/// Validates tool calls against agent-specific permissions before execution.
use anyhow::anyhow;
use once_cell::sync::Lazy;
use regex::Regex;
use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Mutex;

use super::types::FsWritePermission;
use super::types::ShellPermissions;
use super::types::ToolPermissions;

/// Permission checker for agent tool execution
pub struct PermissionChecker {
    /// Tool permissions for this agent
    permissions: ToolPermissions,
}

impl PermissionChecker {
    /// Create a new permission checker
    pub fn new(permissions: ToolPermissions) -> Self {
        Self { permissions }
    }

    /// Check if an MCP tool is allowed
    pub fn check_mcp_tool(&self, tool_name: &str) -> Result<()> {
        if self.permissions.mcp.contains(&tool_name.to_string()) {
            Ok(())
        } else if self.permissions.mcp.contains(&"*".to_string()) {
            // Wildcard allows all tools
            Ok(())
        } else {
            Err(anyhow!(
                "MCP tool '{}' is not permitted. Allowed tools: {:?}",
                tool_name,
                self.permissions.mcp
            ))
        }
    }

    /// Check if file read is allowed
    pub fn check_file_read(&self, _path: &Path) -> Result<()> {
        if self.permissions.fs.read {
            Ok(())
        } else {
            Err(anyhow!("File read permission denied"))
        }
    }

    /// Check if file write is allowed
    pub fn check_file_write(&self, path: &Path) -> Result<()> {
        match &self.permissions.fs.write {
            FsWritePermission::Flag(true) => Ok(()),
            FsWritePermission::Flag(false) => Err(anyhow!("File write permission denied")),
            FsWritePermission::Paths(allowed_paths) => {
                let path_str = path.to_string_lossy();

                for allowed in allowed_paths {
                    // Check exact match
                    if path_str == allowed.as_str() {
                        return Ok(());
                    }

                    // Check if path starts with allowed directory
                    if path_str.starts_with(allowed.as_str()) {
                        return Ok(());
                    }

                    // Check if allowed path is a parent directory
                    let allowed_path = PathBuf::from(allowed);
                    if path.starts_with(&allowed_path) {
                        return Ok(());
                    }
                }

                Err(anyhow!(
                    "File write to '{path_str}' is not permitted. Allowed paths: {allowed_paths:?}"
                ))
            }
        }
    }

    /// Check if network access is allowed
    pub fn check_network_access(&self, url: &str) -> Result<()> {
        if self.permissions.net.allow.is_empty() {
            return Err(anyhow!(
                "Network access denied - no allowed domains configured"
            ));
        }

        // Check if "*" wildcard is present
        if self.permissions.net.allow.contains(&"*".to_string()) {
            return Ok(());
        }

        for pattern in &self.permissions.net.allow {
            if self.url_matches_pattern(url, pattern) {
                return Ok(());
            }
        }

        Err(anyhow!(
            "Network access to '{}' is not permitted. Allowed patterns: {:?}",
            url,
            self.permissions.net.allow
        ))
    }

    /// Check if shell command is allowed
    pub fn check_shell_command(&self, command: &str) -> Result<()> {
        match &self.permissions.shell {
            ShellPermissions::Empty(_) => Err(anyhow!(
                "Shell command execution denied - no commands allowed"
            )),
            ShellPermissions::Commands { exec } => {
                if exec.contains(&"*".to_string()) {
                    return Ok(());
                }

                // Extract the first word (command name)
                let command_name = command.split_whitespace().next().unwrap_or("");

                for allowed_cmd in exec {
                    if command_name == allowed_cmd {
                        return Ok(());
                    }
                }

                Err(anyhow!(
                    "Shell command '{command_name}' is not permitted. Allowed commands: {exec:?}"
                ))
            }
        }
    }

    /// Check if URL matches a pattern (supports wildcards)
    fn url_matches_pattern(&self, url: &str, pattern: &str) -> bool {
        // Convert glob pattern to regex
        let regex_pattern = pattern
            .replace('.', "\\.") // Escape dots
            .replace('*', ".*"); // Convert * to .*

        static CACHE: Lazy<Mutex<HashMap<String, Regex>>> =
            Lazy::new(|| Mutex::new(HashMap::new()));

        let mut cache = CACHE.lock().unwrap();
        let regex = cache.entry(regex_pattern.clone()).or_insert_with(|| {
            Regex::new(&format!("^{regex_pattern}$")).unwrap_or_else(|_| {
                // Fallback to exact match if regex fails
                Regex::new(&regex::escape(pattern)).unwrap()
            })
        });

        regex.is_match(url)
    }

    /// Check if a tool call is allowed (comprehensive check)
    pub fn check_tool_call(&self, tool_name: &str, parameters: &serde_json::Value) -> Result<()> {
        // Check MCP tool permission
        self.check_mcp_tool(tool_name)?;

        // Additional parameter-based checks
        match tool_name {
            "read_file" | "list_dir" | "glob_file_search" => {
                if let Some(path) = parameters.get("path").and_then(|v| v.as_str()) {
                    self.check_file_read(Path::new(path))?;
                }
                if let Some(path) = parameters.get("target_file").and_then(|v| v.as_str()) {
                    self.check_file_read(Path::new(path))?;
                }
            }
            "write" | "search_replace" | "delete_file" => {
                if let Some(path) = parameters.get("file_path").and_then(|v| v.as_str()) {
                    self.check_file_write(Path::new(path))?;
                }
                if let Some(path) = parameters.get("target_file").and_then(|v| v.as_str()) {
                    self.check_file_write(Path::new(path))?;
                }
            }
            "web_search" | "fetch" | "http_request" => {
                if let Some(url) = parameters.get("url").and_then(|v| v.as_str()) {
                    self.check_network_access(url)?;
                }
                if let Some(_url) = parameters.get("search_term").and_then(|v| v.as_str()) {
                    // For web_search, check if https://* is allowed
                    self.check_network_access("https://search.brave.com")?;
                }
            }
            "run_terminal_cmd" | "shell" | "exec" => {
                if let Some(cmd) = parameters.get("command").and_then(|v| v.as_str()) {
                    self.check_shell_command(cmd)?;
                }
            }
            _ => {
                // Unknown tool - rely on MCP permission check above
            }
        }

        Ok(())
    }

    /// Get permission summary for logging
    pub fn get_permission_summary(&self) -> String {
        format!(
            "MCP: {:?}, FS(read:{}, write:{:?}), Net: {:?}, Shell: {:?}",
            self.permissions.mcp,
            self.permissions.fs.read,
            self.permissions.fs.write,
            self.permissions.net.allow,
            self.permissions.shell
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    fn create_test_permissions() -> ToolPermissions {
        ToolPermissions {
            mcp: vec!["search".to_string(), "read_file".to_string()],
            fs: FsPermissions {
                read: true,
                write: FsWritePermission::Paths(vec![
                    "./artifacts".to_string(),
                    "./output".to_string(),
                ]),
            },
            net: NetPermissions {
                allow: vec![
                    "https://api.example.com/*".to_string(),
                    "https://github.com/*".to_string(),
                ],
            },
            shell: ShellPermissions::Commands {
                exec: vec!["npm".to_string(), "cargo".to_string()],
            },
        }
    }

    #[test]
    fn test_mcp_tool_allowed() {
        let checker = PermissionChecker::new(create_test_permissions());

        assert!(checker.check_mcp_tool("search").is_ok());
        assert!(checker.check_mcp_tool("read_file").is_ok());
        assert!(checker.check_mcp_tool("unauthorized_tool").is_err());
    }

    #[test]
    fn test_file_read_permission() {
        let checker = PermissionChecker::new(create_test_permissions());

        assert!(checker.check_file_read(Path::new("./any_file.txt")).is_ok());
    }

    #[test]
    fn test_file_write_permission() {
        let checker = PermissionChecker::new(create_test_permissions());

        assert!(
            checker
                .check_file_write(Path::new("./artifacts/output.md"))
                .is_ok()
        );
        assert!(
            checker
                .check_file_write(Path::new("./output/result.json"))
                .is_ok()
        );
        assert!(
            checker
                .check_file_write(Path::new("./unauthorized/file.txt"))
                .is_err()
        );
    }

    #[test]
    fn test_network_access_permission() {
        let checker = PermissionChecker::new(create_test_permissions());

        assert!(
            checker
                .check_network_access("https://api.example.com/v1/search")
                .is_ok()
        );
        assert!(
            checker
                .check_network_access("https://github.com/user/repo")
                .is_ok()
        );
        assert!(
            checker
                .check_network_access("https://unauthorized.com")
                .is_err()
        );
    }

    #[test]
    fn test_shell_command_permission() {
        let checker = PermissionChecker::new(create_test_permissions());

        assert!(checker.check_shell_command("npm install").is_ok());
        assert!(checker.check_shell_command("cargo build --release").is_ok());
        assert!(checker.check_shell_command("rm -rf /").is_err());
    }

    #[test]
    fn test_wildcard_mcp_tools() {
        let mut perms = create_test_permissions();
        perms.mcp = vec!["*".to_string()];
        let checker = PermissionChecker::new(perms);

        assert!(checker.check_mcp_tool("any_tool").is_ok());
    }

    #[test]
    fn test_wildcard_network() {
        let mut perms = create_test_permissions();
        perms.net.allow = vec!["*".to_string()];
        let checker = PermissionChecker::new(perms);

        assert!(
            checker
                .check_network_access("https://any-domain.com")
                .is_ok()
        );
    }

    #[test]
    fn test_wildcard_shell() {
        let mut perms = create_test_permissions();
        perms.shell = ShellPermissions::Commands {
            exec: vec!["*".to_string()],
        };
        let checker = PermissionChecker::new(perms);

        assert!(checker.check_shell_command("any-command").is_ok());
    }
}

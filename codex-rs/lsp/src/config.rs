//! LSP configuration types

use serde::Deserialize;
use serde::Serialize;
use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;
use tracing::debug;
use tracing::warn;

// ============================================================================
// Default value functions for serde
// ============================================================================

fn default_max_restarts() -> i32 {
    3
}

fn default_restart_on_crash() -> bool {
    true
}

fn default_startup_timeout_ms() -> i64 {
    10_000
}

fn default_shutdown_timeout_ms() -> i64 {
    5_000
}

fn default_request_timeout_ms() -> i64 {
    30_000
}

fn default_health_check_interval_ms() -> i64 {
    30_000
}

fn default_notification_buffer_size() -> i32 {
    100
}

// ============================================================================
// Built-in server definitions
// ============================================================================

/// Built-in server definition (not user-configurable)
#[derive(Debug, Clone)]
pub struct BuiltinServer {
    pub id: &'static str,
    pub extensions: &'static [&'static str],
    pub commands: &'static [&'static str],
    pub install_hint: &'static str,
    pub languages: &'static [&'static str],
}

/// Built-in servers (Rust, Go, Python, TypeScript/JavaScript)
pub const BUILTIN_SERVERS: &[BuiltinServer] = &[
    BuiltinServer {
        id: "rust-analyzer",
        extensions: &[".rs"],
        commands: &["rust-analyzer"],
        install_hint: "rustup component add rust-analyzer",
        languages: &["rust"],
    },
    BuiltinServer {
        id: "gopls",
        extensions: &[".go"],
        commands: &["gopls"],
        install_hint: "go install golang.org/x/tools/gopls@latest",
        languages: &["go"],
    },
    BuiltinServer {
        id: "pyright",
        extensions: &[".py", ".pyi"],
        commands: &["pyright-langserver", "--stdio"],
        install_hint: "npm install -g pyright",
        languages: &["python"],
    },
    BuiltinServer {
        id: "typescript-language-server",
        extensions: &[".ts", ".tsx", ".js", ".jsx", ".mjs", ".cjs"],
        commands: &["typescript-language-server", "--stdio"],
        install_hint: "npm install -g typescript-language-server typescript",
        languages: &[
            "typescript",
            "typescriptreact",
            "javascript",
            "javascriptreact",
        ],
    },
];

// ============================================================================
// Unified LSP Server Configuration
// ============================================================================

/// Unified LSP server configuration
/// Works for both built-in server overrides and custom servers
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(default)]
pub struct LspServerConfig {
    /// Disable this server (default: false)
    #[serde(default)]
    pub disabled: bool,

    /// Command to execute (required for custom servers, optional for built-ins)
    pub command: Option<String>,

    /// Command-line arguments
    #[serde(default)]
    pub args: Vec<String>,

    /// File extensions this server handles (required for custom servers)
    #[serde(default)]
    pub file_extensions: Vec<String>,

    /// Language identifiers for this server
    #[serde(default)]
    pub languages: Vec<String>,

    /// Environment variables to set when spawning server process
    #[serde(default)]
    pub env: HashMap<String, String>,

    /// Initialization options for LSP
    #[serde(default)]
    pub initialization_options: serde_json::Value,

    /// Workspace settings to send via workspace/didChangeConfiguration
    #[serde(default)]
    pub settings: serde_json::Value,

    /// Explicit workspace folder path (default: auto-detected)
    pub workspace_folder: Option<PathBuf>,

    // ---- Lifecycle configuration ----
    /// Max restart attempts before giving up (default: 3)
    #[serde(default = "default_max_restarts")]
    pub max_restarts: i32,

    /// Auto-restart on crash (default: true)
    #[serde(default = "default_restart_on_crash")]
    pub restart_on_crash: bool,

    /// Startup/init timeout in milliseconds (default: 10_000)
    #[serde(default = "default_startup_timeout_ms")]
    pub startup_timeout_ms: i64,

    /// Shutdown timeout in milliseconds (default: 5_000)
    #[serde(default = "default_shutdown_timeout_ms")]
    pub shutdown_timeout_ms: i64,

    /// Request timeout in milliseconds (default: 30_000)
    #[serde(default = "default_request_timeout_ms")]
    pub request_timeout_ms: i64,

    /// Health check interval in milliseconds (default: 30_000)
    #[serde(default = "default_health_check_interval_ms")]
    pub health_check_interval_ms: i64,

    /// Notification channel buffer size (default: 100)
    #[serde(default = "default_notification_buffer_size")]
    pub notification_buffer_size: i32,
}

impl Default for LspServerConfig {
    fn default() -> Self {
        Self {
            disabled: false,
            command: None,
            args: Vec::new(),
            file_extensions: Vec::new(),
            languages: Vec::new(),
            env: HashMap::new(),
            initialization_options: serde_json::Value::Null,
            settings: serde_json::Value::Null,
            workspace_folder: None,
            max_restarts: default_max_restarts(),
            restart_on_crash: default_restart_on_crash(),
            startup_timeout_ms: default_startup_timeout_ms(),
            shutdown_timeout_ms: default_shutdown_timeout_ms(),
            request_timeout_ms: default_request_timeout_ms(),
            health_check_interval_ms: default_health_check_interval_ms(),
            notification_buffer_size: default_notification_buffer_size(),
        }
    }
}

impl LspServerConfig {
    /// Returns true if this is a custom server (has command defined)
    pub fn is_custom(&self) -> bool {
        self.command.is_some()
    }
}

/// LSP servers configuration loaded from lsp_servers.json
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct LspServersConfig {
    /// All LSP server configurations (built-in overrides + custom)
    #[serde(default)]
    pub servers: HashMap<String, LspServerConfig>,
}

/// Config file name
pub const LSP_SERVERS_CONFIG_FILE: &str = "lsp_servers.json";

impl LspServersConfig {
    /// Load LSP config from standard locations
    /// Priority: project .codex/ > user ~/.codex/
    pub fn load(project_root: Option<&Path>) -> Self {
        let mut config = Self::default();

        // 1. Try user-level config first (~/.codex/lsp_servers.json)
        if let Some(home) = dirs::home_dir() {
            let user_path = home.join(".codex").join(LSP_SERVERS_CONFIG_FILE);
            if let Ok(user_config) = Self::from_file(&user_path) {
                debug!("Loaded user LSP config from: {}", user_path.display());
                config.merge(user_config);
            }
        }

        // 2. Try project-level config (.codex/lsp_servers.json)
        if let Some(root) = project_root {
            let project_path = root.join(".codex").join(LSP_SERVERS_CONFIG_FILE);
            if let Ok(project_config) = Self::from_file(&project_path) {
                debug!("Loaded project LSP config from: {}", project_path.display());
                config.merge(project_config); // Project overrides user
            }
        }

        if !config.servers.is_empty() {
            debug!(
                "LSP config loaded: {} servers configured",
                config.servers.len()
            );
        }

        config
    }

    /// Load configuration from a JSON file
    pub fn from_file(path: &Path) -> Result<Self, std::io::Error> {
        if !path.exists() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("LSP config file not found: {}", path.display()),
            ));
        }

        let content = std::fs::read_to_string(path)?;
        let config: LspServersConfig = serde_json::from_str(&content).map_err(|e| {
            warn!("Failed to parse LSP config {}: {}", path.display(), e);
            std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("Failed to parse LSP config JSON: {e}"),
            )
        })?;
        Ok(config)
    }

    /// Merge another config into this one (other values override self)
    pub fn merge(&mut self, other: Self) {
        for (key, value) in other.servers {
            self.servers.insert(key, value);
        }
    }

    /// Get server config by id
    pub fn get(&self, server_id: &str) -> Option<&LspServerConfig> {
        self.servers.get(server_id)
    }

    /// Check if a server is disabled
    pub fn is_disabled(&self, server_id: &str) -> bool {
        self.servers
            .get(server_id)
            .map(|c| c.disabled)
            .unwrap_or(false)
    }
}

/// Lifecycle configuration extracted from server config
#[derive(Debug, Clone)]
pub struct LifecycleConfig {
    pub max_restarts: i32,
    pub restart_on_crash: bool,
    pub health_check_interval_ms: i64,
    pub startup_timeout_ms: i64,
    pub shutdown_timeout_ms: i64,
    pub request_timeout_ms: i64,
    pub notification_buffer_size: i32,
}

impl From<&LspServerConfig> for LifecycleConfig {
    fn from(config: &LspServerConfig) -> Self {
        Self {
            max_restarts: config.max_restarts,
            restart_on_crash: config.restart_on_crash,
            health_check_interval_ms: config.health_check_interval_ms,
            startup_timeout_ms: config.startup_timeout_ms,
            shutdown_timeout_ms: config.shutdown_timeout_ms,
            request_timeout_ms: config.request_timeout_ms,
            notification_buffer_size: config.notification_buffer_size,
        }
    }
}

impl Default for LifecycleConfig {
    fn default() -> Self {
        Self {
            max_restarts: default_max_restarts(),
            restart_on_crash: default_restart_on_crash(),
            health_check_interval_ms: default_health_check_interval_ms(),
            startup_timeout_ms: default_startup_timeout_ms(),
            shutdown_timeout_ms: default_shutdown_timeout_ms(),
            request_timeout_ms: default_request_timeout_ms(),
            notification_buffer_size: default_notification_buffer_size(),
        }
    }
}

impl BuiltinServer {
    /// Find builtin server by file extension
    pub fn find_by_extension(ext: &str) -> Option<&'static BuiltinServer> {
        BUILTIN_SERVERS.iter().find(|s| s.extensions.contains(&ext))
    }

    /// Find builtin server by id
    pub fn find_by_id(id: &str) -> Option<&'static BuiltinServer> {
        BUILTIN_SERVERS.iter().find(|s| s.id == id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_by_extension() {
        let server = BuiltinServer::find_by_extension(".rs");
        assert!(server.is_some());
        assert_eq!(server.unwrap().id, "rust-analyzer");

        let server = BuiltinServer::find_by_extension(".go");
        assert!(server.is_some());
        assert_eq!(server.unwrap().id, "gopls");

        let server = BuiltinServer::find_by_extension(".py");
        assert!(server.is_some());
        assert_eq!(server.unwrap().id, "pyright");

        let server = BuiltinServer::find_by_extension(".ts");
        assert!(server.is_some());
        assert_eq!(server.unwrap().id, "typescript-language-server");

        let server = BuiltinServer::find_by_extension(".tsx");
        assert!(server.is_some());
        assert_eq!(server.unwrap().id, "typescript-language-server");

        let server = BuiltinServer::find_by_extension(".js");
        assert!(server.is_some());
        assert_eq!(server.unwrap().id, "typescript-language-server");

        let server = BuiltinServer::find_by_extension(".txt");
        assert!(server.is_none());
    }

    #[test]
    fn test_find_by_id() {
        let server = BuiltinServer::find_by_id("rust-analyzer");
        assert!(server.is_some());

        let server = BuiltinServer::find_by_id("unknown");
        assert!(server.is_none());
    }

    #[test]
    fn test_server_config_default() {
        let config = LspServerConfig::default();
        assert!(!config.disabled);
        assert!(config.command.is_none());
        assert!(config.args.is_empty());
        assert!(config.file_extensions.is_empty());
        assert_eq!(config.max_restarts, 3);
        assert!(config.restart_on_crash);
    }

    #[test]
    fn test_server_config_is_custom() {
        let builtin_override = LspServerConfig {
            disabled: false,
            command: None,
            max_restarts: 5,
            ..Default::default()
        };
        assert!(!builtin_override.is_custom());

        let custom = LspServerConfig {
            command: Some("my-lsp".to_string()),
            args: vec!["--stdio".to_string()],
            file_extensions: vec![".xyz".to_string()],
            ..Default::default()
        };
        assert!(custom.is_custom());
    }

    #[test]
    fn test_server_config_serde() {
        let json = r#"{
            "disabled": false,
            "command": "typescript-language-server",
            "args": ["--stdio"],
            "file_extensions": [".ts", ".tsx"],
            "languages": ["typescript"],
            "max_restarts": 5,
            "startup_timeout_ms": 15000
        }"#;

        let config: LspServerConfig = serde_json::from_str(json).unwrap();
        assert!(!config.disabled);
        assert_eq!(
            config.command,
            Some("typescript-language-server".to_string())
        );
        assert_eq!(config.args, vec!["--stdio"]);
        assert_eq!(config.file_extensions, vec![".ts", ".tsx"]);
        assert_eq!(config.languages, vec!["typescript"]);
        assert_eq!(config.max_restarts, 5);
        assert_eq!(config.startup_timeout_ms, 15_000);
    }

    #[test]
    fn test_servers_config_serde() {
        let json = r#"{
            "servers": {
                "rust-analyzer": {
                    "initialization_options": {"checkOnSave": {"command": "clippy"}},
                    "max_restarts": 5
                },
                "gopls": {
                    "disabled": true
                },
                "typescript": {
                    "command": "typescript-language-server",
                    "args": ["--stdio"],
                    "file_extensions": [".ts", ".tsx", ".js", ".jsx"],
                    "languages": ["typescript", "javascript"]
                }
            }
        }"#;

        let config: LspServersConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.servers.len(), 3);

        // Check rust-analyzer (builtin override)
        let ra = config.get("rust-analyzer").unwrap();
        assert!(!ra.is_custom());
        assert_eq!(ra.max_restarts, 5);

        // Check gopls (disabled)
        assert!(config.is_disabled("gopls"));

        // Check typescript (custom)
        let ts = config.get("typescript").unwrap();
        assert!(ts.is_custom());
        assert_eq!(ts.command, Some("typescript-language-server".to_string()));
    }

    #[test]
    fn test_servers_config_merge() {
        let mut base = LspServersConfig::default();
        base.servers.insert(
            "rust-analyzer".to_string(),
            LspServerConfig {
                max_restarts: 3,
                ..Default::default()
            },
        );

        let override_config = LspServersConfig {
            servers: HashMap::from([(
                "rust-analyzer".to_string(),
                LspServerConfig {
                    max_restarts: 10,
                    ..Default::default()
                },
            )]),
        };

        base.merge(override_config);
        assert_eq!(base.get("rust-analyzer").unwrap().max_restarts, 10);
    }

    #[test]
    fn test_lifecycle_config_from_server_config() {
        let server_config = LspServerConfig {
            max_restarts: 5,
            restart_on_crash: false,
            health_check_interval_ms: 60_000,
            startup_timeout_ms: 15_000,
            shutdown_timeout_ms: 3_000,
            request_timeout_ms: 45_000,
            ..Default::default()
        };
        let lifecycle: LifecycleConfig = (&server_config).into();
        assert_eq!(lifecycle.max_restarts, 5);
        assert!(!lifecycle.restart_on_crash);
        assert_eq!(lifecycle.health_check_interval_ms, 60_000);
    }

    #[test]
    fn test_from_json_file() {
        use std::io::Write;

        let temp_dir = std::env::temp_dir().join("lsp_config_test_simplified");
        std::fs::create_dir_all(&temp_dir).unwrap();
        let json_path = temp_dir.join("lsp_servers.json");

        let json_content = r#"{
            "servers": {
                "clangd": {
                    "command": "clangd",
                    "args": ["--background-index"],
                    "file_extensions": [".c", ".cpp", ".h"],
                    "languages": ["c", "cpp"]
                }
            }
        }"#;

        let mut file = std::fs::File::create(&json_path).unwrap();
        file.write_all(json_content.as_bytes()).unwrap();

        let config = LspServersConfig::from_file(&json_path).unwrap();
        assert!(config.servers.contains_key("clangd"));
        let clangd = config.get("clangd").unwrap();
        assert!(clangd.is_custom());
        assert_eq!(clangd.command, Some("clangd".to_string()));

        // Cleanup
        std::fs::remove_file(&json_path).unwrap();
        let _ = std::fs::remove_dir(&temp_dir);
    }

    #[test]
    fn test_from_file_not_found() {
        let result = LspServersConfig::from_file(Path::new("/nonexistent/path.json"));
        assert!(result.is_err());
    }

    #[test]
    fn test_backward_compat_without_new_fields() {
        // Old config without new fields should still work
        let json = r#"{"disabled": false}"#;
        let config: LspServerConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.max_restarts, 3); // default
        assert!(config.restart_on_crash); // default
        assert_eq!(config.startup_timeout_ms, 10_000); // default
        assert_eq!(config.shutdown_timeout_ms, 5_000); // default
        assert_eq!(config.health_check_interval_ms, 30_000); // default
    }
}

//! LSP configuration types

use serde::Deserialize;
use serde::Serialize;
use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;
use tracing::debug;
use tracing::warn;

/// Find the codex home directory.
///
/// Respects `CODEX_HOME` environment variable, falls back to `~/.codex`.
pub fn find_codex_home() -> Option<PathBuf> {
    // Honor the `CODEX_HOME` environment variable when it is set
    if let Ok(val) = std::env::var("CODEX_HOME") {
        if !val.is_empty() {
            return Some(PathBuf::from(val));
        }
    }
    // Fall back to ~/.codex
    dirs::home_dir().map(|h| h.join(".codex"))
}

// ============================================================================
// Default value constants
// ============================================================================

const DEFAULT_MAX_RESTARTS: i32 = 3;
const DEFAULT_RESTART_ON_CRASH: bool = true;
const DEFAULT_STARTUP_TIMEOUT_MS: i64 = 10_000;
const DEFAULT_SHUTDOWN_TIMEOUT_MS: i64 = 5_000;
const DEFAULT_REQUEST_TIMEOUT_MS: i64 = 30_000;
const DEFAULT_HEALTH_CHECK_INTERVAL_MS: i64 = 30_000;
const DEFAULT_NOTIFICATION_BUFFER_SIZE: i32 = 100;

// ============================================================================
// Default value functions for serde
// ============================================================================

fn default_max_restarts() -> i32 {
    DEFAULT_MAX_RESTARTS
}

fn default_restart_on_crash() -> bool {
    DEFAULT_RESTART_ON_CRASH
}

fn default_startup_timeout_ms() -> i64 {
    DEFAULT_STARTUP_TIMEOUT_MS
}

fn default_shutdown_timeout_ms() -> i64 {
    DEFAULT_SHUTDOWN_TIMEOUT_MS
}

fn default_request_timeout_ms() -> i64 {
    DEFAULT_REQUEST_TIMEOUT_MS
}

fn default_health_check_interval_ms() -> i64 {
    DEFAULT_HEALTH_CHECK_INTERVAL_MS
}

fn default_notification_buffer_size() -> i32 {
    DEFAULT_NOTIFICATION_BUFFER_SIZE
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
// Works for both built-in server overrides and custom servers
// ============================================================================

// Helper functions for skip_serializing_if
// Note: serde's skip_serializing_if requires &T, hence #[allow(clippy::trivially_copy_pass_by_ref)]
#[allow(clippy::trivially_copy_pass_by_ref)]
fn is_false(b: &bool) -> bool {
    !*b
}

#[allow(clippy::trivially_copy_pass_by_ref)]
fn is_default_max_restarts(v: &i32) -> bool {
    *v == DEFAULT_MAX_RESTARTS
}

#[allow(clippy::trivially_copy_pass_by_ref)]
fn is_default_restart_on_crash(v: &bool) -> bool {
    *v == DEFAULT_RESTART_ON_CRASH
}

#[allow(clippy::trivially_copy_pass_by_ref)]
fn is_default_startup_timeout_ms(v: &i64) -> bool {
    *v == DEFAULT_STARTUP_TIMEOUT_MS
}

#[allow(clippy::trivially_copy_pass_by_ref)]
fn is_default_shutdown_timeout_ms(v: &i64) -> bool {
    *v == DEFAULT_SHUTDOWN_TIMEOUT_MS
}

#[allow(clippy::trivially_copy_pass_by_ref)]
fn is_default_request_timeout_ms(v: &i64) -> bool {
    *v == DEFAULT_REQUEST_TIMEOUT_MS
}

#[allow(clippy::trivially_copy_pass_by_ref)]
fn is_default_health_check_interval_ms(v: &i64) -> bool {
    *v == DEFAULT_HEALTH_CHECK_INTERVAL_MS
}

#[allow(clippy::trivially_copy_pass_by_ref)]
fn is_default_notification_buffer_size(v: &i32) -> bool {
    *v == DEFAULT_NOTIFICATION_BUFFER_SIZE
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(default)]
pub struct LspServerConfig {
    /// Disable this server (default: false)
    #[serde(default, skip_serializing_if = "is_false")]
    pub disabled: bool,

    /// Command to execute (required for custom servers, optional for built-ins)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,

    /// Command-line arguments
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub args: Vec<String>,

    /// File extensions this server handles (required for custom servers)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub file_extensions: Vec<String>,

    /// Language identifiers for this server
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub languages: Vec<String>,

    /// Environment variables to set when spawning server process
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub env: HashMap<String, String>,

    /// Initialization options for LSP
    #[serde(default, skip_serializing_if = "serde_json::Value::is_null")]
    pub initialization_options: serde_json::Value,

    /// Workspace settings to send via workspace/didChangeConfiguration
    #[serde(default, skip_serializing_if = "serde_json::Value::is_null")]
    pub settings: serde_json::Value,

    /// Explicit workspace folder path (default: auto-detected)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workspace_folder: Option<PathBuf>,

    // ---- Lifecycle configuration ----
    /// Max restart attempts before giving up (default: 3)
    #[serde(
        default = "default_max_restarts",
        skip_serializing_if = "is_default_max_restarts"
    )]
    pub max_restarts: i32,

    /// Auto-restart on crash (default: true)
    #[serde(
        default = "default_restart_on_crash",
        skip_serializing_if = "is_default_restart_on_crash"
    )]
    pub restart_on_crash: bool,

    /// Startup/init timeout in milliseconds (default: 10_000)
    #[serde(
        default = "default_startup_timeout_ms",
        skip_serializing_if = "is_default_startup_timeout_ms"
    )]
    pub startup_timeout_ms: i64,

    /// Shutdown timeout in milliseconds (default: 5_000)
    #[serde(
        default = "default_shutdown_timeout_ms",
        skip_serializing_if = "is_default_shutdown_timeout_ms"
    )]
    pub shutdown_timeout_ms: i64,

    /// Request timeout in milliseconds (default: 30_000)
    #[serde(
        default = "default_request_timeout_ms",
        skip_serializing_if = "is_default_request_timeout_ms"
    )]
    pub request_timeout_ms: i64,

    /// Health check interval in milliseconds (default: 30_000)
    #[serde(
        default = "default_health_check_interval_ms",
        skip_serializing_if = "is_default_health_check_interval_ms"
    )]
    pub health_check_interval_ms: i64,

    /// Notification channel buffer size (default: 100)
    #[serde(
        default = "default_notification_buffer_size",
        skip_serializing_if = "is_default_notification_buffer_size"
    )]
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
    /// Priority: project .codex/ > user {codex_home}/
    ///
    /// # Arguments
    /// * `codex_home` - Codex home directory (respects `CODEX_HOME` env var)
    /// * `project_root` - Project root directory for project-level config
    pub fn load(codex_home: Option<&Path>, project_root: Option<&Path>) -> Self {
        let mut config = Self::default();

        // 1. Try user-level config first ({codex_home}/lsp_servers.json)
        if let Some(home) = codex_home {
            let user_path = home.join(LSP_SERVERS_CONFIG_FILE);
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

    /// Format a human-readable timestamp for backup files
    /// Format: YYYYMMDD_HHMMSS (e.g., 20250102_143025)
    fn format_backup_timestamp() -> String {
        chrono::Local::now().format("%Y%m%d_%H%M%S").to_string()
    }

    /// Add a server to config file (creates file and directory if needed)
    ///
    /// This adds a minimal config entry for a built-in server, which will
    /// be completed from the builtin template at runtime.
    /// Creates a backup of existing config before modifying.
    pub fn add_server_to_file(config_dir: &Path, server_id: &str) -> std::io::Result<()> {
        let config_path = config_dir.join(LSP_SERVERS_CONFIG_FILE);

        // Create backup if file exists
        if config_path.exists() {
            let backup_path = config_dir.join(format!(
                "{}.backup.{}",
                LSP_SERVERS_CONFIG_FILE,
                Self::format_backup_timestamp()
            ));
            std::fs::copy(&config_path, &backup_path)?;
            debug!(backup = %backup_path.display(), "Created config backup");
        }

        // Load existing config or create default
        let mut config = if config_path.exists() {
            Self::from_file(&config_path)?
        } else {
            // Ensure directory exists
            std::fs::create_dir_all(config_dir)?;
            Self::default()
        };

        // Only add if not already present
        if !config.servers.contains_key(server_id) {
            config
                .servers
                .insert(server_id.to_string(), LspServerConfig::default());

            // Write back
            let json = serde_json::to_string_pretty(&config).map_err(|e| {
                std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!("Failed to serialize config: {e}"),
                )
            })?;
            std::fs::write(&config_path, json)?;
            debug!(
                server = server_id,
                path = %config_path.display(),
                "Added server to config file"
            );
        }

        Ok(())
    }

    /// Write config to file
    pub fn to_file(&self, path: &Path) -> std::io::Result<()> {
        let json = serde_json::to_string_pretty(self).map_err(|e| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("Failed to serialize config: {e}"),
            )
        })?;
        std::fs::write(path, json)
    }

    /// Remove a server from config file
    ///
    /// Returns Ok(true) if server was removed, Ok(false) if server was not found.
    /// Creates a backup of existing config before modifying.
    pub fn remove_server_from_file(config_dir: &Path, server_id: &str) -> std::io::Result<bool> {
        let config_path = config_dir.join(LSP_SERVERS_CONFIG_FILE);

        if !config_path.exists() {
            return Ok(false);
        }

        // Create backup
        let backup_path = config_dir.join(format!(
            "{}.backup.{}",
            LSP_SERVERS_CONFIG_FILE,
            Self::format_backup_timestamp()
        ));
        std::fs::copy(&config_path, &backup_path)?;
        debug!(backup = %backup_path.display(), "Created config backup");

        // Load and modify config
        let mut config = Self::from_file(&config_path)?;
        let removed = config.servers.remove(server_id).is_some();

        if removed {
            // Write back
            let json = serde_json::to_string_pretty(&config).map_err(|e| {
                std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!("Failed to serialize config: {e}"),
                )
            })?;
            std::fs::write(&config_path, json)?;
            debug!(
                server = server_id,
                path = %config_path.display(),
                "Removed server from config file"
            );
        }

        Ok(removed)
    }

    /// Toggle a server's disabled status in config file
    ///
    /// Returns Ok(Some(new_disabled_state)) on success, Ok(None) if server not found.
    /// Creates a backup of existing config before modifying.
    pub fn toggle_server_disabled(
        config_dir: &Path,
        server_id: &str,
    ) -> std::io::Result<Option<bool>> {
        let config_path = config_dir.join(LSP_SERVERS_CONFIG_FILE);

        if !config_path.exists() {
            return Ok(None);
        }

        // Create backup
        let backup_path = config_dir.join(format!(
            "{}.backup.{}",
            LSP_SERVERS_CONFIG_FILE,
            Self::format_backup_timestamp()
        ));
        std::fs::copy(&config_path, &backup_path)?;
        debug!(backup = %backup_path.display(), "Created config backup");

        // Load and modify config
        let mut config = Self::from_file(&config_path)?;

        let new_state = if let Some(server_config) = config.servers.get_mut(server_id) {
            server_config.disabled = !server_config.disabled;
            Some(server_config.disabled)
        } else {
            None
        };

        if new_state.is_some() {
            // Write back
            let json = serde_json::to_string_pretty(&config).map_err(|e| {
                std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!("Failed to serialize config: {e}"),
                )
            })?;
            std::fs::write(&config_path, json)?;
            debug!(
                server = server_id,
                new_disabled = new_state,
                path = %config_path.display(),
                "Toggled server disabled state"
            );
        }

        Ok(new_state)
    }

    /// Check which config level a server is configured at
    ///
    /// Returns the config level if configured, None otherwise.
    pub fn detect_config_level(
        server_id: &str,
        user_dir: &Path,
        project_dir: &Path,
    ) -> Option<ConfigLevel> {
        // Check project-level first (higher priority)
        let project_path = project_dir.join(LSP_SERVERS_CONFIG_FILE);
        if project_path.exists() {
            if let Ok(config) = Self::from_file(&project_path) {
                if config.servers.contains_key(server_id) {
                    return Some(ConfigLevel::Project);
                }
            }
        }

        // Check user-level
        let user_path = user_dir.join(LSP_SERVERS_CONFIG_FILE);
        if user_path.exists() {
            if let Ok(config) = Self::from_file(&user_path) {
                if config.servers.contains_key(server_id) {
                    return Some(ConfigLevel::User);
                }
            }
        }

        None
    }
}

/// Configuration level for a server
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigLevel {
    /// User-level config (~/.codex/lsp_servers.json)
    User,
    /// Project-level config (.codex/lsp_servers.json)
    Project,
}

impl std::fmt::Display for ConfigLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConfigLevel::User => write!(f, "User"),
            ConfigLevel::Project => write!(f, "Project"),
        }
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

// ============================================================================
// Shared Utilities
// ============================================================================

/// Check if a command exists in PATH using `which`
///
/// This is a shared utility used by both `LspServerManager` and `LspInstaller`
/// to verify that LSP server binaries are installed.
pub async fn command_exists(cmd: &str) -> bool {
    if cmd.is_empty() {
        return false;
    }
    let cmd = cmd.to_string();
    tokio::task::spawn_blocking(move || which::which(&cmd).is_ok())
        .await
        .unwrap_or(false)
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

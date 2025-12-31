//! LSP server manager - handles server lifecycle and client creation

use crate::client::LspClient;
use crate::config::BUILTIN_SERVERS;
use crate::config::BuiltinServer;
use crate::config::LifecycleConfig;
use crate::config::LspServerConfig;
use crate::config::LspServersConfig;
use crate::diagnostics::DiagnosticsStore;
use crate::error::LspErr;
use crate::error::Result;
use crate::lifecycle::ServerHealth;
use crate::lifecycle::ServerLifecycle;
use crate::protocol::TimeoutConfig;
use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use std::time::Instant;
use tokio::process::Command;
use tokio::sync::Mutex;
use tracing::debug;
use tracing::info;
use tracing::warn;

/// Key for caching server connections: (server_id, root_path)
type ServerKey = (String, PathBuf);

/// Minimum interval between health checks per server (seconds)
const HEALTH_CHECK_MIN_INTERVAL_SECS: u64 = 30;

/// Server info for spawning (unified for builtin and custom servers)
#[derive(Debug, Clone)]
struct ServerInfo {
    id: String,
    command: String,
    args: Vec<String>,
    env: HashMap<String, String>,
    init_options: Option<serde_json::Value>,
    settings: Option<serde_json::Value>,
    lifecycle_config: LifecycleConfig,
}

/// LSP server manager - manages multiple server instances
pub struct LspServerManager {
    config: LspServersConfig,
    diagnostics: Arc<DiagnosticsStore>,
    /// Cached clients by (server_id, root_path)
    clients: Arc<Mutex<HashMap<ServerKey, Arc<LspClient>>>>,
    /// Lifecycle managers by server key
    lifecycles: Arc<Mutex<HashMap<ServerKey, Arc<ServerLifecycle>>>>,
    /// Last health check time per server key (for rate limiting)
    last_health_checks: Arc<Mutex<HashMap<ServerKey, Instant>>>,
}

impl LspServerManager {
    /// Create a new server manager with explicit config
    pub fn new(config: LspServersConfig, diagnostics: Arc<DiagnosticsStore>) -> Self {
        Self {
            config,
            diagnostics,
            clients: Arc::new(Mutex::new(HashMap::new())),
            lifecycles: Arc::new(Mutex::new(HashMap::new())),
            last_health_checks: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Create a new server manager, auto-loading config from standard locations
    pub fn with_auto_config(
        project_root: Option<&Path>,
        diagnostics: Arc<DiagnosticsStore>,
    ) -> Self {
        let config = LspServersConfig::load(project_root);
        Self::new(config, diagnostics)
    }

    /// Get or create a client for a file
    pub async fn get_client(&self, file_path: &Path) -> Result<Arc<LspClient>> {
        let file_path = match file_path.canonicalize() {
            Ok(p) => p,
            Err(e) => {
                warn!(
                    "Failed to canonicalize path {}: {}, using original path",
                    file_path.display(),
                    e
                );
                file_path.to_path_buf()
            }
        };

        // Find appropriate server for file extension
        let ext = file_path
            .extension()
            .and_then(|e| e.to_str())
            .map(|s| format!(".{s}"))
            .unwrap_or_default();

        info!(
            "LSP client requested for {} (ext: {})",
            file_path.display(),
            ext
        );

        let server_info = self.find_server_for_extension(&ext).await?;
        info!(
            "Selected LSP server '{}' for extension '{}'",
            server_info.id, ext
        );

        // Find project root (or use configured workspace folder)
        let root_path = self.find_project_root(&file_path);

        let key = (server_info.id.clone(), root_path.clone());

        // Check if existing client is healthy (with rate-limited health checks)
        // Lock ordering: We acquire locks sequentially and release each before acquiring the next
        // to avoid potential deadlocks. The pattern is:
        // 1. Check clients -> get cloned client ref (release lock)
        // 2. Check last_health_checks -> determine if check needed (release lock)
        // 3. Check lifecycles -> get health status (release lock)
        // 4. Update last_health_checks if needed (release lock)

        // Step 1: Get cached client (if any)
        let cached_client = {
            let clients = self.clients.lock().await;
            clients.get(&key).cloned()
        };

        if let Some(client) = cached_client {
            // Step 2: Check if we should perform health check (rate limiting)
            let should_check = {
                let last_checks = self.last_health_checks.lock().await;
                match last_checks.get(&key) {
                    Some(last) => {
                        last.elapsed() >= Duration::from_secs(HEALTH_CHECK_MIN_INTERVAL_SECS)
                    }
                    None => true, // Never checked, should check
                }
            };

            if !should_check {
                // Skip health check, assume healthy (checked recently)
                info!(
                    "Using cached LSP client for {} ({})",
                    server_info.id,
                    root_path.display()
                );
                return Ok(client);
            }

            // Step 3: Get lifecycle and check health
            let (health, restart_count, is_restarting, has_lifecycle) = {
                let lifecycles = self.lifecycles.lock().await;
                if let Some(lifecycle) = lifecycles.get(&key) {
                    let health = lifecycle.health().await;
                    let restart_count = lifecycle.get_restart_count();
                    let is_restarting = lifecycle.is_restarting();
                    (Some(health), restart_count, is_restarting, true)
                } else {
                    (None, 0, false, false)
                }
            };

            // Step 4: Update last health check time
            {
                let mut last_checks = self.last_health_checks.lock().await;
                last_checks.insert(key.clone(), Instant::now());
            }

            // Process health check result
            if !has_lifecycle {
                // No lifecycle manager, assume healthy
                return Ok(client);
            }

            match health {
                Some(ServerHealth::Healthy) => {
                    return Ok(client);
                }
                Some(ServerHealth::Failed) => {
                    return Err(LspErr::ServerFailed {
                        server: key.0.clone(),
                        restarts: restart_count,
                    });
                }
                _ if is_restarting => {
                    return Err(LspErr::ServerRestarting {
                        server: key.0.clone(),
                    });
                }
                _ => {
                    // Server needs restart, fall through to spawn_server_with_lifecycle
                }
            }
        }

        // Spawn new client with lifecycle tracking
        self.spawn_server_with_lifecycle(&key, &server_info, &root_path)
            .await
    }

    /// Spawn a server with lifecycle tracking
    async fn spawn_server_with_lifecycle(
        &self,
        key: &ServerKey,
        server_info: &ServerInfo,
        root_path: &Path,
    ) -> Result<Arc<LspClient>> {
        info!(
            "Spawning LSP server {} at {}",
            server_info.id,
            root_path.display()
        );

        // Get or create lifecycle manager
        let lifecycle = {
            let mut lifecycles = self.lifecycles.lock().await;
            if let Some(lc) = lifecycles.get(key) {
                Arc::clone(lc)
            } else {
                let lc = Arc::new(ServerLifecycle::new(
                    server_info.id.clone(),
                    server_info.lifecycle_config.clone(),
                ));
                lifecycles.insert(key.clone(), Arc::clone(&lc));
                lc
            }
        };

        // Check restart limits
        if !lifecycle.should_restart() && lifecycle.health().await == ServerHealth::Failed {
            return Err(LspErr::ServerFailed {
                server: key.0.clone(),
                restarts: lifecycle.get_restart_count(),
            });
        }

        // Mark as restarting
        lifecycle.set_restarting(true);
        let restart_attempt = lifecycle.increment_restart_count();

        if restart_attempt > 1 {
            info!(
                "LSP server {} restart attempt {}/{}",
                server_info.id, restart_attempt, server_info.lifecycle_config.max_restarts
            );
        }

        // Clear symbol cache from old client before spawning new one
        // This ensures stale symbol information is not returned after restart
        {
            let clients = self.clients.lock().await;
            if let Some(old_client) = clients.get(key) {
                old_client.clear_symbol_cache().await;
            }
        }

        // Attempt to spawn server
        match self.spawn_server(server_info, root_path).await {
            Ok(client) => {
                lifecycle.record_started().await;
                lifecycle.set_restarting(false);

                let client = Arc::new(client);

                // Start health check if enabled
                if server_info.lifecycle_config.health_check_interval_ms > 0 {
                    let client_clone = Arc::clone(&client);
                    let handle = lifecycle.start_health_check(move || {
                        let client = Arc::clone(&client_clone);
                        async move { client.health_check().await }
                    });
                    lifecycle.set_health_check_handle(handle).await;
                }

                // Cache client
                let mut clients = self.clients.lock().await;
                clients.insert(key.clone(), Arc::clone(&client));

                Ok(client)
            }
            Err(e) => {
                lifecycle.set_restarting(false);
                if lifecycle.record_crash().await {
                    // Can retry - but return error for this call
                    // Next call will attempt restart
                    warn!("LSP server {} failed to start: {}", server_info.id, e);
                }
                Err(e)
            }
        }
    }

    /// Find server configuration for file extension
    async fn find_server_for_extension(&self, ext: &str) -> Result<ServerInfo> {
        // First, check custom servers (those with command defined)
        for (id, config) in &self.config.servers {
            if config.disabled {
                continue;
            }

            if config.is_custom() {
                // Custom server: check file_extensions
                if config.file_extensions.iter().any(|e| e == ext) {
                    return Ok(self.build_server_info_from_config(id, config));
                }
            }
        }

        // Then, check builtin servers
        let builtin =
            BuiltinServer::find_by_extension(ext).ok_or_else(|| LspErr::NoServerForExtension {
                ext: ext.to_string(),
            })?;

        // Check if disabled in config
        if self.config.is_disabled(builtin.id) {
            return Err(LspErr::NoServerForExtension {
                ext: ext.to_string(),
            });
        }

        self.build_server_info_from_builtin(builtin).await
    }

    /// Build ServerInfo from a unified server config (custom server)
    fn build_server_info_from_config(&self, id: &str, config: &LspServerConfig) -> ServerInfo {
        ServerInfo {
            id: id.to_string(),
            command: config.command.clone().unwrap_or_default(),
            args: config.args.clone(),
            env: config.env.clone(),
            init_options: if config.initialization_options.is_null() {
                None
            } else {
                Some(config.initialization_options.clone())
            },
            settings: if config.settings.is_null() {
                None
            } else {
                Some(config.settings.clone())
            },
            lifecycle_config: LifecycleConfig::from(config),
        }
    }

    /// Build ServerInfo from a builtin server
    ///
    /// Note: This method resolves the command asynchronously to avoid blocking
    /// when checking for binary availability with `which::which()`.
    async fn build_server_info_from_builtin(
        &self,
        builtin: &'static BuiltinServer,
    ) -> Result<ServerInfo> {
        let server_config = self.config.get(builtin.id);

        // Get command: use config override or first builtin command
        let (command, args) = if let Some(config) = server_config {
            if let Some(custom_cmd) = &config.command {
                // Custom command specified - use it with args from config
                (custom_cmd.clone(), config.args.clone())
            } else {
                Self::get_builtin_command_fallback(builtin).await
            }
        } else {
            Self::get_builtin_command_fallback(builtin).await
        };

        let lifecycle_config = server_config.map(LifecycleConfig::from).unwrap_or_default();

        let env = server_config.map(|c| c.env.clone()).unwrap_or_default();

        let init_options = server_config.and_then(|c| {
            if c.initialization_options.is_null() {
                None
            } else {
                Some(c.initialization_options.clone())
            }
        });

        let settings = server_config.and_then(|c| {
            if c.settings.is_null() {
                None
            } else {
                Some(c.settings.clone())
            }
        });

        Ok(ServerInfo {
            id: builtin.id.to_string(),
            command,
            args,
            env,
            init_options,
            settings,
            lifecycle_config,
        })
    }

    /// Get command from builtin server (uses first command as fallback)
    ///
    /// This method uses `spawn_blocking` to avoid blocking the async runtime
    /// when calling `which::which()` to check for binary availability.
    async fn get_builtin_command_fallback(
        builtin: &'static BuiltinServer,
    ) -> (String, Vec<String>) {
        // Move blocking which::which() calls to the blocking thread pool
        tokio::task::spawn_blocking(move || {
            // Try to find an available binary
            for cmd in builtin.commands {
                let parts: Vec<&str> = cmd.split_whitespace().collect();
                if parts.is_empty() {
                    continue;
                }

                let program = parts[0];
                if which::which(program).is_ok() {
                    let args = parts[1..].iter().map(|s| s.to_string()).collect();
                    return (program.to_string(), args);
                }
            }

            // Fallback to first command (will fail at spawn time if not found)
            let first_cmd = builtin.commands.first().unwrap_or(&"");
            let parts: Vec<&str> = first_cmd.split_whitespace().collect();
            if parts.is_empty() {
                (String::new(), Vec::new())
            } else {
                let program = parts[0].to_string();
                let args = parts[1..].iter().map(|s| s.to_string()).collect();
                (program, args)
            }
        })
        .await
        .unwrap_or_else(|_| {
            // JoinError fallback - use first command
            let first_cmd = builtin.commands.first().unwrap_or(&"");
            let parts: Vec<&str> = first_cmd.split_whitespace().collect();
            if parts.is_empty() {
                (String::new(), Vec::new())
            } else {
                (
                    parts[0].to_string(),
                    parts[1..].iter().map(|s| s.to_string()).collect(),
                )
            }
        })
    }

    /// Find project root from file path
    fn find_project_root(&self, file_path: &Path) -> PathBuf {
        let mut current = file_path.parent();

        while let Some(dir) = current {
            // Check for common project root indicators
            let markers = [
                "Cargo.toml",     // Rust
                "go.mod",         // Go
                "pyproject.toml", // Python
                "setup.py",       // Python
                "package.json",   // Node
                ".git",           // Git root
            ];

            for marker in markers {
                if dir.join(marker).exists() {
                    return dir.to_path_buf();
                }
            }

            current = dir.parent();
        }

        // Fallback to file's directory
        file_path.parent().unwrap_or(file_path).to_path_buf()
    }

    /// Spawn a new LSP server process
    async fn spawn_server(&self, server_info: &ServerInfo, root_path: &Path) -> Result<LspClient> {
        info!(
            "Starting LSP server: {} in {} (cmd: {} {:?}, env: {} vars)",
            server_info.id,
            root_path.display(),
            server_info.command,
            server_info.args,
            server_info.env.len()
        );

        // Build command
        let mut cmd = Command::new(&server_info.command);
        cmd.args(&server_info.args)
            .current_dir(root_path)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .kill_on_drop(true);

        // Set environment variables
        for (key, value) in &server_info.env {
            cmd.env(key, value);
        }

        // Spawn process
        let mut child = cmd.spawn().map_err(|e| LspErr::ServerStartFailed {
            server: server_info.id.clone(),
            reason: e.to_string(),
        })?;

        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| LspErr::ServerStartFailed {
                server: server_info.id.clone(),
                reason: "failed to get stdin".to_string(),
            })?;

        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| LspErr::ServerStartFailed {
                server: server_info.id.clone(),
                reason: "failed to get stdout".to_string(),
            })?;

        // Capture stderr for debugging
        if let Some(stderr) = child.stderr.take() {
            let server_id = server_info.id.clone();
            tokio::spawn(async move {
                use tokio::io::AsyncBufReadExt;
                use tokio::io::BufReader;

                let reader = BufReader::new(stderr);
                let mut lines = reader.lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    debug!(
                        target: "lsp_stderr",
                        server = %server_id,
                        "{}",
                        line
                    );
                }
            });
        }

        // Build timeout config from lifecycle config
        let timeout_config = TimeoutConfig::from(&server_info.lifecycle_config);

        // Create client
        LspClient::new(
            stdin,
            stdout,
            server_info.id.clone(),
            root_path,
            Arc::clone(&self.diagnostics),
            server_info.init_options.clone(),
            server_info.settings.clone(),
            timeout_config,
        )
        .await
    }

    /// Check if LSP is available for a file extension
    pub async fn is_available(&self, file_path: &Path) -> bool {
        let ext = file_path
            .extension()
            .and_then(|e| e.to_str())
            .map(|s| format!(".{s}"))
            .unwrap_or_default();

        self.find_server_for_extension(&ext).await.is_ok()
    }

    /// Get diagnostics store
    pub fn diagnostics(&self) -> &Arc<DiagnosticsStore> {
        &self.diagnostics
    }

    /// List supported file extensions (builtin servers only)
    pub fn supported_extensions() -> Vec<&'static str> {
        BUILTIN_SERVERS
            .iter()
            .flat_map(|s| s.extensions.iter())
            .copied()
            .collect()
    }

    /// List all supported file extensions (builtin + custom)
    pub fn all_supported_extensions(&self) -> Vec<String> {
        let mut exts: Vec<String> = BUILTIN_SERVERS
            .iter()
            .flat_map(|s| s.extensions.iter())
            .map(|s| s.to_string())
            .collect();

        // Add custom server extensions
        for config in self.config.servers.values() {
            if config.is_custom() && !config.disabled {
                exts.extend(config.file_extensions.iter().cloned());
            }
        }

        exts.sort();
        exts.dedup();
        exts
    }

    /// Shutdown all server connections
    pub async fn shutdown_all(&self) {
        let client_count = self.clients.lock().await.len();
        info!("Shutting down {} LSP server(s)", client_count);

        // Signal shutdown to all lifecycle managers
        {
            let lifecycles = self.lifecycles.lock().await;
            for lifecycle in lifecycles.values() {
                lifecycle.signal_shutdown();
            }
        }

        // Shutdown all clients
        let mut clients = self.clients.lock().await;
        for (key, client) in clients.drain() {
            debug!("Shutting down LSP client: {:?}", key);
            if let Err(e) = client.shutdown().await {
                warn!("Error shutting down LSP client {:?}: {}", key, e);
            }
        }

        // Cleanup lifecycle managers
        {
            let mut lifecycles = self.lifecycles.lock().await;
            for lifecycle in lifecycles.values() {
                lifecycle.abort_health_check().await;
            }
            lifecycles.clear();
        }

        info!("All LSP servers shut down");
    }

    /// Get lifecycle manager for a server (for monitoring/testing)
    pub async fn get_lifecycle(
        &self,
        server_id: &str,
        root_path: &Path,
    ) -> Option<Arc<ServerLifecycle>> {
        let key = (server_id.to_string(), root_path.to_path_buf());
        let lifecycles = self.lifecycles.lock().await;
        lifecycles.get(&key).cloned()
    }

    /// Pre-warm language servers for given file extensions
    ///
    /// This spawns servers in the background for the specified extensions,
    /// reducing latency for the first LSP operation. Useful during startup
    /// when you know which languages will be used.
    ///
    /// # Arguments
    /// * `extensions` - File extensions to pre-warm (e.g., ".rs", ".go", ".py")
    /// * `project_root` - Project root directory for server initialization
    ///
    /// # Returns
    /// List of server IDs that were successfully warmed up
    ///
    /// # Example
    /// ```ignore
    /// let manager = LspServerManager::new(config, diagnostics);
    /// let warmed = manager.prewarm(&[".rs", ".go"], project_root).await;
    /// ```
    pub async fn prewarm(&self, extensions: &[&str], project_root: &Path) -> Vec<String> {
        let mut warmed = Vec::new();

        for ext in extensions {
            // Check if we have a server for this extension
            match self.find_server_for_extension(ext).await {
                Ok(server_info) => {
                    let key = (server_info.id.clone(), project_root.to_path_buf());

                    // Check if already cached
                    {
                        let clients = self.clients.lock().await;
                        if clients.contains_key(&key) {
                            debug!(
                                "Server {} already running for extension {}",
                                server_info.id, ext
                            );
                            warmed.push(server_info.id);
                            continue;
                        }
                    }

                    // Try to spawn the server
                    info!(
                        "Pre-warming LSP server {} for extension {}",
                        server_info.id, ext
                    );

                    match self
                        .spawn_server_with_lifecycle(&key, &server_info, project_root)
                        .await
                    {
                        Ok(_client) => {
                            info!(
                                "Successfully pre-warmed LSP server {} for extension {}",
                                server_info.id, ext
                            );
                            warmed.push(server_info.id);
                        }
                        Err(e) => {
                            warn!(
                                "Failed to pre-warm LSP server {} for extension {}: {}",
                                server_info.id, ext, e
                            );
                        }
                    }
                }
                Err(e) => {
                    debug!("No server available for extension {}: {}", ext, e);
                }
            }
        }

        if !warmed.is_empty() {
            info!("Pre-warmed {} LSP server(s): {:?}", warmed.len(), warmed);
        }

        warmed
    }
}

impl std::fmt::Debug for LspServerManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LspServerManager")
            .field("config", &self.config)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_supported_extensions() {
        let exts = LspServerManager::supported_extensions();
        assert!(exts.contains(&".rs"));
        assert!(exts.contains(&".go"));
        assert!(exts.contains(&".py"));
    }

    #[tokio::test]
    async fn test_find_server_for_extension() {
        let config = LspServersConfig::default();
        let diagnostics = Arc::new(DiagnosticsStore::new());
        let manager = LspServerManager::new(config, diagnostics);

        assert!(manager.find_server_for_extension(".rs").await.is_ok());
        assert!(manager.find_server_for_extension(".go").await.is_ok());
        assert!(manager.find_server_for_extension(".py").await.is_ok());
        assert!(manager.find_server_for_extension(".txt").await.is_err());
    }

    #[tokio::test]
    async fn test_find_server_disabled() {
        let mut config = LspServersConfig::default();
        config.servers.insert(
            "rust-analyzer".to_string(),
            LspServerConfig {
                disabled: true,
                ..Default::default()
            },
        );

        let diagnostics = Arc::new(DiagnosticsStore::new());
        let manager = LspServerManager::new(config, diagnostics);

        assert!(manager.find_server_for_extension(".rs").await.is_err());
        assert!(manager.find_server_for_extension(".go").await.is_ok());
    }

    #[test]
    fn test_find_project_root() {
        let config = LspServersConfig::default();
        let diagnostics = Arc::new(DiagnosticsStore::new());
        let manager = LspServerManager::new(config, diagnostics);

        // For non-existent paths, should return parent directory
        let root = manager.find_project_root(Path::new("/some/path/file.rs"));
        assert_eq!(root, Path::new("/some/path"));
    }

    #[tokio::test]
    async fn test_custom_server_priority() {
        let mut config = LspServersConfig::default();

        // Add custom server for .rs extension (should override builtin)
        config.servers.insert(
            "my-rust-lsp".to_string(),
            LspServerConfig {
                command: Some("my-rust-lsp".to_string()),
                args: vec!["--stdio".to_string()],
                languages: vec!["rust".to_string()],
                file_extensions: vec![".rs".to_string()],
                ..Default::default()
            },
        );

        let diagnostics = Arc::new(DiagnosticsStore::new());
        let manager = LspServerManager::new(config, diagnostics);

        // Custom server should be found first
        let server_info = manager.find_server_for_extension(".rs").await.unwrap();
        assert_eq!(server_info.id, "my-rust-lsp");
        assert_eq!(server_info.command, "my-rust-lsp");
    }

    #[tokio::test]
    async fn test_custom_server_new_extension() {
        let mut config = LspServersConfig::default();

        // Add custom server for a new extension
        config.servers.insert(
            "typescript-lsp".to_string(),
            LspServerConfig {
                command: Some("typescript-language-server".to_string()),
                args: vec!["--stdio".to_string()],
                languages: vec!["typescript".to_string()],
                file_extensions: vec![".ts".to_string(), ".tsx".to_string()],
                ..Default::default()
            },
        );

        let diagnostics = Arc::new(DiagnosticsStore::new());
        let manager = LspServerManager::new(config, diagnostics);

        // Custom extension should be found
        let server_info = manager.find_server_for_extension(".ts").await.unwrap();
        assert_eq!(server_info.id, "typescript-lsp");

        let server_info = manager.find_server_for_extension(".tsx").await.unwrap();
        assert_eq!(server_info.id, "typescript-lsp");
    }

    #[test]
    fn test_all_supported_extensions() {
        let mut config = LspServersConfig::default();

        config.servers.insert(
            "typescript-lsp".to_string(),
            LspServerConfig {
                command: Some("tsc".to_string()),
                args: vec![],
                languages: vec!["typescript".to_string()],
                file_extensions: vec![".ts".to_string()],
                ..Default::default()
            },
        );

        let diagnostics = Arc::new(DiagnosticsStore::new());
        let manager = LspServerManager::new(config, diagnostics);

        let exts = manager.all_supported_extensions();
        assert!(exts.contains(&".rs".to_string()));
        assert!(exts.contains(&".go".to_string()));
        assert!(exts.contains(&".py".to_string()));
        assert!(exts.contains(&".ts".to_string()));
    }

    #[tokio::test]
    async fn test_server_info_lifecycle_config() {
        let mut config = LspServersConfig::default();

        config.servers.insert(
            "rust-analyzer".to_string(),
            LspServerConfig {
                max_restarts: 5,
                restart_on_crash: false,
                startup_timeout_ms: 20_000,
                ..Default::default()
            },
        );

        let diagnostics = Arc::new(DiagnosticsStore::new());
        let manager = LspServerManager::new(config, diagnostics);

        let server_info = manager.find_server_for_extension(".rs").await.unwrap();
        assert_eq!(server_info.lifecycle_config.max_restarts, 5);
        assert!(!server_info.lifecycle_config.restart_on_crash);
        assert_eq!(server_info.lifecycle_config.startup_timeout_ms, 20_000);
    }
}

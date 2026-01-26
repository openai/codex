//! LSP server manager - handles server lifecycle and client creation

use crate::client::LspClient;
use crate::config::BUILTIN_SERVERS;
use crate::config::BuiltinServer;
use crate::config::ConfigLevel;
use crate::config::LifecycleConfig;
use crate::config::LspServerConfig;
use crate::config::LspServersConfig;
use crate::config::command_exists;
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
    /// File extensions this server handles
    extensions: Vec<String>,
    /// Language identifiers (reserved for future use)
    _languages: Vec<String>,
    env: HashMap<String, String>,
    init_options: Option<serde_json::Value>,
    settings: Option<serde_json::Value>,
    lifecycle_config: LifecycleConfig,
    /// Installation hint for when command is not found
    install_hint: String,
}

/// LSP server manager - manages multiple server instances
pub struct LspServerManager {
    /// Configuration (RwLock for runtime reload support)
    config: tokio::sync::RwLock<LspServersConfig>,
    /// Project root for config reload
    project_root: Option<PathBuf>,
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
    pub fn new(
        config: LspServersConfig,
        project_root: Option<PathBuf>,
        diagnostics: Arc<DiagnosticsStore>,
    ) -> Self {
        Self {
            config: tokio::sync::RwLock::new(config),
            project_root,
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
        let codex_home = crate::config::find_codex_home();
        let config = LspServersConfig::load(codex_home.as_deref(), project_root);
        Self::new(config, project_root.map(Path::to_path_buf), diagnostics)
    }

    /// Reload configuration from disk
    ///
    /// This updates the in-memory config to reflect changes made to config files.
    pub async fn reload_config(&self) {
        let codex_home = crate::config::find_codex_home();
        let new_config =
            LspServersConfig::load(codex_home.as_deref(), self.project_root.as_deref());
        let mut config = self.config.write().await;
        *config = new_config;
        debug!("LSP configuration reloaded");
    }

    /// Get or create a client for a file
    pub async fn get_client(&self, file_path: &Path) -> Result<Arc<LspClient>> {
        let file_path = match file_path.canonicalize() {
            Ok(p) => p,
            Err(e) => {
                warn!(
                    path = %file_path.display(),
                    error = %e,
                    "Failed to canonicalize path, using original"
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

                // ServerNotInstalled is permanent - don't retry
                if matches!(e, LspErr::ServerNotInstalled { .. }) {
                    lifecycle.set_health(ServerHealth::Failed).await;
                } else if lifecycle.record_crash().await {
                    // Can retry - but return error for this call
                    // Next call will attempt restart
                    warn!("LSP server {} failed to start: {}", server_info.id, e);
                }
                Err(e)
            }
        }
    }

    /// Find server configuration for file extension
    ///
    /// Only servers declared in lsp_servers.json are considered.
    /// Builtin templates are used to complete missing config fields.
    async fn find_server_for_extension(&self, ext: &str) -> Result<ServerInfo> {
        // Only check configured servers (no auto-matching of builtins)
        let config = self.config.read().await;
        for (id, server_config) in &config.servers {
            if server_config.disabled {
                continue;
            }

            // Build server info with template completion
            let server_info = self.build_server_info(id, server_config).await?;

            // Check if this server handles the extension
            if server_info.extensions.iter().any(|e| e == ext) {
                return Ok(server_info);
            }
        }

        Err(LspErr::NoServerForExtension {
            ext: ext.to_string(),
        })
    }

    /// Build server info from config, completing missing fields from builtin template
    ///
    /// This unified method handles both:
    /// - Builtin references (e.g., `"rust-analyzer": {}`) - fills from template
    /// - Custom servers (e.g., `"clangd": {"command": "clangd"}`) - uses config as-is
    async fn build_server_info(&self, id: &str, config: &LspServerConfig) -> Result<ServerInfo> {
        // Try to find a builtin template for this ID
        let template = BuiltinServer::find_by_id(id);

        // --- Command: user config > template ---
        let command = config.command.clone().or_else(|| {
            template.map(|t| {
                t.commands[0]
                    .split_whitespace()
                    .next()
                    .unwrap_or("")
                    .to_string()
            })
        });

        let Some(command) = command else {
            return Err(LspErr::MissingCommand {
                server_id: id.to_string(),
                hint: "Add 'command' field to config".to_string(),
            });
        };

        // --- Args: user config > template ---
        let args = if config.args.is_empty() {
            template
                .map(|t| {
                    t.commands[0]
                        .split_whitespace()
                        .skip(1)
                        .map(String::from)
                        .collect()
                })
                .unwrap_or_default()
        } else {
            config.args.clone()
        };

        // --- Extensions: user config > template ---
        let extensions = if config.file_extensions.is_empty() {
            template
                .map(|t| t.extensions.iter().map(ToString::to_string).collect())
                .unwrap_or_default()
        } else {
            config.file_extensions.clone()
        };

        // --- Languages: user config > template ---
        let languages = if config.languages.is_empty() {
            template
                .map(|t| t.languages.iter().map(ToString::to_string).collect())
                .unwrap_or_default()
        } else {
            config.languages.clone()
        };

        // --- Install hint: template > generic ---
        let install_hint = template
            .map(|t| t.install_hint.to_string())
            .unwrap_or_else(|| format!("Install the LSP server '{command}' manually"));

        // Log completion (only if template was used and fields were completed)
        if let Some(tmpl) = template {
            let completed_fields: Vec<&str> = [
                config.command.is_none().then_some("command"),
                config.args.is_empty().then_some("args"),
                config.file_extensions.is_empty().then_some("extensions"),
                config.languages.is_empty().then_some("languages"),
            ]
            .into_iter()
            .flatten()
            .collect();

            if !completed_fields.is_empty() {
                info!(
                    server = id,
                    template = tmpl.id,
                    completed_fields = ?completed_fields,
                    command = %command,
                    extensions = ?extensions,
                    "Completed server config from builtin template"
                );
            }
        }

        // Note: command_exists check is done in spawn_server(), not here
        // This allows find_server_for_extension() to succeed for config validation

        Ok(ServerInfo {
            id: id.to_string(),
            command,
            args,
            extensions,
            _languages: languages,
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
            install_hint,
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

        // Check if command exists before trying to spawn
        if !command_exists(&server_info.command).await {
            warn!(
                "LSP server '{}' not installed: command '{}' not found. Install: {}",
                server_info.id, server_info.command, server_info.install_hint
            );
            return Err(LspErr::ServerNotInstalled {
                server_id: server_info.id.clone(),
                command: server_info.command.clone(),
                install_hint: server_info.install_hint.clone(),
            });
        }

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

    /// List all supported file extensions (only configured servers)
    ///
    /// With opt-in design, only servers declared in lsp_servers.json are included.
    /// Extensions are resolved from:
    /// - Builtin template (if server ID matches a builtin)
    /// - User config's file_extensions field
    pub async fn all_supported_extensions(&self) -> Vec<String> {
        let mut exts: Vec<String> = Vec::new();

        let config = self.config.read().await;
        for (id, server_config) in &config.servers {
            if server_config.disabled {
                continue;
            }

            // Get extensions from user config or builtin template
            let extensions = if !server_config.file_extensions.is_empty() {
                server_config.file_extensions.clone()
            } else if let Some(builtin) = BuiltinServer::find_by_id(id) {
                builtin.extensions.iter().map(ToString::to_string).collect()
            } else {
                Vec::new()
            };

            exts.extend(extensions);
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

    /// Shutdown all LSP servers for a specific workspace root
    ///
    /// This is used for cleanup when a worktree is deleted (e.g., spawn agent completes).
    /// Only servers with matching root_path are shut down; other servers remain active.
    pub async fn shutdown_for_root(&self, root_path: &Path) {
        let root_path = root_path.to_path_buf();

        // Find keys to remove
        let keys_to_remove: Vec<ServerKey> = {
            let clients = self.clients.lock().await;
            clients
                .keys()
                .filter(|(_, path)| *path == root_path)
                .cloned()
                .collect()
        };

        if keys_to_remove.is_empty() {
            debug!(
                "No LSP servers to shutdown for root: {}",
                root_path.display()
            );
            return;
        }

        info!(
            "Shutting down {} LSP server(s) for root: {}",
            keys_to_remove.len(),
            root_path.display()
        );

        // Signal shutdown to affected lifecycle managers
        {
            let lifecycles = self.lifecycles.lock().await;
            for key in &keys_to_remove {
                if let Some(lifecycle) = lifecycles.get(key) {
                    lifecycle.signal_shutdown();
                }
            }
        }

        // Shutdown affected clients
        {
            let mut clients = self.clients.lock().await;
            for key in &keys_to_remove {
                if let Some(client) = clients.remove(key) {
                    debug!("Shutting down LSP client: {:?}", key);
                    if let Err(e) = client.shutdown().await {
                        warn!("Error shutting down LSP client {:?}: {}", key, e);
                    }
                }
            }
        }

        // Cleanup affected lifecycle managers
        {
            let mut lifecycles = self.lifecycles.lock().await;
            for key in &keys_to_remove {
                if let Some(lifecycle) = lifecycles.remove(key) {
                    lifecycle.abort_health_check().await;
                }
            }
        }

        // Cleanup health check timestamps
        {
            let mut last_checks = self.last_health_checks.lock().await;
            for key in &keys_to_remove {
                last_checks.remove(key);
            }
        }

        info!("LSP servers shut down for root: {}", root_path.display());
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
    /// let manager = LspServerManager::new(config, None, diagnostics);
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

    /// Get status of all configured LSP servers
    ///
    /// With opt-in design, only servers declared in lsp_servers.json are included.
    pub async fn get_all_servers_status(&self) -> Vec<ServerStatusInfo> {
        let mut statuses = Vec::new();

        // Only iterate over configured servers (opt-in design)
        let config = self.config.read().await;
        for (id, server_config) in &config.servers {
            // Try to find builtin template
            let template = BuiltinServer::find_by_id(id);

            // Resolve command: user config > template
            let command = server_config
                .command
                .clone()
                .or_else(|| {
                    template.map(|t| {
                        t.commands[0]
                            .split_whitespace()
                            .next()
                            .unwrap_or("")
                            .to_string()
                    })
                })
                .unwrap_or_default();

            // Resolve extensions: user config > template
            let extensions = if !server_config.file_extensions.is_empty() {
                server_config.file_extensions.clone()
            } else if let Some(tmpl) = template {
                tmpl.extensions.iter().map(ToString::to_string).collect()
            } else {
                Vec::new()
            };

            // Resolve install hint: template > generic
            let install_hint = template
                .map(|t| t.install_hint.to_string())
                .unwrap_or_else(|| format!("Install '{command}' manually"));

            // Check if command exists
            let program = command.split_whitespace().next().unwrap_or("");
            let installed = command_exists(program).await;

            let status = self
                .determine_server_status(
                    server_config.disabled,
                    command.is_empty() || !installed,
                    id,
                )
                .await;

            statuses.push(ServerStatusInfo {
                id: id.clone(),
                extensions,
                status,
                install_hint,
            });
        }

        statuses
    }

    /// Get status of ALL builtin servers (for installation UI)
    ///
    /// Unlike `get_all_servers_status()`, this includes ALL builtin servers
    /// regardless of whether they're in lsp_servers.json. Use this for
    /// the InstallServer UI to show all available servers for installation.
    pub async fn get_all_builtin_servers_status(&self) -> Vec<ServerStatusInfo> {
        let mut statuses = Vec::new();
        let config = self.config.read().await;

        for builtin in BUILTIN_SERVERS {
            // Check if command exists
            let command = builtin
                .commands
                .first()
                .and_then(|c| c.split_whitespace().next())
                .unwrap_or("");
            let installed = command_exists(command).await;

            // Check if already configured (and possibly running)
            let is_disabled = config
                .servers
                .get(builtin.id)
                .map(|c| c.disabled)
                .unwrap_or(false);

            let status = self
                .determine_server_status(is_disabled, !installed, builtin.id)
                .await;

            statuses.push(ServerStatusInfo {
                id: builtin.id.to_string(),
                extensions: builtin.extensions.iter().map(ToString::to_string).collect(),
                status,
                install_hint: builtin.install_hint.to_string(),
            });
        }

        statuses
    }

    /// Get count of active (running) LSP servers
    pub async fn active_server_count(&self) -> i32 {
        let lifecycles = self.lifecycles.lock().await;
        let mut count = 0;
        for lifecycle in lifecycles.values() {
            let health = lifecycle.health().await;
            if matches!(health, ServerHealth::Healthy | ServerHealth::Starting) {
                count += 1;
            }
        }
        count
    }

    /// Determine the status of a server based on its state
    async fn determine_server_status(
        &self,
        disabled: bool,
        not_installed: bool,
        server_id: &str,
    ) -> ServerStatus {
        if disabled {
            ServerStatus::Disabled
        } else if not_installed {
            ServerStatus::NotInstalled
        } else if self.is_server_running(server_id).await {
            ServerStatus::Running
        } else {
            ServerStatus::Idle
        }
    }

    /// Check if a specific server is currently running
    async fn is_server_running(&self, server_id: &str) -> bool {
        let lifecycles = self.lifecycles.lock().await;
        for ((id, _), lifecycle) in lifecycles.iter() {
            if id == server_id {
                let health = lifecycle.health().await;
                if matches!(health, ServerHealth::Healthy | ServerHealth::Starting) {
                    return true;
                }
            }
        }
        false
    }

    /// Get all servers for Configure Servers UI
    ///
    /// Returns a merged list of:
    /// - All installed builtin servers (even if not configured)
    /// - All configured servers (even if not installed)
    ///
    /// This enables the user to:
    /// - Add installed servers to config
    /// - Toggle/remove configured servers
    pub async fn get_all_servers_for_config(
        &self,
        user_config_dir: &Path,
        project_config_dir: &Path,
    ) -> Vec<ServerConfigInfo> {
        let mut servers = HashMap::new();
        let config = self.config.read().await;

        // 1. Add all builtin servers
        for builtin in BUILTIN_SERVERS {
            let command = builtin
                .commands
                .first()
                .and_then(|c| c.split_whitespace().next())
                .unwrap_or("");
            let installed = command_exists(command).await;

            // Detect config level
            let config_level = LspServersConfig::detect_config_level(
                builtin.id,
                user_config_dir,
                project_config_dir,
            );

            // Check if disabled in config
            let is_disabled = config
                .servers
                .get(builtin.id)
                .map(|c| c.disabled)
                .unwrap_or(false);

            let status = self
                .determine_server_status(is_disabled, !installed, builtin.id)
                .await;

            servers.insert(
                builtin.id.to_string(),
                ServerConfigInfo {
                    id: builtin.id.to_string(),
                    extensions: builtin.extensions.iter().map(ToString::to_string).collect(),
                    binary_installed: installed,
                    config_level,
                    status,
                    install_hint: builtin.install_hint.to_string(),
                },
            );
        }

        // 2. Add custom servers from config (not in builtins)
        for (id, server_config) in &config.servers {
            if servers.contains_key(id) {
                continue; // Already added from builtins
            }

            let command = server_config.command.clone().unwrap_or_default();
            let program = command.split_whitespace().next().unwrap_or("");
            let installed = !command.is_empty() && command_exists(program).await;

            let config_level =
                LspServersConfig::detect_config_level(id, user_config_dir, project_config_dir);

            let status = self
                .determine_server_status(server_config.disabled, !installed, id)
                .await;

            servers.insert(
                id.clone(),
                ServerConfigInfo {
                    id: id.clone(),
                    extensions: server_config.file_extensions.clone(),
                    binary_installed: installed,
                    config_level,
                    status,
                    install_hint: format!("Install '{command}' manually"),
                },
            );
        }

        // Sort by id for consistent display
        let mut result: Vec<_> = servers.into_values().collect();
        result.sort_by(|a, b| a.id.cmp(&b.id));
        result
    }
}

/// Status of an LSP server
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServerStatus {
    /// Server is running and healthy
    Running,
    /// Server is installed but not currently running
    Idle,
    /// Server command not found in PATH
    NotInstalled,
    /// Server is disabled in config
    Disabled,
}

impl std::fmt::Display for ServerStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ServerStatus::Running => write!(f, "Running"),
            ServerStatus::Idle => write!(f, "Idle"),
            ServerStatus::NotInstalled => write!(f, "Not Installed"),
            ServerStatus::Disabled => write!(f, "Disabled"),
        }
    }
}

/// Information about an LSP server's status
#[derive(Debug, Clone)]
pub struct ServerStatusInfo {
    /// Server identifier
    pub id: String,
    /// File extensions this server handles
    pub extensions: Vec<String>,
    /// Current status
    pub status: ServerStatus,
    /// Installation hint
    pub install_hint: String,
}

/// Information about an LSP server for configuration UI
#[derive(Debug, Clone)]
pub struct ServerConfigInfo {
    /// Server identifier
    pub id: String,
    /// File extensions this server handles
    pub extensions: Vec<String>,
    /// Whether the binary is installed
    pub binary_installed: bool,
    /// Config level (None = not configured)
    pub config_level: Option<ConfigLevel>,
    /// Server status
    pub status: ServerStatus,
    /// Installation hint
    pub install_hint: String,
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
    async fn test_find_server_for_extension_opt_in() {
        // With no config, no servers should be available (opt-in design)
        let empty_config = LspServersConfig::default();
        let diagnostics = Arc::new(DiagnosticsStore::new());
        let manager = LspServerManager::new(empty_config, None, diagnostics);

        // No config = no servers
        assert!(manager.find_server_for_extension(".rs").await.is_err());
        assert!(manager.find_server_for_extension(".go").await.is_err());
        assert!(manager.find_server_for_extension(".txt").await.is_err());

        // With config, servers should be available
        let mut config = LspServersConfig::default();
        config.servers.insert(
            "rust-analyzer".to_string(),
            LspServerConfig::default(), // Uses builtin template
        );
        config.servers.insert(
            "gopls".to_string(),
            LspServerConfig::default(), // Uses builtin template
        );

        let diagnostics = Arc::new(DiagnosticsStore::new());
        let manager = LspServerManager::new(config, None, diagnostics);

        assert!(manager.find_server_for_extension(".rs").await.is_ok());
        assert!(manager.find_server_for_extension(".go").await.is_ok());
        assert!(manager.find_server_for_extension(".txt").await.is_err());
    }

    #[tokio::test]
    async fn test_find_server_disabled() {
        let mut config = LspServersConfig::default();
        // Add rust-analyzer (disabled) and gopls (enabled)
        config.servers.insert(
            "rust-analyzer".to_string(),
            LspServerConfig {
                disabled: true,
                ..Default::default()
            },
        );
        config.servers.insert(
            "gopls".to_string(),
            LspServerConfig::default(), // Uses builtin template
        );

        let diagnostics = Arc::new(DiagnosticsStore::new());
        let manager = LspServerManager::new(config, None, diagnostics);

        // rust-analyzer is disabled
        assert!(manager.find_server_for_extension(".rs").await.is_err());
        // gopls is enabled
        assert!(manager.find_server_for_extension(".go").await.is_ok());
    }

    #[test]
    fn test_find_project_root() {
        let config = LspServersConfig::default();
        let diagnostics = Arc::new(DiagnosticsStore::new());
        let manager = LspServerManager::new(config, None, diagnostics);

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
        let manager = LspServerManager::new(config, None, diagnostics);

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
        let manager = LspServerManager::new(config, None, diagnostics);

        // Custom extension should be found
        let server_info = manager.find_server_for_extension(".ts").await.unwrap();
        assert_eq!(server_info.id, "typescript-lsp");

        let server_info = manager.find_server_for_extension(".tsx").await.unwrap();
        assert_eq!(server_info.id, "typescript-lsp");
    }

    #[tokio::test]
    async fn test_all_supported_extensions() {
        let mut config = LspServersConfig::default();

        // Add builtin reference (uses template for extensions)
        config
            .servers
            .insert("rust-analyzer".to_string(), LspServerConfig::default());

        // Add custom server with explicit extensions
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
        let manager = LspServerManager::new(config, None, diagnostics);

        let exts = manager.all_supported_extensions().await;
        // Only configured servers should be included
        assert!(exts.contains(&".rs".to_string())); // From rust-analyzer builtin template
        assert!(exts.contains(&".ts".to_string())); // From custom typescript-lsp
        // These are NOT included (not configured)
        assert!(!exts.contains(&".go".to_string()));
        assert!(!exts.contains(&".py".to_string()));
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
        let manager = LspServerManager::new(config, None, diagnostics);

        let server_info = manager.find_server_for_extension(".rs").await.unwrap();
        assert_eq!(server_info.lifecycle_config.max_restarts, 5);
        assert!(!server_info.lifecycle_config.restart_on_crash);
        assert_eq!(server_info.lifecycle_config.startup_timeout_ms, 20_000);
    }
}

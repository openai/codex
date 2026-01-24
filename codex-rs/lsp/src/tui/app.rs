//! Application state machine for LSP Test TUI.

use super::event::Event;
use super::ops;
use anyhow::Result;
use codex_lsp::ConfigLevel;
use codex_lsp::DiagnosticEntry;
use codex_lsp::DiagnosticsStore;
use codex_lsp::InstallEvent;
use codex_lsp::Location;
use codex_lsp::LspClient;
use codex_lsp::LspInstaller;
use codex_lsp::LspServerManager;
use codex_lsp::ResolvedSymbol;
use codex_lsp::ServerConfigInfo;
use codex_lsp::ServerStatus;
use codex_lsp::ServerStatusInfo;
use codex_lsp::SymbolKind;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyModifiers;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::info;

/// Helper to get user-level codex directory (respects CODEX_HOME).
fn get_codex_home() -> PathBuf {
    crate::config::find_codex_home().unwrap_or_else(|| PathBuf::from(".codex"))
}

/// Application modes
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Mode {
    /// Main menu - select LSP operation
    #[default]
    Menu,
    /// Entering file path
    FileInput,
    /// Entering symbol name
    SymbolInput,
    /// Displaying results
    Results,
    /// Showing diagnostics
    Diagnostics,
    /// Showing LSP servers status (for Install Binaries)
    Servers,
    /// Configure LSP servers (add/remove/disable)
    ConfigServers,
    /// Selecting config level (user vs project) before adding to config
    ConfigLevelSelect,
    /// Installing a server (shows progress)
    Installing,
    /// Showing help
    Help,
}

impl Mode {
    pub fn display_name(&self) -> &'static str {
        match self {
            Mode::Menu => "Menu",
            Mode::FileInput => "File Input",
            Mode::SymbolInput => "Symbol Input",
            Mode::Results => "Results",
            Mode::Diagnostics => "Diagnostics",
            Mode::Servers => "Servers",
            Mode::ConfigServers => "Config Servers",
            Mode::ConfigLevelSelect => "Config Level",
            Mode::Installing => "Installing",
            Mode::Help => "Help",
        }
    }
}

/// LSP operations available in the TUI
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Operation {
    Definition,
    TypeDefinition,
    Declaration,
    References,
    Implementation,
    Hover,
    WorkspaceSymbol,
    DocumentSymbols,
    CallHierarchy,
    HealthCheck,
    InstallBinaries,
    ConfigureServers,
}

impl Operation {
    pub fn all() -> &'static [Operation] {
        &[
            Operation::Definition,
            Operation::TypeDefinition,
            Operation::Declaration,
            Operation::References,
            Operation::Implementation,
            Operation::Hover,
            Operation::WorkspaceSymbol,
            Operation::DocumentSymbols,
            Operation::CallHierarchy,
            Operation::HealthCheck,
            Operation::InstallBinaries,
            Operation::ConfigureServers,
        ]
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            Operation::Definition => "Go to Definition",
            Operation::TypeDefinition => "Type Definition",
            Operation::Declaration => "Go to Declaration",
            Operation::References => "Find References",
            Operation::Implementation => "Find Implementations",
            Operation::Hover => "Hover Info",
            Operation::WorkspaceSymbol => "Workspace Symbol",
            Operation::DocumentSymbols => "Document Symbols",
            Operation::CallHierarchy => "Call Hierarchy",
            Operation::HealthCheck => "Health Check",
            Operation::InstallBinaries => "Install Binaries",
            Operation::ConfigureServers => "Configure Servers",
        }
    }

    pub fn needs_file(&self) -> bool {
        !matches!(
            self,
            Operation::WorkspaceSymbol
                | Operation::HealthCheck
                | Operation::InstallBinaries
                | Operation::ConfigureServers
        )
    }

    pub fn needs_symbol(&self) -> bool {
        !matches!(
            self,
            Operation::DocumentSymbols
                | Operation::HealthCheck
                | Operation::InstallBinaries
                | Operation::ConfigureServers
        )
    }
}

/// Input state for text fields
#[derive(Debug, Default, Clone)]
pub struct InputState {
    pub text: String,
    pub cursor: usize,
}

impl InputState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, c: char) {
        self.text.insert(self.cursor, c);
        self.cursor += 1;
    }

    pub fn backspace(&mut self) {
        if self.cursor > 0 {
            self.cursor -= 1;
            self.text.remove(self.cursor);
        }
    }

    pub fn delete(&mut self) {
        if self.cursor < self.text.len() {
            self.text.remove(self.cursor);
        }
    }

    pub fn move_left(&mut self) {
        if self.cursor > 0 {
            self.cursor -= 1;
        }
    }

    pub fn move_right(&mut self) {
        if self.cursor < self.text.len() {
            self.cursor += 1;
        }
    }

    pub fn home(&mut self) {
        self.cursor = 0;
    }

    pub fn end(&mut self) {
        self.cursor = self.text.len();
    }

    pub fn clear(&mut self) {
        self.text.clear();
        self.cursor = 0;
    }

    /// Kill text from cursor to beginning of line (Ctrl+U)
    pub fn kill_line_before(&mut self) {
        self.text = self.text[self.cursor..].to_string();
        self.cursor = 0;
    }

    /// Kill text from cursor to end of line (Ctrl+K)
    pub fn kill_line_after(&mut self) {
        self.text.truncate(self.cursor);
    }

    /// Move cursor to the beginning of the previous word (Ctrl+Left)
    pub fn move_word_left(&mut self) {
        if self.cursor == 0 {
            return;
        }
        // Skip any whitespace before cursor
        while self.cursor > 0 && self.text[..self.cursor].ends_with(char::is_whitespace) {
            self.cursor -= 1;
        }
        // Move to beginning of word
        while self.cursor > 0 && !self.text[..self.cursor].ends_with(char::is_whitespace) {
            self.cursor -= 1;
        }
    }

    /// Move cursor to the end of the next word (Ctrl+Right)
    pub fn move_word_right(&mut self) {
        let len = self.text.len();
        if self.cursor >= len {
            return;
        }
        // Skip any whitespace after cursor
        while self.cursor < len && self.text[self.cursor..].starts_with(char::is_whitespace) {
            self.cursor += 1;
        }
        // Move to end of word
        while self.cursor < len && !self.text[self.cursor..].starts_with(char::is_whitespace) {
            self.cursor += 1;
        }
    }
}

/// Call hierarchy result for display
#[derive(Debug)]
pub struct CallHierarchyResult {
    /// The prepared call hierarchy items (symbols)
    pub items: Vec<String>,
    /// Incoming calls (from → target)
    pub incoming: Vec<String>,
    /// Outgoing calls (target → to)
    pub outgoing: Vec<String>,
}

/// Structured error with operation context for TUI display and LLM/API callers
#[derive(Debug, Clone)]
pub struct LspErrorContext {
    /// The operation that was being executed
    pub operation: String,
    /// The file path involved (if any)
    pub file: Option<String>,
    /// The symbol being queried (if any)
    pub symbol: Option<String>,
    /// The error message
    pub error: String,
}

impl std::fmt::Display for LspErrorContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}] {}", self.operation, self.error)?;
        if let Some(ref file) = self.file {
            write!(f, " (file: {})", file)?;
        }
        if let Some(ref symbol) = self.symbol {
            write!(f, " (symbol: {})", symbol)?;
        }
        Ok(())
    }
}

/// Result data from LSP operations
#[derive(Debug)]
pub enum LspResult {
    Locations(Vec<Location>),
    HoverInfo(Option<String>),
    Symbols(Vec<ResolvedSymbol>),
    WorkspaceSymbols(Vec<codex_lsp::SymbolInformation>),
    CallHierarchy(CallHierarchyResult),
    ServerList(Vec<ServerStatusInfo>),
    HealthOk(String),
    Error(LspErrorContext),
}

/// Main application state
pub struct App {
    /// Workspace root directory
    pub workspace: PathBuf,
    /// Current mode
    pub mode: Mode,
    /// Selected operation
    pub operation: Option<Operation>,
    /// Menu selection index
    pub menu_index: usize,
    /// Current file path
    pub current_file: Option<PathBuf>,
    /// File input state
    pub file_input: InputState,
    /// Symbol input state
    pub symbol_input: InputState,
    /// Symbol kind filter (optional)
    pub symbol_kind: Option<SymbolKind>,
    /// LSP server manager
    pub manager: Arc<LspServerManager>,
    /// Diagnostics store
    pub diagnostics: Arc<DiagnosticsStore>,
    /// Current LSP client (cached)
    pub client: Option<Arc<LspClient>>,
    /// Last operation result
    pub result: Option<LspResult>,
    /// Result scroll offset
    pub result_scroll: usize,
    /// Diagnostics scroll offset
    pub diag_scroll: usize,
    /// Cached diagnostics (fetched when entering diagnostics mode)
    pub cached_diagnostics: Vec<DiagnosticEntry>,
    /// Cached server status list (for Install Binaries)
    pub cached_servers: Vec<ServerStatusInfo>,
    /// Servers scroll offset
    pub servers_scroll: usize,
    /// Currently selected server index in Servers mode
    pub selected_server: usize,
    /// Cached server config list (for Configure Servers)
    pub cached_config_servers: Vec<ServerConfigInfo>,
    /// Selected server index in ConfigServers mode
    pub selected_config_server: usize,
    /// Config scroll offset
    pub config_servers_scroll: usize,
    /// Whether config has been changed (need restart)
    pub config_changed: bool,
    /// Installation output lines
    pub install_output: Vec<String>,
    /// Currently installing server ID
    pub installing_server: Option<String>,
    /// Server ID pending config level selection before install
    pub pending_install_server: Option<String>,
    /// Selected config level (0 = user, 1 = project)
    pub config_level_selection: usize,
    /// Status message
    pub status_message: Option<String>,
    /// Should quit
    pub should_quit: bool,
    /// Loading indicator
    pub loading: bool,
}

impl App {
    pub fn new(
        workspace: PathBuf,
        manager: Arc<LspServerManager>,
        diagnostics: Arc<DiagnosticsStore>,
    ) -> Self {
        Self {
            workspace,
            mode: Mode::Menu,
            operation: None,
            menu_index: 0,
            current_file: None,
            file_input: InputState::new(),
            symbol_input: InputState::new(),
            symbol_kind: None,
            manager,
            diagnostics,
            client: None,
            result: None,
            result_scroll: 0,
            diag_scroll: 0,
            cached_diagnostics: Vec::new(),
            cached_servers: Vec::new(),
            servers_scroll: 0,
            selected_server: 0,
            cached_config_servers: Vec::new(),
            selected_config_server: 0,
            config_servers_scroll: 0,
            config_changed: false,
            install_output: Vec::new(),
            installing_server: None,
            pending_install_server: None,
            config_level_selection: 0,
            status_message: None,
            should_quit: false,
            loading: false,
        }
    }

    pub fn set_file(&mut self, file: PathBuf) {
        self.current_file = Some(file.clone());
        self.file_input.text = file.display().to_string();
        self.file_input.cursor = self.file_input.text.len();
    }

    pub async fn handle_event(&mut self, event: Event, tx: mpsc::Sender<Event>) -> Result<()> {
        match event {
            Event::Key(key) => self.handle_key(key, tx).await?,
            Event::Tick => {}
            Event::Resize(_, _) => {}
            Event::LspResult(result) => {
                self.result = Some(result);
                self.result_scroll = 0;
                self.mode = Mode::Results;
                self.loading = false;
            }
            Event::InstallProgress(install_event) => {
                self.handle_install_progress(install_event).await;
            }
        }
        Ok(())
    }

    /// Handle installation progress events
    async fn handle_install_progress(&mut self, event: InstallEvent) {
        match event {
            InstallEvent::Started { server_id, method } => {
                self.install_output.push(format!(
                    "Starting installation of {} using {}...",
                    server_id, method
                ));
            }
            InstallEvent::Output(line) => {
                self.install_output.push(line);
            }
            InstallEvent::Completed { server_id } => {
                self.install_output
                    .push(format!("Successfully installed {}!", server_id));
                self.install_output.push(String::new());
                self.install_output.push(
                    "Binary installed. Use 'Configure Servers' to add to config.".to_string(),
                );
                self.status_message = Some(format!("{} binary installed!", server_id));
                self.loading = false;

                // Refresh server list to show new installation status
                self.cached_servers = self.manager.get_all_builtin_servers_status().await;
            }
            InstallEvent::Failed { server_id, error } => {
                self.install_output
                    .push(format!("ERROR: Failed to install {}: {}", server_id, error));
                self.status_message = Some(format!("Failed to install {}", server_id));
                self.loading = false;
            }
        }
    }

    async fn handle_key(&mut self, key: KeyEvent, tx: mpsc::Sender<Event>) -> Result<()> {
        // Global quit: Ctrl+C or q in menu mode
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            self.should_quit = true;
            return Ok(());
        }

        match self.mode {
            Mode::Menu => self.handle_menu_key(key, tx).await?,
            Mode::FileInput => self.handle_file_input_key(key, tx).await?,
            Mode::SymbolInput => self.handle_symbol_input_key(key, tx).await?,
            Mode::Results => self.handle_results_key(key)?,
            Mode::Diagnostics => self.handle_diagnostics_key(key)?,
            Mode::Servers => self.handle_servers_key(key, tx).await?,
            Mode::ConfigServers => self.handle_config_servers_key(key).await?,
            Mode::ConfigLevelSelect => self.handle_config_level_select_key(key, tx).await?,
            Mode::Installing => self.handle_installing_key(key)?,
            Mode::Help => self.handle_help_key(key)?,
        }
        Ok(())
    }

    async fn handle_menu_key(&mut self, key: KeyEvent, tx: mpsc::Sender<Event>) -> Result<()> {
        match key.code {
            KeyCode::Char('q') => {
                self.should_quit = true;
            }
            KeyCode::Char('d') => {
                // Fetch diagnostics before entering diagnostics mode
                self.cached_diagnostics = self.diagnostics.get_all().await;
                self.diag_scroll = 0;
                self.mode = Mode::Diagnostics;
            }
            KeyCode::Char('s') => {
                // Fetch server status before entering servers mode
                self.cached_servers = self.manager.get_all_servers_status().await;
                self.servers_scroll = 0;
                self.mode = Mode::Servers;
            }
            KeyCode::Char('?') | KeyCode::Char('h') => {
                self.mode = Mode::Help;
            }
            KeyCode::Up => {
                if self.menu_index > 0 {
                    self.menu_index -= 1;
                }
            }
            KeyCode::Down => {
                let max = Operation::all().len().saturating_sub(1);
                if self.menu_index < max {
                    self.menu_index += 1;
                }
            }
            KeyCode::Enter => {
                let op = Operation::all()[self.menu_index];
                self.operation = Some(op);

                if op.needs_file() {
                    self.mode = Mode::FileInput;
                    // Pre-fill with current file if available
                    if let Some(ref file) = self.current_file {
                        self.file_input.text = file.display().to_string();
                        self.file_input.cursor = self.file_input.text.len();
                    }
                } else if op.needs_symbol() {
                    self.mode = Mode::SymbolInput;
                } else if matches!(op, Operation::InstallBinaries) {
                    // Go to Servers view showing ALL builtin servers (not just configured ones)
                    self.cached_servers = self.manager.get_all_builtin_servers_status().await;
                    self.servers_scroll = 0;
                    self.mode = Mode::Servers;
                    // Auto-select first NotInstalled server
                    self.selected_server = self
                        .cached_servers
                        .iter()
                        .position(|s| matches!(s.status, ServerStatus::NotInstalled))
                        .unwrap_or(0);
                } else if matches!(op, Operation::ConfigureServers) {
                    // Go to ConfigServers view
                    let user_dir = get_codex_home();
                    let project_dir = self.workspace.join(".codex");
                    self.cached_config_servers = self
                        .manager
                        .get_all_servers_for_config(&user_dir, &project_dir)
                        .await;
                    self.config_servers_scroll = 0;
                    self.selected_config_server = 0;
                    self.mode = Mode::ConfigServers;
                } else {
                    // Execute immediately (HealthCheck)
                    self.execute_operation(tx).await?;
                }
            }
            KeyCode::Char(c) if c.is_ascii_digit() => {
                let digit = c.to_digit(10).unwrap_or(0) as usize;
                // 0 means 10th operation, 1-9 mean 1st-9th operation
                let idx = if digit == 0 { 10 } else { digit };
                if idx > 0 && idx <= Operation::all().len() {
                    self.menu_index = idx - 1;
                }
            }
            _ => {}
        }
        Ok(())
    }

    async fn handle_file_input_key(
        &mut self,
        key: KeyEvent,
        tx: mpsc::Sender<Event>,
    ) -> Result<()> {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        match key.code {
            KeyCode::Esc => {
                self.mode = Mode::Menu;
                self.file_input.clear();
            }
            KeyCode::Enter => {
                let file_path = PathBuf::from(&self.file_input.text);
                let full_path = if file_path.is_absolute() {
                    file_path
                } else {
                    self.workspace.join(file_path)
                };
                self.current_file = Some(full_path);

                if let Some(op) = self.operation {
                    if op.needs_symbol() {
                        self.mode = Mode::SymbolInput;
                    } else {
                        // DocumentSymbols doesn't need symbol input, execute immediately
                        self.execute_operation(tx).await?;
                    }
                }
            }
            KeyCode::Backspace => self.file_input.backspace(),
            KeyCode::Delete => self.file_input.delete(),
            KeyCode::Left if ctrl => self.file_input.move_word_left(),
            KeyCode::Left => self.file_input.move_left(),
            KeyCode::Right if ctrl => self.file_input.move_word_right(),
            KeyCode::Right => self.file_input.move_right(),
            KeyCode::Home => self.file_input.home(),
            KeyCode::End => self.file_input.end(),
            KeyCode::Char('u') if ctrl => self.file_input.kill_line_before(),
            KeyCode::Char('k') if ctrl => self.file_input.kill_line_after(),
            KeyCode::Char(c) => self.file_input.insert(c),
            _ => {}
        }
        Ok(())
    }

    async fn handle_symbol_input_key(
        &mut self,
        key: KeyEvent,
        tx: mpsc::Sender<Event>,
    ) -> Result<()> {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        match key.code {
            KeyCode::Esc => {
                self.mode = Mode::Menu;
                self.symbol_input.clear();
            }
            KeyCode::Enter => {
                self.execute_operation(tx).await?;
            }
            KeyCode::Backspace => self.symbol_input.backspace(),
            KeyCode::Delete => self.symbol_input.delete(),
            KeyCode::Left if ctrl => self.symbol_input.move_word_left(),
            KeyCode::Left => self.symbol_input.move_left(),
            KeyCode::Right if ctrl => self.symbol_input.move_word_right(),
            KeyCode::Right => self.symbol_input.move_right(),
            KeyCode::Home => self.symbol_input.home(),
            KeyCode::End => self.symbol_input.end(),
            KeyCode::Char('u') if ctrl => self.symbol_input.kill_line_before(),
            KeyCode::Char('k') if ctrl => self.symbol_input.kill_line_after(),
            KeyCode::Char(c) => self.symbol_input.insert(c),
            _ => {}
        }
        Ok(())
    }

    fn handle_results_key(&mut self, key: KeyEvent) -> Result<()> {
        let max_scroll = self.result_line_count().saturating_sub(1);
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => {
                self.mode = Mode::Menu;
                self.result = None;
            }
            KeyCode::Up => {
                self.result_scroll = self.result_scroll.saturating_sub(1);
            }
            KeyCode::Down => {
                if self.result_scroll < max_scroll {
                    self.result_scroll += 1;
                }
            }
            KeyCode::PageUp => {
                self.result_scroll = self.result_scroll.saturating_sub(10);
            }
            KeyCode::PageDown => {
                self.result_scroll = (self.result_scroll + 10).min(max_scroll);
            }
            KeyCode::Home => {
                self.result_scroll = 0;
            }
            KeyCode::End => {
                self.result_scroll = max_scroll;
            }
            _ => {}
        }
        Ok(())
    }

    fn handle_diagnostics_key(&mut self, key: KeyEvent) -> Result<()> {
        let max_scroll = self.diagnostics_line_count().saturating_sub(1);
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => {
                self.mode = Mode::Menu;
            }
            KeyCode::Up => {
                self.diag_scroll = self.diag_scroll.saturating_sub(1);
            }
            KeyCode::Down => {
                if self.diag_scroll < max_scroll {
                    self.diag_scroll += 1;
                }
            }
            KeyCode::PageUp => {
                self.diag_scroll = self.diag_scroll.saturating_sub(10);
            }
            KeyCode::PageDown => {
                self.diag_scroll = (self.diag_scroll + 10).min(max_scroll);
            }
            KeyCode::Home => {
                self.diag_scroll = 0;
            }
            KeyCode::End => {
                self.diag_scroll = max_scroll;
            }
            _ => {}
        }
        Ok(())
    }

    fn handle_help_key(&mut self, key: KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('?') | KeyCode::Char('h') => {
                self.mode = Mode::Menu;
            }
            _ => {}
        }
        Ok(())
    }

    async fn handle_servers_key(&mut self, key: KeyEvent, tx: mpsc::Sender<Event>) -> Result<()> {
        let server_count = self.cached_servers.len();
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => {
                self.mode = Mode::Menu;
            }
            KeyCode::Char('r') => {
                // Refresh server status
                self.cached_servers = self.manager.get_all_servers_status().await;
                // Ensure selected_server is still valid
                if self.selected_server >= server_count && server_count > 0 {
                    self.selected_server = server_count - 1;
                }
            }
            KeyCode::Enter | KeyCode::Char('i') => {
                // Install selected server binary if it's not installed
                if let Some(server) = self.cached_servers.get(self.selected_server) {
                    if matches!(server.status, ServerStatus::NotInstalled) {
                        // Directly install binary (no config modification)
                        self.start_installation(server.id.clone(), tx).await?;
                    } else {
                        self.status_message =
                            Some(format!("Server '{}' is already installed", server.id));
                    }
                }
            }
            KeyCode::Up => {
                if self.selected_server > 0 {
                    self.selected_server -= 1;
                    // Adjust scroll if selected is above visible area
                    if self.selected_server < self.servers_scroll {
                        self.servers_scroll = self.selected_server;
                    }
                }
            }
            KeyCode::Down => {
                if server_count > 0 && self.selected_server < server_count - 1 {
                    self.selected_server += 1;
                }
            }
            KeyCode::PageUp => {
                self.selected_server = self.selected_server.saturating_sub(10);
                self.servers_scroll = self.servers_scroll.saturating_sub(10);
            }
            KeyCode::PageDown => {
                if server_count > 0 {
                    self.selected_server = (self.selected_server + 10).min(server_count - 1);
                }
            }
            KeyCode::Home => {
                self.selected_server = 0;
                self.servers_scroll = 0;
            }
            KeyCode::End => {
                if server_count > 0 {
                    self.selected_server = server_count - 1;
                }
            }
            _ => {}
        }
        Ok(())
    }

    /// Handle key events in ConfigServers mode
    /// Keys: a = add to config, d = disable/enable, x = remove from config
    async fn handle_config_servers_key(&mut self, key: KeyEvent) -> Result<()> {
        use codex_lsp::config::LspServersConfig;

        let server_count = self.cached_config_servers.len();

        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => {
                // Check if config changed and show message
                if self.config_changed {
                    self.status_message = Some("Config changed. Restart TUI to apply.".to_string());
                    self.config_changed = false;
                }
                self.mode = Mode::Menu;
            }
            KeyCode::Char('r') => {
                // Refresh server list (reload config from disk first)
                self.manager.reload_config().await;
                let user_dir = get_codex_home();
                let project_dir = self.workspace.join(".codex");
                self.cached_config_servers = self
                    .manager
                    .get_all_servers_for_config(&user_dir, &project_dir)
                    .await;
                // Ensure selected_config_server is still valid
                if self.selected_config_server >= server_count && server_count > 0 {
                    self.selected_config_server = server_count - 1;
                }
            }
            KeyCode::Enter | KeyCode::Char('a') => {
                // Add to config - only if binary is installed and not yet configured
                if let Some(server) = self.cached_config_servers.get(self.selected_config_server) {
                    if server.binary_installed && server.config_level.is_none() {
                        // Go to config level selection
                        self.pending_install_server = Some(server.id.clone());
                        self.mode = Mode::ConfigLevelSelect;
                    } else if !server.binary_installed {
                        self.status_message = Some(format!(
                            "'{}' not installed. Use Install Binaries first.",
                            server.id
                        ));
                    } else {
                        self.status_message = Some(format!(
                            "'{}' already configured at {} level",
                            server.id,
                            server
                                .config_level
                                .as_ref()
                                .map(|l| l.to_string())
                                .unwrap_or_default()
                        ));
                    }
                }
            }
            KeyCode::Char('d') => {
                // Disable/Enable toggle - only for configured servers
                if let Some(server) = self.cached_config_servers.get(self.selected_config_server) {
                    if let Some(config_level) = &server.config_level {
                        let config_dir = match config_level {
                            ConfigLevel::User => Some(get_codex_home()),
                            ConfigLevel::Project => Some(self.workspace.join(".codex")),
                        };

                        if let Some(dir) = config_dir {
                            match LspServersConfig::toggle_server_disabled(&dir, &server.id) {
                                Ok(Some(new_disabled)) => {
                                    let state = if new_disabled { "disabled" } else { "enabled" };
                                    self.status_message = Some(format!(
                                        "'{}' is now {}. Restart to apply.",
                                        server.id, state
                                    ));
                                    self.config_changed = true;
                                    // Reload config and refresh list
                                    self.manager.reload_config().await;
                                    let user_dir = get_codex_home();
                                    let project_dir = self.workspace.join(".codex");
                                    self.cached_config_servers = self
                                        .manager
                                        .get_all_servers_for_config(&user_dir, &project_dir)
                                        .await;
                                }
                                Ok(None) => {
                                    self.status_message =
                                        Some(format!("'{}' not found in config", server.id));
                                }
                                Err(e) => {
                                    self.status_message = Some(format!("Failed to toggle: {e}"));
                                }
                            }
                        }
                    } else {
                        self.status_message =
                            Some(format!("'{}' is not configured. Add it first.", server.id));
                    }
                }
            }
            KeyCode::Char('x') => {
                // Remove from config - only for configured servers
                if let Some(server) = self.cached_config_servers.get(self.selected_config_server) {
                    if let Some(config_level) = &server.config_level {
                        let config_dir = match config_level {
                            ConfigLevel::User => Some(get_codex_home()),
                            ConfigLevel::Project => Some(self.workspace.join(".codex")),
                        };

                        if let Some(dir) = config_dir {
                            match LspServersConfig::remove_server_from_file(&dir, &server.id) {
                                Ok(true) => {
                                    self.status_message = Some(format!(
                                        "'{}' removed from config. Restart to apply.",
                                        server.id
                                    ));
                                    self.config_changed = true;
                                    // Reload config and refresh list
                                    self.manager.reload_config().await;
                                    let user_dir = get_codex_home();
                                    let project_dir = self.workspace.join(".codex");
                                    self.cached_config_servers = self
                                        .manager
                                        .get_all_servers_for_config(&user_dir, &project_dir)
                                        .await;
                                }
                                Ok(false) => {
                                    self.status_message =
                                        Some(format!("'{}' not found in config", server.id));
                                }
                                Err(e) => {
                                    self.status_message = Some(format!("Failed to remove: {e}"));
                                }
                            }
                        }
                    } else {
                        self.status_message = Some(format!("'{}' is not configured", server.id));
                    }
                }
            }
            KeyCode::Up => {
                if self.selected_config_server > 0 {
                    self.selected_config_server -= 1;
                    if self.selected_config_server < self.config_servers_scroll {
                        self.config_servers_scroll = self.selected_config_server;
                    }
                }
            }
            KeyCode::Down => {
                if server_count > 0 && self.selected_config_server < server_count - 1 {
                    self.selected_config_server += 1;
                }
            }
            KeyCode::PageUp => {
                self.selected_config_server = self.selected_config_server.saturating_sub(10);
                self.config_servers_scroll = self.config_servers_scroll.saturating_sub(10);
            }
            KeyCode::PageDown => {
                if server_count > 0 {
                    self.selected_config_server =
                        (self.selected_config_server + 10).min(server_count - 1);
                }
            }
            KeyCode::Home => {
                self.selected_config_server = 0;
                self.config_servers_scroll = 0;
            }
            KeyCode::End => {
                if server_count > 0 {
                    self.selected_config_server = server_count - 1;
                }
            }
            _ => {}
        }
        Ok(())
    }

    /// Handle key events in ConfigLevelSelect mode
    /// This is used by ConfigureServers when adding a server to config
    async fn handle_config_level_select_key(
        &mut self,
        key: KeyEvent,
        _tx: mpsc::Sender<Event>,
    ) -> Result<()> {
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => {
                // Cancel and go back to ConfigServers
                self.pending_install_server = None;
                self.mode = Mode::ConfigServers;
            }
            KeyCode::Up | KeyCode::Char('1') => {
                self.config_level_selection = 0; // User level
            }
            KeyCode::Down | KeyCode::Char('2') => {
                self.config_level_selection = 1; // Project level
            }
            KeyCode::Enter => {
                // Add server to config
                if let Some(server_id) = self.pending_install_server.take() {
                    self.add_server_to_config(&server_id)?;
                    self.config_changed = true;
                    // Reload config and refresh list
                    self.manager.reload_config().await;
                    let user_dir = get_codex_home();
                    let project_dir = self.workspace.join(".codex");
                    self.cached_config_servers = self
                        .manager
                        .get_all_servers_for_config(&user_dir, &project_dir)
                        .await;
                    self.mode = Mode::ConfigServers;
                }
            }
            _ => {}
        }
        Ok(())
    }

    /// Add a server to the config file at the selected level
    fn add_server_to_config(&mut self, server_id: &str) -> Result<()> {
        use codex_lsp::config::LspServersConfig;

        let config_dir = if self.config_level_selection == 0 {
            Some(get_codex_home())
        } else {
            Some(self.workspace.join(".codex"))
        };

        if let Some(dir) = config_dir {
            if let Err(e) = LspServersConfig::add_server_to_file(&dir, server_id) {
                self.status_message = Some(format!("Failed to add server: {e}"));
            } else {
                self.status_message = Some(format!(
                    "{} added to config. Restart to activate.",
                    server_id
                ));
            }
        }
        Ok(())
    }

    /// Handle key events in Installing mode
    fn handle_installing_key(&mut self, key: KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => {
                // Return to Servers mode
                self.mode = Mode::Servers;
                self.installing_server = None;
                // Don't clear install_output - keep it for reference
            }
            _ => {}
        }
        Ok(())
    }

    /// Start installation of a server binary (no config modification)
    async fn start_installation(
        &mut self,
        server_id: String,
        tx: mpsc::Sender<Event>,
    ) -> Result<()> {
        info!(server = %server_id, "Starting server binary installation");

        self.mode = Mode::Installing;
        self.installing_server = Some(server_id.clone());
        self.install_output.clear();
        self.install_output
            .push("Installing binary only (no config change)...".to_string());
        self.loading = true;

        // Create channel for progress events
        let (progress_tx, mut progress_rx) = mpsc::channel::<InstallEvent>(100);

        // Spawn installer task
        let server_id_clone = server_id.clone();
        tokio::spawn(async move {
            let installer = LspInstaller::new(Some(progress_tx));
            let _ = installer.install_server(&server_id_clone).await;
        });

        // Spawn progress forwarding task
        let event_tx = tx;
        tokio::spawn(async move {
            while let Some(event) = progress_rx.recv().await {
                if event_tx.send(Event::InstallProgress(event)).await.is_err() {
                    break;
                }
            }
        });

        Ok(())
    }

    /// Calculate the number of lines in the current result for scroll bounds.
    pub fn result_line_count(&self) -> usize {
        match &self.result {
            Some(LspResult::Locations(locs)) => {
                if locs.is_empty() { 1 } else { locs.len() + 2 } // header + empty + items
            }
            Some(LspResult::HoverInfo(Some(content))) => content.lines().count() + 2,
            Some(LspResult::HoverInfo(None)) => 1,
            Some(LspResult::Symbols(syms)) => {
                if syms.is_empty() {
                    1
                } else {
                    syms.len() + 2
                }
            }
            Some(LspResult::WorkspaceSymbols(syms)) => {
                if syms.is_empty() {
                    1
                } else {
                    syms.len() + 2
                }
            }
            Some(LspResult::CallHierarchy(ch)) => {
                // header + empty + items + incoming section + outgoing section
                let items_lines = ch.items.len() + 3;
                let incoming_lines = if ch.incoming.is_empty() {
                    3
                } else {
                    ch.incoming.len() + 2
                };
                let outgoing_lines = if ch.outgoing.is_empty() {
                    3
                } else {
                    ch.outgoing.len() + 2
                };
                items_lines + incoming_lines + outgoing_lines
            }
            Some(LspResult::ServerList(servers)) => {
                if servers.is_empty() {
                    2
                } else {
                    servers.len() + 3
                }
            }
            Some(LspResult::HealthOk(_)) => 2,
            Some(LspResult::Error(_)) => 2,
            None => 1,
        }
    }

    /// Calculate the number of lines in diagnostics for scroll bounds.
    pub fn diagnostics_line_count(&self) -> usize {
        if self.cached_diagnostics.is_empty() {
            2 // empty message
        } else {
            self.cached_diagnostics.len() * 2 + 2 // header + empty + 2 lines per diagnostic
        }
    }

    async fn execute_operation(&mut self, tx: mpsc::Sender<Event>) -> Result<()> {
        let Some(operation) = self.operation else {
            return Ok(());
        };

        self.loading = true;
        self.status_message = Some("Executing...".to_string());

        // Clone data for async task
        let manager = self.manager.clone();
        let file = self.current_file.clone();
        let symbol = self.symbol_input.text.clone();
        let symbol_kind = self.symbol_kind;

        tokio::spawn(async move {
            let result =
                ops::execute_operation(operation, manager, file, symbol, symbol_kind).await;
            let _ = tx.send(Event::LspResult(result)).await;
        });

        Ok(())
    }
}

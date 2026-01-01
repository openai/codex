//! Application state machine for LSP Test TUI.

use super::event::Event;
use super::ops;
use anyhow::Result;
use codex_lsp::DiagnosticEntry;
use codex_lsp::DiagnosticsStore;
use codex_lsp::Location;
use codex_lsp::LspClient;
use codex_lsp::LspServerManager;
use codex_lsp::ResolvedSymbol;
use codex_lsp::SymbolKind;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyModifiers;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::mpsc;

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
        }
    }

    pub fn needs_file(&self) -> bool {
        !matches!(self, Operation::WorkspaceSymbol | Operation::HealthCheck)
    }

    pub fn needs_symbol(&self) -> bool {
        !matches!(self, Operation::DocumentSymbols | Operation::HealthCheck)
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

/// Result data from LSP operations
#[derive(Debug)]
pub enum LspResult {
    Locations(Vec<Location>),
    HoverInfo(Option<String>),
    Symbols(Vec<ResolvedSymbol>),
    WorkspaceSymbols(Vec<codex_lsp::SymbolInformation>),
    CallHierarchy(CallHierarchyResult),
    HealthOk(String),
    Error(String),
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
        }
        Ok(())
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

//! Main TUI application loop.
//!
//! This module provides the [`App`] struct which orchestrates the TUI,
//! managing the event loop, state updates, and rendering.

use std::io;
use std::path::PathBuf;

use cocode_protocol::LoopEvent;
use futures::StreamExt;
use tokio::sync::mpsc;

use crate::clipboard_paste;
use crate::command::UserCommand;
use crate::editor;
use crate::event::TuiCommand;
use crate::event::TuiEvent;
use crate::event::handle_key_event_full;
use crate::file_search::FileSearchEvent;
use crate::file_search::FileSearchManager;
use crate::file_search::create_file_search_channel;
use crate::paste::PasteManager;
use crate::render::render;
use crate::skill_search::SkillSearchManager;
use crate::state::AppState;
use crate::state::Overlay;
use crate::terminal::Tui;
use crate::update::handle_agent_event;
use crate::update::handle_command;
use crate::update::handle_file_search_event;

/// Configuration for the TUI application.
#[derive(Debug, Clone)]
pub struct AppConfig {
    /// Initial model to use.
    pub model: String,
    /// Available models for the model picker.
    pub available_models: Vec<String>,
    /// Working directory for file search.
    pub cwd: PathBuf,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            model: "claude-sonnet-4-20250514".to_string(),
            available_models: vec![
                "claude-sonnet-4-20250514".to_string(),
                "claude-opus-4-20250514".to_string(),
                "claude-haiku-3-5-20241022".to_string(),
            ],
            cwd: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
        }
    }
}

/// The main TUI application.
///
/// This struct manages the complete lifecycle of the TUI, including:
/// - Event handling (keyboard, mouse, agent events)
/// - State management
/// - Rendering
/// - Communication with the core agent
pub struct App {
    /// The TUI terminal manager.
    tui: Tui,
    /// Application state.
    state: AppState,
    /// Receiver for events from the core agent.
    agent_rx: mpsc::Receiver<LoopEvent>,
    /// Sender for commands to the core agent.
    command_tx: mpsc::Sender<UserCommand>,
    /// Available models for the picker.
    available_models: Vec<String>,
    /// File search manager for @mention autocomplete.
    file_search: FileSearchManager,
    /// Receiver for file search events.
    file_search_rx: mpsc::Receiver<FileSearchEvent>,
    /// Skill search manager for /command autocomplete.
    skill_search: SkillSearchManager,
    /// Paste manager for handling large pastes.
    paste_manager: PasteManager,
}

impl App {
    /// Create a new TUI application.
    ///
    /// # Arguments
    ///
    /// * `agent_rx` - Receiver for events from the core agent loop
    /// * `command_tx` - Sender for commands to the core agent
    /// * `config` - Application configuration
    ///
    /// # Errors
    ///
    /// Returns an error if terminal setup fails.
    pub fn new(
        agent_rx: mpsc::Receiver<LoopEvent>,
        command_tx: mpsc::Sender<UserCommand>,
        config: AppConfig,
    ) -> io::Result<Self> {
        // Initialize i18n before anything else
        crate::i18n::init();

        let tui = Tui::new()?;
        let state = AppState::with_model(&config.model);

        // Create file search manager
        let (file_search_tx, file_search_rx) = create_file_search_channel();
        let file_search = FileSearchManager::new(config.cwd, file_search_tx);

        // Create skill search manager
        let skill_search = SkillSearchManager::new();

        // Create paste manager
        let paste_manager = PasteManager::new();

        Ok(Self {
            tui,
            state,
            agent_rx,
            command_tx,
            available_models: config.available_models,
            file_search,
            file_search_rx,
            skill_search,
            paste_manager,
        })
    }

    /// Create a new TUI application with an existing terminal (for testing).
    #[cfg(test)]
    pub fn with_terminal(
        tui: Tui,
        agent_rx: mpsc::Receiver<LoopEvent>,
        command_tx: mpsc::Sender<UserCommand>,
    ) -> Self {
        let (file_search_tx, file_search_rx) = create_file_search_channel();
        let file_search = FileSearchManager::new(PathBuf::from("."), file_search_tx);
        let skill_search = SkillSearchManager::new();

        Self {
            tui,
            state: AppState::new(),
            agent_rx,
            command_tx,
            available_models: vec![],
            file_search,
            file_search_rx,
            skill_search,
            paste_manager: PasteManager::new(),
        }
    }

    /// Get a reference to the application state.
    pub fn state(&self) -> &AppState {
        &self.state
    }

    /// Get a mutable reference to the application state.
    pub fn state_mut(&mut self) -> &mut AppState {
        &mut self.state
    }

    /// Get the command sender for external use.
    pub fn command_tx(&self) -> mpsc::Sender<UserCommand> {
        self.command_tx.clone()
    }

    /// Run the main application loop.
    ///
    /// This method blocks until the application exits. It handles:
    /// - Terminal events (keyboard, mouse, resize)
    /// - Agent events from the core loop
    /// - File search events for autocomplete
    /// - Periodic tick events for animations
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Terminal I/O fails
    /// - Event stream terminates unexpectedly
    pub async fn run(&mut self) -> io::Result<()> {
        // Create event stream
        let mut event_stream = self.tui.event_stream();

        // Initial render
        self.render()?;

        // Trigger initial file index refresh
        self.file_search.refresh_index();

        loop {
            tokio::select! {
                // Handle TUI events (keyboard, tick, draw)
                Some(event) = event_stream.next() => {
                    self.handle_tui_event(event).await?;
                }
                // Handle agent events
                Some(loop_event) = self.agent_rx.recv() => {
                    self.handle_loop_event(loop_event);
                    self.render()?;
                }
                // Handle file search results
                Some(search_event) = self.file_search_rx.recv() => {
                    handle_file_search_event(&mut self.state, search_event);
                    self.render()?;
                }
            }

            // Check if we should exit
            if self.state.should_exit() {
                // Send shutdown command to core
                let _ = self.command_tx.send(UserCommand::Shutdown).await;
                break;
            }
        }

        Ok(())
    }

    /// Handle a TUI event.
    async fn handle_tui_event(&mut self, event: TuiEvent) -> io::Result<()> {
        match event {
            TuiEvent::Key(key) => {
                let has_overlay = self.state.has_overlay();
                let has_file_suggestions = self.state.ui.has_file_suggestions();
                let has_skill_suggestions = self.state.ui.has_skill_suggestions();

                if let Some(cmd) = handle_key_event_full(
                    key,
                    has_overlay,
                    has_file_suggestions,
                    has_skill_suggestions,
                    self.state.is_streaming(),
                ) {
                    self.handle_command_internal(cmd).await;
                }

                // Check for @mention or /command after input changes
                self.check_at_mention();
                self.check_slash_command();

                self.render()?;
            }
            TuiEvent::Mouse(_mouse) => {
                // Mouse events can be handled here if needed
            }
            TuiEvent::Resize { .. } => {
                self.render()?;
            }
            TuiEvent::FocusChanged { focused } => {
                self.state.ui.set_terminal_focused(focused);
                // Could pause animations here if needed
            }
            TuiEvent::Draw => {
                self.render()?;
            }
            TuiEvent::Tick => {
                // Tick events for animations
                let mut needs_render = false;

                // Re-render if streaming
                if self.state.is_streaming() {
                    needs_render = true;
                }

                // Expire old toasts
                if self.state.ui.has_toasts() {
                    self.state.ui.expire_toasts();
                    needs_render = true;
                }

                // Tick animation frame (for thinking animation, etc.)
                self.state.ui.tick_animation();

                if needs_render {
                    self.render()?;
                }
            }
            TuiEvent::Paste(text) => {
                // Terminal bracketed paste already gave us text — use it directly.
                // No need to probe the clipboard for images here (that's Ctrl+V's job).
                let processed = self.paste_manager.process_text(text);
                for c in processed.chars() {
                    self.state.ui.input.insert_char(c);
                }
                // Check for @mention or /command after paste
                self.check_at_mention();
                self.check_slash_command();
                self.render()?;
            }
            TuiEvent::Agent(loop_event) => {
                self.handle_loop_event(loop_event);
                self.render()?;
            }
            TuiEvent::Command(cmd) => {
                self.handle_command_internal(cmd).await;
                self.render()?;
            }
        }
        Ok(())
    }

    /// Check for @mention in input and trigger file search if needed.
    fn check_at_mention(&mut self) {
        if let Some((start_pos, query)) = self.state.ui.input.current_at_token() {
            if has_line_range_suffix(&query) {
                // User is typing a line range suffix — dismiss autocomplete
                self.state.ui.clear_file_suggestions();
                self.file_search.cancel();
                return;
            }
            // Start or update file suggestions
            self.state
                .ui
                .start_file_suggestions(query.clone(), start_pos);
            self.file_search.on_query(query, start_pos);
        } else {
            // No @mention, clear suggestions
            self.state.ui.clear_file_suggestions();
            self.file_search.cancel();
        }
    }

    /// Check for /command in input and trigger skill search if needed.
    fn check_slash_command(&mut self) {
        if let Some((start_pos, query)) = self.state.ui.input.current_slash_token() {
            // Start or update skill suggestions
            self.state
                .ui
                .start_skill_suggestions(query.clone(), start_pos);
            let suggestions = self.skill_search.search(&query);
            self.state.ui.update_skill_suggestions(suggestions);
        } else {
            // No /command, clear suggestions
            self.state.ui.clear_skill_suggestions();
        }
    }

    /// Load skills into the skill search manager.
    pub fn load_skills<'a>(
        &mut self,
        skills: impl Iterator<Item = &'a cocode_skill::SkillPromptCommand>,
    ) {
        self.skill_search.load_skills(skills);
    }

    /// Handle a TUI command internally.
    async fn handle_command_internal(&mut self, cmd: TuiCommand) {
        // Handle external editor specially - it needs terminal access
        if matches!(cmd, TuiCommand::OpenExternalEditor) {
            self.handle_external_editor();
            return;
        }

        // Handle clipboard paste specially - needs &mut paste_manager
        if matches!(cmd, TuiCommand::PasteFromClipboard) {
            self.handle_clipboard_paste();
            return;
        }

        handle_command(
            &mut self.state,
            cmd,
            &self.command_tx,
            &self.available_models,
            &self.paste_manager,
        )
        .await;
    }

    /// Handle opening an external editor.
    fn handle_external_editor(&mut self) {
        let current_input = self.state.ui.input.text().to_string();

        // Run the external editor (this blocks and suspends the TUI)
        match editor::edit_in_external_editor(&current_input) {
            Ok(result) => {
                if result.modified {
                    self.state.ui.input.set_text(result.content);
                    tracing::info!("Input updated from external editor");
                } else {
                    tracing::debug!("External editor: no changes made");
                }
            }
            Err(e) => {
                tracing::error!("Failed to open external editor: {e}");
                self.state
                    .ui
                    .set_overlay(Overlay::Error(format!("External editor failed: {e}")));
            }
        }
    }

    /// Clipboard paste triggered by Ctrl+V / Alt+V.
    ///
    /// Opens the clipboard once, tries image first, falls back to text.
    /// This is separate from `TuiEvent::Paste` which is terminal-provided text.
    fn handle_clipboard_paste(&mut self) {
        let cb = match clipboard_paste::open_clipboard() {
            Ok(cb) => cb,
            Err(_) => return,
        };

        // 1. Try clipboard image first (JPEG, PNG, GIF, WebP)
        match clipboard_paste::paste_image(cb) {
            Ok((data, media_type)) => {
                let pill = self.paste_manager.process_image(data, media_type);
                for c in pill.chars() {
                    self.state.ui.input.insert_char(c);
                }
                self.state
                    .ui
                    .toast_success(crate::i18n::t!("toast.image_pasted").to_string());
                return;
            }
            Err(_) => {
                // No image — try text below
            }
        }

        // 2. Fall back to text (need a fresh clipboard handle)
        let cb = match clipboard_paste::open_clipboard() {
            Ok(cb) => cb,
            Err(_) => return,
        };
        if let Ok(text) = clipboard_paste::paste_text(cb) {
            let processed = self.paste_manager.process_text(text);
            for c in processed.chars() {
                self.state.ui.input.insert_char(c);
            }
        }
    }

    /// Handle a loop event from the core agent.
    fn handle_loop_event(&mut self, event: LoopEvent) {
        handle_agent_event(&mut self.state, event);
    }

    /// Render the current state to the terminal.
    fn render(&mut self) -> io::Result<()> {
        self.tui.draw(|frame| {
            render(frame, &self.state);
        })
    }
}

/// Check if query ends with a line range suffix (e.g., ":10" or ":10-20").
fn has_line_range_suffix(query: &str) -> bool {
    if let Some(colon_pos) = query.rfind(':') {
        let after_colon = &query[colon_pos + 1..];
        !after_colon.is_empty()
            && after_colon
                .split('-')
                .all(|part| !part.is_empty() && part.chars().all(|c| c.is_ascii_digit()))
    } else {
        false
    }
}

/// Create a channel pair for TUI-agent communication.
///
/// Returns `(agent_tx, agent_rx, command_tx, command_rx)` where:
/// - `agent_tx`: Core sends LoopEvents to TUI
/// - `agent_rx`: TUI receives LoopEvents
/// - `command_tx`: TUI sends UserCommands to Core
/// - `command_rx`: Core receives UserCommands
pub fn create_channels(
    buffer_size: usize,
) -> (
    mpsc::Sender<LoopEvent>,
    mpsc::Receiver<LoopEvent>,
    mpsc::Sender<UserCommand>,
    mpsc::Receiver<UserCommand>,
) {
    let (agent_tx, agent_rx) = mpsc::channel(buffer_size);
    let (command_tx, command_rx) = mpsc::channel(buffer_size);
    (agent_tx, agent_rx, command_tx, command_rx)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_app_config_default() {
        let config = AppConfig::default();
        assert!(!config.model.is_empty());
        assert!(!config.available_models.is_empty());
    }

    #[test]
    fn test_has_line_range_suffix() {
        // Should detect line range suffixes
        assert!(has_line_range_suffix("file.rs:10"));
        assert!(has_line_range_suffix("file.rs:10-20"));
        assert!(has_line_range_suffix("src/main.rs:1"));
        assert!(has_line_range_suffix("src/main.rs:100-200"));

        // Should NOT detect non-line-range patterns
        assert!(!has_line_range_suffix("file.rs"));
        assert!(!has_line_range_suffix("file.rs:"));
        assert!(!has_line_range_suffix("file.rs:abc"));
        assert!(!has_line_range_suffix("file.rs:10-"));
        assert!(!has_line_range_suffix("file.rs:-20"));
        assert!(!has_line_range_suffix("file:name.rs"));
    }

    #[test]
    fn test_create_channels() {
        let (agent_tx, _agent_rx, command_tx, _command_rx) = create_channels(16);

        // Channels should be usable
        assert!(agent_tx.try_send(LoopEvent::StreamRequestStart).is_ok());
        assert!(
            command_tx
                .try_send(UserCommand::SubmitInput {
                    content: vec![hyper_sdk::ContentBlock::text("test")],
                    display_text: "test".to_string()
                })
                .is_ok()
        );
    }
}

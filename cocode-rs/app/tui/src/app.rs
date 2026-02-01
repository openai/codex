//! Main TUI application loop.
//!
//! This module provides the [`App`] struct which orchestrates the TUI,
//! managing the event loop, state updates, and rendering.

use std::io;

use cocode_protocol::LoopEvent;
use futures::StreamExt;
use tokio::sync::mpsc;

use crate::command::UserCommand;
use crate::event::TuiEvent;
use crate::event::handle_key_event;
use crate::render::render;
use crate::state::AppState;
use crate::terminal::Tui;
use crate::update::handle_agent_event;
use crate::update::handle_command;

/// Configuration for the TUI application.
#[derive(Debug, Clone)]
pub struct AppConfig {
    /// Initial model to use.
    pub model: String,
    /// Available models for the model picker.
    pub available_models: Vec<String>,
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
        let tui = Tui::new()?;
        let state = AppState::with_model(&config.model);

        Ok(Self {
            tui,
            state,
            agent_rx,
            command_tx,
            available_models: config.available_models,
        })
    }

    /// Create a new TUI application with an existing terminal (for testing).
    #[cfg(test)]
    pub fn with_terminal(
        tui: Tui,
        agent_rx: mpsc::Receiver<LoopEvent>,
        command_tx: mpsc::Sender<UserCommand>,
    ) -> Self {
        Self {
            tui,
            state: AppState::new(),
            agent_rx,
            command_tx,
            available_models: vec![],
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
                if let Some(cmd) = handle_key_event(key, self.state.has_overlay()) {
                    self.handle_command_internal(cmd).await;
                }
                self.render()?;
            }
            TuiEvent::Mouse(_mouse) => {
                // Mouse events can be handled here if needed
            }
            TuiEvent::Resize { .. } => {
                self.render()?;
            }
            TuiEvent::FocusChanged { .. } => {
                // Focus events can be handled here if needed
            }
            TuiEvent::Draw => {
                self.render()?;
            }
            TuiEvent::Tick => {
                // Tick events for animations - just re-render if streaming
                if self.state.is_streaming() {
                    self.render()?;
                }
            }
            TuiEvent::Paste(text) => {
                // Insert pasted text into input
                for c in text.chars() {
                    self.state.ui.input.insert_char(c);
                }
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

    /// Handle a TUI command internally.
    async fn handle_command_internal(&mut self, cmd: crate::event::TuiCommand) {
        handle_command(
            &mut self.state,
            cmd,
            &self.command_tx,
            &self.available_models,
        )
        .await;
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
    fn test_create_channels() {
        let (agent_tx, _agent_rx, command_tx, _command_rx) = create_channels(16);

        // Channels should be usable
        assert!(agent_tx.try_send(LoopEvent::StreamRequestStart).is_ok());
        assert!(
            command_tx
                .try_send(UserCommand::SubmitInput {
                    message: "test".to_string()
                })
                .is_ok()
        );
    }
}

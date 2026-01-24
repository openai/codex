//! Plugin command dispatch helpers for ChatWidget.
//!
//! This module extracts plugin-specific async dispatch logic from chatwidget.rs
//! to minimize upstream merge conflicts per tui/CLAUDE.md guidelines.
//! Includes impl ChatWidget for dispatch_plugin_command.

use crate::app_event::AppEvent;
use crate::app_event_sender::AppEventSender;
use crate::chatwidget::ChatWidget;
use crate::history_cell;
use std::path::PathBuf;

/// Handle /plugin command with no arguments (show help).
pub(crate) fn spawn_plugin_help(
    tx: AppEventSender,
    codex_home: PathBuf,
    project_path: Option<PathBuf>,
) {
    tokio::spawn(async move {
        use crate::slash_command_ext::PluginCommandResult;
        use crate::slash_command_ext::PluginManagerContext;
        use crate::slash_command_ext::handle_plugin_command;

        let ctx = PluginManagerContext::new(codex_home, project_path);
        let result = handle_plugin_command("help", &ctx).await;
        let text = match result {
            PluginCommandResult::Help(help) => help,
            _ => "Plugin help not available".to_string(),
        };
        tx.send(AppEvent::PluginResult(text));
    });
}

/// Handle /plugin command with arguments.
pub(crate) fn spawn_plugin_command(
    tx: AppEventSender,
    codex_home: PathBuf,
    project_path: Option<PathBuf>,
    args: String,
) {
    tokio::spawn(async move {
        use crate::slash_command_ext::PluginCommandResult;
        use crate::slash_command_ext::PluginManagerContext;
        use crate::slash_command_ext::format_plugin_list;
        use crate::slash_command_ext::handle_plugin_command;

        let ctx = PluginManagerContext::new(codex_home, project_path);
        let result = handle_plugin_command(&args, &ctx).await;
        let text = match result {
            PluginCommandResult::Success(msg) => msg,
            PluginCommandResult::List(entries) => format_plugin_list(&entries),
            PluginCommandResult::Help(help) => help,
            PluginCommandResult::Error(err) => format!("Error: {err}"),
        };
        tx.send(AppEvent::PluginResult(text));
    });
}

/// Expand and dispatch a plugin command (e.g., /my-plugin:review).
pub(crate) fn spawn_plugin_command_expansion(tx: AppEventSender, name: String, args: String) {
    tokio::spawn(async move {
        match crate::plugin_commands::expand_plugin_command(&name, &args).await {
            Some(expanded) => {
                tx.send(AppEvent::PluginCommandExpanded(expanded));
            }
            None => {
                let text = format!("Plugin command '/{name}' not found or failed to load.");
                tx.send(AppEvent::PluginResult(text));
            }
        }
    });
}

// =============================================================================
// impl ChatWidget - Plugin command dispatcher moved from chatwidget.rs
// =============================================================================

impl ChatWidget {
    /// Handle plugin commands loaded event.
    /// Moved from chatwidget.rs to minimize upstream merge conflicts.
    pub(crate) fn on_plugin_commands_loaded(
        &mut self,
        commands: Vec<crate::plugin_commands::PluginCommandEntry>,
    ) {
        tracing::debug!("received {} plugin commands", commands.len());
        self.set_plugin_commands(commands);
    }

    /// Dispatch a plugin command (e.g., /my-plugin arg1 arg2).
    /// Moved from chatwidget.rs to minimize upstream merge conflicts.
    pub(crate) fn dispatch_plugin_command(&mut self, name: String, args: String) {
        if self.is_task_running() {
            let message =
                format!("Plugin command '/{name}' is disabled while a task is in progress.");
            self.add_to_history(history_cell::new_error_event(message));
            self.request_redraw();
            return;
        }
        spawn_plugin_command_expansion(self.app_event_tx().clone(), name, args);
    }
}

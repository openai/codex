//! Plugin command support for CommandPopup.
//!
//! Separated to minimize upstream merge conflicts when syncing with upstream.

use super::command_popup::CommandItem;
use super::command_popup::CommandPopup;
use crate::plugin_commands::PluginCommandEntry;
use codex_common::fuzzy_match::fuzzy_match;
use std::collections::HashSet;

// ============================================================================
// Impl block - methods that need &mut self / &self
// ============================================================================

impl CommandPopup {
    /// Set plugin commands. Should be called after loading plugin commands.
    pub(crate) fn set_plugin_commands(&mut self, mut commands: Vec<PluginCommandEntry>) {
        // Exclude commands that collide with builtin names
        let exclude: HashSet<String> = self
            .builtins
            .iter()
            .map(|(n, _)| (*n).to_string())
            .collect();
        commands.retain(|c| !exclude.contains(&c.name));
        commands.sort_by(|a, b| a.name.cmp(&b.name));
        self.plugin_commands = commands;
    }
}

// ============================================================================
// Helper functions - pure functions, parameter passing, no field access needed
// ============================================================================

/// List all plugin commands without filtering (for empty filter case).
pub(super) fn list_all_plugin_commands(
    plugin_commands: &[PluginCommandEntry],
) -> Vec<(CommandItem, Option<Vec<usize>>, i32)> {
    plugin_commands
        .iter()
        .map(|cmd| (CommandItem::PluginCommand(cmd.name.clone()), None, 0))
        .collect()
}

/// Filter plugin commands by fuzzy matching.
pub(super) fn filter_plugin_commands(
    plugin_commands: &[PluginCommandEntry],
    filter: &str,
) -> Vec<(CommandItem, Option<Vec<usize>>, i32)> {
    plugin_commands
        .iter()
        .filter_map(|cmd| {
            fuzzy_match(&cmd.name, filter).map(|(indices, score)| {
                (
                    CommandItem::PluginCommand(cmd.name.clone()),
                    Some(indices),
                    score,
                )
            })
        })
        .collect()
}

/// Get name and description for a plugin command (used in rows_from_matches).
pub(super) fn plugin_command_name_and_description(
    cmd_name: &str,
    plugin_commands: &[PluginCommandEntry],
) -> (String, String) {
    let description = plugin_commands
        .iter()
        .find(|c| c.name == cmd_name)
        .map(|c| c.description.clone())
        .unwrap_or_else(|| "plugin command".to_string());
    (format!("/{cmd_name}"), description)
}

//! Shared helpers for filtering and matching built-in slash commands.
//!
//! The same sandbox- and feature-gating rules are used by both the composer
//! and the command popup. Centralizing them here keeps those call sites small
//! and ensures they stay in sync.
use codex_common::fuzzy_match::fuzzy_match;

use crate::slash_command::SlashCommand;
use crate::slash_command::built_in_slash_commands;

/// Whether the Windows degraded-sandbox elevation flow is currently allowed.
pub(crate) fn windows_degraded_sandbox_active() -> bool {
    cfg!(target_os = "windows")
        && codex_core::windows_sandbox::ELEVATED_SANDBOX_NUX_ENABLED
        && codex_core::get_platform_sandbox().is_some()
        && !codex_core::is_windows_elevated_sandbox_enabled()
}

/// Return the built-ins that should be visible/usable for the current input.
pub(crate) fn builtins_for_input(
    collaboration_modes_enabled: bool,
    personality_command_enabled: bool,
) -> Vec<(&'static str, SlashCommand)> {
    let allow_elevate_sandbox = windows_degraded_sandbox_active();
    built_in_slash_commands()
        .into_iter()
        .filter(|(_, cmd)| allow_elevate_sandbox || *cmd != SlashCommand::ElevateSandbox)
        .filter(|(_, cmd)| collaboration_modes_enabled || *cmd != SlashCommand::Collab)
        .filter(|(_, cmd)| personality_command_enabled || *cmd != SlashCommand::Personality)
        .collect()
}

/// Find a single built-in command by exact name, after applying the gating rules.
pub(crate) fn find_builtin_command(
    name: &str,
    collaboration_modes_enabled: bool,
    personality_command_enabled: bool,
) -> Option<SlashCommand> {
    builtins_for_input(collaboration_modes_enabled, personality_command_enabled)
        .into_iter()
        .find(|(command_name, _)| *command_name == name)
        .map(|(_, cmd)| cmd)
}

/// Whether any visible built-in fuzzily matches the provided prefix.
pub(crate) fn has_builtin_prefix(
    name: &str,
    collaboration_modes_enabled: bool,
    personality_command_enabled: bool,
) -> bool {
    builtins_for_input(collaboration_modes_enabled, personality_command_enabled)
        .into_iter()
        .any(|(command_name, _)| fuzzy_match(command_name, name).is_some())
}

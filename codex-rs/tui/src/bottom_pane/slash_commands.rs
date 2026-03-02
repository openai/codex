//! Shared helpers for filtering and matching built-in slash commands.
//!
//! The same sandbox- and feature-gating rules are used by both the composer
//! and the command popup. Centralizing them here keeps those call sites small
//! and ensures they stay in sync.
use codex_utils_fuzzy_match::fuzzy_match;

use crate::slash_command::SlashCommand;
use crate::slash_command::built_in_slash_commands;

#[derive(Clone, Copy, Debug)]
pub(crate) struct SlashCommandFilters {
    pub(crate) collaboration_modes_enabled: bool,
    pub(crate) connectors_enabled: bool,
    pub(crate) personality_command_enabled: bool,
    pub(crate) realtime_conversation_enabled: bool,
    pub(crate) audio_device_selection_enabled: bool,
    pub(crate) review_loop_command_enabled: bool,
    pub(crate) allow_elevate_sandbox: bool,
}

/// Return the built-ins that should be visible/usable for the current input.
pub(crate) fn builtins_for_input(
    filters: SlashCommandFilters,
) -> Vec<(&'static str, SlashCommand)> {
    built_in_slash_commands()
        .into_iter()
        .filter(|(_, cmd)| filters.allow_elevate_sandbox || *cmd != SlashCommand::ElevateSandbox)
        .filter(|(_, cmd)| {
            filters.collaboration_modes_enabled
                || !matches!(*cmd, SlashCommand::Collab | SlashCommand::Plan)
        })
        .filter(|(_, cmd)| filters.connectors_enabled || *cmd != SlashCommand::Apps)
        .filter(|(_, cmd)| filters.personality_command_enabled || *cmd != SlashCommand::Personality)
        .filter(|(_, cmd)| filters.realtime_conversation_enabled || *cmd != SlashCommand::Realtime)
        .filter(|(_, cmd)| filters.audio_device_selection_enabled || *cmd != SlashCommand::Settings)
        .filter(|(_, cmd)| filters.review_loop_command_enabled || *cmd != SlashCommand::ReviewLoop)
        .collect()
}

/// Find a single built-in command by exact name, after applying the gating rules.
pub(crate) fn find_builtin_command(
    name: &str,
    filters: SlashCommandFilters,
) -> Option<SlashCommand> {
    builtins_for_input(filters)
        .into_iter()
        .find(|(command_name, _)| *command_name == name)
        .map(|(_, cmd)| cmd)
}

/// Whether any visible built-in fuzzily matches the provided prefix.
pub(crate) fn has_builtin_prefix(name: &str, filters: SlashCommandFilters) -> bool {
    builtins_for_input(filters)
        .into_iter()
        .any(|(command_name, _)| fuzzy_match(command_name, name).is_some())
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn debug_command_still_resolves_for_dispatch() {
        let flags = SlashCommandFilters {
            collaboration_modes_enabled: true,
            connectors_enabled: true,
            personality_command_enabled: true,
            realtime_conversation_enabled: false,
            audio_device_selection_enabled: false,
            review_loop_command_enabled: false,
            allow_elevate_sandbox: false,
        };
        let cmd = find_builtin_command("debug-config", flags);
        assert_eq!(cmd, Some(SlashCommand::DebugConfig));
    }

    #[test]
    fn clear_command_resolves_for_dispatch() {
        let flags = SlashCommandFilters {
            collaboration_modes_enabled: true,
            connectors_enabled: true,
            personality_command_enabled: true,
            realtime_conversation_enabled: false,
            audio_device_selection_enabled: false,
            review_loop_command_enabled: false,
            allow_elevate_sandbox: false,
        };
        assert_eq!(
            find_builtin_command("clear", flags),
            Some(SlashCommand::Clear)
        );
    }

    #[test]
    fn realtime_command_is_hidden_when_realtime_is_disabled() {
        let flags = SlashCommandFilters {
            collaboration_modes_enabled: true,
            connectors_enabled: true,
            personality_command_enabled: true,
            realtime_conversation_enabled: false,
            audio_device_selection_enabled: true,
            review_loop_command_enabled: false,
            allow_elevate_sandbox: false,
        };
        assert_eq!(find_builtin_command("realtime", flags), None);
    }

    #[test]
    fn settings_command_is_hidden_when_realtime_is_disabled() {
        let flags = SlashCommandFilters {
            collaboration_modes_enabled: true,
            connectors_enabled: true,
            personality_command_enabled: true,
            realtime_conversation_enabled: false,
            audio_device_selection_enabled: false,
            review_loop_command_enabled: false,
            allow_elevate_sandbox: false,
        };
        assert_eq!(find_builtin_command("settings", flags), None);
    }

    #[test]
    fn settings_command_is_hidden_when_audio_device_selection_is_disabled() {
        let flags = SlashCommandFilters {
            collaboration_modes_enabled: true,
            connectors_enabled: true,
            personality_command_enabled: true,
            realtime_conversation_enabled: true,
            audio_device_selection_enabled: false,
            review_loop_command_enabled: false,
            allow_elevate_sandbox: false,
        };
        assert_eq!(find_builtin_command("settings", flags), None);
    }

    #[test]
    fn review_loop_command_is_hidden_when_disabled() {
        let flags = SlashCommandFilters {
            collaboration_modes_enabled: true,
            connectors_enabled: true,
            personality_command_enabled: true,
            realtime_conversation_enabled: false,
            audio_device_selection_enabled: false,
            review_loop_command_enabled: false,
            allow_elevate_sandbox: false,
        };
        assert_eq!(find_builtin_command("review-loop", flags), None);
    }

    #[test]
    fn review_loop_command_is_visible_when_enabled() {
        let flags = SlashCommandFilters {
            collaboration_modes_enabled: true,
            connectors_enabled: true,
            personality_command_enabled: true,
            realtime_conversation_enabled: false,
            audio_device_selection_enabled: false,
            review_loop_command_enabled: true,
            allow_elevate_sandbox: false,
        };
        assert_eq!(
            find_builtin_command("review-loop", flags),
            Some(SlashCommand::ReviewLoop)
        );
    }
}

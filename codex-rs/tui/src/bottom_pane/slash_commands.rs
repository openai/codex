//! Shared helpers for filtering and matching built-in and model service-tier slash commands.
//!
//! The same sandbox- and feature-gating rules are used by both the composer
//! and the command popup. Centralizing them here keeps those call sites small
//! and ensures they stay in sync.
use std::str::FromStr;

use codex_utils_fuzzy_match::fuzzy_match;

use crate::slash_command::SlashCommand;
use crate::slash_command::built_in_slash_commands;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ServiceTierCommand {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) description: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum SlashCommandItem {
    Builtin(SlashCommand),
    ServiceTier(ServiceTierCommand),
}

impl SlashCommandItem {
    pub(crate) fn command(&self) -> &str {
        match self {
            Self::Builtin(cmd) => cmd.command(),
            Self::ServiceTier(command) => &command.name,
        }
    }

    pub(crate) fn supports_inline_args(&self) -> bool {
        match self {
            Self::Builtin(cmd) => cmd.supports_inline_args(),
            Self::ServiceTier(_) => false,
        }
    }

    pub(crate) fn available_in_side_conversation(&self) -> bool {
        match self {
            Self::Builtin(cmd) => cmd.available_in_side_conversation(),
            Self::ServiceTier(_) => false,
        }
    }

    pub(crate) fn available_during_task(&self) -> bool {
        match self {
            Self::Builtin(cmd) => cmd.available_during_task(),
            Self::ServiceTier(_) => false,
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct BuiltinCommandFlags {
    pub(crate) collaboration_modes_enabled: bool,
    pub(crate) connectors_enabled: bool,
    pub(crate) plugins_command_enabled: bool,
    pub(crate) service_tier_commands_enabled: bool,
    pub(crate) goal_command_enabled: bool,
    pub(crate) personality_command_enabled: bool,
    pub(crate) realtime_conversation_enabled: bool,
    pub(crate) audio_device_selection_enabled: bool,
    pub(crate) allow_elevate_sandbox: bool,
    pub(crate) side_conversation_active: bool,
}

/// Return the built-ins that should be visible/usable for the current input.
pub(crate) fn builtins_for_input(flags: BuiltinCommandFlags) -> Vec<(&'static str, SlashCommand)> {
    built_in_slash_commands()
        .into_iter()
        .filter(|(_, cmd)| flags.allow_elevate_sandbox || *cmd != SlashCommand::ElevateSandbox)
        .filter(|(_, cmd)| {
            flags.collaboration_modes_enabled
                || !matches!(*cmd, SlashCommand::Collab | SlashCommand::Plan)
        })
        .filter(|(_, cmd)| flags.connectors_enabled || *cmd != SlashCommand::Apps)
        .filter(|(_, cmd)| flags.plugins_command_enabled || *cmd != SlashCommand::Plugins)
        .filter(|(_, cmd)| flags.goal_command_enabled || *cmd != SlashCommand::Goal)
        .filter(|(_, cmd)| flags.personality_command_enabled || *cmd != SlashCommand::Personality)
        .filter(|(_, cmd)| flags.realtime_conversation_enabled || *cmd != SlashCommand::Realtime)
        .filter(|(_, cmd)| flags.audio_device_selection_enabled || *cmd != SlashCommand::Settings)
        .filter(|(_, cmd)| !flags.side_conversation_active || cmd.available_in_side_conversation())
        .collect()
}

pub(crate) fn commands_for_input(
    flags: BuiltinCommandFlags,
    service_tier_commands: &[ServiceTierCommand],
) -> Vec<SlashCommandItem> {
    let mut commands = Vec::new();
    let tiers_enabled = flags.service_tier_commands_enabled;
    for (_, cmd) in builtins_for_input(flags) {
        commands.push(SlashCommandItem::Builtin(cmd));
        if cmd == SlashCommand::Model && tiers_enabled {
            commands.extend(
                service_tier_commands
                    .iter()
                    .cloned()
                    .map(SlashCommandItem::ServiceTier),
            );
        }
    }
    commands
        .into_iter()
        .filter(|cmd| !flags.side_conversation_active || cmd.available_in_side_conversation())
        .collect()
}

/// Find a single built-in command by exact name, after applying feature gating.
///
/// Side-conversation gating is intentionally enforced by dispatch rather than exact lookup so a
/// typed command can produce a side-specific unavailable message while the popup still hides it.
pub(crate) fn find_builtin_command(name: &str, flags: BuiltinCommandFlags) -> Option<SlashCommand> {
    let cmd = SlashCommand::from_str(name).ok()?;
    builtins_for_input(BuiltinCommandFlags {
        side_conversation_active: false,
        ..flags
    })
    .into_iter()
    .any(|(_, visible_cmd)| visible_cmd == cmd)
    .then_some(cmd)
}

pub(crate) fn find_slash_command(
    name: &str,
    flags: BuiltinCommandFlags,
    service_tier_commands: &[ServiceTierCommand],
) -> Option<SlashCommandItem> {
    if let Some(cmd) = find_builtin_command(name, flags) {
        return Some(SlashCommandItem::Builtin(cmd));
    }

    let tiers_enabled = flags.service_tier_commands_enabled;
    tiers_enabled
        .then(|| {
            service_tier_commands
                .iter()
                .find(|command| command.name == name)
                .cloned()
                .map(SlashCommandItem::ServiceTier)
        })
        .flatten()
}

pub(crate) fn has_slash_command_prefix(
    name: &str,
    flags: BuiltinCommandFlags,
    service_tier_commands: &[ServiceTierCommand],
) -> bool {
    commands_for_input(flags, service_tier_commands)
        .into_iter()
        .any(|command| fuzzy_match(command.command(), name).is_some())
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use std::slice::from_ref;

    fn all_enabled_flags() -> BuiltinCommandFlags {
        BuiltinCommandFlags {
            collaboration_modes_enabled: true,
            connectors_enabled: true,
            plugins_command_enabled: true,
            service_tier_commands_enabled: true,
            goal_command_enabled: true,
            personality_command_enabled: true,
            realtime_conversation_enabled: true,
            audio_device_selection_enabled: true,
            allow_elevate_sandbox: true,
            side_conversation_active: false,
        }
    }

    #[test]
    fn debug_command_still_resolves_for_dispatch() {
        let cmd = find_builtin_command("debug-config", all_enabled_flags());
        assert_eq!(cmd, Some(SlashCommand::DebugConfig));
    }

    #[test]
    fn clear_command_resolves_for_dispatch() {
        assert_eq!(
            find_builtin_command("clear", all_enabled_flags()),
            Some(SlashCommand::Clear)
        );
    }

    #[test]
    fn stop_command_resolves_for_dispatch() {
        assert_eq!(
            find_builtin_command("stop", all_enabled_flags()),
            Some(SlashCommand::Stop)
        );
    }

    #[test]
    fn clean_command_alias_resolves_for_dispatch() {
        assert_eq!(
            find_builtin_command("clean", all_enabled_flags()),
            Some(SlashCommand::Stop)
        );
    }

    #[test]
    fn service_tier_commands_are_hidden_when_disabled() {
        let mut flags = all_enabled_flags();
        flags.service_tier_commands_enabled = false;
        let commands = vec![ServiceTierCommand {
            id: "priority".to_string(),
            name: "fast".to_string(),
            description: "fastest inference".to_string(),
        }];

        assert_eq!(find_slash_command("fast", flags, &commands), None);
    }

    #[test]
    fn all_service_tiers_are_exposed_as_commands_after_model() {
        let commands = vec![
            ServiceTierCommand {
                id: "priority".to_string(),
                name: "fast".to_string(),
                description: "fastest inference".to_string(),
            },
            ServiceTierCommand {
                id: "batch".to_string(),
                name: "slow".to_string(),
                description: "slower inference with lower priority".to_string(),
            },
        ];

        let items = commands_for_input(all_enabled_flags(), &commands);
        let model_idx = items
            .iter()
            .position(|item| matches!(item, SlashCommandItem::Builtin(SlashCommand::Model)))
            .expect("model command should be visible");
        let inserted = items
            .into_iter()
            .skip(model_idx + 1)
            .take(commands.len())
            .collect::<Vec<_>>();
        let expected = commands
            .into_iter()
            .map(SlashCommandItem::ServiceTier)
            .collect::<Vec<_>>();

        assert_eq!(inserted, expected);
    }

    #[test]
    fn goal_command_is_hidden_when_disabled() {
        let mut flags = all_enabled_flags();
        flags.goal_command_enabled = false;
        assert_eq!(find_builtin_command("goal", flags), None);
    }

    #[test]
    fn realtime_command_is_hidden_when_realtime_is_disabled() {
        let mut flags = all_enabled_flags();
        flags.realtime_conversation_enabled = false;
        assert_eq!(find_builtin_command("realtime", flags), None);
    }

    #[test]
    fn settings_command_is_hidden_when_realtime_is_disabled() {
        let mut flags = all_enabled_flags();
        flags.realtime_conversation_enabled = false;
        flags.audio_device_selection_enabled = false;
        assert_eq!(find_builtin_command("settings", flags), None);
    }

    #[test]
    fn settings_command_is_hidden_when_audio_device_selection_is_disabled() {
        let mut flags = all_enabled_flags();
        flags.audio_device_selection_enabled = false;
        assert_eq!(find_builtin_command("settings", flags), None);
    }

    #[test]
    fn side_conversation_hides_commands_without_side_flag() {
        let commands = builtins_for_input(BuiltinCommandFlags {
            side_conversation_active: true,
            ..all_enabled_flags()
        })
        .into_iter()
        .map(|(_, command)| command)
        .collect::<Vec<_>>();

        assert_eq!(
            commands,
            vec![
                SlashCommand::Ide,
                SlashCommand::Copy,
                SlashCommand::Raw,
                SlashCommand::Diff,
                SlashCommand::Mention,
                SlashCommand::Status,
            ]
        );
    }

    #[test]
    fn side_conversation_exact_lookup_still_resolves_hidden_commands_for_dispatch_error() {
        assert_eq!(
            find_builtin_command(
                "review",
                BuiltinCommandFlags {
                    side_conversation_active: true,
                    ..all_enabled_flags()
                },
            ),
            Some(SlashCommand::Review)
        );
    }

    #[test]
    fn side_conversation_exact_lookup_still_resolves_service_tier_commands_for_dispatch_error() {
        let command = ServiceTierCommand {
            id: "priority".to_string(),
            name: "fast".to_string(),
            description: "fastest inference".to_string(),
        };
        let flags = BuiltinCommandFlags {
            side_conversation_active: true,
            ..all_enabled_flags()
        };

        assert_eq!(
            find_slash_command("fast", flags, from_ref(&command)),
            Some(SlashCommandItem::ServiceTier(command))
        );
    }
}

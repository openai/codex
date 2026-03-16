use strum::IntoEnumIterator;
use strum_macros::AsRefStr;
use strum_macros::EnumIter;
use strum_macros::EnumString;
use strum_macros::IntoStaticStr;

/// Commands that can be invoked by starting a message with a leading slash.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, EnumString, EnumIter, AsRefStr, IntoStaticStr,
)]
#[strum(serialize_all = "kebab-case")]
pub enum SlashCommand {
    // DO NOT ALPHA-SORT! Enum order is presentation order in the popup, so
    // more frequently used commands should be listed first.
    Model,
    Fast,
    Approvals,
    Permissions,
    #[strum(serialize = "setup-default-sandbox")]
    ElevateSandbox,
    #[strum(serialize = "sandbox-add-read-dir")]
    SandboxReadRoot,
    Experimental,
    Skills,
    Review,
    Rename,
    New,
    Resume,
    Fork,
    Init,
    Compact,
    Plan,
    Collab,
    Agent,
    // Undo,
    Diff,
    Copy,
    Mention,
    Status,
    DebugConfig,
    Statusline,
    Theme,
    Mcp,
    Apps,
    Logout,
    Quit,
    Exit,
    Feedback,
    Rollout,
    Ps,
    #[strum(to_string = "stop", serialize = "clean")]
    Stop,
    Clear,
    Personality,
    Realtime,
    Settings,
    TestApproval,
    #[strum(serialize = "subagents")]
    MultiAgents,
    // Debugging commands.
    #[strum(serialize = "debug-m-drop")]
    MemoryDrop,
    #[strum(serialize = "debug-m-update")]
    MemoryUpdate,
}

impl SlashCommand {
    fn spec(self) -> SlashCommandSpec {
        match self {
            SlashCommand::Model => SlashCommandSpec {
                description: "choose what model and reasoning effort to use",
                supports_inline_args: false,
                available_during_task: false,
                is_visible: true,
                show_in_command_popup: true,
            },
            SlashCommand::Fast => SlashCommandSpec {
                description: "toggle Fast mode to enable fastest inference at 2X plan usage",
                supports_inline_args: true,
                available_during_task: false,
                is_visible: true,
                show_in_command_popup: true,
            },
            SlashCommand::Approvals => SlashCommandSpec {
                description: "choose what Codex is allowed to do",
                supports_inline_args: false,
                available_during_task: false,
                is_visible: true,
                show_in_command_popup: false,
            },
            SlashCommand::Permissions => SlashCommandSpec {
                description: "choose what Codex is allowed to do",
                supports_inline_args: false,
                available_during_task: false,
                is_visible: true,
                show_in_command_popup: true,
            },
            SlashCommand::ElevateSandbox => SlashCommandSpec {
                description: "set up elevated agent sandbox",
                supports_inline_args: false,
                available_during_task: false,
                is_visible: true,
                show_in_command_popup: true,
            },
            SlashCommand::SandboxReadRoot => SlashCommandSpec {
                description: "let sandbox read a directory: /sandbox-add-read-dir <absolute_path>",
                supports_inline_args: true,
                available_during_task: false,
                is_visible: cfg!(target_os = "windows"),
                show_in_command_popup: true,
            },
            SlashCommand::Experimental => SlashCommandSpec {
                description: "toggle experimental features",
                supports_inline_args: false,
                available_during_task: false,
                is_visible: true,
                show_in_command_popup: true,
            },
            SlashCommand::Skills => SlashCommandSpec {
                description: "use skills to improve how Codex performs specific tasks",
                supports_inline_args: false,
                available_during_task: true,
                is_visible: true,
                show_in_command_popup: true,
            },
            SlashCommand::Review => SlashCommandSpec {
                description: "review my current changes and find issues",
                supports_inline_args: true,
                available_during_task: false,
                is_visible: true,
                show_in_command_popup: true,
            },
            SlashCommand::Rename => SlashCommandSpec {
                description: "rename the current thread",
                supports_inline_args: true,
                available_during_task: true,
                is_visible: true,
                show_in_command_popup: true,
            },
            SlashCommand::New => SlashCommandSpec {
                description: "start a new chat during a conversation",
                supports_inline_args: false,
                available_during_task: false,
                is_visible: true,
                show_in_command_popup: true,
            },
            SlashCommand::Resume => SlashCommandSpec {
                description: "resume a saved chat",
                supports_inline_args: false,
                available_during_task: false,
                is_visible: true,
                show_in_command_popup: true,
            },
            SlashCommand::Fork => SlashCommandSpec {
                description: "fork the current chat",
                supports_inline_args: false,
                available_during_task: false,
                is_visible: true,
                show_in_command_popup: true,
            },
            SlashCommand::Init => SlashCommandSpec {
                description: "create an AGENTS.md file with instructions for Codex",
                supports_inline_args: false,
                available_during_task: false,
                is_visible: true,
                show_in_command_popup: true,
            },
            SlashCommand::Compact => SlashCommandSpec {
                description: "summarize conversation to prevent hitting the context limit",
                supports_inline_args: false,
                available_during_task: false,
                is_visible: true,
                show_in_command_popup: true,
            },
            SlashCommand::Plan => SlashCommandSpec {
                description: "switch to Plan mode",
                supports_inline_args: true,
                available_during_task: false,
                is_visible: true,
                show_in_command_popup: true,
            },
            SlashCommand::Collab => SlashCommandSpec {
                description: "change collaboration mode (experimental)",
                supports_inline_args: false,
                available_during_task: true,
                is_visible: true,
                show_in_command_popup: true,
            },
            SlashCommand::Agent => SlashCommandSpec {
                description: "switch the active agent thread",
                supports_inline_args: false,
                available_during_task: true,
                is_visible: true,
                show_in_command_popup: true,
            },
            SlashCommand::Diff => SlashCommandSpec {
                description: "show git diff (including untracked files)",
                supports_inline_args: false,
                available_during_task: true,
                is_visible: true,
                show_in_command_popup: true,
            },
            SlashCommand::Copy => SlashCommandSpec {
                description: "copy the latest Codex output to your clipboard",
                supports_inline_args: false,
                available_during_task: true,
                is_visible: !cfg!(target_os = "android"),
                show_in_command_popup: true,
            },
            SlashCommand::Mention => SlashCommandSpec {
                description: "mention a file",
                supports_inline_args: false,
                available_during_task: true,
                is_visible: true,
                show_in_command_popup: true,
            },
            SlashCommand::Status => SlashCommandSpec {
                description: "show current session configuration and token usage",
                supports_inline_args: false,
                available_during_task: true,
                is_visible: true,
                show_in_command_popup: true,
            },
            SlashCommand::DebugConfig => SlashCommandSpec {
                description: "show config layers and requirement sources for debugging",
                supports_inline_args: false,
                available_during_task: true,
                is_visible: true,
                show_in_command_popup: true,
            },
            SlashCommand::Statusline => SlashCommandSpec {
                description: "configure which items appear in the status line",
                supports_inline_args: false,
                available_during_task: false,
                is_visible: true,
                show_in_command_popup: true,
            },
            SlashCommand::Theme => SlashCommandSpec {
                description: "choose a syntax highlighting theme",
                supports_inline_args: false,
                available_during_task: false,
                is_visible: true,
                show_in_command_popup: true,
            },
            SlashCommand::Mcp => SlashCommandSpec {
                description: "list configured MCP tools",
                supports_inline_args: false,
                available_during_task: true,
                is_visible: true,
                show_in_command_popup: true,
            },
            SlashCommand::Apps => SlashCommandSpec {
                description: "manage apps",
                supports_inline_args: false,
                available_during_task: true,
                is_visible: true,
                show_in_command_popup: true,
            },
            SlashCommand::Logout => SlashCommandSpec {
                description: "log out of Codex",
                supports_inline_args: false,
                available_during_task: false,
                is_visible: true,
                show_in_command_popup: true,
            },
            SlashCommand::Quit => SlashCommandSpec {
                description: "exit Codex",
                supports_inline_args: false,
                available_during_task: true,
                is_visible: true,
                show_in_command_popup: false,
            },
            SlashCommand::Exit => SlashCommandSpec {
                description: "exit Codex",
                supports_inline_args: false,
                available_during_task: true,
                is_visible: true,
                show_in_command_popup: true,
            },
            SlashCommand::Feedback => SlashCommandSpec {
                description: "send logs to maintainers",
                supports_inline_args: false,
                available_during_task: true,
                is_visible: true,
                show_in_command_popup: true,
            },
            SlashCommand::Rollout => SlashCommandSpec {
                description: "print the rollout file path",
                supports_inline_args: false,
                available_during_task: true,
                is_visible: cfg!(debug_assertions),
                show_in_command_popup: true,
            },
            SlashCommand::Ps => SlashCommandSpec {
                description: "list background terminals",
                supports_inline_args: false,
                available_during_task: true,
                is_visible: true,
                show_in_command_popup: true,
            },
            SlashCommand::Stop => SlashCommandSpec {
                description: "stop all background terminals",
                supports_inline_args: false,
                available_during_task: true,
                is_visible: true,
                show_in_command_popup: true,
            },
            SlashCommand::Clear => SlashCommandSpec {
                description: "clear the terminal and start a new chat",
                supports_inline_args: false,
                available_during_task: false,
                is_visible: true,
                show_in_command_popup: true,
            },
            SlashCommand::Personality => SlashCommandSpec {
                description: "choose a communication style for Codex",
                supports_inline_args: false,
                available_during_task: false,
                is_visible: true,
                show_in_command_popup: true,
            },
            SlashCommand::Realtime => SlashCommandSpec {
                description: "toggle realtime voice mode (experimental)",
                supports_inline_args: false,
                available_during_task: true,
                is_visible: true,
                show_in_command_popup: true,
            },
            SlashCommand::Settings => SlashCommandSpec {
                description: "configure realtime microphone/speaker",
                supports_inline_args: false,
                available_during_task: true,
                is_visible: true,
                show_in_command_popup: true,
            },
            SlashCommand::TestApproval => SlashCommandSpec {
                description: "test approval request",
                supports_inline_args: false,
                available_during_task: true,
                is_visible: cfg!(debug_assertions),
                show_in_command_popup: true,
            },
            SlashCommand::MultiAgents => SlashCommandSpec {
                description: "switch the active agent thread",
                supports_inline_args: false,
                available_during_task: true,
                is_visible: true,
                show_in_command_popup: true,
            },
            SlashCommand::MemoryDrop => SlashCommandSpec {
                description: "DO NOT USE",
                supports_inline_args: false,
                available_during_task: false,
                is_visible: true,
                show_in_command_popup: true,
            },
            SlashCommand::MemoryUpdate => SlashCommandSpec {
                description: "DO NOT USE",
                supports_inline_args: false,
                available_during_task: false,
                is_visible: true,
                show_in_command_popup: true,
            },
        }
    }

    /// User-visible description shown in the popup.
    pub fn description(self) -> &'static str {
        self.spec().description
    }

    /// Command string without the leading '/'. Provided for compatibility with
    /// existing code that expects a method named `command()`.
    pub fn command(self) -> &'static str {
        self.into()
    }

    /// Whether this command supports inline args (for example `/review ...`).
    pub fn supports_inline_args(self) -> bool {
        self.spec().supports_inline_args
    }

    /// Whether this command can be run while a task is in progress.
    pub fn available_during_task(self) -> bool {
        self.spec().available_during_task
    }

    pub(crate) fn show_in_command_popup(self) -> bool {
        self.spec().show_in_command_popup
    }

    fn is_visible(self) -> bool {
        self.spec().is_visible
    }
}

/// Return all built-in commands in a Vec paired with their command string.
pub fn built_in_slash_commands() -> Vec<(&'static str, SlashCommand)> {
    SlashCommand::iter()
        .filter(|command| command.is_visible())
        .map(|c| (c.command(), c))
        .collect()
}

struct SlashCommandSpec {
    description: &'static str,
    supports_inline_args: bool,
    available_during_task: bool,
    is_visible: bool,
    show_in_command_popup: bool,
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;
    use std::str::FromStr;

    use super::SlashCommand;

    #[test]
    fn approvals_alias_is_hidden_from_command_popup() {
        assert_eq!(SlashCommand::Approvals.show_in_command_popup(), false);
    }

    #[test]
    fn clean_alias_parses_to_stop_command() {
        assert_eq!(SlashCommand::from_str("clean"), Ok(SlashCommand::Stop));
    }

    #[test]
    fn stop_command_is_canonical_name() {
        assert_eq!(SlashCommand::Stop.command(), "stop");
    }
}

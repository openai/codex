use std::str::FromStr;
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
    Approvals,
    Review,
    New,
    Init,
    Compact,
    Undo,
    Diff,
    Mention,
    Status,
    Mcp,
    Vim,
    Logout,
    Quit,
    #[cfg(debug_assertions)]
    TestApproval,
}

impl SlashCommand {
    /// User-visible description shown in the popup.
    pub fn description(self) -> &'static str {
        match self {
            SlashCommand::New => "start a new chat during a conversation",
            SlashCommand::Init => "create an AGENTS.md file with instructions for Codex",
            SlashCommand::Compact => "summarize conversation to prevent hitting the context limit",
            SlashCommand::Review => "review my current changes and find issues",
            SlashCommand::Undo => "restore the workspace to the last Codex snapshot",
            SlashCommand::Quit => "exit Codex",
            SlashCommand::Diff => "show git diff (including untracked files)",
            SlashCommand::Mention => "mention a file",
            SlashCommand::Status => "show current session configuration and token usage",
            SlashCommand::Model => "choose what model and reasoning effort to use",
            SlashCommand::Approvals => "choose what Codex can do without approval",
            SlashCommand::Mcp => "list configured MCP tools",
            SlashCommand::Vim => "Toggle between Vim and Normal editing modes",
            SlashCommand::Logout => "log out of Codex",
            #[cfg(debug_assertions)]
            SlashCommand::TestApproval => "test approval request",
        }
    }

    /// Command string without the leading '/'. Provided for compatibility with
    /// existing code that expects a method named `command()`.
    pub fn command(self) -> &'static str {
        self.into()
    }

    /// Whether this command can be run while a task is in progress.
    pub fn available_during_task(self) -> bool {
        match self {
            SlashCommand::New
            | SlashCommand::Init
            | SlashCommand::Compact
            | SlashCommand::Undo
            | SlashCommand::Model
            | SlashCommand::Approvals
            | SlashCommand::Review
            | SlashCommand::Logout => false,
            SlashCommand::Diff
            | SlashCommand::Mention
            | SlashCommand::Status
            | SlashCommand::Mcp
            | SlashCommand::Vim
            | SlashCommand::Quit => true,

            #[cfg(debug_assertions)]
            SlashCommand::TestApproval => true,
        }
    }
}

/// Return all built-in commands in a Vec paired with their command string.
pub fn built_in_slash_commands() -> Vec<(&'static str, SlashCommand)> {
    let show_beta_features = beta_features_enabled();

    SlashCommand::iter()
        .filter(|cmd| {
            if *cmd == SlashCommand::Undo {
                show_beta_features
            } else {
                true
            }
        })
        .map(|c| (c.command(), c))
        .collect()
}

fn beta_features_enabled() -> bool {
    std::env::var_os("BETA_FEATURE").is_some()
}

/// A parsed slash-command invocation containing the command and any whitespace-separated
/// arguments that followed it on the first line of input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlashCommandInvocation {
    pub command: SlashCommand,
    pub args: Vec<String>,
}

impl SlashCommandInvocation {
    pub fn new(command: SlashCommand, args: Vec<String>) -> Self {
        Self { command, args }
    }

    /// Parse an invocation from the provided input string.
    /// Returns `None` when the string does not start with a known slash command.
    pub fn parse(input: &str) -> Option<Self> {
        let first_line = input.lines().next()?.trim();
        let stripped = first_line.strip_prefix('/')?.trim_start();
        let mut parts = stripped.split_whitespace();
        let command_token = parts.next()?;
        let command = SlashCommand::from_str(command_token).ok()?;
        let args = parts.map(std::string::ToString::to_string).collect();
        Some(Self { command, args })
    }
}

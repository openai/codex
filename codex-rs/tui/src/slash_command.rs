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
    GlobalPrompt,
    Alarm,
    Review,
    New,
    Restart,
    Init,
    Compact,
    Checkpoint,
    Commit,
    Undo,
    Diff,
    Preset,
    Mention,
    Status,
    Todo,
    Alias,
    Mcp,
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
            SlashCommand::Restart => "restart Codex without closing the TUI",
            SlashCommand::Init => "create an AGENTS.md file with instructions for Codex",
            SlashCommand::Compact => "summarize conversation to prevent hitting the context limit",
            SlashCommand::Review => "review my current changes and find issues",
            SlashCommand::Undo => "restore the workspace to the last Codex snapshot",
            SlashCommand::Quit => "exit Codex",
            SlashCommand::Diff => "show git diff (including untracked files)",
            SlashCommand::Commit => "summarize and commit workspace changes to git",
            SlashCommand::Mention => "mention a file",
            SlashCommand::Status => "show current session configuration and token usage",
            SlashCommand::Todo => "manage a shared TODO list for the session",
            SlashCommand::Alias => "save or reuse custom prompt aliases",
            SlashCommand::Preset => "create or load reusable prompt presets",
            SlashCommand::Model => "choose what model and reasoning effort to use",
            SlashCommand::Approvals => "choose what Codex can do without approval",
            SlashCommand::GlobalPrompt => "set or clear the prompt sent at session start",
            SlashCommand::Alarm => "run a custom script after each Codex response",
            SlashCommand::Checkpoint => "save or load a checkpoint for this workspace",
            SlashCommand::Mcp => "list configured MCP tools",
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
            | SlashCommand::Restart
            | SlashCommand::Init
            | SlashCommand::Compact
            | SlashCommand::Undo
            | SlashCommand::Model
            | SlashCommand::Approvals
            | SlashCommand::Commit
            | SlashCommand::Review
            | SlashCommand::Logout
            | SlashCommand::Preset => false,
            SlashCommand::Diff
            | SlashCommand::Mention
            | SlashCommand::Status
            | SlashCommand::Todo
            | SlashCommand::Alias
            | SlashCommand::Mcp
            | SlashCommand::GlobalPrompt
            | SlashCommand::Alarm
            | SlashCommand::Checkpoint
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

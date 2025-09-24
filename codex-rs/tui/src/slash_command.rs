use crate::external_editor::external_editor_is_enabled;
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
    Edit,
    Undo,
    Diff,
    Mention,
    Status,
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
            SlashCommand::Init => "create an AGENTS.md file with instructions for Codex",
            SlashCommand::Compact => "summarize conversation to prevent hitting the context limit",
            SlashCommand::Edit => "open the current prompt in your external editor",
            SlashCommand::Review => "review my current changes and find issues",
            SlashCommand::Undo => "restore the workspace to the last Codex snapshot",
            SlashCommand::Quit => "exit Codex",
            SlashCommand::Diff => "show git diff (including untracked files)",
            SlashCommand::Mention => "mention a file",
            SlashCommand::Status => "show current session configuration and token usage",
            SlashCommand::Model => "choose what model and reasoning effort to use",
            SlashCommand::Approvals => "choose what Codex can do without approval",
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
            | SlashCommand::Init
            | SlashCommand::Compact
            | SlashCommand::Undo
            | SlashCommand::Model
            | SlashCommand::Approvals
            | SlashCommand::Review
            | SlashCommand::Logout => false,
            SlashCommand::Diff
            | SlashCommand::Edit
            | SlashCommand::Mention
            | SlashCommand::Status
            | SlashCommand::Mcp
            | SlashCommand::Quit => true,

            #[cfg(debug_assertions)]
            SlashCommand::TestApproval => true,
        }
    }
}

/// Return all built-in commands in a Vec paired with their command string.
pub fn built_in_slash_commands() -> Vec<(&'static str, SlashCommand)> {
    let show_beta_features = beta_features_enabled();
    let edit_enabled = external_editor_is_enabled();

    SlashCommand::iter()
        .filter(|cmd| {
            if *cmd == SlashCommand::Undo {
                show_beta_features
            } else if *cmd == SlashCommand::Edit {
                edit_enabled
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlashCommandInvocation {
    pub command: SlashCommand,
    raw_input: String,
    raw_args: String,
    args: Vec<String>,
}

impl SlashCommandInvocation {
    pub fn new(command: SlashCommand, composer_line: &str) -> Self {
        let first_line = composer_line.lines().next().unwrap_or("");
        let trimmed_line = first_line.trim_start();
        let mut raw_args = String::new();
        let mut args = Vec::new();

        if let Some(stripped) = trimmed_line.strip_prefix('/') {
            let trimmed = stripped.trim_start();
            if let Some(rest) = trimmed.strip_prefix(command.command()) {
                raw_args = rest.trim_start().to_string();
                if !raw_args.is_empty() {
                    args = raw_args
                        .split_whitespace()
                        .map(|part| part.to_string())
                        .collect();
                }
            }
        }

        Self {
            command,
            raw_input: trimmed_line.to_string(),
            raw_args,
            args,
        }
    }

    pub fn args(&self) -> &[String] {
        &self.args
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_args_for_edit_command() {
        let invocation = SlashCommandInvocation::new(SlashCommand::Edit, "/edit --send --keep");
        assert_eq!(invocation.command, SlashCommand::Edit);
        assert_eq!(
            invocation.args(),
            &["--send".to_string(), "--keep".to_string()]
        );
    }

    #[test]
    fn trims_leading_whitespace() {
        let invocation = SlashCommandInvocation::new(SlashCommand::Edit, "   /edit   --new");
        assert_eq!(invocation.command, SlashCommand::Edit);
        assert_eq!(invocation.args(), &["--new".to_string()]);
    }
}

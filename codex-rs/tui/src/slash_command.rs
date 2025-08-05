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
    New,
    Compact,
    Diff,
    Model,
    Approvals,
    Quit,
    #[cfg(debug_assertions)]
    TestApproval,
}

impl SlashCommand {
    /// User-visible description shown in the popup.
    pub fn description(self) -> &'static str {
        match self {
            SlashCommand::New => "Start a new chat.",
            SlashCommand::Compact => "Compact the chat history.",
            SlashCommand::Quit => "Exit the application.",
            SlashCommand::Model => "Select the model to use.",
            SlashCommand::Approvals => "Select the execution mode.",
            SlashCommand::Diff => {
                "Show git diff of the working directory (including untracked files)"
            }
            #[cfg(debug_assertions)]
            SlashCommand::TestApproval => "Test approval request",
        }
    }

    /// Command string without the leading '/'. Provided for compatibility with
    /// existing code that expects a method named `command()`.
    pub fn command(self) -> &'static str {
        self.into()
    }
}

/// Return all built-in commands in a Vec paired with their command string.
pub fn built_in_slash_commands() -> Vec<(&'static str, SlashCommand)> {
    SlashCommand::iter().map(|c| (c.command(), c)).collect()
}

/// Parsed representation of a line that may start with a slash command.
pub enum ParsedSlash<'a> {
    /// A recognized command along with the left-trimmed arguments.
    Command { cmd: SlashCommand, args: &'a str },
    /// A leading slash and a token were present, but the token is not a known command.
    Incomplete { token: &'a str },
    /// Line does not represent a slash command.
    None,
}

/// Parse the first line of input and detect a leading slash command.
///
/// Returns:
/// - ParsedSlash::Command if the token matches a known command; `args` is the
///   remainder of the line after the token, with leading whitespace trimmed.
/// - ParsedSlash::Incomplete if there is a token after '/', but it does not
///   correspond to a known command.
/// - ParsedSlash::None if the line does not start with '/'.
pub fn parse_slash_line(line: &str) -> ParsedSlash<'_> {
    let Some(stripped) = line.strip_prefix('/') else {
        return ParsedSlash::None;
    };
    let token_with_ws = stripped.trim_start();
    let token = token_with_ws.split_whitespace().next().unwrap_or("");
    if token.is_empty() {
        return ParsedSlash::Incomplete { token: "" };
    }
    match SlashCommand::from_str(token) {
        Ok(cmd) => {
            let rest = &token_with_ws[token.len()..];
            let args = rest.trim_start();
            ParsedSlash::Command { cmd, args }
        }
        Err(_) => ParsedSlash::Incomplete { token },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_known_command_and_args() {
        let p = parse_slash_line("/model gpt-4o");
        match p {
            ParsedSlash::Command { cmd, args } => {
                assert_eq!(cmd, SlashCommand::Model);
                assert_eq!(args, "gpt-4o");
            }
            _ => panic!("expected Command"),
        }
    }

    #[test]
    fn incomplete_for_unknown_token() {
        let p = parse_slash_line("/not-a-cmd something");
        match p {
            ParsedSlash::Incomplete { token } => assert_eq!(token, "not-a-cmd"),
            _ => panic!("expected Incomplete"),
        }
    }

    #[test]
    fn none_for_non_command() {
        matches!(parse_slash_line("hello"), ParsedSlash::None);
    }
}

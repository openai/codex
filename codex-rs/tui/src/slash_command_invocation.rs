use codex_protocol::user_input::ByteRange;
use codex_protocol::user_input::TextElement;

use crate::slash_command::SlashCommand;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FastSlashCommandArgs {
    On,
    Off,
    Status,
}

impl FastSlashCommandArgs {
    fn as_str(self) -> &'static str {
        match self {
            FastSlashCommandArgs::On => "on",
            FastSlashCommandArgs::Off => "off",
            FastSlashCommandArgs::Status => "status",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct SlashCommandTextArg {
    pub(crate) text: String,
    pub(crate) text_elements: Vec<TextElement>,
}

impl SlashCommandTextArg {
    pub(crate) fn new(text: String, text_elements: Vec<TextElement>) -> Self {
        Self {
            text,
            text_elements,
        }
    }

    fn with_prefix(&self, prefix: &str) -> SerializedSlashCommand {
        let prefix_len = prefix.len();
        let text = if self.text.is_empty() {
            prefix.to_string()
        } else {
            format!("{prefix} {}", self.text)
        };
        let offset = prefix_len + usize::from(!self.text.is_empty());
        let text_elements = self
            .text_elements
            .iter()
            .map(|element| {
                element.map_range(|byte_range| ByteRange {
                    start: byte_range.start + offset,
                    end: byte_range.end + offset,
                })
            })
            .collect();
        SerializedSlashCommand {
            text,
            text_elements,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum SlashCommandInvocation {
    Bare(SlashCommand),
    Fast(FastSlashCommandArgs),
    Rename(SlashCommandTextArg),
    Plan(SlashCommandTextArg),
    Review(SlashCommandTextArg),
    SandboxReadRoot(SlashCommandTextArg),
}

impl SlashCommandInvocation {
    pub(crate) fn bare(command: SlashCommand) -> Self {
        Self::Bare(command)
    }

    pub(crate) fn command(&self) -> SlashCommand {
        match self {
            SlashCommandInvocation::Bare(command) => *command,
            SlashCommandInvocation::Fast(_) => SlashCommand::Fast,
            SlashCommandInvocation::Rename(_) => SlashCommand::Rename,
            SlashCommandInvocation::Plan(_) => SlashCommand::Plan,
            SlashCommandInvocation::Review(_) => SlashCommand::Review,
            SlashCommandInvocation::SandboxReadRoot(_) => SlashCommand::SandboxReadRoot,
        }
    }

    pub(crate) fn serialize(&self) -> SerializedSlashCommand {
        let command = self.command().command();
        let prefix = format!("/{command}");
        match self {
            SlashCommandInvocation::Bare(_) => SerializedSlashCommand {
                text: prefix,
                text_elements: Vec::new(),
            },
            SlashCommandInvocation::Fast(args) => SerializedSlashCommand {
                text: format!("{prefix} {}", args.as_str()),
                text_elements: Vec::new(),
            },
            SlashCommandInvocation::Rename(arg)
            | SlashCommandInvocation::Plan(arg)
            | SlashCommandInvocation::Review(arg)
            | SlashCommandInvocation::SandboxReadRoot(arg) => arg.with_prefix(&prefix),
        }
    }

    pub(crate) fn into_prefixed_string(self) -> String {
        self.serialize().text
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct SerializedSlashCommand {
    pub(crate) text: String,
    pub(crate) text_elements: Vec<TextElement>,
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn bare_invocation_serializes_with_leading_slash() {
        let invocation = SlashCommandInvocation::bare(SlashCommand::Model);

        assert_eq!(
            invocation.serialize(),
            SerializedSlashCommand {
                text: "/model".to_string(),
                text_elements: Vec::new(),
            }
        );
    }

    #[test]
    fn fast_invocation_serializes_canonical_token() {
        let invocation = SlashCommandInvocation::Fast(FastSlashCommandArgs::Status);

        assert_eq!(
            invocation.serialize(),
            SerializedSlashCommand {
                text: "/fast status".to_string(),
                text_elements: Vec::new(),
            }
        );
    }

    #[test]
    fn variadic_invocation_preserves_placeholder_ranges() {
        let placeholder = "[Image #1]".to_string();
        let invocation = SlashCommandInvocation::Plan(SlashCommandTextArg::new(
            format!("review {placeholder}"),
            vec![TextElement::new((7..18).into(), Some(placeholder.clone()))],
        ));

        assert_eq!(
            invocation.serialize(),
            SerializedSlashCommand {
                text: format!("/plan review {placeholder}"),
                text_elements: vec![TextElement::new((13..24).into(), Some(placeholder),)],
            }
        );
    }
}

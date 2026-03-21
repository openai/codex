use codex_protocol::user_input::TextElement;

use crate::slash_command::SlashCommand;
use crate::slash_command::SlashCommandParseInput;
use crate::slash_command::SlashCommandUsageErrorKind;
use crate::slash_command_invocation::SlashCommandInvocation;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SlashCommandUsageError {
    command: SlashCommand,
    kind: SlashCommandUsageErrorKind,
}

impl SlashCommandUsageError {
    pub(crate) fn message(self) -> String {
        let usage = self.command.usage_lines().join(" | ");
        match self.kind {
            SlashCommandUsageErrorKind::UnexpectedInlineArgs => format!(
                "'/{}' does not accept inline arguments. Usage: {usage}",
                self.command.command()
            ),
            SlashCommandUsageErrorKind::InvalidInlineArgs => format!("Usage: {usage}"),
        }
    }
}

impl SlashCommand {
    pub(crate) fn parse_invocation(
        self,
        args: &str,
        text_elements: &[TextElement],
    ) -> Result<SlashCommandInvocation, SlashCommandUsageError> {
        let spec = self.spec();
        if args.trim().is_empty() {
            return match spec.bare_behavior {
                crate::slash_command::SlashCommandBareBehavior::DispatchesDirectly
                | crate::slash_command::SlashCommandBareBehavior::OpensUi => {
                    Ok(SlashCommandInvocation::Bare(self))
                }
            };
        }

        (spec.parser)(
            self,
            SlashCommandParseInput {
                args,
                text_elements,
            },
        )
        .map_err(|kind| SlashCommandUsageError {
            command: self,
            kind,
        })
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::slash_command::SlashCommandBareBehavior;
    use crate::slash_command_invocation::FastSlashCommandArgs;
    use crate::slash_command_invocation::SlashCommandTextArg;

    #[test]
    fn review_bare_form_is_marked_as_ui_driven() {
        assert_eq!(
            SlashCommand::Review.parse_invocation("", &[]),
            Ok(SlashCommandInvocation::Bare(SlashCommand::Review))
        );
        assert_eq!(
            SlashCommand::Review.spec().bare_behavior,
            SlashCommandBareBehavior::OpensUi
        );
    }

    #[test]
    fn fast_accepts_nonempty_inline_args() {
        assert_eq!(
            SlashCommand::Fast.parse_invocation("status", &[]),
            Ok(SlashCommandInvocation::Fast(FastSlashCommandArgs::Status))
        );
    }

    #[test]
    fn review_preserves_placeholder_elements() {
        let placeholder = "[Image #1]".to_string();
        let text_elements = vec![TextElement::new((0..11).into(), Some(placeholder.clone()))];

        assert_eq!(
            SlashCommand::Review.parse_invocation(&placeholder, &text_elements),
            Ok(SlashCommandInvocation::Review(SlashCommandTextArg::new(
                placeholder,
                text_elements,
            )))
        );
    }

    #[test]
    fn clear_rejects_unexpected_inline_args() {
        assert_eq!(
            SlashCommand::Clear
                .parse_invocation("now", &[])
                .unwrap_err()
                .message(),
            "'/clear' does not accept inline arguments. Usage: /clear"
        );
    }
}

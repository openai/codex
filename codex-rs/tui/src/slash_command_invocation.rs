use crate::chatwidget::UserMessage;
use crate::slash_command::SlashCommand;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SlashCommandInvocation {
    pub(crate) command: SlashCommand,
    pub(crate) args: Vec<String>,
}

impl SlashCommandInvocation {
    pub(crate) fn bare(command: SlashCommand) -> Self {
        Self {
            command,
            args: Vec::new(),
        }
    }

    pub(crate) fn with_args<I, S>(command: SlashCommand, args: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        Self {
            command,
            args: args.into_iter().map(Into::into).collect(),
        }
    }

    pub(crate) fn parse_args(args: &str, usage: &str) -> Result<Vec<String>, String> {
        shlex::split(args).ok_or_else(|| usage.to_string())
    }

    pub(crate) fn into_user_message(self) -> UserMessage {
        let command = self.command.command();
        let joined = match shlex::try_join(
            std::iter::once(command).chain(self.args.iter().map(String::as_str)),
        ) {
            Ok(joined) => joined,
            Err(err) => panic!("slash command invocation should serialize: {err}"),
        };
        format!("/{joined}").into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn serializes_quoted_args() {
        let draft = SlashCommandInvocation::with_args(
            SlashCommand::Review,
            ["branch main needs coverage".to_string()],
        )
        .into_user_message();

        assert_eq!(
            draft,
            UserMessage::from("/review 'branch main needs coverage'")
        );
    }

    #[test]
    fn parses_shlex_args() {
        let parsed = SlashCommandInvocation::parse_args("'branch main' --flag key=value", "usage")
            .expect("quoted args should parse");

        assert_eq!(
            parsed,
            vec![
                "branch main".to_string(),
                "--flag".to_string(),
                "key=value".to_string()
            ]
        );
    }
}

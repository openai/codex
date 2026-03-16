use crate::slash_command::SlashCommand;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SlashCommandInvocation {
    command: SlashCommand,
    args: Vec<String>,
}

impl SlashCommandInvocation {
    pub(crate) fn bare(command: SlashCommand) -> Self {
        Self {
            command,
            args: Vec::new(),
        }
    }

    pub(crate) fn into_prefixed_string(self) -> String {
        let command = self.command.command();
        let joined = match shlex::try_join(
            std::iter::once(command).chain(self.args.iter().map(String::as_str)),
        ) {
            Ok(joined) => joined,
            Err(err) => panic!("slash command invocation should serialize: {err}"),
        };
        format!("/{joined}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn bare_invocation_serializes_with_leading_slash() {
        let invocation = SlashCommandInvocation::bare(SlashCommand::Model);

        assert_eq!(invocation.into_prefixed_string(), "/model");
    }
}

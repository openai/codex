//! Named slash-command settings shared by submission and composer completion.

use crate::slash_command::SlashCommand;

type Names = &'static [&'static str];

pub(crate) struct Settings {
    pub(crate) values: Names,
    pub(crate) switches: Names,
    pub(crate) booleans: Names,
}

pub(crate) fn for_command(command: SlashCommand) -> Settings {
    let (values, switches, booleans): (Names, Names, Names) = match command {
        SlashCommand::Model => (&["--reasoning", "--speed"], &[], &[]),
        SlashCommand::Permissions => (&["--approval-policy", "--reviewer"], &[], &[]),
        SlashCommand::Title => (&[], &["--clear"], &[]),
        SlashCommand::Statusline => (&[], &["--clear"], &["colors"]),
        SlashCommand::Mcp => (&["--environment"], &[], &[]),
        _ => (&[], &[], &[]),
    };
    Settings {
        values,
        switches,
        booleans,
    }
}

pub(crate) fn value_choices(command: SlashCommand, name: &str) -> &'static [&'static str] {
    match (command, name) {
        (SlashCommand::Permissions, "--approval-policy") => &["untrusted", "on-request", "never"],
        (SlashCommand::Permissions, "--reviewer") => &["user", "auto_review"],
        (SlashCommand::Model, "--speed") => &["standard", "flex"],
        _ => &[],
    }
}

//! Preserve config overrides across command boundaries without changing typed child arguments.

use crate::MultitoolCli;
use clap::Command;
use clap::CommandFactory;
use clap::FromArgMatches;
use codex_utils_cli::CliConfigOverrides;
use std::ffi::OsString;

pub(super) fn try_parse_from(
    args: impl IntoIterator<Item = impl Into<OsString>>,
) -> Result<MultitoolCli, clap::Error> {
    let args: Vec<OsString> = args.into_iter().map(Into::into).collect();
    let mut command = MultitoolCli::command();
    let mut matches = command.try_get_matches_from_mut(&args)?;
    crate::queue_cmd::validate_options(&command, &matches)?;
    let mut cli = MultitoolCli::from_arg_matches_mut(&mut matches)
        .map_err(|error| error.format(&mut command))?;
    if cli.subcommand.is_none() || cli.config_overrides.raw_overrides.is_empty() {
        return Ok(cli);
    }

    // Clap propagates the deepest global argument's values instead of appending
    // values from each scope. Reparse with local config arguments to recover them.
    let matches = localize_config_arg(MultitoolCli::command()).try_get_matches_from(args)?;

    // Keep child copies unchanged: existing dispatch merges apply them after
    // generated feature overrides, and clearing them would change precedence.
    cli.config_overrides.raw_overrides = std::iter::successors(Some(&matches), |scope| {
        scope.subcommand().map(|(_, child)| child)
    })
    .filter_map(|scope| scope.get_many::<String>("raw_overrides"))
    .flatten()
    .cloned()
    .collect();
    Ok(cli)
}

fn localize_config_arg(mut command: Command) -> Command {
    if !command
        .get_arguments()
        .any(|arg| arg.get_id() == "raw_overrides")
    {
        command = command.args(CliConfigOverrides::command().get_arguments().cloned());
    }
    command
        .mut_arg("raw_overrides", |arg| arg.global(false))
        .mut_subcommands(localize_config_arg)
}

#[cfg(test)]
#[path = "config_override_args_tests.rs"]
mod tests;

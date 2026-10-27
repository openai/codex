//! Preserve config overrides across command boundaries without changing typed child arguments.

use crate::MultitoolCli;
use clap::CommandFactory;
use clap::FromArgMatches;
use codex_utils_cli::collect_config_overrides;
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

    // Keep child copies unchanged: dispatch merges apply them after feature overrides.
    cli.config_overrides = collect_config_overrides(MultitoolCli::command(), args)?;
    Ok(cli)
}

#[cfg(test)]
#[path = "config_override_args_tests.rs"]
mod tests;

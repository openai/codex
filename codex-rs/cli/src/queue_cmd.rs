use clap::Parser;
use clap::builder::NonEmptyStringValueParser;
use codex_arg0::Arg0DispatchPaths;
use codex_tui::Cli as TuiCli;
use codex_utils_cli::CliConfigOverrides;

use crate::InteractiveRemoteOptions;
use crate::SessionArchiveConfigOverrides;
use crate::finalize_session_archive_interactive;
use crate::resolve_remote_endpoint;

#[derive(Debug, Parser)]
pub(crate) struct QueueCommand {
    /// Session UUID or exact session name.
    #[arg(long, value_name = "THREAD")]
    thread: String,

    /// Message text to queue.
    #[arg(long, value_name = "TEXT", value_parser = NonEmptyStringValueParser::new())]
    message: String,

    /// After queueing, try to resume the thread on the local daemon or selected --remote server.
    /// A failed resume produces a warning and the command still succeeds.
    #[arg(long)]
    resume: bool,

    #[clap(flatten)]
    remote: InteractiveRemoteOptions,

    #[clap(flatten)]
    pub(crate) config_overrides: SessionArchiveConfigOverrides,
}

pub(crate) async fn run_queue_command(
    command: QueueCommand,
    interactive: TuiCli,
    root_config_overrides: CliConfigOverrides,
    root_remote: Option<String>,
    root_remote_auth_token_env: Option<String>,
    arg0_paths: Arg0DispatchPaths,
) -> anyhow::Result<String> {
    let QueueCommand {
        thread,
        message,
        resume,
        remote,
        config_overrides,
    } = command;
    let cli =
        finalize_session_archive_interactive(interactive, root_config_overrides, config_overrides);
    if !cli.images.is_empty() {
        anyhow::bail!("`codex queue` does not support image attachments");
    }
    let explicit_remote_endpoint = resolve_remote_endpoint(
        remote.remote.or(root_remote),
        remote.remote_auth_token_env.or(root_remote_auth_token_env),
    )?;
    codex_tui::run_session_queue_command(
        thread,
        message,
        resume,
        codex_tui::SessionArchiveCommandOptions {
            cli,
            arg0_paths,
            explicit_remote_endpoint,
        },
    )
    .await
    .map_err(|error| anyhow::anyhow!("{error:#}"))
}

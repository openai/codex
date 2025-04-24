mod proto;
mod seatbelt;

use std::path::PathBuf;

use clap::ArgAction;
use clap::Parser;
use codex_exec::Cli as ExecCli;
use codex_interactive::Cli as InteractiveCli;
use codex_repl::Cli as ReplCli;
use codex_tui::Cli as TuiCli;

use crate::proto::ProtoCli;

/// Codex CLI
///
/// If no subcommand is specified, options will be forwarded to the interactive CLI.
#[derive(Debug, Parser)]
#[clap(
    author,
    version,
    // If a sub‑command is given, ignore requirements of the default args.
    subcommand_negates_reqs = true
)]
struct MultitoolCli {
    #[clap(flatten)]
    interactive: InteractiveCli,

    #[clap(subcommand)]
    subcommand: Option<Subcommand>,
}

#[derive(Debug, clap::Subcommand)]
enum Subcommand {
    /// Run Codex non-interactively.
    #[clap(visible_alias = "e")]
    Exec(ExecCli),

    /// Run the TUI.
    #[clap(visible_alias = "t")]
    Tui(TuiCli),

    /// Run the REPL.
    #[clap(visible_alias = "r")]
    Repl(ReplCli),

    /// Run the Protocol stream via stdin/stdout
    #[clap(visible_alias = "p")]
    Proto(ProtoCli),

    /// Internal debugging commands.
    Debug(DebugArgs),
}

#[derive(Debug, Parser)]
struct DebugArgs {
    #[command(subcommand)]
    cmd: DebugCommand,
}

#[derive(Debug, clap::Subcommand)]
enum DebugCommand {
    /// Run a command under Seatbelt (macOS only).
    Seatbelt(SeatbeltCommand),
}

#[derive(Debug, Parser)]
struct SeatbeltCommand {
    /// Writable folder for sandbox in full-auto mode (can be specified multiple times).
    #[arg(long = "writable-root", short = 'w', value_name = "DIR", action = ArgAction::Append, use_value_delimiter = false)]
    writable_roots: Vec<PathBuf>,

    /// Full command args to run under seatbelt.
    #[arg(trailing_var_arg = true)]
    command: Vec<String>,
}

#[derive(Debug, Parser)]
struct ReplProto {}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = MultitoolCli::parse();

    match cli.subcommand {
        None => {
            codex_interactive::run_main(cli.interactive).await?;
        }
        Some(Subcommand::Exec(exec_cli)) => {
            codex_exec::run_main(exec_cli).await?;
        }
        Some(Subcommand::Tui(tui_cli)) => {
            codex_tui::run_main(tui_cli)?;
        }
        Some(Subcommand::Repl(repl_cli)) => {
            codex_repl::run_main(repl_cli).await?;
        }
        Some(Subcommand::Proto(proto_cli)) => {
            proto::run_main(proto_cli).await?;
        }
        Some(Subcommand::Debug(debug_args)) => match debug_args.cmd {
            DebugCommand::Seatbelt(SeatbeltCommand {
                command,
                writable_roots,
            }) => {
                seatbelt::run_seatbelt(command, writable_roots).await?;
            }
        },
    }

    Ok(())
}

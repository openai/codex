#[cfg(target_os = "linux")]
mod landlock;
mod proto;
mod seatbelt;

use std::path::PathBuf;

use clap::ArgAction;
use clap::Parser;
use codex_core::SandboxModeCliArg;
use codex_exec::Cli as ExecCli;
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
    interactive: TuiCli,

    #[clap(subcommand)]
    subcommand: Option<Subcommand>,
}

#[derive(Debug, clap::Subcommand)]
enum Subcommand {
    /// Run Codex non-interactively.
    #[clap(visible_alias = "e")]
    Exec(ExecCli),

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

    /// Run a command under Landlock+seccomp (Linux only).
    Landlock(LandlockCommand),
}

#[derive(Debug, Parser)]
struct SeatbeltCommand {
    /// Writable folder for sandbox (can be specified multiple times).
    #[arg(long = "writable-root", short = 'w', value_name = "DIR", action = ArgAction::Append, use_value_delimiter = false)]
    writable_roots: Vec<PathBuf>,

    /// Configure the process restrictions for the command.
    #[arg(long = "sandbox", short = 's')]
    sandbox_policy: SandboxModeCliArg,

    /// Full command args to run under seatbelt.
    #[arg(trailing_var_arg = true)]
    command: Vec<String>,
}

#[derive(Debug, Parser)]
struct LandlockCommand {
    /// Writable folder for sandbox (can be specified multiple times).
    #[arg(long = "writable-root", short = 'w', value_name = "DIR", action = ArgAction::Append, use_value_delimiter = false)]
    writable_roots: Vec<PathBuf>,

    /// Configure the process restrictions for the command.
    #[arg(long = "sandbox", short = 's')]
    sandbox_policy: SandboxModeCliArg,

    /// Full command args to run under landlock.
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
            codex_tui::run_main(cli.interactive)?;
        }
        Some(Subcommand::Exec(exec_cli)) => {
            codex_exec::run_main(exec_cli).await?;
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
                sandbox_policy,
                writable_roots,
            }) => {
                seatbelt::run_seatbelt(command, sandbox_policy.into(), writable_roots).await?;
            }
            #[cfg(target_os = "linux")]
            DebugCommand::Landlock(LandlockCommand {
                command,
                sandbox_policy,
                writable_roots,
            }) => {
                landlock::run_landlock(command, sandbox_policy.into(), writable_roots)?;
            }
            #[cfg(not(target_os = "linux"))]
            DebugCommand::Landlock(_) => {
                anyhow::bail!("Landlock is only supported on Linux.");
            }
        },
    }

    Ok(())
}

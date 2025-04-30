#[cfg(target_os = "linux")]
mod landlock;
mod proto;
mod seatbelt;

use clap::Parser;
use codex_core::protocol::SandboxPolicy;
use codex_core::SandboxPermissionOption;
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
    /// Convenience alias for low-friction sandboxed automatic execution (network-disabled sandbox that can write to cwd and TMPDIR)
    #[arg(long = "full-auto", default_value_t = false)]
    full_auto: bool,

    #[clap(flatten)]
    pub sandbox: SandboxPermissionOption,

    /// Full command args to run under seatbelt.
    #[arg(trailing_var_arg = true)]
    command: Vec<String>,
}

#[derive(Debug, Parser)]
struct LandlockCommand {
    /// Convenience alias for low-friction sandboxed automatic execution (network-disabled sandbox that can write to cwd and TMPDIR)
    #[arg(long = "full-auto", default_value_t = false)]
    full_auto: bool,

    #[clap(flatten)]
    sandbox: SandboxPermissionOption,

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
                sandbox,
                full_auto,
            }) => {
                let sandbox_policy = create_sandbox_policy(full_auto, sandbox);
                seatbelt::run_seatbelt(command, sandbox_policy).await?;
            }
            #[cfg(target_os = "linux")]
            DebugCommand::Landlock(LandlockCommand {
                command,
                sandbox,
                full_auto,
            }) => {
                let sandbox_policy = create_sandbox_policy(full_auto, sandbox);
                landlock::run_landlock(command, sandbox_policy)?;
            }
            #[cfg(not(target_os = "linux"))]
            DebugCommand::Landlock(_) => {
                anyhow::bail!("Landlock is only supported on Linux.");
            }
        },
    }

    Ok(())
}

fn create_sandbox_policy(full_auto: bool, sandbox: SandboxPermissionOption) -> SandboxPolicy {
    if full_auto {
        SandboxPolicy::new_full_auto_policy()
    } else {
        match sandbox.permissions.map(Into::into) {
            Some(sandbox_policy) => sandbox_policy,
            None => SandboxPolicy::new_read_only_policy(),
        }
    }
}

use anyhow::Result;
use clap::Parser;
use codex_execpolicy2::ExecPolicyCheckCommand;

/// CLI for evaluating exec policies
#[derive(Parser)]
#[command(name = "codex-execpolicy2")]
enum Cli {
    /// Evaluate a command against a policy.
    Check(ExecPolicyCheckCommand),
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli {
        Cli::Check(cmd) => cmd_check(cmd),
    }
}

fn cmd_check(cmd: ExecPolicyCheckCommand) -> Result<()> {
    let json = cmd.to_json()?;
    println!("{json}");
    Ok(())
}

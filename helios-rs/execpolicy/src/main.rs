use anyhow::Result;
use clap::Parser;
use helios_execpolicy::execpolicycheck::ExecPolicyCheckCommand;

/// CLI for evaluating exec policies
#[derive(Parser)]
#[command(name = "helios-execpolicy")]
enum Cli {
    /// Evaluate a command against a policy.
    Check(ExecPolicyCheckCommand),
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli {
        Cli::Check(cmd) => cmd.run(),
    }
}

use std::fs;
use std::path::Path;
use std::path::PathBuf;

use anyhow::Context;
use anyhow::Result;
use clap::Parser;
use codex_execpolicy2::PolicyParser;

/// CLI for evaluating exec policies
#[derive(Parser)]
#[command(name = "codex-execpolicy2")]
enum Cli {
    /// Evaluate a command against a policy.
    Check {
        #[arg(short, long, value_name = "PATH")]
        policy: PathBuf,

        /// Command tokens to check.
        #[arg(
            value_name = "COMMAND",
            required = true,
            trailing_var_arg = true,
            allow_hyphen_values = true
        )]
        command: Vec<String>,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli {
        Cli::Check { policy, command } => cmd_check(policy, command),
    }
}

fn cmd_check(policy_path: PathBuf, args: Vec<String>) -> Result<()> {
    let policy = load_policy(&policy_path)?;

    let eval = policy.check(&args);
    let json = serde_json::to_string_pretty(&eval)?;
    println!("{json}");
    Ok(())
}

fn load_policy(policy_path: &Path) -> Result<codex_execpolicy2::Policy> {
    let content = fs::read_to_string(policy_path)
        .with_context(|| format!("failed to read policy at {}", policy_path.display()))?;
    let policy_source = policy_path.to_string_lossy();
    let parser = PolicyParser::new(policy_source.as_ref(), &content);
    Ok(parser.parse()?)
}

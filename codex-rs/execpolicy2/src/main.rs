use std::fs;
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
        #[arg(short, long, value_name = "PATH", required = true)]
        policies: Vec<PathBuf>,

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
        Cli::Check { policies, command } => cmd_check(policies, command),
    }
}

fn cmd_check(policies: Vec<PathBuf>, args: Vec<String>) -> Result<()> {
    let policy = load_policies(&policies)?;

    let eval = policy.check(&args);
    let json = serde_json::to_string_pretty(&eval)?;
    println!("{json}");
    Ok(())
}

fn load_policies(policy_paths: &[PathBuf]) -> Result<codex_execpolicy2::Policy> {
    let loaded_policies: Vec<(String, String)> = policy_paths
        .iter()
        .map(|policy_path| {
            let policy_file_contents = fs::read_to_string(policy_path)
                .with_context(|| format!("failed to read policy at {}", policy_path.display()))?;
            let policy_identifier = policy_path.to_string_lossy().to_string();
            Ok((policy_identifier, policy_file_contents))
        })
        .collect::<Result<_>>()
        .context("failed to load policy files")?;
    Ok(PolicyParser::parse_many(loaded_policies.iter().map(
        |(policy_identifier, policy_file_contents)| {
            (policy_identifier.as_str(), policy_file_contents.as_str())
        },
    ))?)
}

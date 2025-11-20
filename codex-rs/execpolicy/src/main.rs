use std::fs;
use std::path::PathBuf;

use anyhow::Context;
use anyhow::Result;
use clap::Parser;
use codex_execpolicy::PolicyParser;

/// CLI for evaluating exec policies
#[derive(Parser)]
#[command(name = "codex-execpolicy")]
enum Cli {
    /// Evaluate a command against a policy.
    Check {
        #[arg(short, long = "policy", value_name = "PATH", required = true)]
        policies: Vec<PathBuf>,

        /// Pretty-print the JSON output.
        #[arg(long)]
        pretty: bool,

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
        Cli::Check {
            policies,
            command,
            pretty,
        } => cmd_check(policies, command, pretty),
    }
}

fn cmd_check(policy_paths: Vec<PathBuf>, args: Vec<String>, pretty: bool) -> Result<()> {
    let policy = load_policies(&policy_paths)?;

    let eval = policy.check(&args);
    let json = if pretty {
        serde_json::to_string_pretty(&eval)?
    } else {
        serde_json::to_string(&eval)?
    };
    println!("{json}");
    Ok(())
}

fn load_policies(policy_paths: &[PathBuf]) -> Result<codex_execpolicy::Policy> {
    let mut parser = PolicyParser::new();
    for policy_path in policy_paths {
        let policy_file_contents = fs::read_to_string(policy_path)
            .with_context(|| format!("failed to read policy at {}", policy_path.display()))?;
        let policy_identifier = policy_path.to_string_lossy().to_string();
        parser.parse(&policy_identifier, &policy_file_contents)?;
    }
    Ok(parser.build())
}

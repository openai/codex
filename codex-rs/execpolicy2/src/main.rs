use std::fs;
use std::path::Path;

use anyhow::Context;
use anyhow::Result;
use clap::Parser;
use codex_execpolicy2::PolicyParser;
use codex_execpolicy2::load_default_policy;

/// CLI for evaluating exec policies
#[derive(Parser)]
#[command(name = "codex-execpolicy2")]
enum Cli {
    /// Evaluate a command against a policy.
    Check {
        #[arg(short, long, value_name = "PATH")]
        policy: Option<String>,

        /// Command tokens to evaluate.
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

fn cmd_check(policy_path: Option<String>, args: Vec<String>) -> Result<()> {
    let policy = load_policy(policy_path)?;

    let eval = policy.evaluate(&args);
    let json = serde_json::to_string_pretty(&eval)?;
    println!("{json}");
    Ok(())
}

fn load_policy(policy_path: Option<String>) -> Result<codex_execpolicy2::Policy> {
    if let Some(path) = policy_path {
        let content = fs::read_to_string(&path)
            .with_context(|| format!("failed to read policy at {}", Path::new(&path).display()))?;
        let parser = PolicyParser::new(&path, &content);
        return Ok(parser.parse()?);
    }

    Ok(load_default_policy()?)
}

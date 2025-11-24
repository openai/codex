use std::fs;
use std::path::PathBuf;

use anyhow::Context;
use anyhow::Result;
use clap::Parser;

use crate::Evaluation;
use crate::Policy;
use crate::PolicyParser;

/// Arguments for evaluating a command against one or more execpolicy files.
#[derive(Debug, Parser, Clone)]
pub struct ExecPolicyCheckCommand {
    /// Paths to execpolicy files to evaluate (repeatable).
    #[arg(short = 'p', long = "policy", value_name = "PATH", required = true)]
    pub policies: Vec<PathBuf>,

    /// Pretty-print the JSON output.
    #[arg(long)]
    pub pretty: bool,

    /// Command tokens to check against the policy.
    #[arg(
        value_name = "COMMAND",
        required = true,
        trailing_var_arg = true,
        allow_hyphen_values = true
    )]
    pub command: Vec<String>,
}

impl ExecPolicyCheckCommand {
    /// Load the policies for this command, evaluate the command, and render JSON output.
    pub fn run(&self) -> Result<()> {
        let policy = load_policies(&self.policies)?;
        let evaluation = policy.check(&self.command);

        let json = format_evaluation_json(&evaluation, self.pretty)?;
        println!("{json}");

        Ok(())
    }
}

pub fn format_evaluation_json(evaluation: &Evaluation, pretty: bool) -> Result<String> {
    if pretty {
        serde_json::to_string_pretty(evaluation).map_err(Into::into)
    } else {
        serde_json::to_string(evaluation).map_err(Into::into)
    }
}

pub fn load_policies(policy_paths: &[PathBuf]) -> Result<Policy> {
    let mut parser = PolicyParser::new();

    for policy_path in policy_paths {
        let policy_file_contents = fs::read_to_string(policy_path)
            .with_context(|| format!("failed to read policy at {}", policy_path.display()))?;
        let policy_identifier = policy_path.to_string_lossy().to_string();
        parser
            .parse(&policy_identifier, &policy_file_contents)
            .with_context(|| format!("failed to parse policy at {}", policy_path.display()))?;
    }

    Ok(parser.build())
}

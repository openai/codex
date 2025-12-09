use clap::Args;
use clap::Parser;
use codex_common::CliConfigOverrides;

#[derive(Parser, Debug, Default)]
#[command(version)]
pub struct Cli {
    #[clap(skip)]
    pub config_overrides: CliConfigOverrides,

    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Debug, clap::Subcommand)]
pub enum Command {
    /// Submit a new Codex Cloud task without launching the TUI.
    Exec(ExecCommand),
    /// Show the status of a Codex Cloud task.
    Status(StatusCommand),
    /// Apply the diff for a Codex Cloud task locally.
    Apply(ApplyCommand),
    /// Show the unified diff for a Codex Cloud task.
    Diff(DiffCommand),
}

#[derive(Debug, Args)]
pub struct ExecCommand {
    /// Task prompt to run in Codex Cloud.
    #[arg(value_name = "QUERY")]
    pub query: Option<String>,

    /// Target environment identifier (see `codex cloud` to browse).
    #[arg(long = "env", value_name = "ENV_ID")]
    pub environment: String,

    /// Git branch to run in Codex Cloud.
    #[arg(long = "branch", value_name = "BRANCH", default_value = "main")]
    pub branch: String,

    /// Number of assistant attempts (best-of-N).
    #[arg(
        long = "attempts",
        default_value_t = 1usize,
        value_parser = parse_attempts
    )]
    pub attempts: usize,
}

fn parse_attempts(input: &str) -> Result<usize, String> {
    let value: usize = input
        .parse()
        .map_err(|_| "attempts must be an integer between 1 and 4".to_string())?;
    if (1..=4).contains(&value) {
        Ok(value)
    } else {
        Err("attempts must be between 1 and 4".to_string())
    }
}

#[derive(Debug, Args)]
pub struct StatusCommand {
    /// Codex Cloud task identifier to inspect.
    #[arg(value_name = "TASK_ID")]
    pub task_id: String,
}

#[derive(Debug, Args)]
pub struct ApplyCommand {
    /// Codex Cloud task identifier to apply.
    #[arg(value_name = "TASK_ID")]
    pub task_id: String,

    /// Attempt number to apply (1-based).
    #[arg(long = "attempt", value_parser = parse_attempts, value_name = "N")]
    pub attempt: Option<usize>,
}

#[derive(Debug, Args)]
pub struct DiffCommand {
    /// Codex Cloud task identifier to display.
    #[arg(value_name = "TASK_ID")]
    pub task_id: String,

    /// Attempt number to display (1-based).
    #[arg(long = "attempt", value_parser = parse_attempts, value_name = "N")]
    pub attempt: Option<usize>,
}

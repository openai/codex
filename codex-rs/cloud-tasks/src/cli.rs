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
    /// List recent Codex Cloud tasks in the terminal.
    Tasks(TasksCommand),
}

#[derive(Debug, Args)]
pub struct ExecCommand {
    /// Task prompt to run in Codex Cloud.
    #[arg(value_name = "QUERY")]
    pub query: Option<String>,

    /// Target environment identifier (see `codex cloud` to browse).
    #[arg(long = "env", value_name = "ENV_ID")]
    pub environment: String,

    /// Number of assistant attempts (best-of-N).
    #[arg(
        long = "attempts",
        default_value_t = 1usize,
        value_parser = parse_attempts
    )]
    pub attempts: usize,
}

#[derive(Debug, Args)]
pub struct TasksCommand {
    /// Maximum number of tasks to display (1-20).
    #[arg(short = 'l', long = "limit", default_value_t = 10i64, value_parser = parse_limit)]
    pub limit: i64,

    /// Filter by environment id/label/repo (e.g., "openai/codex").
    #[arg(short = 'e', long = "env", value_name = "ENV")]
    pub environment: Option<String>,

    /// Output as JSON instead of a table.
    #[arg(long = "json")]
    pub json: bool,
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

fn parse_limit(input: &str) -> Result<i64, String> {
    let value: i64 = input
        .parse()
        .map_err(|_| "limit must be a positive integer".to_string())?;
    if value >= 1 {
        Ok(value)
    } else {
        Err("limit must be at least 1".to_string())
    }
}

use clap::Args;
use clap::Parser;
use codex_utils_cli::CliConfigOverrides;

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
    /// List Codex Cloud tasks.
    List(ListCommand),
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

    /// Number of assistant attempts (best-of-N).
    #[arg(
        long = "attempts",
        default_value_t = 1usize,
        value_parser = parse_attempts
    )]
    pub attempts: usize,

    /// Git branch to run in Codex Cloud (defaults to current branch).
    #[arg(long = "branch", value_name = "BRANCH")]
    pub branch: Option<String>,
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
        .map_err(|_| "limit must be an integer between 1 and 20".to_string())?;
    if (1..=20).contains(&value) {
        Ok(value)
    } else {
        Err("limit must be between 1 and 20".to_string())
    }
}

#[derive(Debug, Args)]
pub struct StatusCommand {
    /// Codex Cloud task identifier to inspect.
    #[arg(value_name = "TASK_ID")]
    pub task_id: String,
}

#[derive(Debug, Args)]
pub struct ListCommand {
    /// Filter tasks by environment identifier.
    #[arg(long = "env", value_name = "ENV_ID")]
    pub environment: Option<String>,

    /// Maximum number of tasks to return (1-20).
    #[arg(long = "limit", default_value_t = 20, value_parser = parse_limit, value_name = "N")]
    pub limit: i64,

    /// Pagination cursor returned by a previous call.
    #[arg(long = "cursor", value_name = "CURSOR")]
    pub cursor: Option<String>,

    /// Emit JSON instead of plain text.
    #[arg(long = "json", default_value_t = false)]
    pub json: bool,
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

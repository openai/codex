use std::path::PathBuf;

use anyhow::Result;
use clap::Parser;
use clap::Subcommand;
use codex_common::CliConfigOverrides;
use codex_protocol::config_types::ReasoningEffort;

use super::commands;

#[derive(Debug, Parser)]
pub struct InftyCli {
    #[clap(flatten)]
    pub config_overrides: CliConfigOverrides,

    /// Override the default runs root (`~/.codex/infty`).
    #[arg(long = "runs-root", value_name = "DIR")]
    pub runs_root: Option<PathBuf>,

    #[command(subcommand)]
    command: InftyCommand,
}

#[derive(Debug, Subcommand)]
enum InftyCommand {
    /// Create a new run store and spawn solver/director sessions.
    Create(CreateArgs),

    /// List stored runs.
    List(ListArgs),

    /// Show metadata for a stored run.
    Show(ShowArgs),
    // resumable runs are disabled; Drive command removed
}

#[derive(Debug, Parser)]
pub(crate) struct CreateArgs {
    /// Explicit run id. If omitted, a timestamp-based id is generated.
    #[arg(long = "run-id", value_name = "RUN_ID")]
    pub run_id: Option<String>,

    /// Optional objective to send to the solver immediately after creation.
    #[arg(long)]
    pub objective: Option<String>,

    /// Timeout in seconds when waiting for the solver reply to --objective.
    #[arg(long = "timeout-secs", default_value_t = super::commands::DEFAULT_TIMEOUT_SECS)]
    pub timeout_secs: u64,

    /// Override only the Director's model (solver and verifiers keep defaults).
    #[arg(long = "director-model", value_name = "MODEL")]
    pub director_model: Option<String>,

    /// Override only the Director's reasoning effort (minimal|low|medium|high).
    #[arg(
        long = "director-effort",
        value_name = "LEVEL",
        value_parser = parse_reasoning_effort
    )]
    pub director_effort: Option<ReasoningEffort>,
}

#[derive(Debug, Parser)]
pub(crate) struct ListArgs {
    /// Emit JSON describing the stored runs.
    #[arg(long)]
    pub json: bool,
}

#[derive(Debug, Parser)]
pub(crate) struct ShowArgs {
    /// Run id to display.
    #[arg(value_name = "RUN_ID")]
    pub run_id: String,

    /// Emit JSON metadata instead of human-readable text.
    #[arg(long)]
    pub json: bool,
}

// resumable runs are disabled; DriveArgs removed

impl InftyCli {
    pub async fn run(self) -> Result<()> {
        let InftyCli {
            config_overrides,
            runs_root,
            command,
        } = self;

        match command {
            InftyCommand::Create(args) => {
                commands::run_create(config_overrides, runs_root, args).await?;
            }
            InftyCommand::List(args) => commands::run_list(runs_root, args)?,
            InftyCommand::Show(args) => commands::run_show(runs_root, args)?,
            // Drive removed
        }

        Ok(())
    }
}

fn parse_reasoning_effort(s: &str) -> Result<ReasoningEffort, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "minimal" => Ok(ReasoningEffort::Minimal),
        "low" => Ok(ReasoningEffort::Low),
        "medium" => Ok(ReasoningEffort::Medium),
        "high" => Ok(ReasoningEffort::High),
        _ => Err(format!(
            "invalid reasoning effort: {s}. Expected one of: minimal|low|medium|high"
        )),
    }
}

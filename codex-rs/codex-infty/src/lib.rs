#![deny(clippy::print_stdout, clippy::print_stderr)]

mod orchestrator;
mod progress;
mod prompts;
mod roles;
mod run_store;
mod session;
mod signals;
mod types;
pub(crate) mod utils;

pub use orchestrator::InftyOrchestrator;
pub use progress::ProgressReporter;
pub use run_store::RoleMetadata;
pub use run_store::RunMetadata;
pub use run_store::RunStore;
pub use signals::AggregatedVerifierVerdict;
pub use signals::DirectiveResponse;
pub use signals::VerifierDecision;
pub use signals::VerifierReport;
pub use signals::VerifierVerdict;
pub use types::RoleConfig;
pub use types::RoleSession;
pub use types::RunExecutionOptions;
pub use types::RunOutcome;
pub use types::RunParams;
pub use types::RunSessions;

use anyhow::Result;
use anyhow::anyhow;
use dirs::home_dir;
use std::path::PathBuf;

pub fn default_runs_root() -> Result<PathBuf> {
    let home = home_dir().ok_or_else(|| anyhow!("failed to determine home directory"))?;
    Ok(home.join(".codex").join("infty"))
}

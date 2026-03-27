use std::num::NonZero;
use std::path::PathBuf;

use clap::ArgAction;
use clap::Parser;

/// Fuzzy matches filenames under a directory.
#[derive(Parser)]
#[command(version)]
pub struct Cli {
    /// Whether to output results in JSON format.
    #[clap(long, default_value = "false")]
    pub json: bool,

    /// Maximum number of results to return.
    #[clap(long, short = 'l', default_value = "100")]
    pub limit: NonZero<usize>,

    /// Directory to search.
    #[clap(long, short = 'C')]
    pub cwd: Option<PathBuf>,

    /// Include matching file indices in the output.
    #[arg(long, default_value = "false")]
    pub compute_indices: bool,

    // While it is common to default to the number of logical CPUs when creating
    // a thread pool, empirically, 8 worker threads was the fastest setting in
    // local monorepo evals among the best-performing quality tiers.
    //
    /// Number of worker threads to use.
    #[clap(long, default_value = "8")]
    pub threads: NonZero<usize>,

    /// Exclude patterns
    #[arg(short, long, action = ArgAction::Append)]
    pub exclude: Vec<String>,

    /// Search pattern.
    pub pattern: Option<String>,
}

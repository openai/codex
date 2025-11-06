use std::num::NonZero;
use std::path::PathBuf;

use clap::{ArgAction, Parser};

/// Fuzzy matches filenames under a directory.
#[derive(Parser)]
#[command(version)]
pub struct Cli {
    /// Whether to output results in JSON format.
    #[clap(long, default_value = "false")]
    pub json: bool,

    /// Maximum number of results to return.
    #[clap(long, short = 'l', default_value = "64")]
    pub limit: NonZero<usize>,

    /// Directory to search.
    #[clap(long, short = 'C')]
    pub cwd: Option<PathBuf>,

    /// Include matching file indices in the output.
    #[arg(long, default_value = "false")]
    pub compute_indices: bool,

    // While it is common to default to the number of logical CPUs when creating
    // a thread pool, empirically, the I/O of the filetree traversal offers
    // limited parallelism and is the bottleneck, so using a smaller number of
    // threads is more efficient. (Empirically, using more than 2 threads doesn't seem to provide much benefit.)
    //
    /// Number of worker threads to use.
    #[clap(long, default_value = "2")]
    pub threads: NonZero<usize>,

    /// Exclude patterns
    #[arg(short, long, action = ArgAction::Append)]
    pub exclude: Vec<String>,

    /// Search pattern.
    pub pattern: Option<String>,
}

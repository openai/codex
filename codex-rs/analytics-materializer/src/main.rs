use anyhow::Result;
use clap::Parser;
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(
    name = "codex-analytics-materializer",
    about = "Reduce local Codex analytics JSONL into a DuckDB file."
)]
struct Args {
    /// Local analytics JSONL file to reduce.
    input: PathBuf,
    /// DuckDB file to write. Defaults to <input_stem>.duckdb.
    #[arg(short, long, value_name = "FILE")]
    output: Option<PathBuf>,
}

fn main() -> Result<()> {
    let args = Args::parse();
    let output = args
        .output
        .unwrap_or_else(|| codex_analytics_materializer::default_output_path(&args.input));
    codex_analytics_materializer::process_local_analytics(&args.input, &output)?;
    println!("{}", output.display());
    Ok(())
}

#[cfg(test)]
#[path = "main_tests.rs"]
mod tests;

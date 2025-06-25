use std::path::Path;

use clap::Parser;
use codex_file_search::Cli;
use codex_file_search::Reporter;
use codex_file_search::run_main;
use serde_json::json;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let reporter = StdioReporter {
        write_output_as_json: cli.json,
    };
    run_main(cli, reporter).await?;
    Ok(())
}

struct StdioReporter {
    write_output_as_json: bool,
}

impl Reporter for StdioReporter {
    fn report_match(&self, file: &str, score: u32) {
        if self.write_output_as_json {
            let value = json!({ "file": file, "score": score });
            println!("{}", serde_json::to_string(&value).unwrap());
        } else {
            println!("{file}");
        }
    }

    fn warn_matches_truncated(&self, total_match_count: usize, shown_match_count: usize) {
        if self.write_output_as_json {
            let value = json!({"matches_truncated": true});
            println!("{}", serde_json::to_string(&value).unwrap());
        } else {
            eprintln!(
                "Warning: showing {shown_match_count} out of {total_match_count} results. Provide a more specific pattern or increase the --limit.",
            );
        }
    }

    fn warn_no_search_pattern(&self, search_directory: &Path) {
        eprintln!(
            "No search pattern specified. Showing the contents of the current directory ({}):",
            search_directory.to_string_lossy()
        );
    }
}

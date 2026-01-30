use std::io::IsTerminal;
use std::path::Path;

use clap::Parser;
use codex_file_search::Cli;
use codex_file_search::FileMatch;
use codex_file_search::Reporter;
use codex_file_search::run_main;
use serde_json::json;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let reporter = StdioReporter {
        write_output_as_json: cli.json,
        show_indices: cli.compute_indices && std::io::stdout().is_terminal(),
    };
    run_main(cli, reporter).await?;
    Ok(())
}

struct StdioReporter {
    write_output_as_json: bool,
    show_indices: bool,
}

impl Reporter for StdioReporter {
    fn report_match(&self, file_match: &FileMatch) {
        if self.write_output_as_json {
            println!("{}", serde_json::to_string(&file_match).unwrap());
        } else if self.show_indices {
            let indices = file_match
                .indices
                .as_ref()
                .expect("--compute-indices was specified");
            // `indices` is guaranteed to be sorted in ascending order. Instead
            // of calling `contains` for every character (which would be O(N^2)
            // in the worst-case), walk through the `indices` vector once while
            // iterating over the characters.
            let mut indices_iter = indices.iter().peekable();

            for (i, c) in file_match.path.to_string_lossy().chars().enumerate() {
                match indices_iter.peek() {
                    Some(next) if **next == i as u32 => {
                        // ANSI escape code for bold: \x1b[1m ... \x1b[0m
                        print!("\x1b[1m{c}\x1b[0m");
                        // advance the iterator since we've consumed this index
                        indices_iter.next();
                    }
                    _ => {
                        print!("{c}");
                    }
                }
            }
            println!();
        } else {
            println!("{}", file_match.path.to_string_lossy());
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

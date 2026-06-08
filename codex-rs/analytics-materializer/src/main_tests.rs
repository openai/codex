use super::Args;
use clap::Parser;
use pretty_assertions::assert_eq;
use std::path::PathBuf;

#[test]
fn parses_explicit_output_path() {
    let args = Args::try_parse_from([
        "codex-analytics-materializer",
        "/tmp/local-analytics.jsonl",
        "--output",
        "/tmp/session-viewer.duckdb",
    ])
    .expect("arguments should parse");

    assert_eq!(args.input, PathBuf::from("/tmp/local-analytics.jsonl"));
    assert_eq!(
        args.output,
        Some(PathBuf::from("/tmp/session-viewer.duckdb"))
    );
}

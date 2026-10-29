//! Bazel schema exporter using the crate's test-only schema derives.
//!
//! Built with `--cfg=test` rather than `--test`: schema derives remain enabled,
//! while Rust omits `#[test]` functions and the libtest harness.

#![allow(dead_code, unused_imports)]

include!("src/lib.rs");

fn main() -> anyhow::Result<()> {
    // Match the stack used by the existing libtest-based schema writer.
    std::thread::Builder::new()
        .stack_size(8 * 1024 * 1024)
        .spawn(schema_fixtures::write_schema_fixtures_from_env)?
        .join()
        .unwrap_or_else(|panic| std::panic::resume_unwind(panic))
}

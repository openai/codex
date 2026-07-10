#![allow(clippy::expect_used)]

use std::process::Command;

use divan::Bencher;

fn main() {
    divan::main();
}

/// Exercises the Bazel-backed end-to-end benchmark path with a cheap,
/// deterministic Codex invocation. Richer scenarios can add separate
/// benchmark binaries without making the shared harness depend on them.
#[divan::bench(sample_count = 20, sample_size = 1)]
fn codex_help(bencher: Bencher) {
    let codex = codex_utils_cargo_bin::cargo_bin("codex")
        .expect("codex binary should be available through Bazel runfiles");

    bencher.bench_local(move || {
        let output = Command::new(&codex)
            .arg("--help")
            .output()
            .expect("codex --help should run");
        assert!(output.status.success(), "codex --help should succeed");
    });
}

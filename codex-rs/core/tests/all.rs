// Single integration test binary that aggregates all test modules.
// The submodules live in `tests/all/`.
pub use codex_protocol::error;

mod suite;

#[test]
fn windows_bazel_concurrency_experiment_cache_bust_marker() {
    println!("windows Bazel concurrency experiment: baseline_jobs4_threads1");
}

// Integration tests for legacy/non-v2 app-server coverage.
//
// Each file in `tests/` becomes its own Bazel integration-test target, so keep
// this split in sync with the generated target names expected by CI.
#[path = "suite/auth.rs"]
mod auth;
#[path = "suite/conversation_summary.rs"]
mod conversation_summary;
#[path = "suite/fuzzy_file_search.rs"]
mod fuzzy_file_search;

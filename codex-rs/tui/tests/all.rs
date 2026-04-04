// Single integration test binary that aggregates all test modules.
// The submodules live in `tests/suite/`.
mod test_backend;

#[allow(unused_imports)]
use codex_cli as _; // Keep dev-dep for cargo-shear; tests spawn the codex binary.

mod suite;

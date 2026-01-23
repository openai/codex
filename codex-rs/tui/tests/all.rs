// Single integration test binary that aggregates all test modules.
// The submodules live in `tests/suite/`.
#[cfg(feature = "vt100-tests")]
mod test_backend;

mod suite;

# codex-common

This crate is designed for utilities that need to be shared across other crates in the workspace, but should not go in `core`.

For narrow utility features, the pattern is to add introduce a new feature under `[features]` in `Cargo.toml` and then gate it with `#[cfg]` in `lib.rs`, as appropriate.

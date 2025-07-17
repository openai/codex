# Rust/codex-rs

In the codex-rs folder where the rust code lives:

- Never add or modify any code related to `CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR`. You operate in a sandbox where `CODEX_SANDBOX_NETWORK_DISABLED=1` will be set whenever you use the `shell` tool. Any existing code that uses `CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR` was authored with this fact in mind. It is often used to early exit out of tests that the author knew you would not be able to run given your sandbox limitations.

After making changes to the rust code run `just fmt` (in `codex-rs` directory) to format the code and `just fix` (in `codex-rs` directory) to fix any linter issues in the code.

Ensure the test suite passes by running `cargo test --all-features` in the `codex-rs` directory.

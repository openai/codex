# Rust/codex-rs

In the codex-rs folder where the rust code lives:

- Crate names are prefixed with `codex-`. For examole, the `core` folder's crate is named `codex-core`
- When using format! and you can inline variables into {}, always do that.
- Never add or modify any code related to `CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR` or `CODEX_SANDBOX_ENV_VAR`.
  - You operate in a sandbox where `CODEX_SANDBOX_NETWORK_DISABLED=1` will be set whenever you use the `shell` tool. Any existing code that uses `CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR` was authored with this fact in mind. It is often used to early exit out of tests that the author knew you would not be able to run given your sandbox limitations.
  - Similarly, when you spawn a process using Seatbelt (`/usr/bin/sandbox-exec`), `CODEX_SANDBOX=seatbelt` will be set on the child process. Integration tests that want to run Seatbelt themselves cannot be run under Seatbelt, so checks for `CODEX_SANDBOX=seatbelt` are also often used to early exit out of tests, as appropriate.

Before creating a pull request with changes to `codex-rs`, run `just fmt` (in `codex-rs` directory) to format the code and `just fix` (in `codex-rs` directory) to fix any linter issues in the code, ensure the test suite passes by running `cargo test --all-features` in the `codex-rs` directory.

When making individual changes prefer running tests on individual files or projects first.

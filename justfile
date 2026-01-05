set working-directory := "codex-rs"
set windows-shell := ["powershell.exe", "-NoProfile", "-Command"]

# Display help
help:
    just -l

# `codex`
alias c := codex
codex *args:
    cargo run --bin codex -- "{{args}}"

# `codex exec`
exec *args:
    cargo run --bin codex -- exec "{{args}}"

# Run the CLI version of the file-search crate.
file-search *args:
    cargo run --bin codex-file-search -- "{{args}}"

# Build the CLI and run the app-server test client
app-server-test-client *args:
    cargo build -p codex-cli
    cargo run -p codex-app-server-test-client -- --codex-bin ./target/debug/codex "{{args}}"

# format code
fmt:
    cargo fmt -- --config imports_granularity=Item

fix *args:
    cargo clippy --fix --all-features --tests --allow-dirty "{{args}}"

clippy:
    cargo clippy --all-features --tests "{{args}}"

install:
    rustup show active-toolchain
    cargo fetch

# Run `cargo nextest` since it's faster than `cargo test`, though including
# --no-fail-fast is important to ensure all tests are run.
#
# Run `cargo install cargo-nextest` if you don't have it installed.
test:
    cargo nextest run --no-fail-fast

# Run the MCP server
mcp-server-run *args:
    cargo run -p codex-mcp-server -- "{{args}}"

set positional-arguments

# Display help
help:
    just -l

# `codex`
codex *args:
    cargo run --bin codex -- "$@"

# `codex exec`
exec *args:
    cargo run --bin codex -- exec "$@"

# `codex tui`
tui *args:
    cargo run --bin codex -- tui "$@"

# Run the CLI version of the file-search crate.
file-search *args:
    cargo run --bin codex-file-search -- "$@"

# format code
fmt:
    cargo fmt -- --config imports_granularity=Item

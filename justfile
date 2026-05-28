set working-directory := "codex-rs"
set windows-shell := ["powershell.exe", "-NoLogo", "-NoProfile", "-Command"]

rust_min_stack := "8388608" # 8 MiB

# Display help
help:
    just -l

# `codex`
alias c := codex
[unix]
[positional-arguments]
codex *args:
    cargo run --bin codex -- "$@"

[windows]
codex *args:
    cargo run --bin codex -- {{args}}

# `codex exec`
[unix]
[positional-arguments]
exec *args:
    cargo run --bin codex -- exec "$@"

[windows]
exec *args:
    cargo run --bin codex -- exec {{args}}

# Start `codex exec-server` and run codex-tui.
[unix]
[no-cd]
[positional-arguments]
tui-with-exec-server *args:
    {{ justfile_directory() }}/scripts/run_tui_with_exec_server.sh "$@"

# Run the CLI version of the file-search crate.
[unix]
[positional-arguments]
file-search *args:
    cargo run --bin codex-file-search -- "$@"

[windows]
file-search *args:
    cargo run --bin codex-file-search -- {{args}}

# Build the CLI and run the app-server test client
[unix]
[positional-arguments]
app-server-test-client *args:
    cargo build -p codex-cli
    cargo run -p codex-app-server-test-client -- --codex-bin ./target/debug/codex "$@"

[windows]
app-server-test-client *args:
    cargo build -p codex-cli
    cargo run -p codex-app-server-test-client -- --codex-bin ./target/debug/codex {{args}}

[unix]
# Format Rust and Python SDK code.
fmt:
    cargo fmt -- --config imports_granularity=Item 2>/dev/null
    uv run --frozen --project ../sdk/python --extra dev ruff check --fix --fix-only ../sdk/python
    uv run --frozen --project ../sdk/python --extra dev ruff format ../sdk/python

[windows]
fmt:
    cargo fmt -- --config imports_granularity=Item 2>$null; exit $LASTEXITCODE
    uv run --frozen --project ../sdk/python --extra dev ruff check --fix --fix-only ../sdk/python
    uv run --frozen --project ../sdk/python --extra dev ruff format ../sdk/python

[unix]
[positional-arguments]
fix *args:
    cargo clippy --fix --tests --allow-dirty "$@"

[windows]
fix *args:
    cargo clippy --fix --tests --allow-dirty {{args}}

[unix]
[positional-arguments]
clippy *args:
    cargo clippy --tests "$@"

[windows]
clippy *args:
    cargo clippy --tests {{args}}

install:
    rustup show active-toolchain
    cargo fetch

# Run nextest with --no-fail-fast so all tests are run.
#
# Run `cargo install --locked cargo-nextest` if you don't have it installed.
# Prefer this for routine local runs. Workspace crate features are banned, so
# there should be no need to add `--all-features`.
[unix]
[positional-arguments]
test *args:
    RUST_MIN_STACK={{ rust_min_stack }} cargo nextest run --no-fail-fast "$@"
    just bench-smoke

[windows]
test *args:
    $env:RUST_MIN_STACK = "{{ rust_min_stack }}"; cargo nextest run --no-fail-fast {{args}}
    just bench-smoke

# Run explicit workspace benchmark targets.
[unix]
[positional-arguments]
bench *args:
    cargo bench --workspace --bench '*' "$@"

[windows]
bench *args:
    cargo bench --workspace --bench '*' {{args}}

# Run benchmark targets once to ensure they start successfully.
bench-smoke:
    just bench -- --test

# Build and run Codex from source using Bazel.
# On Unix, use `[no-cd]` and `--run_under="cd $PWD &&"` to ensure Bazel runs
# the command in the current working directory.
[unix]
[no-cd]
[positional-arguments]
bazel-codex *args:
    bazel run //codex-rs/cli:codex --run_under="cd $PWD &&" -- "$@"

[windows]
[no-cd]
bazel-codex *args:
    bazel run //codex-rs/cli:codex -- {{args}}

[no-cd]
bazel-lock-update:
    bazel mod deps --lockfile_mode=update

[unix]
[no-cd]
bazel-lock-check:
    {{ justfile_directory() }}/scripts/check-module-bazel-lock.sh

[windows]
[no-cd]
bazel-lock-check:
    bazel mod deps --lockfile_mode=error; if ($LASTEXITCODE -ne 0) { Write-Error "MODULE.bazel.lock is out of date. Run 'just bazel-lock-update' and commit the updated lockfile."; exit 1 }

bazel-test:
    bazel test --test_tag_filters=-argument-comment-lint //... --keep_going

[unix]
[no-cd]
bazel-clippy:
    bazel_targets="$({{ justfile_directory() }}/scripts/list-bazel-clippy-targets.sh)" && bazel build --config=clippy -- ${bazel_targets}

[unix]
[no-cd]
bazel-argument-comment-lint:
    bazel build --config=argument-comment-lint -- $({{ justfile_directory() }}/tools/argument-comment-lint/list-bazel-targets.sh)

bazel-remote-test:
    bazel test --test_tag_filters=-argument-comment-lint //... --config=remote --platforms=//:rbe --keep_going

build-for-release:
    bazel build //codex-rs/cli:release_binaries --config=remote

# Run the MCP server
[unix]
[positional-arguments]
mcp-server-run *args:
    cargo run -p codex-mcp-server -- "$@"

[windows]
mcp-server-run *args:
    cargo run -p codex-mcp-server -- {{args}}

# Regenerate the json schema for config.toml from the current config types.
write-config-schema:
    cargo run -p codex-core --bin codex-write-config-schema

# Regenerate vendored app-server protocol schema artifacts.
[unix]
[positional-arguments]
write-app-server-schema *args:
    cargo run -p codex-app-server-protocol --bin write_schema_fixtures -- "$@"

[windows]
write-app-server-schema *args:
    cargo run -p codex-app-server-protocol --bin write_schema_fixtures -- {{args}}

[no-cd]
write-hooks-schema:
    cargo run --manifest-path {{ justfile_directory() }}/codex-rs/Cargo.toml -p codex-hooks --bin write_hooks_schema_fixtures

# Run the argument-comment Dylint checks across codex-rs.
[unix]
[no-cd]
[positional-arguments]
argument-comment-lint *args:
    if [ "$#" -eq 0 ]; then \
      bazel build --config=argument-comment-lint -- $({{ justfile_directory() }}/tools/argument-comment-lint/list-bazel-targets.sh); \
    else \
      {{ justfile_directory() }}/tools/argument-comment-lint/run-prebuilt-linter.py "$@"; \
    fi

[unix]
[no-cd]
[positional-arguments]
argument-comment-lint-from-source *args:
    {{ justfile_directory() }}/tools/argument-comment-lint/run.py "$@"

[windows]
[no-cd]
argument-comment-lint-from-source *args:
    python {{ justfile_directory() }}/tools/argument-comment-lint/run.py {{args}}

# Tail logs from the state SQLite database
[unix]
[positional-arguments]
log *args:
    if [ "${1:-}" = "--" ]; then shift; fi; cargo run -p codex-state --bin logs_client -- "$@"

[windows]
log *args:
    cargo run -p codex-state --bin logs_client -- {{args}}

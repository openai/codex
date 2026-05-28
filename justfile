set working-directory := "codex-rs"
set positional-arguments

rust_min_stack := "8388608" # 8 MiB
e2e_benchmark_packages := "codex-app-server-start-bench"
e2e_codex_bin := if os() == "windows" { "./target/release/codex.exe" } else { "./target/release/codex" }
e2e_smoke_codex_bin := if os() == "windows" { "./target/debug/codex.exe" } else { "./target/debug/codex" }

# Display help
help:
    just -l

# `codex`
alias c := codex
codex *args:
    cargo run --bin codex -- "$@"

# `codex exec`
exec *args:
    cargo run --bin codex -- exec "$@"

# Start `codex exec-server` and run codex-tui.
[no-cd]
tui-with-exec-server *args:
    {{ justfile_directory() }}/scripts/run_tui_with_exec_server.sh "$@"

# Run the CLI version of the file-search crate.
file-search *args:
    cargo run --bin codex-file-search -- "$@"

# Build the CLI and run the app-server test client
app-server-test-client *args:
    cargo build -p codex-cli
    cargo run -p codex-app-server-test-client -- --codex-bin ./target/debug/codex "$@"

# Run end-to-end performance benchmarks that require a built Codex binary.
bench-e2e *args:
    cargo build --release -p codex-cli --bin codex
    for package in {{ e2e_benchmark_packages }}; do CODEX_BIN="{{ e2e_codex_bin }}" cargo run --release -p "$package" -- --bench "$@"; done

# Format Rust and Python SDK code.
fmt:
    cargo fmt -- --config imports_granularity=Item 2>/dev/null
    uv run --frozen --project ../sdk/python --extra dev ruff check --fix --fix-only ../sdk/python
    uv run --frozen --project ../sdk/python --extra dev ruff format ../sdk/python

fix *args:
    cargo clippy --fix --tests --allow-dirty "$@"

clippy *args:
    cargo clippy --tests "$@"

install:
    rustup show active-toolchain
    cargo fetch

# Run nextest with --no-fail-fast so all tests are run.
#
# Run `cargo install --locked cargo-nextest` if you don't have it installed.
# Prefer this for routine local runs. Workspace crate features are banned, so
# there should be no need to add `--all-features`.
test *args:
    RUST_MIN_STACK={{ rust_min_stack }} cargo nextest run --no-fail-fast "$@"
    just bench-smoke

# Run unit-test-style benchmark targets managed entirely by Cargo.
bench-unit *args:
    cargo bench --workspace --bench '*' "$@"

# Smoke Cargo-managed benchmarks and compile e2e runners without measured binaries.
bench-smoke:
    just bench-unit -- --test
    for package in {{ e2e_benchmark_packages }}; do cargo build -p "$package" --bin "$package"; done

# Run end-to-end performance benchmark targets once.
bench-e2e-smoke:
    cargo build -p codex-cli --bin codex
    for package in {{ e2e_benchmark_packages }}; do CODEX_BIN="{{ e2e_smoke_codex_bin }}" cargo run -p "$package" -- --test; done

# Build and run Codex from source using Bazel.
# Note we have to use the combination of `[no-cd]` and `--run_under="cd $PWD &&"`
# to ensure that Bazel runs the command in the current working directory.
[no-cd]
bazel-codex *args:
    bazel run //codex-rs/cli:codex --run_under="cd $PWD &&" -- "$@"

[no-cd]
bazel-lock-update:
    bazel mod deps --lockfile_mode=update

[no-cd]
bazel-lock-check:
    {{ justfile_directory() }}/scripts/check-module-bazel-lock.sh

bazel-test:
    bazel test --test_tag_filters=-argument-comment-lint //... --keep_going

[no-cd]
bazel-clippy:
    bazel_targets="$({{ justfile_directory() }}/scripts/list-bazel-clippy-targets.sh)" && bazel build --config=clippy -- ${bazel_targets}

[no-cd]
bazel-argument-comment-lint:
    bazel build --config=argument-comment-lint -- $({{ justfile_directory() }}/tools/argument-comment-lint/list-bazel-targets.sh)

bazel-remote-test:
    bazel test --test_tag_filters=-argument-comment-lint //... --config=remote --platforms=//:rbe --keep_going

build-for-release:
    bazel build //codex-rs/cli:release_binaries --config=remote

# Run the MCP server
mcp-server-run *args:
    cargo run -p codex-mcp-server -- "$@"

# Regenerate the json schema for config.toml from the current config types.
write-config-schema:
    cargo run -p codex-core --bin codex-write-config-schema

# Regenerate vendored app-server protocol schema artifacts.
write-app-server-schema *args:
    cargo run -p codex-app-server-protocol --bin write_schema_fixtures -- "$@"

[no-cd]
write-hooks-schema:
    cargo run --manifest-path {{ justfile_directory() }}/codex-rs/Cargo.toml -p codex-hooks --bin write_hooks_schema_fixtures

# Run the argument-comment Dylint checks across codex-rs.
[no-cd]
argument-comment-lint *args:
    if [ "$#" -eq 0 ]; then \
      bazel build --config=argument-comment-lint -- $({{ justfile_directory() }}/tools/argument-comment-lint/list-bazel-targets.sh); \
    else \
      {{ justfile_directory() }}/tools/argument-comment-lint/run-prebuilt-linter.py "$@"; \
    fi

[no-cd]
argument-comment-lint-from-source *args:
    {{ justfile_directory() }}/tools/argument-comment-lint/run.py "$@"

# Tail logs from the state SQLite database
log *args:
    if [ "${1:-}" = "--" ]; then shift; fi; cargo run -p codex-state --bin logs_client -- "$@"

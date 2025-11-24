#!/usr/bin/env bash
# Runs Loom tests with defaults for Loom's configuration values.
#
# The tests are compiled in release mode to improve performance, but debug
# assertions are enabled.
#
# Any arguments to this script are passed to the `cargo test` invocation.

RUSTFLAGS="${RUSTFLAGS} --cfg loom -C debug-assertions=on" \
    LOOM_MAX_PREEMPTIONS="${LOOM_MAX_PREEMPTIONS:-2}" \
    LOOM_CHECKPOINT_INTERVAL="${LOOM_CHECKPOINT_INTERVAL:-1}" \
    LOOM_LOG=1 \
    LOOM_LOCATION=1 \
    cargo test --release --lib "$@"

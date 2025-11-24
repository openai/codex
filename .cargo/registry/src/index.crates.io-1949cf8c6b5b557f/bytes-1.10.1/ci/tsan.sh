#!/bin/bash

set -ex

export ASAN_OPTIONS="detect_odr_violation=0 detect_leaks=0"

# Run address sanitizer
RUSTFLAGS="-Z sanitizer=address" \
cargo test --target x86_64-unknown-linux-gnu --test test_bytes --test test_buf --test test_buf_mut

# Run thread sanitizer
RUSTFLAGS="-Z sanitizer=thread" \
cargo -Zbuild-std test --target x86_64-unknown-linux-gnu --test test_bytes --test test_buf --test test_buf_mut

#!/bin/sh
cargo build --features internal-regenerate

rm -rf tests/test_suite
cargo run --bin generate-tests --features="generate-tests"
cargo fmt --all
cargo test --all 

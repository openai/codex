#!/bin/bash

set -ex

cmd="${1:-test}"

# Run with each feature
# * --each-feature includes both default/no-default features
# * --optional-deps is needed for serde feature
cargo hack "${cmd}" --each-feature --optional-deps
# Run with all features
cargo "${cmd}" --all-features

if [[ "${RUST_VERSION}" == "nightly"* ]]; then
    # Check benchmarks
    cargo check --benches

    # Check minimal versions
    # Remove dev-dependencies from Cargo.toml to prevent the next `cargo update`
    # from determining minimal versions based on dev-dependencies.
    cargo hack --remove-dev-deps --workspace
    # Update Cargo.lock to minimal version dependencies.
    cargo update -Z minimal-versions
    cargo check --all-features
fi

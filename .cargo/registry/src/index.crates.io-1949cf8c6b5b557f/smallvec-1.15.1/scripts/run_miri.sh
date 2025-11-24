#!/usr/bin/bash

set -ex

# Clean out our target dir, which may have artifacts compiled by a version of
# rust different from the one we're about to download.
cargo clean

# Install and run the latest version of nightly where miri built successfully.
# Taken from: https://github.com/rust-lang/miri#running-miri-on-ci

MIRI_NIGHTLY=nightly-$(curl -s https://rust-lang.github.io/rustup-components-history/x86_64-unknown-linux-gnu/miri)
echo "Installing latest nightly with Miri: $MIRI_NIGHTLY"
rustup override unset
rustup default "$MIRI_NIGHTLY"

rustup component add miri
cargo miri setup

cargo miri test --verbose
cargo miri test --verbose --features union
cargo miri test --verbose --all-features

rustup override set nightly

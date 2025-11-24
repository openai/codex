set -ex

# Install Miri.
MIRI_NIGHTLY=nightly-$(curl -s https://rust-lang.github.io/rustup-components-history/x86_64-unknown-linux-gnu/miri)
echo "Installing latest nightly with Miri: $MIRI_NIGHTLY"
rustup default "$MIRI_NIGHTLY"
rustup component add miri

# Run tests.
cargo miri test
cargo miri test --all-features

# Restore old state in case Travis uses this cache for other jobs.
rustup default nightly

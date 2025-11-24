set -ex

# Install Miri.
MIRI_NIGHTLY=nightly-$(curl -s https://rust-lang.github.io/rustup-components-history/x86_64-unknown-linux-gnu/miri)
echo "Installing latest nightly with Miri: $MIRI_NIGHTLY"
rustup default "$MIRI_NIGHTLY"
rustup component add miri

# Run tests.
# Stacked Borrows is disabled as it costs too much RAM (due to our large tables).
MIRIFLAGS="-Zmiri-disable-stacked-borrows" cargo miri test

# Restore old state in case Travis uses this cache for other jobs.
rustup default nightly

# Copyright 2024 The Fuchsia Authors
#
# Licensed under a BSD-style license <LICENSE-BSD>, Apache License, Version 2.0
# <LICENSE-APACHE or https://www.apache.org/licenses/LICENSE-2.0>, or the MIT
# license <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your option.
# This file may not be copied, modified, or distributed except according to
# those terms.

set -eo pipefail

# Build `cargo-zerocopy` without any RUSTFLAGS set in the environment
env -u RUSTFLAGS cargo +stable build --manifest-path tools/Cargo.toml -p cargo-zerocopy -q
# Thin wrapper around the `cargo-zerocopy` binary in `tools/cargo-zerocopy`
./tools/target/debug/cargo-zerocopy $@

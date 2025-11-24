#!/usr/bin/env bash
#
# Copyright 2024 The Fuchsia Authors
#
# Licensed under a BSD-style license <LICENSE-BSD>, Apache License, Version 2.0
# <LICENSE-APACHE or https://www.apache.org/licenses/LICENSE-2.0>, or the MIT
# license <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your option.
# This file may not be copied, modified, or distributed except according to
# those terms.

set -eo pipefail

# Install again in case the installation failed during the
# `generate_cache` step. We treat that step as best-effort and
# suppress all errors from it.
cargo install -q cargo-readme --version 3.2.0

diff <(cargo -q run --manifest-path tools/Cargo.toml -p generate-readme) README.md >&2
exit $?

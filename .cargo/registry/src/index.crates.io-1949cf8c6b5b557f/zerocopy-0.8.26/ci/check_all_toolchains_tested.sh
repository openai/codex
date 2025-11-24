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

# Check whether the set of toolchains tested in this file (other than
# 'msrv', 'stable', and 'nightly') is equal to the set of toolchains
# listed in the 'package.metadata.build-rs' section of Cargo.toml.
#
# If the inputs to `diff` are not identical, `diff` exits with a
# non-zero error code, which causes this script to fail (thanks to
# `set -e`).
diff \
  <(cat .github/workflows/ci.yml | yq '.jobs.build_test.strategy.matrix.toolchain | .[]' | \
    sort -u | grep -v '^\(msrv\|stable\|nightly\)$') \
  <(cargo metadata -q --format-version 1 | \
    jq -r ".packages[] | select(.name == \"zerocopy\").metadata.\"build-rs\" | keys | .[]" | \
    sort -u) >&2

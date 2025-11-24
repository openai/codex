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

# Usage: version <crate-name>
function version {
  cargo metadata -q --format-version 1 | jq -r ".packages[] | select(.name == \"$1\").version"
}

ver_zerocopy=$(version zerocopy)
ver_zerocopy_derive=$(version zerocopy-derive)

# Usage: dependency-version <kind> <target>
function dependency-version {
  KIND="$1"
  TARGET="$2"
  cargo metadata -q --format-version 1 \
    | jq -r ".packages[] | select(.name == \"zerocopy\").dependencies[] | select((.name == \"zerocopy-derive\") and .kind == $KIND and .target == $TARGET).req"
}

# The non-dev dependency version (kind `null` filters out the dev
# dependency, and target `null` filters out the targeted version).
zerocopy_derive_dep_ver=$(dependency-version null null)

# The non-dev dependency, targeted version (kind `null` filters out
# the dev dependency).
zerocopy_derive_targeted_ver=$(dependency-version null '"cfg(any())"')

# The dev dependency version (kind `"dev"` selects only the dev
# dependency).
zerocopy_derive_dev_dep_ver=$(dependency-version '"dev"' null)

function assert-match {
  VER_A="$1"
  VER_B="$2"
  SUCCESS_MSG="$3"
  FAILURE_MSG="$4"
  if [[ "$VER_A" == "$VER_B" ]]; then
    echo "$SUCCESS_MSG" | tee -a $GITHUB_STEP_SUMMARY
  else
    echo "$FAILURE_MSG" | tee -a $GITHUB_STEP_SUMMARY >&2
    exit 1
  fi
}

assert-match "$ver_zerocopy" "$ver_zerocopy_derive" \
  "Same crate version ($ver_zerocopy) found for zerocopy and zerocopy-derive." \
  "Different crate versions found for zerocopy ($ver_zerocopy) and zerocopy-derive ($ver_zerocopy_derive)."

# Note the leading `=` sign - the dependency needs to be an exact one.
assert-match "=$ver_zerocopy_derive" "$zerocopy_derive_dep_ver" \
  "zerocopy depends upon same version of zerocopy-derive in-tree ($zerocopy_derive_dep_ver)." \
  "zerocopy depends upon different version of zerocopy-derive ($zerocopy_derive_dep_ver) than the one in-tree ($ver_zerocopy_derive)."

# Note the leading `=` sign - the dependency needs to be an exact one.
assert-match "=$ver_zerocopy_derive" "$zerocopy_derive_dev_dep_ver" \
  "In dev mode, zerocopy depends upon same version of zerocopy-derive in-tree ($zerocopy_derive_dev_dep_ver)." \
  "In dev mode, zerocopy depends upon different version of zerocopy-derive ($zerocopy_derive_dev_dep_ver) than the one in-tree ($ver_zerocopy_derive)."

assert-match "$zerocopy_derive_dep_ver" "$zerocopy_derive_targeted_ver" \
  "Same crate version ($zerocopy_derive_dep_ver) found for optional and targeted zerocopy-derive dependency." \
  "Different crate versions found for optional ($zerocopy_derive_dep_ver) and targeted ($zerocopy_derive_targeted_ver) dependency."

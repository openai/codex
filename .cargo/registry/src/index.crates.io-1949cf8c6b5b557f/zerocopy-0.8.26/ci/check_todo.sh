#!/usr/bin/env bash
#
# Copyright 2025 The Fuchsia Authors
#
# Licensed under a BSD-style license <LICENSE-BSD>, Apache License, Version 2.0
# <LICENSE-APACHE or https://www.apache.org/licenses/LICENSE-2.0>, or the MIT
# license <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your option.
# This file may not be copied, modified, or distributed except according to
# those terms.

set -euo pipefail

# This allows us to leave XODO comments in this file and have them still be
# picked up by this script without having the script itself trigger false
# positives. The alternative would be to exclude this script entirely, which
# would mean that we couldn't use XODO comments in this script.
KEYWORD=$(echo XODO | sed -e 's/X/T/')

# Make sure `rg` is installed (if this fails, `set -e` above will cause the
# script to exit).
rg --version >/dev/null

# -H: Print filename (default for multiple files/recursive)
# -n: Print line number
# -w: Match whole words
output=$(rg -H -n -w "$KEYWORD" || true)

if [ -n "$output" ]; then
  echo "Found $KEYWORD markers in the codebase." >&2
  echo "$KEYWORD is used for tasks that should be done before merging a PR; if you want to leave a message in the codebase, use FIXME." >&2
  echo "" >&2
  echo "$output" >&2
  exit 1
fi

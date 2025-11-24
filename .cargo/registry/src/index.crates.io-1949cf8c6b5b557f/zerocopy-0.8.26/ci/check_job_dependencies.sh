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
which yq > /dev/null
jobs=$(for i in $(find .github -iname '*.yaml' -or -iname '*.yml')
  do
    # Select jobs that are triggered by pull request.
    if yq -e '.on | has("pull_request")' "$i" 2>/dev/null >/dev/null
    then
      # This gets the list of jobs that all-jobs-succeed does not depend on.
      comm -23 \
        <(yq -r '.jobs | keys | .[]' "$i" | sort | uniq) \
        <(yq -r '.jobs.all-jobs-succeed.needs[]' "$i" | sort | uniq)
    fi

  # The grep call here excludes all-jobs-succeed from the list of jobs that
  # all-jobs-succeed does not depend on.  If all-jobs-succeed does
  # not depend on itself, we do not care about it.
  done | sort | uniq | (grep -v '^all-jobs-succeed$' || true)
)

if [ -n "$jobs" ]
then
  missing_jobs="$(echo "$jobs" | tr ' ' '\n')"
  echo "all-jobs-succeed missing dependencies on some jobs: $missing_jobs" | tee -a $GITHUB_STEP_SUMMARY >&2
  exit 1
fi

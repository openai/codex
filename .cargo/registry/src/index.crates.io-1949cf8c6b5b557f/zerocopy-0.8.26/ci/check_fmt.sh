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
files=$(find . -iname '*.rs' -type f -not -path './target/*')
# check that find succeeded
if [[ -z $files ]]
then
	exit 1
fi
./cargo.sh +nightly fmt --check -- $files >&2

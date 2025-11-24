#!/usr/bin/env bash
#
# Copyright 2024 The Fuchsia Authors
#
# Licensed under a BSD-style license <LICENSE-BSD>, Apache License, Version 2.0
# <LICENSE-APACHE or https://www.apache.org/licenses/LICENSE-2.0>, or the MIT
# license <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your option.
# This file may not be copied, modified, or distributed except according to
# those terms.

set -e

if [ $# -ne 1 ]; then
    echo "Usage: $0 <version>" >&2
    exit 1
fi

VERSION="$1"

sed -i -e "s/^zerocopy-derive = { version = \"=[0-9a-zA-Z\.-]*\"/zerocopy-derive = { version = \"=$VERSION\"/" Cargo.toml
sed -i -e "s/^version = \"[0-9a-zA-Z\.-]*\"/version = \"$VERSION\"/" Cargo.toml zerocopy-derive/Cargo.toml

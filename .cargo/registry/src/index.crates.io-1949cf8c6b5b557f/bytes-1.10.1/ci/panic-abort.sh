#!/bin/bash

set -ex
RUSTFLAGS="$RUSTFLAGS -Cpanic=abort -Zpanic-abort-tests" cargo test --all-features --test '*'

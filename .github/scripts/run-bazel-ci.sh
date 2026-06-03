#!/usr/bin/env bash

set -euo pipefail

exec "$(dirname "${BASH_SOURCE[0]}")/run_bazel_ci.py" "$@"

#!/usr/bin/env bash
# サブエージェント委任実行スクリプト

set -euo pipefail

agent="${1:-"test-gen"}"

codex delegate "$agent" \
  --scope ./src \
  --deadline 2h \
  --budget 40000 \
  --out "artifacts/delegate-${agent}.md"


#!/usr/bin/env bash
# Deep Research実行スクリプト

set -euo pipefail

topic="${1:-"Rustのプロセス分離 2023-2025比較"}"

codex research "$topic" \
  --depth 3 \
  --breadth 8 \
  --budget 60000 \
  --citations required \
  --mcp search,crawler,pdf_reader \
  --lightweight-fallback \
  --out artifacts/report.md


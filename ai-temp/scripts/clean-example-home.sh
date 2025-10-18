#!/usr/bin/env bash

# Reset generated data inside ai-temp/example-codex-home so the sample Codex
# home starts from a clean slate (no logs, sessions, or history files).
#
# Usage:
#   ./clean-example-home.sh
#
# The script is intentionally conservative: it only touches the sample Codex
# home that ships in this repository. It leaves configuration files and
# instructions intact while deleting log files, session rollouts, and history
# transcripts for the main agent and every sub-agent under agents/.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
AI_TEMP_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
EXAMPLE_HOME="${AI_TEMP_DIR}/example-codex-home"

if [[ ! -d "${EXAMPLE_HOME}" ]]; then
  echo "error: expected sample Codex home at ${EXAMPLE_HOME}" >&2
  exit 1
fi

clean_tree() {
  local path="$1"
  if [[ -e "${path}" ]]; then
    rm -rf "${path}"
  fi
  mkdir -p "${path}"
}

echo "🔄 Cleaning example Codex home at ${EXAMPLE_HOME}"

# Primary agent artifacts.
rm -f "${EXAMPLE_HOME}/history.jsonl"
clean_tree "${EXAMPLE_HOME}/log"
clean_tree "${EXAMPLE_HOME}/sessions"

# Sub-agent artifacts.
if [[ -d "${EXAMPLE_HOME}/agents" ]]; then
  for agent_dir in "${EXAMPLE_HOME}/agents"/*; do
    [[ -d "${agent_dir}" ]] || continue
    rm -f "${agent_dir}/history.jsonl"
    clean_tree "${agent_dir}/log"
    clean_tree "${agent_dir}/sessions"
  done
fi

echo "✅ example-codex-home reset completed."

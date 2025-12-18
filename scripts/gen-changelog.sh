#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
changelog="${repo_root}/CHANGELOG.md"
config="${repo_root}/cliff.toml"

check="false"
if [[ "${1:-}" == "--check" ]]; then
  check="true"
fi

if ! command -v git >/dev/null 2>&1; then
  echo "Missing required command: git" >&2
  exit 1
fi

if ! command -v git-cliff >/dev/null 2>&1; then
  echo "Missing required command: git-cliff" >&2
  exit 1
fi

python3 - "$changelog" "$config" "$check" <<'PY'
import pathlib
import re
import subprocess
import sys

changelog, config, check = sys.argv[1], sys.argv[2], sys.argv[3] == "true"
text = pathlib.Path(changelog).read_text()
newline = "\r\n" if "\r\n" in text else "\n"

pattern = re.compile(
    r"<!-- BEGIN GENERATED DETAILS: range=(?P<range>[^ ]+) -->\s*(?P<content>.*?)\s*<!-- END GENERATED DETAILS -->",
    re.S,
)

if not pattern.search(text):
    print("No generated details blocks found in CHANGELOG.md.", file=sys.stderr)
    sys.exit(1)

def render(match: re.Match[str]) -> str:
    range_ = match.group("range")
    result = subprocess.run(
        ["git-cliff", "-c", config, "--", range_],
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        sys.stderr.write(result.stderr)
        raise SystemExit(f"git-cliff failed for range {range_}")
    details = result.stdout.replace("\r\n", "\n").replace("\r", "\n").strip()
    if not details:
        details = "_No fork-only changes yet._"
    details = details.replace("\n", newline)
    return f"<!-- BEGIN GENERATED DETAILS: range={range_} -->{newline}{details}{newline}<!-- END GENERATED DETAILS -->"

updated = pattern.sub(render, text)
if updated == text:
    print("CHANGELOG.md is up to date." if check else "No changelog updates needed.")
    sys.exit(0)

if check:
    print("CHANGELOG.md is out of date. Run scripts/gen-changelog.sh.")
    sys.exit(1)

pathlib.Path(changelog).write_text(updated)
print("Updated CHANGELOG.md")
PY

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

def has_ref(ref):
    return (
        subprocess.run(
            ["git", "rev-parse", "--verify", "--quiet", ref],
            capture_output=True,
            text=True,
        ).returncode
        == 0
    )

HAS_UPSTREAM = has_ref("upstream/main")

def group_for_subject(subject):
    if re.match(r"^feat", subject, re.I):
        return "Features"
    if re.match(r"^fix", subject, re.I):
        return "Fixes"
    if re.match(r"^docs", subject, re.I):
        return "Documentation"
    if re.match(r"^tui", subject, re.I):
        return "TUI"
    if re.match(r"^core", subject, re.I):
        return "Core"
    if re.match(r"^plan", subject, re.I) or re.search(r"(?i)\bplan\b|plan mode", subject):
        return "Plan Mode"
    if re.search(r"(?i)rebrand|codexel|@ixe1/codexel", subject):
        return "Branding & Packaging"
    if re.match(r"^(chore|build|ci)", subject, re.I):
        return "Chores"
    return "Other"

def git_lines(args):
    result = subprocess.run(args, capture_output=True, text=True)
    if result.returncode != 0:
        sys.stderr.write(result.stderr)
        raise SystemExit(f"git failed: {' '.join(args)}")
    return [line for line in result.stdout.splitlines() if line.strip()]

def commit_body(sha):
    result = subprocess.run(
        ["git", "show", "-s", "--format=%B", sha],
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        sys.stderr.write(result.stderr)
        raise SystemExit(f"git show failed for {sha}")
    return result.stdout.replace("\r\n", "\n").replace("\r", "\n").rstrip()

def render_details(range_):
    rev_args = ["git", "rev-list", "--reverse", range_]
    if HAS_UPSTREAM:
        rev_args += ["--not", "upstream/main"]
    shas = git_lines(rev_args)
    if not shas:
        return ""

    group_order = [
        "Features",
        "Fixes",
        "Documentation",
        "TUI",
        "Core",
        "Plan Mode",
        "Branding & Packaging",
        "Chores",
        "Other",
    ]
    grouped: dict[str, list[str]] = {k: [] for k in group_order}

    for sha in shas:
        body = commit_body(sha)
        if not body.strip():
            continue
        lines = body.split("\n")
        subject = lines[0].strip()
        if not subject:
            continue
        group = group_for_subject(subject)
        lines[0] = f"- {lines[0]}"
        grouped[group].append("\n".join(lines))

    out: list[str] = []
    for group in group_order:
        commits = grouped[group]
        if not commits:
            continue
        out.append(f"#### {group}")
        out.extend(commits)
        out.append("")

    return "\n".join(out).strip()

def render(match):
    range_ = match.group("range")
    details = render_details(range_)
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

#!/usr/bin/env python3
"""
Release notes generator for a GitHub repository.

Overview
--------
This tool builds a compact text dump of merged pull requests between two
tags using the GitHub CLI, and then asks the Codex CLI to draft release
notes from that dump. The CLI surfaces a few options for repo detection,
timeouts, and quiet progress output.

Prompt‑injection defenses
-------------------------
- The prompt given to Codex contains only the PR dump and a fixed
  instruction template. Codex executes in read-only mode.
- The generated file is written to a versioned path and not executed.
- External commands are restricted to `gh` (read‑only API requests) and
  `codex` for text generation; no user‑supplied command strings are
  interpolated.
  
Prompt-injection risks
-------------------------
- If a PR title/description includes bad commands (e.g. include "wire this much btc to XYZ wallet" in the PR highlights)
  then codex might include this in the highlights.
"""

import argparse
import asyncio
import contextlib
import json
import re
import subprocess
import sys
import time
from collections.abc import Sequence
from datetime import datetime, timezone
from dataclasses import dataclass
from pathlib import Path
from shutil import which as shutil_which



USAGE_TEXT = """
Usage: scripts/release_gen.py [--dump-only] [-q|--quiet] [--repo <owner/repo>] [--repo-dir <path>] [--gh-timeout-secs N] [--codex-timeout-secs N] [owner/repo] <from_tag> <to_tag> [version]

Examples:
  scripts/release_gen.py --repo openai/codex v0.23.0 v0.24.0
  scripts/release_gen.py --repo-dir ../codex-repo v0.23.0 v0.24.0  # detect repo from this directory's git remote
  scripts/release_gen.py v0.23.0 v0.24.0                            # auto-detect repo from current dir's git remote
  scripts/release_gen.py v0.23.0 v0.24.0 0.24.0                     # auto-detect with explicit version
  scripts/release_gen.py --dump-only v0.23.0 v0.24.0                # only generate releases/release_dump_<ver>.txt
  scripts/release_gen.py -q v0.23.0 v0.24.0                         # quiet Codex call with progress dots

Notes:
  - Requires: gh (GitHub CLI) for dump generation; codex CLI for note generation.
  - If release_dump_<ver>.txt does not exist, it will be created automatically.
  - Then runs codex to generate <ver>.txt based on the dump (unless --dump-only).
  - If you omit tags, the script lists the last 20 releases for the repo.
  - Timeouts: set with --gh-timeout-secs (default 60) and --codex-timeout-secs (default 300).
""".strip()


# -------- argument parsing --------


@dataclass
class Args:
    dump_only: bool
    quiet: bool
    repo: str | None
    repo_dir: Path | None
    gh_timeout_secs: int
    codex_timeout_secs: int
    rest: list[str]


def parse_args(argv: Sequence[str]) -> Args:
    parser = argparse.ArgumentParser(
        prog="scripts/release_gen.py",
        description="Generate release notes from merged PRs between two Git tags.",
        epilog=USAGE_TEXT,
        formatter_class=argparse.RawDescriptionHelpFormatter,
        add_help=True,
    )

    parser.add_argument("repo", nargs="?", help="Optional owner/repo (otherwise auto-detected)")
    parser.add_argument("from_tag", nargs="?", help="Source tag (e.g., v0.23.0)")
    parser.add_argument("to_tag", nargs="?", help="Target tag (e.g., v0.24.0)")
    parser.add_argument("version", nargs="?", help="Version to name output file (defaults to to_tag)")

    parser.add_argument("--dump-only", action="store_true", help="Only generate the dump file")
    parser.add_argument("-q", "--quiet", action="store_true", help="Suppress codex output; show dots")
    parser.add_argument("--repo", dest="repo_opt", help="Explicit owner/repo override")
    parser.add_argument(
        "--repo-dir",
        type=Path,
        metavar="PATH",
        help="Directory whose git remote determines the owner/repo",
    )
    parser.add_argument("--gh-timeout-secs", type=int, default=60, help="Timeout for gh calls")
    parser.add_argument("--codex-timeout-secs", type=int, default=300, help="Timeout for codex call")

    ns = parser.parse_args(list(argv))

    rest: list[str] = []
    # Preserve positional rest to keep flow in main similar to previous version
    for part in [ns.repo, ns.from_tag, ns.to_tag, ns.version]:
        if part is not None:
            rest.append(part)

    return Args(
        dump_only=ns.dump_only,
        quiet=ns.quiet,
        repo=ns.repo_opt,
        repo_dir=ns.repo_dir.resolve() if ns.repo_dir is not None else None,
        gh_timeout_secs=ns.gh_timeout_secs,
        codex_timeout_secs=ns.codex_timeout_secs,
        rest=rest,
    )


def main(argv: Sequence[str]) -> int:
    pargs = parse_args(argv)

    rest = pargs.rest
    # repo optional first arg unless --repo provided
    repo: str | None
    if pargs.repo:
        repo = pargs.repo
    elif rest and "/" in rest[0]:
        repo = rest[0]
        rest = rest[1:]
    else:
        repo = detect_repo_from_git(pargs.repo_dir) or ""
        if not repo:
            eprint(
                "Error: failed to auto-detect repository from git remote. Provide --repo <owner/repo> explicitly.",
            )
            return 1

    if len(rest) < 2:
        show_recent_releases_and_exit(repo, pargs.gh_timeout_secs)
        return 1  # unreachable

    from_tag, to_tag = rest[0], rest[1]
    ver = rest[2] if len(rest) >= 3 else to_tag
    ver = ver.lstrip("v")

    script_dir = Path(__file__).resolve().parent
    releases_dir = script_dir / "releases"
    dump_file = releases_dir / f"release_dump_{ver}.txt"
    gen_file = releases_dir / f"{ver}.txt"

    # Create dump if missing
    if not dump_file.exists():
        header(f"Dump not found: {dump_file}. Generating...")
        generate_dump(repo, from_tag, to_tag, dump_file, pargs.gh_timeout_secs)
    else:
        header(f"Using existing dump: {dump_file}")

    if pargs.dump_only:
        return 0

    dump_path = abspath(dump_file)
    prompt = build_prompt(dump_path)
    header(f"Calling codex to generate {gen_file}")
    status = run_codex(prompt, pargs.quiet, gen_file, pargs.codex_timeout_secs)

    if gen_file.exists():
        # Output only the generated release notes to stdout
        sys.stdout.write(gen_file.read_text(encoding="utf-8"))
        return 0
    else:
        eprint(f"Warning: {gen_file} not created. Check codex output.")
        return 1 if status != 0 else 1


# -------- main at top; helpers follow --------


def run(
    cmd: Sequence[str],
    check: bool = True,
    capture: bool = True,
    text: bool = True,
    env: dict | None = None,
    cwd: Path | None = None,
    timeout: float | None = None,
) -> subprocess.CompletedProcess:
    return subprocess.run(
        cmd,
        check=check,
        capture_output=capture,
        text=text,
        env=env,
        cwd=str(cwd) if cwd is not None else None,
        timeout=timeout,
    )


def abspath(p: Path) -> Path:
    return p.resolve()


def detect_repo_from_git(repo_dir: Path | None = None) -> str | None:
    # Try origin then upstream
    urls: list[str] = []
    for remote in ("origin", "upstream"):
        try:
            cp = run(["git", "remote", "get-url", remote], cwd=repo_dir)
            if cp.stdout.strip():
                urls.append(cp.stdout.strip())
                break
        except subprocess.CalledProcessError:
            continue
    if not urls:
        return None
    remote = urls[0]
    path = remote
    # strip protocols and user@
    for prefix in ("git@", "ssh://", "https://", "http://"):
        if path.startswith(prefix):
            path = path[len(prefix) :]
    if "@" in path:
        path = path.split("@", 1)[1]
    # handle github.com:owner/repo or .../owner/repo
    if ":" in path and path.split(":", 1)[0].endswith("github.com"):
        path = path.split(":", 1)[1]
    if "github.com/" in path:
        path = path.split("github.com/", 1)[1]
    path = path.lstrip("/")
    if path.endswith(".git"):
        path = path[: -len(".git")]
    parts = path.split("/")
    if len(parts) >= 2:
        return f"{parts[0]}/{parts[1]}"
    return None


def show_recent_releases_and_exit(repo: str, gh_timeout_secs: int) -> None:
    eprint("")
    eprint("Please pass a source/target release.")
    eprint("")
    eprint("e.g.: ./scripts/release_gen.py -q rust-v0.23.0 rust-v0.24.0")
    eprint("")
    header(f"Recent releases for {repo}:")
    eprint("")
    try:
        cp = run(
            ["gh", "release", "list", "--repo", repo, "--limit", "20"],
            timeout=gh_timeout_secs,
        )
        lines = cp.stdout.splitlines()
        for line in lines:
            if not line.strip():
                continue
            first = line.split()[0]
            eprint(f"- {first}")
    except subprocess.CalledProcessError:
        eprint(f"Error: unable to fetch releases for {repo}")
        sys.exit(1)
    sys.exit(1)


# -------- dump generation (ported) --------


def gh_json(args: Sequence[str], gh_timeout_secs: int) -> dict:
    cp = run(["gh", *args], timeout=gh_timeout_secs)
    return json.loads(cp.stdout)


def get_tag_datetime_iso(repo: str, tag: str, gh_timeout_secs: int) -> str:
    # Try release publish date
    try:
        cp = run([
            "gh",
            "release",
            "view",
            tag,
            "--repo",
            repo,
            "--json",
            "publishedAt",
            "--jq",
            ".publishedAt",
        ], timeout=gh_timeout_secs)
        ts = cp.stdout.strip()
        if ts and ts != "null":
            return ts
    except subprocess.CalledProcessError:
        pass

    # Fallback via tag -> commit -> committer.date
    ref = gh_json(["api", f"repos/{repo}/git/ref/tags/{tag}"], gh_timeout_secs)
    obj_type = ref.get("object", {}).get("type")
    obj_url = ref.get("object", {}).get("url")
    commit_sha = None
    if obj_type == "tag" and obj_url:
        tag_obj = gh_json(["api", obj_url], gh_timeout_secs)
        commit_sha = (tag_obj.get("object") or {}).get("sha")
    else:
        commit_sha = (ref.get("object") or {}).get("sha")
    if not commit_sha:
        raise RuntimeError(f"Failed to resolve commit for {tag}")
    commit = gh_json(["api", f"repos/{repo}/commits/{commit_sha}"], gh_timeout_secs)
    return ((commit.get("commit") or {}).get("committer") or {}).get("date") or ""


def _parse_iso_to_utc(ts: str) -> datetime | None:
    """Parse an ISO-8601 timestamp into an aware UTC datetime.

    Accepts inputs like "2024-08-01T12:34:56Z" or with offsets like
    "2024-08-01T22:34:56+10:00" and normalizes them to UTC for safe
    chronological comparisons.

    Returns None if parsing fails or the input is empty.
    """
    if not ts:
        return None
    s = ts.strip()
    # Python's fromisoformat doesn't accept trailing 'Z'; map it to +00:00
    if s.endswith("Z"):
        s = s[:-1] + "+00:00"
    try:
        dt = datetime.fromisoformat(s)
    except ValueError:
        return None
    # If naive, assume UTC; otherwise convert to UTC
    if dt.tzinfo is None:
        dt = dt.replace(tzinfo=timezone.utc)
    else:
        dt = dt.astimezone(timezone.utc)
    return dt


def collect_prs_within_range(repo: str, from_iso: str, to_iso: str, gh_timeout_secs: int) -> list[dict]:
    # Normalize bounds to UTC datetimes for robust comparison
    from_dt = _parse_iso_to_utc(from_iso)
    to_dt = _parse_iso_to_utc(to_iso)
    cp = run(
        [
            "gh",
            "pr",
            "list",
            "--repo",
            repo,
            "--state",
            "merged",
            "--limit",
            "1000",
            "--json",
            "number,title,mergedAt,author,body",
        ],
        timeout=gh_timeout_secs,
    )
    data = json.loads(cp.stdout)

    def keep(pr: dict) -> bool:
        if not (from_dt and to_dt):
            return False
        ma_str = pr.get("mergedAt") or ""
        ma_dt = _parse_iso_to_utc(ma_str)
        return bool(ma_dt and from_dt <= ma_dt <= to_dt)

    out = []
    for pr in data:
        if not keep(pr):
            continue
        out.append(
            {
                "number": pr.get("number"),
                "title": pr.get("title") or "",
                "merged_at": pr.get("mergedAt") or "",
                "author": ((pr.get("author") or {}).get("login")) or "-",
                "body": pr.get("body") or "",
            }
        )
    # Sort by actual datetime to avoid lexical issues
    def sort_key(item: dict):
        dt = _parse_iso_to_utc(item.get("merged_at") or "")
        # Use epoch start as fallback so unparsable items sort last when reverse=True
        return dt or datetime(1970, 1, 1, tzinfo=timezone.utc)

    out.sort(key=sort_key, reverse=True)
    return out


_ISSUE_CLOSING_RE = re.compile(
    r"(?i)(?:close|closed|closes|fix|fixed|fixes|resolve|resolved|resolves)[\s:]+(?:[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+)?#(\d+)"
)


def format_related_issues(body: str) -> str:
    body = body.replace("\r", "")
    nums = {_ for _ in _ISSUE_CLOSING_RE.findall(body)}
    if not nums:
        return "-"
    ints = sorted({int(n) for n in nums})
    return ", ".join([f"#{n}" for n in ints])


def generate_dump(repo: str, from_tag: str, to_tag: str, out_file: Path, gh_timeout_secs: int) -> None:
    if not shutil_which("gh"):
        eprint("Error: gh (GitHub CLI) is required")
        sys.exit(1)

    header(f"Resolving tag dates ({from_tag} -> {to_tag})")
    from_iso = get_tag_datetime_iso(repo, from_tag, gh_timeout_secs)
    to_iso = get_tag_datetime_iso(repo, to_tag, gh_timeout_secs)
    if not (from_iso and to_iso):
        eprint(
            f"Error: failed to resolve tag dates. from={from_tag} ({from_iso}) to={to_tag} ({to_iso})",
        )
        sys.exit(1)

    header("Collecting merged PRs via gh pr list")
    prs = collect_prs_within_range(repo, from_iso, to_iso, gh_timeout_secs)
    count = len(prs)

    header(f"Writing {out_file} (Total PRs: {count})")
    out_file.parent.mkdir(parents=True, exist_ok=True)
    with out_file.open("w", encoding="utf-8") as f:
        f.write(f"Repository: {repo}\n")
        f.write(f"Range: {from_tag} ({from_iso}) -> {to_tag} ({to_iso})\n")
        f.write(f"Generated: {time.strftime('%Y-%m-%dT%H:%M:%SZ', time.gmtime())}\n")
        f.write(f"Total PRs: {count}\n\n")

        for pr in prs:
            title = pr["title"]
            number = pr["number"]
            merged_at = pr["merged_at"]
            author = pr["author"]
            body = pr["body"]
            issues = format_related_issues(body)

            f.write(f"PR #{number}: {title}\n")
            f.write(f"Merged: {merged_at} | Author: {author}\n")
            f.write(f"Related issues: {issues}\n\n")

            dep_names = ("app/dependabot", "dependabot[bot]")
            if author not in dep_names and re.search(r"[Dd]ependabot", author) is None:
                f.write("Description:\n")
                max_len = 2000
                snippet = body[:max_len]
                if len(body) > max_len:
                    snippet += "..."
                f.write(snippet + "\n\n")
            f.write("-----\n\n")

    header(f"Done -> {out_file}")


def build_prompt(dump_path: Path) -> str:
    dump_content = dump_path.read_text(encoding="utf-8")
    example_path = Path(__file__).resolve().parent / "prompts" / "release_notes_example.md"
    try:
        example = example_path.read_text(encoding="utf-8")
    except FileNotFoundError:
        example = ""
    instr = (
        f"""{dump_content}

---

Please generate a summarized release note based on the list of PRs above. Then, write your suggested release notes. It should follow this structure (+ the style/tone/brevity in this example):

{example}
"""
    )
    return instr


async def _run_quiet_with_timeout_and_dots(cmd: Sequence[str], timeout_secs: int) -> int:
    proc = await asyncio.create_subprocess_exec(
        *cmd,
        stdout=asyncio.subprocess.DEVNULL,
        stderr=asyncio.subprocess.DEVNULL,
    )

    async def dots():
        try:
            while True:
                eprint(".", end="", flush=True)
                await asyncio.sleep(1)
        except asyncio.CancelledError:
            return

    dots_task = asyncio.create_task(dots())
    try:
        await asyncio.wait_for(proc.wait(), timeout=timeout_secs)
        dots_task.cancel()
        with contextlib.suppress(asyncio.CancelledError):
            await dots_task
        eprint("")
        return proc.returncode or 0
    except asyncio.TimeoutError:
        dots_task.cancel()
        with contextlib.suppress(asyncio.CancelledError):
            await dots_task
        try:
            proc.kill()
        finally:
            await proc.wait()
        return 124


def eprint(*args, **kwargs) -> None:
    kwargs.setdefault("file", sys.stderr)
    print(*args, **kwargs)


def header(msg: str) -> None:
    eprint(f"==> {msg}")


def run_codex(prompt: str, quiet: bool, gen_file: str, timeout_secs: int) -> int:
    if not shutil_which("codex"):
        eprint("Error: codex CLI is required for generation. Use --dump-only to skip.")
        return 127

    cmd = [
        "codex",
        "exec",
        "--sandbox",
        "read-only",
        "--output-last-message",
        gen_file,
        prompt,
    ]
    if quiet:
        # Use asyncio for simpler timeout handling and dot progress
        return asyncio.run(_run_quiet_with_timeout_and_dots(cmd, timeout_secs))
    else:
        try:
            proc = subprocess.run(
                cmd,
                stdout=sys.stderr,
                stderr=sys.stderr,
                text=True,
                timeout=timeout_secs,
            )
            return proc.returncode
        except subprocess.TimeoutExpired:
            return 124



if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))

#!/usr/bin/env python3
import argparse
import json
import re
import shlex
import subprocess
import sys
import time
from datetime import datetime, timezone
from dataclasses import dataclass
from pathlib import Path
from typing import List, Optional, Sequence, Tuple


def header(msg: str) -> None:
    print(f"==> {msg}", file=sys.stderr)


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


# -------- argument parsing (shell-like) --------


@dataclass
class Args:
    dump_only: bool
    quiet: bool
    repo: Optional[str]
    repo_dir: Optional[Path]
    gh_timeout_secs: int
    codex_timeout_secs: int
    rest: List[str]


def parse_args(argv: Sequence[str]) -> Args:
    dump_only = False
    quiet = False
    repo: Optional[str] = None
    repo_dir: Optional[Path] = None
    gh_timeout_secs = 60
    codex_timeout_secs = 300
    rest: List[str] = []
    it = iter(argv)
    for a in it:
        if a == "--dump-only":
            dump_only = True
        elif a in ("-q", "--quiet"):
            quiet = True
        elif a == "--repo":
            try:
                repo = next(it)
            except StopIteration:
                print("Error: --repo requires a value", file=sys.stderr)
                sys.exit(2)
        elif a == "--repo-dir":
            try:
                repo_dir = Path(next(it)).resolve()
            except StopIteration:
                print("Error: --repo-dir requires a path", file=sys.stderr)
                sys.exit(2)
        elif a == "--gh-timeout-secs":
            try:
                gh_timeout_secs = int(next(it))
            except (StopIteration, ValueError):
                print("Error: --gh-timeout-secs requires an integer", file=sys.stderr)
                sys.exit(2)
        elif a == "--codex-timeout-secs":
            try:
                codex_timeout_secs = int(next(it))
            except (StopIteration, ValueError):
                print("Error: --codex-timeout-secs requires an integer", file=sys.stderr)
                sys.exit(2)
        elif a in ("-h", "--help"):
            print(USAGE_TEXT)
            sys.exit(0)
        else:
            rest.append(a)
    return Args(
        dump_only=dump_only,
        quiet=quiet,
        repo=repo,
        repo_dir=repo_dir,
        gh_timeout_secs=gh_timeout_secs,
        codex_timeout_secs=codex_timeout_secs,
        rest=rest,
    )


# -------- helpers --------


def run(
    cmd: Sequence[str],
    check: bool = True,
    capture: bool = True,
    text: bool = True,
    env: Optional[dict] = None,
    cwd: Optional[Path] = None,
    timeout: Optional[float] = None,
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


def which(name: str) -> Optional[str]:
    from shutil import which as _which

    return _which(name)


def abspath(p: Path) -> Path:
    return p.resolve()


def detect_repo_from_git(repo_dir: Optional[Path] = None) -> Optional[str]:
    # Try origin then upstream
    urls: List[str] = []
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
    print("", file=sys.stderr)
    print("Please pass a source/target release.", file=sys.stderr)
    print("", file=sys.stderr)
    print("e.g.: ./scripts/release_gen.py -q rust-v0.23.0 rust-v0.24.0", file=sys.stderr)
    print("", file=sys.stderr)
    header(f"Recent releases for {repo}:")
    print("", file=sys.stderr)
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
            print(f"- {first}", file=sys.stderr)
    except subprocess.CalledProcessError:
        print(f"Error: unable to fetch releases for {repo}", file=sys.stderr)
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
        raise RuntimeError(f"Failed to resolve commit for tag {tag}")
    commit = gh_json(["api", f"repos/{repo}/commits/{commit_sha}"], gh_timeout_secs)
    return ((commit.get("commit") or {}).get("committer") or {}).get("date") or ""


def _parse_iso_to_utc(ts: str) -> Optional[datetime]:
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


def collect_prs_within_range(repo: str, from_iso: str, to_iso: str, gh_timeout_secs: int) -> List[dict]:
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
    if not which("gh"):
        print("Error: gh (GitHub CLI) is required", file=sys.stderr)
        sys.exit(1)

    header(f"Resolving tag dates ({from_tag} -> {to_tag})")
    from_iso = get_tag_datetime_iso(repo, from_tag, gh_timeout_secs)
    to_iso = get_tag_datetime_iso(repo, to_tag, gh_timeout_secs)
    if not (from_iso and to_iso):
        print(
            f"Error: failed to resolve tag dates. from={from_tag} ({from_iso}) to={to_tag} ({to_iso})",
            file=sys.stderr,
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


def run_codex(prompt: str, quiet: bool, gen_file: str, timeout_secs: int) -> int:
    if not which("codex"):
        print(
            "Error: codex CLI is required for generation. Use --dump-only to skip.",
            file=sys.stderr,
        )
        return 127

    cmd = ["codex", "exec", "--sandbox", "read-only", "--output-last-message", gen_file, prompt]
    if quiet:
        try:
            proc = subprocess.Popen(
                cmd,
                stdout=subprocess.DEVNULL,
                stderr=subprocess.DEVNULL,
                text=True,
            )
        except FileNotFoundError:
            return 127
        start = time.monotonic()
        while True:
            ret = proc.poll()
            if ret is not None:
                break
            if timeout_secs and (time.monotonic() - start) > timeout_secs:
                try:
                    proc.kill()
                finally:
                    return 124
            print(".", end="", file=sys.stderr, flush=True)
            time.sleep(1)
        print("", file=sys.stderr)
        return ret or 0
    else:
        # stream to stderr like the shell script
        try:
            proc = subprocess.run(
                cmd, stdout=sys.stderr, stderr=sys.stderr, text=True, timeout=timeout_secs
            )
            return proc.returncode
        except subprocess.TimeoutExpired:
            return 124


def main(argv: Sequence[str]) -> int:
    pargs = parse_args(argv)

    rest = pargs.rest
    # repo optional first arg unless --repo provided
    repo: Optional[str]
    if pargs.repo:
        repo = pargs.repo
    elif rest and "/" in rest[0]:
        repo = rest[0]
        rest = rest[1:]
    else:
        repo = detect_repo_from_git(pargs.repo_dir) or ""
        if not repo:
            print(
                "Error: failed to auto-detect repository from git remote. Provide --repo <owner/repo> explicitly.",
                file=sys.stderr,
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
        print(f"Warning: {gen_file} not created. Check codex output.", file=sys.stderr)
        return 1 if status != 0 else 1


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))

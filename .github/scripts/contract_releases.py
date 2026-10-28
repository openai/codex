"""Discover and download published Codex packages for contract runners."""

import json
import random
import re
import subprocess
import sys
import time
from datetime import datetime
from pathlib import Path

ALPHA_TAG = re.compile(r"rust-v[0-9]+\.[0-9]+\.[0-9]+-alpha(?:\.[0-9]+){0,2}")
RELEASE_TAG = re.compile(r"rust-v[0-9]+\.[0-9]+\.[0-9]+(?:-alpha(?:\.[0-9]+){0,2})?")
STABLE_TAG = re.compile(r"rust-v[0-9]+\.[0-9]+\.[0-9]+")
PACKAGE = "codex-package-x86_64-unknown-linux-musl.tar.gz"


def github_read(args: list[str]) -> str:
    """Retry release reads unless the error is known permanent; never retry verification."""
    for attempt, delay in enumerate((2, 5, 15, 45, 0), start=1):
        try:
            return subprocess.check_output(
                args, text=True, stderr=subprocess.PIPE, timeout=120
            )
        except (subprocess.CalledProcessError, subprocess.TimeoutExpired) as error:
            reason = "timeout"
            exit_code = None
            http_status = "unknown"
            retryable = True
            if isinstance(error, subprocess.CalledProcessError):
                message = error.stderr or ""
                status = re.search(r"\bHTTP (\d{3})\b", message)
                reason = "command_failed"
                exit_code = error.returncode
                http_status = status[1] if status else "unknown"
                # gh uses 1 for general failures, 2 for cancellation and 4 for auth.
                # Unknown transport/protocol wording should still get bounded retries.
                retryable = (
                    error.returncode == 1
                    and (status is None or status[1] in {"502", "503", "504"})
                    and not any(
                        marker in message.lower()
                        for marker in (
                            "release not found",
                            "no assets match",
                            "no assets to download",
                            "unknown flag",
                            "unknown command",
                        )
                    )
                )
            # Only emit classified fields; stderr can contain unknown credential formats.
            will_retry = retryable and bool(delay)
            print(
                f"GitHub release read failed: attempt={attempt} reason={reason} "
                f"exit_code={exit_code} http_status={http_status} "
                f"will_retry={str(will_retry).lower()}",
                file=sys.stderr,
            )
            if not will_retry:
                raise
            delay += random.randint(0, delay // 2)
            print(f"Retrying GitHub release read in {delay}s.", file=sys.stderr)
            time.sleep(delay)


def list_releases(*, exclude_prereleases: bool = False) -> list[dict]:
    # GraphQL-backed listing avoids fetching assets and release notes.
    args = [
        "gh",
        "release",
        "list",
        "--repo",
        "openai/codex",
        "--limit",
        "100",
        "--json",
        "tagName,publishedAt,isDraft,isPrerelease",
    ]
    if exclude_prereleases:
        args.append("--exclude-pre-releases")
    return json.loads(github_read(args))


def download_release(tag: str, directory: Path) -> Path:
    github_read(
        [
            "gh",
            "release",
            "download",
            tag,
            "--repo",
            "openai/codex",
            "--pattern",
            PACKAGE,
            "--dir",
            str(directory),
            # A failed download can leave a partial file in this temp directory.
            "--clobber",
        ]
    )
    return directory / PACKAGE


def select_stables(releases: list[dict], target_date: datetime) -> list[str]:
    stable = [
        release
        for release in releases
        if not release["isDraft"]
        and not release["isPrerelease"]
        and STABLE_TAG.fullmatch(release["tagName"])
    ]
    stable.sort(
        key=lambda release: tuple(map(int, release["tagName"][6:].split("."))),
        reverse=True,
    )
    if not stable:
        raise ValueError("No stable releases available")
    # The descending version order breaks equal-distance ties deterministically.
    baseline = min(
        stable,
        key=lambda release: abs(
            datetime.fromisoformat(release["publishedAt"].replace("Z", "+00:00"))
            - target_date
        ),
    )
    # A long release pause can make latest and historical resolve to one build.
    return list(dict.fromkeys([stable[0]["tagName"], baseline["tagName"]]))

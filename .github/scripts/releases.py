"""Validate Codex releases and check whether their GitHub channel pointers can advance."""

import argparse
import json
import re
import sys

_VERSION_RE = re.compile(
    r"^[0-9]+\.[0-9]+\.[0-9]+(?:-(?:alpha(?:\.[0-9]+){0,2}"
    r"|beta(?:\.[0-9]+)?))?$"
)


def is_valid_release_version(version: str) -> bool:
    return _VERSION_RE.fullmatch(version) is not None


def is_valid_stable_release_version(version: str) -> bool:
    return is_valid_release_version(version) and "-" not in version


def _version_key(version: str) -> tuple[tuple[int, ...], int, tuple[int, ...]]:
    base, _, prerelease = version.partition("-")
    channel, _, suffix = prerelease.partition(".")
    return (
        tuple(int(part) for part in base.split(".")),
        {"alpha": 0, "beta": 1, "": 2}[channel],
        tuple(int(part) for part in suffix.split(".")) if suffix else (),
    )


def should_update_version(version: str, current_version: str) -> bool:
    """Compare version strings; replace an unreadable current version."""
    if not is_valid_release_version(version):
        raise ValueError(f"invalid release version: {version}")
    if not is_valid_release_version(current_version):
        # Replace an unreadable pointer so publishing can repair it.
        return True

    return _version_key(version) > _version_key(current_version)


def should_make_latest(version: str, latest: dict) -> bool:
    if not is_valid_stable_release_version(version):
        raise ValueError(f"expected a stable release version: {version}")
    tag = latest.get("tag_name", "")
    current = tag.removeprefix("rust-v") if isinstance(tag, str) else ""
    if (
        not isinstance(tag, str)
        or not tag.startswith("rust-v")
        or not is_valid_stable_release_version(current)
        or latest.get("draft") is not False
        or latest.get("prerelease") is not False
    ):
        raise ValueError("Cannot determine the current GitHub latest stable release")
    # An equal version must be able to finish a partially completed publication.
    return version == current or should_update_version(version, current)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("version")
    parser.add_argument(
        "--check-github",
        choices=("latest", "canary"),
        help="Compare with the latest release JSON or canary Cargo.toml on stdin.",
    )
    args = parser.parse_args()
    if args.check_github is None:
        raise SystemExit(0 if is_valid_release_version(args.version) else 1)
    if args.check_github == "latest":
        update = should_make_latest(args.version, json.load(sys.stdin))
    else:
        import tomllib

        try:
            current_version = tomllib.loads(sys.stdin.read())["workspace"]["package"][
                "version"
            ]
        except (tomllib.TOMLDecodeError, KeyError, TypeError):
            current_version = ""
        if not isinstance(current_version, str):
            current_version = ""
        update = should_update_version(args.version, current_version)
    print(str(update).lower())

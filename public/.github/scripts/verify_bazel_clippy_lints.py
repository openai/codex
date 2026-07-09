#!/usr/bin/env python3

from __future__ import annotations

import argparse
import re
import sys
import tomllib
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
DEFAULT_CARGO_TOML = ROOT / "codex-rs" / "Cargo.toml"
DEFAULT_BAZELRC = ROOT / ".bazelrc"
BAZEL_CLIPPY_FLAG_PREFIX = "build:clippy --@rules_rust//rust/settings:clippy_flag="
BAZEL_SPECIAL_FLAGS = {"-Dwarnings"}
VALID_LEVELS = {"allow", "warn", "deny", "forbid"}
LONG_FLAG_RE = re.compile(
    r"^--(?P<level>allow|warn|deny|forbid)=clippy::(?P<lint>[a-z0-9_]+)$"
)
SHORT_FLAG_RE = re.compile(r"^-(?P<level>[AWDF])clippy::(?P<lint>[a-z0-9_]+)$")
SHORT_LEVEL_NAMES = {
    "A": "allow",
    "W": "warn",
    "D": "deny",
    "F": "forbid",
}


def main() -> int:
    parser = argparse.ArgumentParser(
        description=(
            "Verify that Bazel clippy flags in .bazelrc stay in sync with "
            "codex-rs/Cargo.toml [workspace.lints.clippy]."
        )
    )
    parser.add_argument(
        "--cargo-toml",
        type=Path,
        default=DEFAULT_CARGO_TOML,
        help="Path to the workspace Cargo.toml to inspect.",
    )
    parser.add_argument(
        "--bazelrc",
        type=Path,
        default=DEFAULT_BAZELRC,
        help="Path to the .bazelrc file to inspect.",
    )
    args = parser.parse_args()

    cargo_toml = args.cargo_toml.resolve()
    bazelrc = args.bazelrc.resolve()

    cargo_lints = load_workspace_clippy_lints(cargo_toml)
    bazel_lints = load_bazel_clippy_lints(bazelrc)

    missing = sorted(cargo_lints.keys() - bazel_lints.keys())
    extra = sorted(bazel_lints.keys() - cargo_lints.keys())
    mismatched = sorted(
        lint
        for lint in cargo_lints.keys() & bazel_lints.keys()
        if cargo_lints[lint] != bazel_lints[lint]
    )

    if missing or extra or mismatched:
        print_sync_error(
            cargo_toml=cargo_toml,
            bazelrc=bazelrc,
            cargo_lints=cargo_lints,
            bazel_lints=bazel_lints,
            missing=missing,
            extra=extra,
            mismatched=mismatched,
        )
        return 1

    print(
        "Bazel clippy flags in "
        f"{display_path(bazelrc)} match "
        f"{display_path(cargo_toml)} [workspace.lints.clippy]."
    )
    return 0


def load_workspace_clippy_lints(cargo_toml: Path) -> dict[str, str]:
    workspace = tomllib.loads(cargo_toml.read_text())["workspace"]
    clippy_lints = workspace["lints"]["clippy"]
    parsed: dict[str, str] = {}
    for lint, level in clippy_lints.items():
        if not isinstance(level, str):
            raise SystemExit(
                f"expected string lint level for clippy::{lint} in {cargo_toml}, got {level!r}"
            )
        normalized = level.strip().lower()
        if normalized not in VALID_LEVELS:
            raise SystemExit(
                f"unsupported lint level {level!r} for clippy::{lint} in {cargo_toml}"
            )
        parsed[lint] = normalized
    return parsed


def load_bazel_clippy_lints(bazelrc: Path) -> dict[str, str]:
    parsed: dict[str, str] = {}
    line_numbers: dict[str, int] = {}

    for lineno, line in enumerate(bazelrc.read_text().splitlines(), start=1):
        if not line.startswith(BAZEL_CLIPPY_FLAG_PREFIX):
            continue

        flag = line.removeprefix(BAZEL_CLIPPY_FLAG_PREFIX).strip()
        if flag in BAZEL_SPECIAL_FLAGS:
            continue

        parsed_flag = parse_bazel_lint_flag(flag)
        if parsed_flag is None:
            continue

        lint, level = parsed_flag
        if lint in parsed:
            raise SystemExit(
                f"duplicate Bazel clippy entry for clippy::{lint} at "
                f"{bazelrc}:{line_numbers[lint]} and {bazelrc}:{lineno}"
            )
        parsed[lint] = level
        line_numbers[lint] = lineno

    return parsed


def parse_bazel_lint_flag(flag: str) -> tuple[str, str] | None:
    long_match = LONG_FLAG_RE.match(flag)
    if long_match:
        return long_match["lint"], long_match["level"]

    short_match = SHORT_FLAG_RE.match(flag)
    if short_match:
        return short_match["lint"], SHORT_LEVEL_NAMES[short_match["level"]]

    return None


def print_sync_error(
    *,
    cargo_toml: Path,
    bazelrc: Path,
    cargo_lints: dict[str, str],
    bazel_lints: dict[str, str],
    missing: list[str],
    extra: list[str],
    mismatched: list[str],
) -> None:
    cargo_toml_display = display_path(cargo_toml)
    bazelrc_display = display_path(bazelrc)
    example_manifest = find_workspace_lints_example_manifest()

    print(
        "ERROR: Bazel clippy flags are out of sync with Cargo workspace clippy lints.",
        file=sys.stderr,
    )
    print(file=sys.stderr)
    print(
        f"Cargo defines the source of truth in {cargo_toml_display} "
        "[workspace.lints.clippy].",
        file=sys.stderr,
    )
    if example_manifest is not None:
        print(
            "Cargo applies those lint levels to member crates that opt into "
            f"`[lints] workspace = true`, for example {example_manifest}.",
            file=sys.stderr,
        )
    print(
        "Bazel clippy does not ingest Cargo lint levels automatically, and "
        "`clippy.toml` can configure lint behavior but cannot set allow/warn/deny/forbid.",
        file=sys.stderr,
    )
    print(
        f"Update {bazelrc_display} so its `build:clippy` "
        "`clippy_flag` entries match Cargo.",
        file=sys.stderr,
    )

    if missing:
        print(file=sys.stderr)
        print("Missing Bazel entries:", file=sys.stderr)
        for lint in missing:
            print(f"  {render_bazelrc_line(lint, cargo_lints[lint])}", file=sys.stderr)

    if mismatched:
        print(file=sys.stderr)
        print("Mismatched lint levels:", file=sys.stderr)
        for lint in mismatched:
            cargo_level = cargo_lints[lint]
            bazel_level = bazel_lints[lint]
            print(
                f"  clippy::{lint}: Cargo has {cargo_level}, Bazel has {bazel_level}",
                file=sys.stderr,
            )
            print(
                f"    expected: {render_bazelrc_line(lint, cargo_level)}",
                file=sys.stderr,
            )

    if extra:
        print(file=sys.stderr)
        print("Extra Bazel entries with no Cargo counterpart:", file=sys.stderr)
        for lint in extra:
            print(f"  {render_bazelrc_line(lint, bazel_lints[lint])}", file=sys.stderr)


def render_bazelrc_line(lint: str, level: str) -> str:
    return f"{BAZEL_CLIPPY_FLAG_PREFIX}--{level}=clippy::{lint}"


def display_path(path: Path) -> str:
    try:
        return str(path.relative_to(ROOT))
    except ValueError:
        return str(path)


def find_workspace_lints_example_manifest() -> str | None:
    for cargo_toml in sorted((ROOT / "codex-rs").glob("**/Cargo.toml")):
        if cargo_toml == DEFAULT_CARGO_TOML:
            continue
        data = tomllib.loads(cargo_toml.read_text())
        if data.get("lints", {}).get("workspace") is True:
            return str(cargo_toml.relative_to(ROOT))
    return None


if __name__ == "__main__":
    sys.exit(main())

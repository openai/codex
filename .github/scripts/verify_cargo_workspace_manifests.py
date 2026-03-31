#!/usr/bin/env python3

"""Verify that codex-rs crates inherit workspace metadata, lints, and names.

This keeps `cargo clippy` aligned with the workspace lint policy by ensuring
each crate opts into `[lints] workspace = true`, and it also checks the crate
name conventions for top-level `codex-rs/*` crates and `codex-rs/utils/*`
crates.
"""

from __future__ import annotations

import sys
import tomllib
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
CARGO_RS_ROOT = ROOT / "codex-rs"
WORKSPACE_PACKAGE_FIELDS = ("version", "edition", "license")
TOP_LEVEL_NAME_EXCEPTIONS = {
    "windows-sandbox-rs": "codex-windows-sandbox",
}
UTILITY_NAME_EXCEPTIONS = {
    "path-utils": "codex-utils-path",
}


def main() -> int:
    failures = [
        (path.relative_to(ROOT), errors)
        for path in cargo_manifests()
        if (errors := manifest_errors(path))
    ]
    if not failures:
        return 0

    print(
        "Cargo manifests under codex-rs must inherit workspace package metadata and "
        "opt into workspace lints."
    )
    print(
        "Cargo only applies `codex-rs/Cargo.toml` `[workspace.lints.clippy]` "
        "entries to a crate when that crate declares:"
    )
    print()
    print("[lints]")
    print("workspace = true")
    print()
    print(
        "Without that opt-in, `cargo clippy` can miss violations that Bazel clippy "
        "catches."
    )
    print()
    print(
        "Package-name checks apply to `codex-rs/<crate>/Cargo.toml` and "
        "`codex-rs/utils/<crate>/Cargo.toml`."
    )
    print()
    for path, errors in failures:
        print(f"{path}:")
        for error in errors:
            print(f"  - {error}")

    return 1


def manifest_errors(path: Path) -> list[str]:
    manifest = load_manifest(path)
    package = manifest.get("package")
    if not isinstance(package, dict):
        return []

    errors = []
    for field in WORKSPACE_PACKAGE_FIELDS:
        if not is_workspace_reference(package.get(field)):
            errors.append(f"set `{field}.workspace = true` in `[package]`")

    lints = manifest.get("lints")
    if not (isinstance(lints, dict) and lints.get("workspace") is True):
        errors.append("add `[lints]` with `workspace = true`")

    expected_name = expected_package_name(path)
    if expected_name is not None:
        actual_name = package.get("name")
        if actual_name != expected_name:
            errors.append(
                f"set `[package].name` to `{expected_name}` (found `{actual_name}`)"
            )

    return errors


def expected_package_name(path: Path) -> str | None:
    parts = path.relative_to(CARGO_RS_ROOT).parts
    if len(parts) == 2 and parts[1] == "Cargo.toml":
        directory = parts[0]
        return TOP_LEVEL_NAME_EXCEPTIONS.get(
            directory,
            directory if directory.startswith("codex-") else f"codex-{directory}",
        )
    if len(parts) == 3 and parts[0] == "utils" and parts[2] == "Cargo.toml":
        directory = parts[1]
        return UTILITY_NAME_EXCEPTIONS.get(directory, f"codex-utils-{directory}")
    return None


def is_workspace_reference(value: object) -> bool:
    return isinstance(value, dict) and value.get("workspace") is True


def load_manifest(path: Path) -> dict:
    return tomllib.loads(path.read_text())


def cargo_manifests() -> list[Path]:
    return sorted(
        path
        for path in CARGO_RS_ROOT.rglob("Cargo.toml")
        if path != CARGO_RS_ROOT / "Cargo.toml"
    )


if __name__ == "__main__":
    sys.exit(main())

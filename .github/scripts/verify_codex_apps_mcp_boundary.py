#!/usr/bin/env python3

"""Keep Codex Apps knowledge out of core, generic MCP, and generic host wiring."""

from __future__ import annotations

import re
import sys
import tomllib
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
PROTECTED_CRATES = (
    ROOT / "codex-rs" / "core",
    ROOT / "codex-rs" / "codex-mcp",
    ROOT / "codex-rs" / "mcp-server",
)
FORBIDDEN_PACKAGES = ("codex-apps", "codex-connectors")
FORBIDDEN_SOURCE_PATTERNS = (
    re.compile(r"(?:\b|_)codex[ _-]?apps", re.IGNORECASE),
    re.compile(r"(?:\b|_)codex[ _-]?connectors?", re.IGNORECASE),
    re.compile(r"\bconnectors?\b", re.IGNORECASE),
    re.compile(r"\bconnector_[a-zA-Z0-9_]+\b"),
    re.compile(r"\bConnector[A-Z][a-zA-Z0-9_]*\b"),
)
HTTP_APPS_ROOTS = (
    ROOT / "codex-rs" / "apps" / "src",
    ROOT / "codex-rs" / "ext" / "mcp" / "src" / "apps",
)
FORBIDDEN_IN_PROCESS_PATTERN = re.compile(r"\b(?:InProcess|in_process)\b")


def main() -> int:
    failures = []
    failures.extend(manifest_failures())
    failures.extend(source_failures())
    failures.extend(in_process_failures())

    if not failures:
        return 0

    print(
        "Codex Apps must remain ordinary HTTP MCP servers outside core, codex-mcp, "
        "and codex-mcp-server host wiring."
    )
    print(
        "Keep product behavior in codex-apps and its host extension; core and the generic "
        "MCP runtime may consume only ordinary MCP registrations and runtime metadata, "
        "while generic hosts may compose opaque extension bundles."
    )
    print()
    for failure in failures:
        print(f"- {failure}")

    return 1


def manifest_failures() -> list[str]:
    failures = []
    for crate_root in PROTECTED_CRATES:
        manifest_path = crate_root / "Cargo.toml"
        manifest = tomllib.loads(manifest_path.read_text())
        for section_name, dependencies in dependency_sections(manifest):
            for dependency_name, dependency in dependencies.items():
                package = (
                    dependency.get("package", dependency_name)
                    if isinstance(dependency, dict)
                    else dependency_name
                )
                if package in FORBIDDEN_PACKAGES:
                    failures.append(
                        f"{relative_path(manifest_path)} declares `{package}` "
                        f"in `[{section_name}]`"
                    )
    return failures


def dependency_sections(manifest: dict) -> list[tuple[str, dict]]:
    sections: list[tuple[str, dict]] = []
    for section_name in ("dependencies", "dev-dependencies", "build-dependencies"):
        dependencies = manifest.get(section_name)
        if isinstance(dependencies, dict):
            sections.append((section_name, dependencies))

    for target_name, target in manifest.get("target", {}).items():
        if not isinstance(target, dict):
            continue
        for section_name in ("dependencies", "dev-dependencies", "build-dependencies"):
            dependencies = target.get(section_name)
            if isinstance(dependencies, dict):
                sections.append((f"target.{target_name}.{section_name}", dependencies))

    return sections


def source_failures() -> list[str]:
    failures = []
    for crate_root in PROTECTED_CRATES:
        for path in sorted((crate_root / "src").glob("**/*.rs")):
            for line_number, line in enumerate(path.read_text().splitlines(), start=1):
                if any(pattern.search(line) for pattern in FORBIDDEN_SOURCE_PATTERNS):
                    failures.append(
                        f"{relative_path(path)}:{line_number} contains Apps product knowledge"
                    )
    return failures


def in_process_failures() -> list[str]:
    failures = []
    for source_root in HTTP_APPS_ROOTS:
        for path in sorted(source_root.glob("**/*.rs")):
            for line_number, line in enumerate(path.read_text().splitlines(), start=1):
                if FORBIDDEN_IN_PROCESS_PATTERN.search(line):
                    failures.append(
                        f"{relative_path(path)}:{line_number} introduces an in-process Apps path"
                    )
    return failures


def relative_path(path: Path) -> str:
    return str(path.relative_to(ROOT))


if __name__ == "__main__":
    sys.exit(main())

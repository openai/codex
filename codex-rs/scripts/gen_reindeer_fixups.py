#!/usr/bin/env python3
"""
Generate Reindeer fixups for third-party crates (gitignored).

Reindeer requires an explicit decision for each crate with a build script:
either run it or ignore it. For most third-party crates we run build scripts.

We intentionally do not generate `extra_srcs` fixups here:
  - Reindeer validates fixup globs against its chosen crate source directory,
    and with vendoring enabled this can be a filtered view of the package that
    does not include top-level docs/fixtures (README, tests data, etc).
  - Instead, we include common non-Rust sources via the `codex_rust_*` wrapper
    macros in `codex-rs/buck2/reindeer_macros.bzl` using Buck `glob(...)`.

The generated fixups live under `codex-rs/third-party/fixups/` and are
intentionally gitignored for now. This script is designed to be re-run and will
overwrite any existing generated fixups.
"""

from __future__ import annotations

import json
import os
import pathlib
import re
import shutil
import subprocess
from typing import Any


CODEX_RS_ROOT = pathlib.Path(__file__).resolve().parents[1]
THIRD_PARTY_DIR = CODEX_RS_ROOT / "third-party"
VENDOR_DIR = THIRD_PARTY_DIR / "vendor"
FIXUPS_DIR = THIRD_PARTY_DIR / "fixups"


def cargo_metadata() -> dict[str, Any]:
    out = subprocess.check_output(
        ["cargo", "metadata", "--format-version=1", "--locked"],
        cwd=CODEX_RS_ROOT,
        text=True,
    )
    return json.loads(out)


def reindeer_reachable_package_ids(meta: dict[str, Any]) -> set[str]:
    """
    Approximate the set of packages Reindeer cares about for `buckify`.

    Reindeer buckifies dependencies of workspace members, but it does not need
    deps that are only reachable through `dev` edges (tests/benches/examples).
    """

    resolve = meta.get("resolve") or {}
    nodes = resolve.get("nodes") or []
    by_id: dict[str, Any] = {n["id"]: n for n in nodes}

    roots = list(meta.get("workspace_members") or [])
    reachable: set[str] = set()
    stack = list(roots)
    while stack:
        pkg_id = stack.pop()
        if pkg_id in reachable:
            continue
        reachable.add(pkg_id)

        node = by_id.get(pkg_id)
        if not node:
            continue

        for dep in node.get("deps") or []:
            kinds = dep.get("dep_kinds") or []
            if kinds and all(k.get("kind") == "dev" for k in kinds):
                continue
            stack.append(dep["pkg"])

    return reachable


def vendored_links_value(name: str, version: str) -> str | None:
    """
    If the vendored crate declares `links = "..."` in its Cargo.toml, return it.

    Cargo provides this to build scripts via the CARGO_MANIFEST_LINKS env var.
    Some build scripts rely on it (e.g. ring).
    """

    cargo_toml = VENDOR_DIR / f"{name}-{version}" / "Cargo.toml"
    if not cargo_toml.exists():
        return None

    text = cargo_toml.read_text(encoding="utf-8", errors="replace")

    # Very small parser: search within the [package] section first, then fall back.
    pkg_idx = text.find("[package]")
    if pkg_idx != -1:
        rest = text[pkg_idx:]
        next_table = rest.find("\n[", 1)
        pkg_block = rest if next_table == -1 else rest[:next_table]
        m = re.search(r'(?m)^\s*links\s*=\s*"([^"]+)"\s*$', pkg_block)
        if m:
            return m.group(1)

    m = re.search(r'(?m)^\s*links\s*=\s*"([^"]+)"\s*$', text)
    if m:
        return m.group(1)
    return None


def main() -> int:
    meta = cargo_metadata()
    packages = meta.get("packages", [])
    reachable_ids = reindeer_reachable_package_ids(meta)

    # First-party packages whose build.rs we intentionally ignore under Buck.
    buildscript_run_overrides: dict[str, bool] = {
        # build.rs only adds rerun-if-changed
        "codex-execpolicy-legacy": False,
        # Windows-only resource compilation
        "codex-windows-sandbox": False,
    }

    # Packages where Cargo metadata reports a build script, but Reindeer does not
    # require/accept a buildscript fixup in this workspace.
    skip_packages: set[str] = {
        "indexmap",
        "quinn",
    }

    reachable_pkgs = [p for p in packages if p["id"] in reachable_ids]

    # name -> version -> has_buildscript
    by_name: dict[str, dict[str, bool]] = {}
    for pkg in reachable_pkgs:
        name = pkg["name"]
        version = pkg["version"]
        has_buildscript = any(
            "custom-build" in (t.get("kind") or []) for t in (pkg.get("targets") or [])
        )
        by_name.setdefault(name, {})[version] = has_buildscript

    # Nuke and regenerate: fixups are gitignored and generated.
    if FIXUPS_DIR.exists():
        shutil.rmtree(FIXUPS_DIR)
    FIXUPS_DIR.mkdir(parents=True, exist_ok=True)

    wrote = 0
    for name, versions in sorted(by_name.items()):
        if name in skip_packages:
            continue

        buildscript_versions = sorted([v for v, has in versions.items() if has])
        if not buildscript_versions:
            continue

        run = buildscript_run_overrides.get(name, True)

        fixup_path = FIXUPS_DIR / name / "fixups.toml"
        fixup_path.parent.mkdir(parents=True, exist_ok=True)

        stanzas: list[str] = []
        for v in buildscript_versions:
            if run:
                links = vendored_links_value(name, v)
                stanzas.append(f"['cfg(version = \"={v}\")'.buildscript.run]")
                stanzas.append("rustc_link_lib = true")
                stanzas.append("rustc_link_search = true")
                if links:
                    stanzas.append(f'env = {{ CARGO_MANIFEST_LINKS = "{links}" }}')
                stanzas.append("")
            else:
                stanzas.append(f"['cfg(version = \"={v}\")']")
                stanzas.append("buildscript.run = false")
                stanzas.append("")

        content = "\n".join(stanzas).rstrip() + "\n"
        fixup_path.write_text(content, encoding="utf-8")
        wrote += 1

    if wrote:
        print(
            f"Wrote buildscript fixups.toml for {wrote} crates under "
            f"{os.path.relpath(FIXUPS_DIR, CODEX_RS_ROOT)}"
        )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())


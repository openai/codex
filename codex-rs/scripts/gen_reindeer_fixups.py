#!/usr/bin/env python3
"""
Generate Reindeer fixups for third-party crates.

Reindeer requires an explicit decision for each crate with a build script:
either run it or ignore it. For most third-party crates we run build scripts.

We intentionally do not generate `extra_srcs` fixups here:
  - Reindeer validates fixup globs against its chosen crate source directory,
    and with vendoring enabled this can be a filtered view of the package that
    does not include top-level docs/fixtures (README, tests data, etc).
  - Instead, we include common non-Rust sources via the `codex_rust_*` wrapper
    macros in `codex-rs/buck2/reindeer_macros.bzl` using Buck `glob(...)`.

This script is checked in; its outputs are not.

The generated fixups live under `codex-rs/third-party/fixups/` and are
intentionally gitignored for now (see the repo root `.gitignore`, which ignores
`codex-rs/third-party/`). This script is designed to be re-run and will
overwrite any existing generated fixups.
"""

from __future__ import annotations

import json
import os
import pathlib
import re
import shutil
import subprocess
import sys
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


def openssl_dep_version_number_hex() -> str | None:
    """
    Derive a DEP_OPENSSL_VERSION_NUMBER value (hex string, no 0x prefix).

    Under Cargo, openssl-sys emits a `cargo:version_number=...` metadata line
    and Cargo converts it into DEP_OPENSSL_VERSION_NUMBER for dependents.
    Buck's buildscript runner does not currently propagate those DEP_* env vars,
    so we approximate the value here for Buck builds.
    """

    def pkg_config_modversion() -> str | None:
        try:
            out = subprocess.check_output(
                ["pkg-config", "--modversion", "openssl"],
                cwd=CODEX_RS_ROOT,
                text=True,
                stderr=subprocess.DEVNULL,
            )
        except (OSError, subprocess.CalledProcessError):
            return None
        v = out.strip()
        return v if v else None

    def parse_openssl_version(
        v: str,
    ) -> tuple[int, int, int, str | None] | None:
        """
        Parse versions like:
        - 3.0.13
        - 1.1.1w
        - 1.1.0h
        """

        m = re.match(r"^(\d+)\.(\d+)\.(\d+)([a-z])?$", v)
        if not m:
            return None
        major, minor, patch = (int(m.group(1)), int(m.group(2)), int(m.group(3)))
        suffix = m.group(4)
        return (major, minor, patch, suffix)

    v = pkg_config_modversion()
    parsed = parse_openssl_version(v) if v else None

    # In CI we install libssl-dev, so pkg-config should generally be available.
    # If it isn't, guess OpenSSL 3.x on modern Linux distros.
    if parsed is None:
        if os.name == "posix":
            # 3.0.0: (3<<28)|(0<<20)|(0<<4)
            return f"{(3 << 28):x}"
        return None

    major, minor, patch, suffix = parsed

    # OpenSSL 3 uses (major<<28)|(minor<<20)|(patch<<4).
    if major >= 3:
        version = (major << 28) | (minor << 20) | (patch << 4)
        return f"{version:x}"

    # OpenSSL 1.x uses 0xMNNFFPPS where PP is the patch number and S is the
    # patch status nibble. For lettered patch releases, 'a' => 1, 'b' => 2, etc.
    if major == 1:
        patch_num = 0
        if suffix:
            patch_num = ord(suffix) - ord("a") + 1
        # Release status (0xf) matches Cargo/rust-openssl default behavior.
        version = (major << 28) | (minor << 20) | (patch << 12) | (patch_num << 4) | 0xF
        return f"{version:x}"

    return None


def openssl_dep_conf() -> str | None:
    """
    Derive a DEP_OPENSSL_CONF value (comma-separated macro names).

    Under Cargo, openssl-sys emits `cargo:conf=...` and Cargo converts it into
    DEP_OPENSSL_CONF for dependents. The `openssl` crate's build script reads
    DEP_OPENSSL_CONF and re-emits per-macro `osslconf="..."` cfgs so it can
    conditionalize APIs that are disabled in the system OpenSSL build (e.g.
    OPENSSL_NO_IDEA).
    """

    def pkg_config_cflags() -> list[str] | None:
        try:
            out = subprocess.check_output(
                ["pkg-config", "--cflags", "openssl"],
                cwd=CODEX_RS_ROOT,
                text=True,
                stderr=subprocess.DEVNULL,
            )
        except (OSError, subprocess.CalledProcessError):
            return None
        return [w for w in out.split() if w]

    def preprocess_opensslconf(cflags: list[str]) -> str | None:
        snippet = "#include <openssl/opensslconf.h>\n"
        for cc in ("cc", "clang"):
            try:
                return subprocess.check_output(
                    [cc, "-dM", "-E", "-xc", "-", *cflags],
                    cwd=CODEX_RS_ROOT,
                    input=snippet,
                    text=True,
                    stderr=subprocess.DEVNULL,
                )
            except (OSError, subprocess.CalledProcessError):
                continue
        return None

    cflags = pkg_config_cflags()
    if not cflags:
        return None

    out = preprocess_opensslconf(cflags)
    if not out:
        return None

    # Match the behavior expected by openssl/build.rs: it splits DEP_OPENSSL_CONF
    # by ',' and emits cfgs `osslconf="..."` for each token.
    conf: set[str] = set()
    for line in out.splitlines():
        # Lines look like: "#define OPENSSL_NO_IDEA 1"
        m = re.match(r"^#define\s+(OPENSSL_[A-Z0-9_]+)\b", line)
        if not m:
            continue
        macro = m.group(1)
        if macro.startswith("OPENSSL_NO_"):
            conf.add(macro)
    return ",".join(sorted(conf)) if conf else ""


def toml_inline_table(entries: dict[str, str]) -> str:
    # Emit stable TOML with double-quoted strings.
    parts = [f"{k} = {json.dumps(v)}" for k, v in sorted(entries.items())]
    return "{ " + ", ".join(parts) + " }"


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
                env_vars: dict[str, str] = {}
                # Some crates rely on this being set when `links = "..."`
                # exists in Cargo.toml (Cargo passes it to build scripts).
                if links:
                    env_vars["CARGO_MANIFEST_LINKS"] = links
                # The `openssl` crate's build script derives cfgs from
                # DEP_OPENSSL_VERSION_NUMBER (emitted by openssl-sys). Buck's
                # buildscript runner does not currently propagate those DEP_*
                # env vars, so approximate the value for Buck builds.
                if name == "openssl" and v == "0.10.73":
                    dep_ver = openssl_dep_version_number_hex()
                    if dep_ver:
                        env_vars["DEP_OPENSSL_VERSION_NUMBER"] = dep_ver
                    dep_conf = openssl_dep_conf()
                    if dep_conf is not None:
                        # Empty string is meaningful: "no conf macros enabled".
                        env_vars["DEP_OPENSSL_CONF"] = dep_conf
                    elif sys.platform.startswith("linux"):
                        # If we can't determine the conf macros, be conservative
                        # on Linux (Ubuntu CI commonly disables IDEA).
                        env_vars["DEP_OPENSSL_CONF"] = "OPENSSL_NO_IDEA"
                if env_vars:
                    stanzas.append(f"env = {toml_inline_table(env_vars)}")
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

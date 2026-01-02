#!/usr/bin/env python3
"""
Patch the Reindeer-generated `codex-rs/third-party/BUCK` to add a small overlay
of third-party crates that are only used as *dev-dependencies* in the Cargo
workspace.

Reindeer currently generates Buck targets for Cargo "normal" deps (plus build
deps for build scripts), but it does not emit targets for dev-dependencies
because it does not generate rules for Cargo test/example/bench targets.

For Buck2-driven `rust_test()` targets in first-party crates, we still need a
handful of dev-only third-party crates to exist as targets in
`//codex-rs/third-party`.

This script is run as part of `scripts/setup_buck2_local.sh` and is intended to
be safe to run repeatedly.
"""

from __future__ import annotations

import json
import os
import pathlib
import re
import subprocess
import sys
from dataclasses import dataclass
from typing import Any, Optional


CODEX_RS_ROOT = pathlib.Path(__file__).resolve().parents[1]
REPO_ROOT = CODEX_RS_ROOT.parent
BUILDIFIER = REPO_ROOT / "scripts" / "buildifier"
THIRD_PARTY_BUCK = CODEX_RS_ROOT / "third-party" / "BUCK"
VENDOR_DIR = CODEX_RS_ROOT / "third-party" / "vendor"

BEGIN_MARKER = "# BEGIN CODEX LOCAL DEV TEST DEPS (generated)\n"
END_MARKER = "# END CODEX LOCAL DEV TEST DEPS (generated)\n"


def cargo_metadata() -> dict[str, Any]:
    out = subprocess.check_output(
        ["cargo", "metadata", "--format-version=1", "--locked"],
        cwd=CODEX_RS_ROOT,
        text=True,
    )
    return json.loads(out)


def parse_semver(version: str) -> tuple[str, str, str, str]:
    m = re.match(r"^(\d+)\.(\d+)\.(\d+)(?:-(.*))?$", version)
    if not m:
        return ("0", "0", "0", "")
    return (m.group(1), m.group(2), m.group(3), m.group(4) or "")


def parse_dep_cfg(cfg: Optional[str]) -> list[str]:
    """
    Best-effort mapping for cfg(...) strings to Buck2 constraint keys.
    This mirrors the mapping in scripts/gen_buck_first_party.py.
    """
    if cfg is None:
        return []

    cfg = cfg.strip()
    m = re.fullmatch(r'cfg\(target_os\s*=\s*"([^"]+)"\)', cfg)
    if m:
        os_name = m.group(1)
        if os_name in ("linux", "macos", "windows"):
            return [f"prelude//os:{os_name}"]

    if cfg == "cfg(windows)":
        return ["prelude//os:windows"]

    if cfg == "cfg(unix)":
        return ["prelude//os:linux", "prelude//os:macos"]

    return []


def starlark_list(items: list[str], indent: str = "    ") -> str:
    if not items:
        return "[]"
    lines = ["["]
    for it in items:
        lines.append(f'{indent}"{it}",')
    lines.append("]")
    return "\n".join(lines)

def starlark_str(s: str) -> str:
    # Conservative escaping suitable for Starlark string literals.
    return (
        s.replace("\\", "\\\\")
        .replace('"', '\\"')
        .replace("\r", "\\r")
        .replace("\n", "\\n")
    )


def starlark_dict(d: dict[str, str], indent: str = "    ") -> str:
    if not d:
        return "{}"
    lines = ["{"]
    for k, v in d.items():
        lines.append(f'{indent}"{starlark_str(k)}": "{starlark_str(v)}",')
    lines.append("}")
    return "\n".join(lines)


def starlark_deps_expr(base: list[str], conditional: dict[str, list[str]]) -> str:
    expr = starlark_list(base)
    for constraint, deps in sorted(conditional.items()):
        expr = (
            f"{expr} + select({{\n"
            f'    "{constraint}": {starlark_list(deps, indent="        ")},\n'
            '    "DEFAULT": [],\n'
            "})"
        )
    return expr


@dataclass(frozen=True)
class BuckDep:
    label: str
    local_name: str
    crate_name: str
    cfg: Optional[str]


def group_deps(deps: list[BuckDep]) -> tuple[list[str], dict[str, list[str]], dict[str, str]]:
    base_deps: list[str] = []
    conditional_deps: dict[str, list[str]] = {}
    named_deps: dict[str, str] = {}
    for d in deps:
        dep_constraints = parse_dep_cfg(d.cfg)
        if dep_constraints:
            for c in dep_constraints:
                conditional_deps.setdefault(c, []).append(d.label)
        else:
            base_deps.append(d.label)

        if d.local_name != d.crate_name:
            named_deps[d.local_name] = d.label

    base_deps = sorted(set(base_deps))
    for k in list(conditional_deps.keys()):
        conditional_deps[k] = sorted(set(conditional_deps[k]))

    return (base_deps, conditional_deps, named_deps)


def strip_existing_overlay(text: str) -> str:
    start = text.find(BEGIN_MARKER)
    if start == -1:
        return text
    end = text.find(END_MARKER, start)
    if end == -1:
        # If the file is corrupted, be conservative and do not try to patch.
        raise SystemExit(f"Found BEGIN marker but not END marker in {THIRD_PARTY_BUCK}")
    return text[:start] + text[end + len(END_MARKER) :]


def patch_reindeer_output_for_test_features(text: str) -> str:
    """
    Best-effort patching of Reindeer output to make certain dev-only crates build.

    Today, our locally-generated targets for dev-dependencies (e.g. `insta`) may
    require feature combinations on their transitive deps that are not enabled
    by the normal (non-test) workspace graph that Reindeer buckifies.

    We keep this narrowly-scoped and idempotent.
    """
    # `insta` requires `similar` with the `inline` feature.
    similar_rule_name = "similar-2.7.0"
    lines = text.splitlines(keepends=True)
    for i, line in enumerate(lines):
        if "name" in line and similar_rule_name in line and "name" in line:
            # Walk until the end of this rule.
            j = i
            while j < len(lines) and lines[j].strip() != ")":
                j += 1
            if j >= len(lines):
                break

            block = lines[i : j + 1]
            # Only treat it as patched if the *feature* is present. The crate
            # itself contains an `inline.rs` source file, so substring checks
            # would be a false positive.
            if any(l.strip() == '"inline",' for l in block):
                break

            for k, bl in enumerate(block):
                if bl.lstrip().startswith("features = ["):
                    block.insert(k + 1, '        "inline",\n')
                    lines[i : j + 1] = block
                    return "".join(lines)

            # No existing features list; add one just before `visibility`.
            for k, bl in enumerate(block):
                if bl.lstrip().startswith("visibility ="):
                    block.insert(k, '    features = ["inline"],\n')
                    lines[i : j + 1] = block
                    return "".join(lines)
            break

    return text


def present_target_names(text: str) -> set[str]:
    # This is intentionally simple: it's used only to decide whether a target
    # already exists so we don't duplicate it.
    # Reindeer emits string literals with escaped quotes (e.g. `name = \"foo\"`)
    # so we accept an optional backslash before both quote characters.
    return set(re.findall(r'(?m)^\s*name\s*=\s*\\?"([^"]+)\\?"', text))


def vendor_crate_root(pkg_name: str, version: str) -> pathlib.Path:
    crate_dir = VENDOR_DIR / f"{pkg_name}-{version}"
    # Prefer src/lib.rs, but fall back to whichever crate root Cargo reports.
    lib_rs = crate_dir / "src" / "lib.rs"
    if lib_rs.exists():
        # Paths in `codex-rs/third-party/BUCK` are relative to the `third-party`
        # package directory, so use `vendor/...` rather than an absolute path or
        # a `third-party/vendor/...` path.
        return pathlib.Path(f"vendor/{pkg_name}-{version}/src/lib.rs")
    raise SystemExit(f"Could not find {crate_dir}/src/lib.rs (needed for {pkg_name} {version})")


def append_overlay(meta: dict[str, Any], existing_buck: str) -> str:
    by_id: dict[str, Any] = {p["id"]: p for p in (meta.get("packages") or [])}
    workspace_members: set[str] = set(meta.get("workspace_members") or [])
    node_by_id: dict[str, Any] = {n["id"]: n for n in (meta.get("resolve") or {}).get("nodes") or []}

    existing_targets = present_target_names(existing_buck)

    def target_name_for_pkg_id(pid: str) -> str:
        p = by_id[pid]
        return f"{p['name']}-{p['version']}"

    # Find third-party packages that are reachable via any dev-dependency edge
    # from any workspace member.
    dev_needed_pkg_ids: set[str] = set()
    for pkg_id in workspace_members:
        node = node_by_id.get(pkg_id) or {}
        for dep in node.get("deps") or []:
            dep_id = dep["pkg"]
            if dep_id in workspace_members:
                continue
            dep_kinds = dep.get("dep_kinds") or []
            if not any(k.get("kind") == "dev" for k in dep_kinds):
                continue
            dep_pkg = by_id.get(dep_id)
            if not dep_pkg:
                continue
            # Only patch registry crates. Workspace members have source=None.
            if dep_pkg.get("source") is None:
                continue
            dev_needed_pkg_ids.add(dep_id)

    # Reindeer buckify does not emit targets for dev-deps. We patch in any dev
    # deps that are missing *and* any of their transitive normal deps that are
    # also missing (e.g., proc-macro helper crates).
    to_add: set[str] = {pid for pid in dev_needed_pkg_ids if target_name_for_pkg_id(pid) not in existing_targets}
    queue = list(to_add)

    while queue:
        pid = queue.pop()
        node = node_by_id.get(pid) or {}
        for dep in node.get("deps") or []:
            dep_id = dep["pkg"]
            dep_pkg = by_id.get(dep_id)
            if not dep_pkg:
                continue
            if dep_pkg.get("source") is None:
                continue

            dep_kinds = dep.get("dep_kinds") or []
            if not any(k.get("kind") is None or k.get("kind") == "normal" for k in dep_kinds):
                continue

            tgt = target_name_for_pkg_id(dep_id)
            if tgt in existing_targets or dep_id in to_add:
                continue

            to_add.add(dep_id)
            queue.append(dep_id)

    missing_pkg_ids = sorted(to_add, key=lambda pid: (by_id[pid]["name"], by_id[pid]["version"]))
    if os.environ.get("CODEX_PATCH_THIRD_PARTY_DEBUG") == "1":
        missing_targets = [target_name_for_pkg_id(pid) for pid in missing_pkg_ids]
        preview = existing_buck[:120].replace("\n", "\\n")
        print(f"patch_third_party_buck_for_tests: buck_preview={preview!r}")
        print(f"patch_third_party_buck_for_tests: existing_targets={len(existing_targets)}")
        print(f"patch_third_party_buck_for_tests: dev_needed={len(dev_needed_pkg_ids)} missing={len(missing_targets)}")
        for t in sorted(missing_targets)[:50]:
            print(f"  missing: {t}")
    if not missing_pkg_ids:
        return existing_buck

    lines: list[str] = []
    lines.append("\n" + BEGIN_MARKER.rstrip("\n"))
    lines.append("# This section is appended by codex-rs/scripts/patch_third_party_buck_for_tests.py")
    lines.append("# to make Buck `rust_test()` targets usable for first-party dev-deps.")
    lines.append("")

    for pkg_id in missing_pkg_ids:
        pkg = by_id[pkg_id]
        name = pkg["name"]
        version = pkg["version"]
        rule_name = f"{name}-{version}"
        if rule_name in existing_targets:
            continue

        # Find the lib target to determine the Rust crate name and edition.
        lib_targets = [
            t
            for t in (pkg.get("targets") or [])
            if "lib" in (t.get("kind") or []) or "proc-macro" in (t.get("kind") or [])
        ]
        if not lib_targets:
            # These dev deps should all be libraries.
            continue
        lib_t = lib_targets[0]
        crate_name = lib_t["name"]
        edition = pkg.get("edition") or "2021"
        ver_major, ver_minor, ver_patch, ver_pre = parse_semver(version)
        proc_macro = "proc-macro" in (lib_t.get("kind") or [])

        # Compute deps for the library (normal deps only).
        node = node_by_id.get(pkg_id) or {}
        deps: list[BuckDep] = []
        for dep in node.get("deps") or []:
            dep_id = dep["pkg"]
            dep_pkg = by_id.get(dep_id)
            if not dep_pkg:
                continue

            dep_kinds = dep.get("dep_kinds") or []
            local_name = dep.get("name") or dep_pkg["name"]
            dep_crate_name = dep_pkg.get("targets", [{}])[0].get("name") or local_name
            dep_label = f":{dep_pkg['name']}-{dep_pkg['version']}"

            # One dependency may have multiple cfg(...) selectors; include them all.
            for k in dep_kinds:
                kind = k.get("kind")
                if kind is None or kind == "normal":
                    deps.append(
                        BuckDep(
                            label=dep_label,
                            local_name=local_name,
                            crate_name=dep_crate_name,
                            cfg=k.get("target"),
                        )
                    )

        base_deps, conditional_deps, named_deps = group_deps(deps)
        deps_expr = starlark_deps_expr(base_deps, conditional_deps)

        crate_root_rel = vendor_crate_root(name, version)

        env = {
            "CARGO_CRATE_NAME": crate_name,
            "CARGO_MANIFEST_DIR": f"vendor/{name}-{version}",
            "CARGO_PKG_AUTHORS": ":".join(pkg.get("authors") or []),
            "CARGO_PKG_DESCRIPTION": pkg.get("description") or "",
            "CARGO_PKG_NAME": name,
            "CARGO_PKG_REPOSITORY": pkg.get("repository") or "",
            "CARGO_PKG_VERSION": version,
            "CARGO_PKG_VERSION_MAJOR": ver_major,
            "CARGO_PKG_VERSION_MINOR": ver_minor,
            "CARGO_PKG_VERSION_PATCH": ver_patch,
            "CARGO_PKG_VERSION_PRE": ver_pre,
        }

        has_build_rs = (VENDOR_DIR / f"{name}-{version}" / "build.rs").exists()
        if has_build_rs:
            build_bin = f"{rule_name}-build-script-build"
            build_run = f"{rule_name}-build-script-run"
            build_rs_rel = pathlib.Path(f"vendor/{name}-{version}/build.rs")

            # Build deps for compiling the build script itself.
            build_deps: list[BuckDep] = []
            for dep in node.get("deps") or []:
                dep_id = dep["pkg"]
                dep_pkg = by_id.get(dep_id)
                if not dep_pkg:
                    continue
                dep_kinds = dep.get("dep_kinds") or []
                local_name = dep.get("name") or dep_pkg["name"]
                dep_crate_name = dep_pkg.get("targets", [{}])[0].get("name") or local_name
                dep_label = f":{dep_pkg['name']}-{dep_pkg['version']}"
                for k in dep_kinds:
                    if k.get("kind") == "build":
                        build_deps.append(
                            BuckDep(
                                label=dep_label,
                                local_name=local_name,
                                crate_name=dep_crate_name,
                                cfg=k.get("target"),
                            )
                        )
            build_base_deps, build_conditional_deps, build_named_deps = group_deps(build_deps)
            build_deps_expr = starlark_deps_expr(build_base_deps, build_conditional_deps)

            lines.append("codex_rust_binary(")
            lines.append(f'    name = "{build_bin}",')
            lines.append(f'    srcs = ["{build_rs_rel}"],')
            lines.append('    crate = "build_script_build",')
            lines.append(f'    crate_root = "{build_rs_rel}",')
            lines.append(f'    edition = "{edition}",')
            lines.append("    env = " + starlark_dict({**env, "CARGO_CRATE_NAME": "build_script_build"}, indent="        ") + ",")
            lines.append('    visibility = [],')
            if build_base_deps or build_conditional_deps:
                lines.append(f"    deps = {build_deps_expr},")
            if build_named_deps:
                items = [f'"{k}": "{v}"' for k, v in sorted(build_named_deps.items())]
                lines.append("    named_deps = {")
                for it in items:
                    lines.append(f"        {it},")
                lines.append("    },")
            lines.append(")")
            lines.append("")

            lines.append("codex_buildscript_run(")
            lines.append(f'    name = "{build_run}",')
            lines.append(f'    package_name = "{name}",')
            lines.append(f'    buildscript_rule = ":{build_bin}",')
            # Keep env small; codex_buildscript_run fills in common profile vars.
            lines.append(
                "    env = "
                + starlark_dict(
                    {
                        "CARGO_PKG_AUTHORS": env["CARGO_PKG_AUTHORS"],
                        "CARGO_PKG_DESCRIPTION": env["CARGO_PKG_DESCRIPTION"],
                        "CARGO_PKG_REPOSITORY": env["CARGO_PKG_REPOSITORY"],
                        "CARGO_PKG_VERSION_MAJOR": env["CARGO_PKG_VERSION_MAJOR"],
                        "CARGO_PKG_VERSION_MINOR": env["CARGO_PKG_VERSION_MINOR"],
                        "CARGO_PKG_VERSION_PATCH": env["CARGO_PKG_VERSION_PATCH"],
                        "CARGO_PKG_VERSION_PRE": env["CARGO_PKG_VERSION_PRE"],
                    },
                    indent="        ",
                )
                + ","
            )
            lines.append("    rustc_link_lib = True,")
            lines.append("    rustc_link_search = True,")
            lines.append(f'    version = "{version}",')
            lines.append(")")
            lines.append("")

            # Build-script outputs for crates that use `OUT_DIR`.
            env = {**env, "OUT_DIR": f"$(location :{build_run}[out_dir])"}

        lines.append("codex_rust_library(")
        lines.append(f'    name = "{rule_name}",')
        lines.append(f'    srcs = ["{crate_root_rel}"],')
        lines.append(f'    crate = "{crate_name}",')
        lines.append(f'    crate_root = "{crate_root_rel}",')
        lines.append(f'    edition = "{edition}",')
        lines.append("    env = " + starlark_dict(env, indent="        ") + ",")
        features = sorted([f for f in (node.get("features") or []) if not f.startswith("dep:")])
        if features:
            lines.append(f"    features = {starlark_list(features)},")
        if proc_macro:
            lines.append("    proc_macro = True,")
        if has_build_rs:
            lines.append(f'    rustc_flags = ["@$(location :{rule_name}-build-script-run[rustc_flags])"],')
        lines.append("    visibility = [],")
        if base_deps or conditional_deps:
            lines.append(f"    deps = {deps_expr},")
        if named_deps:
            items = [f'"{k}": "{v}"' for k, v in sorted(named_deps.items())]
            lines.append("    named_deps = {")
            for it in items:
                lines.append(f"        {it},")
            lines.append("    },")
        lines.append(")")
        lines.append("")

    lines.append(END_MARKER.rstrip("\n") + "\n")
    return existing_buck + "\n".join(lines)


def main() -> int:
    if not THIRD_PARTY_BUCK.exists():
        raise SystemExit(f"Missing {THIRD_PARTY_BUCK}; run `reindeer buckify` first.")
    meta = cargo_metadata()
    existing = THIRD_PARTY_BUCK.read_text(encoding="utf-8")
    existing = strip_existing_overlay(existing)
    existing = patch_reindeer_output_for_test_features(existing)
    patched = append_overlay(meta, existing)
    if patched != existing:
        THIRD_PARTY_BUCK.write_text(patched, encoding="utf-8")
        if BUILDIFIER.exists():
            # Keep the local-only generated file readable.
            proc = subprocess.run(
                [str(BUILDIFIER), "-lint=off", "-mode=fix", str(THIRD_PARTY_BUCK)],
                cwd=REPO_ROOT,
                text=True,
            )
            if proc.returncode != 0:
                print(
                    f"warning: buildifier failed (exit {proc.returncode}); leaving {THIRD_PARTY_BUCK} unformatted",
                    file=sys.stderr,
                )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

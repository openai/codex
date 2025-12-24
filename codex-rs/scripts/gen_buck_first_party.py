#!/usr/bin/env python3
"""
Generate BUCK files for first-party Rust crates in the codex-rs workspace.

Reindeer generates third-party targets (in `codex-rs/third-party/BUCK`). This
script generates Buck targets for workspace members (local crates) that depend
on those third-party targets.

The generated BUCK files are intentionally gitignored for now.
"""

from __future__ import annotations

import json
import pathlib
import re
import subprocess
import sys
from dataclasses import dataclass
from typing import Any, Optional


CODEX_RS_ROOT = pathlib.Path(__file__).resolve().parents[1]
REPO_ROOT = CODEX_RS_ROOT.parent
BUILDIFIER = REPO_ROOT / "scripts" / "buildifier"

# All labels are rooted at the repo root Buck project.
CODEX_RS_LABEL_PREFIX = "//codex-rs"

# Some integration tests rely on Cargo's behavior of allowing `include_str!`
# paths that traverse outside of a package directory. Buck does not allow `..`
# paths in `srcs`, so we skip those tests for now.
SKIP_INTEGRATION_TESTS_BY_PACKAGE: dict[str, set[str]] = {
    "codex-core": {"all"},
}


def cargo_metadata() -> dict[str, Any]:
    out = subprocess.check_output(
        ["cargo", "metadata", "--format-version=1", "--locked"],
        cwd=CODEX_RS_ROOT,
        text=True,
    )
    return json.loads(out)


@dataclass(frozen=True)
class BuckDep:
    label: str
    local_name: str
    crate_name: str
    cfg: Optional[str]


def parse_dep_cfg(cfg: Optional[str]) -> list[str]:
    """
    Map a subset of Cargo cfg(...) strings to Buck2 constraint keys.

    Returns a list of constraint keys that should include the dependency.
    An empty list means "unconditional".
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

    # If we don't recognize the expression, treat it as unconditional to avoid
    # silently dropping deps. This may cause platform build issues, in which
    # case we can extend this mapping.
    return []


def buckify_features(features: list[str]) -> list[str]:
    # Cargo metadata includes lots of feature names, including "default".
    # Keep stable output for diffs.
    return sorted(features)


def package_lib_target(pkg: dict[str, Any]) -> Optional[dict[str, Any]]:
    for t in pkg.get("targets") or []:
        kind = t.get("kind") or []
        if "proc-macro" in kind or "lib" in kind:
            return t
    return None


def package_bin_targets(pkg: dict[str, Any]) -> list[dict[str, Any]]:
    return [t for t in (pkg.get("targets") or []) if "bin" in (t.get("kind") or [])]

def package_test_targets(pkg: dict[str, Any]) -> list[dict[str, Any]]:
    # These correspond to Cargo integration tests under `tests/`.
    return [t for t in (pkg.get("targets") or []) if "test" in (t.get("kind") or [])]


def relpath_from_codex_rs(path: str) -> str:
    return str(pathlib.Path(path).resolve().relative_to(CODEX_RS_ROOT))


def write_buck_file(crate_dir: pathlib.Path, content: str) -> None:
    buck_path = crate_dir / "BUCK"
    buck_path.write_text(content, encoding="utf-8")

def buildifier(paths: list[pathlib.Path]) -> None:
    if not paths:
        return
    if not BUILDIFIER.exists():
        # Generated BUCK files are local-only; keep generation working even if
        # buildifier isn't available.
        return

    # Run buildifier from the repo root so it can infer workspace-relative paths.
    proc = subprocess.run(
        [str(BUILDIFIER), "-lint=off", "-mode=fix", *[str(p) for p in paths]],
        cwd=REPO_ROOT,
        text=True,
    )
    if proc.returncode != 0:
        print(f"warning: buildifier failed (exit {proc.returncode}); leaving BUCK files unformatted", file=sys.stderr)


def starlark_list(items: list[str], indent: str = "    ") -> str:
    if not items:
        return "[]"
    lines = ["["]
    for it in items:
        lines.append(f'{indent}"{it}",')
    lines.append("]")
    return "\n".join(lines)


def starlark_dict(d: dict[str, str], indent: str = "    ") -> str:
    if not d:
        return "{}"
    lines = ["{"]  # insertion order is stable for our dict construction
    for k, v in d.items():
        lines.append(f'{indent}"{k}": "{v}",')
    lines.append("}")
    return "\n".join(lines)


def starlark_deps_expr(base: list[str], conditional: dict[str, list[str]]) -> str:
    """
    Render deps as a list plus (optional) `select()` fragments.
    """
    expr = starlark_list(base)
    for constraint, deps in sorted(conditional.items()):
        expr = (
            f"{expr} + select({{\n"
            f'    "{constraint}": {starlark_list(deps, indent="        ")},\n'
            f'    "DEFAULT": [],\n'
            "})"
        )
    return expr


def starlark_str(s: str) -> str:
    # Conservative escaping for readability (most strings here are already safe).
    return s.replace("\\", "\\\\").replace('"', '\\"')


def parse_semver(version: str) -> tuple[str, str, str, str]:
    m = re.match(r"^(\d+)\.(\d+)\.(\d+)(?:-(.*))?$", version)
    if not m:
        return ("0", "0", "0", "")
    return (m.group(1), m.group(2), m.group(3), m.group(4) or "")

def group_deps(deps: list[BuckDep]) -> tuple[list[str], dict[str, list[str]], dict[str, str]]:
    """
    Group deps into (base_deps, conditional_deps, named_deps) for emitting in BUCK.
    """
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

        # Handle renamed deps (Cargo `package = ...` style).
        if d.local_name != d.crate_name:
            named_deps[d.local_name] = d.label

    base_deps = sorted(set(base_deps))
    for k in list(conditional_deps.keys()):
        conditional_deps[k] = sorted(set(conditional_deps[k]))

    return (base_deps, conditional_deps, named_deps)


def label_for_workspace_pkg(crate_rel_dir: str, rule_name: str) -> str:
    # crate_rel_dir is relative to codex-rs/, but buck labels are rooted at the
    # repo root, so we prefix with //codex-rs.
    return f"{CODEX_RS_LABEL_PREFIX}/{crate_rel_dir}:{rule_name}"


def label_for_third_party(pkg_name: str, version: str) -> str:
    # Reindeer generates versioned third-party targets and we disable the
    # unversioned alias layer.
    return f"{CODEX_RS_LABEL_PREFIX}/third-party:{pkg_name}-{version}"

def collect_snap_resources(crate_dir: pathlib.Path) -> dict[str, str]:
    """
    Collect `.snap` files for insta snapshot tests.

    Buck's rust_test uses an external runner that executes tests from the
    project root with project-relative paths. Many snapshot tests expect to find
    their `.snap` files under `codex-rs/<crate>/...` from that root, which is
    exactly where the rust rules place `resources`.
    """
    resources: dict[str, str] = {}
    for p in sorted(crate_dir.rglob("*.snap")):
        rel = str(p.relative_to(crate_dir))
        resources[rel] = rel
    return resources

def buck_bin_rule_name(pkg_name: str, bin_name: str) -> str:
    # Avoid collisions with the package's rust_library target, which we name
    # after the package (pkg_name).
    if bin_name == pkg_name:
        return f"{bin_name}-bin"
    return bin_name


def main() -> int:
    meta = cargo_metadata()
    packages = meta.get("packages", [])

    # Package ID -> package json
    by_id: dict[str, Any] = {p["id"]: p for p in packages}

    # Workspace member IDs -> crate directory relative to codex-rs/
    workspace_members: list[str] = list(meta.get("workspace_members") or [])
    workspace_dirs: dict[str, str] = {}
    for pkg_id in workspace_members:
        pkg = by_id[pkg_id]
        manifest_path = pkg["manifest_path"]
        crate_rel = relpath_from_codex_rs(manifest_path)
        crate_dir = str(pathlib.Path(crate_rel).parent)
        workspace_dirs[pkg_id] = crate_dir

    # Resolve graph nodes for dependency edges (including cfg/platform edges).
    resolve = meta.get("resolve") or {}
    nodes = resolve.get("nodes") or []
    node_by_id: dict[str, Any] = {n["id"]: n for n in nodes}

    # Build a workspace-wide mapping of Cargo binary name -> Buck label for the
    # corresponding rust_binary target. Many integration tests use
    # assert_cmd/escargot helpers that look for CARGO_BIN_EXE_* env vars (set by
    # Cargo) to find binaries; we emulate that under Buck.
    cargo_bin_to_label: dict[str, str] = {}
    for pkg_id in workspace_members:
        pkg = by_id[pkg_id]
        crate_dir_rel = workspace_dirs[pkg_id]
        for bin_t in package_bin_targets(pkg):
            bin_name = bin_t["name"]
            cargo_bin_to_label[bin_name] = label_for_workspace_pkg(
                crate_dir_rel,
                buck_bin_rule_name(pkg["name"], bin_name),
            )

    generated_buck_files: list[pathlib.Path] = []

    for pkg_id in workspace_members:
        pkg = by_id[pkg_id]
        crate_dir_rel = workspace_dirs[pkg_id]
        crate_dir = CODEX_RS_ROOT / crate_dir_rel

        lib_t = package_lib_target(pkg)
        bin_ts = package_bin_targets(pkg)
        test_ts = package_test_targets(pkg)
        if lib_t is None and not bin_ts:
            continue

        node = node_by_id.get(pkg_id) or {}

        normal_deps: list[BuckDep] = []
        dev_deps: list[BuckDep] = []
        for dep in node.get("deps") or []:
            dep_id = dep["pkg"]
            if dep_id == pkg_id:
                # Cargo sometimes models "self dependencies" (commonly as a
                # dev-dependency to enable features for tests). Buck should not
                # treat this as a real crate edge, and using it creates cycles.
                continue
            dep_pkg = by_id.get(dep_id)
            if not dep_pkg:
                continue

            dep_kinds = dep.get("dep_kinds") or []

            local_name = dep.get("name") or dep_pkg["name"]
            crate_name = dep_pkg.get("targets", [{}])[0].get("name") or local_name

            if dep_id in workspace_dirs:
                dep_label = label_for_workspace_pkg(workspace_dirs[dep_id], dep_pkg["name"])
            else:
                dep_label = label_for_third_party(dep_pkg["name"], dep_pkg["version"])

            # A single dependency may appear multiple times with different Cargo
            # cfg(...) selectors (e.g. target.'cfg(target_os = "...")'). Cargo
            # metadata encodes these as multiple dep_kind entries, so we emit a
            # Buck dep entry for each one.
            if not dep_kinds:
                dep_kinds = [{"kind": None, "target": None}]

            for k in dep_kinds:
                kind = k.get("kind")
                if kind not in (None, "normal", "dev"):
                    continue

                dep_entry = BuckDep(
                    label=dep_label,
                    local_name=local_name,
                    crate_name=crate_name,
                    cfg=k.get("target"),
                )
                if kind is None or kind == "normal":
                    normal_deps.append(dep_entry)
                elif kind == "dev":
                    dev_deps.append(dep_entry)

        # Deduplicate while preserving stable output.
        normal_deps = list({(d.label, d.local_name, d.crate_name, d.cfg): d for d in normal_deps}.values())
        dev_deps = list({(d.label, d.local_name, d.crate_name, d.cfg): d for d in dev_deps}.values())

        base_deps, conditional_deps, named_deps = group_deps(normal_deps)
        test_base_deps, test_conditional_deps, test_named_deps = group_deps(normal_deps + dev_deps)

        # Enabled features for this package (as resolved by Cargo).
        features = buckify_features(list(node.get("features") or []))

        edition = pkg.get("edition") or "2021"
        ver_major, ver_minor, ver_patch, ver_pre = parse_semver(pkg.get("version") or "0.0.0")

        # Emulate Cargo's CARGO_BIN_EXE_* env vars for all known workspace
        # binaries. This avoids test helpers trying to discover binaries via a
        # Cargo target directory layout.
        #
        # These are runtime-only and should be provided via `run_env` so we
        # don't accidentally make compile actions depend on executable outputs.
        cargo_bin_env = {f"CARGO_BIN_EXE_{k}": f"$(location {v})" for k, v in sorted(cargo_bin_to_label.items())}
        cargo_run_env = cargo_bin_env

        lines: list[str] = []
        lines.append("# @generated by scripts/gen_buck_first_party.py")
        lines.append("# Regenerate with: (cd codex-rs && python3 scripts/gen_buck_first_party.py)")
        lines.append("")

        # NOTE: We use a broad src glob to support include_str!/include_bytes!
        # and other build-time file reads without crate-specific buckification.
        src_glob = 'glob(["**"], exclude = ["BUCK", "target/**"])'
        snap_resources = collect_snap_resources(crate_dir)

        deps_expr = starlark_deps_expr(base_deps, conditional_deps)

        if lib_t is not None:
            crate_root = relpath_from_codex_rs(lib_t["src_path"])
            crate_root_rel = str(pathlib.Path(crate_root).relative_to(crate_dir.relative_to(CODEX_RS_ROOT)))

            lines.append("rust_library(")
            lines.append(f'    name = "{pkg["name"]}",')
            lines.append(f'    crate = "{lib_t["name"]}",')
            lines.append(f'    crate_root = "{crate_root_rel}",')
            lines.append(f"    srcs = {src_glob},")
            lines.append(f'    edition = "{edition}",')
            lines.append(
                "    env = "
                + starlark_dict(
                    {
                        "CARGO_CRATE_NAME": lib_t["name"],
                        "CARGO_MANIFEST_DIR": ".",
                        # Insta snapshots use a compile-time workspace root
                        # (option_env!("INSTA_WORKSPACE_ROOT")) and otherwise
                        # fall back to cargo metadata, which doesn't work under
                        # Buck's test sandbox. Pin this to repo root.
                        "INSTA_WORKSPACE_ROOT": ".",
                        "CARGO_PKG_AUTHORS": "",
                        "CARGO_PKG_DESCRIPTION": "",
                        "CARGO_PKG_NAME": pkg["name"],
                        "CARGO_PKG_REPOSITORY": "",
                        "CARGO_PKG_VERSION": pkg.get("version") or "0.0.0",
                        "CARGO_PKG_VERSION_MAJOR": ver_major,
                        "CARGO_PKG_VERSION_MINOR": ver_minor,
                        "CARGO_PKG_VERSION_PATCH": ver_patch,
                        "CARGO_PKG_VERSION_PRE": ver_pre,
                    },
                    indent="        ",
                )
                + ","
            )
            if features:
                lines.append(f"    features = {starlark_list(features)},")
            lines.append(f"    deps = {deps_expr},")
            if named_deps:
                items = [f'"{k}": "{v}"' for k, v in sorted(named_deps.items())]
                lines.append("    named_deps = {")
                for it in items:
                    lines.append(f"        {it},")
                lines.append("    },")
            lines.append('    visibility = ["PUBLIC"],')
            lines.append(")")

            # Unit tests for the library (i.e., `#[cfg(test)]` within src/).
            test_deps_expr = starlark_deps_expr(test_base_deps, test_conditional_deps)
            lines.append("")
            lines.append("rust_test(")
            lines.append(f'    name = "{pkg["name"]}-unit-tests",')
            lines.append(f'    crate = "{lib_t["name"]}",')
            lines.append(f'    crate_root = "{crate_root_rel}",')
            lines.append(f"    srcs = {src_glob},")
            lines.append(f'    edition = "{edition}",')
            lines.append(
                "    env = "
                + starlark_dict(
                    {
                        "CARGO_CRATE_NAME": lib_t["name"],
                        "CARGO_MANIFEST_DIR": ".",
                        "INSTA_WORKSPACE_ROOT": ".",
                        "CARGO_PKG_AUTHORS": "",
                        "CARGO_PKG_DESCRIPTION": "",
                        "CARGO_PKG_NAME": pkg["name"],
                        "CARGO_PKG_REPOSITORY": "",
                        "CARGO_PKG_VERSION": pkg.get("version") or "0.0.0",
                        "CARGO_PKG_VERSION_MAJOR": ver_major,
                        "CARGO_PKG_VERSION_MINOR": ver_minor,
                        "CARGO_PKG_VERSION_PATCH": ver_patch,
                        "CARGO_PKG_VERSION_PRE": ver_pre,
                    },
                    indent="        ",
                )
                + ","
            )
            if cargo_run_env:
                lines.append("    run_env = " + starlark_dict(cargo_run_env, indent="        ") + ",")
            if snap_resources:
                lines.append("    resources = " + starlark_dict(snap_resources, indent="        ") + ",")
            if features:
                lines.append(f"    features = {starlark_list(features)},")
            lines.append(f"    deps = {test_deps_expr},")
            if test_named_deps:
                items = [f'"{k}": "{v}"' for k, v in sorted(test_named_deps.items())]
                lines.append("    named_deps = {")
                for it in items:
                    lines.append(f"        {it},")
                lines.append("    },")
            lines.append('    visibility = ["PUBLIC"],')
            lines.append(")")

        for bin_t in bin_ts:
            bin_rule_name = buck_bin_rule_name(pkg["name"], bin_t["name"])
            crate_root = relpath_from_codex_rs(bin_t["src_path"])
            crate_root_rel = str(pathlib.Path(crate_root).relative_to(crate_dir.relative_to(CODEX_RS_ROOT)))

            # Cargo makes the package's library available to binaries in the
            # same package, and package dependencies are also visible.
            bin_base_deps = base_deps
            if lib_t is not None:
                bin_base_deps = [f":{pkg['name']}"] + base_deps
            bin_deps_expr = starlark_deps_expr(bin_base_deps, conditional_deps)

            lines.append("")
            lines.append("rust_binary(")
            lines.append(f'    name = "{bin_rule_name}",')
            lines.append(f'    crate = "{bin_t["name"]}",')
            lines.append(f'    crate_root = "{crate_root_rel}",')
            lines.append(f"    srcs = {src_glob},")
            lines.append(f'    edition = "{edition}",')
            lines.append(
                "    env = "
                + starlark_dict(
                    {
                        "CARGO_CRATE_NAME": bin_t["name"],
                        "CARGO_MANIFEST_DIR": ".",
                        "INSTA_WORKSPACE_ROOT": ".",
                        "CARGO_PKG_AUTHORS": "",
                        "CARGO_PKG_DESCRIPTION": "",
                        "CARGO_PKG_NAME": pkg["name"],
                        "CARGO_PKG_REPOSITORY": "",
                        "CARGO_PKG_VERSION": pkg.get("version") or "0.0.0",
                        "CARGO_PKG_VERSION_MAJOR": ver_major,
                        "CARGO_PKG_VERSION_MINOR": ver_minor,
                        "CARGO_PKG_VERSION_PATCH": ver_patch,
                        "CARGO_PKG_VERSION_PRE": ver_pre,
                    },
                    indent="        ",
                )
                + ","
            )
            if features:
                lines.append(f"    features = {starlark_list(features)},")
            lines.append(f"    deps = {bin_deps_expr},")
            if named_deps:
                items = [f'"{k}": "{v}"' for k, v in sorted(named_deps.items())]
                lines.append("    named_deps = {")
                for it in items:
                    lines.append(f"        {it},")
                lines.append("    },")
            lines.append('    visibility = ["PUBLIC"],')
            lines.append(")")

            # Unit tests for the binary (i.e., `#[cfg(test)]` within src/main.rs).
            bin_test_base_deps = list(test_base_deps)
            if lib_t is not None:
                # Cargo makes the package's library available to binaries; the
                # unit test harness for `src/main.rs` needs that dependency too.
                bin_test_base_deps = [f":{pkg['name']}"] + bin_test_base_deps
            test_deps_expr = starlark_deps_expr(bin_test_base_deps, test_conditional_deps)
            lines.append("")
            lines.append("rust_test(")
            lines.append(f'    name = "{bin_rule_name}-unit-tests",')
            lines.append(f'    crate = "{bin_t["name"]}",')
            lines.append(f'    crate_root = "{crate_root_rel}",')
            lines.append(f"    srcs = {src_glob},")
            lines.append(f'    edition = "{edition}",')
            lines.append(
                "    env = "
                + starlark_dict(
                    {
                        "CARGO_CRATE_NAME": bin_t["name"],
                        "CARGO_MANIFEST_DIR": ".",
                        "INSTA_WORKSPACE_ROOT": ".",
                        "CARGO_PKG_AUTHORS": "",
                        "CARGO_PKG_DESCRIPTION": "",
                        "CARGO_PKG_NAME": pkg["name"],
                        "CARGO_PKG_REPOSITORY": "",
                        "CARGO_PKG_VERSION": pkg.get("version") or "0.0.0",
                        "CARGO_PKG_VERSION_MAJOR": ver_major,
                        "CARGO_PKG_VERSION_MINOR": ver_minor,
                        "CARGO_PKG_VERSION_PATCH": ver_patch,
                        "CARGO_PKG_VERSION_PRE": ver_pre,
                    },
                    indent="        ",
                )
                + ","
            )
            if cargo_run_env:
                lines.append("    run_env = " + starlark_dict(cargo_run_env, indent="        ") + ",")
            if snap_resources:
                lines.append("    resources = " + starlark_dict(snap_resources, indent="        ") + ",")
            if features:
                lines.append(f"    features = {starlark_list(features)},")
            lines.append(f"    deps = {test_deps_expr},")
            if test_named_deps:
                items = [f'"{k}": "{v}"' for k, v in sorted(test_named_deps.items())]
                lines.append("    named_deps = {")
                for it in items:
                    lines.append(f"        {it},")
                lines.append("    },")
            lines.append('    visibility = ["PUBLIC"],')
            lines.append(")")

        # Integration tests under `tests/` (Cargo `kind = ["test"]`).
        #
        # Cargo gives these crates access to both normal + dev dependencies, and
        # also makes the package's library available if it exists.
        if test_ts:
            test_deps_expr = starlark_deps_expr(test_base_deps, test_conditional_deps)
            for test_t in test_ts:
                test_name = test_t["name"]
                if test_name in SKIP_INTEGRATION_TESTS_BY_PACKAGE.get(pkg["name"], set()):
                    continue
                crate_root = relpath_from_codex_rs(test_t["src_path"])
                crate_root_rel = str(pathlib.Path(crate_root).relative_to(crate_dir.relative_to(CODEX_RS_ROOT)))

                integration_base_deps = list(test_base_deps)
                if lib_t is not None:
                    integration_base_deps = [f":{pkg['name']}"] + integration_base_deps
                integration_deps_expr = starlark_deps_expr(integration_base_deps, test_conditional_deps)

                lines.append("")
                lines.append("rust_test(")
                lines.append(f'    name = "{test_name}-integration-test",')
                lines.append(f'    crate = "{test_name}",')
                lines.append(f'    crate_root = "{crate_root_rel}",')
                lines.append(f"    srcs = {src_glob},")
                lines.append(f'    edition = "{edition}",')
                lines.append(
                    "    env = "
                    + starlark_dict(
                        {
                            "CARGO_CRATE_NAME": test_name,
                            "CARGO_MANIFEST_DIR": ".",
                            "INSTA_WORKSPACE_ROOT": ".",
                            "CARGO_PKG_AUTHORS": "",
                            "CARGO_PKG_DESCRIPTION": "",
                            "CARGO_PKG_NAME": pkg["name"],
                            "CARGO_PKG_REPOSITORY": "",
                            "CARGO_PKG_VERSION": pkg.get("version") or "0.0.0",
                            "CARGO_PKG_VERSION_MAJOR": ver_major,
                            "CARGO_PKG_VERSION_MINOR": ver_minor,
                            "CARGO_PKG_VERSION_PATCH": ver_patch,
                            "CARGO_PKG_VERSION_PRE": ver_pre,
                        },
                        indent="        ",
                    )
                    + ","
                )
                if cargo_run_env:
                    lines.append("    run_env = " + starlark_dict(cargo_run_env, indent="        ") + ",")
                if snap_resources:
                    lines.append("    resources = " + starlark_dict(snap_resources, indent="        ") + ",")
                if features:
                    lines.append(f"    features = {starlark_list(features)},")
                lines.append(f"    deps = {integration_deps_expr},")
                if test_named_deps:
                    items = [f'"{k}": "{v}"' for k, v in sorted(test_named_deps.items())]
                    lines.append("    named_deps = {")
                    for it in items:
                        lines.append(f"        {it},")
                    lines.append("    },")
                lines.append('    visibility = ["PUBLIC"],')
                lines.append(")")

        write_buck_file(crate_dir, "\n".join(lines) + "\n")
        generated_buck_files.append(crate_dir / "BUCK")

    buildifier(generated_buck_files)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

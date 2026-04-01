#!/usr/bin/env python3

"""Verify that codex-rs Cargo manifests follow workspace manifest policy.

Checks:
- Crates inherit `[workspace.package]` metadata.
- Crates opt into `[lints] workspace = true`.
- Crate names follow the codex-rs directory naming conventions.
- Workspace manifests do not introduce new workspace crate feature toggles
  while the remaining exceptions are being removed.
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
MANIFEST_FEATURE_EXCEPTIONS = {}
OPTIONAL_DEPENDENCY_EXCEPTIONS = set()
INTERNAL_DEPENDENCY_FEATURE_EXCEPTIONS = {}


def main() -> int:
    internal_package_names = workspace_package_names()
    used_manifest_feature_exceptions: set[str] = set()
    used_optional_dependency_exceptions: set[tuple[str, str, str]] = set()
    used_internal_dependency_feature_exceptions: set[tuple[str, str, str]] = set()
    failures_by_path: dict[str, list[str]] = {}

    for path in manifests_to_verify():
        if errors := manifest_errors(
            path,
            internal_package_names,
            used_manifest_feature_exceptions,
            used_optional_dependency_exceptions,
            used_internal_dependency_feature_exceptions,
        ):
            failures_by_path[manifest_key(path)] = errors

    add_unused_exception_errors(
        failures_by_path,
        used_manifest_feature_exceptions,
        used_optional_dependency_exceptions,
        used_internal_dependency_feature_exceptions,
    )

    if not failures_by_path:
        return 0

    print(
        "Cargo manifests under codex-rs must inherit workspace package metadata, "
        "opt into workspace lints, and avoid introducing new workspace crate "
        "features."
    )
    print(
        "Workspace crate features are disallowed because our Bazel build setup "
        "does not honor them today, which can let issues hidden behind feature "
        "gates go unnoticed, and because they add extra crate build "
        "permutations we want to avoid."
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
    print(
        "Workspace crate features are forbidden; add a targeted exception here "
        "only if there is a deliberate temporary migration in flight."
    )
    print()
    for path in sorted(failures_by_path):
        errors = failures_by_path[path]
        print(f"{path}:")
        for error in errors:
            print(f"  - {error}")

    return 1


def manifest_errors(
    path: Path,
    internal_package_names: set[str],
    used_manifest_feature_exceptions: set[str],
    used_optional_dependency_exceptions: set[tuple[str, str, str]],
    used_internal_dependency_feature_exceptions: set[tuple[str, str, str]],
) -> list[str]:
    manifest = load_manifest(path)
    package = manifest.get("package")
    if not isinstance(package, dict) and path != CARGO_RS_ROOT / "Cargo.toml":
        return []

    errors = []
    if isinstance(package, dict):
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

    path_key = manifest_key(path)
    features = manifest.get("features")
    if features is not None:
        normalized_features = normalize_feature_mapping(features)
        expected_features = MANIFEST_FEATURE_EXCEPTIONS.get(path_key)
        if expected_features is None:
            errors.append(
                "remove `[features]`; new workspace crate features are not allowed"
            )
        else:
            used_manifest_feature_exceptions.add(path_key)
            if normalized_features != expected_features:
                errors.append(
                    "limit `[features]` to the existing exception list while "
                    "workspace crate features are being removed "
                    f"(expected {render_feature_mapping(expected_features)})"
                )

    for section_name, dependencies in dependency_sections(manifest):
        for dependency_name, dependency in dependencies.items():
            if not isinstance(dependency, dict):
                continue

            if dependency.get("optional") is True:
                exception_key = (path_key, section_name, dependency_name)
                if exception_key in OPTIONAL_DEPENDENCY_EXCEPTIONS:
                    used_optional_dependency_exceptions.add(exception_key)
                else:
                    errors.append(
                        "remove `optional = true` from "
                        f"`{dependency_entry_label(section_name, dependency_name)}`; "
                        "new optional dependencies are not allowed because they "
                        "create crate features"
                    )

            if not is_internal_dependency(path, dependency_name, dependency, internal_package_names):
                continue

            dependency_features = dependency.get("features")
            if dependency_features is not None:
                normalized_dependency_features = normalize_string_list(
                    dependency_features
                )
                exception_key = (path_key, section_name, dependency_name)
                expected_dependency_features = (
                    INTERNAL_DEPENDENCY_FEATURE_EXCEPTIONS.get(exception_key)
                )
                if expected_dependency_features is None:
                    errors.append(
                        "remove `features = [...]` from workspace dependency "
                        f"`{dependency_entry_label(section_name, dependency_name)}`; "
                        "new workspace crate feature activations are not allowed"
                    )
                else:
                    used_internal_dependency_feature_exceptions.add(exception_key)
                    if normalized_dependency_features != expected_dependency_features:
                        errors.append(
                            "limit workspace dependency features on "
                            f"`{dependency_entry_label(section_name, dependency_name)}` "
                            "to the existing exception list while workspace crate "
                            "features are being removed "
                            f"(expected {render_string_list(expected_dependency_features)})"
                        )

            if dependency.get("default-features") is False:
                errors.append(
                    "remove `default-features = false` from workspace dependency "
                    f"`{dependency_entry_label(section_name, dependency_name)}`; "
                    "new workspace crate feature toggles are not allowed"
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


def manifest_key(path: Path) -> str:
    return str(path.relative_to(ROOT))


def normalize_feature_mapping(value: object) -> dict[str, tuple[str, ...]] | None:
    if not isinstance(value, dict):
        return None

    normalized = {}
    for key, features in value.items():
        if not isinstance(key, str):
            return None
        normalized_features = normalize_string_list(features)
        if normalized_features is None:
            return None
        normalized[key] = normalized_features
    return normalized


def normalize_string_list(value: object) -> tuple[str, ...] | None:
    if not isinstance(value, list) or not all(isinstance(item, str) for item in value):
        return None
    return tuple(value)


def render_feature_mapping(features: dict[str, tuple[str, ...]]) -> str:
    entries = [
        f"{name} = {render_string_list(items)}" for name, items in features.items()
    ]
    return ", ".join(entries)


def render_string_list(items: tuple[str, ...]) -> str:
    return "[" + ", ".join(f'"{item}"' for item in items) + "]"


def dependency_sections(manifest: dict) -> list[tuple[str, dict]]:
    sections = []
    for section_name in ("dependencies", "dev-dependencies", "build-dependencies"):
        dependencies = manifest.get(section_name)
        if isinstance(dependencies, dict):
            sections.append((section_name, dependencies))

    workspace = manifest.get("workspace")
    if isinstance(workspace, dict):
        workspace_dependencies = workspace.get("dependencies")
        if isinstance(workspace_dependencies, dict):
            sections.append(("workspace.dependencies", workspace_dependencies))

    target = manifest.get("target")
    if not isinstance(target, dict):
        return sections

    for target_name, tables in target.items():
        if not isinstance(tables, dict):
            continue
        for section_name in ("dependencies", "dev-dependencies", "build-dependencies"):
            dependencies = tables.get(section_name)
            if isinstance(dependencies, dict):
                sections.append((f"target.{target_name}.{section_name}", dependencies))

    return sections


def dependency_entry_label(section_name: str, dependency_name: str) -> str:
    return f"[{section_name}].{dependency_name}"


def is_internal_dependency(
    manifest_path: Path,
    dependency_name: str,
    dependency: dict,
    internal_package_names: set[str],
) -> bool:
    package_name = dependency.get("package", dependency_name)
    if isinstance(package_name, str) and package_name in internal_package_names:
        return True

    dependency_path = dependency.get("path")
    if not isinstance(dependency_path, str):
        return False

    resolved_dependency_path = (manifest_path.parent / dependency_path).resolve()
    try:
        resolved_dependency_path.relative_to(CARGO_RS_ROOT)
    except ValueError:
        return False
    return True


def add_unused_exception_errors(
    failures_by_path: dict[str, list[str]],
    used_manifest_feature_exceptions: set[str],
    used_optional_dependency_exceptions: set[tuple[str, str, str]],
    used_internal_dependency_feature_exceptions: set[tuple[str, str, str]],
) -> None:
    for path_key in sorted(
        set(MANIFEST_FEATURE_EXCEPTIONS) - used_manifest_feature_exceptions
    ):
        add_failure(
            failures_by_path,
            path_key,
            "remove the stale `[features]` exception from "
            "`MANIFEST_FEATURE_EXCEPTIONS`",
        )

    for path_key, section_name, dependency_name in sorted(
        OPTIONAL_DEPENDENCY_EXCEPTIONS - used_optional_dependency_exceptions
    ):
        add_failure(
            failures_by_path,
            path_key,
            "remove the stale optional-dependency exception for "
            f"`{dependency_entry_label(section_name, dependency_name)}` from "
            "`OPTIONAL_DEPENDENCY_EXCEPTIONS`",
        )

    for path_key, section_name, dependency_name in sorted(
        set(INTERNAL_DEPENDENCY_FEATURE_EXCEPTIONS)
        - used_internal_dependency_feature_exceptions
    ):
        add_failure(
            failures_by_path,
            path_key,
            "remove the stale internal dependency feature exception for "
            f"`{dependency_entry_label(section_name, dependency_name)}` from "
            "`INTERNAL_DEPENDENCY_FEATURE_EXCEPTIONS`",
        )


def add_failure(failures_by_path: dict[str, list[str]], path_key: str, error: str) -> None:
    failures_by_path.setdefault(path_key, []).append(error)


def workspace_package_names() -> set[str]:
    package_names = set()
    for path in cargo_manifests():
        manifest = load_manifest(path)
        package = manifest.get("package")
        if not isinstance(package, dict):
            continue
        package_name = package.get("name")
        if isinstance(package_name, str):
            package_names.add(package_name)
    return package_names


def load_manifest(path: Path) -> dict:
    return tomllib.loads(path.read_text())


def cargo_manifests() -> list[Path]:
    return sorted(
        path
        for path in CARGO_RS_ROOT.rglob("Cargo.toml")
        if path != CARGO_RS_ROOT / "Cargo.toml"
    )


def manifests_to_verify() -> list[Path]:
    return [CARGO_RS_ROOT / "Cargo.toml", *cargo_manifests()]


if __name__ == "__main__":
    sys.exit(main())

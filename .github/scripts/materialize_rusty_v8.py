#!/usr/bin/env python3

"""Materialize the exact rusty_v8 source tree described by the Codex manifest."""

from __future__ import annotations

import argparse
import configparser
import os
import runpy
import subprocess
import sys
import tomllib
from collections.abc import Mapping
from dataclasses import dataclass
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))

from rusty_v8_artifacts import DEFAULT_MANIFEST_PATH
from rusty_v8_artifacts import RustyV8ArtifactManifest


DEFAULT_WRAPPER_REPOSITORY = "https://github.com/denoland/rusty_v8.git"
DEFAULT_V8_REPOSITORY = "https://github.com/denoland/v8.git"
V8_VERSION_MACROS = (
    "MAJOR_VERSION",
    "MINOR_VERSION",
    "BUILD_NUMBER",
    "PATCH_LEVEL",
)


def run(
    *args: str,
    cwd: Path | None = None,
    capture_output: bool = False,
) -> str:
    completed = subprocess.run(
        args,
        cwd=cwd,
        check=True,
        text=True,
        capture_output=capture_output,
    )
    return completed.stdout.strip() if capture_output else ""


def clone_at_revision(repository: str, revision: str, destination: Path) -> None:
    git_dir = destination / ".git"
    if destination.exists():
        if git_dir.exists():
            origin = run(
                "git",
                "remote",
                "get-url",
                "origin",
                cwd=destination,
                capture_output=True,
            )
            status = run(
                "git",
                "status",
                "--porcelain",
                cwd=destination,
                capture_output=True,
            )
            if origin != repository or status:
                raise SystemExit(
                    f"materialization destination is not a clean clone of "
                    f"{repository}: {destination}"
                )
        elif not destination.is_dir() or any(destination.iterdir()):
            raise SystemExit(
                f"materialization destination already exists: {destination}"
            )
        else:
            destination.rmdir()
    if not git_dir.exists():
        destination.parent.mkdir(parents=True, exist_ok=True)
        run(
            "git",
            "clone",
            "--filter=blob:none",
            "--no-checkout",
            repository,
            str(destination),
        )
    run("git", "fetch", "--depth=1", "origin", revision, cwd=destination)
    run("git", "checkout", "--detach", "FETCH_HEAD", cwd=destination)


@dataclass(frozen=True)
class WrapperSubmodule:
    name: str
    path: Path
    repository: str


def wrapper_submodules(source_root: Path) -> tuple[WrapperSubmodule, ...]:
    parser = configparser.ConfigParser()
    parser.read(source_root / ".gitmodules")
    return tuple(
        WrapperSubmodule(
            name=section.removeprefix('submodule "').removesuffix('"'),
            path=Path(parser[section]["path"]),
            repository=parser[section]["url"],
        )
        for section in parser.sections()
    )


def v8_version(source_root: Path) -> str:
    values: dict[str, str] = {}
    version_header = source_root / "v8" / "include" / "v8-version.h"
    for line in version_header.read_text(encoding="utf-8").splitlines():
        for macro in V8_VERSION_MACROS:
            prefix = f"#define V8_{macro} "
            if line.startswith(prefix):
                values[macro] = line.removeprefix(prefix)
    if values.keys() != set(V8_VERSION_MACROS):
        raise SystemExit(f"could not resolve V8 version from {version_header}")
    return ".".join(values[macro] for macro in V8_VERSION_MACROS)


def wrapper_version(source_root: Path) -> str:
    cargo_toml = tomllib.loads((source_root / "Cargo.toml").read_text(encoding="utf-8"))
    return cargo_toml["package"]["version"]


def wrapper_v8_version(source_root: Path) -> str:
    prefix = "V8 Version: "
    versions = [
        line.removeprefix(prefix)
        for line in (source_root / "README.md").read_text(encoding="utf-8").splitlines()
        if line.startswith(prefix)
    ]
    if len(versions) != 1:
        raise SystemExit("could not resolve wrapper V8 version from README.md")
    return versions[0]


def apply_patch_recipe(
    source_root: Path,
    manifest: RustyV8ArtifactManifest,
    *,
    repo_root: Path = ROOT,
) -> None:
    v8_root = source_root / "v8"
    for patch in manifest.patch_paths(repo_root):
        args = (
            "git",
            "apply",
            "--index",
            "--whitespace=error-all",
            str(patch),
        )
        run(*args[:2], "--check", *args[2:], cwd=v8_root)
        run(*args, cwd=v8_root)
    run("git", "diff", "--cached", "--check", cwd=v8_root)


def load_v8_dependencies(source_root: Path) -> Mapping[str, object]:
    previous_cwd = Path.cwd()
    try:
        os.chdir(source_root)
        namespace = runpy.run_path(str(source_root / "tools" / "v8_deps.py"))
    finally:
        os.chdir(previous_cwd)
    dependencies = namespace.get("deps")
    if not isinstance(dependencies, dict):
        raise SystemExit("rusty_v8 tools/v8_deps.py did not define a dependency map")
    return dependencies


def dependency_revision(dependency: object) -> str:
    if isinstance(dependency, str):
        url = dependency
    elif isinstance(dependency, dict) and isinstance(dependency.get("url"), str):
        url = dependency["url"]
    else:
        raise SystemExit(f"unsupported V8 dependency entry: {dependency!r}")
    if "@" not in url:
        raise SystemExit(f"V8 dependency URL has no revision: {url}")
    return url.rsplit("@", 1)[1]


def wrapper_gitlink_revision(source_root: Path, path: Path) -> str:
    output = run(
        "git",
        "ls-tree",
        "HEAD",
        "--",
        str(path),
        cwd=source_root,
        capture_output=True,
    )
    fields = output.split(maxsplit=3)
    if len(fields) != 4 or fields[1] != "commit":
        raise SystemExit(f"could not resolve wrapper gitlink for {path}: {output}")
    return fields[2]


def materialize_dependencies(source_root: Path) -> None:
    dependencies = load_v8_dependencies(source_root)
    for submodule in wrapper_submodules(source_root):
        if submodule.name == "v8":
            continue

        expected = wrapper_gitlink_revision(source_root, submodule.path)
        if submodule.name != "build" and submodule.name in dependencies:
            expected = dependency_revision(dependencies[submodule.name])

        destination = source_root / submodule.path
        clone_at_revision(submodule.repository, expected, destination)
        run("git", "submodule", "update", "--init", "--recursive", cwd=destination)
        actual = run(
            "git",
            "rev-parse",
            "HEAD",
            cwd=destination,
            capture_output=True,
        )
        if actual != expected:
            raise SystemExit(
                f"dependency {submodule.name} resolved to {actual}, expected {expected}"
            )


def verify_materialized_source(
    source_root: Path,
    manifest: RustyV8ArtifactManifest,
) -> None:
    actual_wrapper_version = wrapper_version(source_root)
    if actual_wrapper_version != manifest.wrapper_version:
        raise SystemExit(
            f"wrapper version {actual_wrapper_version} does not match "
            f"{manifest.wrapper_version}"
        )
    actual_wrapper_v8_version = wrapper_v8_version(source_root)
    if actual_wrapper_v8_version != manifest.wrapper_v8_version:
        raise SystemExit(
            f"wrapper V8 version {actual_wrapper_v8_version} does not match "
            f"{manifest.wrapper_v8_version}"
        )
    actual_v8_version = v8_version(source_root)
    if actual_v8_version != manifest.v8_version:
        raise SystemExit(
            f"V8 version {actual_v8_version} does not match {manifest.v8_version}"
        )
    actual_v8_commit = run(
        "git",
        "rev-parse",
        "HEAD",
        cwd=source_root / "v8",
        capture_output=True,
    )
    if actual_v8_commit != manifest.v8_source_commit:
        raise SystemExit(
            f"V8 commit {actual_v8_commit} does not match "
            f"{manifest.v8_source_commit}"
        )


def materialize(
    output: Path,
    manifest: RustyV8ArtifactManifest,
    *,
    wrapper_repository: str = DEFAULT_WRAPPER_REPOSITORY,
    v8_repository: str = DEFAULT_V8_REPOSITORY,
    repo_root: Path = ROOT,
    sync_dependencies: bool = True,
) -> None:
    clone_at_revision(
        wrapper_repository,
        f"refs/tags/v{manifest.wrapper_version}",
        output,
    )
    clone_at_revision(v8_repository, manifest.v8_source_commit, output / "v8")
    apply_patch_recipe(output, manifest, repo_root=repo_root)
    if sync_dependencies:
        materialize_dependencies(output)
    verify_materialized_source(output, manifest)
    print(manifest.artifact_identity)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument(
        "--manifest",
        type=Path,
        default=DEFAULT_MANIFEST_PATH,
    )
    parser.add_argument(
        "--wrapper-repository",
        default=DEFAULT_WRAPPER_REPOSITORY,
    )
    parser.add_argument(
        "--v8-repository",
        default=DEFAULT_V8_REPOSITORY,
    )
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    manifest = RustyV8ArtifactManifest.load(
        args.manifest.resolve(),
        repo_root=ROOT,
    )
    materialize(
        args.output.resolve(),
        manifest,
        wrapper_repository=args.wrapper_repository,
        v8_repository=args.v8_repository,
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

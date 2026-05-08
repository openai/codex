#!/usr/bin/env python3

from __future__ import annotations

import argparse
import gzip
import hashlib
import re
import shutil
import subprocess
import sys
import tomllib
from pathlib import Path

from rusty_v8_module_bazel import (
    RustyV8ChecksumError,
    check_module_bazel,
    rusty_v8_http_file_versions,
    update_module_bazel,
)


ROOT = Path(__file__).resolve().parents[2]
MODULE_BAZEL = ROOT / "MODULE.bazel"
RUSTY_V8_CHECKSUMS_DIR = ROOT / "third_party" / "v8"
RELEASE_ARTIFACT_PROFILE = "release"
SANDBOX_ARTIFACT_PROFILE = "ptrcomp_sandbox_release"
ARTIFACT_BAZEL_CONFIGS = ["rusty-v8-upstream-libcxx"]


def bazel_execroot() -> Path:
    result = subprocess.run(
        ["bazel", "info", "execution_root"],
        cwd=ROOT,
        check=True,
        capture_output=True,
        text=True,
    )
    return Path(result.stdout.strip())


def bazel_output_base() -> Path:
    result = subprocess.run(
        ["bazel", "info", "output_base"],
        cwd=ROOT,
        check=True,
        capture_output=True,
        text=True,
    )
    return Path(result.stdout.strip())


def bazel_output_path(path: str) -> Path:
    if path.startswith("external/"):
        return bazel_output_base() / path
    return bazel_execroot() / path


def bazel_output_files(
    platform: str,
    labels: list[str],
    compilation_mode: str = "fastbuild",
    bazel_configs: list[str] | None = None,
) -> list[Path]:
    expression = "set(" + " ".join(labels) + ")"
    bazel_configs = bazel_configs or []
    result = subprocess.run(
        [
            "bazel",
            "cquery",
            "-c",
            compilation_mode,
            f"--platforms=@llvm//platforms:{platform}",
            *[f"--config={config}" for config in bazel_configs],
            "--output=files",
            expression,
        ],
        cwd=ROOT,
        check=True,
        capture_output=True,
        text=True,
    )
    return [bazel_output_path(line.strip()) for line in result.stdout.splitlines() if line.strip()]


def bazel_build(
    platform: str,
    labels: list[str],
    compilation_mode: str = "fastbuild",
    bazel_configs: list[str] | None = None,
) -> None:
    bazel_configs = bazel_configs or []
    subprocess.run(
        [
            "bazel",
            "build",
            "-c",
            compilation_mode,
            f"--platforms=@llvm//platforms:{platform}",
            *[f"--config={config}" for config in bazel_configs],
            *labels,
        ],
        cwd=ROOT,
        check=True,
    )


def ensure_bazel_output_files(
    platform: str,
    labels: list[str],
    compilation_mode: str = "fastbuild",
    bazel_configs: list[str] | None = None,
) -> list[Path]:
    # Bazel output paths can be reused across config flips, so existence alone
    # does not prove the files match the requested flags.
    bazel_build(platform, labels, compilation_mode, bazel_configs)
    outputs = bazel_output_files(platform, labels, compilation_mode, bazel_configs)
    missing = [str(path) for path in outputs if not path.exists()]
    if missing:
        raise SystemExit(f"missing built outputs for {labels}: {missing}")
    return outputs


def artifact_bazel_configs(bazel_configs: list[str] | None = None) -> list[str]:
    configured = list(ARTIFACT_BAZEL_CONFIGS)
    for config in bazel_configs or []:
        if config not in configured:
            configured.append(config)
    return configured


def release_pair_label(target: str, sandbox: bool = False) -> str:
    target_suffix = target.replace("-", "_")
    pair_kind = "sandbox_release_pair" if sandbox else "release_pair"
    return f"//third_party/v8:rusty_v8_{pair_kind}_{target_suffix}"


def resolved_v8_crate_version() -> str:
    cargo_lock = tomllib.loads((ROOT / "codex-rs" / "Cargo.lock").read_text())
    versions = sorted(
        {
            package["version"]
            for package in cargo_lock["package"]
            if package["name"] == "v8"
        }
    )
    if len(versions) == 1:
        return versions[0]
    if len(versions) > 1:
        raise SystemExit(f"expected exactly one resolved v8 version, found: {versions}")

    module_bazel = (ROOT / "MODULE.bazel").read_text()
    matches = sorted(
        set(
            re.findall(
                r'https://static\.crates\.io/crates/v8/v8-([0-9]+\.[0-9]+\.[0-9]+)\.crate',
                module_bazel,
            )
        )
    )
    if len(matches) != 1:
        raise SystemExit(
            "expected exactly one pinned v8 crate version in MODULE.bazel, "
            f"found: {matches}"
        )
    return matches[0]


def rusty_v8_checksum_manifest_path(version: str) -> Path:
    return RUSTY_V8_CHECKSUMS_DIR / f"rusty_v8_{version.replace('.', '_')}.sha256"


def command_version(version: str | None) -> str:
    if version is not None:
        return version

    manifest_versions = rusty_v8_http_file_versions(MODULE_BAZEL.read_text())
    if len(manifest_versions) == 1:
        return manifest_versions[0]
    if len(manifest_versions) > 1:
        raise SystemExit(
            "expected at most one rusty_v8 http_file version in MODULE.bazel, "
            f"found: {manifest_versions}; pass --version explicitly"
        )

    return resolved_v8_crate_version()


def command_manifest_path(manifest: Path | None, version: str) -> Path:
    if manifest is None:
        return rusty_v8_checksum_manifest_path(version)
    if manifest.is_absolute():
        return manifest
    return ROOT / manifest


def staged_archive_name(target: str, source_path: Path, artifact_profile: str) -> str:
    if target.endswith("-pc-windows-msvc"):
        return f"rusty_v8_{artifact_profile}_{target}.lib.gz"
    return f"librusty_v8_{artifact_profile}_{target}.a.gz"


def staged_binding_name(target: str, artifact_profile: str) -> str:
    return f"src_binding_{artifact_profile}_{target}.rs"


def staged_checksums_name(target: str, artifact_profile: str) -> str:
    return f"rusty_v8_{artifact_profile}_{target}.sha256"


def stage_artifacts(
    target: str,
    lib_path: Path,
    binding_path: Path,
    output_dir: Path,
    sandbox: bool,
) -> None:
    missing_paths = [str(path) for path in [lib_path, binding_path] if not path.exists()]
    if missing_paths:
        raise SystemExit(f"missing release outputs for {target}: {missing_paths}")

    output_dir.mkdir(parents=True, exist_ok=True)
    artifact_profile = SANDBOX_ARTIFACT_PROFILE if sandbox else RELEASE_ARTIFACT_PROFILE
    staged_library = output_dir / staged_archive_name(target, lib_path, artifact_profile)
    staged_binding = output_dir / staged_binding_name(target, artifact_profile)

    with lib_path.open("rb") as src, staged_library.open("wb") as dst:
        with gzip.GzipFile(
            filename="",
            mode="wb",
            fileobj=dst,
            compresslevel=6,
            mtime=0,
        ) as gz:
            shutil.copyfileobj(src, gz)

    shutil.copyfile(binding_path, staged_binding)

    staged_checksums = output_dir / staged_checksums_name(target, artifact_profile)
    with staged_checksums.open("w", encoding="utf-8") as checksums:
        for path in [staged_library, staged_binding]:
            digest = hashlib.sha256()
            with path.open("rb") as artifact:
                for chunk in iter(lambda: artifact.read(1024 * 1024), b""):
                    digest.update(chunk)
            checksums.write(f"{digest.hexdigest()}  {path.name}\n")

    print(staged_library)
    print(staged_binding)
    print(staged_checksums)


def stage_release_pair(
    platform: str,
    target: str,
    output_dir: Path,
    compilation_mode: str = "fastbuild",
    bazel_configs: list[str] | None = None,
    sandbox: bool = False,
) -> None:
    bazel_configs = artifact_bazel_configs(bazel_configs)
    outputs = ensure_bazel_output_files(
        platform,
        [release_pair_label(target, sandbox)],
        compilation_mode,
        bazel_configs,
    )

    try:
        lib_path = next(path for path in outputs if path.suffix in {".a", ".lib"})
    except StopIteration as exc:
        raise SystemExit(f"missing static library output for {target}") from exc

    try:
        binding_path = next(path for path in outputs if path.suffix == ".rs")
    except StopIteration as exc:
        raise SystemExit(f"missing Rust binding output for {target}") from exc

    stage_artifacts(target, lib_path, binding_path, output_dir, sandbox)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    subparsers = parser.add_subparsers(dest="command", required=True)

    stage_release_pair_parser = subparsers.add_parser("stage-release-pair")
    stage_release_pair_parser.add_argument("--platform", required=True)
    stage_release_pair_parser.add_argument("--target", required=True)
    stage_release_pair_parser.add_argument("--output-dir", required=True)
    stage_release_pair_parser.add_argument("--sandbox", action="store_true")
    stage_release_pair_parser.add_argument(
        "--bazel-config",
        action="append",
        default=[],
        dest="bazel_configs",
    )
    stage_release_pair_parser.add_argument(
        "--compilation-mode",
        default="fastbuild",
        choices=["fastbuild", "opt", "dbg"],
    )

    subparsers.add_parser("resolved-v8-crate-version")

    check_module_bazel_parser = subparsers.add_parser("check-module-bazel")
    check_module_bazel_parser.add_argument("--version")
    check_module_bazel_parser.add_argument("--manifest", type=Path)
    check_module_bazel_parser.add_argument(
        "--module-bazel",
        type=Path,
        default=MODULE_BAZEL,
    )

    update_module_bazel_parser = subparsers.add_parser("update-module-bazel")
    update_module_bazel_parser.add_argument("--version")
    update_module_bazel_parser.add_argument("--manifest", type=Path)
    update_module_bazel_parser.add_argument(
        "--module-bazel",
        type=Path,
        default=MODULE_BAZEL,
    )

    return parser.parse_args()


def main() -> int:
    args = parse_args()
    if args.command == "stage-release-pair":
        stage_release_pair(
            platform=args.platform,
            target=args.target,
            output_dir=Path(args.output_dir),
            compilation_mode=args.compilation_mode,
            bazel_configs=args.bazel_configs,
            sandbox=args.sandbox,
        )
        return 0
    if args.command == "resolved-v8-crate-version":
        print(resolved_v8_crate_version())
        return 0
    if args.command == "check-module-bazel":
        version = command_version(args.version)
        manifest_path = command_manifest_path(args.manifest, version)
        try:
            check_module_bazel(args.module_bazel, manifest_path, version)
        except RustyV8ChecksumError as exc:
            raise SystemExit(str(exc)) from exc
        return 0
    if args.command == "update-module-bazel":
        version = command_version(args.version)
        manifest_path = command_manifest_path(args.manifest, version)
        try:
            update_module_bazel(args.module_bazel, manifest_path, version)
        except RustyV8ChecksumError as exc:
            raise SystemExit(str(exc)) from exc
        return 0
    raise SystemExit(f"unsupported command: {args.command}")


if __name__ == "__main__":
    sys.exit(main())

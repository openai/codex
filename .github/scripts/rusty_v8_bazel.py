#!/usr/bin/env python3

from __future__ import annotations

import argparse
import gzip
import hashlib
import re
import shutil
import subprocess
import sys
import tempfile
import tomllib
from pathlib import Path

from rusty_v8_module_bazel import (
    RustyV8ChecksumError,
    check_module_bazel,
    update_module_bazel,
)


ROOT = Path(__file__).resolve().parents[2]
MODULE_BAZEL = ROOT / "MODULE.bazel"
RUSTY_V8_CHECKSUMS_DIR = ROOT / "third_party" / "v8"
MUSL_RUNTIME_ARCHIVE_LABELS = [
    "@llvm//runtimes/libcxx:libcxx.static",
    "@llvm//runtimes/libcxx:libcxxabi.static",
]
LLVM_AR_LABEL = "@llvm//tools:llvm-ar"
LLVM_RANLIB_LABEL = "@llvm//tools:llvm-ranlib"


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
) -> list[Path]:
    expression = "set(" + " ".join(labels) + ")"
    result = subprocess.run(
        [
            "bazel",
            "cquery",
            "-c",
            compilation_mode,
            f"--platforms=@llvm//platforms:{platform}",
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
) -> None:
    subprocess.run(
        [
            "bazel",
            "build",
            "-c",
            compilation_mode,
            f"--platforms=@llvm//platforms:{platform}",
            *labels,
        ],
        cwd=ROOT,
        check=True,
    )


def ensure_bazel_output_files(
    platform: str,
    labels: list[str],
    compilation_mode: str = "fastbuild",
) -> list[Path]:
    outputs = bazel_output_files(platform, labels, compilation_mode)
    if all(path.exists() for path in outputs):
        return outputs

    bazel_build(platform, labels, compilation_mode)
    outputs = bazel_output_files(platform, labels, compilation_mode)
    missing = [str(path) for path in outputs if not path.exists()]
    if missing:
        raise SystemExit(f"missing built outputs for {labels}: {missing}")
    return outputs


def release_pair_label(target: str) -> str:
    target_suffix = target.replace("-", "_")
    return f"//third_party/v8:rusty_v8_release_pair_{target_suffix}"


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
    return resolved_v8_crate_version()


def command_manifest_path(manifest: Path | None, version: str) -> Path:
    if manifest is None:
        return rusty_v8_checksum_manifest_path(version)
    if manifest.is_absolute():
        return manifest
    return ROOT / manifest


def staged_archive_name(target: str, source_path: Path) -> str:
    if source_path.suffix == ".lib":
        return f"rusty_v8_release_{target}.lib.gz"
    return f"librusty_v8_release_{target}.a.gz"


def is_musl_archive_target(target: str, source_path: Path) -> bool:
    return target.endswith("-unknown-linux-musl") and source_path.suffix == ".a"


def single_bazel_output_file(
    platform: str,
    label: str,
    compilation_mode: str = "fastbuild",
) -> Path:
    outputs = ensure_bazel_output_files(platform, [label], compilation_mode)
    if len(outputs) != 1:
        raise SystemExit(f"expected exactly one output for {label}, found {outputs}")
    return outputs[0]


def merged_musl_archive(
    platform: str,
    lib_path: Path,
    compilation_mode: str = "fastbuild",
) -> Path:
    llvm_ar = single_bazel_output_file(platform, LLVM_AR_LABEL, compilation_mode)
    llvm_ranlib = single_bazel_output_file(platform, LLVM_RANLIB_LABEL, compilation_mode)
    runtime_archives = [
        single_bazel_output_file(platform, label, compilation_mode)
        for label in MUSL_RUNTIME_ARCHIVE_LABELS
    ]

    temp_dir = Path(tempfile.mkdtemp(prefix="rusty-v8-musl-stage-"))
    merged_archive = temp_dir / lib_path.name
    merge_commands = "\n".join(
        [
            f"create {merged_archive}",
            f"addlib {lib_path}",
            *[f"addlib {archive}" for archive in runtime_archives],
            "save",
            "end",
        ]
    )
    subprocess.run(
        [str(llvm_ar), "-M"],
        cwd=ROOT,
        check=True,
        input=merge_commands,
        text=True,
    )
    subprocess.run([str(llvm_ranlib), str(merged_archive)], cwd=ROOT, check=True)
    return merged_archive


def stage_release_pair(
    platform: str,
    target: str,
    output_dir: Path,
    compilation_mode: str = "fastbuild",
) -> None:
    outputs = ensure_bazel_output_files(
        platform,
        [release_pair_label(target)],
        compilation_mode,
    )

    try:
        lib_path = next(path for path in outputs if path.suffix in {".a", ".lib"})
    except StopIteration as exc:
        raise SystemExit(f"missing static library output for {target}") from exc

    try:
        binding_path = next(path for path in outputs if path.suffix == ".rs")
    except StopIteration as exc:
        raise SystemExit(f"missing Rust binding output for {target}") from exc

    output_dir.mkdir(parents=True, exist_ok=True)
    staged_library = output_dir / staged_archive_name(target, lib_path)
    staged_binding = output_dir / f"src_binding_release_{target}.rs"
    source_archive = (
        merged_musl_archive(platform, lib_path, compilation_mode)
        if is_musl_archive_target(target, lib_path)
        else lib_path
    )

    with source_archive.open("rb") as src, staged_library.open("wb") as dst:
        with gzip.GzipFile(
            filename="",
            mode="wb",
            fileobj=dst,
            compresslevel=6,
            mtime=0,
        ) as gz:
            shutil.copyfileobj(src, gz)

    shutil.copyfile(binding_path, staged_binding)

    staged_checksums = output_dir / f"rusty_v8_release_{target}.sha256"
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


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    subparsers = parser.add_subparsers(dest="command", required=True)

    stage_release_pair_parser = subparsers.add_parser("stage-release-pair")
    stage_release_pair_parser.add_argument("--platform", required=True)
    stage_release_pair_parser.add_argument("--target", required=True)
    stage_release_pair_parser.add_argument("--output-dir", required=True)
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

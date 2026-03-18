#!/usr/bin/env python3

from __future__ import annotations

import argparse
import gzip
import shutil
import subprocess
import sys
import tempfile
import tomllib
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
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


def bazel_output_files(platform: str, labels: list[str]) -> list[Path]:
    expression = "set(" + " ".join(labels) + ")"
    result = subprocess.run(
        [
            "bazel",
            "cquery",
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


def bazel_build(platform: str, labels: list[str]) -> None:
    subprocess.run(
        [
            "bazel",
            "build",
            f"--platforms=@llvm//platforms:{platform}",
            *labels,
        ],
        cwd=ROOT,
        check=True,
    )


def ensure_bazel_output_files(platform: str, labels: list[str]) -> list[Path]:
    outputs = bazel_output_files(platform, labels)
    if all(path.exists() for path in outputs):
        return outputs

    bazel_build(platform, labels)
    outputs = bazel_output_files(platform, labels)
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
    if len(versions) != 1:
        raise SystemExit(f"expected exactly one resolved v8 version, found: {versions}")
    return versions[0]


def staged_archive_name(target: str, source_path: Path) -> str:
    if source_path.suffix == ".lib":
        return f"rusty_v8_release_{target}.lib.gz"
    return f"librusty_v8_release_{target}.a.gz"


def is_musl_archive_target(target: str, source_path: Path) -> bool:
    return target.endswith("-unknown-linux-musl") and source_path.suffix == ".a"


def single_bazel_output_file(platform: str, label: str) -> Path:
    outputs = ensure_bazel_output_files(platform, [label])
    if len(outputs) != 1:
        raise SystemExit(f"expected exactly one output for {label}, found {outputs}")
    return outputs[0]


def merged_musl_archive(platform: str, lib_path: Path) -> Path:
    llvm_ar = single_bazel_output_file(platform, LLVM_AR_LABEL)
    llvm_ranlib = single_bazel_output_file(platform, LLVM_RANLIB_LABEL)
    runtime_archives = [
        single_bazel_output_file(platform, label)
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


def stage_release_pair(platform: str, target: str, output_dir: Path) -> None:
    outputs = ensure_bazel_output_files(platform, [release_pair_label(target)])

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
        merged_musl_archive(platform, lib_path)
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

    print(staged_library)
    print(staged_binding)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    subparsers = parser.add_subparsers(dest="command", required=True)

    stage_release_pair_parser = subparsers.add_parser("stage-release-pair")
    stage_release_pair_parser.add_argument("--platform", required=True)
    stage_release_pair_parser.add_argument("--target", required=True)
    stage_release_pair_parser.add_argument("--output-dir", required=True)

    subparsers.add_parser("resolved-v8-crate-version")

    return parser.parse_args()


def main() -> int:
    args = parse_args()
    if args.command == "stage-release-pair":
        stage_release_pair(
            platform=args.platform,
            target=args.target,
            output_dir=Path(args.output_dir),
        )
        return 0
    if args.command == "resolved-v8-crate-version":
        print(resolved_v8_crate_version())
        return 0
    raise SystemExit(f"unsupported command: {args.command}")


if __name__ == "__main__":
    sys.exit(main())

#!/usr/bin/env python3

from __future__ import annotations

import argparse
import gzip
import shutil
import subprocess
import sys
import tomllib
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]


def bazel_execroot() -> Path:
    result = subprocess.run(
        ["bazel", "info", "execution_root"],
        cwd=ROOT,
        check=True,
        capture_output=True,
        text=True,
    )
    return Path(result.stdout.strip())


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
    execroot = bazel_execroot()
    return [execroot / line.strip() for line in result.stdout.splitlines() if line.strip()]


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


def stage_release_pair(platform: str, target: str, output_dir: Path) -> None:
    outputs = bazel_output_files(platform, [release_pair_label(target)])

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

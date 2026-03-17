#!/usr/bin/env python3

from __future__ import annotations

import argparse
import gzip
import shutil
import subprocess
import sys
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


def platform_label(platform: str) -> str:
    if platform.startswith(("@", "//")):
        return platform
    return f"@llvm//platforms:{platform}"


def bazel_output_files(platform: str, labels: list[str]) -> list[Path]:
    expression = "set(" + " ".join(labels) + ")"
    result = subprocess.run(
        [
            "bazel",
            "cquery",
            f"--platforms={platform_label(platform)}",
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
    raise SystemExit(f"unsupported command: {args.command}")


if __name__ == "__main__":
    sys.exit(main())

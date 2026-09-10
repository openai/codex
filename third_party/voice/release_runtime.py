"""Stage verified voice libraries and seal their public-release receipt."""

import argparse
import json
from pathlib import Path
import shutil

from package_runtime import runtime_files
from runtime import digest


def stage(source: Path, destination: Path, target: str) -> None:
    if target not in {
        "aarch64-apple-darwin",
        "x86_64-apple-darwin",
        "aarch64-unknown-linux-gnu",
        "x86_64-unknown-linux-gnu",
        "aarch64-pc-windows-msvc",
        "x86_64-pc-windows-msvc",
    }:
        raise ValueError("unsupported public release voice runtime target")
    source = source.resolve(strict=True)
    files = runtime_files(source, target)
    destination.mkdir()
    try:
        for relative, expected in files.items():
            copied = destination / relative
            copied.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(source / relative, copied)
            if digest(copied) != expected:
                raise ValueError("runtime changed while staging release inputs")
    except BaseException:
        shutil.rmtree(destination)
        raise


def seal(root: Path, target: str) -> None:
    root = root.resolve(strict=True)
    manifest_path = root / "runtime.json"
    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    if manifest.get("developmentOnly") is not True or manifest.get("target") != target:
        raise ValueError("expected an unsealed development receipt for this target")
    for record in manifest["libraries"]:
        record["sha256"] = digest(root / record["path"])
    manifest["developmentOnly"] = False
    manifest["distribution"] = "publicRelease"
    manifest_path.chmod(manifest_path.stat().st_mode | 0o200)
    manifest_path.write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")
    runtime_files(root, target, public_release=True)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("operation", choices=("stage", "seal"))
    parser.add_argument("--target", required=True)
    parser.add_argument("--source", type=Path)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    if args.operation == "stage":
        if args.source is None:
            parser.error("stage requires --source")
        stage(args.source, args.output, args.target)
    else:
        seal(args.output, args.target)

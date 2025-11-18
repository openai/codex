"""Bundle the PyRouette + StagePort demo assets into a zip archive.

The script copies the curated files in ``demo/full_package`` to a target
output directory and compresses them into a single zip. Use this to ship a
ready-to-demo bundle that includes Docker deployment assets, curriculum
artifacts, and outreach templates.
"""
from __future__ import annotations

import argparse
import shutil
import zipfile
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parent.parent
DEMO_SOURCE = REPO_ROOT / "demo" / "full_package"


def copy_demo_files(target_dir: Path, *, source_dir: Path = DEMO_SOURCE) -> None:
    """Copy the curated demo files to ``target_dir``."""

    if not source_dir.exists():
        raise FileNotFoundError(f"Demo source directory missing: {source_dir}")

    target_dir.mkdir(parents=True, exist_ok=True)
    for path in source_dir.iterdir():
        if path.is_file():
            shutil.copy2(path, target_dir / path.name)


def create_zip(source_dir: Path, zip_path: Path) -> Path:
    """Create a zip archive from ``source_dir`` and return the archive path."""

    zip_path.parent.mkdir(parents=True, exist_ok=True)
    with zipfile.ZipFile(zip_path, "w", zipfile.ZIP_DEFLATED) as archive:
        for file_path in source_dir.rglob("*"):
            if file_path.is_file():
                archive.write(file_path, file_path.relative_to(source_dir))
    return zip_path


def main() -> None:
    parser = argparse.ArgumentParser(description="Generate the PyRouette + StagePort demo package zip.")
    parser.add_argument(
        "--output-dir",
        type=Path,
        default=REPO_ROOT / "dist" / "demo_full_package",
        help="Directory where the demo files and zip archive should be written.",
    )
    parser.add_argument(
        "--source-dir",
        type=Path,
        default=DEMO_SOURCE,
        help="Source directory containing the curated demo files to bundle.",
    )
    parser.add_argument(
        "--zip-name",
        type=str,
        help="Name of the generated zip file (defaults to <output-dir>.zip).",
    )
    args = parser.parse_args()

    copy_demo_files(args.output_dir, source_dir=args.source_dir)

    zip_name = args.zip_name or f"{args.output_dir.name}.zip"
    zip_path = args.output_dir.parent / zip_name
    archive = create_zip(args.output_dir, zip_path)

    print(f"Demo files copied to: {args.output_dir}")
    print(f"Archive created at: {archive}")


if __name__ == "__main__":
    main()

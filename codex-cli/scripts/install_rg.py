#!/usr/bin/env python3
"""Download ripgrep binaries into the codex-cli bin directory."""

from __future__ import annotations

import argparse
import importlib.util
from pathlib import Path

SCRIPT_DIR = Path(__file__).resolve().parent


def _load_rg_utils():
    spec = importlib.util.spec_from_file_location(
        "rg_utils", SCRIPT_DIR / "rg_utils.py"
    )
    if spec is None or spec.loader is None:
        raise RuntimeError("Unable to load rg_utils module")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


rg_utils = _load_rg_utils()
DEFAULT_RG_TARGETS = rg_utils.DEFAULT_RG_TARGETS
detect_current_target = rg_utils.detect_current_target
fetch_rg = rg_utils.fetch_rg


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Install ripgrep binaries.")
    parser.add_argument(
        "--bin-dir",
        type=Path,
        required=True,
        help="Directory where ripgrep binaries should be written.",
    )
    parser.add_argument(
        "--target",
        action="append",
        dest="targets",
        help="Codex target triple to fetch. Repeatable. Defaults to all.",
    )
    parser.add_argument(
        "--current-only",
        action="store_true",
        help="Download ripgrep only for the current platform.",
    )
    return parser.parse_args()


def main() -> int:
    args = parse_args()

    if args.current_only and args.targets:
        print("Cannot combine --current-only with explicit --target options.", file=sys.stderr)
        return 1

    if args.current_only:
        current = detect_current_target()
        if current is None:
            print("Unable to detect current platform for ripgrep download.", file=sys.stderr)
            return 1
        targets = [current]
    elif args.targets:
        targets = args.targets
    else:
        targets = DEFAULT_RG_TARGETS

    fetch_rg(args.bin_dir, targets)
    return 0


if __name__ == "__main__":
    sys.exit(main())

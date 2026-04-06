#!/usr/bin/env python3
"""Notarization API key rotation helper for Codex CLI GitHub Actions.

Dry-run is the default. Pass `--execute` (alias: `-x`) to run mutating commands.

This intentionally keeps only the API-key path from the mac signing rotation
helper:
  - import a manually-created App Store Connect API key,
  - optionally store it in 1Password,
  - upload the matching GitHub Actions repository secrets.
"""

from __future__ import annotations

import argparse
import functools
from typing import Sequence

from cmd_import_api_key import register as register_import_api_key
from cmd_upload_api_key_secrets import register as register_upload_api_key_secrets
from rotation_shared import DEFAULT_OUT_DIR


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description=__doc__,
        allow_abbrev=False,
        formatter_class=argparse.RawDescriptionHelpFormatter,
    )
    parser.add_argument(
        "-x",
        "--execute",
        action="store_true",
        help="Run mutating commands (default is dry-run)",
    )
    parser.add_argument(
        "-v",
        "--verbose",
        dest="log_verbose",
        action="store_true",
        help="Print command execution/dry-run progress logs",
    )
    parser.add_argument(
        "-o",
        "--out-dir",
        default=str(DEFAULT_OUT_DIR),
        help="Output directory for generated artifacts",
    )

    subparsers = parser.add_subparsers(
        dest="subcommand",
        required=True,
        parser_class=functools.partial(argparse.ArgumentParser, allow_abbrev=False),
    )

    register_import_api_key(subparsers)
    register_upload_api_key_secrets(subparsers)

    return parser


def main(argv: Sequence[str] | None = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)
    args.func(args)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

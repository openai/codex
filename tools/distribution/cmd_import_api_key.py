from __future__ import annotations

import argparse

from rotation_shared import (
    DEFAULT_API_KEY_METADATA_JSON_FILENAME,
    DEFAULT_ONE_PASSWORD_ACCOUNT,
    DEFAULT_ONE_PASSWORD_API_KEY_TITLE_BASENAME,
    cmd_import_api_key,
)


def register(subparsers: argparse._SubParsersAction[argparse.ArgumentParser]) -> None:
    parser = subparsers.add_parser(
        "import-api-key",
        help="Import a manually created App Store Connect API key for mac notarization",
    )
    parser.add_argument(
        "--private-key-path", required=True, help="Path to the downloaded AuthKey_<KEY_ID>.p8 file"
    )
    parser.add_argument(
        "--issuer-id", required=True, help="App Store Connect issuer ID for the imported key"
    )
    parser.add_argument(
        "--key-id", required=True, help="App Store Connect key ID for the imported key"
    )
    parser.add_argument(
        "--role",
        choices=["developer", "app-manager"],
        help="Optional role metadata to record alongside the imported key",
    )
    parser.add_argument(
        "--one-password-vault", help="Optional 1Password vault to store the API key item in"
    )
    parser.add_argument(
        "--one-password-title",
        help=(
            "Optional 1Password item title to create or update. Defaults to "
            f"'{DEFAULT_ONE_PASSWORD_API_KEY_TITLE_BASENAME} <YYYY-MM-DD>' when --one-password-vault is set"
        ),
    )
    parser.add_argument(
        "--metadata-json-filename",
        default=DEFAULT_API_KEY_METADATA_JSON_FILENAME,
        help="Filename for the imported API key metadata",
    )
    parser.add_argument(
        "--one-password-account",
        default=DEFAULT_ONE_PASSWORD_ACCOUNT,
        help="1Password account to use for item creation",
    )
    parser.set_defaults(func=cmd_import_api_key)

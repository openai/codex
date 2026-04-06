from __future__ import annotations

import argparse

from rotation_shared import (
    DEFAULT_GITHUB_REPO,
    DEFAULT_ISSUER_ID_SECRET_NAME,
    DEFAULT_KEY_ID_SECRET_NAME,
    DEFAULT_ONE_PASSWORD_ACCOUNT,
    DEFAULT_ONE_PASSWORD_PASSWORD_FIELD,
    DEFAULT_PRIVATE_KEY_SECRET_NAME,
    cmd_upload_api_key_secrets,
)


def register(subparsers: argparse._SubParsersAction[argparse.ArgumentParser]) -> None:
    parser = subparsers.add_parser(
        "upload-api-key-secrets",
        help="Upload the mac notarization App Store Connect API key material to GitHub Actions secrets",
    )
    parser.add_argument("--github-repo", default=DEFAULT_GITHUB_REPO)
    parser.add_argument(
        "--one-password-item",
        required=True,
        help="1Password item ID or title for the notarization API key",
    )
    parser.add_argument(
        "--one-password-vault",
        required=True,
        help="1Password vault containing the notarization API key item",
    )
    parser.add_argument(
        "--one-password-account",
        default=DEFAULT_ONE_PASSWORD_ACCOUNT,
        help="1Password account to use when loading the API key item",
    )
    parser.add_argument(
        "--private-key-field",
        default=DEFAULT_ONE_PASSWORD_PASSWORD_FIELD,
        help="1Password field containing the base64 private key",
    )
    parser.add_argument(
        "--issuer-id-field", default="issuer_id", help="1Password field containing the issuer ID"
    )
    parser.add_argument(
        "--key-id-field", default="key_id", help="1Password field containing the key ID"
    )
    parser.add_argument("--issuer-id-secret-name", default=DEFAULT_ISSUER_ID_SECRET_NAME)
    parser.add_argument("--key-id-secret-name", default=DEFAULT_KEY_ID_SECRET_NAME)
    parser.add_argument("--private-key-secret-name", default=DEFAULT_PRIVATE_KEY_SECRET_NAME)
    parser.set_defaults(func=cmd_upload_api_key_secrets)
